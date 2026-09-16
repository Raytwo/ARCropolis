use skyline::libc::c_char;
use smash_arc::{Hash40, LoadedArc};

use crate::offsets;

#[skyline::hook(offset = offsets::lookup_stream_hash())]
fn lookup_stream_hash(out_path: *mut c_char, loaded_arc: &LoadedArc, size_out: &mut usize, offset_out: &mut u64, hash: Hash40) {
    let fs = crate::GLOBAL_FILESYSTEM.read().unwrap();
    if let Some((path, size)) = fs.modfs().resolve_stream_path(hash) {
        *size_out = size;
        *offset_out = 0;
        let cpath = format!("{}\0", path.display());
        let out_buffer = unsafe { std::slice::from_raw_parts_mut(out_path, cpath.len()) };
        out_buffer.copy_from_slice(cpath.as_bytes());
        return;
    }

    original!()(out_path, loaded_arc, size_out, offset_out, hash)
}

pub fn install() {
    let base = offsets::load_stream();

    skyline::patching::Patch::in_text(base + 0x84).nop().unwrap();  // Patch out first `offset_out == 0` check
    skyline::patching::Patch::in_text(base + 0x154).nop().unwrap(); // Patch out second `offset_out == 0` check
    skyline::patching::Patch::in_text(base + 0x230).nop().unwrap(); // Patch out third `offfset_out == 0` check

    skyline::install_hooks!(lookup_stream_hash);
}
