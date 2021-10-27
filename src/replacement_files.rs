use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
};

use crate::{callbacks::CallbackKind, config::{CONFIG, REGION}, fs::{DiscoveryResults, ModFile, RejectionReason}, runtime};

use smash_arc::{ArcLookup, FileDataIdx, FileInfoIndiceIdx, Hash40};

use runtime::{LoadedArcEx, LoadedTables};

use log::{debug, warn};
use owo_colors::OwoColorize;

// use crate::cache;

lazy_static::lazy_static! {
    pub static ref MOD_FILES: parking_lot::RwLock<ModFiles> = parking_lot::RwLock::new(ModFiles::new());

    // For ResInflateThread
    pub static ref INCOMING_LOAD: parking_lot::RwLock<IncomingLoad> = parking_lot::RwLock::new(IncomingLoad::None);

    // For... Callbacks.
    pub static ref CALLBACKS: parking_lot::RwLock<HashMap<Hash40, CallbackKind>> = parking_lot::RwLock::new(HashMap::new());

    // For unsharing the files :)
    // pub static ref UNSHARE_LUT: parking_lot::RwLock<Option<cache::UnshareCache>> = parking_lot::RwLock::new(None);
}

const REGIONS: &[&str] = &[
    "jp_ja", "us_en", "us_fr", "us_es", "eu_en", "eu_fr", "eu_es", "eu_de", "eu_nl", "eu_it",
    "eu_ru", "kr_ko", "zh_cn", "zh_tw",
];

pub enum IncomingLoad {
    Index(FileIndex),
    ExtCallback(/* ext */ Hash40, FileInfoIndiceIdx),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileIndex {
    Regular(FileInfoIndiceIdx),
    Stream(Hash40),
}

// FileIndex -> ModdedFile, FileIndex -> Vanilla file size
// pub struct ModFiles(pub HashMap<FileIndex, FileCtx>, pub Vec<(FileDataIdx, u32)>);
pub struct ModFiles {
    pub modded_files: HashMap<FileIndex, FileCtx>,
    pub backup_sizes: Vec<(FileDataIdx, u32)>
}
#[derive(Clone)]
pub struct FileCtx {
    pub file: FileBacking,
    pub hash: Hash40,
    pub index: FileInfoIndiceIdx,
}

#[derive(Clone)]
pub enum FileBacking {
    LoadFromArc,
    ModFile(ModFile),
    Callback {
        callback: CallbackKind,
        original: Box<FileBacking>,
    },
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
    pub fn reinitialize(&mut self) {
        let arc = LoadedTables::get_arc_mut();
        let datas = arc.get_file_datas_mut();
        for (idx, size) in self.backup_sizes.iter() {
            datas[*idx].decomp_size = *size;
        }
        let config = CONFIG.read();

        let mut discover_results = DiscoveryResults {
            accepted: HashMap::new(),
            rejected: Vec::new(),
            stream: HashMap::new()
        };

        // ARC mods
        if config.paths.arc.exists() {
            crate::fs::visit::discovery(arc, &config.paths.arc, &mut discover_results);
        }
        // UMM mods
        if config.paths.umm.exists() {
            crate::fs::visit::umm_discovery(arc, &config.paths.umm, &mut discover_results);
        }

        if let Some(extra_paths) = &config.paths.extra_paths {
            for path in extra_paths {
                // Extra UMM mods
                if path.exists() {
                    crate::fs::visit::umm_discovery(arc, path, &mut discover_results);
                }
            }
        }

        let DiscoveryResults { accepted, rejected, stream } = discover_results;

        let rejected_exts = crate::api::REJECTED_EXT_CALLBACKS.read();
        for (path, reason) in rejected.into_iter() {
            if let crate::fs::RejectionReason::NotFound(smash_path) = reason {
                let extension_hash = Hash40::from(path
                    .extension()
                    .unwrap()
                    .to_str()
                    .unwrap()
                );
                let filepath_slice = path.to_str().unwrap().as_bytes();
                let arc_path_slice = smash_path.to_str().unwrap().as_bytes();
                if let Some(callbacks) = rejected_exts.get(&extension_hash) {
                    for cb in callbacks.iter() {
                        cb(
                            filepath_slice.as_ptr(),
                            filepath_slice.len(),
                            arc_path_slice.as_ptr(),
                            arc_path_slice.len()
                        );
                    }
                } else {
                    warn!(
                        "[ARC::Discovery] File '{}' rejected. Reason: Not found in data.arc",
                        path.display().bright_yellow()
                    );
                }
            } else {
                match reason {
                    RejectionReason::DuplicateFile(file) => {
                        warn!(
                            "[ARC::Discovery] File '{}' rejected. Reason: File already replaced by '{}'",
                            path.display().bright_yellow(),
                            file.display().bright_yellow()
                        );
                    },
                    RejectionReason::MissingExtension => {
                        warn!(
                            "[ARC::Discovery] File '{}' rejected. Reason: Missing extension",
                            path.display().bright_yellow()
                        );
                    },
                    _ => {}
                }
            }
        }

        //Self::unshare(&modfiles);
        let (modded_files, original_sizes) = ModFiles::process_mods(&accepted, &stream);
        self.modded_files = modded_files;
        self.backup_sizes = original_sizes;
    }

    fn new() -> Self {
        let config = CONFIG.read();

        let arc = LoadedTables::get_arc();

        let mut discover_results = DiscoveryResults {
            accepted: HashMap::new(),
            rejected: Vec::new(),
            stream: HashMap::new()
        };

        // ARC mods
        if config.paths.arc.exists() {
            crate::fs::visit::discovery(arc, &config.paths.arc, &mut discover_results);
        } else {
            std::fs::create_dir_all("sd:/atmosphere/contents/01006A800016E000/romfs/arc");
        }
        // UMM mods
        if config.paths.umm.exists() {
            crate::fs::visit::umm_discovery(arc, &config.paths.umm, &mut discover_results);
        } else {
            std::fs::create_dir_all(&config.paths.umm);

        }

        if let Some(extra_paths) = &config.paths.extra_paths {
            for path in extra_paths {
                // Extra UMM mods
                if path.exists() {
                    crate::fs::visit::umm_discovery(arc, path, &mut discover_results);
                }
            }
        }

        let DiscoveryResults { accepted, rejected, stream } = discover_results;

        let rejected_exts = crate::api::REJECTED_EXT_CALLBACKS.read();
        for (path, reason) in rejected.into_iter() {
            if let crate::fs::RejectionReason::NotFound(smash_path) = reason {
                let extension_hash = Hash40::from(path
                    .extension()
                    .unwrap()
                    .to_str()
                    .unwrap()
                );
                let filepath_slice = path.to_str().unwrap().as_bytes();
                let arc_path_slice = smash_path.to_str().unwrap().as_bytes();
                if let Some(callbacks) = rejected_exts.get(&extension_hash) {
                    for cb in callbacks.iter() {
                        cb(
                            filepath_slice.as_ptr(),
                            filepath_slice.len(),
                            arc_path_slice.as_ptr(),
                            arc_path_slice.len()
                        );
                    }
                } else {
                    warn!(
                        "[ARC::Discovery] File '{}' rejected. Reason: Not found in data.arc",
                        path.display().bright_yellow()
                    );
                }
            } else {
                match reason {
                    RejectionReason::DuplicateFile(file) => {
                        warn!(
                            "[ARC::Discovery] File '{}' rejected. Reason: File already replaced by '{}'",
                            path.display().bright_yellow(),
                            file.display().bright_yellow()
                        );
                    },
                    RejectionReason::MissingExtension => {
                        warn!(
                            "[ARC::Discovery] File '{}' rejected. Reason: Missing extension",
                            path.display().bright_yellow()
                        );
                    },
                    _ => {}
                }
            }
        }

        //Self::unshare(&modfiles);
        let (modded_files, backup_sizes) = ModFiles::process_mods(&accepted, &stream);
        Self {
            modded_files,
            backup_sizes
        }
    }

    fn process_mods(modfiles: &HashMap<Hash40, ModFile>, stream_files: &HashMap<Hash40, ModFile>) -> (HashMap<FileIndex, FileCtx>, Vec<(FileDataIdx, u32)>) {
        let arc = LoadedTables::get_arc_mut();

        let mut to_unshare = crate::unsharing::TO_UNSHARE_ON_DISCOVERY.lock();
        let mut mods = modfiles
            .iter()
            .filter_map(|(hash, modpath)| {
                let mut filectx = FileCtx::new();

                filectx.file = FileBacking::ModFile(modpath.clone());
                filectx.hash = *hash;
                if let Some((dir_index, file_info_idx)) = to_unshare.remove(hash) {
                    crate::unsharing::unshare_file(dir_index, file_info_idx);
                }
                match arc.get_file_path_index_from_hash(*hash) {
                    Ok(index) => {
                        let file_info = arc.get_file_info_from_path_index(index);

                        filectx.index = file_info.file_info_indice_index;

                        Some((FileIndex::Regular(filectx.index), filectx))
                    }
                    Err(_) => {
                        warn!(
                            "[ARC::Patching] File '{}' was not found in data.arc",
                            modpath.to_smash_path().display().bright_yellow()
                        );
                        None
                    }
                }
            })
            .collect::<HashMap<FileIndex, FileCtx>>();

        for (hash, modpath) in stream_files.iter() {
            let mut filectx = FileCtx::new();
            filectx.file = FileBacking::ModFile(modpath.clone());
            filectx.hash = *hash;
            mods.insert(FileIndex::Stream(*hash), filectx);
        }

        // Process callbacks here?
        let callbacks = CALLBACKS.read();

        // This is disgusting please help

        callbacks.iter().for_each(|(hash, callback)| {
            // Get what kind of callback this is
            match callback {
                // Regular file callback
                CallbackKind::Regular(callback) => {
                    // Check if the file exists in data.arc
                    match arc.get_file_path_index_from_hash(*hash) {
                        // This hash exists in the regular files
                        Ok(path_index) => {
                            let info = arc
                                .get_file_info_from_path_index(path_index)
                                .file_info_indice_index;

                            // Check if we already have a FileCtx for it
                            match mods.get_mut(&FileIndex::Regular(info)) {
                                // A file on the SD or another callback is present
                                Some(filectx) => {
                                    // Update the FileCtx
                                    let new_callback = callback;

                                    let new_backing =
                                        if let FileBacking::Callback { callback, original: _ } =
                                            &*callback.previous
                                        {
                                            FileBacking::Callback {
                                                callback: CallbackKind::Regular(
                                                    new_callback.clone(),
                                                ),
                                                original: Box::new(FileBacking::Callback {
                                                    callback: callback.clone(),
                                                    original: Box::new(filectx.file.clone()),
                                                }),
                                            }
                                        } else {
                                            FileBacking::Callback {
                                                callback: CallbackKind::Regular(
                                                    new_callback.clone(),
                                                ),
                                                original: Box::new(filectx.file.clone()),
                                            }
                                        };

                                    filectx.file = new_backing;
                                }
                                // Doesn't exist on the SD
                                None => {
                                    // Create a FileCtx for it
                                    let new_callback = callback;

                                    let mut filectx = FileCtx::new();
                                    filectx.hash = *hash;
                                    filectx.index = info;

                                    filectx.file = FileBacking::Callback {
                                        callback: CallbackKind::Regular(new_callback.clone()),
                                        original: Box::new(FileBacking::LoadFromArc),
                                    };

                                    mods.insert(FileIndex::Regular(info), filectx);
                                }
                            }
                        }
                        // The file does not exist in data.arc, but this should be implement when file addition is a thing.
                        Err(_) => debug!(
                            "A callback registered for a hash that does not exist ({:#x})",
                            hash.as_u64()
                        ),
                    }
                }
                // Stream file callback
                CallbackKind::Stream(callback) => {
                    // Check if we already have a FileCtx for it
                    match mods.get_mut(&FileIndex::Stream(*hash)) {
                        // A file on the SD or another callback is present
                        Some(filectx) => {
                            // Update the FileCtx
                            let new_callback = callback;

                            let new_backing = if let FileBacking::Callback { callback, original: _ } =
                                &*callback.previous
                            {
                                FileBacking::Callback {
                                    callback: CallbackKind::Stream(new_callback.clone()),
                                    original: Box::new(FileBacking::Callback {
                                        callback: callback.clone(),
                                        original: Box::new(filectx.file.clone()),
                                    }),
                                }
                            } else {
                                FileBacking::Callback {
                                    callback: CallbackKind::Stream(new_callback.clone()),
                                    original: Box::new(filectx.file.clone()),
                                }
                            };

                            filectx.file = new_backing;
                        }
                        // Doesn't exist on the SD
                        None => {
                            // Create a FileCtx for it
                            let new_callback = callback;

                            let mut filectx = FileCtx::new();
                            filectx.hash = *hash;

                            filectx.file = FileBacking::Callback {
                                callback: CallbackKind::Stream(new_callback.clone()),
                                original: Box::new(FileBacking::LoadFromArc),
                            };

                            mods.insert(FileIndex::Stream(*hash), filectx);
                        }
                    }
                }
            }
        });

        let mut original_sizes = Vec::new();

        let modded_files = mods.iter_mut()
            .map(|(index, ctx)| {
                if let FileIndex::Regular(info_index) = index {
                    let info_index = arc.get_file_info_indices()[*info_index].file_info_index;
                    let file_info = arc.get_file_infos()[info_index];

                    let orig_decomp_size = arc.patch_filedata(&file_info, ctx.len());
                    original_sizes.push((arc.get_file_in_folder(&file_info, *REGION).file_data_index, orig_decomp_size));
                }

                (*index, ctx.clone())
            })
            .collect();

        (modded_files, original_sizes)
    }

    pub fn get(&self, file_index: FileIndex) -> Option<&FileCtx> {
        self.modded_files.get(&file_index)
    }

    // fn unshare(files: &HashMap<Hash40, ModPath>) {
    //     lazy_static::lazy_static! {
    //         static ref UNSHARE_WHITELIST: Vec<Hash40> = vec![
    //             Hash40::from("fighter")
    //         ];
    //     }

    //     let arc = LoadedTables::get_arc();
    //     let mut to_unshare = Vec::new();
    //     let read_cache = UNSHARE_LUT.read();
    //     let cache = read_cache.as_ref().unwrap();
    //     for (game_path, mod_file) in files.iter() {
    //         let path_idx = match arc.get_file_path_index_from_hash(*game_path) {
    //             Ok(index) => index,
    //             Err(_) => {
    //                 warn!("[ARC::Unsharing] Unable to get path index for '{}' ({:#x})", mod_file.as_path().display().bright_yellow(), game_path.0.red());
    //                 continue;
    //             }
    //         };
    //         let mut index = HashToIndex::default();
    //         index.set_hash((game_path.0 & 0xFFFF_FFFF) as u32);
    //         index.set_length((game_path.0 >> 32) as u8);
    //         index.set_index(path_idx.0);
    //         let dir_entry = match cache.entries.get(&index) {
    //             Some((dir_entry, _)) => dir_entry,
    //             None => {
    //                 panic!("Lookup table file does not contain entry for '{}' ({:#x})", mod_file.as_path().display(), game_path.0);
    //             }
    //         };
    //         let top_level = get_top_level_parent(dir_entry.hash40());
    //         if UNSHARE_WHITELIST.contains(&top_level) {
    //             to_unshare.push(dir_entry.hash40());
    //         }
    //     }
    //     to_unshare.sort();
    //     to_unshare.dedup();
    //     LoadedTables::unshare_mass_loading_groups(&to_unshare).unwrap();

    //     fn get_top_level_parent(path: Hash40) -> Hash40 {
    //         let arc = LoadedTables::get_arc();
    //         let mut dir_info = arc.get_dir_info_from_hash(path).unwrap();
    //         while dir_info.parent.hash40() != Hash40(0) {
    //             dir_info = arc.get_dir_info_from_hash(dir_info.parent.hash40()).unwrap();
    //         }
    //         dir_info.name.hash40()
    //     }
    // }
}

pub fn get_region_id(region: &str) -> Option<u32> {
    REGIONS.iter().position(|x| x == &region).map(|x| x as u32)
}

impl FileCtx {
    pub fn new() -> Self {
        FileCtx {
            file: FileBacking::LoadFromArc,
            hash: Hash40(0),
            index: FileInfoIndiceIdx(0),
        }
    }

    pub fn extension(&self) -> Hash40 {
        match &self.file {
            FileBacking::ModFile(modpath) => modpath.extension(),
            _ => {
                let arc = LoadedTables::get_arc();
                let path_idx = arc.get_file_path_index_from_hash(self.hash).unwrap();
                let file_path = &arc.get_file_paths()[path_idx];

                file_path.ext.hash40()
            }
        }
    }

    pub fn len(&self) -> u32 {
        match &self.file {
            FileBacking::ModFile(modpath) => modpath.size as u32,
            FileBacking::LoadFromArc => {
                let user_region = *REGION;

                let arc = LoadedTables::get_arc();
                // Careful, this could backfire once the size is patched
                arc.get_file_data_from_hash(self.hash, user_region)
                    .unwrap()
                    .decomp_size
            }
            FileBacking::Callback { callback, original: _ } => {
                if let CallbackKind::Regular(cb) = callback {
                    cb.len
                } else {
                    0
                }
            }
        }
    }

    // TODO: Eventually make this a Option<&Path> instead? Or just stop logging filepaths
    pub fn path(&self) -> Option<PathBuf> {
        match &self.file {
            FileBacking::ModFile(modpath) => Some(modpath.to_path_buf()),
            // lol, lmao
            FileBacking::LoadFromArc => None,
            FileBacking::Callback {
                callback: _,
                original: _,
            } => Some(PathBuf::from("Callback")),
        }
    }

    pub fn get_file_content(&self) -> Vec<u8> {
        recursive_file_backing_load(self.hash, &self.file)
    }
}

pub fn recursive_file_backing_load(hash: Hash40, backing: &FileBacking) -> Vec<u8> {
    match backing {
        // TODO: Add error handling in case the user deleted the file while running and reboot Smash if they did. But maybe this requires extract checks because of callbacks?
        FileBacking::ModFile(modpath) => fs::read(modpath.full_path()).unwrap(),
        FileBacking::LoadFromArc => {
            let user_region = *REGION;

            let arc = LoadedTables::get_arc();
            arc.get_file_contents(hash, user_region).unwrap()
        }
        FileBacking::Callback { callback, original } => {
            if let CallbackKind::Regular(callback) = callback {
                let cb = callback.callback_fn;

                // Prepare a buffer with the patched size
                let mut buffer = Vec::with_capacity(callback.len as _);
                unsafe { buffer.set_len(callback.len as usize) };

                let mut out_size: usize = 0;

                if cb(
                    hash.as_u64(),
                    buffer.as_mut_ptr(),
                    callback.len as usize,
                    &mut out_size,
                ) {
                    println!("Callback returned size: {:#x}", out_size);
                    buffer[0..out_size].to_vec()
                } else {
                    recursive_file_backing_load(hash, &**original)
                }
            } else {
                // If this is a CallbackKind::Stream, just defer to the next FileBacking in line (SD/Arc)
                recursive_file_backing_load(hash, &**original)
            }
        }
    }
}
