use std::{ fs, io, slice};
use std::fs::DirEntry;
use std::sync::RwLock;
use std::path::PathBuf;
use std::collections::HashMap;

use rayon::iter::{ ParallelIterator, ParallelBridge, IntoParallelRefIterator, IndexedParallelIterator };

use smash::hash40;
use smash::resource::{ResServiceState, LoadedTables, SubFile};

use crate::config::CONFIG;

lazy_static::lazy_static! {
    pub static ref ARC_FILES: ArcFiles = ArcFiles::new();
}

pub struct ArcFiles(pub RwLock<HashMap<u64, FileCtx>>);

pub struct FileCtx {
    pub path: PathBuf,
    pub hash: u64,
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
        fs::read_dir(dir)?.par_bridge().map(|entry| {
                let entry = entry?;
                let path = PathBuf::from(&format!("{}/{}", dir.display(), entry.path().display()));

                // Check if the entry is a directory or a file
                if entry.file_type().unwrap().is_dir() {
                    // If it is one of the stream randomizer directories
                    if let Some(_) = path.extension() {
                        match self.visit_file(&entry, &path, arc_dir_len) {
                            Ok(file_ctx) => {
                                self.0.write().unwrap().insert(file_ctx.hash, file_ctx);
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
                        Ok(file_ctx) => {
                            self.0.write().unwrap().insert(file_ctx.hash, file_ctx);
                            return Ok(());
                        },
                        Err(err) => {
                            println!("{}", err);
                            return Ok(())
                        }
                    }
                }

                Ok(())
            }).collect()
    }

    fn visit_file(&self, entry: &DirEntry, full_path: &PathBuf, arc_dir_len: usize) -> Result<FileCtx, String> {
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

        let mut file_ctx = FileCtx::new();

        file_ctx.path = full_path.to_path_buf();
        file_ctx.hash = hash40(&game_path);

        file_ctx.filesize = match entry.metadata() {
            Ok(meta) => meta.len() as u32,
            Err(err) => panic!(err),
        };

        // TODO: Move this method in a impl for FileCtx
        file_ctx.filesize_replacement();
        Ok(file_ctx)
    }
}

impl FileCtx {
    pub fn new() -> Self {
        FileCtx {
            path: PathBuf::new(),
            hash: 0,
            filesize: 0,
            orig_subfile: SubFile {
                offset: 0,
                compressed_size: 0,
                decompressed_size: 0,
                flags: 0,
            },
        }
    }

    pub fn get_region(&self) -> u32 {
        // Default to the player's region index
        let mut region_index = ResServiceState::get_instance().regular_region_idx;

        // Make sure the file has an extension
        if let Some(_) = self.path.extension() {
            // Split the region identifier from the filepath
            let region = self.path.to_str().unwrap().to_string();
            // Check if the filepath it contains a + symbol
            if let Some(region_marker) = region.find('+') {
                match &region[region_marker+1..region_marker+6] {
                    "jp_ja" => region_index = 0,
                    "us_en" => region_index = 1,
                    "us_fr" => region_index = 2,
                    "us_es" => region_index = 3,
                    "eu_en" => region_index = 4,
                    "eu_fr" => region_index = 5,
                    "eu_es" => region_index = 6,
                    "eu_de" => region_index = 7,
                    "eu_nl" => region_index = 8,
                    "eu_it" => region_index = 9,
                    "eu_ru" => region_index = 10,
                    "kr_ko" => region_index = 11,
                    "zh_cn" => region_index = 12,
                    "zh_tw" => region_index = 13,
                    _ => region_index = 1,
                }
            }
        }

        region_index
    }

    pub fn filesize_replacement(&mut self) {
        let loaded_tables = LoadedTables::get_instance();

        let extension = match self.path.extension() {
            Some(ext) => ext.to_str().unwrap(),
            None => {
                println!("File {} does not have an extension, skipping", self.path.display());
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
                .par_iter()
                .position_any(|x| x.path.hash40.as_u64() == self.hash)
            {
                Some(index) => index as u32,
                None => {
                    println!(
                        "[ARC::Patching] Hash for file {} not found in table1, skipping",
                        self.path.display()
                    );
                    return;
                }
            };

            let mut subfile = loaded_tables.get_arc().get_subfile_by_t1_index(t1_index);

            // Gotta make SubFile derive Clone and Copy 'cause this is massive ass
            self.orig_subfile.offset = (*subfile).offset;
            self.orig_subfile.compressed_size = (*subfile).compressed_size;
            self.orig_subfile.decompressed_size = (*subfile).decompressed_size;
            self.orig_subfile.flags = (*subfile).flags;

            if (subfile.decompressed_size < self.filesize) && extension == "nutexb" {
                // Is compressed?
                if (subfile.flags & 0x3) == 3 {
                    subfile.decompressed_size = self.filesize;

                    println!(
                        "[ARC::Patching] New decompressed size for {}: {:#x}",
                        self.path.display(),
                        subfile.decompressed_size
                    );
                }
            } else {
                if subfile.decompressed_size < self.filesize {
                    subfile.decompressed_size = self.filesize;
                    println!(
                        "[ARC::Patching] New decompressed size for {}: {:#x}",
                        self.path.display(),
                        subfile.decompressed_size
                    );
                }
            }
        }
    }
}
