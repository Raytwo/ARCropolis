use std::path::Path;
use std::{
    fs, io,
    io::{Error, ErrorKind},
    ptr,
};

use skyline::hook;
use skyline::libc::c_char;

use log::{info, warn};
use rand::seq::SliceRandom;

use crate::replacement_files::MOD_FILES;
use crate::{offsets::LOOKUP_STREAM_HASH_OFFSET, replacement_files::FileIndex};

use smash_arc::{Hash40, LoadedArc};

pub fn random_media_select(directory: &str) -> io::Result<String> {
    let mut rng = rand::thread_rng();

    let media_files: Vec<_> = fs::read_dir(Path::new(directory))?
        .filter_map(|entry| {
            let entry = entry.unwrap();
            let filename = entry.path();
            let real_path = format!("{}/{}", directory, filename.display());

            if !Path::new(&real_path).is_dir() {
                Some(real_path)
            } else {
                None
            }
        })
        .collect();

    if media_files.is_empty() {
        return Err(Error::new(ErrorKind::Other, "No Files Found!"));
    }

    Ok(media_files.choose(&mut rng).unwrap().to_string())
}

// (char *out_path,void *loadedArc,undefined8 *size_out,undefined8 *offset_out, ulonglong hash)
#[hook(offset = LOOKUP_STREAM_HASH_OFFSET)]
fn lookup_by_stream_hash(
    out_path: *mut c_char,
    loaded_arc: &LoadedArc,
    size_out: *mut u64,
    offset_out: *mut u64,
    hash: Hash40,
) {
    if let Some(file_ctx) = MOD_FILES.read().0.get(&FileIndex::Stream(hash)) {
        let file;
        let metadata;
        let size;
        let random_selection;

        let directory = file_ctx.file.path().display().to_string();

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
                .file
                .path()
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
            *out_path.add(bytes.len()) = 0u8;
        }
    } else {
        original!()(out_path, loaded_arc, size_out, offset_out, hash);
    }
}
