#![feature(proc_macro_hygiene)]
#![feature(str_strip)]

use std::fs;

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
use config::{CONFIG};

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        // Uncomment to enable logging
        println!($($arg)*);
    }
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
        println!(
            "[ARC::Replace] Hash matching for file path: {}",
            path.display()
        );

        let mut table2entry = loaded_tables.get_t2_mut(table1_idx).unwrap();

        if table2entry.state == FileState::Loaded {
            return;
        }

        println!("[ARC::Replace] Replacing {}...", internal_filepath);

        unsafe {
            nn::os::LockMutex(mutex);
        }

        let data = fs::read(path).unwrap().into_boxed_slice();
        let data = Box::leak(data);

        table2entry.data = data.as_ptr();
        table2entry.state = FileState::Loaded;
        table2entry.flags = 45;

        unsafe {
            nn::os::UnlockMutex(mutex);
        }

        log!("[ARC::Replace] Table2 entry status: {}", table2entry);
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

#[hook(offset = 0x34e42f0)]
fn parse_nutexb_footer(
    unk1: *const u64,
    data: *const u8,
    decompressed_size: u64,
    unk2: *mut u64,
) -> u64 {
    // Write 0xFF over half of the buffer to see changes

    // unsafe {
    //     let data_slice = std::slice::from_raw_parts_mut(data as *mut u8, decompressed_size as usize);

    //     for mut value in data_slice[0..((decompressed_size - 0xb0) / 2) as usize].iter_mut() {
    //         *value = 0xFF;
    //     }
    // }
    original!()(unk1, data, decompressed_size, unk2)
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
    // Patch filesizes in the Subfile table
    patching::filesize_replacement();
    // Attempt at expanding table2 (Does not work, do not use!)
    //patching::expand_table2();

    install_hooks!(
        idk,
        add_idx_to_table1_and_table2,
        stream::lookup_by_stream_hash
    );

    println!(
        "ARCropolis v{} - File replacement plugin is now installed",
        env!("CARGO_PKG_VERSION")
    );
}
