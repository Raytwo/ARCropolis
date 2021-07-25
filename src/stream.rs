use std::path::Path;
use std::{
    fs, io,
    io::{Error, ErrorKind},
    ptr,
};

use skyline::hook;
use skyline::libc::c_char;

use log::info;
use rand::seq::SliceRandom;

use crate::{callbacks::CallbackKind, replacement_files::MOD_FILES};
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

#[hook(offset = LOOKUP_STREAM_HASH_OFFSET)]
fn lookup_by_stream_hash(
    out_path: *mut c_char,
    loaded_arc: &LoadedArc,
    size_out: &mut usize,
    offset_out: *mut u64,
    hash: Hash40,
) {
    // If we have a FileCtx for this stream, use it
    if let Some(file_ctx) = MOD_FILES.read().0.get(&FileIndex::Stream(hash)) {
        match &file_ctx.file {
            // Goes without saying
            crate::replacement_files::FileBacking::LoadFromArc => {
                original!()(out_path, loaded_arc, size_out, offset_out, hash)
            }
            // Load the file from the SD
            crate::replacement_files::FileBacking::ModFile(modfile) => {
                let path = modfile.full_path();

                // Daily reminder that Raytwo did not write this so please don't blame him for it looking bad.
                // blujay here, definitely going to blame Ray for the code looking like this
                unsafe {
                    *size_out = path.metadata().unwrap().len() as usize;
                    *offset_out = 0;
                    let string = path.to_str().unwrap().to_string();
                    info!("Loading '{}'...", string);
                    let bytes = string.as_bytes();
                    ptr::copy_nonoverlapping(bytes.as_ptr(), out_path, bytes.len());
                    *out_path.add(bytes.len()) = 0u8;
                }
            }
            // Load the file from a StreamCallback
            crate::replacement_files::FileBacking::Callback { callback, original: _ } => {
                // If we have a StreamCallback, call it
                if let CallbackKind::Stream(cb) = callback {
                    let callback_fn = cb.callback_fn;

                    if !callback_fn(hash.as_u64(), out_path, size_out) {
                        // If the callback returned false, load the original file
                        original!()(out_path, loaded_arc, size_out, offset_out, hash);
                    }
                } else {
                    // If we don't (very strange), load the original file
                    original!()(out_path, loaded_arc, size_out, offset_out, hash);
                    return;
                }
            }
        }
    } else {
        // If we don't have a FileCtx, proceed as usual
        original!()(out_path, loaded_arc, size_out, offset_out, hash);
    }
}
