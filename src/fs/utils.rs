use std::{
    collections::HashSet,
    path::PathBuf,
};

use arc_config::ToExternal;

use crate::{modfs::PatchLayer, PathExtension};

pub fn collect_nus3bank_deps(
    patch: &PatchLayer,
    unshare_blacklist: &[hash40::Hash40],
) -> HashSet<PathBuf> {
    let mut nus3audio_deps = HashSet::new();
    let mut nus3banks_found = HashSet::new();

    for (local, _entry) in patch.iter_files() {
        if local.is_stream() {
            continue;
        }
        if local.has_extension("nus3audio") {
            if let Ok(hash) = local.smash_hash() {
                if !unshare_blacklist.contains(&hash.to_external()) {
                    nus3audio_deps.insert(local.with_extension("nus3bank"));
                }
            }
        } else if local.has_extension("nus3bank") {
            nus3banks_found.insert(local.to_path_buf());
        }
    }

    for bank in nus3banks_found.into_iter() {
        nus3audio_deps.remove(&bank);
    }

    nus3audio_deps
}


