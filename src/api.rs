pub mod callback;
pub mod event;
pub mod file;

pub use callback::*;
pub use event::*;
pub use file::*;

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
    minor: 5
};

#[no_mangle]
pub extern "C" fn arcrop_api_version() -> &'static ApiVersion {
    debug!("arcrop_api_version -> Function called");

    &API_VERSION
}