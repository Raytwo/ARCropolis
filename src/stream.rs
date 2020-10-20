use rand::Rng;

use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::{fs, io, ptr};

use skyline::hook;
use skyline::libc::{c_char, c_void};

use crate::get_from_hash;
use crate::offsets::LOOKUP_STREAM_HASH_OFFSET;

use log::{ info, warn };

pub fn random_media_select(directory: &str) -> io::Result<String> {
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
        return Err(Error::new(ErrorKind::Other, "No Files Found!"));
    }

    let random_result = rng.gen_range(0..media_count);

    Ok(media_files.get(&random_result).unwrap().to_string())
}

// (char *out_path,void *loadedArc,undefined8 *size_out,undefined8 *offset_out, ulonglong hash)
#[hook(offset = LOOKUP_STREAM_HASH_OFFSET)]
fn lookup_by_stream_hash(
    out_path: *mut c_char,
    loaded_arc: *const c_void,
    size_out: *mut u64,
    offset_out: *mut u64,
    hash: u64,
) {
    if let Ok(file_ctx) = get_from_hash!(hash) {
        let file;
        let metadata;
        let size;
        let random_selection;

        let directory = file_ctx.path.display().to_string();

        if Path::new(&directory).is_dir() {
            match random_media_select(&directory) {
                Ok(pass) => random_selection = pass,
                Err(_err) => {
                    warn!("{}", _err);
                    original!()(out_path, loaded_arc, size_out, offset_out, hash);
                    return;
                }
            };

            file = fs::File::open(&random_selection).unwrap();
            metadata = file.metadata().unwrap();
            size = metadata.len() as u64;
        } else {
            random_selection = file_ctx
                .path
                .to_str()
                .expect("Paths must be valid unicode")
                .to_string();
            file = fs::File::open(&random_selection).unwrap();
            metadata = file.metadata().unwrap();
            size = metadata.len() as u64;
        }

        unsafe {
            *size_out = size;
            *offset_out = 0;
            let string = random_selection;
            info!("Loading '{}'...", string);
            let bytes = string.as_bytes();
            ptr::copy_nonoverlapping(bytes.as_ptr(), out_path, bytes.len());
            *out_path.offset(bytes.len() as _) = 0u8;
        }
    } else {
        original!()(out_path, loaded_arc, size_out, offset_out, hash);
    }
}
