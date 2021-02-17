use std::{fs, io};
use std::fs::DirEntry;
use std::path::PathBuf;
use std::collections::HashMap;

use crate::{config::CONFIG, runtime, visit::{Mod, ModPath}};

use owo_colors::OwoColorize;

use smash_arc::{ArcLookup, FileData, FileInfo, Hash40};

use runtime::{ LoadedTables, ResServiceState };

use log::{ info, warn };

type ArcCallback = extern "C" fn(Hash40, *mut skyline::libc::c_void, usize) -> bool;

lazy_static::lazy_static! {
    pub static ref ARC_FILES: parking_lot::RwLock<ArcFiles> = parking_lot::RwLock::new(ArcFiles::new());
    pub static ref STREAM_FILES: parking_lot::RwLock<StreamFiles> = parking_lot::RwLock::new(StreamFiles::new());

    // For ResInflateThread
    pub static ref INCOMING: parking_lot::RwLock<Option<u32>> = parking_lot::RwLock::new(None);
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

// Table2Index
pub struct ArcFiles(pub HashMap<u32, FileCtx>);

pub struct StreamFiles(pub HashMap<Hash40, FileCtx>);

#[derive(Debug, Clone)]
pub struct FileCtx {
    pub path: PathBuf,
    pub hash: Hash40,
    pub filesize: u32,
    pub extension: Hash40,
    pub virtual_file: bool,
    pub orig_subfile: smash_arc::FileData,
    pub index: u32,
}

// TODO: Either rename this or stop using it altogether, considering there is literally one use of it AFAIK.
#[macro_export]
macro_rules! get_from_info_index {
    ($index:expr) => {
        parking_lot::RwLockReadGuard::try_map(
            $crate::replacement_files::ARC_FILES.read(),
            |x| x.get($index)
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

        let contexts: Vec<FileCtx> = mods.iter().map(|test| {
            let base_path = test.path.to_owned();

            let contexts: Vec<FileCtx> = test.mods.iter().filter_map(|modpath| {
                // TODO: Handle this better
                // If it's a stream file, ignore everything and add it to the STREAM list for now
                if modpath.is_stream() {
                    let mut filectx = FileCtx::new();
                
                    let mut full_path = base_path.to_owned();
                    full_path.push(&modpath.path);

                    filectx.path = full_path;
                    filectx.hash = modpath.hash40().unwrap();
                    filectx.extension = Hash40::from(modpath.path.extension().unwrap().to_str().unwrap());
                    filectx.filesize = modpath.size as u32;

                    STREAM_FILES.write().0.insert(filectx.hash, filectx.clone());
                    warn!("File '{}' placed in the STREAM table", filectx.path.display().bright_yellow());
                    return None;
                }

                // Does the file exist in the FilePath table? If not, discard it.
                let file_index = match arc.get_file_path_index_from_hash(modpath.hash40().unwrap()) {
                    Ok(index) => index,
                    Err(err) => {
                        warn!("Error: {}", err);
                        warn!("File: {}", modpath.as_smash_path().display());
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
                filectx.index = file_info.hash_index_2;
                filectx.filesize = modpath.size as u32;

                // TODO: Move this in the for loop below
                filectx.filesize_replacement();
                
                Some(filectx)
            }).collect();
            
            contexts
        }).flatten().collect();

        for context in contexts {
            // TODO: If a file shares a FileInfoIndices index we already have, discard it.
            instance.0.entry(context.index).or_insert(context);
        }

        instance
    }

    pub fn get(&self, file_path_index: u32) -> Option<&FileCtx> {
        self.0.get(&file_path_index)
    }
}

pub fn get_region_id(region: &str) -> u32 {
    match region {
                "jp_ja" => 0,
                "us_en" => 1,
                "us_fr" => 2,
                "us_es" => 3,
                "eu_en" => 4,
                "eu_fr" => 5,
                "eu_es" => 6,
                "eu_de" => 7,
                "eu_nl" => 8,
                "eu_it" => 9,
                "eu_ru" => 10,
                "kr_ko" => 11,
                "zh_cn" => 12,
                "zh_tw" => 13,
                _ => ResServiceState::get_instance().game_region_idx,
            }
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
                flags: smash_arc::FileDataFlags::new().with_compressed(false).with_use_zstd(false).with_unk(0),
            },
            index: 0,
        }
    }

    pub fn get_region(&self) -> u32 {
        // Default to the player's region index
        let mut region_index = get_region_id(&CONFIG.read().misc.region.as_ref().unwrap());

        // Make sure the file has an extension
        if let Some(_) = self.path.extension() {
            // Split the region identifier from the filepath
            let region = self.path.file_name().unwrap().to_str().unwrap().to_string();
            // Check if the filepath it contains a + symbol
            if let Some(region_marker) = region.find('+') {
                region_index = match &region[region_marker + 1..region_marker + 6] {
                    "jp_ja" => 0,
                    "us_en" => 1,
                    "us_fr" => 2,
                    "us_es" => 3,
                    "eu_en" => 4,
                    "eu_fr" => 5,
                    "eu_es" => 6,
                    "eu_de" => 7,
                    "eu_nl" => 8,
                    "eu_it" => 9,
                    "eu_ru" => 10,
                    "kr_ko" => 11,
                    "zh_cn" => 12,
                    "zh_tw" => 13,
                    _ => 1,
                };
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
        // TODO: Add error handling in case the user deleted the file while running and reboot Smash if they did.
        fs::read(&self.path).unwrap()
    }

    pub fn filesize_replacement(&mut self) {
        let loaded_tables = LoadedTables::get_instance();
        let arc = loaded_tables.get_arc_mut();

        // Backup the Subfile for when file watching is added
        self.orig_subfile = self.get_subfile().clone();

        let file_path_index = arc.get_file_path_index_from_hash(self.hash).unwrap();
        let file_path = arc.get_file_paths()[file_path_index as usize];

        let t2_indexes: Vec<FileInfo> = arc.get_file_infos()
                .iter()
                .filter_map(|entry| {
                    if entry.hash_index_2 == file_path.path.index() {
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
