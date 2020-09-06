use smash::hash40;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use crate::config::CONFIG;

lazy_static::lazy_static! {
    pub static ref ARC_FILES: ArcFiles = ArcFiles::new();
    pub static ref STREAM_FILES: StreamFiles = StreamFiles::new();
}

pub struct ArcFiles(pub HashMap<u64, PathBuf>);

pub struct StreamFiles(pub HashMap<u64, PathBuf>);

impl StreamFiles {
    fn new() -> Self {
        let mut instance = Self(HashMap::new());

        let _ = instance.visit_dir(Path::new(&CONFIG.paths.stream), CONFIG.paths.stream.len());

        let _ = instance.visit_umm_dirs(Path::new(&CONFIG.paths.umm));

        instance
    }

    fn visit_dir(&mut self, dir: &Path, cut_len: usize) -> io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let filename = entry.path();
                let real_path = format!("{}/{}", dir.display(), filename.display());
                let path = Path::new(&real_path);
                if path.is_dir() && path.display().to_string().contains(".") {
                    self.visit_file(path, cut_len);
                } else if path.is_dir() {
                    self.visit_dir(&path, cut_len)?;
                } else {
                    self.visit_file(path, cut_len);
                }
            }
        }

        Ok(())
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
                let stream_entry_path =
                    format!("{}/{}/stream;", dir.display(), entry.path().display());
                let cut_len = stream_entry_path.len();
                if Path::new(&stream_entry_path).exists() {
                    for stream_entry in fs::read_dir(Path::new(&stream_entry_path))? {
                        let stream_entry = stream_entry?;
                        let filename = stream_entry.path();
                        let real_path = format!("{}/{}", stream_entry_path, filename.display());
                        let path = Path::new(&real_path);
                        self.visit_dir(&path, cut_len)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn visit_file(&mut self, path: &Path, cut_len: usize) {
        let mut game_path = format!("stream:{}", &path.display().to_string()[cut_len..]);
        match game_path.strip_suffix("mp4") {
            Some(x) => game_path = format!("{}{}", x, "webm"),
            None => (),
        }
        if !format!("{:?}", &path.file_name().unwrap()).contains("._") {
            let hash = hash40(&game_path);
            self.0.insert(hash, path.to_owned());
        }
    }
}

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
                if path.is_dir() {
                    self.visit_dir(&path, arc_dir_len)?;
                } else {
                    self.visit_file(path, arc_dir_len);
                }
            }
        }

        Ok(())
    }

    fn visit_file(&mut self, path: &Path, arc_dir_len: usize) {
        let mut file_ext;

        match path.extension().and_then(std::ffi::OsStr::to_str) {
            Some(x) => file_ext = x,
            None => {
                println!("Error getting file extension for: {}", path.display());
                return;
            }
        }

        // Here was CoolSonicKirby's fix to ignore unsupported formats. May it rest in peace.
        let mut game_path = path.display().to_string()[arc_dir_len + 1..].replace(";", ":");

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
