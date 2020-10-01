#![feature(proc_macro_hygiene)]
#![feature(str_strip)]

use std::fs;
use std::io::Write;
use std::path::Path;

use skyline::{hook, install_hooks};
use skyline::hooks::InlineCtx;

use skyline::nn;

mod config;
mod hashes;
mod stream;

mod replacement_files;

mod offsets;
use offsets::{ ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET, IDK_OFFSET, PARSE_EFF_OFFSET, PARSE_NUTEXB_OFFSET, PARSE_PARAM_OFFSET, PARSE_EFF_NUTEXB_OFFSET };

use smash::resource::{FileState, LoadedTables, ResServiceState};

use owo_colors::{ OwoColorize };

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
    log!("--- [Idk] ---");
    handle_file_load(table1_idx);
    original!()(res_state, table1_idx, flag_related);
}

#[hook(offset = ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET)]
unsafe fn add_idx_to_table1_and_table2(loaded_table: *const LoadedTables, table1_idx: u32) {
    log!("--- [AddIdx] ---");
    handle_file_load(table1_idx);
    original!()(loaded_table, table1_idx);
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

#[hook(offset = 0x32f89b4, inline)]
fn parse_model_xmb(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*((*ctx.registers[22].x.as_ref()) as *const u32));
    }
}

#[hook(offset = 0x3016524, inline)]
fn parse_arc_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[8].w.as_ref());
    }
}

#[hook(offset = 0x3476808, inline)]
fn parse_font_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*((*ctx.registers[19].x.as_ref()) as *const u32));
    }
}

#[hook(offset = 0x32da328, inline)]
fn parse_numdlb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[1].w.as_ref());
    }
}

#[hook(offset = 0x32e18c4, inline)]
fn parse_numshexb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[25].w.as_ref());
    }
}

#[hook(offset = 0x330559c, inline)]
fn parse_numatb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[23].w.as_ref());
    }
}

#[skyline::from_offset(0x3643590)]
pub fn smash_free_mayb(src: *const skyline::libc::c_void);

fn handle_file_load(table1_idx: u32) {
    let loaded_tables = LoadedTables::get_instance();
    let mutex = loaded_tables.mutex;
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();
    let internal_filepath = hashes::get(hash).unwrap_or(&"Unknown");

    let mut table2entry = match loaded_tables.get_t2_mut(table1_idx) {
        Ok(entry) => entry,
        Err(_) => {
            return;
        }
    };

    log!(
        "[ARC::Loading | #{}] File: {}, Hash: {}, Status: {}",
        table1_idx.green(),
        internal_filepath.bright_yellow(),
        hash.cyan(),
        table2entry.bright_magenta(),
    );

    // Println!() calls are on purpose so these show up no matter what.
    if let Some(file_ctx) = get_from_hash!(hash) {
        // Some formats don't appreciate me replacing the data pointer
        if !is_file_allowed(&file_ctx.path) {
            return;
        }

        println!(
            "[ARC::Loading | #{}] Hash matching for file: '{}'",
            table1_idx.green(),
            file_ctx.path.display().bright_yellow(),
        );

        if table2entry.state == FileState::Loaded {
            if file_ctx.path.extension().unwrap().to_str().unwrap() == "bntx" {
                handle_file_overwrite(table1_idx);
                return;
            }
            if file_ctx.path.extension().unwrap().to_str().unwrap() == "numshb" {
                handle_file_overwrite(table1_idx);
                return;
            }
            if file_ctx.path.extension().unwrap().to_str().unwrap() == "nusktb" {
                handle_file_overwrite(table1_idx);
                return;
            }
            if file_ctx.path.extension().unwrap().to_str().unwrap() == "nuanmb" {
                handle_file_overwrite(table1_idx);
                return;
            }
            if file_ctx.path.file_name().unwrap().to_str().unwrap() == "motion_list.bin" {
                handle_file_overwrite(table1_idx);
                return;
            }
        }

        println!(
            "[ARC::Replace | #{}] Replacing '{}'",
            table1_idx.green(),
            internal_filepath.bright_yellow(),
        );

        unsafe {
            nn::os::LockMutex(mutex);
        }

        let data = fs::read(&file_ctx.path).unwrap().into_boxed_slice();
        let data = Box::leak(data);

        unsafe {
            //skyline::libc::free(table2entry.data as *const skyline::libc::c_void);
            smash_free_mayb(table2entry.data as *const skyline::libc::c_void);
        }

        table2entry.data = data.as_ptr();
        table2entry.state = FileState::Loaded;
        table2entry.flags = 43;

        unsafe {
            nn::os::UnlockMutex(mutex);
        }
    }
}

fn handle_file_overwrite(table1_idx: u32) {
    let loaded_tables = LoadedTables::get_instance();
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();
    let internal_filepath = hashes::get(hash).unwrap_or(&"Unknown");

    let t2_entry = match loaded_tables.get_t2_mut(table1_idx) {
        Ok(entry) => entry,
        Err(_) => {
            return;
        }
    };

    log!(
        "[ARC::Loading | #{}] File: {}, Hash: {}, Status: {}",
        table1_idx.green(),
        internal_filepath.bright_yellow(),
        hash.cyan(),
        t2_entry.bright_magenta(),
    );

    if let Some(file_ctx) = get_from_hash!(hash) {
        println!(
            "[ARC::Loading | #{}] Hash matching for file: '{}'",
            table1_idx.green(),
            file_ctx.path.display().bright_yellow(),
        );

        if t2_entry.state != FileState::Loaded {
            return;
        }

        println!(
            "[ARC::Replace | #{}] Replacing '{}'",
            table1_idx.green(),
            internal_filepath.bright_yellow(),
        );

        let file = fs::read(&file_ctx.path).unwrap();
        let file_slice = file.as_slice();

        unsafe {
            let mut data_slice =
                std::slice::from_raw_parts_mut(t2_entry.data as *mut u8, file_slice.len());
            data_slice.write(file_slice).unwrap();
        }
    }
}

fn handle_texture_files(table1_idx: u32) {
    unsafe {
        let loaded_tables = LoadedTables::get_instance();
        let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();
        let internal_filepath = hashes::get(hash).unwrap_or(&"Unknown");

        if let Some(file_ctx) = get_from_hash!(hash) {
            println!(
                "[ARC::Loading | #{}] Hash matching for file: '{}'",
                table1_idx.green(),
                file_ctx.path.display().bright_yellow(),
            );    

            let table2entry = match loaded_tables.get_t2_mut(table1_idx) {
                Ok(entry) => entry,
                Err(_) => {
                    return;
                }
            };

            if table2entry.state != FileState::Loaded {
                return;
            }

            println!(
                "[ARC::Replace | #{}] Replacing '{}'",
                table1_idx.green(),
                internal_filepath.bright_yellow(),
            );

            let file = fs::read(&file_ctx.path).unwrap();
            let file_slice = file.as_slice();

            let orig_size = LoadedTables::get_instance()
                .get_arc()
                .get_subfile_by_t1_index(table1_idx)
                .decompressed_size as usize;

            let mut data_slice =
                std::slice::from_raw_parts_mut(table2entry.data as *mut u8, orig_size);

            if orig_size > file_slice.len() {
                // Copy our new footer at the end
                data_slice[orig_size - 0xB0..orig_size]
                    .copy_from_slice(&file_slice[file_slice.len() - 0xB0..file_slice.len()]);
                // Copy the content at the beginning
                data_slice[0..file_slice.len() - 0xB0]
                    .copy_from_slice(&file_slice[0..file_slice.len() - 0xB0]);
            } else {
                data_slice.write(file_slice).unwrap();
            }
        }
    }
}

pub fn is_file_allowed(filepath: &Path) -> bool {
    // Check extensions
    match filepath.extension().unwrap().to_str().unwrap() {
        "nutexb" | "eff" | "prc" | "stdat" | "stprm" | "xmb" | "arc" | "bfotf" | "bfttf" | "numdlb" | "numatb" | "numshexb" => false,
        &_ => true,
    }
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
        parse_arc_file,
        parse_font_file,
        parse_numdlb_file,
        parse_numshexb_file,
        parse_numatb_file,
    );

    println!(
        "ARCropolis v{} - File replacement plugin is now installed",
        env!("CARGO_PKG_VERSION")
    );
}
