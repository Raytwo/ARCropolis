use std::{
    fs,
    path::PathBuf,
    collections::HashMap,
};

use crate::{
    runtime,
    visit::Mod,
    config::CONFIG,
};

use owo_colors::OwoColorize;

use smash_arc::{ArcLookup, FileData, FileInfo, FileInfoIndiceIdx, Hash40};

use runtime::{ LoadedTables, ResServiceState };

use log::{ info, warn };

type ArcCallback = extern "C" fn(Hash40, *mut skyline::libc::c_void, usize) -> bool;

lazy_static::lazy_static! {
    pub static ref ARC_FILES: parking_lot::RwLock<ArcFiles> = parking_lot::RwLock::new(ArcFiles::new());

    // For ResInflateThread
    pub static ref INCOMING: parking_lot::RwLock<Option<FileIndex>> = parking_lot::RwLock::new(None);
}

#[no_mangle]
pub extern "C" fn subscribe_callback(_hash: Hash40, _extension: *const u8, _extension_len: usize, _callback: ArcCallback) {
    // Deprecated
    warn!("{}", "Another plugin is trying to reach ARCropolis, but this API is deprecated.".red());
}

#[no_mangle]
pub extern "C" fn subscribe_callback_with_size(_hash: Hash40, _filesize: u32, _extension: *const u8, _extension_len: usize, _callback: ArcCallback) {
    // Deprecated
    warn!("{}", "Another plugin is trying to reach ARCropolis, but this API is deprecated.".red());
}

const REGIONS: &[&str] = &[
    "jp_ja",
    "us_en",
    "us_fr",
    "us_es",
    "eu_en",
    "eu_fr",
    "eu_es",
    "eu_de",
    "eu_nl",
    "eu_it",
    "eu_ru",
    "kr_ko",
    "zh_cn",
    "zh_tw",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileIndex {
    Regular(FileInfoIndiceIdx),
    Stream(Hash40),
}

// Table2Index
pub struct ArcFiles(pub HashMap<FileIndex, FileCtx>);

pub struct StreamFiles(pub HashMap<Hash40, FileCtx>);

#[derive(Debug, Clone)]
pub struct FileCtx {
    pub path: PathBuf,
    pub hash: Hash40,
    pub filesize: u32,
    pub extension: Hash40,
    pub virtual_file: bool,
    pub orig_subfile: smash_arc::FileData,
    pub index: FileInfoIndiceIdx,
}

#[macro_export]
macro_rules! get_from_file_info_indice_index {
    ($index:expr) => {
        parking_lot::RwLockReadGuard::try_map(
            $crate::replacement_files::ARC_FILES.read(),
            |x| x.get(FileIndex::Regular($index))
        )
    };
}

impl StreamFiles {
    fn new() -> Self {
        Self(HashMap::new())
    }
}

impl ArcFiles {
    fn new() -> Self {
        let mut instance = Self(HashMap::new());

        let config = CONFIG.read();

        // TODO: Move this elsewhere
        
        let mut mods: Vec<Mod> = vec![];

        // TODO: Build a cache using the timestamp of every Mod directory to confirm if something changed. If not, load everything and fill the tables without running a discovery

        if config.paths.arc.exists() {
            mods.push(crate::visit::discover(&config.paths.arc));
        }

        if config.paths.umm.exists() {
            mods.append(&mut crate::visit::umm_directories(&config.paths.umm));
        }

        if let Some(extra_paths) = &config.paths.extra_paths {
            for path in extra_paths {
                if path.exists() {
                    mods.append(&mut crate::visit::umm_directories(&path));
                }
            }
        }

        // TODO: Read the info.toml for every Mod instance if it exists, store the priority and then sort the vector

        let resource = ResServiceState::get_instance();
        let arc = LoadedTables::get_instance().get_arc_mut();

        let contexts: Vec<(FileIndex, FileCtx)> = mods.iter().map(|test| {
            let base_path = test.path.to_owned();

            let contexts: Vec<(FileIndex, FileCtx)> = test.mods.iter().filter_map(|modpath| {
                match modpath.is_stream() {
                    true => {
                        let mut filectx = FileCtx::new();

                        let mut full_path = base_path.to_owned();
                        full_path.push(&modpath.path);

                        filectx.path = full_path;
                        filectx.hash = modpath.hash40().unwrap();
                        filectx.extension = Hash40::from(modpath.path.extension().unwrap().to_str().unwrap());
                        filectx.filesize = modpath.size as u32;

                        warn!("[ARC::Patching] File '{}' added as a Stream", filectx.path.display().bright_yellow());
                        Some((FileIndex::Stream(filectx.hash), filectx))
                    }
                    false => {
                        // Does the file exist in the FilePath table? If not, discard it.
                        let file_index = match arc.get_file_path_index_from_hash(modpath.hash40().unwrap()) {
                            Ok(index) => index,
                            Err(_) => {
                                warn!("[ARC::Patching] File '{}' was not found in data.arc", modpath.as_smash_path().display().bright_yellow());
                                return None;
                            },
                        };
        
                        let file_info = arc.get_file_info_from_path_index(file_index);
        
                        // Check if a file is regional.
                        if file_info.flags.is_regional() {
                            // Check if the file has a regional indicator
                            let region = match modpath.get_region() {
                                Some(region) => region,
                                // No regional indicator, use the system's region as default (Why? Because by this point, it isn't storing the game's region yet)
                                None => smash_arc::Region::from(resource.game_region_idx +1),
                            };
        
                            // Check if the Region of a file matches with the game's. If not, discard it.
                            if region != smash_arc::Region::from(resource.game_region_idx + 1) {
                                return None;
                            }
                        }
        
                        // Use a FileCtx until the system is fully reworked
                        let mut filectx = FileCtx::new();
                        
                        let mut full_path = base_path.to_owned();
                        full_path.push(&modpath.path);
        
                        filectx.path = full_path;
                        filectx.hash = modpath.hash40().unwrap();
                        filectx.extension = Hash40::from(modpath.path.extension().unwrap().to_str().unwrap());
                        filectx.index = file_info.file_info_indice_index;
                        filectx.filesize = modpath.size as u32;
        
                        // TODO: Move this in the for loop below
                        filectx.filesize_replacement();
                        
                        Some((FileIndex::Regular(filectx.index), filectx))
                    }
                }
            }).collect();
            
            contexts
        }).flatten().collect();

        for (index, context) in contexts {
            // TODO: If a file shares a FileInfoIndices index we already have, discard it.
            instance.0.entry(index).or_insert(context);
        }

        instance
    }

    pub fn get(&self, file_index: FileIndex) -> Option<&FileCtx> {
        self.0.get(&file_index)
    }
}

pub fn get_region_id(region: &str) -> Option<u32> {
    REGIONS
        .iter()
        .position(|x| x == &region)
        .map(|x| x as u32)
}

impl FileCtx {
    pub fn new() -> Self {
        FileCtx {
            path: PathBuf::new(),
            hash: Hash40(0),
            filesize: 0,
            virtual_file: false,
            extension: Hash40(0),
            orig_subfile: smash_arc::FileData {
                offset_in_folder: 0,
                comp_size: 0,
                decomp_size: 0,
                flags: smash_arc::FileDataFlags::new()
                .with_compressed(false)
                .with_use_zstd(false)
                .with_unk(0),
            },
            index: FileInfoIndiceIdx(0),
        }
    }

    pub fn get_region(&self) -> u32 {
        // Default to the player's region index
        let mut region_index = get_region_id(&CONFIG.read().misc.region.as_ref().unwrap()).unwrap_or_else(|| ResServiceState::get_instance().game_region_idx);

        // Make sure the file has an extension
        if let Some(_) = self.path.extension() {
            // Split the region identifier from the filepath
            let region = self.path.file_name().unwrap().to_str().unwrap().to_string();
            // Check if the filepath it contains a + symbol
            if let Some(region_marker) = region.find('+') {
                region_index = get_region_id(&region[region_marker + 1..region_marker + 6]).unwrap_or(1);
            }
        }

        region_index
    }

    // TODO: Should probably replace this, considering the new findings related to shared files
    // Refer to "filesize_replacement"
    pub fn get_subfile(&self) -> &mut FileData {
        let loaded_arc = LoadedTables::get_instance().get_arc_mut();
        let file_info = *loaded_arc.get_file_info_from_hash(self.hash).unwrap();
        loaded_arc.get_file_data_mut(&file_info.to_owned(), smash_arc::Region::from(self.get_region() + 1))
    }

    pub fn get_file_content(&self) -> Vec<u8> {
        // TODO: Add error handling in case the user deleted the file while running and reboot Smash if they did. But maybe this requires extract checks because of callbacks?
        fs::read(&self.path).unwrap()
    }

    pub fn filesize_replacement(&mut self) {
        let loaded_tables = LoadedTables::get_instance();
        let arc = loaded_tables.get_arc_mut();

        // Backup the Subfile for when file watching is added
        self.orig_subfile = self.get_subfile().clone();

        let file_path_index = arc.get_file_path_index_from_hash(self.hash).unwrap();
        let file_path = arc.get_file_paths()[usize::from(file_path_index)];

        let t2_indexes: Vec<FileInfo> = arc.get_file_infos()
                .iter()
                .filter_map(|entry| {
                    if entry.file_info_indice_index == FileInfoIndiceIdx(file_path.path.index()) {
                        Some(*entry)
                    } else {
                        None
                    }
                }).collect();

        t2_indexes.iter().for_each(|info| {
            let mut subfile = arc.get_file_data_mut(info, smash_arc::Region::from(self.get_region() + 1));

            if subfile.decomp_size < self.filesize { 
                subfile.decomp_size = self.filesize;
                info!("[ARC::Patching] File '{}' has a new patched decompressed size: {:#x}",self.path.display().bright_yellow(),subfile.decomp_size.bright_red());
            }
        });
    }
}
