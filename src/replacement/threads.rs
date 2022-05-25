use owo_colors::OwoColorize;
use skyline::{hook, hooks::InlineCtx};
use smash_arc::{ArcLookup, Hash40};

use super::FileInfoFlagsExt;
use crate::{
    config, hashes, offsets, reg_w, reg_x,
    resource::{self, InflateFile, LoadInfo, LoadType},
    GLOBAL_FILESYSTEM,
};

#[hook(offset = offsets::inflate(), inline)]
fn inflate_incoming(ctx: &InlineCtx) {
    let arc = resource::arc();
    let service = resource::res_service();

    let info_index = (service.processing_file_idx_start + reg_w!(ctx, 27)) as usize;
    let file_info = &arc.get_file_infos()[info_index];

    let file_path = &arc.get_file_paths()[file_info.file_path_index];
    let path_hash = file_path.path.hash40();

    info!(
        target: "no-mod-path",
        "[ResInflateThread::inflate_incoming | #{:#08X} | Type: {} | {:>3} / {:>3}] Incoming '{}'",
        usize::from(file_info.file_path_index).green(),
        reg_w!(ctx, 21).green(),
        reg_x!(ctx, 27).yellow(),
        service.processing_file_idx_count.yellow(),
        hashes::find(path_hash).bright_yellow()
    );

    let mut fs = GLOBAL_FILESYSTEM.write();

    // Is setting incoming to None needed? Considering take() is called to acquire the incoming hash, it'd already be None.
    if let Some(path) = fs.get_physical_path(path_hash) {
        info!("Added file '{}' to the queue.", path.display().yellow());
        fs.set_incoming_file(path_hash);
    }
}

#[hook(offset = offsets::inflate_dir_file())]
fn inflate_dir_file(arg: u64, out_decomp_data: &mut InflateFile, comp_data: &InflateFile) -> u64 {
    trace!(
        target: "no-mod-path",
        "[ResInflateThread::inflate_dir_file] Incoming decompressed filesize: {:#x}",
        out_decomp_data.len()
    );

    let result = call_original!(arg, out_decomp_data, comp_data);

    if result == 0x0 {
        // Returns 0x0 on the very last read, since they can be read in chunks
        if let Some(hash) = crate::GLOBAL_FILESYSTEM.write().get_incoming_file() {
            handle_file_replace(hash);
        }
    }

    result
}

pub fn handle_file_replace(hash: Hash40) {
    let arc = resource::arc();
    let filesystem_info = resource::filesystem_info();

    let file_info = arc.get_file_info_from_hash(hash).expect(&format!(
        "Failed to find file info for '{}' ({:#x}) when replacing.",
        hashes::find(hash),
        hash.as_u64()
    ));

    let filepath_index = usize::from(file_info.file_path_index);
    let file_info_indice_index = usize::from(file_info.file_info_indice_index);

    let decompressed_size = arc.get_file_data(file_info, config::region()).decomp_size;

    if filesystem_info.get_loaded_filepaths()[filepath_index].is_loaded == 0 {
        warn!(
            "When replacing file '{}' ({:#x}), the file is not marked as loaded. FilepathIdx: {:#x}, LoadedDataIdx: {:#x}",
            hashes::find(hash),
            hash.0,
            filepath_index,
            file_info_indice_index
        );
    }

    if filesystem_info.get_loaded_datas()[file_info_indice_index].data.is_null() {
        warn!(
            "When replacing file '{}' ({:#x}), the loaded data buffer is empty. FilepathIdx: {:#x}, LoadedDataIdx: {:#x}",
            hashes::find(hash),
            hash.0,
            filepath_index,
            file_info_indice_index
        );
        return
    }

    let fs = crate::GLOBAL_FILESYSTEM.read();

    let mut buffer = unsafe {
        std::slice::from_raw_parts_mut(
            filesystem_info.get_loaded_datas()[file_info_indice_index].data as *mut u8,
            decompressed_size as usize,
        )
    };

    // TODO: Move this to a extension handler
    if let Ok(size) = fs.load_file_into(hash, &mut buffer) {
        if arc.get_file_paths()[filepath_index].ext.hash40() == Hash40::from("nutexb") {
            if size < decompressed_size as usize {
                let (contents, footer) = buffer.split_at_mut((decompressed_size - 0xb0) as usize);
                footer.copy_from_slice(&contents[(size - 0xb0)..size]);
            }
        }

        // TODO: Move this to a extension handler

        // else if file_info.flags.unshared_nus3bank() {
        //     static GRP_BYTES: &[u8] = &[0x47, 0x52, 0x50, 0x20];
        //     if let Some(id) = fs.get_bank_id(hash) {
        //         let buffer = &mut buffer[0x30..];
        //         if let Some(offset) = buffer.windows(GRP_BYTES.len()).position(|window| window == GRP_BYTES) {
        //             buffer[(offset - 4)..offset].copy_from_slice(&id.to_le_bytes());
        //         }
        //     }
        // }

        info!(
            "Replaced file '{}' ({:#x}) with buffer size {:#x} and file size {:#x}. Game buffer size: {:#x}",
            hashes::find(hash),
            hash.0,
            buffer.len(),
            size,
            resource::res_service().buffer_size
        );
    } else {
        warn!(
            "Failed to load file '{}' ({:#x}) into buffer with size {:#X}",
            hashes::find(hash),
            hash.0,
            decompressed_size
        );
    }
}

// handles submitting files to be loaded manually
#[hook(offset = offsets::res_load_loop_start(), inline)]
fn res_loop_start(_: &InlineCtx) {
    res_loop_common();
}

#[hook(offset = offsets::res_load_loop_refresh(), inline)]
fn res_loop_refresh(_: &InlineCtx) {
    res_loop_common();
}

// basically when we unshare or add files we mark them with the standalone_file flag. When the ResLoadingThread goes to load the files, it will either load it as part of a directory or as it's own file (all of UI is loaded as regular files, for example). If it's unshared or added, when the game goes to load the file as part of a directory, it's just going to crash because it points to invalid data. So what we do is right before it begins the process of loading directories/files, we:
// 1.  iterate through the list of what it is about to load
// 2. check if what it is loading is a directory
// 3. grab that directory index and iterate through all of it's files
// 4. if we find a file that is marked with standalone_file, we push it to the front of the list and ask that it be loaded as a regular file
// the game will check to see if the file is already loaded before attempting to load/decompress the data, so it will skip that file when loading as part of a directory
// - blujay
fn res_loop_common() {
    let arc = resource::arc();
    let service = resource::res_service_mut();
    let file_paths = arc.get_file_paths();
    let _file_info_indices = arc.get_file_info_indices();
    let file_infos = arc.get_file_infos();
    let dir_infos = arc.get_dir_infos();

    let mut standalone_files = vec![Vec::new(); 5];

    for (list_idx, list) in service.res_lists.iter().enumerate() {
        for entry in list.iter() {
            match entry.ty {
                LoadType::Directory => {
                    for info in file_infos[dir_infos[entry.directory_index as usize].file_info_range()].iter() {
                        if info.flags.standalone_file() {
                            standalone_files[list_idx].push(info.file_path_index);
                        }
                    }
                },
                _ => {},
            }
        }
    }

    for (idx, vec) in standalone_files.into_iter().enumerate() {
        for path_idx in vec.into_iter() {
            trace!(
                "Adding file to standalone queue: {} ({:#x})",
                hashes::find(file_paths[path_idx].path.hash40()),
                file_paths[path_idx].path.hash40().0
            );
            service.res_lists[idx].insert(LoadInfo {
                ty: LoadType::File,
                filepath_index: path_idx.0,
                directory_index: 0xFF_FFFF,
                files_to_load: 0,
            });
        }
    }
}

pub fn install() {
    skyline::install_hooks!(inflate_incoming, inflate_dir_file, res_loop_start, res_loop_refresh);
}
