use std::{
    collections::{HashMap, HashSet},
    fmt,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use arc_config::{Config as ModConfig, ToExternal, ToSmashArc};
use discover::DiscoveryResult;
use owo_colors::OwoColorize;
use smash_arc::{ArcLookup, Hash40, LoadedArc, LoadedSearchSection};

use crate::{
    api, get_path_from_hash, hashes,
    replacement::{self, LoadedArcEx, SearchEx},
    resource, PathExtension,
};

mod cache;
mod discover;
pub mod inference;
mod utils;
pub use discover::*;
pub mod loaders;
pub use loaders::*;

static DEFAULT_CONFIG: &str = include_str!("../resources/override.json");
static IS_INIT: AtomicBool = AtomicBool::new(false);

pub struct FilesystemUninitializedError;

impl fmt::Debug for FilesystemUninitializedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Filesystem is uninitialized!")
    }
}

pub struct CachedFilesystem {
    modfs: crate::modfs::ModFs,
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

    /// Parse a pending API call and add it to the API tree. This function returns the hash, as well as the size (if needed)
    /// so that the caller can insert those into the global structs depending on the time that this call is handled
    fn handle_panding_api_call(pending: &api::PendingApiCall) -> ApiCallResult {
        use api::PendingApiCall;

        match pending {
            PendingApiCall::GenericCallback { hash, max_size, .. } => ApiCallResult {
                hash: *hash,
                path: get_path_from_hash(*hash),
                size: Some(*max_size),
            },
            PendingApiCall::StreamCallback { hash, .. } => ApiCallResult {
                hash: *hash,
                path: get_path_from_hash(*hash),
                size: None,
            },
        }
    }

    fn register_virtual_from_call(virt: &mut crate::modfs::VirtualLayer, call: &api::PendingApiCall) {
        use api::PendingApiCall;

        use crate::modfs::VirtualEntry;
        let (hash, callback, max_size) = match call {
            PendingApiCall::GenericCallback { hash, max_size, callback } => {
                (*hash, ApiCallback::GenericCallback(*callback), *max_size)
            },
            PendingApiCall::StreamCallback { hash, callback } => (*hash, ApiCallback::StreamCallback(*callback), 0x100),
        };
        virt.register(hash, VirtualEntry { callback, max_size });
    }

    /// Use the file information that was generated during file discovery to fill out a GlobalFilesystem struct
    pub fn make_from_promise(discovery: DiscoveryResult) -> CachedFilesystem {
        let arc = resource::arc();

        // Load the default config, which we will then join with the other configs
        let mut config = match ModConfig::from_json(DEFAULT_CONFIG) {
            Ok(cfg) => cfg,
            Err(_) => {
                error!("Failed to deserialize the default config.");
                ModConfig::default()
            },
        };

        let mut modfs = Self::build_modfs(&discovery.entries, &mut config);
        modfs.finalize(&mut config);

        inference::merge_into_config(&discovery.entries, &mut config);

        drop(discovery);

        let nus3audio_deps = utils::collect_nus3bank_deps(modfs.patch(), &config.unshare_blacklist);
        let mut hashed_paths: HashMap<Hash40, PathBuf> = HashMap::new();
        let mut hashed_sizes: HashMap<Hash40, usize> = HashMap::new();

        for hash in modfs.handlers().bound_hashes() {
            if let Ok(data) = arc.get_file_data_from_hash(hash, config::region()) {
                let multiplier = modfs.handlers().size_multiplier_for_hash(hash).max(10) as usize;
                hashed_paths.insert(hash, get_path_from_hash(hash));
                hashed_sizes.insert(hash, (data.decomp_size as usize) * multiplier);
            }
        }

        for dep in nus3audio_deps {
            if let Ok(hash) = crate::PathExtension::smash_hash(dep.as_path()) {
                hashed_paths.insert(hash, dep);
                hashed_sizes.insert(hash, 0);
            }
        }

        // Lock the pending callbacks and then swap the memory so that we can release lock on callbacks
        let mut pending_calls = api::PENDING_CALLBACKS.lock().unwrap();
        let mut calls = Vec::new();
        std::mem::swap(&mut *pending_calls, &mut calls);
        drop(pending_calls);

        for call in calls {
            Self::register_virtual_from_call(modfs.virt_mut(), &call);
            let ApiCallResult { hash, path, size } = Self::handle_panding_api_call(&call);
            hashed_paths.insert(hash, path);
            if let Some(size) = size {
                hashed_sizes.insert(hash, size);
            }
        }

        // Set the global flag that we are initialized (referenced by API)
        IS_INIT.store(true, Ordering::SeqCst);

        CachedFilesystem {
            modfs,
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

    fn build_modfs(entries: &[(PathBuf, PathBuf, usize)], config: &mut ModConfig) -> crate::modfs::ModFs {
        let mut modfs = crate::modfs::ModFs::new();
        modfs.populate_from(entries, config);
        modfs
    }

    /// Patches a file in the LoadedArc
    fn patch_file(&self, hash: Hash40, size: usize) -> Option<usize> {
        let arc = resource::arc_mut();
        let region = config::region();
        let decomp_size = match arc.get_file_data_from_hash(hash, region) {
            Ok(data) => data.decomp_size as usize,
            Err(_) => {
                debug!(
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

    pub fn local_hash(&self, hash: Hash40) -> Option<PathBuf> {
        if let Some(path) = self.hash_lookup.get(&hash) {
            return Some(path.clone());
        }
        self.modfs
            .patch()
            .entry_for_hash(hash)
            .map(|(local, _)| local.to_path_buf())
    }

    pub fn load(&self, hash: Hash40) -> Option<Vec<u8>> {
        match self.modfs.read_by_hash(hash) {
            Ok(data) => Some(data),
            Err(e) => {
                debug!("Failed to load '{}' ({:#x}): {:?}", hashes::find(hash), hash.0, e);
                None
            },
        }
    }

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
            debug!(
                "Removing file '{}' ({:#x}) from incoming load before using it.",
                hashes::find(hash),
                hash.0
            );
        }
        self.incoming_load = hash;
        if let Some(hash) = hash {
            self.bytes_remaining = self.size_for_hash(hash).unwrap_or(0);
        } else {
            self.bytes_remaining = 0;
        }
    }

    fn size_for_hash(&self, hash: Hash40) -> Option<usize> {
        if let Some(&size) = self.hash_size_cache.get(&hash) {
            return Some(size);
        }
        self.modfs.patch().entry_for_hash(hash).map(|(_, e)| e.size)
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

    pub fn patch_files(&mut self) {
        let mut sum_size = 0;

        let mut hash_cache = HashMap::new();
        std::mem::swap(&mut hash_cache, &mut self.hash_size_cache);
        for (hash, size) in hash_cache.iter_mut() {
            sum_size += *size;
            if let Some(old_size) = self.patch_file(*hash, *size) {
                *size = old_size;
            }
        }
        self.hash_size_cache = hash_cache;

        let patches: Vec<(Hash40, usize)> = self
            .modfs
            .patch()
            .iter_files()
            .filter_map(|(local, entry)| {
                if local.is_stream() {
                    return None;
                }
                let hash = local.smash_hash().ok()?;
                Some((hash, entry.size))
            })
            .collect();
        for (hash, size) in patches {
            sum_size += size;
            let _ = self.patch_file(hash, size);
        }

        self.total_size = sum_size;
    }

    pub fn reshare_files(&mut self) {
        let arc = resource::arc();
        let file_paths = arc.get_file_paths();

        let special_remaps: Vec<(Hash40, Hash40)> = self
            .hash_lookup
            .keys()
            .filter_map(|&hash| {
                arc.get_file_info_from_hash(hash).ok().and_then(|info| {
                    let canonical = file_paths[info.file_path_index].path.hash40();
                    if canonical != hash { Some((hash, canonical)) } else { None }
                })
            })
            .collect();
        for (old_hash, new_hash) in special_remaps {
            if let Some(path) = self.hash_lookup.remove(&old_hash) {
                self.hash_lookup.insert(new_hash, path);
            }
        }
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
        let mut base_pairs: Vec<(&String, &String)> = self.config.new_dir_infos_base.iter().collect();
        base_pairs.sort_by(|a, b| a.0.cmp(b.0));
        for (new, base) in base_pairs {
            replacement::addition::add_dir_info_with_base(&mut context, Path::new(new), Path::new(base));
        }

        let expected = self.modfs.patch().num_entries();
        context.reserve_additions(expected);
        search_context.reserve_additions(expected);

        fn is_handler_consumed(local: &std::path::Path) -> bool {
            let Some(name) = local.file_name().and_then(|n| n.to_str()) else {
                return false;
            };
            matches!(name, "config.json" | "plugin.nro" | "bgm_property.bin")
                || [
                    "prcx", "prcxml", "stdatx", "stdatxml", "stprmx", "stprmxml",
                    "xmsbt", "patch3audio", "motdiff", "yml",
                ]
                .iter()
                .any(|ext| name.ends_with(ext))
        }

        // Go through and add any files that were not found in the data.arc
        for (local, _entry) in self.modfs.patch().iter_files() {
            if local.is_stream() {
                continue;
            }
            if is_handler_consumed(local) {
                continue;
            }
            let hash = match local.smash_hash() {
                Ok(h) => h,
                Err(_) => continue,
            };
            if context.contains_file(hash) {
                continue;
            }
            replacement::addition::add_file(&mut context, local);
            replacement::addition::add_searchable_file_recursive(&mut search_context, local);
        }

        // Don't unshare any files in the unshare blacklist (nus3audio handled during filesystem finish)
        let mut files_set: HashSet<Hash40> = HashSet::new();
        for (&hash, _) in self.hash_lookup.iter() {
            if !self.config.unshare_blacklist.contains(&hash.to_external()) {
                files_set.insert(hash);
            }
        }
        for (local, _) in self.modfs.patch().iter_files() {
            if local.is_stream() {
                continue;
            }
            if is_handler_consumed(local) {
                continue;
            }
            if let Ok(hash) = local.smash_hash() {
                if !self.config.unshare_blacklist.contains(&hash.to_external()) {
                    files_set.insert(hash);
                }
            }
        }
        let files: Vec<Hash40> = files_set.into_iter().collect();

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

        replacement::unshare::unshare_files(&mut context, hash_ignore, files.into_iter());

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
        let mut dir_entries: Vec<(&hash40::Hash40, &Vec<hash40::Hash40>)> = self.config.new_dir_files.iter().collect();
        dir_entries.sort_by_key(|(k, _)| k.0);
        for (hash, files) in dir_entries {
            replacement::addition::add_files_to_directory(&mut context, hash.to_smash_arc(), files.iter().map(|hash| hash.to_smash_arc()).collect());
        }

        resource::arc_mut().take_context(context);
        resource::search_mut().take_context(search_context);
    }

    /// Gets the global mod config
    pub fn config(&self) -> &ModConfig {
        &self.config
    }

    pub fn modfs(&self) -> &crate::modfs::ModFs {
        &self.modfs
    }

    /// Handles late API calls
    pub fn handle_late_api_call(&mut self, call: api::PendingApiCall) {
        Self::register_virtual_from_call(self.modfs.virt_mut(), &call);
        let ApiCallResult { hash, path, size } = Self::handle_panding_api_call(&call);

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

    pub fn get_cached_size(&self, hash: Hash40) -> Option<usize> {
        self.size_for_hash(hash)
    }

    pub fn get_sum_size(&self) -> usize {
        self.total_size
    }
}

pub enum GlobalFilesystem {
    Uninitialized,
    Promised(std::thread::JoinHandle<DiscoveryResult>),
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
                Ok(discovery) => Ok(Self::Initialized(Box::new(CachedFilesystem::make_from_promise(discovery)))),
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

    pub fn modfs(&self) -> &crate::modfs::ModFs {
        match self {
            Self::Initialized(fs) => fs.modfs(),
            _ => panic!("Global Filesystem is not initialized!"),
        }
    }

    pub fn local_hash(&self, hash: Hash40) -> Option<PathBuf> {
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
