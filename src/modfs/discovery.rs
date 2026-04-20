use std::path::PathBuf;

use arc_config::Config as ModConfig;
use smash_arc::Hash40;

use super::{patch::FileEntry, registry::HandlerId, ModFs, PatchLayer};

pub struct DiscoveryContext<'a> {
    pub patch: &'a mut PatchLayer,
    pub config: &'a mut ModConfig,
    pub bindings: Vec<(Hash40, HandlerId)>,
}

impl<'a> DiscoveryContext<'a> {
    pub fn new(patch: &'a mut PatchLayer, config: &'a mut ModConfig) -> Self {
        Self {
            patch,
            config,
            bindings: Vec::new(),
        }
    }

    pub fn bind_hash(&mut self, hash: Hash40, id: HandlerId) {
        self.bindings.push((hash, id));
    }
}

impl ModFs {
    pub fn populate_from(&mut self, entries: &[(PathBuf, PathBuf, usize)], config: &mut ModConfig) -> Vec<(&'static str, usize)> {
        let mut handler_counts = vec![0usize; self.handlers.len()];

        for (root, local, size) in entries {
            let full_path = root.join(local);

            let entry = FileEntry {
                root: root.clone(),
                size: *size,
            };
            let hash = crate::PathExtension::smash_hash(local.as_path()).ok();
            self.patch.insert(local.clone(), entry, hash);

            if let Some(s) = local.to_str() {
                crate::hashes::add(s);
            }

            if let Some(handler_id) = self.handlers.lookup(local) {
                handler_counts[handler_id.0] += 1;
                let mut ctx = DiscoveryContext::new(&mut self.patch, config);
                let bindings = {
                    let handler = self.handlers.handler_mut(handler_id);
                    if let Some(target_hash) = handler.discover(&mut ctx, &full_path, local, *size) {
                        ctx.bindings.push((target_hash, handler_id));
                    }
                    std::mem::take(&mut ctx.bindings)
                };
                for (hash, id) in bindings {
                    self.handlers.bind_hash(hash, id);
                }
            }
        }

        (0..self.handlers.len())
            .map(|i| (self.handlers.handler_name(HandlerId(i)), handler_counts[i]))
            .collect()
    }

    pub fn populate_stats(&self) -> ModFsStats {
        ModFsStats {
            patch_files: self.patch.iter_files().count(),
            virtual_files: 0,
        }
    }
}

#[derive(Debug)]
pub struct ModFsStats {
    pub patch_files: usize,
    pub virtual_files: usize,
}
