#![feature(proc_macro_hygiene)]
#![feature(str_strip)]
#![feature(asm)]
#![feature(ptr_offset_from)]

use std::ffi::CStr;
use std::fs::File;
use std::io::prelude::*;
use std::net::IpAddr;

use skyline::{hook, hooks::InlineCtx, install_hooks, nn};

mod cpp_vector;

mod config;
use config::CONFIG;

mod hashes;
mod stream;

mod replacement_files;
use replacement_files::{FileCtx, FileIndex, INCOMING_IDX, MOD_FILES};

mod offsets;
use offsets::{
    INFLATE_DIR_FILE_OFFSET, INFLATE_OFFSET, INITIAL_LOADING_OFFSET, MANUAL_OPEN_OFFSET,
    MEMCPY_1_OFFSET, MEMCPY_2_OFFSET, MEMCPY_3_OFFSET, TITLE_SCREEN_VERSION_OFFSET,
};

mod runtime;
use runtime::{LoadedTables, ResServiceState, Table2Entry};

mod menus;

mod logging;
use log::{info, trace};

mod visit;

mod fs;

use smash_arc::{ArcLookup, FileInfoIndiceIdx, Hash40};

use owo_colors::OwoColorize;

fn get_filectx_by_index<'a>(
    file_index: FileIndex,
) -> Option<(
    parking_lot::MappedRwLockReadGuard<'a, FileCtx>,
    &'a mut Table2Entry,
)> {
    match file_index {
        FileIndex::Regular(info_indice_index) => {
            let tables = LoadedTables::get_instance();

            let table2entry = match tables.get_t2_mut(info_indice_index) {
                Ok(entry) => entry,
                Err(_) => {
                    return None;
                }
            };

            match get_from_file_info_indice_index!(info_indice_index) {
                Ok(file_ctx) => {
                    info!(
                        "[ARC::Loading | #{}] Hash matching for file: '{:?}'",
                        usize::from(info_indice_index).green(),
                        file_ctx.file.path().display().bright_yellow()
                    );
                    Some((file_ctx, table2entry))
                }
                Err(_) => None,
            }
        }
        _ => None,
    }
}

fn replace_file_by_index(file_index: FileIndex) {
    if let Some((file_ctx, table2entry)) = get_filectx_by_index(file_index) {
        if table2entry.data == 0 as _ {
            return;
        }

        // Call extension callbacks from here
        if file_ctx.file.extension() == Hash40::from("nutexb") {
            replace_textures_by_index(&file_ctx, table2entry);
            return;
        }

        let file_slice = file_ctx.get_file_content().into_boxed_slice();

        info!(
            "[ResInflateThread | #{}] Replacing '{}'",
            usize::from(file_ctx.index).green(),
            hashes::get(file_ctx.hash)
                .unwrap_or(&"Unknown")
                .bright_yellow()
        );

        unsafe {
            let mut data_slice =
                std::slice::from_raw_parts_mut(table2entry.data as *mut u8, file_slice.len());
            data_slice.write_all(&file_slice).unwrap();
        }
    }
}

// TODO: Probably remove this once extension callbacks are a thing
fn replace_textures_by_index(file_ctx: &FileCtx, table2entry: &mut Table2Entry) {
    let orig_size = file_ctx.orig_subfile.decomp_size as usize;

    let file_slice = file_ctx.get_file_content().into_boxed_slice();

    info!(
        "[ResInflateThread | #{}] Replacing '{}'",
        usize::from(file_ctx.index).green(),
        hashes::get(file_ctx.hash)
            .unwrap_or(&"Unknown")
            .bright_yellow()
    );

    if orig_size > file_slice.len() {
        let data_slice =
            unsafe { std::slice::from_raw_parts_mut(table2entry.data as *mut u8, orig_size) };
        // Copy the content at the beginning
        data_slice[0..file_slice.len() - 0xB0]
            .copy_from_slice(&file_slice[0..file_slice.len() - 0xB0]);
        // Copy our new footer at the end
        data_slice[orig_size - 0xB0..orig_size]
            .copy_from_slice(&file_slice[file_slice.len() - 0xB0..file_slice.len()]);
    } else {
        let mut data_slice = unsafe {
            std::slice::from_raw_parts_mut(table2entry.data as *mut u8, file_ctx.file.len() as _)
        };
        data_slice.write_all(&file_slice).unwrap();
    }
}

#[hook(offset = INFLATE_OFFSET, inline)]
fn inflate_incoming(ctx: &InlineCtx) {
    unsafe {
        let arc = LoadedTables::get_arc();
        let res_service = ResServiceState::get_instance();

        let info_index =
            (res_service.processing_file_idx_start + *ctx.registers[27].x.as_ref() as u32) as usize;
        let file_info = arc.get_file_infos()[info_index];
        let info_indice_index = file_info.file_info_indice_index;

        let path_idx = usize::from(file_info.file_path_index);

        let hash = arc.get_file_paths()[path_idx].path.hash40();

        info!(
            "[ResInflateThread | #{} | Type: {} | {} / {}] Incoming '{}'",
            path_idx.green(),
            (*ctx.registers[21].w.as_ref()).green(),
            (*ctx.registers[27].x.as_ref()).yellow(),
            res_service.processing_file_idx_count.yellow(),
            hashes::get(hash).unwrap_or(&"Unknown").bright_yellow()
        );

        let mut incoming = INCOMING_IDX.write();

        if let Ok(context) = get_from_file_info_indice_index!(info_indice_index) {
            *incoming = Some(FileIndex::Regular(context.index));
            info!(
                "[ResInflateThread | #{}] Added index {} to the queue",
                path_idx.green(),
                usize::from(context.index).green()
            );
        } else {
            *incoming = None;
        }
    }
}

#[hook(offset = 0x33b6798, inline)]
fn loading_incoming(ctx: &InlineCtx) {
    unsafe {
        let arc = LoadedTables::get_arc();

        let path_idx = *ctx.registers[25].x.as_ref() as u32;
        let hash = arc.get_file_paths()[path_idx as usize].path.hash40();

        info!(
            "[ResLoadingThread | #{}] Incoming '{}'",
            path_idx.bright_yellow(),
            hashes::get(hash).unwrap_or(&"Unknown").bright_yellow()
        );
    }
}

/// For small uncompressed files
#[hook(offset = MEMCPY_1_OFFSET, inline)]
fn memcpy_uncompressed(_ctx: &InlineCtx) {
    trace!("[ResInflateThread | Memcpy1] Entering function");
    memcpy_impl();
}

/// For uncompressed files a bit larger
#[hook(offset = MEMCPY_2_OFFSET, inline)]
fn memcpy_uncompressed_2(_ctx: &InlineCtx) {
    trace!("[ResInflateThread | Memcpy2] Entering function");
    memcpy_impl();
}

/// For uncompressed files being read in multiple chunks
#[hook(offset = MEMCPY_3_OFFSET, inline)]
fn memcpy_uncompressed_3(_ctx: &InlineCtx) {
    trace!("[ResInflateThread | Memcpy3] Entering function");
    memcpy_impl();
}

fn memcpy_impl() {
    let incoming = INCOMING_IDX.read();

    if let Some(index) = *incoming {
        replace_file_by_index(index);
    }
}

#[repr(C)]
pub struct InflateFile {
    pub content: *const u8,
    pub size: u64,
}

#[hook(offset = INFLATE_DIR_FILE_OFFSET)]
fn load_directory_hook(
    unk1: *const u64,
    out_decomp_data: &InflateFile,
    comp_data: &InflateFile,
) -> u64 {
    trace!(
        "[LoadFileFromDirectory] Incoming decompressed filesize: {:x}",
        out_decomp_data.size
    );
    // if out_decomp_data.size == 0 {
    //     println!("Detected bad file size, crashing.");
    //     std::thread::sleep(std::time::Duration::from_millis(500));
    //     unsafe { *(0 as *mut u8) = 0x69; }
    // }

    // Let the file be inflated
    let result: u64 = original!()(unk1, out_decomp_data, comp_data);

    let incoming = INCOMING_IDX.read();

    if let Some(index) = *incoming {
        if index == FileIndex::Regular(FileInfoIndiceIdx(0)) {
            return result;
        }

        replace_file_by_index(index);
    }

    result
}

#[hook(offset = TITLE_SCREEN_VERSION_OFFSET)]
fn change_version_string(arg1: u64, string: *const u8) {
    let original_str = unsafe { CStr::from_ptr(string as _).to_str().unwrap() };

    if original_str.contains("Ver.") {
        let new_str = format!(
            "Smash {}\nARCropolis Ver. {}\0",
            original_str,
            env!("CARGO_PKG_VERSION").to_string()
        );

        original!()(arg1, skyline::c_str(&new_str))
    } else {
        original!()(arg1, string)
    }
}

#[hook(offset = MANUAL_OPEN_OFFSET)]
unsafe fn manual_hook(page_path: *const u8, unk2: *const u8, unk3: *const u64, unk4: u64) {
    let original_page = CStr::from_ptr(page_path as _).to_str().unwrap();

    let is_manual = if original_page.contains("contents.htdocs/help/html/") {
        if original_page.ends_with("index.html") {
            menus::workspace_selector();
            true
        } else {
            false
        }
    } else if original_page.contains("contents.htdocs/howto/html/") {
        if original_page.ends_with("index.html") {
            menus::show_arcadia();
            false
        } else {
            false
        }
    } else {
        false
    };

    if !is_manual {
        original!()(page_path, unk2, unk3, unk4)
    }
}

#[hook(offset = INITIAL_LOADING_OFFSET, inline)]
fn initial_loading(_ctx: &InlineCtx) {
    //menus::show_arcadia();
    logging::init(CONFIG.read().logger.as_ref().unwrap().logger_level.into()).unwrap();

    // Check if an update is available
    if skyline_update::check_update(
        IpAddr::V4(CONFIG.read().updater.as_ref().unwrap().server_ip),
        "ARCropolis",
        env!("CARGO_PKG_VERSION"),
        CONFIG.read().updater.as_ref().unwrap().beta_updates,
    ) {
        skyline::nn::oe::RestartProgramNoArgs();
    }

    // Lmao gross
    let changelog = if let Ok(mut file) =
        File::open("sd:/atmosphere/contents/01006A800016E000/romfs/changelog.md")
    {
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        Some(format!("Changelog\n\n{}", &content))
    } else {
        None
    };

    // TODO: Replace by a proper Smash-like menu someday
    if let Some(text) = changelog {
        skyline_web::DialogOk::ok(text);
        std::fs::remove_file("sd:/atmosphere/contents/01006A800016E000/romfs/changelog.md")
            .unwrap();
    }

    // Discover files
    unsafe {
        nn::oe::SetCpuBoostMode(nn::oe::CpuBoostMode::Boost);
        let mod_map = replacement_files::ModFileMap::new();
        mod_map.unshare().unwrap();
        lazy_static::initialize(&MOD_FILES);
        *MOD_FILES.write() = mod_map.to_mod_files();

        nn::oe::SetCpuBoostMode(nn::oe::CpuBoostMode::Disabled);
    }
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    // Load hashes from rom:/skyline/hashes.txt if the file is present
    hashes::init();
    // Look for the offset of the various functions to hook
    offsets::search_offsets();

    install_hooks!(
        initial_loading,
        inflate_incoming,
        //loading_incoming,
        memcpy_uncompressed,
        memcpy_uncompressed_2,
        memcpy_uncompressed_3,
        load_directory_hook,
        manual_hook,
        change_version_string,
        stream::lookup_by_stream_hash,
    );

    unsafe {
        skyline::patching::patch_data_from_text(skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as *const u8, 0x34636c4, &0x14000002);
    }

    println!(
        "ARCropolis v{} - File replacement plugin is now installed",
        env!("CARGO_PKG_VERSION")
    );
}
