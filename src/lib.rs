#![feature(proc_macro_hygiene)]
#![feature(asm)]
#![allow(dead_code)]
#![allow(unaligned_references)]

extern crate skyline_communicate as cli;

use skyline::{hook, hooks::InlineCtx, install_hooks, nn};
use std::io::prelude::*;
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
mod remote;
mod replacement_files;
mod runtime;
mod stream;

use config::{CONFIG, REGION};
use replacement_files::{FileCtx, FileIndex, IncomingLoad, INCOMING_LOAD, MOD_FILES};
use runtime::{LoadedTables, ResServiceState, Table2Entry};

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
