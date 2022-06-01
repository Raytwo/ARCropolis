pub mod extensions;
pub mod lookup;

pub mod addition;
pub mod config;
pub mod preprocess;
mod stream;
mod threads;
mod uncompressed;
pub mod unshare;

pub use extensions::*;
use owo_colors::OwoColorize;
use smash_arc::{Hash40, ArcLookup};

pub fn patch_sizes(data: &[(Hash40, u64)]) {
    let arc = crate::resource::arc_mut();
    let region = crate::config::region();

    for (hash, size) in data {

        let decomp_size = match arc.get_file_data_from_hash(*hash, region) {
            Ok(data) => {
                //println!("Patched {:#x} with size {:#x}", hash.as_u64(), size);
                data.decomp_size as usize
            },
            Err(_) => {
                warn!("Failed to patch {:#x} filesize! It should be {:#x}.", hash.as_u64(), size.green());
                continue;
            },
        };

        if *size as usize > decomp_size {
            if let Ok(old_size) = arc.patch_filedata(*hash, *size as u32, region) {
                info!("File {:#x} has a new decompressed filesize! {:#x} -> {:#x}", hash.as_u64(), old_size.red(), size.green());
            }
        }
    }
}

pub fn install() {
    stream::install();
    threads::install();
    uncompressed::install();
}
