use rayon::iter::ParallelBridge;
use rayon::prelude::ParallelIterator;
use rayon::prelude::*;

use std::fs::DirEntry;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::{collections::HashMap, fs, io, slice};

use smash::hash40;
use smash::resource::{LoadedTables, SubFile};

use crate::config::CONFIG;

lazy_static::lazy_static! {
    pub static ref ARC_FILES: ArcFiles = {
    let instance = ArcFiles::new();
    instance
    };
}

pub struct ArcFiles(pub HashMap<u64, FileCtx>);

pub struct FileCtx {
    pub path: PathBuf,
    pub filesize: u32,
    pub orig_subfile: SubFile,
}

impl ArcFiles {
    fn new() -> Self {
        // let mut instance = Self(Mutex<HashMap::new()>);

        // let _ = visit_dir_rewrite(Path::new(&CONFIG.paths.arc), CONFIG.paths.arc.len());
        // let _ = instance.visit_umm_dirs(Path::new(&CONFIG.paths.umm));

        // instance
        ArcFiles(HashMap::new())
    }

    /// Visit Ultimate Mod Manager directories for backwards compatibility
    fn visit_umm_dirs(&mut self, dir: &Path) -> io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                if entry
                    .path()
                    .file_name()
                    .map(|s| s.to_str().map(|s| s.starts_with(".")))
                    .flatten()
                    .unwrap_or(false)
                {
                    continue;
                }
                let filename = entry.path();
                let real_path = format!("{}/{}", dir.display(), filename.display());
                let path = Path::new(&real_path);

                if path.is_dir() {
                    self.visit_dir(&path, real_path.len())?;
                }
            }
        }

        Ok(())
    }

    fn visit_dir_rewrite(&self, dir: &Path, arc_dir_len: usize) {
        let files = fs::read_dir(dir)
            .unwrap()
            .map(|x| {
                let entry = x.unwrap();
                let path = PathBuf::from(&format!("{}/{}", dir.display(), entry.path().display()));

                if entry.file_type().unwrap().is_dir() {
                    if let Some(_) = path.extension() {
                        let (hash, context) = self.visit_file(&entry, arc_dir_len);
                        return;
                    }

                    self.visit_dir_rewrite(&path, arc_dir_len);
                } else {
                    let (hash, context) = self.visit_file(&entry, arc_dir_len);
                }
            })
            .collect::<Vec<_>>();
    }

    fn visit_file(&self, entry: &DirEntry, arc_dir_len: usize) -> (u64, FileCtx) {
        match entry.path().extension() {
            Some(_) => {}
            None => {
                println!(
                    "Error getting file extension for: {}",
                    entry.path().display()
                );
            }
        }

        // This is the path that gets hashed. Replace ; to : for Smash's internal paths since ; is not a valid character for filepaths.
        let mut game_path = entry.path().to_str().unwrap()[arc_dir_len + 1..].replace(";", ":");

        // TODO: Move that stuff in a separate function that can handle more than one format
        match game_path.strip_suffix("mp4") {
            Some(x) => game_path = format!("{}{}", x, "webm"),
            None => (),
        }

        let mut file_ctx = FileCtx {
            path: entry.path(),
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
        (hash, file_ctx)
    }

    fn visit_dir(&mut self, dir: &Path, arc_dir_len: usize) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let filename = entry.path();
            let real_path = format!("{}/{}", dir.display(), filename.display());
            let path = PathBuf::from(&real_path);

            let mut file_ctx = FileCtx {
                path: path.clone(),
                filesize: 0,
                orig_subfile: SubFile {
                    offset: 0,
                    compressed_size: 0,
                    decompressed_size: 0,
                    flags: 0,
                },
            };

            if path.is_dir()
                && path
                    .file_name()
                    .unwrap()
                    .to_os_string()
                    .into_string()
                    .unwrap()
                    .contains(".")
            {
                match path.extension().and_then(std::ffi::OsStr::to_str) {
                    Some(_) => {}
                    None => {
                        println!("Error getting file extension for: {}", path.display());
                    }
                }

                let mut game_path = path.display().to_string()[arc_dir_len + 1..].replace(";", ":");

                match game_path.strip_suffix("mp4") {
                    Some(x) => game_path = format!("{}{}", x, "webm"),
                    None => (),
                }

                let hash = hash40(&game_path);
                let metadata = match entry.metadata() {
                    Ok(meta) => meta,
                    Err(err) => panic!(err),
                };

                file_ctx.filesize = metadata.len() as _;

                self.filesize_replacement(hash, &mut file_ctx);
                self.0.insert(hash, file_ctx);
            } else if path.is_dir() {
                self.visit_dir(&path, arc_dir_len).unwrap();
            } else {
                match path.extension().and_then(std::ffi::OsStr::to_str) {
                    Some(_) => {}
                    None => {
                        println!("Error getting file extension for: {}", path.display());
                    }
                }

                let mut game_path = path.display().to_string()[arc_dir_len + 1..].replace(";", ":");

                match game_path.strip_suffix("mp4") {
                    Some(x) => game_path = format!("{}{}", x, "webm"),
                    None => (),
                }

                let hash = hash40(&game_path);
                let metadata = match entry.metadata() {
                    Ok(meta) => meta,
                    Err(err) => panic!(err),
                };

                file_ctx.filesize = metadata.len() as _;

                self.filesize_replacement(hash, &mut file_ctx);
                self.0.insert(hash, file_ctx);
            }
        }

        Ok(())
    }

    pub fn filesize_replacement(&self, hash: u64, file_ctx: &mut FileCtx) {
        let loaded_tables = LoadedTables::get_instance();

        unsafe {
            let extension = file_ctx.path.extension().unwrap().to_str().unwrap();
            // Some formats don't appreciate me messing with their size
            match extension {
                "bntx" | "nutexb" | "eff" | "numshexb" | "arc" | "prc" => {}
                &_ => return,
            }

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

    pub fn get_from_hash(&self, hash: u64) -> Option<&FileCtx> {
        self.0.get(&hash)
    }
}
