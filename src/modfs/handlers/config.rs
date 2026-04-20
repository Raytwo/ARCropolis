use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use arc_config::{
    hash40::Hash40 as CfgHash,
    search::{File as CfgFile, FileSet as CfgFileSet, Folder as CfgFolder},
    Config as ModConfig,
};
use serde::{Deserialize, Serialize};
use smash_arc::Hash40;

use crate::modfs::{registry::FileHandler, DiscoveryContext};

#[derive(Default, Deserialize)]
#[serde(default)]
struct PartialModConfig {
    #[serde(alias = "keep-shared")]
    #[serde(alias = "keep_shared")]
    #[serde(alias = "unshare-blacklist")]
    unshare_blacklist: Vec<CfgHash>,

    #[serde(alias = "preprocess-reshare")]
    preprocess_reshare: HashMap<CfgHash, CfgHash>,

    #[serde(alias = "share-to-vanilla")]
    share_to_vanilla: HashMap<CfgHash, CfgFileSet>,

    #[serde(alias = "share-to-added")]
    #[serde(alias = "new-shared-files")]
    #[serde(alias = "new_shared_files")]
    share_to_added: HashMap<CfgHash, CfgFileSet>,

    #[serde(alias = "new-dir-infos")]
    new_dir_infos: Vec<String>,

    #[serde(alias = "new-dir-infos-base")]
    new_dir_infos_base: HashMap<String, String>,

    #[serde(alias = "new-dir-files")]
    new_dir_files: HashMap<CfgHash, Vec<CfgHash>>,
}

impl PartialModConfig {
    fn merge_into(self, config: &mut ModConfig) {
        let PartialModConfig {
            unshare_blacklist,
            preprocess_reshare,
            share_to_vanilla,
            share_to_added,
            new_dir_infos,
            new_dir_infos_base,
            new_dir_files,
        } = self;
        config.unshare_blacklist.extend(unshare_blacklist);
        config.preprocess_reshare.extend(preprocess_reshare);
        for (k, v) in share_to_vanilla {
            config
                .share_to_vanilla
                .entry(k)
                .or_insert_with(|| CfgFileSet(Vec::new()))
                .0
                .extend(v.0);
        }
        for (k, v) in share_to_added {
            config
                .share_to_added
                .entry(k)
                .or_insert_with(|| CfgFileSet(Vec::new()))
                .0
                .extend(v.0);
        }
        config.new_dir_infos.extend(new_dir_infos);
        config.new_dir_infos_base.extend(new_dir_infos_base);
        for (k, v) in new_dir_files {
            config.new_dir_files.entry(k).or_default().extend(v);
        }
    }
}

#[derive(Serialize, Deserialize)]
struct CachedFolder {
    full_path: u64,
    name: Option<u64>,
    parent: Option<Box<CachedFolder>>,
}

#[derive(Serialize, Deserialize)]
struct CachedFile {
    full_path: u64,
    file_name: u64,
    parent: CachedFolder,
    extension: u64,
}

#[derive(Serialize, Deserialize)]
struct CachedFileSet(Vec<CachedFile>);

#[derive(Serialize, Deserialize)]
struct CachedConfig {
    unshare_blacklist: Vec<u64>,
    preprocess_reshare: Vec<(u64, u64)>,
    share_to_vanilla: Vec<(u64, CachedFileSet)>,
    share_to_added: Vec<(u64, CachedFileSet)>,
    new_dir_files: Vec<(u64, Vec<u64>)>,
    new_dir_infos: Vec<String>,
    new_dir_infos_base: Vec<(String, String)>,
}

fn folder_to_cached(f: &CfgFolder) -> CachedFolder {
    CachedFolder {
        full_path: f.full_path.0,
        name: f.name.map(|n| n.0),
        parent: f.parent.as_ref().map(|p| Box::new(folder_to_cached(p))),
    }
}

fn folder_from_cached(c: CachedFolder) -> CfgFolder {
    CfgFolder {
        full_path: CfgHash(c.full_path),
        name: c.name.map(CfgHash),
        parent: c.parent.map(|p| Box::new(folder_from_cached(*p))),
    }
}

fn file_to_cached(f: &CfgFile) -> CachedFile {
    CachedFile {
        full_path: f.full_path.0,
        file_name: f.file_name.0,
        parent: folder_to_cached(&f.parent),
        extension: f.extension.0,
    }
}

fn file_from_cached(c: CachedFile) -> CfgFile {
    CfgFile {
        full_path: CfgHash(c.full_path),
        file_name: CfgHash(c.file_name),
        parent: folder_from_cached(c.parent),
        extension: CfgHash(c.extension),
    }
}

fn config_to_cached(cfg: &ModConfig) -> CachedConfig {
    CachedConfig {
        unshare_blacklist: cfg.unshare_blacklist.iter().map(|h| h.0).collect(),
        preprocess_reshare: cfg.preprocess_reshare.iter().map(|(k, v)| (k.0, v.0)).collect(),
        share_to_vanilla: cfg
            .share_to_vanilla
            .iter()
            .map(|(k, set)| (k.0, CachedFileSet(set.0.iter().map(file_to_cached).collect())))
            .collect(),
        share_to_added: cfg
            .share_to_added
            .iter()
            .map(|(k, set)| (k.0, CachedFileSet(set.0.iter().map(file_to_cached).collect())))
            .collect(),
        new_dir_files: cfg
            .new_dir_files
            .iter()
            .map(|(k, v)| (k.0, v.iter().map(|h| h.0).collect()))
            .collect(),
        new_dir_infos: cfg.new_dir_infos.clone(),
        new_dir_infos_base: cfg.new_dir_infos_base.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
    }
}

fn config_from_cached(c: CachedConfig) -> ModConfig {
    let mut cfg = ModConfig::new();
    cfg.unshare_blacklist = c.unshare_blacklist.into_iter().map(CfgHash).collect();
    cfg.preprocess_reshare = c.preprocess_reshare.into_iter().map(|(k, v)| (CfgHash(k), CfgHash(v))).collect();
    cfg.share_to_vanilla = c
        .share_to_vanilla
        .into_iter()
        .map(|(k, set)| (CfgHash(k), CfgFileSet(set.0.into_iter().map(file_from_cached).collect())))
        .collect();
    cfg.share_to_added = c
        .share_to_added
        .into_iter()
        .map(|(k, set)| (CfgHash(k), CfgFileSet(set.0.into_iter().map(file_from_cached).collect())))
        .collect();
    cfg.new_dir_files = c
        .new_dir_files
        .into_iter()
        .map(|(k, v)| (CfgHash(k), v.into_iter().map(CfgHash).collect()))
        .collect();
    cfg.new_dir_infos = c.new_dir_infos;
    cfg.new_dir_infos_base = c.new_dir_infos_base.into_iter().collect::<HashMap<_, _>>();
    cfg
}

#[derive(Default)]
pub struct ConfigHandler {
    paths: Vec<(PathBuf, usize)>,
}

impl ConfigHandler {
    fn cache_file() -> PathBuf {
        crate::utils::paths::cache().join("config_merged.cache").into()
    }

    fn cache_key(paths: &[(PathBuf, usize)]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut entries: Vec<&(PathBuf, usize)> = paths.iter().collect();
        entries.sort();

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        env!("CARGO_PKG_VERSION").hash(&mut hasher);
        entries.hash(&mut hasher);
        hasher.finish()
    }

    fn load_cached(key: u64) -> Option<ModConfig> {
        let bytes = std::fs::read(Self::cache_file()).ok()?;
        if bytes.len() < 8 {
            return None;
        }
        let mut key_bytes = [0u8; 8];
        key_bytes.copy_from_slice(&bytes[..8]);
        let stored_key = u64::from_le_bytes(key_bytes);
        if stored_key != key {
            return None;
        }
        bincode::deserialize::<CachedConfig>(&bytes[8..])
            .ok()
            .map(config_from_cached)
    }

    fn save_cached(key: u64, config: &ModConfig) {
        let mut bytes = key.to_le_bytes().to_vec();
        let cached = config_to_cached(config);
        match bincode::serialize(&cached) {
            Ok(serialized) => bytes.extend_from_slice(&serialized),
            Err(e) => {
                warn!("Failed to serialize merged config for caching: {:?}", e);
                return;
            },
        }
        if let Err(e) = std::fs::write(Self::cache_file(), &bytes) {
            warn!("Failed to write merged config cache: {:?}", e);
        }
    }
}

impl FileHandler for ConfigHandler {
    fn name(&self) -> &'static str {
        "config"
    }

    fn filenames(&self) -> &'static [&'static str] {
        &["config.json"]
    }

    fn discover(&mut self, _ctx: &mut DiscoveryContext, full_path: &Path, _local: &Path, size: usize) -> Option<Hash40> {
        self.paths.push((full_path.to_path_buf(), size));
        None
    }

    fn finalize(&self, config: &mut ModConfig) {
        let key = Self::cache_key(&self.paths);

        if let Some(cached) = Self::load_cached(key) {
            *config = cached;
            return;
        }

        let mut parsed = 0usize;
        let mut failed = 0usize;
        for (path, _size) in &self.paths {
            match std::fs::read(path) {
                Ok(bytes) => match serde_json::from_slice::<PartialModConfig>(&bytes) {
                    Ok(partial) => {
                        partial.merge_into(config);
                        parsed += 1;
                    },
                    Err(e) => {
                        warn!("config: parse failed for {}: {}", path.display(), e);
                        failed += 1;
                    },
                },
                Err(e) => {
                    warn!("config: read failed for {}: {}", path.display(), e);
                    failed += 1;
                },
            }
        }
        log::info!(
            target: "std",
            "config: merged {} load-bearing fields from {}/{} config.json ({} failed)",
            config.share_to_vanilla.len() + config.share_to_added.len()
                + config.unshare_blacklist.len() + config.preprocess_reshare.len(),
            parsed,
            self.paths.len(),
            failed,
        );

        Self::save_cached(key, config);
    }
}
