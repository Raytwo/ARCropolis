#![feature(proc_macro_hygiene)]
#![feature(str_strip)]

use std::io::Write;
use std::ffi::CStr;
use std::path::Path;

use skyline::hooks::InlineCtx;
use skyline::{hook, install_hooks};

mod config;
mod hashes;
mod stream;

mod replacement_files;
use replacement_files::{ FileCtx, ARC_CALLBACKS, subscribe_callback };

mod offsets;
use offsets::{
    ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET, IDK_OFFSET, PARSE_EFF_NUTEXB_OFFSET, PARSE_EFF_OFFSET,
    PARSE_NUTEXB_OFFSET, PARSE_PARAM_OFFSET, PARSE_MODEL_XMB_OFFSET, PARSE_ARC_FILE_OFFSET, PARSE_FONT_FILE_OFFSET,
    PARSE_NUMATB_NUTEXB_OFFSET, PARSE_NUMSHEXB_FILE_OFFSET, PARSE_NUMATB_FILE_OFFSET, PARSE_NUMDLB_FILE_OFFSET,
    PARSE_LOG_XMB_OFFSET, PARSE_MODEL_XMB_2_OFFSET,
};

use owo_colors::OwoColorize;

use smash::resource::{FileState, LoadedTables, ResServiceState, Table2Entry};

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        // Uncomment to enable logging
        if crate::config::CONFIG.misc.debug {
            println!($($arg)*);
        }
    };
}

#[hook(offset = IDK_OFFSET)]
unsafe fn idk(res_state: *const ResServiceState, table1_idx: u32, flag_related: u32) {
    handle_file_load(table1_idx);
    original!()(res_state, table1_idx, flag_related);
}

#[hook(offset = ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET)]
unsafe fn add_idx_to_table1_and_table2(loaded_table: *const LoadedTables, table1_idx: u32) {
    handle_file_load(table1_idx);
    original!()(loaded_table, table1_idx);
}

#[hook(offset = PARSE_EFF_OFFSET, inline)]
fn parse_eff(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[10].w.as_ref());
    }
}

#[hook(offset = PARSE_PARAM_OFFSET, inline)]
fn parse_param_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*((*ctx.registers[20].x.as_ref()) as *const u32));
    }
}

#[hook(offset = PARSE_MODEL_XMB_OFFSET, inline)]
fn parse_model_xmb(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[22].w.as_ref());
    }
}

#[hook(offset = PARSE_MODEL_XMB_2_OFFSET, inline)]
fn parse_model_xmb2(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[22].w.as_ref());
    }
}

#[hook(offset = PARSE_LOG_XMB_OFFSET, inline)]
fn parse_log_xmb(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[19].w.as_ref());
    }
}

#[hook(offset = PARSE_ARC_FILE_OFFSET, inline)]
fn parse_arc_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[8].w.as_ref());
    }
}

#[hook(offset = PARSE_FONT_FILE_OFFSET, inline)]
fn parse_font_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*((*ctx.registers[19].x.as_ref()) as *const u32));
    }
}

#[hook(offset = PARSE_NUMDLB_FILE_OFFSET, inline)]
fn parse_numdlb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[1].w.as_ref());
    }
}

#[hook(offset = PARSE_NUMSHEXB_FILE_OFFSET, inline)]
fn parse_numshexb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[25].w.as_ref());
    }
}

#[hook(offset = PARSE_NUMATB_FILE_OFFSET, inline)]
fn parse_numatb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[23].w.as_ref());
    }
}

#[hook(offset = PARSE_NUTEXB_OFFSET, inline)]
fn parse_fighter_nutexb(ctx: &InlineCtx) {
    unsafe {
        handle_texture_files(*ctx.registers[25].w.as_ref());
    }
}

#[hook(offset = PARSE_EFF_NUTEXB_OFFSET, inline)]
fn parse_eff_nutexb(ctx: &InlineCtx) {
    unsafe {
        handle_texture_files(*ctx.registers[24].w.as_ref());
    }
}

#[hook(offset = PARSE_NUMATB_NUTEXB_OFFSET, inline)]
fn parse_numatb_nutexb(ctx: &InlineCtx) {
    unsafe {
        handle_texture_files(*ctx.registers[25].w.as_ref());
    }
}

fn get_filectx_by_t1index<'a>(table1_idx: u32) -> Option<(parking_lot::MappedRwLockReadGuard<'a, FileCtx>, &'a mut Table2Entry)> {
    let loaded_tables = LoadedTables::get_instance();
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();

    let table2entry = match loaded_tables.get_t2_mut(table1_idx) {
        Ok(entry) => entry,
        Err(_) => {
            return None;
        }
    };

    log!("[ARC::Loading | #{}] File: {}, Hash: {}, Status: {}", table1_idx.green(), hashes::get(hash).unwrap_or(&"Unknown").bright_yellow(), hash.cyan(), table2entry.bright_magenta());

    let file_ctx = get_from_hash!(hash);
    if file_ctx.is_some() {
        let file_ctx = parking_lot::MappedRwLockReadGuard::map(file_ctx, |x| x.unwrap());
        println!("[ARC::Loading | #{}] Hash matching for file: '{}'", table1_idx.green(), file_ctx.path.display().bright_yellow());
        Some((file_ctx, table2entry))
    } else {
        None
    }
}

fn handle_file_load(table1_idx: u32) {
    // Println!() calls are on purpose so these show up no matter what.
    if let Some((file_ctx, table2entry)) = get_filectx_by_t1index(table1_idx) {
        // Some formats don't appreciate me replacing the data pointer
        if !is_file_allowed(&file_ctx.path) {
            return;
        }

        if table2entry.state == FileState::Loaded {
            // For files that are too dependent on timing, make sure the pointer is overwritten instead of swapped
            match file_ctx.path.extension().unwrap().to_str().unwrap() {
                "bntx" | "nusktb" | "bin" => {
                    handle_file_overwrite(table1_idx);
                    return;
                }
                &_ => {}
            }
        }

        println!("[ARC::Replace | #{}] Replacing '{}'", table1_idx.green(), hashes::get(file_ctx.hash).unwrap_or(&"Unknown").bright_yellow());

        let data = file_ctx.get_file_content().into_boxed_slice();
        let data = Box::leak(data);

        unsafe {
            if !table2entry.data.is_null() {
                skyline::libc::free(table2entry.data as *const skyline::libc::c_void);
            }
        }

        table2entry.data = data.as_ptr();
        table2entry.state = FileState::Loaded;
        table2entry.flags = 43;
    }
}

fn handle_file_overwrite(table1_idx: u32) {
    if let Some((file_ctx, table2entry)) = get_filectx_by_t1index(table1_idx) {
        if table2entry.state != FileState::Loaded {
            return;
        }

        let hash = file_ctx.hash;

        let orig_size = file_ctx.get_subfile(table1_idx).decompressed_size as usize;

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
                file_slice.copy_from_slice(file_ctx.get_file_content().as_slice());
            } else {
                // The file does not actually exist on the SD, so we abort here
                return;
            }
        }

        println!("[ARC::Replace | #{}] Replacing '{}'", table1_idx.green(), hashes::get(file_ctx.hash).unwrap_or(&"Unknown").bright_yellow());

        unsafe {
            let mut data_slice =
                std::slice::from_raw_parts_mut(table2entry.data as *mut u8, file_slice.len());
            data_slice.write(&file_slice).unwrap();
        }
    }
}

fn handle_texture_files(table1_idx: u32) {
    if let Some((file_ctx, table2entry)) = get_filectx_by_t1index(table1_idx) {
        if table2entry.state != FileState::Loaded {
            return;
        }

        let hash = file_ctx.hash;

        let orig_size = file_ctx.get_subfile(table1_idx).decompressed_size as usize;

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
                file_slice.copy_from_slice(file_ctx.get_file_content().as_slice());
            } else {
                // The file does not actually exist on the SD, so we abort here
                return;
            }
        }

        println!("[ARC::Replace | #{}] Replacing '{}'", table1_idx.green(), hashes::get(file_ctx.hash).unwrap_or(&"Unknown").bright_yellow());

        unsafe {
            let mut data_slice = std::slice::from_raw_parts_mut(table2entry.data as *mut u8, orig_size);

            if orig_size > file_slice.len() {
                // Copy our new footer at the end
                data_slice[orig_size - 0xB0..orig_size].copy_from_slice(&file_slice[file_slice.len() - 0xB0..file_slice.len()]);
                // Copy the content at the beginning
                data_slice[0..file_slice.len() - 0xB0].copy_from_slice(&file_slice[0..file_slice.len() - 0xB0]);
            } else {
                data_slice.write(&file_slice).unwrap();
            }
        }
    }
}

pub fn is_file_allowed(filepath: &Path) -> bool {
    // Check extensions
    match filepath.extension().unwrap().to_str().unwrap() {
        "nutexb" | "eff" | "prc" | "xmb" | "arc" | "bfotf" | "bfttf" | "numdlb" | "numatb" | "numshexb" => false,
        &_ => true,
    }
}

#[hook(offset = 0x34b8320)]
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


pub extern "C" fn callback_test(hash: u64, buffer: *const skyline::libc::c_void, buffer_size: usize) -> bool {
    println!("Callback is being hit");
    false
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    // Load hashes from rom:/skyline/hashes.txt if the file is present
    hashes::init();
    // Look for the offset of the various functions to hook
    offsets::search_offsets();

    install_hooks!(
        idk,
        add_idx_to_table1_and_table2,
        stream::lookup_by_stream_hash,
        parse_fighter_nutexb,
        parse_eff_nutexb,
        parse_eff,
        parse_param_file,
        parse_model_xmb,
        parse_model_xmb2,
        parse_log_xmb,
        parse_arc_file,
        parse_font_file,
        parse_numdlb_file,
        parse_numshexb_file,
        parse_numatb_file,
        parse_numatb_nutexb,
        change_version_string,
    );

    println!(
        "ARCropolis v{} - File replacement plugin is now installed",
        env!("CARGO_PKG_VERSION")
    );
}
