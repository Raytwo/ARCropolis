use skyline::libc::c_char;
use smash_arc::{Hash40, LoadedArc, ArcLookup};
use crate::{offsets, hashes};

#[skyline::hook(offset = offsets::lookup_stream_hash())]
fn lookup_stream_hash(
    out_path: *mut c_char,
    loaded_arc: &LoadedArc,
    size_out: &mut usize,
    offset_out: &mut u64,
    hash: Hash40
) {
    let fs = crate::GLOBAL_FILESYSTEM.read();
    if let Some(local_path) = fs.local_hash(hash) {
        // restrictions by the stream API require us to be able to load this file via std::fs
        // therefore, it is fair to use the StandardLoader to query both its existence and the filesize
        if let Some(path) = fs.hash(hash) {
            // at this point if it is a patch file this should pass, if it's a callback file
            // this should fail
            // if it is a callback file, it has to return a valid path that the system can read so we can just
            // stat it
            if let Some(size) = fs.get().query_max_filesize(local_path) {
                *size_out = size;
                *offset_out = 0;
                let cpath = format!("{}\0", path.display());
                let out_buffer = unsafe {
                    std::slice::from_raw_parts_mut(out_path, cpath.len())
                };
                out_buffer.copy_from_slice(cpath.as_bytes());
                return;
            } else if path.exists() {
                match std::fs::metadata(&path).map(|x| x.len()) {
                    Ok(size) => {
                        *size_out = size as usize;
                        *offset_out = 0;
                        let cpath = format!("{}\0", path.display());
                        let out_buffer = unsafe {
                            std::slice::from_raw_parts_mut(out_path, cpath.len())
                        };
                        out_buffer.copy_from_slice(cpath.as_bytes());
                        return;
                    },
                    _ => {}
                }
            }
        }
    }
	
	// query information from the arc via a smash_arc lookup instead of calling the original function
	match loaded_arc.get_stream_data(hash) {
		Ok(stream_data) => {
			*size_out = stream_data.size as usize;
			*offset_out = stream_data.offset;
			let cpath = "rom:/data.arc"; // the game normally populates the out_path with this string in the original function, so we do the same here
			let out_buffer = unsafe {
				std::slice::from_raw_parts_mut(out_path, cpath.len())
			};
			out_buffer.copy_from_slice(cpath.as_bytes());
			return;
		}
		_ => {
			error!("Could not find StreamData for '{}' ({:#x}) in data.arc.", hashes::find(hash), hash.0);
		}
	}
}

pub fn install() {
    skyline::install_hooks!(
        lookup_stream_hash
    );
}
