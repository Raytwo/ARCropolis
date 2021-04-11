use std::io::Write;

use crate::{
    CONFIG,
    callbacks::{Callback, CallbackFn},
    hashes,
    replacement_files::{CALLBACKS, FileBacking, FileIndex, MOD_FILES, get_region_id},
    runtime::LoadedTables
};

use log::debug;
use owo_colors::OwoColorize;
use smash_arc::{ArcLookup, Hash40, Region};

#[no_mangle]
pub extern "C" fn arcrop_load_file(hash: u64, out_buffer: *mut u8, length: usize) {
    debug!("[Arcropolis-API::load_file] Hash received: {}, Buffer len: {:#x}", hashes::get(Hash40(hash)).green(), length);

    let arc = LoadedTables::get_arc();
    let mut buffer = unsafe { std::slice::from_raw_parts_mut(out_buffer, length) };

    // TODO: Require extra code to handle streams
    let path_idx = arc.get_file_path_index_from_hash(Hash40(hash)).unwrap();
    let info_indice_idx = arc.get_file_info_from_path_index(path_idx).file_info_indice_index;

    // Get the FileCtx for this hash
    if let Some(filectx) = MOD_FILES.read().get(FileIndex::Regular(info_indice_idx)) {
        // Get the callback for this file as well as the file it applies to (either extracted from data.arc or from the SD)
        if let FileBacking::Callback { callback, original } = &filectx.file {
            match &**original {
                // Extract the file from data.arc
                FileBacking::LoadFromArc => {
                    let user_region = smash_arc::Region::from(get_region_id(CONFIG.read().misc.region.as_ref().unwrap()).unwrap() + 1);
                    let content = arc.get_file_contents(hash, user_region).unwrap();
                    buffer.write(&content).unwrap();
                }
                // Use the file on the SD
                FileBacking::Path(modpath) => {
                    let content = std::fs::read(modpath).unwrap();
                    buffer.write(&content).unwrap();
                }
                // Call a parent callback that itself will provide its processed file. Unsupported for now
                FileBacking::Callback { callback: test, original: _ } => {
                    unreachable!()
                    // let cb = test.callback;
                    // cb(hash, buffer.as_mut_ptr(), length as usize);
                }
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn arcrop_register_callback(hash: u64, length: usize, cb: CallbackFn) {
    debug!("[Arcropolis-API::register_callback] Hash received: '{}'", hashes::get(Hash40(hash)).green());

    let mut callbacks = CALLBACKS.write();

    let bigger_length = if let Some(previous_cb) =  callbacks.get(&Hash40(hash)) {
        // Always get the larger length for patching to accomodate the callback that requires the biggest buffer
        if previous_cb.len > length as u32 { previous_cb.len }  else { length as u32 }
    } else {
        length as u32
    };

    let callback = Callback {
        callback: cb,
        len: bigger_length,
        fallback: Box::new(FileBacking::LoadFromArc),
    };

    // Overwrite the previous callback. Could probably be done better.
    callbacks.insert(Hash40(hash), callback);
}

#[no_mangle]
pub extern "C" fn arcrop_api_version() {
    debug!("[Arcropolis-API::api_version] Function called");
    unimplemented!()
}