use smash_arc::Hash40;

#[no_mangle]
pub extern "C" fn arcrop_load_file(hash: Hash40, out_buffer: *mut u8, length: usize) {
    let buffer = unsafe { std::slice::from_raw_parts_mut(out_buffer, length) };
    
    unimplemented!()
}