use std::io::Write;

use crate::{CONFIG, callbacks::{Callback, CallbackFn}, hashes, replacement_files::{self, CALLBACKS, FileBacking, FileIndex, MOD_FILES, get_region_id}, runtime::LoadedTables};

use log::debug;
use owo_colors::OwoColorize;
use smash_arc::{ArcLookup, Hash40, Region};

#[no_mangle]
pub extern "C" fn arcrop_load_file(out_size: *mut usize, hash: u64, out_buffer: *mut u8, buf_length: usize) {
    debug!("[Arcropolis-API::load_file] Hash received: {}, Buffer len: {:#x}", hashes::get(Hash40(hash)).green(), buf_length);

    let arc = LoadedTables::get_arc();
    let mut buffer = unsafe { std::slice::from_raw_parts_mut(out_buffer, buf_length) };

    // TODO: Require extra code to handle streams
    let path_idx = arc.get_file_path_index_from_hash(Hash40(hash)).unwrap();
    let info_indice_idx = arc.get_file_info_from_path_index(path_idx).file_info_indice_index;

    // Get the FileCtx for this hash
    if let Some(filectx) = MOD_FILES.read().get(FileIndex::Regular(info_indice_idx)) {
        // Get the callback for this file as well as the file it applies to (either extracted from data.arc or from the SD)
        let content = replacement_files::test(Hash40(hash), &filectx.file);
        unsafe { *out_size = content.len(); }
        buffer.write(&content).unwrap();
    }
}

#[no_mangle]
pub extern "C" fn arcrop_register_callback(hash: u64, length: usize, cb: CallbackFn) {
    debug!("[Arcropolis-API::register_callback] Hash received: '{}'", hashes::get(Hash40(hash)).green());

    let mut callbacks = CALLBACKS.write();

    let callback = if let Some(previous_cb) =  callbacks.get(&Hash40(hash)) {
        // Always get the larger length for patching to accomodate the callback that requires the biggest buffer
        let length = if previous_cb.len > length as u32 { previous_cb.len }  else { length as u32 };

        Callback {
            callback: cb,
            len: length,
            previous: Box::new(FileBacking::Callback {
                callback: previous_cb.clone(),
                original: Box::new(FileBacking::LoadFromArc),
            }),
        }
    } else {
        Callback {
            callback: cb,
            len: length as u32,
            previous: Box::new(FileBacking::LoadFromArc),
        }
    };

    // Overwrite the previous callback. Could probably be done better.
    callbacks.insert(Hash40(hash), callback);
}

#[no_mangle]
pub extern "C" fn arcrop_api_version() {
    debug!("[Arcropolis-API::api_version] Function called");
    unimplemented!()
}