#![feature(proc_macro_hygiene)]

use skyline::{nn, libc::{c_void, c_char}, logging::hex_dump_ptr, hook, install_hooks};
use std::{ptr, fs, io, path::Path, collections::HashMap};
use std::io::{Error, ErrorKind};
use rand::Rng;

mod replacement_files;
use replacement_files::*;

mod hashes;
mod resource;
use resource::*;

static mut IDK_OFFSET: usize = 0x32545a0; // default = 8.0.0 offset
static mut ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET: usize = 0x324e9f0; // default = 8.0.0 offset
static mut LOOKUP_STREAM_HASH_OFFSET: usize = 0x324f7a0; // default = 8.0.0 offset

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


#[skyline::main(name = "replace")]
pub fn main() {
    lazy_static::initialize(&ARC_FILES);
    lazy_static::initialize(&STREAM_FILES);
    hashes::init();

    install_hooks!(idk, add_idx_to_table1_and_table2, lookup_by_stream_hash);
    log!("File replacement mod installed.");
}