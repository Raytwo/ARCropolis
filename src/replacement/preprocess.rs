use smash_arc::{ArcLookup, FilePathIdx, Hash40};
use std::collections::HashMap;

use crate::hashes;

use super::{AdditionContext, LoadedArcEx};

pub fn reshare_contained_files(ctx: &mut AdditionContext, source: Hash40, dependent: Hash40) {
    let source_range = match ctx.get_dir_info_from_hash(source) {
        Ok(dir_info) => dir_info.file_info_range(),
        Err(_) => {
            error!("Failed to find source directory '{}' ({:#x}) when attempting to reshare their contained files.", hashes::find(source), source.0);
            return;
        }
    };

    let dependent_range = match ctx.get_dir_info_from_hash(dependent) {
        Ok(dir_info) => dir_info.file_info_range(),
        Err(_) => {
            error!("Failed to find dependent directory '{}' ({:#x}) when attempting to reshare their contained files.", hashes::find(dependent), dependent.0);
            return;
        }
    };

    let filepath_to_index: HashMap<FilePathIdx, FilePathIdx> = ctx.file_infos[source_range]
        .iter()
        .filter_map(|x| {
            let hash = ctx.filepaths[usize::from(x.file_path_index)].path.hash40();
            match ctx.get_shared_file(hash) {
                Ok(index) => Some((index, x.file_path_index)),
                Err(_) => {
                    warn!("Could not get shared file for file '{}' ({:#x}) when attempting to reshare it.", hashes::find(hash), hash.0);
                    None
                }
            }
        })
        .collect();

    let filepaths = ctx.arc.get_file_paths();
    for info in ctx.file_infos[dependent_range].iter_mut() {
        let hash = filepaths[usize::from(info.file_path_index)].path.hash40();
        if let Ok(shared_idx) = &ctx.arc.get_shared_file(hash) {
            if let Some(new_index) = filepath_to_index.get(&shared_idx) {
                info.file_path_index = *new_index;
            }
        }
    }
}