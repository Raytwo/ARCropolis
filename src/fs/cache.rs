use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::PathBuf,
};

use smash_arc::Region;

use super::discover::{DiscoveryResult, RootWalk};

const DISCOVERY_CACHE_FILE: &str = "discovery.cache";

fn cache_dir() -> PathBuf {
    crate::utils::paths::cache().into()
}

fn discovery_cache_path() -> PathBuf {
    cache_dir().join(DISCOVERY_CACHE_FILE)
}

pub fn discovery_key(region: Region, walked: &[(PathBuf, RootWalk)]) -> u64 {
    let mut hasher = DefaultHasher::new();
    env!("CARGO_PKG_VERSION").hash(&mut hasher);
    (region as u32).hash(&mut hasher);

    for (root, walk) in walked {
        root.hash(&mut hasher);
        let mut folded: u64 = 0;
        for (local, size) in walk.staged_tree.iter().chain(walk.staged_collected.iter()) {
            let mut entry = DefaultHasher::new();
            local.hash(&mut entry);
            size.hash(&mut entry);
            folded = folded.wrapping_add(entry.finish());
        }
        folded.hash(&mut hasher);
        (walk.staged_tree.len() + walk.staged_collected.len()).hash(&mut hasher);
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
