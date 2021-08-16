use std::io::Write;

use arcropolis_api::{CallbackFn, StreamCallbackFn};
use log::debug;
use owo_colors::OwoColorize;
use smash_arc::{ArcLookup, Hash40};

use crate::{callbacks::{Callback, CallbackKind, StreamCallback}, config::REGION, hashes, offsets::offset_to_addr, replacement_files::{
        recursive_file_backing_load, FileBacking, FileIndex, CALLBACKS, MOD_FILES,
    }, runtime::{LOADED_TABLES_OFFSET, LoadedTables}};

/// NOTE: THIS MUST BE BUMPED ANY TIME THE EXTERNALLY-FACING API IS CHANGED
///
/// How to know which to bump:
///
/// Do your changes modify an existing API: Major bump
/// Do your changes only add new APIs in a backwards compatible way: Minor bump
///
/// Are your changes only internal? No version bump
static API_VERSION: ApiVersion = ApiVersion { major: 1, minor: 3 };

use lazy_static::lazy_static;
use parking_lot::RwLock;
use std::collections::HashMap;

pub use arcropolis_api::*;

lazy_static! {
    pub static ref EXT_CALLBACKS: RwLock<HashMap<Hash40, Vec<ExtCallbackFn>>> =
        RwLock::new(HashMap::new());
    
    pub static ref REJECTED_EXT_CALLBACKS: RwLock<HashMap<Hash40, Vec<RejectedExtFn>>> =
        RwLock::new(HashMap::new());
}

#[no_mangle]
pub extern "C" fn arcrop_load_file(
    hash: Hash40,
    out_buffer: *mut u8,
    buf_length: usize,
    out_size: &mut usize,
) -> bool {
    debug!(
        "[Arcropolis-API::load_file] Hash received: {}, Buffer len: {:#x}",
        hashes::get(hash).green(),
        buf_length
    );

    let arc = LoadedTables::get_arc();
    let mut buffer = unsafe { std::slice::from_raw_parts_mut(out_buffer, buf_length) };

    // TODO: Require extra code to handle streams
    let path_idx = arc.get_file_path_index_from_hash(hash).unwrap();
    let info_indice_idx = arc
        .get_file_info_from_path_index(path_idx)
        .file_info_indice_index;

    // Get the FileCtx for this hash
    if let Some(filectx) = MOD_FILES.read().get(FileIndex::Regular(info_indice_idx)) {
        // Get the callback for this file as well as the file it applies to (either extracted from data.arc or from the SD)
        let content = recursive_file_backing_load(hash, &filectx.file);
        *out_size = content.len();
        buffer.write_all(&content).unwrap();
    } else if let Some(filectx) = MOD_FILES.read().get(FileIndex::Stream(hash)) {
        // Get the callback for this file as well as the file it applies to (either extracted from data.arc or from the SD)
        let content = recursive_file_backing_load(hash, &filectx.file);
        *out_size = content.len();
        buffer.write_all(&content).unwrap();
    } else {
        match arc.get_file_contents(hash, *REGION) {
            Ok(contents) => {
                *out_size = contents.len();
                buffer.write_all(&contents).unwrap();
            }
            Err(_) => return false
        }
    }

    true
}

#[no_mangle]
pub extern "C" fn arcrop_register_callback(hash: Hash40, length: usize, cb: CallbackFn) {
    debug!(
        "[Arcropolis-API::register_callback] Hash received: '{}'",
        hashes::get(hash).green()
    );

    let mut callbacks = CALLBACKS.write();

    let callback = if let Some(previous_cb) = callbacks.get(&hash) {
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
            crate::callbacks::CallbackKind::Stream(_) => {
                panic!("Trying to register a regular callback over a Stream callback.")
            }
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
    debug!(
        "[Arcropolis-API::register_callback] Hash received: '{}'",
        hashes::get(hash).green()
    );

    let mut callbacks = CALLBACKS.write();

    let callback = if let Some(previous_cb) = callbacks.get(&hash) {
        match previous_cb {
            crate::callbacks::CallbackKind::Regular(_) => {
                panic!("Trying to register a Stream callback over a Regular callback.")
            }
            crate::callbacks::CallbackKind::Stream(previous_cb) => StreamCallback {
                callback_fn: cb,
                previous: Box::new(FileBacking::Callback {
                    callback: CallbackKind::Stream(previous_cb.clone()),
                    original: Box::new(FileBacking::LoadFromArc),
                }),
            },
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
pub extern "C" fn arcrop_register_extension_callback(hash: Hash40, cb: ExtCallbackFn) {
    EXT_CALLBACKS
        .write()
        .entry(hash)
        .or_default()
        .push(cb)
}

#[no_mangle]
pub extern "C" fn arcrop_register_rejected_extension(hash: Hash40, cb: RejectedExtFn) {
    REJECTED_EXT_CALLBACKS
        .write()
        .entry(hash)
        .or_default()
        .push(cb)
}

#[no_mangle]
pub extern "C" fn arcrop_get_decompressed_size(hash: Hash40, out_size: &mut usize) -> bool {
    if unsafe { *(offset_to_addr(LOADED_TABLES_OFFSET) as *mut *mut ()) }.is_null() {
        false
    } else {
        LoadedTables::get_arc().get_file_data_from_hash(hash, *REGION)
            .map(|data| *out_size = data.decomp_size as usize)
            .is_ok()
    }
}

#[no_mangle]
pub extern "C" fn arcrop_api_version() -> &'static ApiVersion {
    debug!("[Arcropolis-API::api_version] Function called");

    &API_VERSION
}

fn show_arcrop_update_prompt() -> ! {
    skyline_web::DialogOk::ok(
        "Your ARCropolis version is older than one of your plugins supports, an update is required",
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
