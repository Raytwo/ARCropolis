use std::{
    collections::HashMap,
    io::Cursor,
    path::{Path, PathBuf},
};

use smash_arc::Hash40;
use smash_bgm_property::BgmPropertyFile;

use crate::{
    hashes,
    modfs::{registry::FileHandler, DiscoveryContext, ModFsError},
};

#[derive(Default)]
pub struct BgmHandler {
    patches: HashMap<Hash40, Vec<PathBuf>>,
}

impl FileHandler for BgmHandler {
    fn name(&self) -> &'static str {
        "bgm"
    }

    fn patches_load(&self) -> bool {
        true
    }

    fn filenames(&self) -> &'static [&'static str] {
        &["bgm_property.bin"]
    }

    fn size_multiplier(&self) -> u32 {
        30
    }

    fn discover(&mut self, _ctx: &mut DiscoveryContext, full_path: &Path, local: &Path, _size: usize) -> Option<Hash40> {
        let (base_local, _region) = super::strip_regional(local);

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

        let mut reader = Cursor::new(&bytes);
        let mut bgm_property = BgmPropertyFile::read(&mut reader)
            .map_err(|e| ModFsError::Handler(format!("failed to parse base bgm_property: {:?}", e)))?;

        for patch_path in patches {
            let mut patch_file = BgmPropertyFile::from_file(patch_path)
                .map_err(|e| ModFsError::Handler(format!("failed to read bgm_property patch {}: {:?}", patch_path.display(), e)))?;
            bgm_property.entries.append(&mut patch_file.entries);
        }

        let mut writer = Cursor::new(Vec::new());
        bgm_property
            .write(&mut writer)
            .map_err(|e| ModFsError::Handler(format!("failed to write patched bgm_property: {:?}", e)))?;
        Ok(writer.into_inner())
    }
}
