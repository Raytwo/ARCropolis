use std::path::{Path, PathBuf};
use std::{collections::HashMap, fs, io, slice};

use smash::hash40;
use smash::resource::LoadedTables;

use crate::config::CONFIG;

lazy_static::lazy_static! {
    pub static ref ARC_FILES: ArcFiles = ArcFiles::new();
}

pub struct ArcFiles(pub HashMap<u64, PathBuf>);

impl ArcFiles {
    fn new() -> Self {
        let mut instance = Self(HashMap::new());

        let _ = instance.visit_dir(Path::new(&CONFIG.paths.arc), CONFIG.paths.arc.len());
        let _ = instance.visit_umm_dirs(Path::new(&CONFIG.paths.umm));

        instance
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

    fn visit_dir(&mut self, dir: &Path, arc_dir_len: usize) -> io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let filename = entry.path();
                let real_path = format!("{}/{}", dir.display(), filename.display());
                let path = Path::new(&real_path);
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

                    println!("Path: {}", path.display());
                    let mut game_path =
                        path.display().to_string()[arc_dir_len + 1..].replace(";", ":");

                    match game_path.strip_suffix("mp4") {
                        Some(x) => game_path = format!("{}{}", x, "webm"),
                        None => (),
                    }

                    let hash = hash40(&game_path);
                    let metadata = match entry.metadata() {
                        Ok(meta) => meta,
                        Err(err) => panic!(err),
                    };
                    self.0.insert(hash, path.to_owned());
                    self.filesize_replacement_rewrite(hash, path, metadata.len() as _);
                } else if path.is_dir() {
                    self.visit_dir(&path, arc_dir_len)?;
                } else {
                    match path.extension().and_then(std::ffi::OsStr::to_str) {
                        Some(_) => {}
                        None => {
                            println!("Error getting file extension for: {}", path.display());
                        }
                    }

                    println!("Path: {}", path.display());
                    let mut game_path =
                        path.display().to_string()[arc_dir_len + 1..].replace(";", ":");

                    match game_path.strip_suffix("mp4") {
                        Some(x) => game_path = format!("{}{}", x, "webm"),
                        None => (),
                    }

                    let hash = hash40(&game_path);
                    let metadata = match entry.metadata() {
                        Ok(meta) => meta,
                        Err(err) => panic!(err),
                    };
                    self.0.insert(hash, path.to_owned());
                    self.filesize_replacement_rewrite(hash, path, metadata.len() as _);
                }
            }
        }

        Ok(())
    }

    pub fn filesize_replacement_rewrite(&self, hash: u64, path: &Path, filesize: u32) {
        let loaded_tables = LoadedTables::get_instance();

        unsafe {
            let extension = path.extension().unwrap().to_str().unwrap();
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
                        path.display()
                    );
                    return;
                }
            };

            let mut subfile = loaded_tables.get_arc().get_subfile_by_t1_index(t1_index);

            if (subfile.decompressed_size < filesize) && extension == "nutexb" {
                // Is compressed?
                if (subfile.flags & 0x3) == 3 {
                    subfile.decompressed_size = filesize;

                    println!(
                        "[ARC::Patching] New decompressed size for {}: {:#x}",
                        path.display(),
                        subfile.decompressed_size
                    );
                }
            } else {
                if subfile.decompressed_size < filesize {
                    subfile.decompressed_size = filesize;
                    println!(
                        "[ARC::Patching] New decompressed size for {}: {:#x}",
                        path.display(),
                        subfile.decompressed_size
                    );
                }
            }
        }
    }

    pub fn get_from_hash(&self, hash: u64) -> Option<&PathBuf> {
        self.0.get(&hash)
    }
}
