use smash_arc::*;
use crate::{
    resource,
    config,
    hashes
};
use owo_colors::OwoColorize;

#[no_mangle]
pub extern "C" fn arcrop_load_file(
    hash: Hash40,
    out_buffer: *mut u8,
    buf_length: usize,
    out_size: &mut usize
) -> bool {
    debug!(
        "arcrop_load_file -> Hash received: {} ({:#x}), Buffer len: {:#x}",
        hashes::find(hash).green(),
        hash.0,
        buf_length
    );
    
    let mut buffer = unsafe {
        std::slice::from_raw_parts_mut(
            out_buffer,
            buf_length
        )
    };

    // This function is intended to only be called by an arc api, which means that we have already write locked the thread and cannot read lock it
    if let Some(size) = unsafe { (*crate::GLOBAL_FILESYSTEM.data_ptr()).load_into(hash, &mut buffer) } {
        *out_size = size;
        debug!("arcrop_load_file -> Successfully loaded file. Bytes read: {:#x}", size);
        true
    } else {
        *out_size = 0;
        debug!("arcrop_load_file -> Failed to read file!");
        false
    }
}

#[no_mangle]
pub extern "C" fn arcrop_get_decompressed_size(hash: Hash40, out_size: &mut usize) -> bool {
    debug!("arcrop_get_decompressed_size -> Received hash {} ({:#x})", hashes::find(hash).green(), hash.0);
    if !resource::initialized() {
        false
    } else {
        resource::arc()
            .get_file_data_from_hash(hash, config::region())
            .map_or_else(|_| false, |x| {
                *out_size = x.decomp_size as usize;
                true
            })
    }
}

#[no_mangle]
pub extern "C" fn arcrop_get_loaded_arc(out: &mut &'static LoadedArc) -> bool {
    debug!("arcrop_get_loaded_arc -> Sending loaded arc");
    if !resource::initialized() {
        false
    } else {
        *out = resource::arc();
        true
    }
}

#[no_mangle]
pub extern "C" fn arcrop_is_file_loaded(hash: Hash40) -> bool {
    debug!("arcrop_is_file_loaded -> Received hash {} ({:#x})", hashes::find(hash).green(), hash.0);
    if !resource::initialized() {
        false
    } else {
        let arc = resource::arc();
        let filesystem_info = resource::filesystem_info();
        match arc.get_file_path_index_from_hash(hash) {
            Ok(file_path_index) => { 
                filesystem_info.get_loaded_filepaths()[file_path_index.0 as usize].is_loaded == 1
            },
            _ => {
                false
            },
        }
    }
}
