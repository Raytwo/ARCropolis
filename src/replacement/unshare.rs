use smash_arc::*;

use super::lookup;
use super::extensions::*;
use crate::config;
use crate::hashes;

fn unshare_file(ctx: &mut AdditionContext, hash: Hash40) {
    if !lookup::is_shared_file(hash) {
        trace!("File '{}' ({:#x}) did not need to be unshared.", hashes::find(hash), hash.0);
        return;
    }

    let shared_file = match ctx.get_shared_file(hash) {
        Ok(filepath_idx) => filepath_idx,
        Err(e) => {
            warn!("Failed to find shared file for '{}' ({:#x}) in the unsharing lookup. This file will not be unshared.", hashes::find(hash), hash.0);
            return;
        }   
    };
    
    let (dir_hash, idx) = match lookup::get_dir_entry_for_file(hash) {
        Some(val) => val,
        None => {
            warn!("Failed to find '{}' ({:#x}) in the unsharing lookup. This file will not be unshared.", hashes::find(hash), hash.0);
            return;
        }
    };

    let dir_info = match ctx.get_dir_info_from_hash(dir_hash) {
        Ok(dir) => *dir,
        Err(e) => {
            warn!("Failed to find directory for '{}' ({:#x}). This file will not be unshared.", hashes::find(hash), hash.0);
            return;
        }
    };

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

    dir_file_info.file_info_indice_index = new_info_indice_idx;
    dir_file_info.info_to_data_index = new_info_to_data_idx;
    dir_file_info.flags = new_file_info.flags;

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
    while let Some(hash) = hashes.next() {
        unshare_file(ctx, hash);
    }
}