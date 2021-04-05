use std::{collections::HashMap, fs, path::{Path, PathBuf}, vec};

use crate::{
    runtime,
    config::CONFIG,
    fs::visit::ModPath,
    callbacks::Callback,
};

use smash_arc::{ArcLookup, FileInfoIndiceIdx, Hash40, HashToIndex};

use runtime::{LoadedArcEx, LoadedTables};

use log::warn;
use owo_colors::OwoColorize;

use crate::cache;

lazy_static::lazy_static! {
    pub static ref MOD_FILES: parking_lot::RwLock<ModFiles> = parking_lot::RwLock::new(ModFiles::new());

    // For ResInflateThread
    pub static ref INCOMING_IDX: parking_lot::RwLock<Option<FileIndex>> = parking_lot::RwLock::new(None);

    // For unsharing the files :)
    pub static ref UNSHARE_LUT: parking_lot::RwLock<Option<cache::UnshareCache>> = parking_lot::RwLock::new(None);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileIndex {
    Regular(FileInfoIndiceIdx),
    Stream(Hash40),
}

#[repr(transparent)]
pub struct ModFiles(pub HashMap<FileIndex, FileCtx>);

#[derive(Debug, Clone)]
pub struct FileCtx {
    pub file: FileBacking,
    pub hash: Hash40,
    pub orig_size: u32,
    pub index: FileInfoIndiceIdx,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileBacking {
    LoadFromArc,
    Path(ModPath),
    Callback {
        callback: Callback,
        original: Box<FileBacking>,
    }
}

#[macro_export]
macro_rules! get_from_file_info_indice_index {
    ($index:expr) => {
        parking_lot::RwLockReadGuard::try_map($crate::replacement_files::MOD_FILES.read(), |x| {
            x.get(FileIndex::Regular($index))
        })
    };
}

impl ModFiles {
    fn new() -> Self {
        let config = CONFIG.read();

        let mut modfiles: HashMap<Hash40, ModPath> = HashMap::new();

        // ARC mods
        if config.paths.arc.exists() {
            modfiles.extend(crate::fs::visit::discovery(&config.paths.arc));
        }
        // UMM mods
        if config.paths.umm.exists() {
            modfiles.extend(crate::fs::visit::umm_discovery(&config.paths.umm));
        }

        if let Some(extra_paths) = &config.paths.extra_paths {
            for path in extra_paths {
                // Extra UMM mods
                if path.exists() {
                    modfiles.extend(crate::fs::visit::umm_discovery(path));
                }
            }
        }
        
        Self::unshare(&modfiles);
        Self(ModFiles::process_mods(&modfiles))
    }

    fn process_mods(modfiles: &HashMap<Hash40, ModPath>) -> HashMap<FileIndex, FileCtx> {
        let arc = LoadedTables::get_arc_mut();

        modfiles.iter().filter_map(|(hash, modfile)| {
            let mut filectx = FileCtx::new();

            filectx.file = FileBacking::Path(modfile.clone());
            filectx.hash = *hash;

            if modfile.is_stream() {
                warn!("[ARC::Patching] File '{}' added as a Stream", filectx.path().display().bright_yellow());
                Some((FileIndex::Stream(filectx.hash), filectx))
            } else {
                match arc.get_file_path_index_from_hash(*hash) {
                    Ok(index) => {
                        let file_info = arc.get_file_info_from_path_index(index);

                        filectx.index = file_info.file_info_indice_index;

                        Some((FileIndex::Regular(filectx.index), filectx))
                    }
                    Err(_) => {
                        warn!("[ARC::Patching] File '{}' was not found in data.arc", modfile.to_smash_path().display().bright_yellow());
                        None
                    }
                }
            }
        }).collect::<HashMap<FileIndex, FileCtx>>().iter_mut().map(|(index, ctx)| {
            if let FileIndex::Regular(info_index) = index {
                let info_index = arc.get_file_info_indices()[usize::from(*info_index)].file_info_index;
                let file_info = arc.get_file_infos()[usize::from(info_index)];

                ctx.orig_size = arc.patch_filedata(&file_info, ctx.len())
            }

            (*index, ctx.clone())
        }).collect()
    }

    pub fn get(&self, file_index: FileIndex) -> Option<&FileCtx> {
        self.0.get(&file_index)
    }

    fn unshare(files: &HashMap<Hash40, ModPath>) {
        lazy_static::lazy_static! {
            static ref UNSHARE_WHITELIST: Vec<Hash40> = vec![
                Hash40::from("fighter")
            ];
        }

        let arc = LoadedTables::get_arc();
        let mut to_unshare = Vec::new();
        let read_cache = UNSHARE_LUT.read();
        let cache = read_cache.as_ref().unwrap();
        for (game_path, mod_file) in files.iter() {
            let path_idx = match arc.get_file_path_index_from_hash(*game_path) {
                Ok(index) => index,
                Err(_) => {
                    warn!("[ARC::Unsharing] Unable to get path index for '{}' ({:#x})", mod_file.as_path().display().bright_yellow(), game_path.0.red());
                    continue;
                }
            };
            let mut index = HashToIndex::default();
            index.set_hash((game_path.0 & 0xFFFF_FFFF) as u32);
            index.set_length((game_path.0 >> 32) as u8);
            index.set_index(path_idx.0);
            let dir_entry = match cache.entries.get(&index) {
                Some((dir_entry, _)) => dir_entry,
                None => {
                    panic!("Lookup table file does not contain entry for '{}' ({:#x})", mod_file.as_path().display(), game_path.0);
                }
            };
            let top_level = get_top_level_parent(dir_entry.hash40());
            if UNSHARE_WHITELIST.contains(&top_level) {
                to_unshare.push(dir_entry.hash40());
            }
        }
        to_unshare.sort();
        to_unshare.dedup();
        LoadedTables::unshare_mass_loading_groups(&to_unshare).unwrap();

        fn get_top_level_parent(path: Hash40) -> Hash40 {
            let arc = LoadedTables::get_arc();
            let mut dir_info = arc.get_dir_info_from_hash(path).unwrap();
            while dir_info.parent.hash40() != Hash40(0) {
                dir_info = arc.get_dir_info_from_hash(dir_info.parent.hash40()).unwrap();
            }
            dir_info.name.hash40()
        }
    }
}

pub fn get_region_id(region: &str) -> Option<u32> {
    REGIONS.iter().position(|x| x == &region).map(|x| x as u32)
}

impl FileCtx {
    pub fn new() -> Self {
        FileCtx {
            file: FileBacking::Path(ModPath::new()),
            hash: Hash40(0),
            orig_size: 0,
            index: FileInfoIndiceIdx(0),
        }
    }

    // #[allow(dead_code)]
    // pub fn metadata(&self) -> Result<Metadata, String> {
    //     crate::fs::metadata(self.hash)
    // }

    pub fn extension(&self) -> Hash40 {
        let arc = LoadedTables::get_arc();
        let path_idx = arc.get_file_path_index_from_hash(self.hash).unwrap();
        let file_path = &arc.get_file_paths()[usize::from(path_idx)];
        file_path.ext.hash40()
    }

    pub fn len(&self) -> u32 {
        match &self.file {
            FileBacking::Path(modpath) => modpath.len() as u32,
            FileBacking::LoadFromArc => unimplemented!(),
            FileBacking::Callback { callback, original } => unimplemented!(),
        }
    }

    pub fn path(&self) -> &Path {
        match &self.file {
            FileBacking::Path(modpath) => modpath,
            FileBacking::LoadFromArc => unimplemented!(),
            FileBacking::Callback { callback, original } => unimplemented!(),
        }
    }

    pub fn get_file_content(&self) -> Vec<u8> {
        // TODO: Add error handling in case the user deleted the file while running and reboot Smash if they did. But maybe this requires extract checks because of callbacks?
        match &self.file {
            FileBacking::Path(modpath) => fs::read(modpath).unwrap(),
            FileBacking::LoadFromArc => unimplemented!(),
            FileBacking::Callback { callback, original } => unimplemented!(),
        }
    }
}
