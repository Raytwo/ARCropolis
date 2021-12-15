use orbits::Tree;
use orbits::{
    orbit::LaunchPad, ConflictHandler, ConflictKind, DiscoverSystem, FileEntryType, FileLoader,
    Orbit, StandardLoader,
};
use skyline::nn::{self, ro::{Module, RegistrationInfo}};
use smash_arc::serde::Hash40String;
use smash_arc::{ArcLookup, Hash40, LoadedArc, LookupError, SearchLookup, LoadedSearchSection};
use owo_colors::OwoColorize;

use crate::chainloader::{NroBuilder, NrrBuilder, NrrRegistrationFailedError};
use crate::replacement::{self, LoadedArcEx, SearchEx};
use crate::{PathExtension, config, hashes, resource};
use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    ops::Deref,
    path::Path
};
use thiserror::Error;

use std::fmt;

pub mod api;

pub mod loaders;
pub use loaders::*;

static DEFAULT_CONFIG: &'static str = include_str!("../resources/override.json");
static IS_INIT: AtomicBool = AtomicBool::new(false);
// pub type ApiLoader = StandardLoader; // temporary until an actual ApiLoader is implemented


pub type ArcropolisOrbit = Orbit<ArcLoader, StandardLoader, ApiLoader>;

pub struct FilesystemUninitializedError;

impl fmt::Debug for FilesystemUninitializedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Filesystem is uninitialized!")
    }
}

pub struct CachedFilesystem {
    loader: ArcropolisOrbit,
    config: replacement::config::ModConfig,
    hash_lookup: HashMap<Hash40, PathBuf>,
    hash_size_cache: HashMap<Hash40, usize>,
    incoming_load: Option<Hash40>,
    bytes_remaining: usize,
    current_nus3bank_id: u32,
    nus3banks: HashMap<Hash40, u32>
}

pub enum GlobalFilesystem {
    Uninitialized,
    Promised(std::thread::JoinHandle<LaunchPad<StandardLoader, ApiLoader>>),
    Initialized(CachedFilesystem),
}

impl GlobalFilesystem {
    pub fn finish(self, arc: &'static LoadedArc) -> Result<Self, FilesystemUninitializedError> {
        match self {
            Self::Uninitialized => Err(FilesystemUninitializedError),
            Self::Promised(promise) => match promise.join() {
                Ok(mut launchpad) => {
                    let mut hashed_paths = HashMap::new();
                    let mut hashed_sizes = HashMap::new();
                    launchpad.patch.tree.walk_paths(|node, entry_type| {
                        if entry_type.is_file() {
                            if let Ok(hash) = crate::get_smash_hash(node.get_local()) {
                                let full_path = node.full_path();
                                match std::fs::metadata(&full_path) {
                                    Ok(md) => {
                                        let _ = hashed_sizes.insert(hash, md.len() as usize);
                                        let _ = hashed_paths.insert(hash, node.get_local().to_path_buf());
                                    },
                                    Err(_) => {
                                        error!("Failed to stat file '{}' ({:#x}) -- this file will not be replaced.", hashes::find(hash), hash.0);
                                    }
                                }
                            } else {
                                error!("Failed to generate smash hash for path '{}' -- this file will not be replaced.", node.full_path().display());
                            }
                        }

                        // Add all hashes from our file discovery to the global hashes, so that added files also get logged correctly when they are loaded
                        if let Some(path) = node.get_local().as_os_str().to_str() { 
                            hashes::add(path);
                        }
                    });
                    let mut config = match serde_json::from_str(DEFAULT_CONFIG) {
                        Ok(cfg) => cfg,
                        Err(_) => {
                            error!("Failed to deserialize the default config.");
                            replacement::config::ModConfig::new()
                        }
                    };
                    for (root, local) in launchpad.patch.collected.iter() {
                        let full_path = root.join(local);
                        if !full_path.exists() && full_path.ends_with("config.json") {
                            warn!("Mod config at '{}' does not exist.", full_path.display());
                            continue;
                        }
                        match std::fs::read_to_string(&full_path) {
                            Ok(contents) => match serde_json::from_str(contents.as_str()) {
                                Ok(mod_cfg) => {
                                    config.merge(mod_cfg);
                                    info!("Merged config '{}' into main config.", full_path.display());
                                },
                                Err(e) => warn!("Failed to deserialize mod config at '{}'. Reason: {:?}", full_path.display(), e)
                            },
                            Err(e) => warn!("Failed to read mod config at '{}'. Reason: {:?}", full_path.display(), e)
                        }
                    }
                    let mut nus3banks = HashSet::new();
                    let mut nus3audio_requires = HashSet::new();
                    launchpad.patch.tree.walk_paths(|node, ty| {
                        if ty.is_file() {
                            let local = node.get_local();
                            if !local.is_stream() && local.has_extension("nus3audio") {
                                nus3audio_requires.insert(local.with_extension("nus3bank"));
                            }
                            if !local.is_stream() && local.has_extension("nus3bank") {
                                nus3banks.insert(local.to_path_buf());
                            }
                        }
                    });
                    for bank in nus3banks {
                        nus3audio_requires.remove(&bank);
                    }
                    let nus3bank_patch_root = Path::new("api:/patch-nus3bank");
                    for required in nus3audio_requires {
                        launchpad.virt.tree.insert_file(nus3bank_patch_root, &required);
                        if let Ok(hash) = crate::get_smash_hash(&required) {
                            launchpad.virt.tree.loader.push_entry(hash, nus3bank_patch_root, ApiCallback::None);
                            hashed_paths.insert(hash, required);
                            hashed_sizes.insert(hash, 0); // use vanilla size regardless
                        }
                    }
                    let mut pending_requests = api::PENDING_API_CALLS.lock();
                    let mut tmp = Vec::new();
                    std::mem::swap(&mut *pending_requests, &mut tmp);
                    for call in tmp.into_iter() {
                        match call {
                            api::PendingApiCall::GenericCallback {
                                hash,
                                max_size,
                                callback
                            } => {
                                if let Some(known_path) = hashed_paths.get(&hash) {
                                    launchpad.virt.tree.insert_file("api:/generic-cb", known_path);
                                } else {
                                    let new_local_path = PathBuf::from(hashes::try_find(hash).map_or(format!("{:#x}", hash.0), |x| x.to_string()));
                                    launchpad.virt.tree.insert_file("api:/generic-cb", &new_local_path);
                                    hashed_paths.insert(hash, new_local_path);
                                }
                                launchpad.virt.tree.loader.push_entry(hash, Path::new("api:/generic-cb"), loaders::ApiCallback::GenericCallback(callback));
                                if let Some(current_size) = hashed_sizes.get_mut(&hash) {
                                    *current_size = (*current_size).max(max_size);
                                } else {
                                    hashed_sizes.insert(hash, max_size);
                                }
                            },
                            api::PendingApiCall::StreamCallback {
                                hash,
                                callback
                            } => {
                                if let Some(known_path) = hashed_paths.get(&hash) {
                                    launchpad.virt.tree.insert_file("api:/stream-cb", known_path);
                                } else {
                                    let new_local_path = PathBuf::from(hashes::try_find(hash).map_or(format!("{:#x}", hash.0), |x| x.to_string()));
                                    launchpad.virt.tree.insert_file("api:/stream-cb", &new_local_path);
                                    hashed_paths.insert(hash, new_local_path);
                                }
                                launchpad.virt.tree.loader.push_entry(hash, Path::new("api:/stream-cb"), loaders::ApiCallback::StreamCallback(callback));
                            },
                            api::PendingApiCall::ExtensionCallback {
                                ext,
                                callback
                            } => {
                                let local_path = format!("{}~", ext);
                                let hash = crate::get_smash_hash(&local_path).expect("Failed to get local hash for ext callback!");
                                if hashed_paths.get(&hash).is_none() {
                                    hashed_paths.insert(hash, PathBuf::from(&local_path));
                                }
                                launchpad.virt.tree.insert_file("api:/extension-cb", local_path);
                                launchpad.virt.tree.loader.push_entry(hash, Path::new("api:/generic-cb"), loaders::ApiCallback::ExtCallback(callback));
                            }
                        }
                    }
                    IS_INIT.store(true, Ordering::SeqCst);
                    Ok(Self::Initialized(CachedFilesystem {
                        loader: launchpad.launch(ArcLoader(arc)),
                        config,
                        hash_lookup: hashed_paths,
                        hash_size_cache: hashed_sizes,
                        incoming_load: None,
                        bytes_remaining: 0,
                        current_nus3bank_id: 7420,
                        nus3banks: HashMap::new()
                    }))
                },
                Err(_) => Err(FilesystemUninitializedError),
            },
            Self::Initialized(filesystem) => Ok(Self::Initialized(filesystem)),
        }
    }

    pub fn is_init() -> bool {
        IS_INIT.load(Ordering::SeqCst)
    }

    pub fn take(&mut self) -> Self {
        let mut out = GlobalFilesystem::Uninitialized;
        std::mem::swap(self, &mut out);
        out
    }

    pub fn get(&self) -> &ArcropolisOrbit {
        match self {
            Self::Initialized(fs) => &fs.loader,
            _ => panic!("Global Filesystem is not initialized!")
        }
    }

    pub fn get_mut(&mut self) -> &mut ArcropolisOrbit {
        match self {
            Self::Initialized(fs) => &mut fs.loader,
            _ => panic!("Global Filesystem is not initialized!")
        }
    }

    pub fn hash(&self, hash: Hash40) -> Option<PathBuf> {
        match self {
            Self::Initialized(fs) => {
                self
                    .local_hash(hash)
                    .map_or(None, |x| fs.loader.query_actual_path(x))
            },
            _ => None
        }
    }

    pub fn local_hash(&self, hash: Hash40) -> Option<&PathBuf> {
        match self {
            Self::Initialized(fs) => fs.hash_lookup.get(&hash),
            _ => None
        }
    }

    pub fn load_into(&self, hash: Hash40, mut buffer: &mut [u8]) -> Option<usize> {
        if let Some(data) = self.load(hash) {
            if buffer.len() < data.len() {
                error!("The size of the file data is larger than the size of the provided buffer when loading file '{}' ({:#x}).", hashes::find(hash), hash.0);
                None
            } else {
                buffer.write_all(&data).unwrap();
                Some(data.len())
            }
        } else {
            None
        }
    }

    pub fn load(&self, hash: Hash40) -> Option<Vec<u8>> {
        match self {
            Self::Initialized(fs) => {
                if let Some(path) = fs.hash_lookup.get(&hash) {
                    match fs.loader.load(path) {
                        Ok(data) => Some(data),
                        Err(orbits::Error::Virtual(ApiLoaderError::NoVirtFile)) if fs.loader.get_patch_entry_type(path).is_ok() => match fs.loader.load_patch(path) {
                            Ok(data) => Some(data),
                            Err(e) => {
                                error!("Failed to load data for '{}' ({:#x}). Reason: {:?}", path.display(), hash.0, e);
                                None
                            }
                        }
                        Err(orbits::Error::Virtual(ApiLoaderError::NoVirtFile)) => match ArcLoader(resource::arc()).load_path(Path::new(""), path) {
                            Ok(data) => Some(data),
                            Err(e) => {
                                error!("Failed to load data for '{}' ({:#x}). Reason: {:?}", path.display(), hash.0, e);
                                None
                            }
                        }
                        Err(e) => {
                            error!("Failed to load data for '{}' ({:#x}). Reason: {:?}", path.display(), hash.0, e);
                            None
                        }
                    }
                } else {
                    error!("Failed to load data for '{}' ({:#x}) because the filesystem does not contain it!", hashes::find(hash), hash.0);
                    None
                }
            },
            _ => {
                error!("Cannot load data for '{}' ({:#x}) because the filesystem is not initialized!", hashes::find(hash), hash.0);
                None
            }
        }
    }

    pub fn set_incoming(&mut self, hash: Option<Hash40>) {
        match self {
            Self::Initialized(fs) => {
                if let Some(hash) = fs.incoming_load.take() {
                    warn!("Removing file '{}' ({:#x}) from incoming load before using it.", hashes::find(hash), hash.0);
                }
                fs.incoming_load = hash;
                if let Some(hash) = hash {
                    fs.bytes_remaining = *fs.hash_size_cache.get(&hash).unwrap_or(&0);
                } else {
                    fs.bytes_remaining = 0;
                }
            },
            _ if let Some(hash) = hash => error!("Cannot set the incoming load to '{}' ({:#x}) because the filesystem is not initialized!", hashes::find(hash), hash.0),
            _ => error!("Cannot null out the incoming load because the filesystem is not initialized!")
        }
    }

    pub fn sub_remaining_byes(&mut self, count: usize) -> Option<Hash40> {
        match self {
            Self::Initialized(fs) => {
                if count >= fs.bytes_remaining {
                    fs.bytes_remaining = 0;
                    self.get_incoming()
                } else {
                    fs.bytes_remaining -= count;
                    None
                }
            },
            _ => {
                error!("Cannot subtract reamining bytes because the filesystem is not initialized!");
                None
            }
        }
    }

    pub fn get_incoming(&mut self) -> Option<Hash40> {
        match self {
            Self::Initialized(fs) => fs.incoming_load.take(),
            _ => {
                error!("Cannot get the incoming load because the filesystem is not initialized!");
                None
            }
        }
    }

    pub fn patch_sizes(&mut self, arc: &'static mut LoadedArc) {
        match self {
            Self::Initialized(fs) => {
                let region = config::region();
                for (hash, size) in fs.hash_size_cache.iter_mut() {
                    let hash = *hash;
                    let decomp_size = match arc.get_file_data_from_hash(hash, region) {
                        Ok(data) => data.decomp_size as usize,
                        Err(_) => {
                            warn!(
                                "Failed to patch '{}' ({:#x}) filesize! It should be {:#x}.",
                                hashes::find(hash).bright_yellow(),
                                hash.0,
                                size.green()
                            );
                            continue;
                        }
                    };
                    if *size > decomp_size {
                        match arc.patch_filedata(hash, *size as u32, region) {
                            Ok(old_size) => {
                                info!(
                                    "File '{}' ({:#x}) has a new decompressed filesize! {:#x} -> {:#x}",
                                    hashes::find(hash).bright_yellow(),
                                    hash.0,
                                    old_size.red(),
                                    size.green()
                                );
                                *size = decomp_size;
                            },
                            Err(_) => {}
                        }
                    }
                }
            },
            _ => {
                error!("Cannot patch sizes because the filesystem is not initialized!");
            }
        }
    }

    pub fn share_hashes(&mut self, arc: &'static LoadedArc) {
        match self {
            Self::Initialized(fs) => {
                let file_paths = arc.get_file_paths();
                let mut old_map = HashMap::new();
                std::mem::swap(&mut fs.hash_lookup, &mut old_map);
                fs.hash_lookup = old_map.into_iter().map(|(hash, path)| {
                    (
                        arc.get_file_info_from_hash(hash).map_or_else(|_| hash, |info| file_paths[info.file_path_index].path.hash40()),
                        path
                    )
                }).collect();
            },
            _ => {
                error!("Cannot share the hashes because the filesystem is not initialized!");
            }
        }
    }

    pub fn process_file_manipulation(&mut self, arc: &'static mut LoadedArc, search: &'static mut LoadedSearchSection) {
        match self {
            Self::Initialized(fs) => {
                let mut context = LoadedArc::make_addition_context();
                let mut search_context = LoadedSearchSection::make_context();
                let mut hash_ignore = HashSet::new();
                for (dep, source) in fs.config.preprocess_reshare.iter() {
                    hash_ignore.extend(replacement::preprocess::reshare_contained_files(&mut context, dep.0, source.0).into_iter());
                }
                fs.loader.walk_patch(|node, entry_type| {
                    match node.get_local().smash_hash() {
                        Ok(hash) => {
                            if entry_type.is_file() && !context.contains_file(hash) {
                                replacement::addition::add_file(&mut context, node.get_local());
                                replacement::addition::add_searchable_file_recursive(&mut search_context, node.get_local());
                            } 
                        }
                        _ => {}
                    }
                });
                replacement::unshare::reshare_file_groups(&mut context);
                replacement::unshare::unshare_files(&mut context, hash_ignore, fs.hash_lookup.iter().filter_map(|(hash, _)| {
                    if !fs.config.unshare_blacklist.contains(&Hash40String(*hash)) {
                        Some(*hash)
                    } else {
                        None
                    }
                }));
                for (hash, files) in fs.config.new_dir_files.iter() {
                    replacement::addition::add_files_to_directory(&mut context, hash.0, files.iter().map(|x| x.0).collect());
                }
                arc.take_context(context);
                search.take_context(search_context);
            },
            _ => {
                error!("Cannot unshare files because the filesystem is not initialized!");
            }
        }
    }

    pub fn get_config(&self) -> &replacement::config::ModConfig {
        match self {
            Self::Initialized(fs) => &fs.config,
            _ => panic!("Global Filesystem is not initialized!")
        }   
    }

    pub fn get_bank_id(&mut self, hash: Hash40) -> Option<u32> {
        match self {
            Self::Initialized(fs) => {
                if let Some(id) = fs.nus3banks.get(&hash) {
                    Some(*id)
                } else {
                    let id = fs.current_nus3bank_id;
                    fs.current_nus3bank_id += 1;
                    fs.nus3banks.insert(hash, id);
                    Some(id)
                }
            },
            _ => None
        }
    }

    pub fn handle_api_request(&mut self, _req: api::PendingApiCall) {
        error!("Currently, the API does not support adding callbacks after the file system has been initialized.");
    }
}

fn mount_prebuilt_nrr<A: FileLoader>(tree: &Tree<A>) -> Result<Option<RegistrationInfo>, NrrRegistrationFailedError>
where <A as FileLoader>::ErrorType: std::fmt::Debug {
    let fighter_nro_parent = Path::new("prebuilt;/nro/release");
    let mut fighter_nro_nrr = NrrBuilder::new();

    tree.walk_paths(|node, entry_type| {
        match node.get_local().parent() {
            Some(parent) if entry_type.is_file() && parent == fighter_nro_parent => {
                info!("Reading '{}' for module registration.", node.full_path().display());
                if let Ok(data) = std::fs::read(node.full_path()) {
                    fighter_nro_nrr.add_module(data.as_slice());
                }
            },
            _ => {}
        }
    });

    fighter_nro_nrr.register()
}

pub fn perform_discovery() -> LaunchPad<StandardLoader, ApiLoader> {
    let filter = |x: &Path| {
        match x.file_name() {
            Some(name) if let Some(name) = name.to_str() => {
                !name.starts_with(".")
            },
            _ => false
        }
    };

    let ignore = |x: &Path| {
        match x.file_name() {
            Some(name) if let Some(name) = name.to_str() => {
                let is_root_file = x.parent().is_none() || x.parent().unwrap().as_os_str().is_empty();
                let is_dot = name.starts_with(".");
                let is_out_of_region = if let Some(index) = name.find("+") {
                    let (_, end) = name.split_at(index + 1);
                    !end.starts_with(config::region_str())
                } else {
                    false
                };
                is_root_file || is_out_of_region || is_dot
            },
            _ => false
        }
    };

    let collect = |x: &Path| {
        match x.file_name() {
            Some(name) if let Some(name) = name.to_str() => {
                static RESERVED_NAMES: &[&'static str] = &[
                    "config.json",
                    "plugin.nro",
                ];
                RESERVED_NAMES.contains(&name)
            },
            _ => false
        }
    };

    let mut launchpad = LaunchPad::new(
        DiscoverSystem::new(StandardLoader, ConflictHandler::NoRoot),
        DiscoverSystem::new(ApiLoader::new(), ConflictHandler::NoRoot),
    );

    let arc_path = config::arc_path();

    launchpad.patch.collecting(collect);
    launchpad.patch.ignoring(ignore);

    let mut conflicts = if std::fs::try_exists(arc_path).unwrap_or(false) {
        launchpad.patch.discover_in_root(config::arc_path())
    } else {
        Vec::new()
    };

    let umm_path = config::umm_path();
    if std::fs::try_exists(umm_path).unwrap_or(false) {
        conflicts.extend(
            launchpad
                .patch
                .discover_roots(config::umm_path(), 1, filter),
        );
    }

    for path in config::extra_paths() {
        if std::fs::try_exists(path).unwrap_or(false) {
            conflicts.extend(launchpad.patch.discover_roots(path, 1, filter));
        }
    }

    let should_prompt = !conflicts.is_empty();

    for conflict in conflicts.into_iter() {
        match conflict {
            ConflictKind::StandardConflict { error_root, source_root, local } => warn!(
                "File '{}' was rejected for file '{}' during discovery.",
                error_root.join(&local).display(),
                source_root.join(local).display()
            ),
            ConflictKind::RootConflict(root_path, kept) => warn!(
                "Mod root '{}' was rejected for a file conflict with '{}' during discovery.",
                root_path.display(),
                kept.display()
            )
        }
    }

    if should_prompt {
        if skyline_web::Dialog::yes_no("During file discovery, ARCropolis encountered file conflicts.<br>Do you want to run it again to list all file conflicts?") {
            let mut launchpad = LaunchPad::new(
                DiscoverSystem::new(StandardLoader, ConflictHandler::First),
                DiscoverSystem::new(StandardLoader, ConflictHandler::First),
            );

            let arc_path = config::arc_path();

            launchpad.patch.collecting(collect);
            launchpad.patch.ignoring(ignore);

            let mut conflicts = if std::fs::try_exists(arc_path).unwrap_or(false) {
                launchpad.patch.discover_in_root(config::arc_path())
            } else {
                Vec::new()
            };
        
            let umm_path = config::umm_path();
            if std::fs::try_exists(umm_path).unwrap_or(false) {
                conflicts.extend(
                    launchpad
                        .patch
                        .discover_roots(config::umm_path(), 1, filter),
                );
            }
        
            for path in config::extra_paths() {
                if std::fs::try_exists(path).unwrap_or(false) {
                    conflicts.extend(launchpad.patch.discover_roots(path, 1, filter));
                }
            }

            let mut conflict_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

            for conflict in conflicts.into_iter() {
                match conflict {
                    ConflictKind::StandardConflict { error_root, local, source_root } => {
                        if let Some(conflicting_mods) = conflict_map.get_mut(&local) {
                            conflicting_mods.push(error_root);
                        } else {
                            conflict_map.insert(local, vec![source_root, error_root]);
                        }
                    },
                    _ => {}
                }
            }

            let should_log = match serde_json::to_string_pretty(&conflict_map) {
                Ok(json) => match std::fs::write("sd:/ultimate/arcropolis/conflicts.json", json.as_bytes()) {
                    Ok(_) => {
                        crate::dialog_error("Please check sd:/ultimate/arcropolis/conflicts.json for all of the file conflicts.");
                        false
                    },
                    Err(e) => {
                        crate::dialog_error(format!("Failed to write conflict map to sd:/ultimate/arcropolis/conflicts.json<br>{:?}", e));
                        true
                    }
                },
                Err(e) => {
                    crate::dialog_error(format!("Failed to serialize conflict map to JSON. {:?}", e));
                    true
                }
            };

            if should_log {
                for (local, roots) in conflict_map {
                    error!("The file {} is used by the following roots:", local.display());
                    for root in roots {
                        error!("{}", root.display());
                    }
                }
            }
        }
    }

    match mount_prebuilt_nrr(&launchpad.patch.tree) {
        Ok(Some(_)) => info!("Successfully registered fighter modules."),
        Ok(_) => info!("No fighter modules found to register."),
        Err(e) => {
            error!("{:?}", e);
            crate::dialog_error("ARCropolis failed to register module information for fighter modules.<br>You may experience infinite loading on some fighters.");
        }
    }

    load_and_run_plugins(&launchpad.patch.collected);

    launchpad
}

pub fn load_and_run_plugins(plugins: &Vec<(PathBuf, PathBuf)>) {
    let mut plugin_nrr = NrrBuilder::new();

    let modules: Vec<NroBuilder> = plugins.iter().filter_map(|(root, local)| {
        let full_path = root.join(local);

        if full_path.exists() && full_path.ends_with("plugin.nro") {
            match NroBuilder::open(&full_path) {
                Ok(builder) => {
                    info!("Loaded plugin at '{}' for chainloading.", full_path.display());
                    plugin_nrr.add_module(&builder);
                    Some(builder)
                },
                Err(e) => {
                    error!("Failed to load plugin at '{}'. {:?}", full_path.display(), e);
                    None
                }
            }
        } else {
            error!("File discovery collected path '{}' but it does not exist and/or is invalid!", full_path.display());
            None
        }
    }).collect();

    if modules.is_empty() {
        info!("No plugins found for chainloading.");
        return;
    }

    let mut registration_info = match plugin_nrr.register() {
        Ok(Some(info)) => info,
        Ok(_) => return,
        Err(e) => {
            error!("{:?}", e);
            crate::dialog_error("ARCropolis failed to register plugin module info.");
            return;
        }
    };

    let modules: Vec<Module> = modules.into_iter().filter_map(|x| {
        match x.mount() {
            Ok(module) => Some(module),
            Err(e) => {
                error!("Failed to mount chainloaded plugin. {:?}", e);
                None
            }
        }
    }).collect();

    unsafe {
        // Unfortunately, without unregistering this it will cause the game to crash, cause is unknown, but likely due to page alignment I'd guess
        // It does not matter if we use only one NRR for both the prebuilt modules and the plugins, it will still cause a crash
        nn::ro::UnregisterModuleInfo(&mut registration_info);
    }

    if modules.len() < plugins.len() {
        crate::dialog_error("ARCropolis failed to load/mount some plugins.");
    } else {
        info!("Successfully chainloaded all collected plugins.");
    }

    for module in modules.into_iter() {
        let callable = unsafe {
            let mut sym_loc = 0usize;
            let rc = nn::ro::LookupModuleSymbol(&mut sym_loc, &module, "main\0".as_ptr() as _);
            if rc != 0 {
                warn!("Failed to find symbol 'main' in chainloaded plugin.");
                None
            } else {
                Some(std::mem::transmute::<usize, extern "C" fn()>(sym_loc))
            }
        };

        if let Some(entrypoint) = callable {
            info!("Calling 'main' in chainloaded plugin"); 
            entrypoint();
            info!("Finished calling 'main' in chainloaded plugin");
        }
    }
}