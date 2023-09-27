pub mod callback;
pub mod event;
pub mod file;
pub mod utils;
pub mod lua;

pub use callback::*;
pub use event::*;
pub use file::*;
pub use utils::*;
pub use lua::*;

#[repr(C)]
pub struct ApiVersion {
    major: u32,
    minor: u32,
}

/// NOTE: THIS MUST BE BUMPED ANY TIME THE EXTERNALLY-FACING API IS CHANGED
///
/// How to know which to bump:
///
/// Do your changes modify an existing API: Major bump
/// Do your changes only add new APIs in a backwards compatible way: Minor bump
///
/// Are your changes only internal? No version bump
static API_VERSION: ApiVersion = ApiVersion { major: 1, minor: 9 };

#[no_mangle]
pub extern "C" fn arcrop_api_version() -> &'static ApiVersion {
    debug!("arcrop_api_version -> Function called");

    &API_VERSION
}

pub fn show_dialog(text: &str) {
    skyline_web::DialogOk::ok(text);
}

#[no_mangle]
pub extern "C" fn arcrop_require_api_version(major: u32, minor: u32) {
    if major > API_VERSION.major || (major == API_VERSION.major && minor > API_VERSION.minor) {
        show_arcrop_update_prompt()
    } else if major < API_VERSION.major {
        show_plugin_update_prompt()
    }
}

fn show_arcrop_update_prompt() -> ! {
    show_dialog("Your ARCropolis version is older than one of your plugins supports, an update is required");

    unsafe { skyline::nn::oe::ExitApplication() }
}

fn show_plugin_update_prompt() -> ! {
    show_dialog("Your ARCropolis version is too new for one of your plugins, it must be updated to support this API version");

    unsafe { skyline::nn::oe::ExitApplication() }
}
