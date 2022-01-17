use parking_lot::Mutex;
use smash_arc::Hash40;
use arcropolis_api::{
    CallbackFn,
    StreamCallbackFn
};
use crate::fs::*;
use crate::hashes;
use owo_colors::OwoColorize;


pub enum PendingApiCall {
    GenericCallback {
        hash: Hash40,
        max_size: usize,
        callback: CallbackFn
    },
    StreamCallback {
        hash: Hash40,
        callback: StreamCallbackFn
    },
}

unsafe impl Send for PendingApiCall {}
unsafe impl Sync for PendingApiCall {}

lazy_static! {
    pub static ref PENDING_CALLBACKS: Mutex<Vec<PendingApiCall>> = Mutex::new(Vec::new());
}

#[no_mangle]
pub extern "C" fn arcrop_register_callback(hash: Hash40, max_size: usize, cb: CallbackFn) {
    debug!(
        "arcrop_register_callback -> Hash received: {} ({:#x})",
        hashes::find(hash).green(),
        hash.0
    );

    let request = PendingApiCall::GenericCallback {
        hash,
        max_size,
        callback: cb
    };

    let mut pending_calls = PENDING_CALLBACKS.lock();

    if GlobalFilesystem::is_init() {
        crate::GLOBAL_FILESYSTEM.write().handle_api_request(request);
    } else {
        pending_calls.push(request);
    }
}

#[no_mangle]
pub extern "C" fn arcrop_register_callback_with_path(hash: Hash40, cb: StreamCallbackFn) {
    debug!(
        "arcrop_register_callback_with_path -> Hash received: {} ({:#x})",
        hashes::find(hash).green(),
        hash.0
    );

    let request = PendingApiCall::StreamCallback {
        hash,
        callback: cb
    };

    let mut pending_calls = PENDING_CALLBACKS.lock();

    if GlobalFilesystem::is_init() {
        crate::GLOBAL_FILESYSTEM.write().handle_api_request(request);
    } else {
        debug!("Pushing to pending calls!");
        pending_calls.push(request);
    }
}

#[no_mangle]
pub extern "C" fn arcrop_register_extension_callback() {
    error!("Extension callbacks are not (yet) supported in ARCropolis 3.0.0. Please contact the developer to have them update their plugin.");
}