use std::{hash::{Hash, Hasher}, path::{Path, PathBuf}};
use std::ops::Deref;

use serde::{Serialize, Deserialize};
use smash_arc::{Hash40, Region};

#[derive(Debug)]
pub enum RejectionReason {
    DuplicateFile(PathBuf),
    NotFound(SmashPath),
    MissingExtension
}

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SmashPath(pub PathBuf);

impl Deref for SmashPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<Path> for SmashPath {
    fn as_ref(&self) -> &Path {
        &self
    }
}

impl From<SmashPath> for PathBuf {
    fn from(smashpath: SmashPath) -> Self {
        smashpath.0
    }
}

impl SmashPath {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path = path.as_ref();

        if path.extension().is_some() {
            Ok(Self(path.to_path_buf()))
        } else {
            Err(String::from("This path does not have an extension"))
        }
    }

    pub fn as_path(&self) -> &Path {
        &self
    }

    pub fn hash40(&self) -> Result<Hash40, String> {
        let smash_path = self.to_smash_path();

        smash_path.to_str()
            .map(Hash40::from)
            .ok_or_else(|| format!("Couldn't convert {} to an &str", self.as_path().display()))
    }

    pub fn to_smash_path(&self) -> PathBuf {
        let mut arc_path = self.to_str().unwrap().to_string();
        
        if arc_path.find(';').is_some() {
            arc_path = arc_path.replace(";", ":");
        }

        if let Some(regional_marker) = arc_path.find('+') {
            arc_path.replace_range(regional_marker..regional_marker + 6, "");
        }

        if let Some(ext) = arc_path.strip_suffix("mp4") {
            arc_path = format!("{}{}", ext, "webm");
        }

        PathBuf::from(arc_path.to_lowercase())
    }

    pub fn is_stream(&self) -> bool {
        self.0.to_str().map(|path| path.contains("stream")).unwrap_or(false)
    }

    pub fn extension(&self) -> Hash40 {
        Hash40::from(
            self
                .as_path()
                .extension()
                .unwrap()
                .to_str()
                .unwrap()
        )
    }

    pub fn get_region(&self) -> Option<Region> {
        let filename = self
            .as_path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        
        if let Some(region_marker) = filename.find('+') {
            Some(Region::from(
                crate::replacement_files::get_region_id(
                    &filename[region_marker + 1..region_marker + 6]
                ).unwrap_or(0) + 1
            ))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModFile {
    pub mod_folder: PathBuf,
    pub path: SmashPath,
    pub size: usize
}

impl Deref for ModFile {
    type Target = SmashPath;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl Hash for ModFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl ModFile {
    pub fn new(mod_folder: PathBuf, path: SmashPath) -> Self {
        Self {
            size: std::fs::metadata(mod_folder.join(path.as_path())).unwrap().len() as usize,
            mod_folder,
            path,
        }
    }

    pub fn from_parts(mod_folder: PathBuf, path: SmashPath, size: usize) -> Self {
        Self {
            mod_folder,
            path,
            size
        }
    }

    pub fn full_path(&self) -> PathBuf {
        self.mod_folder.join(&self.path)
    }
}

// Cache file structures
#[derive(Serialize, Deserialize)]
pub struct FileInformation {
    pub path: String,
    pub size: usize,
    pub replacement_hash: u64
}

#[derive(Serialize, Deserialize)]
pub struct CacheFile {
    pub arc_version: u32,
    pub mod_version: u32,
    pub file_infos: Vec<FileInformation>
}

impl CacheFile {
    pub fn new(arc_version: u32, mod_version: u32) -> Self {
        Self {
            arc_version,
            mod_version,
            file_infos: Vec::new()
        }
    }

    pub fn open<P: AsRef<Path>>(path: P, arc_version: u32, mod_version: u32) -> Option<Self> {
        let data = std::fs::read(path).unwrap();
        let mut data = std::io::Cursor::new(data);
        let cached_arc_version: u32 = bincode::deserialize_from(&mut data)
            .expect("Failed to deserialize ARC version");
        if arc_version != cached_arc_version {
            return None;
        }
        let cached_mod_version: u32 = bincode::deserialize_from(&mut data)
            .expect("Failed to deserialize MOD version");
        if mod_version != cached_mod_version {
            return None;
        }
        Some(Self {
            arc_version: cached_arc_version,
            mod_version: cached_mod_version,
            file_infos: bincode::deserialize_from(&mut data)
                .expect("Failed to deserialize file information!")
        })
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) {
        let data = bincode::serialize(self).unwrap();
        std::fs::write(path, data).unwrap();
    }
}
