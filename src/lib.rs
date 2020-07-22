#![feature(proc_macro_hygiene)]

use std::fs;
use skyline::{hook, install_hooks};
use skyline::hooks::{getRegionAddress, Region};

mod replacement_files;
use replacement_files::ARC_FILES;

mod hashes;
mod resource;
use resource::*;

static mut IDK_OFFSET: usize = 0x32545a0;
static mut ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET: usize = 0x324e9f0;

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

static IDK_SEARCH_CODE: &[u8] = &[
    0xf8, 0x5f, 0xbc, 0xa9,
    0xf6, 0x57, 0x01, 0xa9,
    0xf4, 0x4f, 0x02, 0xa9,
    0xfd, 0x7b, 0x03, 0xa9,
    0xfd, 0xc3, 0x00, 0x91,
    0xe8, 0x5f, 0x00, 0x32,
    0x3f, 0x00, 0x08, 0x6b,
];

static ADD_IDX_TO_TABLE1_AND_TABLE2_SEARCH_CODE: &[u8] = &[
    0xf6, 0x57, 0xbd, 0xa9,
    0xf4, 0x4f, 0x01, 0xa9,
    0xfd, 0x7b, 0x02, 0xa9,
    0xfd, 0x83, 0x00, 0x91,
    0x08, 0x18, 0x40, 0xb9,
    0x1f, 0x01, 0x01, 0x6b,
];

macro_rules! log {
    ($($arg:tt)*) => {
        // Uncomment to enable logging
        //println!($($arg)*);
    }
}

#[allow(unused_variables)]
fn print_table1idx_info(table1_idx: u32) {
    let loaded_tables = LoadedTables::get_instance();
    let table1entry = match loaded_tables.table_1().get(table1_idx as usize) {
        Some(entry) => entry,
        None => {
            log!("Could not fetch table1entry.");
            return;
        }
    };

    let table2entry = match loaded_tables.table_2().get(table1entry.table2_index as usize) {
            Some(entry) => entry,
            None => {
                log!("Could not fetch the table2entry.");
                return;
            }
        };

    let hash = loaded_tables.get_hash_from_t1_index(table1_idx);

    log!(
        "Filename: {}, State: {}, Flags: {}, RefCount: {:x?}, Data loaded: {}",
        hashes::get(hash.as_u64()).unwrap_or(&"none"),
        table2entry.state,
        table2entry.flags,
        table2entry.ref_count,
        !table2entry.data.is_null()
    );
}

#[hook(offset = 0x324eb00)]
fn dec_ref_count(loaded_tables: *const LoadedTables, path_index: u32)
{
    log!("--- [Dec ref count] ---");
    original!()(loaded_tables, path_index);
    print_table1idx_info(path_index);
}

#[hook(offset = IDK_OFFSET)]
fn idk(res_state: *const u64, table1_idx: u32, flag_related: u32) {
    let loaded_tables = LoadedTables::get_instance();
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();

    original!()(res_state, table1_idx, flag_related);

    if let Some(path) = ARC_FILES.get_from_hash(hash) {
        log!("File hash matching, path: {}", path.display());
        let mut table2entry = loaded_tables.get_t2_mut(table1_idx).unwrap();

        if table2entry.state == FileState::Loaded {
            // Return if already loaded
            return;
        }

        log!("Replacing...");

        let data = fs::read(path).unwrap().into_boxed_slice();
        let data = Box::leak(data);
        table2entry.data = data.as_ptr();
        table2entry.state = FileState::Loaded;
        table2entry.flags = 45;

        print_table1idx_info(table1_idx);
    }
}

#[hook(offset = ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET)]
fn add_idx_to_table1_and_table2(loaded_table: *const LoadedTables, table1_idx: u32) {
    original!()(loaded_table, table1_idx);


    let loaded_tables = LoadedTables::get_instance();
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();

    if let Some(path) = ARC_FILES.get_from_hash(hash) {
        log!("File hash matching, path: {}", path.display());
        let mut table2entry = loaded_tables.get_t2_mut(table1_idx).unwrap();

        if table2entry.state == FileState::Loaded {
            return;
        }

        log!("Replacing...");

        let data = fs::read(path).unwrap().into_boxed_slice();
        let data = Box::leak(data);
        table2entry.data = data.as_ptr();
        table2entry.state = FileState::Loaded;
        table2entry.flags = 0x43;
    }
}

#[skyline::main(name = "replace")]
pub fn main() {
    lazy_static::initialize(&ARC_FILES);
    hashes::init();
    
    unsafe {
        let text_ptr = getRegionAddress(Region::Text) as *const u8;
        let text_size = (getRegionAddress(Region::Rodata) as usize) - (text_ptr as usize);
        let text = std::slice::from_raw_parts(text_ptr, text_size);
        if let Some(offset) = find_subsequence(text, IDK_SEARCH_CODE) {
            IDK_OFFSET = offset
        } else {
            println!("Error: no offset found for function 'idk'. Defaulting to 8.0.0 offset. This likely won't work.");
        }
        
        if let Some(offset) = find_subsequence(text, ADD_IDX_TO_TABLE1_AND_TABLE2_SEARCH_CODE) {
            ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET = offset
        } else {
            println!("Error: no offset found for function 'add_idx_to_table1_and_table2'. Defaulting to 8.0.0 offset. This likely won't work.");
        }
    }

    install_hooks!(idk, add_idx_to_table1_and_table2);
    log!("File replacement mod installed.");
}
