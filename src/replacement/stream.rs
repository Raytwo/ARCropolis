use skyline::libc::c_char;
use smash_arc::{Hash40, LoadedArc};
use crate::offsets;

#[skyline::hook(offset = offsets::lookup_stream_hash())]
fn lookup_stream_hash(
    out_path: *mut c_char,
    loaded_arc: &LoadedArc,
    size_out: &mut usize,
    offset_out: &mut u64,
    hash: Hash40
) {
    let fs = crate::GLOBAL_FILESYSTEM.read();
    if let Some(path) = fs.local_hash(hash) {
        if let Some(size) = fs.get().query_max_filesize(path) {
            if let Some(path) = fs.hash(hash) {
                *size_out = size;
                *offset_out = 0;
                let cpath = format!("{}\0", path.display());
                let out_buffer = unsafe {
                    std::slice::from_raw_parts_mut(out_path, cpath.len())
                };
                out_buffer.copy_from_slice(cpath.as_bytes());
                return;
            }
        }
    }
    original!()(out_path, loaded_arc, size_out, offset_out, hash)
}

pub fn install() {
    skyline::install_hooks!(
        lookup_stream_hash
    );
}