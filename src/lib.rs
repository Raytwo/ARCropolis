#![feature(proc_macro_hygiene)]
#![feature(asm)]
#![feature(map_try_insert)]
#![allow(dead_code)]
#![allow(unaligned_references)]
extern crate skyline_communicate as cli;
#[macro_use]
extern crate lazy_static;

use arcropolis_api::load_original_file;
use res_list::{LoadInfo, LoadType};
use skyline::{hook, hooks::InlineCtx, install_hooks, libc::{c_char, malloc}, nn};
use std::{ffi, io::prelude::*};
use std::net::IpAddr;
use std::ffi::CStr;

mod api;
// mod cache;
mod callbacks;
mod config;
mod cpp_vector;
mod fs;
mod hashes;
mod logging;
mod menus;
mod offsets;
mod res_list;
mod remote;
mod replacement_files;
mod runtime;
mod stream;
mod unsharing;

use config::{CONFIG, REGION};
use replacement_files::{FileCtx, FileIndex, IncomingLoad, INCOMING_LOAD, MOD_FILES};
use runtime::{FileState, LoadedTables, ResServiceState, Table2Entry};

use offsets::{
    INFLATE_DIR_FILE_OFFSET, INFLATE_OFFSET, INITIAL_LOADING_OFFSET, MANUAL_OPEN_OFFSET,
    MEMCPY_1_OFFSET, MEMCPY_2_OFFSET, MEMCPY_3_OFFSET, TITLE_SCREEN_VERSION_OFFSET,
};

use api::EXT_CALLBACKS;

use log::{info, trace, warn};
use owo_colors::OwoColorize;
use smash_arc::{ArcLookup, FileInfoIndiceIdx, Hash40};

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

        info!(
            "[ResInflateThread | #{}] Replacing '{}'",
            usize::from(file_ctx.index).green(),
            hashes::get(file_ctx.hash).bright_yellow()
        );

        unsafe {
            let mut data_slice = std::slice::from_raw_parts_mut(
                table2entry.data as *mut u8,
                file_ctx.len() as usize,
            );
            data_slice.write_all(&file_slice).unwrap();
        }
    }
}

// TODO: Probably remove this once extension callbacks are a thing
fn replace_textures_by_index(file_ctx: &FileCtx, table2entry: &mut Table2Entry) {
    // get the file data to be loaded into the buffer
    let file_slice = file_ctx.get_file_content().into_boxed_slice();

    info!(
        "[ResInflateThread | #{}] Replacing '{}'",
        usize::from(file_ctx.index).green(),
        hashes::get(file_ctx.hash).bright_yellow()
    );

    // get the size of the buffer the game allocated
    let arc = LoadedTables::get_arc();
    let buffer_size = arc
        .get_file_data_from_hash(file_ctx.hash, *REGION)
        .unwrap()
        .decomp_size;

    // length of the buffer before header extension
    let real_size = file_slice.len();

    // file_ctx.len() - size of the allocated buffer, the max size
    // table2entry.data - pointer to the buffer allocated
    // ??? - size of the nutexb before extension
    // ??? - the file data needing extension
    let data_out =
        unsafe { std::slice::from_raw_parts_mut(table2entry.data as *mut u8, buffer_size as _) };

    // Copy data into out buffer
    data_out[..file_slice.len()].copy_from_slice(&file_slice);

    // this will point to the index where the footer needs to be
    let max_data_size = data_out.len() - 0xb0;

    // if the data given is smaller than the out buffer, we need to copy the nutexb footer
    // to the end of the buffer
    if file_slice.len() < data_out.len() {
        let start_of_footer = real_size - 0xb0;

        let (contents, footer) = data_out.split_at_mut(max_data_size);

        let original_footer = &contents[start_of_footer..real_size];

        // copy the footer to the end of the buffer
        footer.copy_from_slice(original_footer);
    }
}

fn replace_extension_callback(extension: Hash40, index: FileInfoIndiceIdx) {
    let tables = LoadedTables::get_instance();
    let arc = LoadedTables::get_arc();

    let info_index = arc.get_file_info_indices()[index].file_info_index;
    let file_info = &arc.get_file_infos()[info_index];
    let file_path = &arc.get_file_paths()[file_info.file_path_index];

    let path_hash = file_path.path.hash40();

    let table2entry = match tables.get_t2_mut(index) {
        Ok(entry) => entry,
        Err(_) => return,
    };

    let file_data = arc.get_file_data(file_info, *REGION);

    let data = table2entry.data as *mut u8;
    let max_len = file_data.decomp_size as usize;

    let file_slice = unsafe { std::slice::from_raw_parts_mut(data, max_len) };

    let mut out_len = 0;
    for callback in EXT_CALLBACKS.read().get(&extension).iter().map(|x| x.iter()).flatten() {
        if callback(path_hash.as_u64(), data, max_len, &mut out_len) {
            // handle extending nutexb footers
            if file_path.ext.hash40() == Hash40::from("nutexb") {
                // this will point to the index where the footer needs to be
                let max_data_size = max_len - 0xb0;

                // if the data given is smaller than the out buffer, we need to copy the nutexb footer
                // to the end of the buffer
                if out_len < max_len {
                    let start_of_footer = out_len - 0xb0;

                    let (contents, footer) = file_slice.split_at_mut(max_data_size);

                    let original_footer = &contents[start_of_footer..out_len];

                    // copy the footer to the end of the buffer
                    footer.copy_from_slice(original_footer);
                }
            }

            return
        }
    }

    // if the file wasn't loaded by any of the callbacks, search for a fallback
    if MOD_FILES.read().0.contains_key(&FileIndex::Regular(index)) {
        replace_file_by_index(FileIndex::Regular(index));
    } else {
        // load vanilla
        let mut buffer = unsafe { std::slice::from_raw_parts_mut(data, max_len) };
        match arc.get_file_contents(path_hash, *REGION) {
            Ok(contents) => {
                buffer.write_all(&contents).unwrap();
            }
            Err(_) => panic!("Failed to load fallback file {:#x?}", path_hash)
        }
    }
}

#[hook(offset = 0x34e3c0c, inline)]
fn before_read(ctx: &InlineCtx) {
    unsafe {
        let data_idx = *ctx.registers[9].w.as_ref();
        let dir_offset_idx = *ctx.registers[8].w.as_ref();
        let arc = LoadedTables::get_arc();
        let comp_size = arc.get_file_datas()[data_idx as usize].comp_size;
        let decomp_size = arc.get_file_datas()[data_idx as usize].decomp_size;
        trace!(
            "[ResLoadingThread | #{} | #{}] Preparing to load file with compressed size {:#x} and decompressed size {:#x}.",
            data_idx.green(),
            dir_offset_idx.green(),
            comp_size.red(),
            decomp_size.yellow()
        );
    }
}

#[hook(offset = 0x34e3c94, inline)]
fn before_inflation(ctx: &InlineCtx) {
    unsafe {
        let left_to_read = *ctx.registers[27].w.as_ref();
        trace!(
            "[ResLoadingThread] Left to read after this cycle: {:#x}.",
            left_to_read.red()
        );
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

        let file_path = &arc.get_file_paths()[path_idx];
        let hash = file_path.path.hash40();

        info!(
            "[ResInflateThread | #{} | Type: {} | {} / {}] Incoming '{}'",
            path_idx.green(),
            (*ctx.registers[21].w.as_ref()).green(),
            (*ctx.registers[27].x.as_ref()).yellow(),
            res_service.processing_file_idx_count.yellow(),
            hashes::get(hash).bright_yellow()
        );

        let mut incoming = INCOMING_LOAD.write();

        *incoming = IncomingLoad::None;

        let ext_callbacks = EXT_CALLBACKS.read();
        if !ext_callbacks.is_empty() {
            let ext = file_path.path.hash40();
            if ext_callbacks.contains_key(&ext) {
                *incoming = IncomingLoad::ExtCallback(ext, info_indice_index);
                return
            }
        }

        if let Ok(context) = get_from_file_info_indice_index!(info_indice_index) {
            *incoming = IncomingLoad::Index(FileIndex::Regular(context.index));
            info!(
                "[ResInflateThread | #{}] Added index {} to the queue",
                path_idx.green(),
                usize::from(context.index).green()
            );
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
    let incoming = INCOMING_LOAD.read();

    match *incoming {
        IncomingLoad::Index(index) => replace_file_by_index(index),
        IncomingLoad::ExtCallback(ext, index) => replace_extension_callback(ext, index),
        IncomingLoad::None => (),
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

    let incoming = INCOMING_LOAD.read();

    match *incoming {
        IncomingLoad::Index(FileIndex::Regular(FileInfoIndiceIdx(0))) | IncomingLoad::None => (),
        IncomingLoad::Index(index) => replace_file_by_index(index),
        IncomingLoad::ExtCallback(ext, index) => replace_extension_callback(ext, index),
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

pub static mut ORIGINAL_SHARED_INDEX: u32 = 0;

// static mut LUT_LOADER_HANDLE: Option<std::thread::JoinHandle<()>> = None;

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
        skyline_web::DialogOk::ok(
            "The update was downloaded successfully<br>ARCropolis will now reboot.",
        );
        skyline::nn::oe::RestartProgramNoArgs();
    }

    if let Ok(changelog) = std::fs::read_to_string(
        "sd:/atmosphere/contents/01006A800016E000/romfs/arcropolis/changelog.toml",
    ) {
        match toml::from_str(&changelog) {
            Ok(changelog) => {
                menus::display_update_page(&changelog);
                std::fs::remove_file(
                    "sd:/atmosphere/contents/01006A800016E000/romfs/arcropolis/changelog.toml",
                )
                .unwrap();
            }
            Err(_) => {
                warn!("Changelog could not be parsed. Is the file malformed?");
            }
        }
    }

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

        // unsharing::reshare_dir_info(Hash40::from("fighter/jack/c00"));
        // unsharing::reshare_dir_info(Hash40::from("fighter/jack/c01"));
        // unsharing::unshare_files_in_directory(Hash40::from("fighter/koopa/c01"), vec![
        //     Hash40::from("fighter/koopa/model/body/c01/dark_model.numatb"),
        //     Hash40::from("fighter/koopa/model/body/c01/deyes_eye_koopa_d_col.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/deyes_eye_koopa_wd_col.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/eye_koopa_b_col.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/eye_koopa_g_col.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/eye_koopa_shell_col.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/eye_koopa_w_col.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/eye_koopa_w_nor.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/eye_koopa_w_prm.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/leyes_eye_koopa_l_col.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/light_model.numatb"),
        //     Hash40::from("fighter/koopa/model/body/c01/metal_koopa_001_col.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/metal_koopa_001_nor.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/metal_koopa_001_prm.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/metamon_model.numatb"),
        //     Hash40::from("fighter/koopa/model/body/c01/model.numatb"),
        //     Hash40::from("fighter/koopa/model/body/c01/model.numdlb"),
        //     Hash40::from("fighter/koopa/model/body/c01/model.numshb"),
        //     Hash40::from("fighter/koopa/model/body/c01/model.numshexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/model.nusrcmdlb"),
        //     Hash40::from("fighter/koopa/model/body/c01/skin_koopa_001_col.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/skin_koopa_001_nor.nutexb"),
        //     Hash40::from("fighter/koopa/model/body/c01/skin_koopa_001_prm.nutexb"),
        // ]);

        // unsharing::unshare_files_in_directory(Hash40::from("fighter/lucario/c01"), vec![
        //     Hash40::from("fighter/lucario/model/body/c01/alp_lucario_001_col.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c01/alp_lucario_001_nor.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c01/alp_lucario_001_prm.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c01/eye_lucario_b_col.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c01/eye_lucario_g_col.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c01/eye_lucario_w_col.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c01/eye_lucario_w_nor.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c01/eye_lucario_w_prm.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c01/model.numdlb"),
        //     Hash40::from("fighter/lucario/model/body/c01/model.numshb"),
        // ]);
        // println!("unsharing lucario");
        ORIGINAL_SHARED_INDEX = LoadedTables::get_arc().get_shared_data_index();
        unsharing::unshare_files(Hash40::from("stage"));
        // println!("unsharing cloud");
        // unsharing::unshare_files(Hash40::from("fighter/cloud"));
        // println!("unsharing bowser");
        // unsharing::unshare_files(Hash40::from("fighter/koopa"));
        // println!("unsharing battlefield");
        // std::thread::sleep(std::time::Duration::from_millis(200));
        // unsharing::unshare_files(Hash40::from("stage/battlefield"));
        // unsharing::unshare_files_in_directory(Hash40::from("fighter/lucario/c00"), vec![
        //     Hash40::from("fighter/lucario/model/body/c00/alp_lucario_001_col.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c00/alp_lucario_001_nor.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c00/alp_lucario_001_prm.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c00/deyes_eye_lucario_d_col.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c00/deyes_eye_lucario_wd_col.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c00/eye_lucario_b_col.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c00/eye_lucario_g_col.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c00/eye_lucario_w_col.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c00/eye_lucario_w_nor.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c00/eye_lucario_w_prm.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c00/eye_mario_w_nor.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c00/leyes_eye_lucario_l_col.nutexb"),
        //     Hash40::from("fighter/lucario/model/body/c00/model.numdlb"),
        //     Hash40::from("fighter/lucario/model/body/c00/model.numshb"),
        //     Hash40::from("fighter/lucario/model/body/c00/model.numshexb"),
        // ]);

        for hti in LoadedTables::get_arc().get_dir_hash_to_info_index() {
            if hti.hash40() == Hash40::from("fighter/lucario/c00") {
                unsafe {
                    LUCARIO_DIR_INFO = hti.index();
                }
            }
            if hti.hash40() == Hash40::from("fighter/lucario/c01") {
                unsafe {
                    LUCARIO_DIR_INFO2 = hti.index();
                }
            }
        }

        // unsharing::unshare_file_in_directory(Hash40::from("fighter/jack/c01"), Hash40::from("fighter/jack/motion/body/c01/a00wait1.nuanmb"));
        // unsharing::unshare_file_in_directory(Hash40::from("fighter/jack/c01"), Hash40::from("fighter/jack/motion/body/c01/a01turn.nuanmb"));
        // unsharing::unshare_file_in_directory(Hash40::from("fighter/jack/c01"), Hash40::from("fighter/jack/motion/body/c01/a05squat.nuanmb"));
        // unsharing::unshare_file_in_directory(Hash40::from("fighter/jack/c01"), Hash40::from("fighter/jack/motion/body/c01/a05squatwait.nuanmb"));
        // unsharing::unshare_file_in_directory(Hash40::from("fighter/jack/c01"), Hash40::from("fighter/jack/motion/body/c01/c00attack11.nuanmb"));
        // unsharing::unshare_file_in_directory(Hash40::from("fighter/jack/c01"), Hash40::from("fighter/jack/motion/body/c01/c00attack12.nuanmb"));
        // unsharing::unshare_file_in_directory(Hash40::from("fighter/jack/c01"), Hash40::from("fighter/jack/motion/body/c01/c00attackdash.nuanmb"));
        // unsharing::unshare_file_in_directory(Hash40::from("fighter/jack/c01"), Hash40::from("fighter/jack/motion/body/c01/c05attackairf.nuanmb"));
        // unsharing::unshare_file_in_directory(Hash40::from("fighter/jack/c01"), Hash40::from("fighter/jack/motion/body/c01/c05attackairhi.nuanmb"));
        // unsharing::reshare_dir_info(Hash40::from("fighter/jack/c02"));
        // unsharing::reshare_dir_info(Hash40::from("fighter/jack/c03"));
        // unsharing::reshare_dir_info(Hash40::from("fighter/jack/c04"));
        // unsharing::reshare_dir_info(Hash40::from("fighter/jack/c05"));
        // unsharing::reshare_dir_info(Hash40::from("fighter/jack/c06"));
        // unsharing::reshare_dir_info(Hash40::from("fighter/jack/c07"));

        lazy_static::initialize(&MOD_FILES);

        nn::oe::SetCpuBoostMode(nn::oe::CpuBoostMode::Disabled);
    }
}

#[skyline::hook(replace = skyline::nn::fs::OpenFile)]
unsafe fn open_file(handle: u64, path: *const i8, open_mode: i32) -> u32 {
    let string = ffi::CStr::from_ptr(path);
    let string = string.to_str().unwrap();
    println!("Reading file: {}", string);
    original!()(handle, path, open_mode)
}

static mut LUCARIO_DIR_INFO: u32 = 0xFFFFFF;
static mut LUCARIO_DIR_INFO2: u32 = 0xFFFFFF;

#[skyline::hook(offset = 0x34e3e24, inline)]
pub unsafe fn load_node(ctx: &mut skyline::hooks::InlineCtx) {
    if *ctx.registers[26].x.as_ref() == *ctx.registers[8].x.as_ref() {
        println!("Skipping this one");
        return;
    }
    let load_info = *ctx.registers[26].x.as_ref() as *mut res_list::ListNode;
    let load_info = &mut *load_info;
    // for entry in list.iter() {
    //     if entry.directory_index == 0x472 {
    //         should = true;
    //         break;
    //     }
    // }
    // if should {
    //     list.insert(LoadInfo { ty: LoadType::File, filepath_index: 0x10000, directory_index: 0xFFFFFF, directory_info: 0x0 });
    // }
    match load_info.data.ty {
        res_list::LoadType::Directory => {
            // if load_info.value.directory_index == 0x472 { // hardcode joker
            //     let arc = LoadedTables::get_arc();
            //     let file_info_range = arc.get_dir_infos()[load_info.value.directory_index as usize].file_info_range();
            //     let filepath_indices: Vec<smash_arc::FilePathIdx> = arc.get_file_infos()[file_info_range].iter().map(|x| x.file_path_index).collect();

            //     let mut curr = load_info as *mut ListNode;
            //     for idx in filepath_indices.into_iter() {
            //         (*(*curr).prev).next = ListNode::insert((*curr).next, (*curr).prev, LoadInfo { ty: LoadType::FilepathTable, directory_index: 0xFFFFFF, filepath_index: idx.0 });
            //         (*(*curr).next).prev = (*(*curr).prev).next;
            //     }
            //     *ctx.registers[26].x.as_mut() = (*load_info.prev).next as u64;
            // }
            log::debug!("DirectoryEntry: {:#x}, DirectoryInfo: {:#x}", load_info.data.directory_index, load_info.data.directory_info);
        },
        res_list::LoadType::File => {
            log::debug!("FilepathEntry: {:#x}", load_info.data.filepath_index);
        },
        _ => {
            log::debug!("Unknown load type");
        }
    }
}

static mut RES_LOADING_THREAD_LOCK: bool = false;

#[skyline::hook(offset = 0x34e42f8, inline)]
unsafe fn res_loop_refresh(_ctx: &skyline::hooks::InlineCtx) {
    res_loop_start(_ctx);
}

#[skyline::hook(offset = 0x34e34c4, inline)]
unsafe fn res_loop_start(_: &skyline::hooks::InlineCtx) {
    use unsharing::LoadedTableAdditions;
    use std::collections::HashMap;
    let unshared_dirs = unsharing::UNSHARED_FILES.lock();
    while RES_LOADING_THREAD_LOCK {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let mut directories = HashMap::new();
    for (x, list) in ResServiceState::get_instance().res_lists.iter_mut().enumerate() {
        for entry in list.iter_mut() {
            match entry.ty {
                LoadType::Directory => {
                    if unshared_dirs.contains_key(&entry.directory_index) {
                        let _ = directories.try_insert(entry.directory_index, x);
                    }
                },
                _ => {}
            }
        }
    }
    for (dir_index, list_index) in directories.iter() {
        let paths_to_load = unshared_dirs.get(dir_index).unwrap();
        for path in paths_to_load.iter() {
            println!("adding: {:#x}", *path);
            ResServiceState::get_instance().res_lists[*list_index].insert(LoadInfo {
                ty: LoadType::File, filepath_index: *path, directory_index: 0xFF_FFFF, directory_info: 0
            });
        }
    }
}

#[skyline::hook(offset = 0x38d03d0, inline)]
unsafe fn insert_inline(ctx: &skyline::hooks::InlineCtx) {
    insert_hook(std::mem::transmute(*ctx.registers[0].x.as_ref() as *const ()), std::mem::transmute(*ctx.registers[1].x.as_ref() as *const ()));
}

#[skyline::hook(offset = 0x34dfcdc, inline)]
unsafe fn test(ctx: &skyline::hooks::InlineCtx) {
    println!("{:#x}", *ctx.registers[0].x.as_ref());
}

#[skyline::hook(offset = 0x34e4e48, inline)]
unsafe fn null1(_: &InlineCtx) {
    log::debug!("Setting nullptr1");
}

#[skyline::hook(offset = 0x34e4e38, inline)]
unsafe fn null2(ctx: &InlineCtx) {
    if *ctx.registers[17].w.as_ref() != 0 {
        log::debug!("Setting nullptr2");
    }
}

#[skyline::hook(offset = 0x34e4e9c, inline)]
unsafe fn flags(ctx: &InlineCtx) {
    let t2_ptr = *ctx.registers[26].x.as_ref() as *mut Table2Entry;
    if t2_ptr.is_null() {
        println!("Table2Entry is Null!!!!!!");
    } else {
        let t2_ptr = t2_ptr as usize;
        let table_ptr = LoadedTables::get_instance().table_2().as_ptr() as usize;
        let index = (t2_ptr - table_ptr) / std::mem::size_of::<Table2Entry>();
        println!("Table2 index: {:#x}", index);
    }
}

// #[skyline::hook(offset = 0x34e4eec, inline)]
// unsafe fn flags(ctx: &InlineCtx) {
//     let t2_ptr = *ctx.registers[8].w.as_ref();
//     if t2_ptr != 0 {
//         println!("Skiping the file load!");
//     }
// }

unsafe fn insert_hook(root: &[u64; 0x5], node: &[u64; 0x5]) {
    println!("root: {:#x?}", root);
    println!("node: {:#x?}", node);
    // original!()(root, node)
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    // Load hashes from rom:/skyline/hashes.txt if the file is present
    hashes::init();
    // Look for the offset of the various functions to hook
    offsets::search_offsets();

    install_hooks!(
        res_loop_start,
        res_loop_refresh,
        open_file,
        // null1,
        // null2,
        // before_read,
        // flags,
        // before_inflation,
        // load_node,
        initial_loading,
        inflate_incoming,
        memcpy_uncompressed,
        memcpy_uncompressed_2,
        memcpy_uncompressed_3,
        load_directory_hook,
        manual_hook,
        change_version_string,
        stream::lookup_by_stream_hash,
        load_node
    );

    fn receive(args: Vec<String>) {
        // println!("{:?}", remote::arc::Arc::from_iter(args.into_iter()));
        let _ = cli::send(remote::handle_command(args).as_str());
    }

    std::thread::spawn(|| {
        skyline_communicate::set_on_receive(cli::Receiver::CLIStyle(receive));
        skyline_communicate::start_server("ARCropolis", 6968);
    });

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
