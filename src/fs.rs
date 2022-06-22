use std::{collections::{HashMap, HashSet}, io::Write};

use ahash::AHashSet;
use camino::{Utf8PathBuf, Utf8Path};
use owo_colors::OwoColorize;
use serde::Serialize;
use smash_arc::{Hash40, ArcLookup, hash40};
use thiserror::Error;

use crate::{hashes, FILESYSTEM, replacement::LoadedArcEx};

// pub mod api;
// mod event;

mod discover;
pub mod interner;
pub use discover::*;

static DEFAULT_CONFIG: &str = include_str!("../resources/override.json");

#[derive(Default)]
pub struct LoadingState {
    incoming_file: Option<Hash40>,
    remaining_bytes: usize,
}

impl LoadingState {
    pub fn new() -> Self {
        Self { incoming_file: None, remaining_bytes: 0 }
    }

    // NOTE: Some sources such as API callbacks cannot provide a physical path. This needs proper handling
    pub fn get_physical_path<H: Into<Hash40>>(&self, hash: H) -> Option<&Utf8PathBuf> {
        FILESYSTEM.get().unwrap().get(&hash.into())
    }

    pub fn set_incoming_file<H: Into<Hash40>>(&mut self, hash: H) {
        if let Some(hash) = self.incoming_file.take() {
            println!(
            "Removing file '{}' ({:#x}) from incoming load before using it.",
                hashes::find(hash),
                hash.0
            );
        }

        let hash = hash.into();
        
        self.incoming_file = Some(hash);
        self.remaining_bytes = std::fs::metadata(FILESYSTEM.get().unwrap().get(&hash).unwrap()).unwrap().len() as _;
    }

    pub fn get_incoming_file(&mut self) -> Option<Hash40> {
        self.incoming_file.take()
    }

    pub fn sub_remaining_bytes(&mut self, size: usize) -> Option<Hash40> {
        if size >= self.remaining_bytes {
            self.get_incoming_file()
        } else {
            self.remaining_bytes -= size;
            None
        }
    }

    pub fn load_file_into<H: Into<Hash40>, B: AsMut<[u8]>>(&self, hash: H, mut buffer: B) -> Result<usize, ModpackError> {
        let hash = hash.into();
        let data = std::fs::read(FILESYSTEM.get().unwrap().get(&hash).unwrap())?;
        // let data = self.load(hash)?;
        buffer.as_mut().write_all(&data)?;
        Ok(data.len())
    }

    pub fn load<H: Into<Hash40>>(&self, hash: H) -> Result<Vec<u8>, ModpackError> {
        let hash = hash.into();
        let path = self.get_physical_path(hash).unwrap();
        println!("Path: {}", path);
        Ok(std::fs::read(path).unwrap())
    }
}

/// The user's set of mods presented in a way that makes referencing easy.
/// Ultimately this should only be used for files physically present, so no API stuff.
#[derive(Default)]
pub struct Modpack {
    pub mods: Vec<ModDir>,
    // files: HashMap<Hash40, InternedPath<{ discover::MAX_COMPONENT_COUNT }>>,
}

pub fn get_additional_files(files: &mut Vec<ModFile>) -> Vec<ModFile> {
    let arc = crate::resource::arc();
    files.drain_filter(|file| arc.get_file_path_index_from_hash(hash40(file.path.as_str())).is_ok() ).collect()
}

pub struct UnconflictingModpack(Modpack);
pub struct CollectedModpack(pub Modpack);

pub struct PatchedModpack(Modpack);



#[derive(Error, Debug)]
pub enum ModpackError {
    #[error("could not write file to the buffer")]
    IoError(#[from] std::io::Error),
    #[error("failed to find the file {} in the filesystem", hashes::find(*.0))]
    FileMissing(Hash40),
}

impl Modpack {
    
}

#[derive(Default, Clone, Hash, PartialEq, Eq)]
pub struct ModDir {
    pub root: Utf8PathBuf,
    pub files: Vec<ModFile>,
}

impl ModDir {
    pub fn new<P: Into<Utf8PathBuf>>(root: P) -> Self {
        Self {
            root: root.into(),
            files: Vec::new(),
        }
    }
    pub fn get_patch(&self) -> Vec<(Hash40, u64)> {
        self.files.iter().map(|file| (file.hash, file.size)).collect()
    }

    pub fn get_filesystem(&self) -> HashMap<Hash40, Utf8PathBuf> {
        self.files.iter().map(|file| (file.hash, file.path.to_owned())).collect()
    }
}

pub fn check_for_conflicts(mut modpack:  Modpack) -> (UnconflictingModpack, ConflictManager) {
    // let mut conflict_set: HashSet<ModDir> = HashSet::new();

    // let conflict_discovery = std::time::Instant::now();

    // let conflicts: Vec<ConflictV2> = modpack.mods
    //     .iter()
    //     .enumerate()
    //     .flat_map(|(i, curr_dir)| {
    //         let curr_files: AHashSet<&ModFile> = curr_dir.files.iter().collect();

    //         // Check for conflict
    //         modpack.mods
    //         .iter()
    //         .enumerate()
    //         .filter_map(|(j, dir)| (i != j).then(|| dir)) // Make sure we don't process the current directory
    //         .filter(|dir| dir.files.iter().any(|file| curr_files.contains(&file))) // Only keep the directories that are conflicting 
    //         .map(|conflict| {
    //             conflict_set.insert(curr_dir.clone());
    //             conflict_set.insert(conflict.clone());
    //             ConflictV2::new(curr_dir.clone(), conflict.clone())
    //         })
    //     }).collect();

    //     println!("Conflict discovery took {}s", conflict_discovery.elapsed().as_secs_f32());

    //     let conflict_removal = std::time::Instant::now();

    //     // Remove all of the mods that are conflicting from the Modpack
    //     if !conflict_set.is_empty() {
    //         modpack.mods.drain_filter(|mods| {
    //             conflict_set.contains(&mods)
    //             // conflicts.iter().any(|conflict| conflict.first == *mods || conflict.second == *mods)
    //         });
    //     }

    //     println!("Conflict removal took {}s", conflict_removal.elapsed().as_secs_f32());


        (UnconflictingModpack(modpack), ConflictManager(Vec::new()))
}

/// Utility method to know if a path shouldn't be checked for conflicts
pub fn is_collectable(x: &Utf8Path) -> bool {
    match x.file_name() {
        Some(name) => {
            static RESERVED_NAMES: &[&str] = &["config.json", "plugin.nro"];

            static PATCH_EXTENSIONS: &[&str] = &["prcx", "prcxml", "stdatx", "stdatxml", "stprmx", "stprmxml", "xmsbt"];

            RESERVED_NAMES.contains(&name) || PATCH_EXTENSIONS.iter().any(|x| name.ends_with(x))
        },
        _ => false,
    }
}

pub fn collect_files(mut modpack: UnconflictingModpack) -> (CollectedModpack, Vec<ModFile>) {
    let collected: Vec<ModFile> = modpack.0.mods.iter_mut().flat_map(|dirs| dirs.files.drain_filter(|file|is_collectable(&file.path)) ).collect();
    (CollectedModpack(modpack.0), collected)
}

pub fn patch_sizes(modpack: CollectedModpack) -> PatchedModpack {
    let arc = crate::resource::arc_mut();
    let region = crate::config::region();

    let data: Vec<(Hash40, u64)> = modpack.0.mods.iter().flat_map(|mods| mods.get_patch()).collect();

    for (hash, size) in data {

        let decomp_size = match arc.get_file_data_from_hash(hash, region) {
            Ok(data) => {
                //println!("Patched {:#x} with size {:#x}", hash.as_u64(), size);
                data.decomp_size as usize
            },
            Err(_) => {
                //warn!("Failed to patch {:#x} filesize! It should be {:#x}.", hash.as_u64(), size.green());
                continue;
            },
        };

        if size as usize > decomp_size {
            if let Ok(old_size) = arc.patch_filedata(hash, size as u32, region) {
                info!("File {:#x} has a new decompressed filesize! {:#x} -> {:#x}", hash.as_u64(), old_size.red(), size.green());
            }
        }
    }

    PatchedModpack(modpack.0)
}

pub fn acquire_filesystem(modpack: PatchedModpack) -> HashMap<Hash40, Utf8PathBuf> {
    modpack.0.mods.iter().flat_map(|mods| mods.get_filesystem()).collect()
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, Serialize)]
pub struct ModFile {
    pub hash: Hash40,
    pub path: Utf8PathBuf,
    pub size: u64,
}

impl Default for ModFile {
    fn default() -> Self {
        Self { 
            hash: Hash40(0),
            path: Default::default(),
            size: Default::default()
        }
    }
}

impl ModFile {
    pub fn hash40(&self) -> Hash40 {
        // Need a method that checks for stream and all
        hash40(self.path.as_str())
    }
}

#[derive(Debug, Default, PartialEq, Hash, Eq, Serialize)]
pub struct Conflict {
    #[serde(rename = "Conflicting mod")]
    conflicting_mod: Utf8PathBuf,
    #[serde(rename = "Conflicting with")]
    conflict_with: Utf8PathBuf,
}

pub struct ConflictV2 {
    pub first: ModDir,
    pub second: ModDir,
}

impl ConflictV2 {
    pub fn new(first: ModDir, second: ModDir) -> Self {
        Self { first, second }
    }
}

impl From<Vec<ConflictV2> > for ConflictManager {
    fn from(vec: Vec<ConflictV2>) -> Self {
        Self(vec)
    }
}

pub struct ConflictManager(Vec<ConflictV2>);

impl ConflictManager {
    pub fn rebase(&mut self, dir: &ModDir) {
        self.0.drain_filter(|conflict| conflict.first == *dir || conflict.second == *dir);
    }

    pub fn next(&mut self) -> Option<ConflictV2> {
        self.0.pop()
    }
}

// enum ModFileSource {
//     Api,
//     Mod,
//     Cache,
// }

// impl ModFileSource {
//     pub fn get_file(&self) -> Vec<u8> {
//         Vec::new()
//     }
// }

// // Adding placeholder functions here until the backend for it is written
// pub fn get_modded_file(path: &Path) -> Vec<u8> {
//     // Acquire from source
//     let source = acquire_source(path);

//     // Check if this source allows for patching, as Cached files should already be patched by now
//     if can_be_patched(&source) {
//         match acquire_extension_handler(Path::new("xmsbt")) {
//             Some(_) => todo!(),
//             None => todo!(),
//         }
//         source.get_file()
//     } else {
//         source.get_file()
//     }
// }

// pub fn acquire_source(path: &Path) -> ModFileSource {
//     // Placeholder
//     ModFileSource::Mod
// }

// pub fn can_be_patched(source: &ModFileSource) -> bool {
//     match source {
//         ModFileSource::Cache => false,
//         _ => true
//     }
// }
