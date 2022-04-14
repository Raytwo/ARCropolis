use std::collections::{HashMap, HashSet};

use smash_arc::{ArcLookup, FileInfoIdx, FilePathIdx, Hash40};

use super::{AdditionContext, LoadedArcEx};
use crate::hashes;

pub fn reshare_contained_files(ctx: &mut AdditionContext, dependent: Hash40, source: Hash40) -> HashSet<Hash40> {
    let source_range = match ctx.get_dir_info_from_hash(source) {
        Ok(dir_info) => dir_info.file_info_range(),
        Err(_) => {
            error!(
                "Failed to find source directory '{}' ({:#x}) when attempting to reshare their contained files.",
                hashes::find(source),
                source.0
            );
            return HashSet::new();
        },
    };

    let dependent_range = match ctx.get_dir_info_from_hash(dependent) {
        Ok(dir_info) => dir_info.file_info_range(),
        Err(_) => {
            error!(
                "Failed to find dependent directory '{}' ({:#x}) when attempting to reshare their contained files.",
                hashes::find(dependent),
                dependent.0
            );
            return HashSet::new();
        },
    };

    let filepath_to_index: HashMap<FilePathIdx, FilePathIdx> = ctx.file_infos[source_range]
        .iter()
        .filter_map(|x| {
            let hash = ctx.filepaths[usize::from(x.file_path_index)].path.hash40();
            match ctx.get_shared_file(hash) {
                Ok(index) => Some((index, x.file_path_index)),
                Err(_) => {
                    warn!(
                        "Could not get shared file for file '{}' ({:#x}) when attempting to reshare it.",
                        hashes::find(hash),
                        hash.0
                    );
                    None
                },
            }
        })
        .collect();

    dependent_range
        .into_iter()
        .filter_map(|dep_idx| {
            let shared_file_idx = ctx.get_shared_info_index(FileInfoIdx(dep_idx as u32));
            if let Some(new_path_idx) = filepath_to_index.get(&ctx.file_infos[usize::from(shared_file_idx)].file_path_index) {
                let hash = ctx.filepaths[usize::from(ctx.file_infos[dep_idx].file_path_index)].path.hash40();
                ctx.file_infos[dep_idx].file_path_index = *new_path_idx;
                Some(hash)
            } else {
                None
            }
        })
        .collect()
}
