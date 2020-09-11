use smash::hash40;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use std::ffi::CString;

use skyline::{c_str, nn};

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
                if path.is_dir() && path.file_name().unwrap().to_os_string().into_string().unwrap().contains("."){
                    self.visit_file(path, arc_dir_len);
                }
                else if path.is_dir() {
                    self.visit_dir(&path, arc_dir_len)?;
                } else {
                    self.visit_file(path, arc_dir_len);
                }
            }
        }

        Ok(())
    }

    // This is for the rework, don't mind it for now
    fn visit_dir_rewrite(&mut self, dir: &Path, _arc_dir_len: usize) -> io::Result<()> {
        if dir.is_dir() {
            unsafe {
                let mut handle = nn::fs::DirectoryHandle {
                    handle: 0 as *mut skyline::libc::c_void,
                };

                nn::fs::OpenDirectory(
                    &mut handle,
                    c_str(dir.as_os_str().to_str().unwrap()),
                    nn::fs::OpenDirectoryMode_OpenDirectoryMode_All as i32,
                );

                let mut entry_count = 0;
                nn::fs::GetDirectoryEntryCount(&mut entry_count, handle);

                let mut dir_entries: Vec<nn::fs::DirectoryEntry> = vec![nn::fs::DirectoryEntry { name: [0; 769], _x302: [0;3], type_: 0, _x304: 0, fileSize: 0 }; entry_count as usize];
                let dir_entries = dir_entries.as_mut_slice();
                let mut count_result = 0;
                nn::fs::ReadDirectory(&mut count_result, dir_entries.as_mut_ptr(), handle, entry_count);

                println!("{}", CString::from_vec_unchecked(dir_entries[0].name.to_vec()).to_str().unwrap());
            }
        }

        Ok(())
    }

    fn visit_file(&mut self, path: &Path, arc_dir_len: usize) {
        match path.extension().and_then(std::ffi::OsStr::to_str) {
            Some(_) => {}
            None => {
                println!("Error getting file extension for: {}", path.display());
                return;
            }
        }

        // Here was CoolSonicKirby's fix to ignore unsupported formats. May it rest in peace.
        let mut game_path = path.display().to_string()[arc_dir_len + 1..].replace(";", ":");
        match game_path.strip_suffix("mp4") {
            Some(x) => game_path = format!("{}{}", x, "webm"),
            None => (),
        }

        let hash = hash40(&game_path);
        self.0.insert(hash, path.to_owned());
    }

    pub fn get_from_hash(&self, hash: u64) -> Option<&PathBuf> {
        self.0.get(&hash)
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, u64, std::path::PathBuf> {
        self.0.iter()
    }
}
