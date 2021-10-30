use owo_colors::OwoColorize;

use smash_arc::ArcLookup;

use skyline::{
    hook,
    hooks::InlineCtx
};

use crate::{
    reg_x,
    reg_w,
    GLOBAL_FILESYSTEM,
    offsets,
    resource::{
        self,
        InflateFile
    },
    hashes
};

#[hook(offset = offsets::inflate(), inline)]
fn inflate_incoming(ctx: &InlineCtx) {
    let arc = resource::arc();
    let service = resource::res_service();

    let info_index = (service.processing_file_idx_start + reg_w!(ctx, 27)) as usize;
    let file_info = &arc.get_file_infos()[info_index];
    let info_index_idx = file_info.file_info_indice_index;
    
    let file_path = &arc.get_file_paths()[file_info.file_path_index];
    let path_hash = file_path.path.hash40();

    info!(
        target: "no-mod-path",
        "[ResInflateThread::inflate_incoming | #{}{:06X} | Type: {} | {:>3} / {:>3}] Incoming '{}'",
        "0x".green(),
        usize::from(file_info.file_path_index).green(),
        reg_w!(ctx, 21).green(),
        reg_x!(ctx, 27).yellow(),
        service.processing_file_idx_count.yellow(),
        hashes::find(path_hash).bright_yellow()
    );

    let mut fs = GLOBAL_FILESYSTEM.write();

    let should_add = if let Some(path) = fs.hash(path_hash) {
        info!(
            "Added file '{}' to the queue.",
            path.display().yellow()
        );
        true
    } else {
        false
    };
    
    if should_add {
        fs.set_incoming(path_hash);
    }
}

#[hook(offset = offsets::inflate_dir_file())]
fn inflate_dir_file(arg: u64, out_decomp_data: &mut InflateFile, comp_data: &InflateFile) -> u64 {
    trace!(
        target: "no-mod-path",
        "[ResInflateThread::inflate_dir_file] Incoming decompressed filesize: {:#x}",
        out_decomp_data.len()
    );

    call_original!(arg, out_decomp_data, comp_data)
}

pub fn install() {
    skyline::install_hooks!(
        inflate_incoming,
        inflate_dir_file
    );
}