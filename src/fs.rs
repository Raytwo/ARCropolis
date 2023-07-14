use std::{
    cell::UnsafeCell,
    collections::{HashMap, HashSet},
    fmt,
    io::Write,
    ops::Deref,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use arc_config::{Config as ModConfig, ToExternal, ToSmashArc};
use orbits::{orbit::LaunchPad, Error, FileEntryType, FileLoader, Orbit, StandardLoader, Tree};
use owo_colors::OwoColorize;
use smash_arc::{ArcLookup, Hash40, LoadedArc, LoadedSearchSection, LookupError, SearchLookup};
use thiserror::Error;

// pub mod api;
// mod event;
use crate::{
    api, config, get_path_from_hash, hashes,
    replacement::{self, LoadedArcEx, SearchEx},
    resource, PathExtension,
};

mod discover;
mod utils;
pub use discover::*;
pub mod loaders;
pub use loaders::*;

static DEFAULT_CONFIG: &str = include_str!("../resources/override.json");
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
    config: ModConfig,
    hash_lookup: HashMap<Hash40, PathBuf>,
    hash_size_cache: HashMap<Hash40, usize>,
    incoming_load: Option<Hash40>,
    bytes_remaining: usize,
    current_nus3bank_id: u32,
    nus3banks: HashMap<Hash40, u32>,
    total_size: usize,
}

impl CachedFilesystem {
    /// Load all configs that were found during discovery and join them into a singular config
    fn load_remaining_configs(current: &mut ModConfig, launchpad: &LaunchPad<StandardLoader>) {
        for (root, local) in launchpad.collected_paths().iter() {
            let full_path = root.join(local);
            if !full_path.exists() {
                warn!("Collected path at {} does not exist.", full_path.display());
                continue;
            }

            if !full_path.ends_with("config.json") {
                trace!("Skipping path {} while loading all configs", full_path.display());
                continue;
            }

            // Read the file data and map it to a json. If that fails, just skip this current JSON.
            let cfg = ModConfig::from_file_json(&full_path).ok();

            if let Some(cfg) = cfg {
                current.merge(cfg);
            } else {
                warn!("Could not read/parse JSON data from file {}", full_path.display());
            }
        }
    }

    /// Get a list of all PRC patch files and add them to the virtual tree
    fn initialize_prc_patches(launchpad: &LaunchPad<StandardLoader>, api_tree: &mut Tree<ApiLoader>) -> HashSet<Hash40> {
        let mut set = HashSet::new();
        for (root, path) in launchpad.collected_paths().iter() {
            // The collected paths gives us everything so we only want these extensions
            if path.has_extension("prcx")
                || path.has_extension("prcxml")
                || path.has_extension("stdatx")
                || path.has_extension("stdatxml")
                || path.has_extension("stprmx")
                || path.has_extension("stprmxml")
            {
                if let Some(hash) = utils::add_prc_patch(api_tree, root, path) {
                    set.insert(hash);
                }
            }
        }
        set
    }

    /// Get a list of all MSBT patch files and add them to the virtual tree
    fn initialize_msbt_patches(launchpad: &LaunchPad<StandardLoader>, api_tree: &mut Tree<ApiLoader>) -> HashSet<Hash40> {
        let mut set = HashSet::new();
        for (root, path) in launchpad.collected_paths().iter() {
            // The collected paths gives us everything so we only want these extensions
            if path.has_extension("xmsbt") {
                if let Some(hash) = utils::add_msbt_patch(api_tree, root, path) {
                    set.insert(hash);
                }
            }
        }
        set
    }

    /// Get a list of all nus3audio patch files and add them to the virtual tree
    fn initialize_nus3audio_patches(launchpad: &LaunchPad<StandardLoader>, api_tree: &mut Tree<ApiLoader>) -> HashSet<Hash40> {
        let mut set = HashSet::new();
        for (root, path) in launchpad.collected_paths().iter() {
            // The collected paths gives us everything so we only want these extensions
            if path.has_extension("patch3audio") {
                if let Some(hash) = utils::add_nus3audio_patch(api_tree, root, path) {
                    set.insert(hash);
                }
            }
        }
        set
    }

    /// Get a list of all motion list patch files and add them to the virtual tree
    fn initialize_motionlist_patches(launchpad: &LaunchPad<StandardLoader>, api_tree: &mut Tree<ApiLoader>) -> HashSet<Hash40> {
        let mut set = HashSet::new();
        for (root, path) in launchpad.collected_paths().iter() {
            // The collected paths gives us everything so we only want these extensions
            if path.has_extension("motdiff") || path.ends_with("motion_list.yml") {
                if let Some(hash) = utils::add_motionlist_patch(api_tree, root, path) {
                    set.insert(hash);
                }
            }
        }
        set
    }

    /// Get a list of all bgm_property files and add them to the virtual tree
    fn initialize_bgm_property_patches(launchpad: &LaunchPad<StandardLoader>, api_tree: &mut Tree<ApiLoader>) -> HashSet<Hash40> {
        let mut set = HashSet::new();
        for (root, path) in launchpad.collected_paths().iter() {
            // The collected paths gives us everything so we only want these extensions
            if path.file_name() == Path::new("bgm_property.bin").file_name() {
                if let Some(hash) = utils::add_bgm_property_patch(api_tree, root, path) {
                    set.insert(hash);
                }
            }
        }
        set
    }

    /// Parse a pending API call and add it to the API tree. This function returns the hash, as well as the size (if needed)
    /// so that the caller can insert those into the global structs depending on the time that this call is handled
    fn handle_panding_api_call(api_tree: &mut Tree<ApiLoader>, pending: api::PendingApiCall) -> ApiCallResult {
        use api::PendingApiCall;

        match pending {
            PendingApiCall::GenericCallback { hash, max_size, callback } => {
                let path = get_path_from_hash(hash);

                utils::add_file_to_api_tree(api_tree, "api:/generic-cb", &path, ApiCallback::GenericCallback(callback));

                ApiCallResult {
                    hash,
                    path,
                    size: Some(max_size),
                }
            },
            PendingApiCall::StreamCallback { hash, callback } => {
                let path = get_path_from_hash(hash);

                utils::add_file_to_api_tree(api_tree, "api:/stream-cb", &path, ApiCallback::StreamCallback(callback));

                ApiCallResult { hash, path, size: None }
            },
        }
    }

    /// Use the file information that was generated during file discovery to fill out a GlobalFilesystem struct
    fn make_from_promise(launchpad: LaunchPad<StandardLoader>) -> CachedFilesystem {
        let arc = resource::arc();
        // Provide the discovered tree and get two hashmaps, one of the sizes of each file discovered (for patching)
        // and also get hash40 -> PathBuf lookup, since it's going to be a lot faster when the game is loading
        // individual files
        let (mut hashed_sizes, mut hashed_paths) = utils::make_hash_maps(launchpad.tree());

        // Add the discovered paths to the global hashes, so that when a file is loading that *we have discovered* we can guarantee
        // that we are printing the real path in the logger.
        for (_hash, path) in hashed_paths.iter() {
            if let Some(string) = path.to_str() {
                hashes::add(string);
            }
        }

        // Load the default config, which we will then join with the other configs
        let mut config = match ModConfig::from_json(DEFAULT_CONFIG) {
            Ok(cfg) => cfg,
            Err(_) => {
                error!("Failed to deserialize the default config.");
                ModConfig::default()
            },
        };

        // Load all of the user configs into the main config
        Self::load_remaining_configs(&mut config, &launchpad);

        // Collect all of the NUS3BANK dependencies that audio files have in order to be unshared
        // Note that we pass the unshare blacklist because if the NUS3AUDIO files are blacklisted then we shouldn't unshare the
        // actual nus3bank either
        let nus3audio_deps = utils::get_required_nus3banks(launchpad.tree(), &config.unshare_blacklist);

        // Create the API file tree and start adding things to it
        let mut api_tree = Tree::new(ApiLoader::default());

        // Set up the API tree with all of the patch files
        let mut hashes = Self::initialize_prc_patches(&launchpad, &mut api_tree);
        hashes.extend(Self::initialize_msbt_patches(&launchpad, &mut api_tree));
        hashes.extend(Self::initialize_nus3audio_patches(&launchpad, &mut api_tree));
        hashes.extend(Self::initialize_motionlist_patches(&launchpad, &mut api_tree));
        hashes.extend(Self::initialize_bgm_property_patches(&launchpad, &mut api_tree));

        // Add the hash files and set the new size to 10x the original files
        for hash in hashes {
            if let Ok(data) = arc.get_file_data_from_hash(hash, config::region()) {
                hashed_paths.insert(hash, get_path_from_hash(hash));
                hashed_sizes.insert(hash, (data.decomp_size as usize) * 10);
            }
        }

        // Add all of the NUS3BANKs that our NUS3AUDIOs depend on to the API tree
        for dep in nus3audio_deps {
            let hash = utils::add_file_to_api_tree(&mut api_tree, "api:/patch-nus3bank", &dep, ApiCallback::None);
            if let Some(hash) = hash {
                hashed_paths.insert(hash, dep);
                hashed_sizes.insert(hash, 0); // We want to use vanilla size because we are only editing the content
            }
        }

        // Lock the pending callbacks and then swap the memory so that we can release lock on callbacks
        let mut pending_calls = api::PENDING_CALLBACKS.lock();
        let mut calls = Vec::new();
        std::mem::swap(&mut *pending_calls, &mut calls);
        drop(pending_calls);

        // Go through each API call, insert it into the api tree, and then insert it's info into the global data
        for call in calls {
            let ApiCallResult { hash, path, size } = Self::handle_panding_api_call(&mut api_tree, call);

            hashed_paths.insert(hash, path);
            if let Some(size) = size {
                hashed_sizes.insert(hash, size);
            }
        }

        // Set the global flag that we are initialized (referenced by API)
        IS_INIT.store(true, Ordering::SeqCst);

        // Construct a CachedFilesystem
        CachedFilesystem {
            loader: launchpad.launch(ArcLoader(arc), api_tree),
            config,
            hash_lookup: hashed_paths,
            hash_size_cache: hashed_sizes,
            incoming_load: None,
            bytes_remaining: 0,
            current_nus3bank_id: 7420,
            nus3banks: HashMap::new(),
            total_size: 0,
        }
    }

    /// Patches a file in the LoadedArc
    fn patch_file(&self, hash: Hash40, size: usize) -> Option<usize> {
        let arc = resource::arc_mut();
        let region = config::region();
        let decomp_size = match arc.get_file_data_from_hash(hash, region) {
            Ok(data) => data.decomp_size as usize,
            Err(_) => {
                warn!(
                    "Failed to patch '{}' ({:#x}) filesize! It should be {:#x}.",
                    hashes::find(hash).bright_yellow(),
                    hash.0,
                    size.green()
                );
                return None;
            },
        };

        if size > decomp_size {
            match arc.patch_filedata(hash, size as u32, region) {
                Ok(old_size) => {
                    // info!(
                    //     "File '{}' ({:#x}) has a new decompressed filesize! {:#x} -> {:#x}",
                    //     hashes::find(hash).bright_yellow(),
                    //     hash.0,
                    //     old_size.red(),
                    //     size.green()
                    // );
                    Some(old_size as usize)
                },
                Err(_) => None,
            }
        } else {
            None
        }
    }

    // Search the provided hash for a PathBuf in the hash lookup
    pub fn local_hash(&self, hash: Hash40) -> Option<&PathBuf> {
        self.hash_lookup.get(&hash)
    }

    // Get the "actual path" for a file hash
    pub fn hash(&self, hash: Hash40) -> Option<PathBuf> {
        self.local_hash(hash).and_then(|x| self.loader.query_actual_path(x))
    }

    // Load the file data from the Orbits filesystem
    pub fn load(&self, hash: Hash40) -> Option<Vec<u8>> {
        let path = if let Some(path) = self.hash_lookup.get(&hash) {
            path
        } else {
            error!(
                "Failed to load data for '{}' ({:#x}) because the filesystem does not contain it!",
                hashes::find(hash),
                hash.0
            );
            return None;
        };

        match self.loader.load(path) {
            Ok(data) => Some(data),
            Err(Error::Virtual(ApiLoaderError::NoVirtFile)) => {
                if let Ok(data) = self.loader.load_patch(path) {
                    Some(data)
                } else if let Ok(data) = ArcLoader(resource::arc()).load_path(Path::new(""), path) {
                    Some(data)
                } else {
                    error!("Failed to load data for {} because all load paths failed.", path.display());
                    None
                }
            },
            Err(e) => {
                error!("Failed to load data for {}. Reason: {:?}", path.display(), e);
                None
            },
        }
    }

    // Load the file data from the Orbits filesystem into a pre-allocated buffer
    pub fn load_into(&self, hash: Hash40, mut buffer: &mut [u8]) -> Option<usize> {
        if let Some(data) = self.load(hash) {
            if buffer.len() < data.len() {
                error!(
                    "The size of the file data is larger than the size of the provided buffer when loading file '{}' ({:#x}).",
                    hashes::find(hash),
                    hash.0
                );
                None
            } else {
                buffer.write_all(&data).unwrap();
                Some(data.len())
            }
        } else {
            None
        }
    }

    // Sets the incoming file to be loaded
    pub fn set_incoming(&mut self, hash: Option<Hash40>) {
        if let Some(hash) = self.incoming_load.take() {
            warn!(
                "Removing file '{}' ({:#x}) from incoming load before using it.",
                hashes::find(hash),
                hash.0
            );
        }
        self.incoming_load = hash;
        if let Some(hash) = hash {
            self.bytes_remaining = *self.hash_size_cache.get(&hash).unwrap_or(&0);
        } else {
            self.bytes_remaining = 0;
        }
    }

    // Gets the incoming file to be loaded
    pub fn get_incoming(&mut self) -> Option<Hash40> {
        self.incoming_load.take()
    }

    // Subtracts the amount of bytes remanining from the current load.
    // This prevents multiloads on the same file
    pub fn sub_remaining_bytes(&mut self, count: usize) -> Option<Hash40> {
        if count >= self.bytes_remaining {
            self.bytes_remaining = 0;
            self.get_incoming()
        } else {
            self.bytes_remaining -= count;
            None
        }
    }

    // Patch all files in the hash size cache
    pub fn patch_files(&mut self) {
        let mut hash_cache = HashMap::new();
        let mut sum_size = 0;
        std::mem::swap(&mut hash_cache, &mut self.hash_size_cache);
        for (hash, size) in hash_cache.iter_mut() {
            sum_size += *size;
            if let Some(old_size) = self.patch_file(*hash, *size) {
                *size = old_size;
            }
        }
        self.hash_size_cache = hash_cache;
        self.total_size = sum_size;
    }

    // Reshares all hashes that still need to be shared, so that we don't get fake one-slot behavior
    pub fn reshare_files(&mut self) {
        let arc = resource::arc();
        let file_paths = arc.get_file_paths();
        let mut old_map = HashMap::new();
        std::mem::swap(&mut self.hash_lookup, &mut old_map);
        self.hash_lookup = old_map
            .into_iter()
            .map(|(hash, path)| {
                (
                    arc.get_file_info_from_hash(hash)
                        .map_or_else(|_| hash, |info| file_paths[info.file_path_index].path.hash40()),
                    path,
                )
            })
            .collect();
    }

    /// Goes through and performs the required file manipulation in order to load mods
    pub fn process_mods(&mut self) {
        let mut context = LoadedArc::make_addition_context();
        let mut search_context = LoadedSearchSection::make_context();

        let mut hash_ignore = HashSet::new();
        // Reshare certain files to the right directories
        // This is mostly used for Dark Samus because of her victory bunshin article
        for (dep, source) in self.config.preprocess_reshare.iter() {
            hash_ignore.extend(replacement::preprocess::reshare_contained_files(
                &mut context,
                dep.to_smash_arc(),
                source.to_smash_arc(),
            ));
        }

        // Add new dir infos before resharing the file group to avoid some characters inf loading (Pyra c00)
        // Add new dir infos
        for dir_info in self.config.new_dir_infos.iter() {
            replacement::addition::add_dir_info(&mut context, Path::new(dir_info));
        }

        // Add new dir infos that use a base before adding the files
        for (new, base) in self.config.new_dir_infos_base.iter() {
            replacement::addition::add_dir_info_with_base(&mut context, Path::new(new), Path::new(base));
        }

        // Go through and add any files that were not found in the data.arc
        self.loader.walk_patch(|node, ty| {
            if node.get_local().is_stream() || !ty.is_file() {
                return;
            }

            let _hash = if let Ok(hash) = node.get_local().smash_hash() {
                if context.contains_file(hash) {
                    return;
                }
                hash
            } else {
                return;
            };

            replacement::addition::add_file(&mut context, node.get_local());
            replacement::addition::add_searchable_file_recursive(&mut search_context, node.get_local());
        });

        // Don't unshare any files in the unshare blacklist (nus3audio handled during filesystem finish)
        let files = self.hash_lookup.iter().filter_map(
            |(hash, _path)| {
                if self.config.unshare_blacklist.contains(&hash.to_external()) {
                    None
                } else {
                    Some(*hash)
                }
            },
        );

        for (hash, new_file_set) in self.config.share_to_vanilla.iter() {
            for new_file in new_file_set.0.iter() {
                if context.contains_file(new_file.full_path.to_smash_arc()) {
                    replacement::unshare::reshare_file(&mut context, new_file.full_path.to_smash_arc(), hash.to_smash_arc());
                } else {
                    replacement::addition::add_shared_file(&mut context, new_file, hash.to_smash_arc());
                    replacement::addition::add_shared_searchable_file(&mut search_context, new_file);
                }
            }
        }

        // Reshare any files that depend on files in file groups, as we need to get rid of those else we crash.
        replacement::unshare::reshare_file_groups(&mut context);

        replacement::unshare::unshare_files(&mut context, hash_ignore, files);

        // Add new shared files to added files
        for (hash, new_file_set) in self.config.share_to_added.iter() {
            for new_file in new_file_set.0.iter() {
                if context.contains_file(new_file.full_path.to_smash_arc()) {
                    replacement::unshare::reshare_file(&mut context, new_file.full_path.to_smash_arc(), hash.to_smash_arc());
                } else {
                    replacement::addition::add_shared_file(&mut context, new_file, hash.to_smash_arc());
                    replacement::addition::add_shared_searchable_file(&mut search_context, new_file);
                }
            }
        }

        println!("Adding files to dir infos...");
        // Add new files to the dir infos
        for (hash, files) in self.config.new_dir_files.iter() {
            replacement::addition::add_files_to_directory(&mut context, hash.to_smash_arc(), files.iter().map(|hash| hash.to_smash_arc()).collect());
        }

        resource::arc_mut().take_context(context);
        resource::search_mut().take_context(search_context);
    }

    /// Gets the global mod config
    pub fn config(&self) -> &ModConfig {
        &self.config
    }

    /// Handles late API calls
    pub fn handle_late_api_call(&mut self, call: api::PendingApiCall) {
        let ApiCallResult { hash, path, size } = Self::handle_panding_api_call(self.loader.virt_mut(), call);

        self.hash_lookup.insert(hash, path);
        if let Some(size) = size {
            if let Some(old_size) = self.patch_file(hash, size) {
                if let Some(size_mut) = self.hash_size_cache.get_mut(&hash) {
                    if *size_mut > old_size {
                        *size_mut = old_size;
                    }
                } else {
                    self.hash_size_cache.insert(hash, size);
                }
            }
        }
    }

    /// Gets the cached size
    pub fn get_cached_size(&self, hash: Hash40) -> Option<usize> {
        self.hash_size_cache.get(&hash).copied()
    }

    pub fn get_sum_size(&self) -> usize {
        self.total_size
    }
}

pub enum GlobalFilesystem {
    Uninitialized,
    Promised(std::thread::JoinHandle<LaunchPad<StandardLoader>>),
    Initialized(Box<CachedFilesystem>),
}

struct ApiCallResult {
    hash: Hash40,
    path: PathBuf,
    size: Option<usize>,
}

impl GlobalFilesystem {
    pub fn finish(self, _arc: &'static LoadedArc) -> Result<Self, FilesystemUninitializedError> {
        match self {
            Self::Uninitialized => Err(FilesystemUninitializedError),
            Self::Promised(promise) => match promise.join() {
                Ok(launchpad) => Ok(Self::Initialized(Box::new(CachedFilesystem::make_from_promise(launchpad)))),
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
            _ => panic!("Global Filesystem is not initialized!"),
        }
    }

    pub fn get_mut(&mut self) -> &mut ArcropolisOrbit {
        match self {
            Self::Initialized(fs) => &mut fs.loader,
            _ => panic!("Global Filesystem is not initialized!"),
        }
    }

    pub fn hash(&self, hash: Hash40) -> Option<PathBuf> {
        match self {
            Self::Initialized(fs) => fs.hash(hash),
            _ => None,
        }
    }

    pub fn local_hash(&self, hash: Hash40) -> Option<&PathBuf> {
        match self {
            Self::Initialized(fs) => fs.local_hash(hash),
            _ => None,
        }
    }

    pub fn load_into(&self, hash: Hash40, buffer: &mut [u8]) -> Option<usize> {
        match self {
            Self::Initialized(fs) => fs.load_into(hash, buffer),
            _ => {
                error!(
                    "Cannot load data for '{}' ({:#x}) because the filesystem is not initialized!",
                    hashes::find(hash),
                    hash.0
                );
                None
            },
        }
    }

    pub fn load(&self, hash: Hash40) -> Option<Vec<u8>> {
        match self {
            Self::Initialized(fs) => fs.load(hash),
            _ => {
                error!(
                    "Cannot load data for '{}' ({:#x}) because the filesystem is not initialized!",
                    hashes::find(hash),
                    hash.0
                );
                None
            },
        }
    }

    pub fn set_incoming(&mut self, hash: Option<Hash40>) {
        match self {
            Self::Initialized(fs) => fs.set_incoming(hash),
            _ if let Some(hash) = hash => error!("Cannot set the incoming load to '{}' ({:#x}) because the filesystem is not initialized!", hashes::find(hash), hash.0),
            _ => error!("Cannot null out the incoming load because the filesystem is not initialized!")
        }
    }

    pub fn sub_remaining_bytes(&mut self, count: usize) -> Option<Hash40> {
        match self {
            Self::Initialized(fs) => fs.sub_remaining_bytes(count),
            _ => {
                error!("Cannot subtract reamining bytes because the filesystem is not initialized!");
                None
            },
        }
    }

    pub fn get_incoming(&mut self) -> Option<Hash40> {
        match self {
            Self::Initialized(fs) => fs.get_incoming(),
            _ => {
                error!("Cannot get the incoming load because the filesystem is not initialized!");
                None
            },
        }
    }

    pub fn patch_files(&mut self) {
        match self {
            Self::Initialized(fs) => fs.patch_files(),
            _ => error!("Cannot patch sizes because the filesystem is not initialized!"),
        }
    }

    pub fn share_hashes(&mut self) {
        match self {
            Self::Initialized(fs) => fs.reshare_files(),
            _ => {
                error!("Cannot share the hashes because the filesystem is not initialized!");
            },
        }
    }

    pub fn process_mods(&mut self) {
        match self {
            Self::Initialized(fs) => fs.process_mods(),
            _ => {
                error!("Cannot unshare files because the filesystem is not initialized!");
            },
        }
    }

    pub fn config(&self) -> &ModConfig {
        match self {
            Self::Initialized(fs) => fs.config(),
            _ => panic!("Global Filesystem is not initialized!"),
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
            _ => None,
        }
    }

    pub fn handle_api_request(&mut self, call: api::PendingApiCall) {
        debug!("Incoming API request");
        if let Self::Initialized(fs) = self {
            fs.handle_late_api_call(call)
        }
    }

    pub fn get_cached_size(&self, hash: Hash40) -> Option<usize> {
        match self {
            Self::Initialized(fs) => fs.get_cached_size(hash),
            _ => None,
        }
    }

    pub fn get_sum_size(&self) -> Option<usize> {
        match self {
            Self::Initialized(fs) => Some(fs.get_sum_size()),
            _ => None,
        }
    }
}
