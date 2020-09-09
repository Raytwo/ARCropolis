#![feature(proc_macro_hygiene)]
#![feature(str_strip)]

use std::fs;
use std::fs::File;
use std::io::Write;
use std::slice;

use skyline::hooks::InlineCtx;
use skyline::{hook, install_hooks, nn};

mod hashes;
mod stream;

mod patching;
use patching::{
    ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET, IDK_OFFSET, PARSE_EFF_OFFSET, PARSE_NUTEXB_OFFSET,
    RES_SERVICE_INITIALIZED_OFFSET,
};

mod replacement_files;
use replacement_files::ARC_FILES;

mod resource;
use resource::*;

mod config;
use config::CONFIG;

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
        "[ARC::Loading | #{}] File path: {}, Hash: {}, {}",
        table1_idx,
        internal_filepath,
        hash,
        table2entry
    );

    // Println!() calls are on purpose so these show up no matter what.
    if let Some(path) = ARC_FILES.get_from_hash(hash) {
        // Some formats don't appreciate me replacing the data pointer
        match path.as_path().extension().unwrap().to_str().unwrap() {
            "nutexb" | "eff" | "prc" | "stdat" | "stprm" => return,
            &_ => (),
        }

        println!(
            "[ARC::Replace] Hash matching for file path: {}",
            path.display()
        );

        println!("[ARC::Replace] Replacing {}", internal_filepath);

        // This is a personal request, don't mind it too much.
        if let Some(_) = CONFIG.misc.mowjoh {
            skyline::error::show_error(
                69,
                &format!("[ARC::Replace] Replacing {}", internal_filepath),
                &format!("{}", internal_filepath),
            );
        }

        unsafe {
            nn::os::LockMutex(mutex);
        }

        let data = fs::read(path).unwrap().into_boxed_slice();
        let data = Box::leak(data);

        unsafe { skyline::libc::free(table2entry.data as *const skyline::libc::c_void); }

        table2entry.data = data.as_ptr();
        table2entry.state = FileState::Loaded;
        table2entry.flags = 43;

        unsafe {
            nn::os::UnlockMutex(mutex);
        }

        println!("[ARC::Replace] Table2 entry status: {}", table2entry);
    }
}

#[hook(offset = PARSE_NUTEXB_OFFSET, inline)]
fn parse_fighter_nutexb(ctx: &InlineCtx) {
    unsafe {
        handle_texture_files(*ctx.registers[25].w.as_ref());
    }
}

#[hook(offset = 0x3278f20, inline)]
fn parse_eff_nutexb(ctx: &InlineCtx) {
    unsafe {
        handle_texture_files(*ctx.registers[24].w.as_ref());
    }
}

#[hook(offset = 0x3436890, inline)]
fn parse_prc_file(ctx: &InlineCtx) {
    unsafe {
        let table1_idx = (*ctx.registers[20].x.as_ref()) as *const u32;
        let loaded_tables = LoadedTables::get_instance();
        let hash = loaded_tables.get_hash_from_t1_index(*table1_idx).as_u64();
        let internal_filepath = hashes::get(hash).unwrap_or(&"Unknown");

        let mut t2_entry = match loaded_tables.get_t2_mut(*table1_idx) {
            Ok(entry) => entry,
            Err(_) => {
                return;
            }
        };

        println!(
            "[ARC::Loading | #{}] File path: {}, Hash: {}, {}",
            *table1_idx,
            internal_filepath,
            hash,
            t2_entry
        );

        if let Some(path) = ARC_FILES.get_from_hash(hash) {
            println!(
                "[ARC::Replace] Hash matching for file path: {}",
                path.display()
            );

            println!("[ARC::Replace] Replacing {}...", internal_filepath);

            let file = fs::read(path).unwrap();
            let file_slice = file.as_slice();

            let mut data_slice =
                std::slice::from_raw_parts_mut(t2_entry.data as *mut u8, file_slice.len());

            data_slice.write(file_slice);
        }
    }
}

fn handle_texture_files(table1_idx: u32) {
    unsafe {
        let loaded_tables = LoadedTables::get_instance();
        let mutex = loaded_tables.mutex;
        let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();
        let internal_filepath = hashes::get(hash).unwrap_or(&"Unknown");

        if let Some(path) = ARC_FILES.get_from_hash(hash) {
            println!(
                "[ARC::Replace] Hash matching for file path: {}",
                path.display()
            );

            let mut table2entry = match loaded_tables.get_t2_mut(table1_idx) {
                Ok(entry) => entry,
                Err(_) => {
                    return;
                }
            };

            if table2entry.state != FileState::Loaded {
                return;
            }

            println!("[ARC::Replace] Replacing {}...", internal_filepath);

            let file = fs::read(path).unwrap();
            let file_slice = file.as_slice();

            let orig_size = LoadedTables::get_instance()
                .get_arc()
                .get_subfile_by_t1_index(table1_idx)
                .decompressed_size as usize;

            let mut data_slice =
                std::slice::from_raw_parts_mut(table2entry.data as *mut u8, orig_size);

            if (orig_size > file_slice.len()) {
                // Copy our new footer at the end
                data_slice[orig_size - 0xB0..orig_size]
                    .copy_from_slice(&file_slice[file_slice.len() - 0xB0..file_slice.len()]);
                // Copy the content at the beginning
                data_slice[0..file_slice.len() - 0xB0]
                    .copy_from_slice(&file_slice[0..file_slice.len() - 0xB0]);
            } else {
                data_slice.write(file_slice);
            }
        }
    }
}

#[hook(offset = PARSE_EFF_OFFSET, inline)]
fn parse_eff(ctx: &InlineCtx) {
    unsafe {
        let table1_idx = *ctx.registers[10].w.as_ref();
        let loaded_tables = LoadedTables::get_instance();
        let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();
        let internal_filepath = hashes::get(hash).unwrap_or(&"Unknown");

        let mut t2_entry = match loaded_tables.get_t2_mut(table1_idx) {
            Ok(entry) => entry,
            Err(_) => {
                return;
            }
        };

        log!(
            "[ARC::Loading | #{}] File path: {}, Hash: {}, {}",
            table1_idx,
            internal_filepath,
            hash,
            t2_entry
        );

        if let Some(path) = ARC_FILES.get_from_hash(hash) {
            println!(
                "[ARC::Replace] Hash matching for file path: {}",
                path.display()
            );

            println!("[ARC::Replace] Replacing {}...", internal_filepath);

            let file = fs::read(path).unwrap();
            let file_slice = file.as_slice();

            let mut data_slice =
                std::slice::from_raw_parts_mut(t2_entry.data as *mut u8, file_slice.len());

            data_slice.write(file_slice);
        }
    }
}

#[hook(offset = RES_SERVICE_INITIALIZED_OFFSET, inline)]
fn resource_service_initialized(ctx: &InlineCtx) {
    // Patch filesizes in the Subfile table
    patching::filesize_replacement();
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    // Read the configuration so we can set the filepaths
    lazy_static::initialize(&CONFIG);
    lazy_static::initialize(&ARC_FILES);

    // Load hashes from rom:/skyline/hashes.txt if the file is present
    hashes::init();
    // Look for the offset of the various functions to hook
    patching::search_offsets();
    // Not working so far, does not crash the game
    //patching::shared_redirection();
    // Attempt at expanding table2 (Does not work, do not use!)
    //patching::expand_table2();

    // This is a personal request, don't mind it too much.
    if let Some(_) = CONFIG.misc.mowjoh {
        skyline::error::show_error(69, "I'm Mowjoh!", "No really, he is.");
    }

    install_hooks!(
        idk,
        add_idx_to_table1_and_table2,
        stream::lookup_by_stream_hash,
        parse_fighter_nutexb,
        parse_eff_nutexb,
        parse_eff,
        parse_prc_file,
        resource_service_initialized,
    );

    println!(
        "ARCropolis v{} - File replacement plugin is now installed",
        env!("CARGO_PKG_VERSION")
    );
}
