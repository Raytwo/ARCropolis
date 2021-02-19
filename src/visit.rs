use std::{
    fs,
    path::{
        Path,
        PathBuf
    }
};

use log::warn;
use smash_arc::{Hash40, Region};

enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

#[derive(Debug)]
pub struct Mod {
    pub path: PathBuf,
    pub mods: Vec<ModPath>
}

#[derive(Debug, Default)]
pub struct ModPath {
    pub path: PathBuf,
    pub size: u64,
}

impl ModPath {
    pub fn new<P: AsRef<Path>>(path: &P) -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn as_smash_path(&self) -> PathBuf {
        let mut arc_path = self.path.to_str().unwrap().to_string();

        if let Some(_) = arc_path.find(";") {
            arc_path = arc_path.replace(";", ":");
        }

        if let Some(regional_marker) = arc_path.find("+") {
            arc_path.replace_range(regional_marker..arc_path.find(".").unwrap(), "");
        }

        if let Some(ext) = arc_path.strip_suffix("mp4") {
            arc_path = format!("{}{}", ext, "webm");
        }

        // Some mods forget that paths do not have capitals. This fixes that.
        arc_path = arc_path.to_lowercase();

        PathBuf::from(arc_path)
    }

    pub fn hash40(&self) -> Result<Hash40, String> {
        let smash_path = self.as_smash_path();

        match smash_path.to_str() {
            Some(path) => Ok(Hash40::from(path)),
            // TODO: Replace this by a proper error. This-error or something else.
            None => Err(String::from(format!("Couldn't convert {} to a &str", self.path.display()))),
        }
    }

    pub fn is_stream(&self) -> bool {
        self.path.starts_with("stream;")
        // TODO: Probably an extra check for the extension too?
    }

    pub fn get_region(&self) -> Option<Region> {
        match self.path.extension() {
            Some(_) => {
                // Split the region identifier from the filepath
                let filename = self.path.file_name().unwrap().to_str().unwrap().to_string();
                // Check if the filepath it contains a + symbol
                let region = if let Some(region_marker) = filename.find('+') {
                    let region = match &filename[region_marker + 1..region_marker + 6] {
                        "jp_ja" => Region::Japanese,
                        "us_en" => Region::UsEnglish,
                        "us_fr" => Region::UsFrench,
                        "us_es" => Region::UsSpanish,
                        "eu_en" => Region::EuEnglish,
                        "eu_fr" => Region::EuFrench,
                        "eu_es" => Region::EuSpanish,
                        "eu_de" => Region::EuGerman,
                        "eu_nl" => Region::EuDutch,
                        "eu_it" => Region::EuItalian,
                        "eu_ru" => Region::EuRussian,
                        "kr_ko" => Region::Korean,
                        "zh_cn" => Region::ChinaChinese,
                        "zh_tw" => Region::TaiwanChinese,
                        // If the regional indicator makes no sense, default to us_en
                        _ => Region::UsEnglish,
                    };

                    Some(region)
                } else {
                    None
                };

                region
            },
            None => None,
        }
    }
}

pub fn discover<P: AsRef<Path>>(path: &P) -> Mod {
    let mut new_mod = Mod {
        path: path.as_ref().to_path_buf(),
        mods: vec![],
    };

    new_mod.mods = directory(&path);

    new_mod.mods.iter_mut().for_each(|mut filepath| {
        filepath.path = filepath.path.strip_prefix(&path).unwrap().to_path_buf();
        //ModPath(filepath.path.strip_prefix(&path).unwrap().to_path_buf())
    });

    new_mod
}

/// Visit Ultimate Mod Manager directories for backwards compatibility
pub fn umm_directories<P: AsRef<Path>>(path: &P) -> Vec<Mod> {
    let mut mods = Vec::<Mod>::new();

    let base_path = path.as_ref();

    // TODO: Careful here, sometimes a /umm path does not exist.
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();

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

pub fn directory<P: AsRef<Path>>(path: &P) -> Vec<ModPath> {
    let path = path.as_ref();

    // TODO: Make sure the path exists before proceeding
    let paths: Vec<OneOrMany<ModPath>> = fs::read_dir(path).unwrap().filter_map(|entry| {
        let entry = entry.unwrap();

        let mut entry_path = path.to_path_buf();
        entry_path.push(entry.path());

        // Ignore anything that starts with a period
        if entry_path.file_name().unwrap().to_str().unwrap().starts_with(".") {
            return None;
        }

        if entry.file_type().unwrap().is_dir() {
            Some(OneOrMany::Many(directory(&entry_path)))
        } else {
            match file(&entry_path) {
                Ok(file_ctx) => {
                    let modpath = ModPath {
                        path: file_ctx,
                        size: entry.metadata().unwrap().len(),
                    };
                    Some(OneOrMany::One(modpath))
                },
                Err(err) => {
                    warn!("{}", err);
                    None
                }
            }
        }
    }).collect();

    let mut final_vec: Vec<ModPath> = Vec::new();

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