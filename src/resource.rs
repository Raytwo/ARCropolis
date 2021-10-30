mod containers;
mod types;

pub use containers::*;
use smash_arc::LoadedArc;
pub use types::*;

use crate::offsets;

fn offset_to_addr<T>(offset: usize) -> *mut T {
    unsafe {
        (skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as usize + offset) as *mut T
    }
}

pub fn filesystem_info() -> &'static FilesystemInfo {
    let addr = offset_to_addr::<&'static FilesystemInfo>(offsets::filesystem_info());
    unsafe { *addr }
}

pub fn arc() -> &'static LoadedArc {
    filesystem_info().path_info.arc
}

pub fn res_service() -> &'static ResServiceNX {
    let addr = offset_to_addr::<&'static ResServiceNX>(offsets::res_service());
    unsafe { *addr }
}