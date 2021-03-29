use std::{collections::HashMap, fs, io, path::PathBuf};

use crate::{
    config::CONFIG,
    fs::Metadata,
    runtime,
    visit::{ModFile, Modpath},
};

use owo_colors::OwoColorize;

use smash_arc::{ArcLookup, FileData, FileDataFlags, FileInfoIndiceIdx, Hash40};

use runtime::{LoadedArcEx, LoadedTables};

use log::warn;

type ArcCallback = extern "C" fn(Hash40, *mut skyline::libc::c_void, usize) -> bool;

lazy_static::lazy_static! {
    pub static ref MOD_FILES: parking_lot::RwLock<ModFiles> = parking_lot::RwLock::new(ModFiles(HashMap::new()));

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
        let mut instance = Self(HashMap::new());

        let config = CONFIG.read();

        let _ = instance.visit_dir(
            &PathBuf::from(&config.paths.arc),
            config.paths.arc.to_str().unwrap().len(),
        );
        let _ = instance.visit_umm_dirs(&PathBuf::from(&config.paths.umm));

        if let Some(extra_paths) = &config.paths.extra_paths {
            for path in extra_paths {
                let _ = instance.visit_umm_dirs(&PathBuf::from(path));
            }
        }

        instance
    }

    /// Visit Ultimate Mod Manager directories for backwards compatibility
    fn visit_umm_dirs(&mut self, dir: &PathBuf) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;

            // Skip any directory starting with a period
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

                // Check if the entry is a directory or a file
                if entry.file_type().unwrap().is_dir() {
                    // If it is one of the stream randomizer directories
                    if path.extension().is_some() {
                        match self.visit_file(&path, arc_dir_len) {
                            Ok((index, file_ctx)) => {
                                self.0.insert(index, file_ctx);
                                return Ok(());
                            }
                            Err(err) => {
                                warn!("{}", err);
                                return Ok(());
                            }
                        }
                    }

                    // If not, treat it as a regular directory
                    self.visit_dir(&path, arc_dir_len).unwrap();
                } else {
                    match self.visit_file(&path, arc_dir_len) {
                        Ok((index, context)) => {
                            if self.0.get_mut(&index).is_none() {
                                self.0.insert(index as _, context);
                            }
                            return Ok(());
                        }
                        Err(err) => {
                            warn!("{}", err);
                            return Ok(());
                        }
                    }
                }

                Ok(())
            })
            .collect()
    }

    fn visit_file(
        &self,
        full_path: &PathBuf,
        arc_dir_len: usize,
    ) -> Result<(FileIndex, FileCtx), String> {
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

        let game_path = Modpath::from(PathBuf::from(
            &full_path.to_str().unwrap()[arc_dir_len + 1..],
        ));
        let mut file_ctx = FileCtx::new();

        file_ctx.file = ModFile::from(full_path);
        file_ctx.hash = game_path.hash40().unwrap();

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
        let arc = LoadedTables::get_arc();
        let mut parent_to_group = HashMap::new();
        let file_paths = arc.get_file_paths();
        for (game_path, path) in self.0.iter() {
            let path_idx = match arc.get_file_path_index_from_hash(*game_path) {
                Ok(index) => index,
                Err(_) => {
                    warn!("[ARC::Unsharing] Unable to get path index for '{}' ({:#x})", path.display().bright_yellow(), game_path.0.red());
                    continue;
                }
            };
            let parent = file_paths[usize::from(path_idx)].parent.hash40();
            if !parent_to_group.contains_key(&parent) {
                let group_hash = arc.get_mass_load_group_hash_from_file_hash(*game_path)?;
                if !arc.is_unshareable_group(group_hash) {
                    parent_to_group.insert(parent, Hash40(0));
                    warn!("[ARC::Unsharing] Unable to unshare directory containing {} ({:#x}).", crate::hashes::get(*game_path).unwrap_or(&format!("{:#x}", game_path.0).as_str()).bright_yellow(), game_path.0.red());
                } else {
                    parent_to_group.insert(parent, group_hash);
                }
            }
        }
        let mut to_unshare: Vec<Hash40> = parent_to_group.into_iter().filter_map(|(_, hash)| if hash == Hash40(0) { None } else { Some(hash) }).collect();
        to_unshare.sort();
        to_unshare.dedup();
        LoadedTables::unshare_mass_loading_groups(&to_unshare).unwrap();
        Ok(())
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