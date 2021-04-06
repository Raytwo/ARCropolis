use log::debug;
use smash_arc::Hash40;

#[no_mangle]
pub extern "C" fn arcrop_load_file(hash: Hash40, out_buffer: *mut u8, length: usize) {
    debug!("[Arcropolis-API::load_file] Hash40: {:#x}, Buffer len: {:#x}", hash.as_u64(), length);
    let buffer = unsafe { std::slice::from_raw_parts_mut(out_buffer, length) };
    
    unimplemented!()
}

pub extern "C" fn arcrop_register_callback(hash: Hash40) {
    debug!("[Arcropolis-API::register_callback] hash: {:#x}", hash.as_u64());
}

pub extern "C" fn arcrop_api_version() {
    debug!("[Arcropolis-API::api_version] Function called");
}