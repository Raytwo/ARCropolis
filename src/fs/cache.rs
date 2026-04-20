use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use super::discover::DiscoveryResult;

fn hash_tree_signature<H: Hasher>(dir: &Path, hasher: &mut H) {
    let iter = match std::fs::read_dir(dir) {
        Ok(it) => it,
        Err(_) => return,
    };
    let mut subdirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<(PathBuf, u64)> = Vec::new();
    for entry in iter.filter_map(|e| e.ok()) {
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if ft.is_dir() {
            subdirs.push(entry.path());
        } else if ft.is_file() {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            files.push((entry.path(), size));
        }
    }
    subdirs.sort();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    for (file, size) in &files {
        if let Some(s) = file.to_str() {
            s.hash(hasher);
        }
        size.hash(hasher);
    }
    for sub in subdirs {
        hash_tree_signature(&sub, hasher);
    }
}

const DISCOVERY_CACHE_FILE: &str = "discovery.cache";

fn cache_dir() -> PathBuf {
    crate::utils::paths::cache().into()
}

fn discovery_cache_path() -> PathBuf {
    cache_dir().join(DISCOVERY_CACHE_FILE)
}

pub fn discovery_key(active_roots: &[PathBuf]) -> u64 {
    let mut hasher = DefaultHasher::new();
    env!("CARGO_PKG_VERSION").hash(&mut hasher);

    let mut roots: Vec<&PathBuf> = active_roots.iter().collect();
    roots.sort();
    for root in roots {
        root.hash(&mut hasher);
        hash_tree_signature(root, &mut hasher);
    }
    hasher.finish()
}

pub fn load_discovery(key: u64) -> Option<DiscoveryResult> {
    let bytes = std::fs::read(discovery_cache_path()).ok()?;
    if bytes.len() < 8 {
        return None;
    }
    let mut kb = [0u8; 8];
    kb.copy_from_slice(&bytes[..8]);
    if u64::from_le_bytes(kb) != key {
        return None;
    }
    bincode::deserialize::<DiscoveryResult>(&bytes[8..]).ok()
}

pub fn save_discovery(key: u64, result: &DiscoveryResult) {
    let mut bytes = key.to_le_bytes().to_vec();
    match bincode::serialize(result) {
        Ok(payload) => bytes.extend_from_slice(&payload),
        Err(e) => {
            warn!("failed to serialize discovery cache: {:?}", e);
            return;
        },
    }
    if let Err(e) = std::fs::write(discovery_cache_path(), &bytes) {
        warn!("failed to write discovery cache: {:?}", e);
    }
}

pub fn ensure_cache_dir() {
    let _ = std::fs::create_dir_all(cache_dir());
}
