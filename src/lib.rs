#![feature(proc_macro_hygiene)]
#![feature(str_strip)]
#![feature(asm)]

use std::io::Write;
use std::ffi::CStr;
use std::net::IpAddr;
use std::sync::atomic::Ordering;

use skyline::hooks::InlineCtx;
use skyline::{nn, hook, install_hooks};

mod config;
use config::CONFIG;
mod hashes;
mod stream;

mod replacement_files;
use replacement_files::{ FileCtx, ARC_FILES, ARC_CALLBACKS, QUEUE_HANDLED, CB_QUEUE };

mod offsets;
use offsets::TITLE_SCREEN_VERSION_OFFSET;

use owo_colors::OwoColorize;

use smash::resource::{FileState, LoadedTables, ResServiceState, Table2Entry, CppVector, FileNX};

use log::{ trace, info };
mod logging;

fn get_filectx_by_t1index<'a>(table1_idx: u32) -> Option<(parking_lot::MappedRwLockReadGuard<'a, FileCtx>, &'a mut Table2Entry)> {
    let loaded_tables = LoadedTables::get_instance();
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();

    let table2entry = match loaded_tables.get_t2_mut(table1_idx) {
        Ok(entry) => entry,
        Err(_) => {
            return None;
        }
    };

    trace!("[ARC::Loading | #{}] File: {}, Hash: {}, Status: {}", table1_idx.green(), hashes::get(hash).unwrap_or(&"Unknown").bright_yellow(), hash.cyan(), table2entry.bright_magenta());

    if QUEUE_HANDLED.swap(true, Ordering::SeqCst) {
        for (hash, ctx) in CB_QUEUE.write().iter_mut() {
            let found = match ARC_FILES.write().0.get_mut(&*hash) {
                Some(context) => {
                    if context.filesize < ctx.filesize {
                        context.filesize = ctx.filesize;
                        ctx.filesize_replacement();
                    }
                    true
                },
                None => false,
            };

            if !found {
                ctx.filesize_replacement();
                ARC_FILES.write().0.insert(*hash, ctx.clone());
            }
        }

        CB_QUEUE.write().clear();
    }

    match get_from_hash!(hash) {
        Ok(file_ctx) => {
            info!("[ARC::Loading | #{}] Hash matching for file: '{}'", table1_idx.green(), file_ctx.path.display().bright_yellow());
            Some((file_ctx, table2entry))
        }
        Err(_) => None,
    }
}

#[hook(offset = TITLE_SCREEN_VERSION_OFFSET)]
fn change_version_string(arg1: u64, string: *const u8) {
    unsafe {
        let original_str = CStr::from_ptr(string as _).to_str().unwrap();

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
}

#[repr(u32)]
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum LoadingType {
    Directory = 0,
    Unk1 = 1,
    Unk2 = 2,
    Unk3 = 3,
    File = 4,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ResService{
    pub mutex: *mut nn::os::MutexType,
    pub res_update_event: *mut nn::os::EventType,
    unk1: *const (),
    pub io_swap_event: *mut nn::os::EventType,
    unk2: *const (),
    pub semaphore1: *const (),
    pub semaphore2: *const (),
    pub res_update_thread: *mut nn::os::ThreadType,
    pub res_loading_thread: *mut nn::os::ThreadType,
    pub res_inflate_thread: *mut nn::os::ThreadType,
    unk4: *const (),
    pub directory_idx_queue: [CppVector<CppVector<u32>>; 4],
    unk6: *const (),
    unk7: *const (),
    unk8: *const (),
    pub loaded_tables: *mut LoadedTables,
    pub unk_region_idx: u32,
    pub regular_region_idx: u32,
    unk9: u32,
    pub state: i16,
    pub is_loader_thread_running: bool,
    unk10: u8,
    pub data_arc_string: [u8; 256],
    unk11: *const (),
    pub data_arc_filenx: *mut *mut FileNX,
    pub buffer_size: usize,
    pub buffer_array: [*const skyline::libc::c_void; 2],
    pub buffer_array_idx: u32,
    unk12: u32,
    pub data_ptr: *const skyline::libc::c_void,
    pub offset_into_read: u64,
    pub processing_file_idx_curr: u32,
    pub processing_file_idx_count: u32,
    pub processing_file_idx_start: u32,
    pub processing_type: LoadingType,
    pub processing_dir_idx_start: u32,
    pub processing_dir_idx_single: u32,
    pub current_index: u32,
    pub current_dir_index: u32,
    //Still need to add some
}

fn handle_file_overwrite_test(table1_idx: u32) {
    if let Some((file_ctx, table2entry)) = get_filectx_by_t1index(table1_idx) {
        if table2entry.state != FileState::Unloaded {
            return;
        }

        let hash = file_ctx.hash;

        let orig_size = file_ctx.filesize as usize;

        let file = vec![0;orig_size];
        let mut file_slice = file.into_boxed_slice();

        let cb_result = match ARC_CALLBACKS.read().get(&hash) {
            Some(cb) => {
                cb(hash, file_slice.as_mut_ptr() as *mut skyline::libc::c_void, orig_size)
            },
            None => false,
        };

        if !cb_result {
            if !file_ctx.virtual_file {
                file_slice = file_ctx.get_file_content().into_boxed_slice();
            } else {
                // The file does not actually exist on the SD, so we abort here
                return;
            }
        }

        info!("[ResInflateThread | #{}] Replacing '{}'", table1_idx.green(), hashes::get(file_ctx.hash).unwrap_or(&"Unknown").bright_yellow());

        unsafe {
            let mut data_slice = std::slice::from_raw_parts_mut(table2entry.data as *mut u8, file_slice.len());
            data_slice.write(&file_slice).unwrap();
        }
    }
}
#[hook(offset = 0x33b71e8, inline)]
fn inflate_incoming(ctx: &InlineCtx) {
    unsafe {
        let loaded_tables = LoadedTables::get_instance();
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);
        let arc = loaded_tables.get_arc();

        // Replace all this mess by Smash-arc
        let current_index = *ctx.registers[27].x.as_ref() as u32;
        let file_infos = arc.file_info;
        let file_info = &*file_infos.offset((res_service.processing_file_idx_start + current_index) as isize);
        let t1_idx = file_info.path_index;
        let hash = loaded_tables.get_hash_from_t1_index(t1_idx).as_u64();

        // Seems to be unused, store it here so the State_change hook can get it back
        res_service.processing_file_idx_curr = t1_idx;

        match ARC_FILES.write().0.get_mut(&hash) {
            Some(context) => {
                context.filesize_replacement();
                println!("[ResInflateThread] Replaced FileData");
            },
            None => {},
        }
    }
}

#[hook(offset = 0x33b7fbc, inline)]
fn state_change(_ctx: &InlineCtx) {
    unsafe {
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);
        handle_file_overwrite_test(res_service.processing_file_idx_curr);
        // Set it back to 0 just in case
        res_service.processing_file_idx_curr = 0;
    }
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    logging::init(CONFIG.logger.as_ref().unwrap().logger_level.into()).unwrap();

    // Check if an update is available
    if skyline_update::check_update(IpAddr::V4(CONFIG.updater.as_ref().unwrap().server_ip), "ARCropolis", env!("CARGO_PKG_VERSION"), CONFIG.updater.as_ref().unwrap().beta_updates) {
        skyline::nn::oe::RestartProgramNoArgs();
    }

    // TODO: Future changelog stuff go here

    // Load hashes from rom:/skyline/hashes.txt if the file is present
    hashes::init();
    // Look for the offset of the various functions to hook
    offsets::search_offsets();

    install_hooks!(
        stream::lookup_by_stream_hash,
        inflate_incoming,
        state_change,
        change_version_string,
    );

    println!(
        "ARCropolis v{} - File replacement plugin is now installed",
        env!("CARGO_PKG_VERSION")
    );
}
