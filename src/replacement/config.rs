use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use serde::Deserialize;
use smash_arc::serde::Hash40String;

#[derive(Debug, Deserialize)]
pub struct ModConfig {
    #[serde(alias = "unshare-blacklist")]
    #[serde(default = "HashSet::new")]
    pub unshare_blacklist: HashSet<Hash40String>,

    #[serde(alias = "new-files")]
    #[serde(default = "HashMap::new")]
    pub new_files: HashMap<Hash40String, Option<HashSet<Hash40String>>>,

    #[serde(alias = "preprocess-reshare")]
    #[serde(default = "HashMap::new")]
    pub preprocess_reshare: HashMap<Hash40String, Hash40String>,

    #[serde(alias = "preprocess-reshare-ext")]
    #[serde(default = "HashMap::new")]
    pub preprocess_reshare_ext: HashMap<Hash40String, Hash40String>,

    #[serde(alias = "new-shared-files")]
    #[serde(default = "HashMap::new")]
    pub new_shared_files: HashMap<Hash40String, HashSet<PathBuf>>,

    #[serde(alias = "new-dir-files")]
    #[serde(default = "HashMap::new")]
    pub new_dir_files: HashMap<Hash40String, HashSet<Hash40String>>,
}

impl ModConfig {
    pub fn new() -> Self {
        Self {
            unshare_blacklist: HashSet::new(),
            new_files: HashMap::new(),
            preprocess_reshare: HashMap::new(),
            new_dir_files: HashMap::new(),
            new_shared_files: HashMap::new(),
            preprocess_reshare_ext: HashMap::new(),
        }
    }

    pub fn merge(&mut self, other: ModConfig) {
        let Self {
            unshare_blacklist,
            new_files,
            preprocess_reshare,
            preprocess_reshare_ext,
            new_shared_files,
            new_dir_files,
        } = other;

        self.unshare_blacklist.extend(unshare_blacklist.into_iter());
        self.preprocess_reshare.extend(preprocess_reshare.into_iter());
        self.preprocess_reshare_ext.extend(preprocess_reshare_ext.into_iter());

        for (hash, list) in new_files.into_iter() {
            if let Some(list) = list {
                if let Some(Some(current_list)) = self.new_files.get_mut(&hash) {
                    current_list.extend(list.into_iter());
                } else {
                    let _ = self.new_files.insert(hash, Some(list));
                }
            }
        }

        for (hash, list) in new_dir_files.into_iter() {
            if let Some(current_list) = self.new_dir_files.get_mut(&hash) {
                current_list.extend(list.into_iter());
            } else {
                let _ = self.new_dir_files.insert(hash, list);
            }
        }

        for (hash, list) in new_shared_files.into_iter() {
            if let Some(current_list) = self.new_shared_files.get_mut(&hash) {
                current_list.extend(list.into_iter());
            } else {
                let _ = self.new_shared_files.insert(hash, list);
            }
        }
    }
}
