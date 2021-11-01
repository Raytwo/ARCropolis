use orbits::Tree;
use orbits::{
    orbit::LaunchPad, ConflictHandler, ConflictKind, DiscoverSystem, FileEntryType, FileLoader,
    Orbit, StandardLoader,
};
use skyline::nn::{self, ro::{Module, RegistrationInfo}};
use smash_arc::{ArcLookup, Hash40, LoadedArc, LookupError, SearchLookup};
use owo_colors::OwoColorize;

use crate::chainloader::{NroBuilder, NrrBuilder, NrrRegistrationFailedError};
use crate::replacement::LoadedArcEx;
use crate::{config, hashes};
use std::collections::HashMap;
use std::path::PathBuf;
use std::{
    ops::Deref,
    path::Path
};

use std::fmt;

pub type ApiLoader = StandardLoader; // temporary until an actual ApiLoader is implemented
pub type ArcropolisOrbit = Orbit<ArcLoader, StandardLoader, ApiLoader>;

pub struct FilesystemUninitializedError;

impl fmt::Debug for FilesystemUninitializedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Filesystem is uninitialized!")
    }
}

pub struct CachedFilesystem {
    loader: ArcropolisOrbit,
    hash_lookup: HashMap<Hash40, PathBuf>,
    incoming_load: Option<Hash40>
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
                Ok(launchpad) => {
                    let mut hashed_paths = HashMap::new();
                    launchpad.patch.tree.walk_paths(|node, entry_type| {
                        if entry_type.is_file() {
                            if let Ok(hash) = crate::get_smash_hash(node.get_local()) {
                                if let Some(previous_path) = hashed_paths.insert(hash, node.get_local().to_path_buf()) {
                                    error!("Found duplicate file path in the filesystem: {}", previous_path.display());
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
                    Ok(Self::Initialized(CachedFilesystem {
                        loader: launchpad.launch(ArcLoader(arc)),
                        hash_lookup: hashed_paths,
                        incoming_load: None

                    }))
                },
                Err(_) => Err(FilesystemUninitializedError),
            },
            Self::Initialized(filesystem) => Ok(Self::Initialized(filesystem)),
        }
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

    pub fn hash(&self, hash: Hash40) -> Option<&PathBuf> {
        match self {
            Self::Initialized(fs) => fs.hash_lookup.get(&hash),
            _ => None
        }
    }

    pub fn load_into(&self, hash: Hash40, buffer: &mut [u8]) -> Option<usize> {
        if let Some(data) = self.load(hash) {
            if buffer.len() < data.len() {
                error!("The size of the file data is larger than the size of the provided buffer when loading file '{}' ({:#x}).", hashes::find(hash), hash.0);
                None
            } else {
                unsafe {
                    std::ptr::copy_nonoverlapping(data.as_ptr(), buffer.as_mut_ptr(), data.len());
                }
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
            Self::Initialized(fs) => fs.incoming_load = hash,
            _ if let Some(hash) = hash => error!("Cannot set the incoming load to '{}' ({:#x}) because the filesystem is not initialized!", hashes::find(hash), hash.0),
            _ => error!("Cannot null out the incoming load because the filesystem is not initialized!")
        }
    }

    pub fn get_incoming(&mut self) -> Option<Hash40> {
        match self {
            Self::Initialized(fs) => fs.incoming_load,
            _ => {
                error!("Cannot get the incoming load because the filesystem is not initialized!");
                None
            }
        }
    }

    pub fn patch_sizes(&self, arc: &'static mut LoadedArc) {
        match self {
            Self::Initialized(fs) => {
                let region = config::region();
                for (hash, path) in fs.hash_lookup.iter() {
                    let layered_size = fs.loader.query_max_layered_filesize(path);
                    if layered_size.is_some() && layered_size > fs.loader.physical_filesize(path) {
                        let new_size = layered_size.unwrap();
                        match arc.patch_filedata(*hash, new_size as u32, region) {
                            Ok(old_size) => info!(
                                "File '{}' has a new decompressed filesize! {:#x} -> {:#x}",
                                path.display().bright_yellow(),
                                old_size.red(),
                                new_size.green()
                            ),
                            Err(_) => warn!(
                                "Failed to patch '{}' filesize! It should be {:#x}.",
                                path.display().bright_yellow(),
                                new_size.green()
                            )
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
}

#[repr(transparent)]
pub struct ArcLoader(&'static LoadedArc);

unsafe impl Send for ArcLoader {}
unsafe impl Sync for ArcLoader {}

impl Deref for ArcLoader {
    type Target = LoadedArc;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl FileLoader for ArcLoader {
    type ErrorType = LookupError;

    fn path_exists(&self, _: &Path, local_path: &Path) -> bool {
        match crate::get_smash_hash(local_path) {
            Ok(hash) => self.get_file_path_index_from_hash(hash).is_ok(),
            _ => false
        }
    }

    fn get_file_size(&self, _: &Path, local_path: &Path) -> Option<usize> {
        match crate::get_smash_hash(local_path) {
            Ok(hash) => self.get_file_data_from_hash(hash, config::region()).map_or_else(|_| { println!("{:#x}", hash.0); None }, |data| Some(data.decomp_size as usize)),
            Err(_) => None
        }
    }

    fn get_path_type(&self, _: &Path, local_path: &Path) -> Result<FileEntryType, Self::ErrorType> {
        match crate::get_smash_hash(local_path) {
            Ok(hash) => match self.get_path_list_entry_from_hash(hash)?.is_directory() {
                true => Ok(FileEntryType::Directory),
                false => Ok(FileEntryType::File)
            },
            _ => Err(LookupError::Missing)
        }
    }

    fn load_path(&self, _: &Path, local_path: &Path) -> Result<Vec<u8>, Self::ErrorType> {
        match crate::get_smash_hash(local_path) {
            Ok(path) => self.get_file_contents(path, config::region()),
            Err(_) => Err(LookupError::Missing),
        }
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

pub fn perform_discovery() -> LaunchPad<StandardLoader, StandardLoader> {
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
                static RESERVED_NAMES: &[&'static str] = &[
                    "info.toml",
                ];
                RESERVED_NAMES.contains(&name)
            },
            _ => false
        }
    };

    let collect = |x: &Path| {
        let is_config = match x.file_name() {
            Some(name) if let Some(name) = name.to_str() => {
                static RESERVED_NAMES: &[&'static str] = &[
                    "config.json"
                ];
                RESERVED_NAMES.contains(&name)
            },
            _ => false
        };

        is_config || x == Path::new("plugin.nro")
    };

    let mut launchpad = LaunchPad::new(
        DiscoverSystem::new(StandardLoader, ConflictHandler::NoRoot),
        DiscoverSystem::new(StandardLoader, ConflictHandler::NoRoot),
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
            ConflictKind::StandardConflict(error, kept) => warn!(
                "File '{}' was rejected for file '{}' during discovery.",
                error.display(),
                kept.display()
            ),
            ConflictKind::RootConflict(root_path, kept) => warn!(
                "Mod root '{}' was rejected for a file conflict with '{}' during discovery.",
                root_path.display(),
                kept.display()
            )
        }
    }

    if should_prompt {
        crate::dialog_error("During file discovery, ARCropolis encountered file conflicts.");
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