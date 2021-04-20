use std::{
    ffi::CStr,
    io::Write,
    path::PathBuf
};

use log::debug;
use owo_colors::OwoColorize;
use smash_arc::{ArcLookup, Hash40, Region};
use arcropolis_api::{CallbackFn, StreamCallbackFn};

use crate::{CONFIG, callbacks::{Callback, CallbackKind, StreamCallback}, hashes, replacement_files::{
        CALLBACKS,
        MOD_FILES,
        FileBacking,
        FileIndex,
        get_region_id,
        recursive_file_backing_load,
    }, runtime::LoadedTables};

/// NOTE: THIS MUST BE BUMPED ANY TIME THE EXTERNALLY-FACING API IS CHANGED
///
/// How to know which to bump:
/// 
/// Do your changes modify an existing API: Major bump
/// Do your changes only add new APIs in a backwards compatible way: Minor bump
///
/// Are your changes only internal? No version bump
static API_VERSION: ApiVersion = ApiVersion {
    major: 1,
    minor: 1
};

#[no_mangle]
pub extern "C" fn arcrop_load_file(hash: Hash40, out_buffer: *mut u8, buf_length: usize, out_size: &mut usize) -> bool {
    debug!("[Arcropolis-API::load_file] Hash received: {}, Buffer len: {:#x}", hashes::get(hash).green(), buf_length);

    let arc = LoadedTables::get_arc();
    let mut buffer = unsafe { std::slice::from_raw_parts_mut(out_buffer, buf_length) };

    // TODO: Require extra code to handle streams
    let path_idx = arc.get_file_path_index_from_hash(hash).unwrap();
    let info_indice_idx = arc.get_file_info_from_path_index(path_idx).file_info_indice_index;

    // Get the FileCtx for this hash
    if let Some(filectx) = MOD_FILES.read().get(FileIndex::Regular(info_indice_idx)) {
        // Get the callback for this file as well as the file it applies to (either extracted from data.arc or from the SD)
        let content = recursive_file_backing_load(hash, &filectx.file);
        unsafe { *out_size = content.len(); }
        buffer.write(&content).unwrap();
        return true;
    }

    if let Some(filectx) = MOD_FILES.read().get(FileIndex::Stream(hash)) {
        // Get the callback for this file as well as the file it applies to (either extracted from data.arc or from the SD)
        let content = recursive_file_backing_load(hash, &filectx.file);
        unsafe { *out_size = content.len(); }
        buffer.write(&content).unwrap();
        return true;
    }

    false
}

#[no_mangle]
pub extern "C" fn arcrop_register_callback(hash: Hash40, length: usize, cb: CallbackFn) {
    debug!("[Arcropolis-API::register_callback] Hash received: '{}'", hashes::get(hash).green());

    let mut callbacks = CALLBACKS.write();

    let callback = if let Some(previous_cb) =  callbacks.get(&hash) {
        match previous_cb {
            crate::callbacks::CallbackKind::Regular(previous_cb) => {
                // Always get the larger length for patching to accomodate the callback that requires the biggest buffer
                let length = u32::max(previous_cb.len, length as u32);

                Callback {
                    callback_fn: cb,
                    len: length,
                    previous: Box::new(FileBacking::Callback {
                        callback: CallbackKind::Regular(previous_cb.clone()),
                        original: Box::new(FileBacking::LoadFromArc),
                    }),
                }
            }
            crate::callbacks::CallbackKind::Stream(_) => panic!("Trying to register a regular callback over a Stream callback."),
        }
    } else {
        Callback {
            callback_fn: cb,
            len: length as u32,
            previous: Box::new(FileBacking::LoadFromArc),
        }
    };

    // Overwrite the previous callback. Could probably be done better.
    callbacks.insert(hash, CallbackKind::Regular(callback));
}

#[no_mangle]
pub extern "C" fn arcrop_register_callback_with_path(hash: Hash40, cb: StreamCallbackFn) {
    debug!("[Arcropolis-API::register_callback] Hash received: '{}'", hashes::get(hash).green());

    let mut callbacks = CALLBACKS.write();

    let callback = if let Some(previous_cb) =  callbacks.get(&hash) {
        match previous_cb {
            crate::callbacks::CallbackKind::Regular(_) => panic!("Trying to register a Stream callback over a Regular callback."),
            crate::callbacks::CallbackKind::Stream(previous_cb) => {
                StreamCallback {
                    callback_fn: cb,
                    previous: Box::new(FileBacking::Callback {
                        callback: CallbackKind::Stream(previous_cb.clone()),
                        original: Box::new(FileBacking::LoadFromArc),
                    }),
                }
            }
        }
    } else {
        StreamCallback {
            callback_fn: cb,
            previous: Box::new(FileBacking::LoadFromArc),
        }
    };

    // Overwrite the previous callback. Could probably be done better.
    callbacks.insert(hash, CallbackKind::Stream(callback));
}

#[no_mangle]
pub extern "C" fn arcrop_api_version() -> &'static ApiVersion {
    debug!("[Arcropolis-API::api_version] Function called");

    &API_VERSION
}

fn show_arcrop_update_prompt() -> ! {
    skyline_web::DialogOk::ok(
        "Your ARCropolis version is older than one of your plugins supports, an update is required"
    );

    unsafe { skyline::nn::oe::ExitApplication() }
}

fn show_plugin_update_prompt() -> ! {
    skyline_web::DialogOk::ok(
        "Your ARCropolis version is too new for one of your plugins, it must be updated to support this API version"
    );

    unsafe { skyline::nn::oe::ExitApplication() }
}

#[no_mangle]
pub extern "C" fn arcrop_require_api_version(major: u32, minor: u32) {
    if major > API_VERSION.major || (major == API_VERSION.major && minor > API_VERSION.minor) {
        show_arcrop_update_prompt()
    } else if major < API_VERSION.major {
        show_plugin_update_prompt()
    }
}

#[repr(C)]
pub struct ApiVersion {
    major: u32,
    minor: u32,
}
