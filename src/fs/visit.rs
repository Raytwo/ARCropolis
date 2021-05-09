use std::{
    collections::HashMap,
    ops::Deref,
    path::{Path, PathBuf},
};

use crate::config::{CONFIG, REGION};
use crate::replacement_files::get_region_id;

use smash_arc::{Hash40, Region};

use walkdir::WalkDir;

/// Discover every file in a directory and its sub-directories.  
/// Files starting with a period are filtered out, and only the files with relevant regions are kept.  
/// This signifies that if your goal is to simply get all the files, this is not the method to use.
pub fn discovery(dir: &PathBuf) -> HashMap<Hash40, ModPath> {
    let user_region = *REGION;

    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|entry| {
            // If it starts with a period
            !entry.file_name().to_str().unwrap().starts_with('.')
        })
        .filter_map(|entry| {
            let entry = entry.unwrap();

            // Only process files
            if entry.file_type().is_file() {
                // Make sure the file has an extension
                if entry.path().extension().is_some() {
                    let path: SmashPath =
                        SmashPath(entry.path().strip_prefix(dir).unwrap().to_path_buf());

                    if let Some(region) = path.get_region() {
                        if region != user_region {
                            return None;
                        }
                    }

                    let hash = path.hash40().unwrap();
                    Some((hash, entry.path().to_path_buf().into()))
                } else {
                    println!("File has no extension, aborting");
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

/// Run ``discovery`` on every directory found using the path  
/// Files starting with a period are filtered out, and only the files with relevant regions are kept.  
/// This signifies that if your goal is to simply get all the files, this is not the method to use.  
/// This method exists to support backward compatibility with Ultimate Mod Manager.  
pub fn umm_discovery(dir: &PathBuf) -> HashMap<Hash40, ModPath> {
    WalkDir::new(dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_str().unwrap().starts_with('.'))
        .flat_map(|entry| {
            let entry = entry.unwrap();

            if !entry.file_type().is_dir() {
                return Err(());
            }

            Ok(discovery(&entry.into_path()))
        })
        .flatten()
        .collect()
}

/// Utility struct for the purpose of storing a relative Smash path (starting at the root of the ``/arc`` filesystem).  
/// A few methods are provided to obtain a Hash40 or strip ARCropolis-relevant informations such as a regional indicator.
#[repr(transparent)]
#[derive(Debug, Clone, Default)]
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
    fn from(modpath: SmashPath) -> Self {
        modpath.0
    }
}

impl From<PathBuf> for SmashPath {
    fn from(path: PathBuf) -> Self {
        Self(path)
    }
}

impl From<ModPath> for PathBuf {
    fn from(modfile: ModPath) -> Self {
        modfile.0
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
            Err("This path does not have an extension".to_string())
        }
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn hash40(&self) -> Result<Hash40, String> {
        let smash_path = self.to_smash_path();

        match smash_path.to_str() {
            Some(path) => Ok(Hash40::from(path)),
            // TODO: Replace this by a proper error. This-error or something else.
            None => Err(format!(
                "Couldn't convert {} to a &str",
                self.as_path().display()
            )),
        }
    }

    pub fn to_smash_path(&self) -> PathBuf {
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

    pub fn get_region(&self) -> Option<Region> {
        // Split the region identifier from the filepath
        let filename = self
            .as_path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        // Check if the filepath it contains a + symbol
        if let Some(region_marker) = filename.find('+') {
            Some(Region::from(
                get_region_id(&filename[region_marker + 1..region_marker + 6]).unwrap_or(0) + 1,
            ))
        } else {
            None
        }
    }
}

// TODO: Should probably deref to a Path
/// Utility struct for the purpose of storing an absolute modfile path (starting at the root of the ``sd:/`` filesystem)
/// A few methods are provided to obtain a ARCropolis-relevant informations such as the regional indicator
#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ModPath(PathBuf);

impl Deref for ModPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<Path> for ModPath {
    fn as_ref(&self) -> &Path {
        &self
    }
}

impl From<PathBuf> for ModPath {
    fn from(path: PathBuf) -> Self {
        Self(path)
    }
}

impl From<SmashPath> for ModPath {
    fn from(modpath: SmashPath) -> Self {
        Self(modpath.0)
    }
}

impl ModPath {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path = path.as_ref();

        if path.extension().is_some() {
            Ok(Self(path.to_path_buf()))
        } else {
            Err("This path does not have an extension".to_string())
        }
    }

    pub fn as_path(&self) -> &Path {
        &self
    }

    pub fn to_smash_path(&self) -> PathBuf {
        let mut arc_path = self.as_path().to_str().unwrap().to_string();

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
        Hash40::from(self.as_path().extension().unwrap().to_str().unwrap())
    }

    pub fn len(&self) -> u32 {
        std::fs::metadata(self).unwrap().len() as u32
    }

    pub fn is_stream(&self) -> bool {
        self.to_str().unwrap().contains("stream")
    }

    pub fn get_region(&self) -> Option<Region> {
        // Split the region identifier from the filepath
        let filename = self
            .as_path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        // Check if the filepath it contains a + symbol
        if let Some(region_marker) = filename.find('+') {
            Some(Region::from(
                get_region_id(&filename[region_marker + 1..region_marker + 6]).unwrap_or(0) + 1,
            ))
        } else {
            None
        }
    }
}
