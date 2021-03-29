use std::{
    fs,
    path::{Path, PathBuf},
};

use fs::metadata;
use smash_arc::{Hash40, Region};

use crate::replacement_files::get_region_id;

#[derive(Debug, Clone)]
pub struct Modpack {
    path: PathBuf,
    pub mods: Vec<Modpath>,
}

impl Modpack {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_owned(),
            mods: vec![],
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&mut self, modpaths: Vec<Modpath>) {
        self.mods = modpaths
            .iter()
            .map(|filepath| filepath.0.strip_prefix(&self.path).unwrap().to_owned())
            .filter(|path| {
                if path.starts_with(".") {
                    false
                } else {
                    // Make sure the file has an extension, because if not we might get a panic later on
                    path.extension().is_some()
                }
            })
            .map(|path| path.into())
            .collect();
    }

    // TODO: Rework this to be a iterator like DirEntry but with Modpaths/Modfile
    pub fn merge(&self) -> Vec<(Hash40, ModFile)> {
        self.mods
            .iter()
            .map(|modpath| {
                let full_path = self.path.to_owned().join(&modpath.path()).into();

                let hash = modpath.hash40().unwrap();

                (hash, full_path)
            })
            .collect()
    }
}
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct Modpath(pub PathBuf);

impl From<Modpath> for PathBuf {
    fn from(modpath: Modpath) -> Self {
        modpath.0
    }
}

impl From<PathBuf> for Modpath {
    fn from(path: PathBuf) -> Self {
        Self(path)
    }
}

impl From<PathBuf> for ModFile {
    fn from(path: PathBuf) -> Self {
        Self(path)
    }
}

impl From<ModFile> for PathBuf {
    fn from(modfile: ModFile) -> Self {
        modfile.0
    }
}

impl From<Modpath> for ModFile {
    fn from(modpath: Modpath) -> Self {
        Self(modpath.0)
    }
}

impl Modpath {
    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn hash40(&self) -> Result<Hash40, String> {
        let smash_path = self.as_smash_path();

        match smash_path.to_str() {
            Some(path) => Ok(Hash40::from(path)),
            // TODO: Replace this by a proper error. This-error or something else.
            None => Err(format!(
                "Couldn't convert {} to a &str",
                self.path().display()
            )),
        }
    }

    pub fn as_smash_path(&self) -> PathBuf {
        let mut arc_path = self.0.to_str().unwrap().to_string();

        if arc_path.find(';').is_some() {
            arc_path = arc_path.replace(";", ":");
        }

        if let Some(regional_marker) = arc_path.find('+') {
            arc_path.replace_range(regional_marker..regional_marker + 6, "");
        }

        if let Some(ext) = arc_path.strip_suffix("mp4") {
            arc_path = format!("{}{}", ext, "webm");
        }

        // Some mods forget that paths do not have capitals. This fixes that.
        arc_path = arc_path.to_lowercase();

        PathBuf::from(arc_path)
    }

    pub fn is_stream(&self) -> bool {
        self.0.to_str().unwrap().contains("stream")
    }
}

// TODO: Make a "Modpath" variant for everything that is relative to /arc, and use "Modfile" for storing an absolute SD path
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct ModFile(PathBuf);

impl ModFile {
    pub fn new() -> Self {
        Self(PathBuf::new())
    }
    pub fn from<P: AsRef<Path>>(path: P) -> Self {
        Self(path.as_ref().to_path_buf())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn set_path<P: AsRef<Path>>(&mut self, new_path: P) {
        self.0 = new_path.as_ref().to_path_buf();
    }

    pub fn as_smash_path(&self) -> PathBuf {
        let mut arc_path = self.path().to_str().unwrap().to_string();

        if arc_path.find(';').is_some() {
            arc_path = arc_path.replace(";", ":");
        }

        if let Some(regional_marker) = arc_path.find('+') {
            arc_path.replace_range(regional_marker..regional_marker + 6, "");
        }

        if let Some(ext) = arc_path.strip_suffix("mp4") {
            arc_path = format!("{}{}", ext, "webm");
        }

        // Some mods forget that paths do not have capitals. This fixes that.
        arc_path = arc_path.to_lowercase();

        PathBuf::from(arc_path)
    }

    pub fn extension(&self) -> Hash40 {
        Hash40::from(self.path().extension().unwrap().to_str().unwrap())
    }

    pub fn len(&self) -> u32 {
        metadata(&self.path()).unwrap().len() as u32
    }

    pub fn is_stream(&self) -> bool {
        //self.path.starts_with("stream")
        self.path().to_str().unwrap().contains("stream")
    }

    pub fn get_region(&self) -> Option<Region> {
        match self.path().extension() {
            Some(_) => {
                // Split the region identifier from the filepath
                let filename = self
                    .path()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();
                // Check if the filepath it contains a + symbol
                if let Some(region_marker) = filename.find('+') {
                    Some(Region::from(
                        get_region_id(&filename[region_marker + 1..region_marker + 6]).unwrap_or(0)
                            + 1,
                    ))
                } else {
                    None
                }
            }
            None => None,
        }
    }
}
