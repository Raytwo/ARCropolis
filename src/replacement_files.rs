use std::fs::DirEntry;
use std::path::PathBuf;
use std::sync::RwLock;
use std::{collections::HashMap, fs, io, slice};

use rayon::iter::ParallelBridge;
use rayon::prelude::ParallelIterator;

use smash::hash40;
use smash::resource::{LoadedTables, SubFile};

use crate::config::CONFIG;

lazy_static::lazy_static! {
    pub static ref ARC_FILES: ArcFiles = ArcFiles::new();
}

pub struct ArcFiles(pub RwLock<HashMap<u64, FileCtx>>);

pub struct FileCtx {
    pub path: PathBuf,
    pub filesize: u32,
    pub orig_subfile: SubFile,
}

#[macro_export]
macro_rules! get_from_hash {
    ($hash:expr) => {
        $crate::replacement_files::ARC_FILES.0.read().unwrap().get(&($hash))
    };
}

impl ArcFiles {
    fn new() -> Self {
        let mut instance = Self(RwLock::new(HashMap::new()));

        let _ = instance.visit_dir(&PathBuf::from(&CONFIG.paths.arc), CONFIG.paths.arc.len());
        let _ = instance.visit_umm_dirs(&PathBuf::from(&CONFIG.paths.umm));

        instance
    }

    /// Visit Ultimate Mod Manager directories for backwards compatibility
    fn visit_umm_dirs(&mut self, dir: &PathBuf) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;

            if entry.file_name().to_str().unwrap().starts_with(".") {
                continue
            }

            let path = PathBuf::from(&format!("{}/{}", dir.display(), entry.path().display()));

            if path.is_dir() {
                self.visit_dir(&path, path.to_str().unwrap().len())?;
            }
        }

        Ok(())
    }

    fn visit_dir(&self, dir: &PathBuf, arc_dir_len: usize) -> io::Result<()> {
        fs::read_dir(dir)?
            .par_bridge()
            .map(|entry| {
                let entry = entry?;
                let path = PathBuf::from(&format!("{}/{}", dir.display(), entry.path().display()));

                println!("{}", path.display());
                // Check if the entry is a directory or a file
                if entry.file_type().unwrap().is_dir() {
                    // If it is one of the stream randomizer directories
                    if let Some(_) = path.extension() {
                        match self.visit_file(&entry, &path, arc_dir_len) {
                            Ok((hash, file_ctx)) => {
                                self.0.write().unwrap().insert(hash, file_ctx);
                                return Ok(());
                            },
                            Err(err) => {
                                println!("{}", err);
                                return Ok(())
                            }
                        }
                    }

                    // If not, treat it as a regular directory
                    self.visit_dir(&path, arc_dir_len).unwrap();
                } else {
                    match self.visit_file(&entry, &path, arc_dir_len) {
                        Ok((hash, file_ctx)) => {
                            self.0.write().unwrap().insert(hash, file_ctx);
                            return Ok(());
                        },
                        Err(err) => {
                            println!("{}", err);
                            return Ok(())
                        }
                    }
                }

                Ok(())
            })
            .collect()
    }

    fn visit_file(&self, entry: &DirEntry, full_path: &PathBuf, arc_dir_len: usize) -> Result<(u64, FileCtx), String> {
        match full_path.extension() {
            Some(_) => {}
            None => return Err(format!("Error getting file extension for: {}", full_path.display())),
        }

        // This is the path that gets hashed. Replace ; to : for Smash's internal paths since ; is not a valid character for filepaths.
        let mut game_path = full_path.to_str().unwrap()[arc_dir_len + 1..].replace(";", ":");

        // TODO: Move that stuff in a separate function that can handle more than one format
        match game_path.strip_suffix("mp4") {
            Some(x) => game_path = format!("{}{}", x, "webm"),
            None => (),
        }

        let mut file_ctx = FileCtx {
            path: full_path.to_path_buf(),
            filesize: 0,
            orig_subfile: SubFile {
                offset: 0,
                compressed_size: 0,
                decompressed_size: 0,
                flags: 0,
            },
        };

        let hash = hash40(&game_path);

        let metadata = match entry.metadata() {
            Ok(meta) => meta,
            Err(err) => panic!(err),
        };

        file_ctx.filesize = metadata.len() as _;

        // TODO: Move this method in a impl for FileCtx
        self.filesize_replacement(hash, &mut file_ctx);
        Ok((hash, file_ctx))
    }

    pub fn filesize_replacement(&self, hash: u64, file_ctx: &mut FileCtx) {
        let loaded_tables = LoadedTables::get_instance();

        let extension = match file_ctx.path.extension() {
            Some(ext) => ext.to_str().unwrap(),
            None => {
                println!("File {} does not have an extension, skipping", file_ctx.path.display());
                return;
            },
        };

        // Some formats don't appreciate me messing with their filesize
        match extension {
            "bntx" | "nutexb" | "eff" | "numshexb" | "arc" | "prc" => {}
            &_ => return,
        }

        unsafe {
            let hashindexgroup_slice = slice::from_raw_parts(
                loaded_tables.get_arc().file_info_path,
                (*loaded_tables).table1_len as usize,
            );

            let t1_index = match hashindexgroup_slice
                .iter()
                .position(|x| x.path.hash40.as_u64() == hash)
            {
                Some(index) => index as u32,
                None => {
                    println!(
                        "[ARC::Patching] Hash for file {} not found in table1, skipping",
                        file_ctx.path.display()
                    );
                    return;
                }
            };

            let mut subfile = loaded_tables.get_arc().get_subfile_by_t1_index(t1_index);

            // Gotta make SubFile derive Clone and Copy 'cause this is massive ass
            file_ctx.orig_subfile.offset = (*subfile).offset;
            file_ctx.orig_subfile.compressed_size = (*subfile).compressed_size;
            file_ctx.orig_subfile.decompressed_size = (*subfile).decompressed_size;
            file_ctx.orig_subfile.flags = (*subfile).flags;

            if (subfile.decompressed_size < file_ctx.filesize) && extension == "nutexb" {
                // Is compressed?
                if (subfile.flags & 0x3) == 3 {
                    subfile.decompressed_size = file_ctx.filesize;

                    println!(
                        "[ARC::Patching] New decompressed size for {}: {:#x}",
                        file_ctx.path.display(),
                        subfile.decompressed_size
                    );
                }
            } else {
                if subfile.decompressed_size < file_ctx.filesize {
                    subfile.decompressed_size = file_ctx.filesize;
                    println!(
                        "[ARC::Patching] New decompressed size for {}: {:#x}",
                        file_ctx.path.display(),
                        subfile.decompressed_size
                    );
                }
            }
        }
    }
}
