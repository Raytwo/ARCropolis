use std::{collections::HashMap, fs, io, path::PathBuf, vec};

use crate::{config::CONFIG, fs::Metadata, runtime, visit::{ModFile, Modpack, Modpath}};

use owo_colors::OwoColorize;

use smash_arc::{ArcLookup, FileData, FileDataFlags, FileInfoIndiceIdx, Hash40};

use runtime::{LoadedArcEx, LoadedTables};

use log::warn;

use walkdir::WalkDir;

type ArcCallback = extern "C" fn(Hash40, *mut skyline::libc::c_void, usize) -> bool;

lazy_static::lazy_static! {
    pub static ref MOD_FILES: parking_lot::RwLock<ModFiles> = parking_lot::RwLock::new(ModFiles::new());

    // For ResInflateThread
    pub static ref INCOMING_IDX: parking_lot::RwLock<Option<FileIndex>> = parking_lot::RwLock::new(None);
}

#[no_mangle]
pub extern "C" fn subscribe_callback(
    _hash: Hash40,
    _extension: *const u8,
    _extension_len: usize,
    _callback: ArcCallback,
) {
    // Deprecated
    warn!(
        "{}",
        "Another plugin is trying to reach ARCropolis, but this API is deprecated.".red()
    );
}

#[no_mangle]
pub extern "C" fn subscribe_callback_with_size(
    _hash: Hash40,
    _filesize: u32,
    _extension: *const u8,
    _extension_len: usize,
    _callback: ArcCallback,
) {
    // Deprecated
    warn!(
        "{}",
        "Another plugin is trying to reach ARCropolis, but this API is deprecated.".red()
    );
}

const REGIONS: &[&str] = &[
    "jp_ja", "us_en", "us_fr", "us_es", "eu_en", "eu_fr", "eu_es", "eu_de", "eu_nl", "eu_it",
    "eu_ru", "kr_ko", "zh_cn", "zh_tw",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileIndex {
    Regular(FileInfoIndiceIdx),
    Stream(Hash40),
}

#[repr(transparent)]
pub struct ModFiles(pub HashMap<FileIndex, FileCtx>);

#[derive(Debug, Clone)]
pub struct FileCtx {
    pub file: ModFile,
    pub hash: Hash40,
    pub orig_subfile: FileData,
    pub index: FileInfoIndiceIdx,
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

        let mut modfiles: HashMap<Hash40, ModFile> = HashMap::new();

        // ARC mods
        if config.paths.arc.exists() {
            modfiles.extend(ModFiles::discovery(&config.paths.arc));
        }
        // UMM mods
        if config.paths.umm.exists() {
            modfiles.extend(ModFiles::umm_discovery(&config.paths.umm));
        }

        if let Some(extra_paths) = &config.paths.extra_paths {
            for path in extra_paths {
                // Extra UMM mods
                if path.exists() {
                    modfiles.extend(ModFiles::umm_discovery(path));
                }
            }
        }

        Self(ModFiles::process_mods(&modfiles))
    }

    fn discovery(dir: &PathBuf) -> HashMap<Hash40, ModFile> {
        let user_region = smash_arc::Region::from(get_region_id(CONFIG.read().misc.region.as_ref().unwrap()).unwrap() + 1);

        WalkDir::new(dir).into_iter().filter_entry(|entry| {
            // If it starts with a period
            !entry.file_name().to_str().unwrap().starts_with('.')
        }).filter_map(|entry| {
            let entry = entry.unwrap();

            // Only process files
            if entry.file_type().is_file() {
                // Make sure the file has an extension
                if entry.path().extension().is_some() {
                    let path: ModFile = ModFile::from(entry.path().strip_prefix(dir).unwrap().to_path_buf());

                    match path.get_region() {
                        Some(region) => {
                            if region != user_region {
                                return None;
                            }
                        }
                        None => ()
                    }

                    let hash = Modpath(entry.path().strip_prefix(dir).unwrap().to_path_buf()).hash40().unwrap();
                    Some((hash, entry.path().to_path_buf().into()))
                } else {
                    println!("File has no extension, aborting");
                    None
                }
            } else {
                None
            }

            
        }).collect()
    }

    /// Visit Ultimate Mod Manager directories for backwards compatibility
    fn umm_discovery(dir: &PathBuf) -> HashMap<Hash40, ModFile> {
        WalkDir::new(dir).min_depth(1).max_depth(1).into_iter().filter_entry(|entry| {
            !entry.file_name().to_str().unwrap().starts_with('.')
        }).flat_map(|entry| {
            let entry = entry.unwrap();

            if !entry.file_type().is_dir() {
                return Err(());
            }

            Ok(ModFiles::discovery(&entry.into_path()))
        }).flatten().collect()
    }

    fn process_mods(modfiles: &HashMap<Hash40, ModFile>) -> HashMap<FileIndex, FileCtx> {
        let arc = LoadedTables::get_arc_mut();
        let user_region = smash_arc::Region::from(get_region_id(CONFIG.read().misc.region.as_ref().unwrap()).unwrap() + 1);

        modfiles.iter().filter_map(|(hash, modfile)| {
            let mut filectx = FileCtx::new();

            filectx.file = modfile.clone();
            filectx.hash = *hash;

            if modfile.is_stream() {
                warn!("[ARC::Patching] File '{}' added as a Stream", filectx.file.path().display().bright_yellow());
                Some((FileIndex::Stream(filectx.hash), filectx))
            } else {
                match arc.get_file_path_index_from_hash(*hash) {
                    Ok(index) => {
                        let file_info = arc.get_file_info_from_path_index(index);

                        filectx.index = file_info.file_info_indice_index;

                        Some((FileIndex::Regular(filectx.index), filectx))
                    }
                    Err(_) => {
                        warn!("[ARC::Patching] File '{}' was not found in data.arc", modfile.as_smash_path().display().bright_yellow());
                        None
                    }
                }
            }
        }).collect::<HashMap<FileIndex, FileCtx>>().iter_mut().map(|(index, ctx)| {
            match index {
                FileIndex::Regular(info_index) => {
                    let info_index = arc.get_file_info_indices()[usize::from(*info_index)].file_info_index;
                    let file_info = arc.get_file_infos()[usize::from(info_index)];

                    ctx.orig_subfile = arc.patch_filedata(&file_info, ctx.file.len())
                }
                _ => {},
            }

            (*index, ctx.clone())
        }).collect()
    }

    pub fn get(&self, file_index: FileIndex) -> Option<&FileCtx> {
        self.0.get(&file_index)
    }
}

pub fn get_region_id(region: &str) -> Option<u32> {
    REGIONS.iter().position(|x| x == &region).map(|x| x as u32)
}

impl FileCtx {
    pub fn new() -> Self {
        FileCtx {
            file: PathBuf::new().into(),
            hash: Hash40(0),
            orig_subfile: FileData {
                offset_in_folder: 0,
                comp_size: 0,
                decomp_size: 0,
                flags: FileDataFlags::new()
                    .with_compressed(false)
                    .with_use_zstd(false)
                    .with_unk(0),
            },
            index: FileInfoIndiceIdx(0),
        }
    }

    pub fn metadata(&self) -> Result<Metadata, String> {
        crate::fs::metadata(self.hash)
    }

    pub fn get_file_content(&self) -> Vec<u8> {
        // TODO: Add error handling in case the user deleted the file while running and reboot Smash if they did. But maybe this requires extract checks because of callbacks?
        fs::read(&self.file.path()).unwrap()
    }
}
