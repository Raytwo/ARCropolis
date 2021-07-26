use std::{
    collections::HashMap,
    path::PathBuf,
};

use crate::config::REGION;

use smash_arc::{ArcLookup, Hash40};

use walkdir::WalkDir;

use super::{ModFile, SmashPath, RejectionReason};

pub struct DiscoveryResults {
    pub accepted: HashMap<Hash40, ModFile>,
    pub rejected: Vec<(PathBuf, RejectionReason)>,
    pub stream: HashMap<Hash40, ModFile>
}

/// Discover every file in a directory and its sub-directories.  
/// Files starting with a period are filtered out, and only the files with relevant regions are kept.  
/// This signifies that if your goal is to simply get all the files, this is not the method to use.
pub fn discovery<Arc: ArcLookup>(arc: &Arc, path: &PathBuf, results: &mut DiscoveryResults) {
    for mod_file in WalkDir::new(path)
        .min_depth(1)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_str().unwrap().starts_with('.')) {
        if let Ok(mod_file) = mod_file {
            if mod_file.file_type().is_file() {
                if mod_file.path().extension().is_some() {
                    let smash_path = SmashPath(mod_file.path().strip_prefix(path).unwrap().to_path_buf());
                    if let Some(region) = smash_path.get_region() {
                        if region != *REGION {
                            continue;
                        }
                    }
                    let hash = smash_path.hash40().unwrap();
                    if let Some(previous) = results.accepted.get(&hash) {
                        results.rejected.push((mod_file.path().to_path_buf(), RejectionReason::DuplicateFile(previous.path.to_path_buf())));
                    } else {
                        match arc.get_file_path_index_from_hash(hash) {
                            Ok(_) => {
                                results.accepted.insert(hash, ModFile::new(
                                    path.to_path_buf(),
                                    smash_path
                                ));
                            },
                            Err(_) => {
                                if smash_path.is_stream() {
                                    if let Some(previous) = results.stream.get(&hash) {
                                        results.rejected.push((mod_file.path().to_path_buf(), RejectionReason::DuplicateFile(previous.path.to_path_buf())));
                                    } else {
                                        results.stream.insert(hash, ModFile::new(
                                            path.to_path_buf(),
                                            smash_path
                                        ));
                                    }
                                } else {
                                    results.rejected.push((mod_file.path().to_path_buf(), RejectionReason::NotFound(smash_path)));
                                }
                            }
                        }
                    }
                } else {
                    results.rejected.push((mod_file.path().to_path_buf(), RejectionReason::MissingExtension));
                }
            }
        }
    }
}

/// Run ``discovery`` on every directory found using the path  
/// Files starting with a period are filtered out, and only the files with relevant regions are kept.  
/// This signifies that if your goal is to simply get all the files, this is not the method to use.  
/// This method exists to support backward compatibility with Ultimate Mod Manager.  
pub fn umm_discovery<Arc: ArcLookup>(arc: &Arc, dir: &PathBuf, results: &mut DiscoveryResults) {
    for mod_directory in WalkDir::new(dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_str().unwrap().starts_with('.')) {
        if let Ok(mod_directory) = mod_directory {
            if mod_directory.file_type().is_dir() {
                discovery(arc, &mod_directory.into_path(), results);
            }
        }        
    }
}