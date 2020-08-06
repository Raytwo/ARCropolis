#![feature(proc_macro_hygiene)]
#![feature(str_strip)]

use skyline::hooks::{getRegionAddress, Region};
use skyline::{nn, libc::{c_void, c_char}, logging::hex_dump_ptr, hook, install_hooks};
use std::{ptr, fs, io, path::Path, collections::HashMap};
use std::io::{Error, ErrorKind};
use rand::Rng;

mod replacement_files;
use replacement_files::*;

mod hashes;
mod resource;
use resource::*;

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

static LOADED_TABLES_ADRP_SEARCH_CODE: &[u8] = &[
    0x28, 0x4b, 0x40, 0xb9,
    0xf4, 0x03, 0x01, 0x2a,
    0x1f, 0x01, 0x01, 0x6b,
    0x29, 0x0a, 0x00, 0x54,
    0x36, 0x03, 0x40, 0xf9,
    0xe0, 0x03, 0x16, 0xaa,
];

static LOOKUP_STREAM_HASH_SEARCH_CODE: &[u8] = &[
    0x29, 0x58, 0x40, 0xf9,
    0x28, 0x60, 0x40, 0xf9,
    0x2a, 0x05, 0x40, 0xb9,
    0x09, 0x0d, 0x0a, 0x8b,
    0xaa, 0x01, 0x00, 0x34,
    0x5f, 0x01, 0x00, 0xf1,
];

// default 8.0.0 offsets
static mut IDK_OFFSET: usize = 0x32545a0;
static mut ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET: usize = 0x324e9f0;
static mut LOOKUP_STREAM_HASH_OFFSET: usize = 0x324f7a0;
static mut LOADED_TABLES_ADRP_OFFSET: usize = 0x324c3a0;

macro_rules! log {
    ($($arg:tt)*) => {
        // Uncomment to enable logging
        //println!($($arg)*);
    }
}

fn offset_from_adrp(adrp_offset: usize) -> usize {
    unsafe {
        let adrp = *(resource::offset_to_addr(adrp_offset) as *const u32);
        let immhi = (adrp & 0b0_00_00000_1111111111111111111_00000) >> 3;
        let immlo = (adrp & 0b0_11_00000_0000000000000000000_00000) >> 29;
        let imm = ((immhi | immlo) << 12) as i32 as usize;
        let base = adrp_offset & 0xFFFFFFFFFFFFF000;
        base + imm
    }
}

fn offset_from_ldr(ldr_offset: usize) -> usize {
    unsafe {
        let ldr = *(resource::offset_to_addr(ldr_offset) as *const u32);
        let size = (ldr & 0b11_000_0_00_00_000000000000_00000_00000) >> 30;
        let imm = (ldr & 0b00_000_0_00_00_111111111111_00000_00000) >> 10;
        (imm as usize) << size
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

    // if !hashes::get(hash.as_u64()).unwrap_or(&"none").ends_with("nutexb") {
    //     return;
    // }

    log!(
        "Filename: {}, State: {}, Flags: {}, RefCount: {:x?}, Data loaded: {}",
        hashes::get(hash.as_u64()).unwrap_or(&"none"),
        table2entry.state,
        table2entry.flags,
        table2entry.ref_count,
        !table2entry.data.is_null()
    );
}

#[hook(offset = IDK_OFFSET)]
unsafe fn idk(res_state: *const u64, table1_idx: u32, flag_related: u32) {
    original!()(res_state, table1_idx, flag_related);

    let loaded_tables = LoadedTables::get_instance();
    let mutex = loaded_tables.mutex;
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();

    if let Some(path) = ARC_FILES.get_from_hash(hash) {
        log!("--- [Idk] ---");
        log!("File hash matching, path: {}", path.display());

        let mut table2entry = loaded_tables.get_t2_mut(table1_idx).unwrap();

        if table2entry.state == FileState::Loaded {
            return;
        }

        log!("Replacing...");

        nn::os::LockMutex(mutex);

        let data = fs::read(path).unwrap().into_boxed_slice();
        let data = Box::leak(data);
        table2entry.data = data.as_ptr();
        table2entry.state = FileState::Loaded;
        table2entry.flags = 45;

        nn::os::UnlockMutex(mutex);

        print_table1idx_info(table1_idx);
    }
}

#[hook(offset = ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET)]
unsafe fn add_idx_to_table1_and_table2(loaded_table: *const LoadedTables, table1_idx: u32) {
    original!()(loaded_table, table1_idx);

    let loaded_tables = LoadedTables::get_instance();
    let mutex = loaded_tables.mutex;
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();

    if let Some(path) = ARC_FILES.get_from_hash(hash) {
        log!("--- [AddIdx] ---");
        log!("File hash matching, path: {}", path.display());

        let mut table2entry = loaded_tables.get_t2_mut(table1_idx).unwrap();

        if table2entry.state == FileState::Loaded || table2entry.state == FileState::Unloaded && !table2entry.data.is_null() {
            return;
        }

        log!("Replacing...");

        nn::os::LockMutex(mutex);

        let data = fs::read(path).unwrap().into_boxed_slice();
        let data = Box::leak(data);
        table2entry.data = data.as_ptr();
        table2entry.state = FileState::Loaded;
        table2entry.flags = 43;

        nn::os::UnlockMutex(mutex);

        print_table1idx_info(table1_idx);
    }
}


pub fn random_media_select(directory: &str) -> io::Result<String>{
    let mut rng = rand::thread_rng();

    let mut media_files = HashMap::new();

    let mut media_count = 0;

    for entry in fs::read_dir(Path::new(directory))? {
        let entry = entry?;
        let filename = entry.path();
        let real_path = format!("{}/{}", directory, filename.display());
        if !Path::new(&real_path).is_dir() {
            media_files.insert(media_count, real_path);
            media_count += 1;
        }
    }

    if media_count <= 0 {
        return Err(Error::new(ErrorKind::Other, "No Files Found!"))
    }

    let random_result = rng.gen_range(0, media_count);

    Ok(media_files.get(&random_result).unwrap().to_string())
}

// (char *out_path,void *loadedArc,undefined8 *size_out,undefined8 *offset_out, ulonglong hash)
#[hook(offset = LOOKUP_STREAM_HASH_OFFSET)]
fn lookup_by_stream_hash(
    out_path: *mut c_char, loaded_arc: *const c_void, size_out: *mut u64, offset_out: *mut u64, hash: u64
) {
    if let Some(path) = STREAM_FILES.0.get(&hash) {
        let file;
        let metadata;
        let size;
        let random_selection;

        let directory = path.display().to_string();

        if  Path::new(&directory).is_dir() {

            match random_media_select(&directory){
                Ok(pass) => random_selection = pass,
                Err(_err) => {
                    log!("{}", _err);
                    original!()(out_path, loaded_arc, size_out, offset_out, hash);
                    return;
                }
            };

            file = fs::File::open(&random_selection).unwrap();
            metadata = file.metadata().unwrap();
            size = metadata.len() as u64;

        } else{
            random_selection = path.to_str().expect("Paths must be valid unicode").to_string();
            file = fs::File::open(&random_selection).unwrap();
            metadata = file.metadata().unwrap();
            size = metadata.len() as u64;
        }

        unsafe {
            *size_out = size;
            *offset_out = 0;
            let string = random_selection;
            log!("Loading '{}'...", string);
            let bytes = string.as_bytes();
            ptr::copy_nonoverlapping(
                bytes.as_ptr(), out_path, bytes.len()
            );
            *out_path.offset(bytes.len() as _) = 0u8;
        }
        hex_dump_ptr(out_path);
    } else {
        original!()(out_path, loaded_arc, size_out, offset_out, hash);
    }
}


#[skyline::main(name = "arcropolis")]
pub fn main() {
    lazy_static::initialize(&ARC_FILES);
    lazy_static::initialize(&STREAM_FILES);
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

        if let Some(offset) = find_subsequence(text, LOOKUP_STREAM_HASH_SEARCH_CODE) {
            LOOKUP_STREAM_HASH_OFFSET = offset
        } else {
            println!("Error: no offset found for function 'add_idx_to_table1_and_table2'. Defaulting to 8.0.0 offset. This likely won't work.");
        }

        if let Some(offset) = find_subsequence(text, LOADED_TABLES_ADRP_SEARCH_CODE) {
            LOADED_TABLES_ADRP_OFFSET = offset - 8
        } else {
            println!("Error: no offset found for 'loaded_tables_adrp'. Defaulting to 8.0.0 offset. This likely won't work.");
        }
        let adrp_offset = offset_from_adrp(LOADED_TABLES_ADRP_OFFSET);
        let ldr_offset = offset_from_ldr(LOADED_TABLES_ADRP_OFFSET + 4);
        resource::LOADED_TABLES_OFFSET = adrp_offset + ldr_offset;
    }

    install_hooks!(idk, add_idx_to_table1_and_table2, lookup_by_stream_hash);
    log!("File replacement mod installed.");
}