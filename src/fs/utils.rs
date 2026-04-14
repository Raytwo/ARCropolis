use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    path::{Path, PathBuf},
};

use arc_config::ToExternal;
use orbits::{FileLoader, Tree};
use smash_arc::Hash40;

use super::{ApiCallback, ApiLoader};
use crate::{hashes, PathExtension};

/// Single tree walk that builds hash maps (size + path) and collects nus3bank dependencies.
///
/// Regional variant priority: if a regional file (containing '+') is found, it takes priority
/// over the non-regional variant. See original comment by blujay for details.
pub fn make_hash_maps_and_nus3bank_deps<L: FileLoader>(
    tree: &Tree<L>,
    unshare_blacklist: &[hash40::Hash40],
) -> (HashMap<Hash40, usize>, HashMap<Hash40, PathBuf>, HashSet<PathBuf>)
where
    <L as FileLoader>::ErrorType: Debug,
{
    let mut regional_overrides = HashSet::new();
    let mut size_map = HashMap::new();
    let mut path_map = HashMap::new();
    let mut nus3audio_deps = HashSet::new();
    let mut nus3banks_found = HashSet::new();

    tree.walk_paths(|node, ty| {
        if !ty.is_file() {
            return;
        }

        let local = node.get_local();

        // Collect nus3bank dependency info for non-stream files
        if !local.is_stream() {
            if local.has_extension("nus3audio") {
                if let Ok(hash) = local.smash_hash() {
                    if !unshare_blacklist.contains(&hash.to_external()) {
                        nus3audio_deps.insert(local.with_extension("nus3bank"));
                    }
                }
            } else if local.has_extension("nus3bank") {
                nus3banks_found.insert(local.to_path_buf());
            }
        }

        // Use cached size from discovery (falls back to loader if not cached)
        if let Some(size) = tree.query_filesize(local) {
            match local.smash_hash() {
                Ok(hash) => {
                    if regional_overrides.contains(&hash) {
                        return;
                    }

                    let is_regional_variant = if let Some(node) = local.to_str() { node.contains('+') } else { false };

                    size_map.insert(hash, size);
                    path_map.insert(hash, local.to_path_buf());

                    if is_regional_variant {
                        regional_overrides.insert(hash);
                    }
                },
                Err(e) => error!("Failed to get hash for {}. Reason: {:?}", local.display(), e),
            }
        } else {
            error!("Failed to stat file {}. This file may have issues.", node.full_path().display());
        }
    });

    // Remove nus3bank deps that have actual nus3bank files present
    for bank in nus3banks_found.into_iter() {
        nus3audio_deps.remove(&bank);
    }

    (size_map, path_map, nus3audio_deps)
}

pub fn add_file_to_api_tree<P: AsRef<Path>, Q: AsRef<Path>>(
    tree: &mut Tree<ApiLoader>,
    root: P,
    local: Q,
    callback_kind: ApiCallback,
) -> Option<Hash40> {
    let root = root.as_ref();
    let local = local.as_ref();
    match local.smash_hash() {
        Ok(hash) => {
            tree.insert_file(root, local);
            tree.loader.push_entry(hash, root, callback_kind);
            Some(hash)
        },
        Err(e) => {
            error!("Could not add file {} to API tree. Reason: {:?}", local.display(), e);
            None
        },
    }
}

/// Adds a PRC patch file and information to the API loader
pub fn add_prc_patch<P: AsRef<Path>, Q: AsRef<Path>>(tree: &mut Tree<ApiLoader>, phys_root: P, local: Q) -> Option<Hash40> {
    let local = local.as_ref();
    let base_local = if local.has_extension("prcx") || local.has_extension("prcxml") {
        // patch files have different extensions
        local.with_extension("prc")
    } else if local.has_extension("stdatx") || local.has_extension("stdatxml") {
        local.with_extension("stdat")
    } else if local.has_extension("stprmx") || local.has_extension("stprmxml") {
        local.with_extension("stprm")
    } else {
        unreachable!()
    };
    let base_local = if let Some(name) = base_local.file_name().and_then(|os_str| os_str.to_str()) {
        if let Some(idx) = name.find('+') {
            let mut new_name = name.to_string();
            new_name.replace_range(idx..idx + 6, "");
            base_local.with_file_name(new_name)
        } else {
            base_local
        }
    } else {
        base_local
    };
    let full_path = phys_root.as_ref().join(local); // need the full path so that our API loader can load it
    match base_local.smash_hash() {
        Ok(hash) => {
            tree.insert_file("api:/patch-prc", &base_local);
            tree.loader.push_entry(hash, Path::new("api:/patch-prc"), ApiCallback::None);
            // We need to add our file to the vector of patch files
            tree.loader.insert_prc_patch(hash, &full_path);
            if let Some(local) = local.to_str() {
                hashes::add(local);
            }
            if let Some(base_local) = base_local.to_str() {
                hashes::add(base_local);
            }
            Some(hash)
        },
        Err(e) => {
            error!("Could not add file {} to API tree. Reason: {:?}", full_path.display(), e);
            None
        },
    }
}

/// Adds a MSBT patch file and information to the API loader
pub fn add_msbt_patch<P: AsRef<Path>, Q: AsRef<Path>>(tree: &mut Tree<ApiLoader>, phys_root: P, local: Q) -> Option<Hash40> {
    let local = local.as_ref();
    let base_local = local.with_extension("msbt"); // patch files have different extensions
    let mut is_current_region = true;

    let base_local = if let Some(name) = base_local.file_name().and_then(|os_str| os_str.to_str()) {
        if let Some(idx) = name.find('+') {
            is_current_region = (&name[idx + 1..idx + 6] == format!("{}", config::region())); //Check if XMSBT's region is current region
            let mut new_name = name.to_string();
            new_name.replace_range(idx..idx + 6, "");
            base_local.with_file_name(new_name)
        } else {
            base_local
        }
    } else {
        base_local
    };
    let full_path = phys_root.as_ref().join(local); // need the full path so that our API loader can load it
    match base_local.smash_hash() {
        Ok(hash) => {
            tree.insert_file("api:/patch-msbt", &base_local);
            tree.loader.push_entry(hash, Path::new("api:/patch-msbt"), ApiCallback::None);
            // We need to add our file to the vector of patch files
            tree.loader.insert_msbt_patch(hash, &full_path);
            if let Some(local) = local.to_str() {
                hashes::add(local);
            }
            if let Some(base_local) = base_local.to_str() {
                if is_current_region {
                    hashes::add(base_local);
                }
            }
            Some(hash)
        },
        Err(e) => {
            error!("Could not add file {} to API tree. Reason: {:?}", full_path.display(), e);
            None
        },
    }
}

pub fn add_nus3audio_patch<P: AsRef<Path>, Q: AsRef<Path>>(tree: &mut Tree<ApiLoader>, phys_root: P, local: Q) -> Option<Hash40> {
    let local = local.as_ref();
    let base_local = local.with_extension("nus3audio");

    let base_local = if let Some(name) = base_local.file_name().and_then(|os_str| os_str.to_str()) {
        if let Some(idx) = name.find('+') {
            let mut new_name = name.to_string();
            new_name.replace_range(idx..idx + 6, "");
            base_local.with_file_name(new_name)
        } else {
            base_local
        }
    } else {
        base_local
    };
    let full_path = phys_root.as_ref().join(local); // need the full path so that our API loader can load it
    match base_local.smash_hash() {
        Ok(hash) => {
            tree.insert_file("api:/patch-nus3audio", &base_local);
            tree.loader.push_entry(hash, Path::new("api:/patch-nus3audio"), ApiCallback::None);
            // We need to add our file to the vector of patch files
            tree.loader.insert_nus3audio_patch(hash, &full_path);
            if let Some(local) = local.to_str() {
                hashes::add(local);
            }
            if let Some(base_local) = base_local.to_str() {
                hashes::add(base_local);
            }
            Some(hash)
        },
        Err(e) => {
            error!("Could not add file {} to API tree. Reason: {:?}", full_path.display(), e);
            None
        },
    }
}

pub fn add_motionlist_patch<P: AsRef<Path>, Q: AsRef<Path>>(tree: &mut Tree<ApiLoader>, phys_root: P, local: Q) -> Option<Hash40> {
    let local = local.as_ref();
    let base_local = local.with_extension("bin");

    let base_local = if let Some(name) = base_local.file_name().and_then(|os_str| os_str.to_str()) {
        if let Some(idx) = name.find('+') {
            let mut new_name = name.to_string();
            new_name.replace_range(idx..idx + 6, "");
            base_local.with_file_name(new_name)
        } else {
            base_local
        }
    } else {
        base_local
    };
    let full_path = phys_root.as_ref().join(local); // need the full path so that our API loader can load it
    if let Some(name) = full_path.file_name() {
        if name.to_str().unwrap().contains(&"motion_list") {
            match base_local.smash_hash() {
                Ok(hash) => {
                    tree.insert_file("api:/patch-motionlist", &base_local);
                    tree.loader.push_entry(hash, Path::new("api:/patch-motionlist"), ApiCallback::None);
                    // We need to add our file to the vector of patch files
                    tree.loader.insert_motionlist_patch(hash, &full_path);
                    if let Some(local) = local.to_str() {
                        hashes::add(local);
                    }
                    if let Some(base_local) = base_local.to_str() {
                        hashes::add(base_local);
                    }
                    return Some(hash);
                },
                Err(e) => {
                    error!("Could not add file {} to API tree. Reason: {:?}", full_path.display(), e);
                    return None;
                },
            }
        }
    }
    error!(
        "Could not add file {} to API tree. Reason: This is not a motion_list.bin file.",
        full_path.display()
    );
    None
}

pub fn add_bgm_property_patch<P: AsRef<Path>, Q: AsRef<Path>>(tree: &mut Tree<ApiLoader>, phys_root: P, local: Q) -> Option<Hash40> {
    let local = local.as_ref();
    let base_local = local.with_extension("bin");

    let base_local = if let Some(name) = base_local.file_name().and_then(|os_str| os_str.to_str()) {
        if let Some(idx) = name.find('+') {
            let mut new_name = name.to_string();
            new_name.replace_range(idx..idx + 6, "");
            base_local.with_file_name(new_name)
        } else {
            base_local
        }
    } else {
        base_local
    };
    let full_path = phys_root.as_ref().join(local); // need the full path so that our API loader can load it
    if let Some(name) = full_path.file_name() {
        if name.to_str().unwrap().contains(&"bgm_property") {
            match base_local.smash_hash() {
                Ok(hash) => {
                    tree.insert_file("api:/patch-bgm_property", &base_local);
                    tree.loader.push_entry(hash, Path::new("api:/patch-bgm_property"), ApiCallback::None);
                    // We need to add our file to the vector of patch files
                    tree.loader.insert_bgm_property_patch(hash, &full_path);
                    if let Some(local) = local.to_str() {
                        hashes::add(local);
                    }
                    if let Some(base_local) = base_local.to_str() {
                        hashes::add(base_local);
                    }
                    return Some(hash);
                },
                Err(e) => {
                    error!("Could not add file {} to API tree. Reason: {:?}", full_path.display(), e);
                    return None;
                },
            }
        }
    }
    error!(
        "Could not add file {} to API tree. Reason: This is not a bgm_property.bin file.",
        full_path.display()
    );
    None
}
