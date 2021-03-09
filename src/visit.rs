use std::{
    fs,
    path::{
        Path,
        PathBuf
    }
};

use log::warn;
use smash_arc::{Hash40, Region};

use crate::replacement_files::get_region_id;

enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

#[derive(Debug, Clone)]
pub struct Modpack {
    pub path: PathBuf,
    pub mods: Vec<ModFile>
}

impl Modpack {
    pub fn flatten(&self) -> Vec<(Hash40, ModFile)> {
        self.mods.iter().filter_map(|file| {
            let mut full_path = self.path.to_owned();
            full_path.push(&file.path());

            let hash = file.hash40().unwrap();
            let mut new_file = file.to_owned();
            new_file.set_path(full_path);

            Some((hash, new_file))
        }).collect()
    }
}

#[derive(Debug, Clone)]
pub struct ModFile {
    path: PathBuf,
    size: u32,
}

impl ModFile {
    pub fn new() -> Self {
        Self {
            path: PathBuf::new(),
            size: 0,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn set_path<P: AsRef<Path>>(&mut self, new_path: P) {
        self.path = new_path.as_ref().to_path_buf();
    }

    pub fn as_smash_path(&self) -> PathBuf {
        let mut arc_path = self.path.to_str().unwrap().to_string();

        if let Some(_) = arc_path.find(";") {
            arc_path = arc_path.replace(";", ":");
        }

        if let Some(regional_marker) = arc_path.find("+") {
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

    pub fn hash40(&self) -> Result<Hash40, String> {
        let smash_path = self.as_smash_path();

        match smash_path.to_str() {
            Some(path) => Ok(Hash40::from(path)),
            // TODO: Replace this by a proper error. This-error or something else.
            None => Err(String::from(format!("Couldn't convert {} to a &str", self.path.display()))),
        }
    }

    pub fn len(&self) -> u32 {
        self.size
    }

    pub fn is_stream(&self) -> bool {
        //self.path.starts_with("stream")
        self.path.to_str().unwrap().contains("stream")
    }

    pub fn get_region(&self) -> Option<Region> {
        match self.path.extension() {
            Some(_) => {
                // Split the region identifier from the filepath
                let filename = self.path.file_name().unwrap().to_str().unwrap().to_string();
                // Check if the filepath it contains a + symbol
                let region = if let Some(region_marker) = filename.find('+') {
                    Some(Region::from(get_region_id(&filename[region_marker + 1..region_marker + 6]).unwrap_or(0) + 1))
                } else {
                    None
                };

                region
            },
            None => None,
        }
    }
}

pub fn discover<P: AsRef<Path>>(path: &P) -> Modpack {
    let mut modpack = Modpack {
        path: path.as_ref().to_path_buf(),
        mods: vec![],
    };

    modpack.mods = directory(&path);

    modpack.mods.iter_mut().for_each(|mut filepath| {
        filepath.path = filepath.path.strip_prefix(&path).unwrap().to_path_buf();
    });

    modpack
}

/// Visit Ultimate Mod Manager directories for backwards compatibility
pub fn umm_directories<P: AsRef<Path>>(path: &P) -> Vec<Modpack> {
    let mut mods = Vec::<Modpack>::new();

    let base_path = path.as_ref();

    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();

        // Make sure this is a directory we're dealing with
        if !entry.file_type().unwrap().is_dir() {
            continue;
        }

        // Skip any directory starting with a period
        if entry.file_name().to_str().unwrap().starts_with(".") {
            continue;
        }

        let mut subdir_path = base_path.to_owned();
        subdir_path.push(entry.path());

        mods.push(discover(&subdir_path));
    }

    mods
}

pub fn directory<P: AsRef<Path>>(path: &P) -> Vec<ModFile> {
    let path = path.as_ref();

    let paths: Vec<OneOrMany<ModFile>> = match fs::read_dir(path) {
        Ok(res) => {
            res.filter_map(|entry| {
                let entry = entry.unwrap();

                let mut entry_path = path.to_path_buf();
                entry_path.push(entry.path());

                // Ignore anything that starts with a period
                if entry_path.file_name().unwrap().to_str().unwrap().starts_with(".") {
                    return None;
                }

                if entry.file_type().unwrap().is_dir(){
                    if entry_path.file_name().unwrap().to_str().unwrap().contains(".") {
                        let modpath = ModFile {
                            path: entry_path,
                            size: 0,
                        };
                        Some(OneOrMany::One(modpath))
                    } else {
                        Some(OneOrMany::Many(directory(&entry_path).to_vec()))
                    } } else {
                    match file(&entry_path) {
                        Ok(file_ctx) => {
                            let modpath = ModFile {
                                path: file_ctx,
                                size: entry.metadata().unwrap().len() as u32,
                            };
                            Some(OneOrMany::One(modpath))
                        },
                        Err(err) => {
                            warn!("{}", err);
                            None
                        }
                    }
                }
            }).collect()
        },
        Err(err) => {
            warn!("{}", err);
            vec![]
        }
    };

    let mut final_vec: Vec<ModFile> = Vec::new();

    for instance in paths {
        match instance {
            OneOrMany::One(context) => final_vec.push(context),
            OneOrMany::Many(mut contexts) => final_vec.append(&mut contexts),
        }
    }

    final_vec
}

pub fn file<P: AsRef<Path>>(path: &P) -> Result<PathBuf, String> {
    let path = path.as_ref();

        if path.is_dir() {
            return Err("[ARC::Discovery] Not a file".to_string());
        }

        // Make sure the file has an extension to not cause issues with the code that follows
        if path.extension() == None {
            return Err(format!("[ARC::Discovery] File '{}' does not have an extension, skipping", path.display()));
        }

        Ok(path.to_path_buf())
}
