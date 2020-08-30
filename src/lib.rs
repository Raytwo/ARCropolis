#![feature(proc_macro_hygiene)]
#![feature(str_strip)]

use std::fs;
use std::fs::File;
use std::slice;

use skyline::hooks::InlineCtx;
use skyline::{hook, install_hooks, nn};

mod hashes;
mod stream;

mod patching;
use patching::{ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET, IDK_OFFSET};

mod replacement_files;
use replacement_files::{ARC_FILES, STREAM_FILES};

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

fn handle_file_load(table1_idx: u32) {
    let loaded_tables = LoadedTables::get_instance();
    let mutex = loaded_tables.mutex;
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();
    let internal_filepath = hashes::get(hash).unwrap_or(&"Unknown");

    log!(
        "[ARC::Loading | #{}] File path: {}, Hash: {}, {}",
        table1_idx,
        internal_filepath,
        hash,
        loaded_tables
            .get_t1_mut(table1_idx)
            .unwrap()
            .get_t2_entry()
            .unwrap()
    );

    // Println!() calls are on purpose so these show up no matter what.
    if let Some(path) = ARC_FILES.get_from_hash(hash) {
        // Some formats don't appreciate me replacing the data pointer
        match path.as_path().extension().unwrap().to_str().unwrap() {
            "nutexb" => return,
            &_ => (),
        }

        println!(
            "[ARC::Replace] Hash matching for file path: {}",
            path.display()
        );

        let mut table2entry = loaded_tables.get_t2_mut(table1_idx).unwrap();

        if table2entry.state == FileState::Loaded
            || table2entry.state == FileState::Unloaded && !table2entry.data.is_null()
        {
            return;
        }

        println!("[ARC::Replace] Replacing {}...", internal_filepath);

        // This is a personal request, don't mind it too much.
        if let Some(_) = CONFIG.misc.mowjoh {
            skyline::error::show_error(
                69,
                &format!("[ARC::Replace] Replacing {}...", internal_filepath),
                "Nothing to see here",
            );
        }

        unsafe {
            nn::os::LockMutex(mutex);
        }

        let data = fs::read(path).unwrap().into_boxed_slice();
        let data = Box::leak(data);

        table2entry.data = data.as_ptr();
        table2entry.state = FileState::Loaded;
        table2entry.flags = 43;

        unsafe {
            nn::os::UnlockMutex(mutex);
        }

        println!("[ARC::Replace] Table2 entry status: {}", table2entry);
    }
}

#[hook(offset = IDK_OFFSET)]
unsafe fn idk(res_state: *const ResServiceState, table1_idx: u32, flag_related: u32) {
    original!()(res_state, table1_idx, flag_related);

    log!("--- [Idk] ---");
    handle_file_load(table1_idx);
}

#[hook(offset = ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET)]
unsafe fn add_idx_to_table1_and_table2(loaded_table: *const LoadedTables, table1_idx: u32) {
    original!()(loaded_table, table1_idx);

    log!("--- [AddIdx] ---");
    handle_file_load(table1_idx);
}

// This is a bit ew for now, I'll try fixing it eventually
#[hook(offset = 0x330615c, inline)]
fn parse_nutexb_footer(ctx: &InlineCtx) {
    unsafe {
        let table1_idx = *ctx.registers[25].w.as_ref();
        let loaded_tables = LoadedTables::get_instance();
        let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();
        let internal_filepath = hashes::get(hash).unwrap_or(&"Unknown");

        println!(
            "[ARC::Loading | #{}] File path: {}, Hash: {}, {}",
            table1_idx,
            internal_filepath,
            hash,
            loaded_tables
                .get_t1_mut(table1_idx)
                .unwrap()
                .get_t2_entry()
                .unwrap()
        );

        if let Some(path) = ARC_FILES.get_from_hash(hash) {
            println!(
                "[ARC::Replace] Hash matching for file path: {}",
                path.display()
            );

            println!("[ARC::Replace] Replacing {}...", internal_filepath);

            let file = fs::read(path).unwrap();
            let file_slice = file.as_slice();

            let data_slice = std::slice::from_raw_parts_mut(
                *ctx.registers[1].x.as_ref() as *mut u8,
                *ctx.registers[2].x.as_ref() as usize,
            );

            for (i, value) in data_slice.iter_mut().enumerate() {
                *value = file_slice[i];
            }
        }
    }
}

// Somewhat working, does not affect fighter textures. Only BC2 textures?
#[hook(offset = 0x3355d80)]
unsafe fn get_texture_by_table1_index(unk1: &u64, table1_idx: &u32) {
    log!("--- [GetTextureByPath?] ---");
    handle_file_load(*table1_idx);

    original!()(unk1, table1_idx);
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    // Read the configuration so we can set the filepaths
    lazy_static::initialize(&CONFIG);
    lazy_static::initialize(&ARC_FILES);
    lazy_static::initialize(&STREAM_FILES);

    // Load hashes from rom:/skyline/hashes.txt if the file is present
    hashes::init();
    // Look for the offset of the various functions to hook
    patching::search_offsets();
    // Not working so far, does not crash the game
    //patching::shared_redirection();
    // Patch filesizes in the Subfile table
    patching::filesize_replacement();
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
        parse_nutexb_footer
    );

    println!(
        "ARCropolis v{} - File replacement plugin is now installed",
        env!("CARGO_PKG_VERSION")
    );
}
