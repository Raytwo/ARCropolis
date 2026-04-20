use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
};

use smash_arc::Hash40;

use crate::{
    hashes,
    modfs::{registry::FileHandler, DiscoveryContext, ModFsError},
    PathExtension,
};

#[derive(Default)]
pub struct PrcHandler {
    patches: HashMap<Hash40, Vec<PathBuf>>,
}

impl FileHandler for PrcHandler {
    fn name(&self) -> &'static str {
        "prc"
    }

    fn patches_load(&self) -> bool {
        true
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["prcx", "prcxml", "stdatx", "stdatxml", "stprmx", "stprmxml"]
    }

    fn discover(&mut self, _ctx: &mut DiscoveryContext, full_path: &Path, local: &Path, _size: usize) -> Option<Hash40> {
        let base_ext = if local.has_extension("prcx") || local.has_extension("prcxml") {
            "prc"
        } else if local.has_extension("stdatx") || local.has_extension("stdatxml") {
            "stdat"
        } else if local.has_extension("stprmx") || local.has_extension("stprmxml") {
            "stprm"
        } else {
            return None;
        };

        let base_local = local.with_extension(base_ext);
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

        let mut param_data = prcx::read_stream(&mut Cursor::new(&bytes))
            .map_err(|_| ModFsError::Handler("unable to parse base param data".to_string()))?;

        for patch_path in patches {
            let patch = match prcx::open(patch_path) {
                Ok(p) => p,
                Err(_) => {
                    let file = File::open(patch_path)
                        .map_err(|e| ModFsError::Handler(format!("failed to open prc patch {}: {}", patch_path.display(), e)))?;
                    let mut reader = BufReader::new(file);
                    prcx::read_xml(&mut reader)
                        .map_err(|_| ModFsError::Handler(format!("failed to parse prc patch {}", patch_path.display())))?
                },
            };

            prcx::apply_patch(&patch, &mut param_data)
                .map_err(|_| ModFsError::Handler(format!("failed to apply prc patch {}", patch_path.display())))?;
        }

        let mut writer = Cursor::new(Vec::new());
        prcx::write_stream(&mut writer, &param_data)
            .map_err(|e| ModFsError::Handler(format!("failed to write patched prc: {:?}", e)))?;
        Ok(writer.into_inner())
    }
}
