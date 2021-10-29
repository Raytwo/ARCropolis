use smash_arc::{ArcLookup, Hash40, LoadedArc};
use std::collections::HashMap;
use parking_lot::RwLock;
use std::ops::{Deref, DerefMut};
use serde::{Deserialize, Serialize, de::Visitor, ser::SerializeMap};

// FilePath -> (DirInfo, child_index)
pub struct UnshareLookup(HashMap<Hash40, (Hash40, usize)>);

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

impl Serialize for UnshareLookup {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
            S: serde::Serializer {
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (path, (dir, idx)) in self.iter() {
            map.serialize_entry(&path.0, &(dir.0, *idx))?;
        }
        map.end()
    }
}

struct UnshareLookupVisitor;

impl<'de> Visitor<'de> for UnshareLookupVisitor {
    type Value = UnshareLookup;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("An Unshare LUT")
    }

    fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
            A: serde::de::MapAccess<'de>, {
        let mut map = UnshareLookup(HashMap::with_capacity(access.size_hint().unwrap_or(0)));

        while let Some((path, (dir, idx))) = access.next_entry::<u64, (u64, usize)>()? {
            map.insert(Hash40(path), (Hash40(dir), idx));
        }

        Ok(map)
    }
}

impl<'de> Deserialize<'de> for UnshareLookup {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
            D: serde::Deserializer<'de> {
        deserializer.deserialize_map(UnshareLookupVisitor)
    }
}

enum UnshareLookupState {
    Missing,
    Generated(UnshareLookup)
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
}

pub fn initialize(arc: Option<&LoadedArc>) {
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