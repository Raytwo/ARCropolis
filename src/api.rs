use std::io::Write;

use crate::{
    hashes,
    runtime::LoadedTables,
    replacement_files::{
        FileIndex,
        FileBacking,
        MOD_FILES,
        CALLBACKS,
    },
    callbacks::{Callback, CallbackFn},
};

use log::debug;
use owo_colors::OwoColorize;
use smash_arc::{ArcLookup, Hash40, Region};

#[no_mangle]
pub extern "C" fn arcrop_load_file(hash: u64, out_buffer: *mut u8, length: usize) {
    debug!("[Arcropolis-API::load_file] Hash received: {}, Buffer len: {:#x}", hashes::get(Hash40(hash)).green(), length);

    // Just get the file from data.arc for now
    let arc = LoadedTables::get_arc();
    let mut buffer = unsafe { std::slice::from_raw_parts_mut(out_buffer, length) };
    // "Error while reading magic number"
    let content = arc.get_file_contents(hash, Region::EuFrench).unwrap();
    
    buffer.write(&content).unwrap();

    // Should call the Fallback until we get the content?
    // let path_idx = arc.get_file_path_index_from_hash(hash).unwrap();
    // let info_indice_idx = arc.get_file_info_from_path_index(path_idx).file_info_indice_index;
    
    // match MOD_FILES.read().get(FileIndex::Regular(info_indice_idx)) {
    //     Some(filectx) => {
    //         let test = filectx.get_file_content();
    //         let content = arc.get_file_contents(filectx.hash, Region::None).unwrap();
    //     }
    //     None => panic!("arcrop_load_file is being called for a ModFile that does not exist")
    // }
}

#[no_mangle]
pub extern "C" fn arcrop_register_callback(hash: u64, length: usize, cb: CallbackFn) {
    debug!("[Arcropolis-API::register_callback] Hash received: '{}'", hashes::get(Hash40(hash)).green());

    let mut callbacks = CALLBACKS.write();

    let callback = Callback {
        callback: cb,
        len: length as u32,
        fallback: Box::new(FileBacking::LoadFromArc),
    };

    callbacks.insert(Hash40(hash), callback);
}

#[no_mangle]
pub extern "C" fn arcrop_api_version() {
    debug!("[Arcropolis-API::api_version] Function called");
    unimplemented!()
}