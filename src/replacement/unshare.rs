use std::{collections::HashSet, ops::Range};

use once_cell::sync::Lazy;
use smash_arc::*;

use super::{extensions::*, lookup};
use crate::{
    config, hashes,
    resource::{self, LoadedFilepath},
};

pub static SHARED_FILE_INDEX: Lazy<u32> = Lazy::new(|| resource::arc().get_shared_data_index());

fn reshare_dependent_files(ctx: &mut AdditionContext, hash_ignore: &HashSet<Hash40>, hash: Hash40) {
    info!("Attempting to reshare files dependent on '{}' ({:#x})", hashes::find(hash), hash.0);
    // First, I need to create a unique filepath which will not conflict with any other path
    // in the game (this is important for later when we resort the HashToIndex based off of the filepaths)
    // To do this, we simply set its length to a length impossible to find in the base data.arc
    // Since the latter 4 bytes are the CRC32, it's safe to assume that this will be unique
    let shared_file_path_index = match ctx.get_file_path_index_from_hash(hash) {
        Ok(idx) => idx,
        Err(_) => {
            error!(
                "Failed to find the path index when resharing dependent files on '{}' ({:#x}). This will probably cause infinite loads.",
                hashes::find(hash),
                hash.0
            );
            return
        },
    };

    // Get the number of shared files from our lookup, if there are none then we shouldn't even be here, so print out a warning
    // If you read in `unshare_file`, you'll see that we remove files we unshare from the shared file cache, this is where that comes into play
    // If we unshare files ourselves, they won't pop up in here. It's fine if we reshare some files and then unshare them again, it just means that
    // at runtime the file tables will be a little messy but nothing the game can't handle
    let shared_file_count = lookup::get_shared_file_count(hash);
    if shared_file_count == 0 {
        warn!(
            "Attempted to reshare dependent files on file '{}' ({:#x}), which has no shared files!",
            hashes::find(hash),
            hash.0
        );
        return
    }

    // Here we set the length to 255, because no path in the game even comes close to that long we should be fine.
    let mut new_filepath = ctx.filepaths[usize::from(shared_file_path_index)];
    new_filepath.path.set_length(0xFF);

    // We need to copy over the loading structure for the shared file into a new InfoIdx and new FileInfo
    // We needn't worry about the FileInfoToFileData or FileData, since we actually create new ones when unsharing instead, so it doesn't matter
    let new_filepath_idx = FilePathIdx(ctx.filepaths.len() as u32);
    let new_info_indice_idx = FileInfoIndiceIdx(ctx.file_info_indices.len() as u32);
    let new_info_idx = FileInfoIdx(ctx.file_infos.len() as u32);
    let new_info_to_data_idx = InfoToDataIdx(ctx.info_to_datas.len() as u32);
    let new_data_idx = FileDataIdx(ctx.file_datas.len() as u32);

    // set the new filepath's InfoIndiceIdx to the index of the new one
    new_filepath.path.set_index(new_info_indice_idx.0);

    // Create a new FileInfoIndex with a directory offset that doesn't matter and a file info index that we are pointing to
    ctx.file_info_indices.push(FileInfoIndex {
        dir_offset_index: 0xFF_FFFF,
        file_info_index: new_info_idx,
    });

    // we can safely unwrap here, since we are guaranteed to have our file path index since we found it above
    // Unlike unsharing, we actually are creating a new FilePath here, since we have to reshare to something that won't get unshared later.
    // This is why we change the filepath index to something new instead of what is in the DirInfo
    let mut new_file_info = *ctx.get_file_info_from_hash(hash).unwrap();
    new_file_info.file_path_index = new_filepath_idx;
    new_file_info.file_info_indice_index = new_info_indice_idx;
    new_file_info.flags.set_standalone_file(true);

    // Do the same as unsharing where we get the right InfoToData depending on regional
    // NOTE: NEITHER THIS CODE NOR THE CODE BELOW IS EQUIPPED TO HANDLE THE OTHER KIND OF REGIONAL FILES
    let info_to_data_index = if new_file_info.flags.is_regional() {
        new_file_info.flags.set_is_regional(false);
        usize::from(new_file_info.info_to_data_index) + config::region() as usize
    } else {
        usize::from(new_file_info.info_to_data_index)
    };

    // Copy the old InfoToData and also set the index of the new one in our file info
    let mut new_file_info_to_data = ctx.info_to_datas[info_to_data_index];

    new_file_info.info_to_data_index = new_info_to_data_idx;

    // Clone the old file info and push it back onto our context
    // This old file info should never get changed.
    let new_data = ctx.file_datas[usize::from(new_file_info_to_data.file_data_index)];
    ctx.file_datas.push(new_data);

    // Modify our InfoToData to point to the new FileData we have created
    new_file_info_to_data.file_data_index = new_data_idx;

    // Push the remainder of our structures. We only have to create one new FilePath -> InfoIdx -> ... -> FileData chain
    // since we are going to just be redirecting all of the dependent files to this new one
    ctx.info_to_datas.push(new_file_info_to_data);
    ctx.file_infos.push(new_file_info);
    ctx.filepaths.push(new_filepath);

    // Modify the load path of each of the files that are shared to this one
    // Only get the shared files as well, this is a secondary check to confirm that we aren't
    // resharing files that we have already unshared
    for dependent_hash in (0..shared_file_count).filter_map(|x| {
        let hash = lookup::get_shared_file(hash, x).unwrap();
        if lookup::is_shared_file(hash) {
            Some(hash)
        } else {
            None
        }
    }) {
        // Don't worry about files in our ignore list
        // If this seems confusing, note that the `hash_ignore` comes from files that we do our preprocessing on (i.e. Dark Samus models for victory screen)
        if hash_ignore.contains(&dependent_hash) {
            continue
        }
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
                continue
            },
        };

        // Attempt to get the child info range from the dir info so that we can modify the entry in question
        let child_info_range = match ctx.get_dir_info_from_hash(dir_hash) {
            Ok(info) => info.file_info_range(),
            Err(_) => {
                error!(
                    "Failed to find the directory containing file '{}' ({:#x}) while trying to separate it from '{}' ({:#x}). This file will infinite load.",
                    hashes::find(dependent_hash),
                    dependent_hash.0,
                    hashes::find(hash),
                    hash.0
                );
                continue
            },
        };

        // Get the file info, modify which info indice it points to as well as set the filepath to point to the new info indice as well
        // We have to modify the InfoIndiceIdx in case this file gets loaded as part of a directory, else it will point to the old file
        // file that we are definitely going to be patching, leading to potential decompression issues or UB
        let dependent_info = &mut ctx.file_infos[child_info_range][child_idx];
        let dependent_filepath_index = dependent_info.file_path_index;
        dependent_info.file_info_indice_index = new_info_indice_idx;
        dependent_info.flags.set_standalone_file(true);

        info!(
            "Reshared file '{}' ({:#x}), which depended on '{}' ({:#x})",
            hashes::find(dependent_hash),
            dependent_hash.0,
            hashes::find(hash),
            hash.0
        );

        // Finally set the FileInfoIndiceIdx on the FilePath to our new reshared one, this is important in case the file gets loaded as a singular file
        // (and also the game loads many of the files via filepath anyways, even if they are in a directory)
        ctx.filepaths[usize::from(dependent_filepath_index)].path.set_index(new_info_indice_idx.0);
    }

    // increase the number of filepaths/datas our fs info can handle
    // Funnily enough, we have to actually push on LoadedFilepaths, because just allocating space for it is bad as it won't clear the data if there
    // are zero references
    ctx.loaded_filepaths.push(LoadedFilepath::default());
    ctx.loaded_datas.reserve(1);
}

fn unshare_file(ctx: &mut AdditionContext, hash_ignore: &HashSet<Hash40>, hash: Hash40) {
    // Ignore the provided hash if it is contained in our list of ignored files
    if hash_ignore.contains(&hash) {
        return
    }

    // Check if the file is stored in our lookup table (the `is_shared_search` field)
    if !lookup::is_shared_file(hash) {
        trace!("File '{}' ({:#x}) did not need to be unshared.", hashes::find(hash), hash.0);
        return
    }

    // Get the shared file path index from the LoadedArc
    // If it's missing, just early return
    let shared_file = match ctx.get_file_path_index_from_hash(hash) {
        Ok(filepath_idx) => {
            ctx.file_infos[usize::from(ctx.file_info_indices[ctx.filepaths[usize::from(filepath_idx)].path.index() as usize].file_info_index)]
                .file_path_index
        },
        Err(_) => {
            warn!(
                "Failed to find filepath index for '{}' ({:#x}). This file will not be unshared.",
                hashes::find(hash),
                hash.0
            );
            return
        },
    };

    // Grab the directory file info entry from our unsharing lookup, if it's missing then early return
    let (dir_hash, idx) = match lookup::get_dir_entry_for_file(hash) {
        Some(val) => val,
        None => {
            warn!(
                "Failed to find '{}' ({:#x}) in the unsharing lookup. This file will not be unshared.",
                hashes::find(hash),
                hash.0
            );
            return
        },
    };

    // Lookup the directory from the cache, if it's missing then early return
    let dir_info = match ctx.get_dir_info_from_hash(dir_hash) {
        Ok(dir) => *dir,
        Err(_) => {
            warn!(
                "Failed to find directory for '{}' ({:#x}). This file will not be unshared.",
                hashes::find(hash),
                hash.0
            );
            return
        },
    };

    // Get the current filepath index for the hash to unshare
    // If it is equal to the shared path index, then we know that this is the source file and we need
    // To reshare all of the files which depend on it
    match ctx.get_file_path_index_from_hash(hash) {
        Ok(current_path_index) if current_path_index == shared_file => {
            reshare_dependent_files(ctx, hash_ignore, hash);
            let file_info = &mut ctx.file_infos
                [usize::from(ctx.file_info_indices[ctx.filepaths[usize::from(current_path_index)].path.index() as usize].file_info_index)];
            file_info.flags.set_standalone_file(true);
            if ctx.arc.get_file_in_folder(file_info, config::region()).file_data_index.0 < *SHARED_FILE_INDEX {
                return
            }
        },
        Ok(_) => {},
        Err(_) => {
            warn!(
                "Failed to find path index for file '{}' ({:#x}) when attempting to unshare it. This file will not be unshared.",
                hashes::find(hash),
                hash.0
            );
            return
        },
    }

    // Precalculate all of our new indices, since we reuse them multiple times and are actively changing the addition ctx
    let new_info_indice_idx = FileInfoIndiceIdx(ctx.file_info_indices.len() as u32);
    let new_info_idx = FileInfoIdx(ctx.file_infos.len() as u32);
    let new_info_to_data_idx = InfoToDataIdx(ctx.info_to_datas.len() as u32);
    let new_data_idx = FileDataIdx(ctx.file_datas.len() as u32);

    // get the shared file info and copy it so that we can make modifications
    // we copy it here so that we grab the same flags from the original file, else we will run into issues like textures not loading properly
    // or effects crashing the game (although effects aren't shared...)
    let mut new_file_info = {
        let shared_info_indice_idx = ctx.filepaths[usize::from(shared_file)].path.index() as usize;
        let shared_info_idx = ctx.file_info_indices[shared_info_indice_idx].file_info_index;
        ctx.file_infos[usize::from(shared_info_idx)]
    };

    // Get the proper InfoToData for the file if it is regional.
    let info_to_data_index = if new_file_info.flags.is_regional() {
        new_file_info.flags.set_is_regional(false);
        usize::from(new_file_info.info_to_data_index) + config::region() as usize
    } else {
        usize::from(new_file_info.info_to_data_index)
    };

    // Copy the info to data to preserve valid FileData and Folder offset information, else game will crash or infinite load on file load
    let mut new_info_to_data = ctx.info_to_datas[info_to_data_index];

    // Copy and repush the file data, since we will manually patch it later
    let file_data = ctx.file_datas[usize::from(new_info_to_data.file_data_index)];
    ctx.file_datas.push(file_data);

    // Change the file data idx and push on to our new InfoToData
    new_info_to_data.file_data_index = new_data_idx;
    ctx.info_to_datas.push(new_info_to_data);

    // We need to get the directory file info (file infos in directories are not always the same as standalone files)
    // to modify its `standalone_file` flag (this is one that ARCropolis adds)
    // This isn't technically necessary but since we are here anyways it does help. This information is also stored in one of the
    // cache files
    let mut dir_file_info = ctx.file_infos[dir_info.file_info_range()][idx];

    // Before pushing the new file info, we want to change following data as follows:
    //  * `standalone_file` will always be true here, since this file information won't load properly unless loaded as an individual file.
    //      This is important because all shared files are valid to be loaded as standalone files (pass the information to ResLoadingThread as File
    //      vs. as part of a DirInfo set), but they are not guaranteed to have valid DirInfo information. This fact is also what will cause issues
    //      when trying to unshare/add files on the raw data.arc archive, since DirInfos require all of their file data to either be shared or stored
    //      in sequence
    //  * `unshared_nus3bank` is another flag that ARCropolis uses in order to patch NUS3BANKs for unshared NUS3AUDIO files. This is to enable one-slot
    //      voice mods without packaging a vanilla nus3bank, while modded/replaced nus3banks still work just the same.
    //  * `file_path_index` needs to be changed to point to the same file_path_index as the directory file info. This makes sure that if we have a file,
    //      say, "fighter/marth/model/body/c00/model.numdlb" which we are unsharing from "fighter/marth/model/body/c02/model.numdlb", when the
    //      ResLoadingThread goes to do it's redirects, it gets sent back to c00 instead of c02. This is how vanilla unshared files behave as well.
    //      Another way to think about it is with this diagram:
    //      File 1 ------> File 2 ------>
    //                        ^         |
    //                        |         |  <------ This is for shared files, where File 1 is shared to File 2. This chain can have up to 3 unique files in it
    //                        |         |
    //                        -----------
    //      File 1 ------>
    //         ^         |
    //         |         |  <------- This is for unshared files, they always will point back into themselves
    //         |         |
    //         -----------
    //  * 'file_info_indice_index' needs to point to the same info indice index that we are changing the unshared filepath to. Shared filepaths
    //      will always be how the loader threads progress through the file chain (see above), meaning in order to unshare we have to kill that
    //      chain right at the root and redirect it to the right file info index
    //  * 'info_to_data_index' needs to point to the right InfoToData (and therefore, right FileData) because after the LoadedArc takes the
    //      context, we will need individual paths to patch
    new_file_info.flags.set_standalone_file(true);
    new_file_info
        .flags
        .set_unshared_nus3bank(ctx.filepaths[usize::from(dir_file_info.file_path_index)].ext.hash40() == Hash40::from("nus3bank"));
    new_file_info.file_path_index = dir_file_info.file_path_index;
    new_file_info.file_info_indice_index = new_info_indice_idx;
    new_file_info.info_to_data_index = new_info_to_data_idx;

    dir_file_info.file_info_indice_index = new_info_indice_idx;
    dir_file_info.flags.set_standalone_file(true);

    // Recopy the new formmated FileInfo back into the DirInfo's children
    ctx.file_infos[dir_info.file_info_range()][idx] = dir_file_info;

    // Finally push the new file info, make the new FileInfoIndex and push that as well
    ctx.file_infos.push(new_file_info);

    ctx.file_info_indices.push(FileInfoIndex {
        dir_offset_index: 0xFF_FFFF,
        file_info_index: new_info_idx,
    });

    // Patch the now-unshared FilePath to point to our unshared file chain
    ctx.filepaths[usize::from(dir_file_info.file_path_index)]
        .path
        .set_index(new_info_indice_idx.0);

    // we only need to reserve memory here, since none of these are active
    ctx.loaded_datas.reserve(1);

    // The reasoning for this is that there is something called "source" files, which is basically the only file in the
    // shared file chain that contains the actual data. For example, let's say that Marth's source file for his model's `model.numdlb`
    // is in slot 3 ("fighter/marth/model/body/c03/model.numdlb"). Well, if we need to unshare two files, c00/model.numdlb and c03/model.numdlb,
    // what happens if we encounter c00's first? Well, if we unshare it and *don't* remove it from the shared file cache, then
    // when we come across c03's, we would "reshare it" (basically generate and add a new file to share it against so we can properly unshare c03)
    // and would kill the unsharing and cause undefined behavior. If it's unshared properly, then *we* should consider it as such and not have to keep
    // track of it
    lookup::remove_shared_file(hash);
    ctx.file_infos[usize::from(ctx.file_info_indices[ctx.filepaths[usize::from(shared_file)].path.index() as usize].file_info_index)]
        .flags
        .set_standalone_file(true);
    let shared_hash = ctx.filepaths[usize::from(shared_file)].path.hash40();
    info!(
        "Unshared file '{}' ({:#x}) from '{}' ({:#x})",
        hashes::find(hash),
        hash.0,
        hashes::find(shared_hash),
        shared_hash.0
    );
}

fn reshare_file_group(ctx: &mut AdditionContext, dir_info: Range<usize>, file_group: Range<usize>) {
    // What blujay calls a "FileGroup" is what smash-arc calls a "FolderOffset", it can point to either FileInfos *or* FileDatas
    // Every DirInfo points to a FolderOffset which contains FileDatas, and that FolderOffset redirects to the "shared FileGroup",
    // which contains the *true FileInfos* of each file. Usually these are shared files in the `model` folders of characters and stages.
    // This proved to be problematic when these files would get unshared/reshared, because the game would load invalid data from an invalid address
    // To remedy this, a quick workaround can be developed by telling the game to load all of these as standalone files (this forces the game to load
    // based off of FilePath instead of sequential InfoToDatas).
    let referenced_file_infos: HashSet<FileInfoIdx> = file_group
        .clone()
        .into_iter()
        .map(|x| ctx.get_shared_info_index(FileInfoIdx(x as u32)))
        .collect();
    for dir_index in dir_info {
        let dir_index = FileInfoIdx(dir_index as u32);
        let shared_idx = ctx.get_shared_info_index(dir_index);
        if shared_idx == dir_index
            || (ctx
                .get_file_in_folder(&ctx.file_infos[usize::from(shared_idx)], config::region())
                .file_data_index
                .0
                < *SHARED_FILE_INDEX
                && !file_group.contains(&usize::from(shared_idx))
                && !referenced_file_infos.contains(&shared_idx))
        {
            continue
        }

        ctx.file_infos[usize::from(dir_index)].flags.set_standalone_file(true);
    }
}

pub fn reshare_file_groups(ctx: &mut AdditionContext) {
    let arc = resource::arc();
    // Iterate through each DirInfo in the LoadedArc and check if it points to a shared FileGroup. If it does, we want to kill that link
    // to prevent i-loads or crashes
    for dir_info in arc.get_dir_infos() {
        if let Some(RedirectionType::Shared(file_group)) = arc.get_directory_dependency(dir_info) {
            if file_group.directory_index != 0xFF_FFFF {
                reshare_file_group(ctx, dir_info.file_info_range(), file_group.range());
                // Tell the game that there is no shared FileGroup here because we remove it's purpose
                resource::arc_mut().get_folder_offsets_mut()[dir_info.path.index() as usize].directory_index = 0xFF_FFFF;
            }
        }
    }
}

pub fn unshare_files(ctx: &mut AdditionContext, hash_ignore: HashSet<Hash40>, hashes: impl Iterator<Item = Hash40>) {
    for hash in hashes {
        unshare_file(ctx, &hash_ignore, hash);
    }
}
