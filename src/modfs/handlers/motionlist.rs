use std::{
    collections::HashMap,
    fs::File,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use hash40::diff::Diff;
use serde_yaml::from_str;
use smash_arc::Hash40;

use crate::{
    hashes,
    modfs::{registry::FileHandler, DiscoveryContext, ModFsError},
    PathExtension,
};

#[derive(Default)]
pub struct MotionListHandler {
    patches: HashMap<Hash40, Vec<PathBuf>>,
}

impl FileHandler for MotionListHandler {
    fn name(&self) -> &'static str {
        "motionlist"
    }

    fn patches_load(&self) -> bool {
        true
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["motdiff"]
    }

    fn filenames(&self) -> &'static [&'static str] {
        &["motion_list.yml"]
    }

    fn discover(&mut self, _ctx: &mut DiscoveryContext, full_path: &Path, local: &Path, _size: usize) -> Option<Hash40> {
        let base_local = local.with_extension("bin");
        let (base_local, _region) = super::strip_regional(&base_local);

        let name = base_local.file_name().and_then(|n| n.to_str())?;
        if !name.contains("motion_list") {
            warn!("modfs: rejected non-motion_list file {}", full_path.display());
            return None;
        }

        let hash = super::try_smash_hash(&base_local)?;
        self.patches.entry(hash).or_default().push(full_path.to_path_buf());

        if let Some(s) = local.to_str() {
            hashes::add(s);
        }
        if let Some(s) = base_local.to_str() {
            hashes::add(s);
        }
        Some(hash)
    }

    fn apply(&self, hash: Hash40, bytes: Vec<u8>) -> Result<Vec<u8>, ModFsError> {
        let Some(patches) = self.patches.get(&hash) else {
            return Ok(bytes);
        };

        let mut yml_patches: Vec<&PathBuf> = Vec::new();
        let mut diff_patches: Vec<&PathBuf> = Vec::new();
        for patch_path in patches {
            if patch_path.has_extension("motdiff") {
                diff_patches.push(patch_path);
            } else if patch_path.ends_with("motion_list.yml") {
                yml_patches.push(patch_path);
            } else {
                return Err(ModFsError::Handler(format!(
                    "unknown motion list patch kind for {}",
                    patch_path.display()
                )));
            }
        }

        let mut reader = Cursor::new(&bytes);
        let mut motion_list = motion_lib::read_stream(&mut reader)
            .map_err(|e| ModFsError::Handler(format!("failed to parse base motion_list: {:?}", e)))?;

        if !yml_patches.is_empty() {
            if yml_patches.len() > 1 {
                warn!("Multiple motion_list.yml files for hash {:#x}; last one wins.", hash.0);
            }
            for full_patch in &yml_patches {
                let mut contents = String::new();
                File::open(full_patch)
                    .and_then(|mut f| f.read_to_string(&mut contents))
                    .map_err(|e| ModFsError::Handler(format!("failed to read motion_list.yml {}: {}", full_patch.display(), e)))?;
                if let Some(full) = from_str::<Option<_>>(&contents)
                    .map_err(|e| ModFsError::Handler(format!("failed to parse motion_list.yml {}: {}", full_patch.display(), e)))?
                {
                    motion_list = full;
                }
            }
        }

        for patch_path in &diff_patches {
            let mut contents = String::new();
            File::open(patch_path)
                .and_then(|mut f| f.read_to_string(&mut contents))
                .map_err(|e| ModFsError::Handler(format!("failed to read motdiff {}: {}", patch_path.display(), e)))?;
            let diff: Option<_> = from_str(&contents)
                .map_err(|e| ModFsError::Handler(format!("failed to parse motdiff {}: {}", patch_path.display(), e)))?;
            let Some(diff) = diff else {
                return Err(ModFsError::Handler(format!("not a motion list patch: {}", patch_path.display())));
            };
            motion_list.apply(&diff);
        }

        let mut writer = Cursor::new(Vec::new());
        motion_lib::write_stream(&mut writer, &motion_list)
            .map_err(|e| ModFsError::Handler(format!("failed to write patched motion_list: {:?}", e)))?;
        Ok(writer.into_inner())
    }
}
