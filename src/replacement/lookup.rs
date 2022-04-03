use std::{
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use smash_arc::{ArcLookup, Hash40, LoadedArc};

use super::LoadedArcEx;
use crate::hashes;

// FilePath -> (DirInfo, child_index)
#[derive(Deserialize, Serialize)]
pub struct UnshareLookup(HashMap<Hash40, (Hash40, usize)>);

#[derive(Deserialize, Serialize)]
pub struct ShareLookup {
    pub is_shared_search: HashSet<Hash40>,
    pub shared_file_lookup: HashMap<Hash40, Vec<Hash40>>,
}

impl Deref for UnshareLookup {
    type Target = HashMap<Hash40, (Hash40, usize)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for UnshareLookup {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

enum UnshareLookupState {
    Missing,
    Generated(UnshareLookup),
}

enum ShareLookupState {
    Missing,
    Generated(ShareLookup),
}

lazy_static! {
    static ref UNSHARE_LOOKUP: RwLock<UnshareLookupState> = {
        let path = crate::CACHE_PATH.join("unshare.lut");
        let lut = match std::fs::read(&path) {
            Ok(data) => {
                match bincode::deserialize(&data) {
                    Ok(lut) => UnshareLookupState::Generated(lut),
                    Err(e) => {
                        error!(
                            "Unable to parse '{}' for unsharing. Reason: {:?}. Boot time might be a bit slow.",
                            path.display(),
                            *e
                        );
                        UnshareLookupState::Missing
                    },
                }
            },
            Err(err) => {
                error!("Unable to read '{}'. Reason: {:?}", path.display(), err);
                UnshareLookupState::Missing
            },
        };

        RwLock::new(lut)
    };
    static ref SHARE_LOOKUP: RwLock<ShareLookupState> = {
        let path = crate::CACHE_PATH.join("share.lut");
        let lut = match std::fs::read(&path) {
            Ok(data) => {
                match bincode::deserialize(&data) {
                    Ok(lut) => ShareLookupState::Generated(lut),
                    Err(e) => {
                        error!(
                            "Unable to parse '{}' for share lookup. Reason: {:?}. Boot time might be a bit slow.",
                            path.display(),
                            *e
                        );
                        ShareLookupState::Missing
                    },
                }
            },
            Err(err) => {
                error!("Unable to read '{}'. Reason: {:?}", path.display(), err);
                ShareLookupState::Missing
            },
        };

        RwLock::new(lut)
    };
}

pub fn initialize_unshare(arc: Option<&LoadedArc>) {
    if arc.is_none() {
        lazy_static::initialize(&UNSHARE_LOOKUP);
        return
    }
    let arc = arc.unwrap();
    let mut lookup_state = UNSHARE_LOOKUP.write();
    let lookup = match *lookup_state {
        UnshareLookupState::Missing => {
            let mut lookup = UnshareLookup(HashMap::new());

            let file_paths = arc.get_file_paths();

            for dir_info in arc.get_dir_infos() {
                for (child_index, file_info) in arc.get_file_infos()[dir_info.file_info_range()].iter().enumerate() {
                    lookup.insert(file_paths[file_info.file_path_index].path.hash40(), (dir_info.path.hash40(), child_index));
                }
            }

            match bincode::serialize(&lookup) {
                Ok(data) => {
                    let path = crate::CACHE_PATH.join("unshare.lut");
                    if let Err(e) = std::fs::write(&path, data) {
                        error!("Failed to write unshare LUT to cache file at '{}'. Reason: {:?}", path.display(), e);
                    }
                },
                Err(e) => {
                    error!("Failed to serialize unshare LUT into bytes. Reason: {:?}", *e);
                },
            }

            UnshareLookupState::Generated(lookup)
        },
        UnshareLookupState::Generated(_) => return,
    };
    *lookup_state = lookup;
}

pub fn initialize_share(arc: Option<&LoadedArc>) {
    if arc.is_none() {
        lazy_static::initialize(&SHARE_LOOKUP);
        return
    }

    let arc = arc.unwrap();
    let mut lookup_state = SHARE_LOOKUP.write();
    let lookup = match *lookup_state {
        ShareLookupState::Missing => {
            let mut path_shared: HashMap<Hash40, Vec<Hash40>> = HashMap::new();

            let mut shared_files = HashSet::new();

            let filepaths = arc.get_file_paths();

            for (current_index, file_path) in filepaths.iter().enumerate() {
                let hash = file_path.path.hash40();

                let shared_file_index = match arc.get_shared_file(hash) {
                    Ok(idx) => {
                        if usize::from(idx) == current_index {
                            continue
                        }
                        idx
                    },
                    Err(_) => {
                        error!(
                            "Failed to get shared file for '{}' ({:#x}) while generating share.lut",
                            hashes::find(hash),
                            hash.0
                        );
                        continue
                    },
                };

                let shared_hash = filepaths[shared_file_index].path.hash40();

                if let Some(shared_file_hashes) = path_shared.get_mut(&shared_hash) {
                    shared_file_hashes.push(hash);
                } else {
                    let _ = path_shared.insert(shared_hash, vec![hash]);
                }
            }

            for (src, shared) in path_shared.iter() {
                shared_files.insert(*src);
                for share in shared {
                    shared_files.insert(*share);
                }
            }

            let lookup = ShareLookup {
                is_shared_search: shared_files,
                shared_file_lookup: path_shared,
            };

            match bincode::serialize(&lookup) {
                Ok(data) => {
                    let path = crate::CACHE_PATH.join("share.lut");
                    if let Err(e) = std::fs::write(&path, data) {
                        error!("Failed to write share LUT to cache file at '{}'. Reason: {:?}", path.display(), e);
                    }
                },
                Err(e) => {
                    error!("Failed to serialize share LUT into bytes. Reason: {:?}", *e);
                },
            }

            ShareLookupState::Generated(lookup)
        },
        ShareLookupState::Generated(_) => return,
    };
    *lookup_state = lookup;
}

pub fn initialize(arc: Option<&LoadedArc>) {
    initialize_unshare(arc);
    initialize_share(arc);
}

pub fn get_dir_entry_for_file<H: Into<Hash40>>(hash: H) -> Option<(Hash40, usize)> {
    let lut = UNSHARE_LOOKUP.read();
    match &*lut {
        UnshareLookupState::Generated(lut) => lut.get(&hash.into()).map(|x| *x),
        _ => None,
    }
}

pub fn is_shared_file<H: Into<Hash40>>(hash: H) -> bool {
    let lut = SHARE_LOOKUP.read();
    match &*lut {
        ShareLookupState::Generated(lut) => lut.is_shared_search.contains(&hash.into()),
        _ => false,
    }
}

pub fn add_shared_file<H: Into<Hash40>>(hash: H, shared_to: H) {
    let mut lut = SHARE_LOOKUP.write();
    match &mut *lut {
        ShareLookupState::Generated(lut) => {
            let shared_to = shared_to.into();
            let hash = hash.into();
            lut.is_shared_search.insert(shared_to);
            if let Some(list) = lut.shared_file_lookup.get_mut(&hash) {
                list.push(hash);
            } else {
                lut.shared_file_lookup.insert(shared_to, vec![hash]);
            }
        },
        _ => {},
    }
}

pub fn remove_shared_file<H: Into<Hash40>>(hash: H) -> bool {
    let mut lut = SHARE_LOOKUP.write();
    match &mut *lut {
        ShareLookupState::Generated(lut) => lut.is_shared_search.remove(&hash.into()),
        _ => false,
    }
}

pub fn get_shared_file_count<H: Into<Hash40>>(hash: H) -> usize {
    let lut = SHARE_LOOKUP.read();
    match &*lut {
        ShareLookupState::Generated(lut) => lut.shared_file_lookup.get(&hash.into()).map_or_else(|| 0, |hashes| hashes.len()),
        _ => 0,
    }
}

pub fn get_shared_file<H: Into<Hash40>>(hash: H, index: usize) -> Option<Hash40> {
    let lut = SHARE_LOOKUP.read();
    match &*lut {
        ShareLookupState::Generated(lut) => {
            lut.shared_file_lookup
                .get(&hash.into())
                .map_or_else(|| None, |hashes| hashes.get(index).map(|hash| *hash))
        },
        _ => None,
    }
}
