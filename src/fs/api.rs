use parking_lot::Mutex;
use smash_arc::*;
use super::*;

#[repr(C)]
pub struct ApiVersion {
    major: u32,
    minor: u32
}

/// NOTE: THIS MUST BE BUMPED ANY TIME THE EXTERNALLY-FACING API IS CHANGED
///
/// How to know which to bump:
///
/// Do your changes modify an existing API: Major bump
/// Do your changes only add new APIs in a backwards compatible way: Minor bump
///
/// Are your changes only internal? No version bump
/// 
/// ily but i don't care <3 - blujay
static API_VERSION: ApiVersion = ApiVersion {
    major: 1,
    minor: 4
};

// temporary
pub type CallbackFn = extern "C" fn(Hash40, *mut u8, usize, &mut usize) -> bool;
pub type StreamCallbackFn = extern "C" fn(Hash40, *mut u8, &mut usize) -> bool;
pub type ExtCallbackFn = extern "C" fn(Hash40, Hash40, *mut u8, usize, &mut usize) -> bool;
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
    ExtensionCallback {
        ext: String,
        callback: ExtCallbackFn
    }
}

unsafe impl Send for PendingApiCall {}
unsafe impl Sync for PendingApiCall {}

lazy_static! {
    pub static ref PENDING_API_CALLS: Mutex<Vec<PendingApiCall>> = Mutex::new(Vec::new());
}

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

    if let Some(size) = crate::GLOBAL_FILESYSTEM.read().load_into(hash, &mut buffer) {
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

    let mut pending_calls = PENDING_API_CALLS.lock();

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

    let mut pending_calls = PENDING_API_CALLS.lock();

    if GlobalFilesystem::is_init() {
        crate::GLOBAL_FILESYSTEM.write().handle_api_request(request);
    } else {
        pending_calls.push(request);
    }
}

#[no_mangle]
pub extern "C" fn arcrop_register_extension_callback() {
    error!("Old-style extension callbacks are not supported in ARCropolis 3.0.0. Please contact the developer to have them update their plugin.");
}

#[no_mangle]
pub extern "C" fn arcrop_register_ext_callback(str_data: *const u8, str_len: usize, cb: ExtCallbackFn) {
    let utf8 = unsafe {
        std::slice::from_raw_parts(str_data, str_len)
    };
    let extension = String::from_utf8_lossy(utf8).to_string();
    debug!("arcrop_register_ext_callback -> Extension received {}", extension.green());
    
    let request = PendingApiCall::ExtensionCallback {
        ext: extension,
        callback: cb
    };

    let mut pending_calls = PENDING_API_CALLS.lock();

    if GlobalFilesystem::is_init() {
        crate::GLOBAL_FILESYSTEM.write().handle_api_request(request);
    } else {
        pending_calls.push(request);
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
pub extern "C" fn arcrop_api_version() -> &'static ApiVersion {
    debug!("arcrop_api_version -> Function called");

    &API_VERSION
}

pub fn show_dialog(text: &str) {
    skyline_web::DialogOk::ok(text);
}

fn show_arcrop_update_prompt() -> ! {
    show_dialog(
        "Your ARCropolis version is older than one of your plugins supports, an update is required",
    );

    unsafe { skyline::nn::oe::ExitApplication() }
}

fn show_plugin_update_prompt() -> ! {
    show_dialog(
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