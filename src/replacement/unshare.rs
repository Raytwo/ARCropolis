use smash_arc::*;

use super::lookup;
use super::extensions::*;
use crate::config;
use crate::hashes;

fn reshare_dependent_files(ctx: &mut AdditionContext, hash: Hash40) {
    // First, I need to create a unique filepath which will not conflict with any other path
    // in the game (this is important for later when we resort the HashToIndex based off of the filepaths)
    // To do this, we simply set its length to a length impossible to find in the base data.arc
    // Since the latter 4 bytes are the CRC32, it's safe to assume that this will be unique
    let shared_file_path_index = match ctx.get_file_path_index_from_hash(hash) {
        Ok(idx) => idx,
        Err(e) => {
            error!(
                "Failed to find the path index when resharing dependent files on '{}' ({:#x}). This will probably cause infinite loads.",
                hashes::find(hash),
                hash.0
            );
            return;
        }
    };

    // Get the number of shared files from our lookup, if there are none then we shouldn't even be here, so print out a warning
    let shared_file_count = lookup::get_shared_file_count(hash);
    if shared_file_count == 0 {
        warn!("Attempted to reshare dependent files on file '{}' ({:#x}), which has no shared files!", hashes::find(hash), hash.0);
        return;
    }

    let mut new_filepath = ctx.filepaths[usize::from(shared_file_path_index)];
    new_filepath.path.set_length(0xFF);

    // We need to copy over the loading structure for the shared file into a new InfoIdx and new FileInfo
    // We needn't worry about the FileInfoToFileData or FileData, since we actually create new ones when unsharing instead, so it doesn't matter
    let new_filepath_idx = FilePathIdx(ctx.filepaths.len() as u32);
    let new_info_indice_idx = FileInfoIndiceIdx(ctx.file_info_indices.len() as u32);
    let new_info_idx = FileInfoIdx(ctx.file_infos.len() as u32);

    // set the new filepath's InfoIndiceIdx to the index of the new one
    new_filepath.path.set_index(new_info_indice_idx.0);

    ctx.file_info_indices.push(FileInfoIndex {
        dir_offset_index: 0xFF_FFFF,
        file_info_index: new_info_idx
    });

    // we can safely unwrap here, since we are guaranteed to have our file path index since we found it above
    let mut new_file_info = *ctx.get_file_info_from_hash(hash).unwrap(); 
    new_file_info.file_path_index = new_filepath_idx;
    new_file_info.file_info_indice_index = new_info_indice_idx;

    ctx.file_infos.push(new_file_info);
    ctx.filepaths.push(new_filepath);

    // Modify the load path of each of the files that are shared to this one
    for dependent_hash in (0..shared_file_count).map(|x| lookup::get_shared_file(hash, x).unwrap()) {
        // Get the DirInfo and the child index of the dependent hash, if it doesn't exist... then just move on to the next one ig
        let (dir_hash, child_idx) = match lookup::get_dir_entry_for_file(dependent_hash) {
            Some(entry) => entry,
            None => {
                error!(
                    "Failed to find directory entry for file '{}' ({:#x}) while trying to reshare it to a new file, separate from '{}' ({:#x}). This file will cause infinite loads.",
                    hashes::find(dependent_hash),
                    dependent_hash.0,
                    hashes::find(hash),
                    hash.0
                );
                continue;
            }
        };

        // Attempt to get the child info range from the dir info so that we can modify the entry in question
        let child_info_range = match ctx.get_dir_info_from_hash(dir_hash) {
            Ok(info) => info.file_info_range(),
            Err(e) => {
                error!(
                    "Failed to find the directory containing file '{}' ({:#x}) while trying to separate it from '{}' ({:#x}). This file will infinite load.",
                    hashes::find(dependent_hash),
                    dependent_hash.0,
                    hashes::find(hash),
                    hash.0
                );
                continue;
            }
        };

        // Get the file info, modify which info indice it points to as well as set the filepath to point to the new info indice as well
        let dependent_info = &mut ctx.file_infos[child_info_range][child_idx];
        let dependent_filepath_index = dependent_info.file_path_index;
        dependent_info.file_info_indice_index = new_info_indice_idx;
        drop(dependent_info);

        ctx.filepaths[usize::from(dependent_filepath_index)].path.set_index(new_info_indice_idx.0);
    }

    // increase the number of filepaths/datas our fs info can handle
    ctx.loaded_filepaths.reserve(1);
    ctx.loaded_datas.reserve(1);
}

fn unshare_file(ctx: &mut AdditionContext, hash: Hash40) {
    // Check if the file is stored in our lookup table (the `is_shared_search` field)
    if !lookup::is_shared_file(hash) {
        trace!("File '{}' ({:#x}) did not need to be unshared.", hashes::find(hash), hash.0);
        return;
    }

    // Get the shared file path index from the LoadedArc
    // If it's missing, just early return
    let shared_file = match ctx.get_shared_file(hash) {
        Ok(filepath_idx) => filepath_idx,
        Err(e) => {
            warn!("Failed to find shared file for '{}' ({:#x}) in the unsharing lookup. This file will not be unshared.", hashes::find(hash), hash.0);
            return;
        }   
    };
    
    // Grab the directory file info entry from our unsharing lookup, if it's missing then early return
    let (dir_hash, idx) = match lookup::get_dir_entry_for_file(hash) {
        Some(val) => val,
        None => {
            warn!("Failed to find '{}' ({:#x}) in the unsharing lookup. This file will not be unshared.", hashes::find(hash), hash.0);
            return;
        }
    };

    // Lookup the directory from the cache, if it's missing then early return
    let dir_info = match ctx.get_dir_info_from_hash(dir_hash) {
        Ok(dir) => *dir,
        Err(e) => {
            warn!("Failed to find directory for '{}' ({:#x}). This file will not be unshared.", hashes::find(hash), hash.0);
            return;
        }
    };

    // Get the current filepath index for the hash to unshare
    // If it is equal to the shared path index, then we know that this is the source file and we need
    // To reshare all of the files which depend on it
    match ctx.get_file_path_index_from_hash(hash) {
        Ok(current_path_index) if current_path_index == shared_file => reshare_dependent_files(ctx, hash),
        Ok(_) => {},
        Err(e) => {
            warn!("Failed to find path index for file '{}' ({:#x}) when attempting to unshare it. This file will not be unshared.", hashes::find(hash), hash.0);
            return;
        },
    }

    let new_info_indice_idx = FileInfoIndiceIdx(ctx.file_info_indices.len() as u32);
    let new_info_idx = FileInfoIdx(ctx.file_infos.len() as u32);
    let new_info_to_data_idx = InfoToDataIdx(ctx.info_to_datas.len() as u32);
    let new_data_idx = FileDataIdx(ctx.file_datas.len() as u32);

    let mut new_file_info = { // get the shared file info and copy it so that we can make modifications
        let shared_info_indice_idx = ctx.filepaths[usize::from(shared_file)].path.index() as usize;
        let shared_info_idx = ctx.file_info_indices[shared_info_indice_idx].file_info_index;
        ctx.file_infos[usize::from(shared_info_idx)]
    };

    let info_to_data_index = if new_file_info.flags.is_regional() {
        new_file_info.flags.set_is_regional(false);
        usize::from(new_file_info.info_to_data_index) + config::region() as usize
    } else {
        usize::from(new_file_info.info_to_data_index)
    };

    let mut new_info_to_data = ctx.info_to_datas[info_to_data_index];

    let file_data = ctx.file_datas[usize::from(new_info_to_data.file_data_index)];
    ctx.file_datas.push(file_data);

    new_info_to_data.file_data_index = new_data_idx;
    ctx.info_to_datas.push(new_info_to_data);

    let mut dir_file_info = ctx.file_infos[dir_info.file_info_range()][idx];

    new_file_info.flags.set_standalone_file(true); // set the file as standalone so that our ResLoadingThread hook can tell that it needs to be loaded manually
    new_file_info.file_path_index = dir_file_info.file_path_index;
    new_file_info.file_info_indice_index = new_info_indice_idx;
    new_file_info.info_to_data_index = new_info_to_data_idx;

    dir_file_info.flags.set_standalone_file(true);
    dir_file_info.file_info_indice_index = new_info_indice_idx;
    dir_file_info.info_to_data_index = new_info_to_data_idx;
    dir_file_info.flags.set_is_regional(new_file_info.flags.is_regional());


    ctx.file_infos[dir_info.file_info_range()][idx] = dir_file_info;

    ctx.file_infos.push(new_file_info);

    ctx.file_info_indices.push(FileInfoIndex {
        dir_offset_index: 0xFF_FFFF,
        file_info_index: new_info_idx
    });

    
    ctx.filepaths[usize::from(dir_file_info.file_path_index)].path.set_index(new_info_indice_idx.0);

    // we only need to reserve memory here, since none of these are active
    ctx.loaded_datas.reserve(1);
}

pub fn unshare_files(ctx: &mut AdditionContext, mut hashes: impl Iterator<Item = Hash40>) {
    for hash in hashes {
        unshare_file(ctx, hash);
    }
}