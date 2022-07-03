// use std::{
//     collections::{HashMap, HashSet},
//     path::{Path},
// };

// use serde::{Deserialize, de::{Visitor, MapAccess, Error}};
// use smash_arc::{serde::Hash40String, Hash40};

// use crate::PathExtension;

// struct FolderVisitor;

// impl<'de> Visitor<'de> for FolderVisitor {
//     type Value = Folder;

//     fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
//         formatter.write_str("a valid folder path structure")
//     }

//     fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
//         where
//             A: MapAccess<'de>,
//     {
//         let mut full_path = Hash40String(Hash40(u64::MAX));
//         let mut name = None;
//         let mut parent = None;

//         while let Some(k) = map.next_key::<String>()? {
//             match k.as_str() {
//                 "full_path" | "full-path" => full_path = map.next_value::<Hash40String>()?,
//                 "name" => name = Some(map.next_value::<Hash40String>()?),
//                 "parent" => parent = Some(Box::new(map.next_value::<Folder>()?)),
//                 _ => return Err(A::Error::custom("expected member of folder path structure"))
//             }
//         }

//         Ok(Folder {
//             full_path,
//             name,
//             parent
//         })
//     }

//     fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
//         where
//             E: Error,
//     {
//         if v.starts_with("0x") {
//             let val = u64::from_str_radix(v.trim_start_matches("0x"), 16)
//                 .map_err(|_err| E::custom("invalid integer type for hash"))?;
            
//             return Ok(Folder {
//                 full_path: Hash40String(Hash40(val)),
//                 name: None,
//                 parent: None
//             });
//         }

//         let path = Path::new(v);

//         let full_path = path
//             .smash_hash()
//             .map_or_else(
//                 |_err| Err(E::custom("unable to get hash from path")),
//                 |hash| Ok(Hash40String(hash))
//             )?;

//         let parent = path
//             .parent()
//             .and_then(|parent| parent.to_str())
//             .ok_or_else(|| E::custom("path does not have a valid parent"))?;

//         let parent = if parent.is_empty() || parent == "/" {
//             None
//         } else {
//             FolderVisitor.visit_str::<E>(parent).ok().map(Box::new)
//         };

//         let name = path
//             .file_name()
//             .map(Path::new)
//             .ok_or_else(|| E::custom("path does not have a name"))?
//             .smash_hash()
//             .map_or_else(
//                 |_err| Err(E::custom("path contains invalid characters for hash")),
//                 |hash| Ok(Hash40String(hash))
//             )?;

//         Ok(Folder {
//             full_path,
//             parent,
//             name: Some(name)
//         })
//     }
// }

// #[derive(Debug)]
// pub struct Folder {
//     pub full_path: Hash40String,
//     pub name: Option<Hash40String>,
//     pub parent: Option<Box<Folder>>,
// }

// impl<'de> Deserialize<'de> for Folder {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//         where
//             D: serde::Deserializer<'de> {
//         deserializer.deserialize_any(FolderVisitor)
//     }
// }

// struct NewFileSetVisitor;

// impl<'de> Deserialize<'de> for NewFileSet {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//         where
//             D: serde::Deserializer<'de>
//     {
//         deserializer.deserialize_any(NewFileSetVisitor)    
//     }
// }

// #[derive(Debug)]
// pub struct NewFileSet(pub Vec<NewFile>);

// impl<'de> Visitor<'de> for NewFileSetVisitor {
//     type Value = NewFileSet;

//     fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
//         formatter.write_str("At least one complete file structure")
//     }

//     fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
//         where
//             A: serde::de::SeqAccess<'de>,
//     {
//         let mut vec = vec![];
        
//         while let Some(element) = seq.next_element::<NewFile>()? {
//             vec.push(element);
//         }

//         Ok(NewFileSet(vec))
//     }

//     fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
//         where
//             E: Error,
//     {
//         Ok(NewFileSet(vec![NewFileVisitor.visit_str(v)?]))
//     }

//     fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
//         where
//             A: MapAccess<'de>,
//     {
//         Ok(NewFileSet(vec![NewFileVisitor.visit_map(map)?]))    
//     }
// }

// struct NewFileVisitor;

// #[derive(Debug)]
// pub struct NewFile {
//     pub full_path: Hash40String,
//     pub file_name: Hash40String,
//     pub parent: Folder,
//     pub extension: Hash40String
// }

// impl<'de> Visitor<'de> for NewFileVisitor {
//     type Value = NewFile;

//     fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
//         formatter.write_str("A complete file structure")
//     }

//     fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
//         where
//             A: serde::de::MapAccess<'de>,
//     {
//         const INVALID: Hash40String = Hash40String(Hash40(u64::MAX));
//         let mut full_path = INVALID;
//         let mut file_name = INVALID;
//         let mut parent = None;
//         let mut extension = INVALID;

//         while let Some(key) = map.next_key::<String>()? {
//             match key.as_str() {
//                 "full-path" | "full_path" => full_path = map.next_value::<Hash40String>()?,
//                 "file-name" | "file_name" => file_name = map.next_value::<Hash40String>()?,
//                 "extension" => extension = map.next_value::<Hash40String>()?,
//                 "parent" => parent = Some(map.next_value::<Folder>()?),
//                 _ => return Err(A::Error::custom("expected member of file path structure"))
//             }
//         }

//         if full_path == INVALID {
//             return Err(A::Error::custom("file map is missing file path"));
//         }

//         if file_name == INVALID {
//             return Err(A::Error::custom("file map is missing file name"));
//         }

//         if parent.is_none() {
//             return Err(A::Error::custom("file map is missing parent"));
//         }

//         if extension == INVALID {
//             return Err(A::Error::custom("file map is missing extension"));
//         }

//         Ok(NewFile {
//             full_path,
//             file_name,
//             parent: parent.unwrap(),
//             extension
//         })
//     }

//     fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
//         where
//             E: Error,
//     {
//         let path = Path::new(v);

//         let full_path = path.smash_hash()
//             .map_err(|_err| E::custom("file path contained invalid characters"))?;

//         let parent = path
//         .parent()
//         .and_then(|parent| parent.to_str())
//         .ok_or_else(|| E::custom("path does not have a valid parent"))?;

//         let parent = if parent.is_empty() || parent == "/" {
//             return Err(E::custom("path does not have a valid parent"));
//         } else {
//             FolderVisitor.visit_str::<E>(parent)?
//         };

//         let file_name = path
//             .file_name()
//             .map_or_else(
//                 || Err(E::custom("file name does not exist")),
//                 |file_name| Ok(Path::new(file_name))
//             )?
//             .smash_hash()
//             .map_err(|_err| E::custom("file name contained invalid data"))?;

//         let extension = path
//             .extension()
//             .map_or_else(
//                 || Err(E::custom("file name does not exist")),
//                 |file_name| Ok(Path::new(file_name))
//             )?
//             .smash_hash()
//             .map_err(|_err| E::custom("file name contained invalid data"))?;

//         Ok(NewFile {
//             full_path: Hash40String(full_path),
//             file_name: Hash40String(file_name),
//             parent,
//             extension: Hash40String(extension)
//         })
        
//     }
// }

// impl<'de> Deserialize<'de> for NewFile {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//         where
//             D: serde::Deserializer<'de> {
//         deserializer.deserialize_any(NewFileVisitor)
//     }
// }

// #[derive(Debug, Default, Deserialize)]
// pub struct ModConfig {
//     #[serde(alias = "unshare-blacklist")]
//     #[serde(default = "HashSet::new")]
//     pub unshare_blacklist: HashSet<Hash40String>,

//     #[serde(alias = "new-files")]
//     #[serde(default = "HashMap::new")]
//     pub new_files: HashMap<Hash40String, Option<HashSet<Hash40String>>>,

//     #[serde(alias = "preprocess-reshare")]
//     #[serde(default = "HashMap::new")]
//     pub preprocess_reshare: HashMap<Hash40String, Hash40String>,

//     #[serde(alias = "preprocess-reshare-ext")]
//     #[serde(default = "HashMap::new")]
//     pub preprocess_reshare_ext: HashMap<Hash40String, Hash40String>,

//     #[serde(alias = "share-to-vanilla")]
//     #[serde(default = "HashMap::new")]
//     pub share_to_vanilla: HashMap<Hash40String, NewFileSet>,

//     #[serde(alias = "share-to-added")]
//     #[serde(alias = "new-shared-files")]
//     #[serde(alias = "new_shared_files")]
//     #[serde(default = "HashMap::new")]
//     pub share_to_added: HashMap<Hash40String, NewFileSet>,

//     #[serde(alias = "new-dir-files")]
//     #[serde(default = "HashMap::new")]
//     pub new_dir_files: HashMap<Hash40String, HashSet<Hash40String>>,
// }

// impl ModConfig {
//     pub fn merge(&mut self, other: ModConfig) {
//         let Self {
//             unshare_blacklist,
//             new_files,
//             preprocess_reshare,
//             preprocess_reshare_ext,
//             share_to_vanilla,
//             share_to_added,
//             new_dir_files,
//         } = other;

//         self.unshare_blacklist.extend(unshare_blacklist.into_iter());
//         self.preprocess_reshare.extend(preprocess_reshare.into_iter());
//         self.preprocess_reshare_ext.extend(preprocess_reshare_ext.into_iter());

//         for (hash, list) in new_files.into_iter() {
//             if let Some(list) = list {
//                 if let Some(Some(current_list)) = self.new_files.get_mut(&hash) {
//                     current_list.extend(list.into_iter());
//                 } else {
//                     let _ = self.new_files.insert(hash, Some(list));
//                 }
//             }
//         }

//         for (hash, list) in new_dir_files.into_iter() {
//             if let Some(current_list) = self.new_dir_files.get_mut(&hash) {
//                 current_list.extend(list.into_iter());
//             } else {
//                 let _ = self.new_dir_files.insert(hash, list);
//             }
//         }

//         self.share_to_vanilla.extend(share_to_vanilla);
//         self.share_to_added.extend(share_to_added);
//     }
// }
