use skyline::{
    hook,
    hooks::InlineCtx,
    libc::{c_void, memcpy},
    patching::patch_data,
};

use crate::{offsets, reg_x};

/// Fixes the issue where files originally stored as uncompressed in the data.arc
/// would crash if you replaced them with a file of a larger size.
/// The reason that this works is because the destination buffer (register `x0`) is
/// allocated with the same size as the uncompressed field of the FileData, and so
/// when the game goes to memcpy the data out of the ResService buffer and into the destination
/// it will crash. If we simply replace the file here instead of wherever else it would try,
/// the game will not crash and we won't get a mem read access violation
fn memcpy_uncompressed_fix(ctx: &InlineCtx) {
    // For now, we will leave this as an unconditionally true if statement
    let buffer_size = reg_x!(ctx, 2) as usize;

    if let Some(hash) = crate::GLOBAL_FILESYSTEM.write().sub_remaining_bytes(buffer_size) {
        println!("About to replace file with hash {:#x}", hash.as_u64());
        super::threads::handle_file_replace(hash);
    } else {
        let dest = reg_x!(ctx, 0) as *mut c_void;
        let src = reg_x!(ctx, 1) as *const c_void;

        unsafe {
            memcpy(dest, src, buffer_size);
        }
    }
}

#[hook(offset = offsets::memcpy_1(), inline)]
fn memcpy_1(ctx: &InlineCtx) {
    trace!(
        target: "no-mod-path",
        "[ResInflateThread::Memcpy1] Entering function"
    );
    memcpy_uncompressed_fix(ctx)
}

#[hook(offset = offsets::memcpy_2(), inline)]
fn memcpy_2(ctx: &InlineCtx) {
    trace!(
        target: "no-mod-path",
        "[ResInflateThread::Memcpy2] Entering function"
    );
    memcpy_uncompressed_fix(ctx)
}

#[hook(offset = offsets::memcpy_3(), inline)]
fn memcpy_3(ctx: &InlineCtx) {
    trace!(
        target: "no-mod-path",
        "[ResInflateThread::Memcpy3] Entering function"
    );
    memcpy_uncompressed_fix(ctx)
}

pub fn install() {
    // Must patch memcpy offsets before we install the hooks, otherwise the inline hook will not get called
    // and might crash
    unsafe {
        const NOP: u32 = 0xD503201F;
        patch_data(offsets::memcpy_1(), &NOP).expect("Unable to patch Memcpy1");
        patch_data(offsets::memcpy_2(), &NOP).expect("Unable to patch Memcpy2");
        patch_data(offsets::memcpy_3(), &NOP).expect("Unable to patch Memcpy3");
    }

    skyline::install_hooks!(memcpy_1, memcpy_2, memcpy_3);
}
