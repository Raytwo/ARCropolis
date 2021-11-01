use smash_arc::{ArcLookup, Hash40, LoadedArc};
use std::collections::{HashMap, HashSet};
use parking_lot::RwLock;
use std::ops::{Deref, DerefMut};
use serde::{Deserialize, Serialize};

// FilePath -> (DirInfo, child_index)
#[derive(Deserialize, Serialize)]
pub struct UnshareLookup(HashMap<Hash40, (Hash40, usize)>);

#[derive(Deserialize, Serialize)]
pub struct ShareLookup(HashSet<Hash40>);

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

impl Deref for ShareLookup {
    type Target = HashSet<Hash40>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ShareLookup {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

enum UnshareLookupState {
    Missing,
    Generated(UnshareLookup)
}

enum ShareLookupState {
    Missing,
    Generated(ShareLookup)
}

lazy_static! {
    static ref UNSHARE_LOOKUP: RwLock<UnshareLookupState> = {
        let path = crate::CACHE_PATH.join("unshare.lut");
        let lut = match std::fs::read(&path) {
            Ok(data) => {
                match bincode::deserialize(&data) {
                    Ok(lut) => UnshareLookupState::Generated(lut),
                    Err(e) => {
                        error!("Unable to parse '{}' for unsharing. Reason: {:?}. Boot time might be a bit slow.", path.display(), *e);
                        UnshareLookupState::Missing
                    }
                }
            },
            Err(err) => {
                error!("Unable to read '{}'. Reason: {:?}", path.display(), err);
                UnshareLookupState::Missing
            }
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
                        error!("Unable to parse '{}' for share lookup. Reason: {:?}. Boot time might be a bit slow.", path.display(), *e);
                        ShareLookupState::Missing
                    }
                }
            },
            Err(err) => {
                error!("Unable to read '{}'. Reason: {:?}", path.display(), err);
                ShareLookupState::Missing
            }
        };

        RwLock::new(lut)
    };
}

pub fn initialize_unshare(arc: Option<&LoadedArc>) {
    if arc.is_none() {
        lazy_static::initialize(&UNSHARE_LOOKUP);
        return;
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
                }
            }
    
            UnshareLookupState::Generated(lookup)
        },
        UnshareLookupState::Generated(_) => {
            return;
        }
    };
    *lookup_state = lookup;
}

pub fn initialize_share(arc: Option<&LoadedArc>) {
    if arc.is_none() {
        lazy_static::initialize(&SHARE_LOOKUP);
        return;
    }

    let arc = arc.unwrap();
    let mut lookup_state = SHARE_LOOKUP.write();
    let lookup = match *lookup_state {
        ShareLookupState::Missing => {
            let mut lookup = ShareLookup(HashSet::new());

            let mut info_index_ref_count: HashMap<u32, usize> = HashMap::new();

            for file_path in arc.get_file_paths().iter() {
                if let Some(count) = info_index_ref_count.get_mut(&file_path.path.index()) {
                    *count += 1;
                } else {
                    info_index_ref_count.insert(file_path.path.index(), 1);
                }
            }

            for file_path in arc.get_file_paths().iter() {
                match info_index_ref_count.get(&file_path.path.index()) {
                    Some(count) if *count > 1 => {
                        lookup.insert(file_path.path.hash40());
                    },
                    _ => {}
                }
            }

            match bincode::serialize(&lookup) {
                Ok(data) => {
                    let path = crate::CACHE_PATH.join("share.lut");
                    if let Err(e) = std::fs::write(&path, data) {
                        error!("Failed to write share LUT to cache file at '{}'. Reason: {:?}", path.display(), e);
                    }
                },
                Err(e) => {
                    error!("Failed to serialize share LUT into bytes. Reason: {:?}", *e);
                }
            }

            ShareLookupState::Generated(lookup)
        },
        ShareLookupState::Generated(_) => {
            return;
        }
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
        ShareLookupState::Generated(lut) => lut.contains(&hash.into()),
        _ => false
    }
}