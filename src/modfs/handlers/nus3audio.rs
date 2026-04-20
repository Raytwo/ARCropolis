use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use nus3audio::{AudioFile, Nus3audioFile};
use smash_arc::Hash40;

use crate::{
    hashes,
    modfs::{registry::FileHandler, DiscoveryContext, ModFsError},
};

#[derive(Default)]
pub struct Nus3audioHandler {
    patches: HashMap<Hash40, Vec<PathBuf>>,
}

impl FileHandler for Nus3audioHandler {
    fn name(&self) -> &'static str {
        "nus3audio"
    }

    fn patches_load(&self) -> bool {
        true
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["patch3audio"]
    }

    fn discover(&mut self, _ctx: &mut DiscoveryContext, full_path: &Path, local: &Path, _size: usize) -> Option<Hash40> {
        let base_local = local.with_extension("nus3audio");
        let (base_local, _region) = super::strip_regional(&base_local);

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

        let mut original = Nus3audioFile::from_bytes(&bytes);

        let mut known: HashMap<String, AudioFile> =
            original.files.iter().map(|af| (af.name.clone(), af.clone())).collect();

        for patch_path in patches {
            let patch_bytes = std::fs::read(patch_path)
                .map_err(|e| ModFsError::Handler(format!("failed to read nus3audio patch {}: {}", patch_path.display(), e)))?;
            let modified = Nus3audioFile::from_bytes(&patch_bytes);

            for mut audio_file in modified.files {
                if let Some(existing) = known.get_mut(&audio_file.name) {
                    existing.data = audio_file.data.clone();
                } else {
                    audio_file.id = (known.len() + 1) as u32;
                    known.insert(audio_file.name.clone(), audio_file);
                }
            }
        }

        let mut new_files: Vec<AudioFile> = known.into_values().collect();
        new_files.sort_by_key(|af| af.id);
        original.files = new_files;

        let mut out = Vec::new();
        original.write(&mut out);
        Ok(out)
    }
}
