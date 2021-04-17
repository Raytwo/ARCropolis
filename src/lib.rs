#![feature(proc_macro_hygiene)]
#![feature(str_strip)]
#![feature(asm)]
#![feature(ptr_offset_from)]
#![feature(slice_fill)]

use std::{ffi::CStr, path::PathBuf};
use std::io::prelude::*;
use std::net::IpAddr;
use callbacks::Callback;
use skyline::{hook, hooks::InlineCtx, install_hooks, nn};

mod cache;
mod cpp_vector;
mod config;
mod hashes;
mod stream;
mod replacement_files;
mod offsets;
mod runtime;
mod menus;
mod logging;
mod fs;
mod callbacks;
mod api;

use config::CONFIG;
use runtime::{LoadedTables, ResServiceState, Table2Entry};
use replacement_files::{FileCtx, FileIndex, INCOMING_IDX, MOD_FILES, UNSHARE_LUT};

use offsets::{
    INFLATE_DIR_FILE_OFFSET, INFLATE_OFFSET, INITIAL_LOADING_OFFSET, MANUAL_OPEN_OFFSET,
    MEMCPY_1_OFFSET, MEMCPY_2_OFFSET, MEMCPY_3_OFFSET, TITLE_SCREEN_VERSION_OFFSET,
};


use binread::*;
use owo_colors::OwoColorize;
use log::{info, trace, warn};
use smash_arc::{ArcLookup, FileInfoIndiceIdx, Hash40};
use arcropolis_api as arc_api;

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
                        hashes::get(file_ctx.hash).bright_yellow()
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
        if file_ctx.extension() == Hash40::from("nutexb") {
            replace_textures_by_index(&file_ctx, table2entry);
            return;
        }

        let file_slice = file_ctx.get_file_content();
        println!("Replace_by_idx slice size: {:#x}", file_slice.len());

        info!(
            "[ResInflateThread | #{}] Replacing '{}'",
            usize::from(file_ctx.index).green(),
            hashes::get(file_ctx.hash).bright_yellow()
        );

        unsafe {
            let mut data_slice = std::slice::from_raw_parts_mut(table2entry.data as *mut u8, file_ctx.len() as usize);
            data_slice.write_all(&file_slice).unwrap();
        }
    }
}

// TODO: Probably remove this once extension callbacks are a thing
fn replace_textures_by_index(file_ctx: &FileCtx, table2entry: &mut Table2Entry) {
    let orig_size = file_ctx.orig_size as usize;

    let file_slice = file_ctx.get_file_content().into_boxed_slice();
    println!("Replace_texture_by_idx slice size: {:#x}", file_slice.len());

    info!(
        "[ResInflateThread | #{}] Replacing '{}'",
        usize::from(file_ctx.index).green(),
        hashes::get(file_ctx.hash).bright_yellow()
    );

    if file_ctx.len() as usize > file_slice.len() {
        let data_slice = unsafe { std::slice::from_raw_parts_mut(table2entry.data as *mut u8, file_ctx.len() as usize) };

        let (mut from, mut to) = data_slice.split_at_mut(file_ctx.len() as usize - 0xB0);
        from.write(&file_slice[0..file_slice.len() - 0xB0]);
        to.write(&file_slice[file_slice.len() - 0xB0..file_slice.len()]);
    } else {
        let mut data_slice = unsafe { std::slice::from_raw_parts_mut(table2entry.data as *mut u8, file_slice.len() as _) };
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
            hashes::get(hash).bright_yellow()
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
            true
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

static mut LUT_LOADER_HANDLE: Option<std::thread::JoinHandle<()>> = None;

extern "C" fn replace_msg_name(out_size: *mut usize, hash: u64, buffer: *mut u8, length: usize) -> bool {
    println!("Hash received: {}", hashes::get(hash));
    let mut buffer = unsafe { std::slice::from_raw_parts_mut(buffer, length) };

    // Get EuFrench msg_name.msbt and write that in the buffer
    let arc = LoadedTables::get_arc();
    let content = arc.get_nonstream_file_contents(Hash40(hash), smash_arc::Region::EuFrench).unwrap();
    buffer.write(&content);

    let mut size = out_size;
    unsafe { *size = length };

    false

    // Load the file on the SD, or from data.arc if there are none
    // arc_api::load_original_file(hash, buffer);
}

extern "C" fn chained_replace_msg_name(out_size: *mut usize, hash: u64, buffer: *mut u8, length: usize) -> bool {
    println!("Hash received: {}", hashes::get(hash));
    let mut buffer = unsafe { std::slice::from_raw_parts_mut(buffer, length) };

    // Get EuFrench msg_name.msbt and write that in the buffer
    let arc = LoadedTables::get_arc();
    let content = arc.get_nonstream_file_contents(Hash40(hash), smash_arc::Region::EuDutch).unwrap();
    buffer.write(&content);

    let mut size = out_size;
    unsafe { *size = content.len() };

    // If returning true, msg_name will be in dutch
    //true
    // If returning false, msg_name will be loaded by the next callback in line if there is one, or a file on the SD or data.arc
    false

    // Load the file on the SD, or from data.arc if there are none
    // arc_api::load_original_file(hash, buffer);
}

extern "C" fn replace_title_screen_music(out_size: *mut usize, hash: u64, buf: *mut u8, length: usize) -> bool {
    println!("Stream hash received: {}", hashes::get(hash));
    return false;
    
    let mut buffer = unsafe { std::slice::from_raw_parts_mut(buf, length) };

    arc_api::load_original_file(hash, buffer);

    let mut size = out_size;
    unsafe { *size = length };

    true
}

#[hook(offset = INITIAL_LOADING_OFFSET, inline)]
fn initial_loading(_ctx: &InlineCtx) {
    let config = CONFIG.read();
    
    if logging::init(config.logger.unwrap().logger_level.into()).is_err() {
        println!("ARCropolis logger could not be initialized.")
    }

    // Check if an update is available
    if skyline_update::check_update(
        IpAddr::V4(config.updater.unwrap().server_ip),
        "ARCropolis",
        env!("CARGO_PKG_VERSION"),
        config.updater.unwrap().beta_updates,
    ) {
        skyline_web::DialogOk::ok("The update was downloaded successfully<br>ARCropolis will now reboot."); 
        skyline::nn::oe::RestartProgramNoArgs();
    }

    if let Ok(changelog) = std::fs::read_to_string("sd:/atmosphere/contents/01006A800016E000/romfs/arcropolis/changelog.toml") 
    {
        match toml::from_str(&changelog) {
            Ok(changelog) => {
                menus::display_update_page(&changelog);
                std::fs::remove_file("sd:/atmosphere/contents/01006A800016E000/romfs/arcropolis/changelog.toml").unwrap();
            },
            Err(_) => {
                warn!("Changelog could not be parsed. Is the file malformed?");
            }
        }
    }

    // Register a callback before file discovery happens to test the API
    // Size for EuFrench msg_name.msbt on 11.0.1
    //arc_api::register_callback("ui/message/msg_name.msbt", 0x800a0, replace_msg_name);
    //arc_api::register_callback("ui/message/msg_name.msbt", 0x77580, chained_replace_msg_name);
    arc_api::register_stream_callback("stream:/sound/bgm/bgm_crs2_01_menu.nus3audio", 0x6148, "sd:/bgm_crs2_01_menu.nus3audio", replace_title_screen_music);

    // Discover files
    unsafe {
        nn::oe::SetCpuBoostMode(nn::oe::CpuBoostMode::Boost);

        // if let Some(handle) = LUT_LOADER_HANDLE.take() {
        //     handle.join().unwrap();
        //     let lut = UNSHARE_LUT.read();
        //     if lut.is_none() {
        //         skyline_web::DialogOk::ok("No valid unsharing lookup table found. One will be generated and the game will restart.");
        //         let cache = cache::UnshareCache::new(LoadedTables::get_arc());
        //         cache::UnshareCache::write(LoadedTables::get_arc(), &cache, &PathBuf::from("sd:/atmosphere/contents/01006A800016E000/romfs/skyline/unshare_lut.bin")).unwrap();
        //         nn::oe::RestartProgramNoArgs();
        //     } else if lut.as_ref().unwrap().arc_version != (*LoadedTables::get_arc().fs_header).version {
        //         skyline_web::DialogOk::ok("Found unsharing lookup table for a different game version. A new one will be generated and the game will restart.");
        //         let cache = cache::UnshareCache::new(LoadedTables::get_arc());
        //         cache::UnshareCache::write(LoadedTables::get_arc(), &cache, &PathBuf::from("sd:/atmosphere/contents/01006A800016E000/romfs/skyline/unshare_lut.bin")).unwrap();
        //         nn::oe::RestartProgramNoArgs();
        //     }
        // }

        lazy_static::initialize(&MOD_FILES);

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
        memcpy_uncompressed,
        memcpy_uncompressed_2,
        memcpy_uncompressed_3,
        load_directory_hook,
        manual_hook,
        change_version_string,
        stream::lookup_by_stream_hash,
    );

    // unsafe {
    //     skyline::patching::patch_data_from_text(skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as *const u8, 0x346_36c4, &0x1400_0002);
        
    //     LUT_LOADER_HANDLE = Some(std::thread::spawn(|| {
    //         let mut unshare_lut = UNSHARE_LUT.write();
    //         *unshare_lut = match std::fs::read("rom:/skyline/unshare_lut.bin") {
    //             Ok(file_data) => {
    //                 let mut reader = std::io::Cursor::new(file_data);
    //                 match cache::UnshareCache::read(&mut reader) {
    //                     Ok(lut) => Some(lut),
    //                     Err(_) => None
    //                 }
    //             },
    //             Err(_) => None
    //         }
    //     }));
    // }

    println!(
        "ARCropolis v{} - File replacement plugin is now installed",
        env!("CARGO_PKG_VERSION")
    );
}
