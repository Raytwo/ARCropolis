use std::{collections::HashMap, fs, io, path::PathBuf, vec};

use crate::{config::CONFIG, fs::Metadata, runtime, visit::{ModFile, Modpack, Modpath}};

use owo_colors::OwoColorize;

use smash_arc::{ArcLookup, FileData, FileDataFlags, FileInfoIndiceIdx, Hash40, HashToIndex};

use runtime::{LoadedArcEx, LoadedTables};

use log::warn;

use walkdir::WalkDir;

type ArcCallback = extern "C" fn(Hash40, *mut skyline::libc::c_void, usize) -> bool;

use binread::*;
use crate::cache;

lazy_static::lazy_static! {
    pub static ref MOD_FILES: parking_lot::RwLock<ModFiles> = parking_lot::RwLock::new(ModFiles(HashMap::new()));

    // For ResInflateThread
    pub static ref INCOMING_IDX: parking_lot::RwLock<Option<FileIndex>> = parking_lot::RwLock::new(None);

    // For unsharing the files :)
    pub static ref UNSHARE_LUT: parking_lot::RwLock<Option<cache::UnshareCache>> = parking_lot::RwLock::new(None);
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
        WalkDir::new(dir).into_iter().filter_entry(|entry| {
            // If it starts with a period
            !entry.file_name().to_str().unwrap().starts_with('.')
        }).filter_map(|entry| {
            let entry = entry.unwrap();

            // Only process files
            if entry.file_type().is_file() {
                // Make sure the file has an extension
                if entry.path().extension().is_some() {
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

                        // Check if a file is regional.
                        if file_info.flags.is_regional() {
                            // Check if the file has a regional indicator
                            let region = match modfile.get_region() {
                                Some(region) => {
                                    region
                                }
                                // No regional indicator, use the system's region as default (Why? Because by this point, it isn't storing the game's region yet)
                                None => user_region,
                            };
        
                            // Check if the Region of a file matches with the game's. If not, discard it.
                            if region != user_region {
                                return None;
                            }
                        }

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

#[repr(transparent)]
pub struct ModFileMap(pub HashMap<Hash40, PathBuf>);

impl ModFileMap {
    pub fn new() -> Self {
        let mut instance = Self(HashMap::new());
        let config = CONFIG.read();

        let _ = instance.visit_dir(
            &PathBuf::from(&config.paths.arc),
            config.paths.arc.to_str().unwrap().len()
        );

        let _ = instance.visit_umm_dirs(&PathBuf::from(&config.paths.umm));

        if let Some(extra_paths) = &config.paths.extra_paths {
            for path in extra_paths {
                let _ = instance.visit_umm_dirs(&PathBuf::from(path));
            }
        }

        instance
    }

    fn visit_umm_dirs(&mut self, dir: &PathBuf) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;

            if entry.file_name().to_str().unwrap().starts_with('.') {
                continue;
            }

            let path = PathBuf::from(&format!("{}/{}", dir.display(), entry.path().display()));

            if path.is_dir() {
                self.visit_dir(&path, path.to_str().unwrap().len())?;
            }
        }

        Ok(())
    }

    fn visit_dir(&mut self, dir: &PathBuf, arc_dir_len: usize) -> io::Result<()> {
        fs::read_dir(dir)?
            .map(|entry| {
                let entry = entry?;
                let path = PathBuf::from(&format!("{}/{}", dir.display(), entry.path().display()));

                if entry.file_type().unwrap().is_dir() {
                    if path.extension().is_some() {
                        let game_path = Modpath::from(PathBuf::from(
                            &path.to_str().unwrap()[arc_dir_len + 1..]
                        ));
                        self.0.insert(game_path.hash40().unwrap(), path.into());
                        return Ok(())
                    }
                    self.visit_dir(&path, arc_dir_len).unwrap();
                } else {
                    let game_path = Modpath::from(PathBuf::from(
                        &path.to_str().unwrap()[arc_dir_len + 1..]
                    ));
                    self.0.insert(game_path.hash40().unwrap(), path.into());
                    return Ok(())
                }
                Ok(())
            }).collect()
    }

    pub fn unshare(&self) -> Result<(), smash_arc::LookupError> {
        lazy_static::lazy_static! {
            static ref UNSHARE_WHITELIST: Vec<Hash40> = vec![
                Hash40::from("fighter")
            ];
        }
        let arc = LoadedTables::get_arc();
        let mut to_unshare = Vec::new();
        let file_paths = arc.get_file_paths();
        let dir_infos = arc.get_dir_infos();
        let read_cache = UNSHARE_LUT.read();
        let cache = read_cache.as_ref().unwrap();
        for (game_path, path) in self.0.iter() {
            let path_idx = match arc.get_file_path_index_from_hash(*game_path) {
                Ok(index) => index,
                Err(_) => {
                    warn!("[ARC::Unsharing] Unable to get path index for '{}' ({:#x})", path.display().bright_yellow(), game_path.0.red());
                    continue;
                }
            };
            let mut hti = smash_arc::HashToIndex::default();
            hti.set_hash((game_path.0 & 0xFFFFFFFF) as u32);
            hti.set_length((game_path.0 >> 32) as u8);
            hti.set_index(path_idx.0);
            let dir_entry = match cache.entries.get(&hti) {
                Some((dir_entry, _)) => dir_entry,
                None => {
                    panic!("[ARC::Unsharing] Lookup table file does not contain entry for '{}' ({:#x})", path.display().bright_yellow(), game_path.0.red());
                }
            };
            let tl_parent = get_top_level_parent(dir_entry.hash40());
            if UNSHARE_WHITELIST.contains(&tl_parent) {
                to_unshare.push(dir_entry.hash40());
            }
        }
        to_unshare.sort();
        to_unshare.dedup();
        LoadedTables::unshare_mass_loading_groups(&to_unshare).unwrap();
        return Ok(());

        fn get_top_level_parent(path: Hash40) -> Hash40 {
            let arc = LoadedTables::get_arc();
            let dir_infos = arc.get_dir_infos();
            let mut dir_info = arc.get_dir_info_from_hash(path).unwrap();
            while dir_info.parent.hash40().0 != 0 {
                dir_info = arc.get_dir_info_from_hash(dir_info.parent.hash40()).unwrap();
            }
            dir_info.name.hash40()
        }
    }

    pub fn to_mod_files(self) -> ModFiles {
        let mut mod_files = ModFiles(HashMap::new());

        for (arc_hash, path) in self.0.iter() {
            match Self::visit_file(*arc_hash, path) {
                Ok((index, file_ctx)) => {
                    mod_files.0.insert(index, file_ctx);
                },
                Err(err) => {
                    warn!("{}", err);
                }
            }
        }

        mod_files
    }

    fn visit_file(game_path: Hash40, full_path: &PathBuf) -> Result<(FileIndex, FileCtx), String> {
        // Skip any file starting with a period, to avoid any error related to path.extension()
        if full_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with('.')
        {
            return Err(format!(
                "[ARC::Discovery] File '{}' starts with a period, skipping",
                full_path.display().bright_yellow()
            ));
        }

        // Make sure the file has an extension to not cause issues with the code that follows
        match full_path.extension() {
            Some(_) => {
                //file_ctx.extension = Hash40::from(ext.to_str().unwrap());
            }
            None => {
                return Err(format!(
                    "[ARC::Discovery] File '{}' does not have an extension, skipping",
                    full_path.display().bright_yellow()
                ))
            }
        }
        
        let mut file_ctx = FileCtx::new();

        file_ctx.file = ModFile::from(full_path);
        file_ctx.hash = game_path;

        let user_region = smash_arc::Region::from(
            get_region_id(CONFIG.read().misc.region.as_ref().unwrap()).unwrap() + 1,
        );

        if file_ctx.file.is_stream() {
            //STREAM_FILES.write().0.insert(file_ctx.hash, file_ctx.clone());
            warn!(
                "[Arc::Discovery] File '{}' placed in the STREAM table",
                file_ctx.file.path().display().bright_yellow()
            );
            Ok((FileIndex::Stream(file_ctx.hash), file_ctx))
        }
        else {
            let arc = LoadedTables::get_arc_mut();

            match arc.get_file_path_index_from_hash(file_ctx.hash) {
                Ok(index) => {
                    let file_info = *arc.get_file_info_from_path_index(index);

                    // Check if a file is regional.
                    if file_info.flags.is_regional() {
                        // Check if the file has a regional indicator
                        let region = match file_ctx.file.get_region() {
                            Some(region) => region,
                            // No regional indicator, use the system's region as default (Why? Because by this point, it isn't storing the game's region yet)
                            None => user_region,
                        };

                        // Check if the Region of a file matches with the game's. If not, discard it.
                        if region != user_region {
                            return Err("File's region does not match".to_string());
                        }
                    }

                    file_ctx.index = file_info.file_info_indice_index;

                    file_ctx.orig_subfile = arc.patch_filedata(&file_info, file_ctx.file.len());

                    Ok((FileIndex::Regular(file_ctx.index), file_ctx))
                }
                Err(_) => Err(format!(
                    "[ARC::Patching] File '{}' was not found in data.arc",
                    full_path.display().bright_yellow()
                )),
            }
        }
    }
}