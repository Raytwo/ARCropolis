use std::{collections::{HashMap, HashSet}, path::{PathBuf, Path}};
use std::fmt::Debug;
use smash_arc::Hash40;
use orbits::{Tree, FileLoader};
use smash_arc::serde::Hash40String;
use crate::{PathExtension, hashes};

use super::{ApiLoader, ApiCallback};

pub fn make_hash_maps<L: FileLoader>(tree: &Tree<L>) -> (HashMap<Hash40, usize>, HashMap<Hash40, PathBuf>)
where
    <L as FileLoader>::ErrorType: Debug
{
    // This defines the previously undefined behavior of what happens when you have two files that overlap each other due to
    // regional things
    // I.E.: ui/message/msg_menu.msbt and ui/message/msg_menu+us_en.msbt
    // The regional variant should take priority. Since there can only be one regional file, there are only two situations which need to be handled:
    // 1.) ui/message/msg_menu.msbt is found and then ui/message/msg_menu+us_en.msbt is found. ui/message/msg_menu+us_en.msbt should overwrite the previous file
    // 2.) ui/message/msg_menu+us_en.msbt is found first, and when ui/message/msg_menu.msbt is found it should be discarded
    // To solve this I store the hash of every file which has a regional variant which has been found, and then if a non-regional variant is found
    // it is ignored
    // - blujay
    let mut regional_overrides = HashSet::new();
    let mut size_map = HashMap::new();
    let mut path_map = HashMap::new();
    tree.walk_paths(|node, ty| {
        if !ty.is_file() {
            return;
        }

        if let Some(size) = tree.query_filesize(node.get_local()) {
            match node.get_local().smash_hash() {
                Ok(hash) => {
                    if regional_overrides.contains(&hash) { return; }

                    let is_regional_variant = if let Some(node) = node.get_local().to_str() {
                        node.contains("+")
                    } else {
                        false
                    };

                    size_map.insert(hash, size);
                    path_map.insert(hash, node.get_local().to_path_buf());

                    if is_regional_variant {
                        regional_overrides.insert(hash);
                    }
                },
                Err(e) => error!("Failed to get hash for {}. Reason: {:?}", node.get_local().display(), e)
            }
        } else {
            error!("Failed to stat file {}. This file may have issues.", node.full_path().display());
        }
    });

    (size_map, path_map)
}

pub fn get_required_nus3banks<L: FileLoader>(tree: &Tree<L>, unshare_blacklist: &HashSet<Hash40String>) -> HashSet<PathBuf>
where
    <L as FileLoader>::ErrorType: Debug
{
    let mut nus3audio_deps = HashSet::new();
    let mut nus3banks_found = HashSet::new();
    tree.walk_paths(|node, ty| {
        if !ty.is_file() {
            return;
        }

        let local = node.get_local();
        if local.is_stream() {
            return;
        }

        if local.has_extension("nus3audio") {
            match local.smash_hash() {
                Ok(hash) if !unshare_blacklist.contains(&Hash40String(hash)) => {
                    nus3audio_deps.insert(local.with_extension("nus3bank"));
                },
                Err(e) => error!("Failed to get hash for path {}. Reason: {:?}", local.display(), e),
                _ => {}
            }
        } else if local.has_extension("nus3bank") {
            nus3banks_found.insert(local.to_path_buf());
        }
    });

    for bank in nus3banks_found.into_iter() {
        nus3audio_deps.remove(&bank);
    }

    nus3audio_deps
}

pub fn add_file_to_api_tree<P: AsRef<Path>, Q: AsRef<Path>>(tree: &mut Tree<ApiLoader>, root: P, local: Q, callback_kind: ApiCallback) -> Option<Hash40> {
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
        }
    }
}


/// Adds a PRC patch file and information to the API loader
pub fn add_prc_patch<P: AsRef<Path>, Q: AsRef<Path>>(tree: &mut Tree<ApiLoader>, phys_root: P, local: Q) -> Option<Hash40> {
    let local = local.as_ref();
    let base_local = if local.has_extension("prcx") || local.has_extension("prcxml") { // patch files have different extensions
        local.with_extension("prc")
    } else if local.has_extension("stdatx") || local.has_extension("stdatxml") {
        local.with_extension("stdat")
    } else if local.has_extension("stprmx") || local.has_extension("stprmxml") {
        local.with_extension("stprm")
    } else {
        unreachable!()
    };
    let base_local = if let Some(name) = base_local.file_name().map(|os_str| os_str.to_str()).flatten() {
        if let Some(idx) = name.find("+") {
            let mut new_name = name.to_string();
            new_name.replace_range(idx..idx+6, "");
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
            tree.insert_file("api:/patch-prc",  &base_local);
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
        }
    }
}

/// Adds a MSBT patch file and information to the API loader
pub fn add_msbt_patch<P: AsRef<Path>, Q: AsRef<Path>>(tree: &mut Tree<ApiLoader>, phys_root: P, local: Q) -> Option<Hash40> {
    let local = local.as_ref();
    let base_local = local.with_extension("msbt"); // patch files have different extensions
    let base_local = if let Some(name) = base_local.file_name().map(|os_str| os_str.to_str()).flatten() {
        if let Some(idx) = name.find("+") {
            let mut new_name = name.to_string();
            new_name.replace_range(idx..idx+6, "");
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
            tree.insert_file("api:/patch-msbt",  &base_local);
            tree.loader.push_entry(hash, Path::new("api:/patch-msbt"), ApiCallback::None);
            // We need to add our file to the vector of patch files
            tree.loader.insert_msbt_patch(hash, &full_path);
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
        }
    }
}