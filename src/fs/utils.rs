use std::{collections::{HashMap, HashSet}, path::{PathBuf, Path}};
use std::fmt::Debug;
use smash_arc::Hash40;
use orbits::{Tree, FileLoader};
use smash_arc::serde::Hash40String;
use crate::PathExtension;

use super::{ApiLoader, ApiCallback};

pub fn make_hash_maps<L: FileLoader>(tree: &Tree<L>) -> (HashMap<Hash40, usize>, HashMap<Hash40, PathBuf>)
where
    <L as FileLoader>::ErrorType: Debug
{
    let mut size_map = HashMap::new();
    let mut path_map = HashMap::new();
    tree.walk_paths(|node, ty| {
        if !ty.is_file() {
            return;
        }

        if let Some(size) = tree.query_filesize(node.get_local()) {
            match node.get_local().smash_hash() {
                Ok(hash) => {
                    size_map.insert(hash, size);
                    path_map.insert(hash, node.get_local().to_path_buf());
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