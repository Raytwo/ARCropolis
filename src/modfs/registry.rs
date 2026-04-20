use std::{collections::HashMap, path::Path};

use arc_config::Config as ModConfig;
use smash_arc::Hash40;

use super::{DiscoveryContext, ModFsError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandlerId(pub(crate) usize);

pub trait FileHandler: Send + Sync + 'static {
    fn name(&self) -> &'static str {
        "unnamed"
    }

    fn patches_load(&self) -> bool {
        false
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn filenames(&self) -> &'static [&'static str] {
        &[]
    }

    fn discover(&mut self, ctx: &mut DiscoveryContext, full_path: &Path, local: &Path, size: usize) -> Option<Hash40>;

    fn apply(&self, _hash: Hash40, bytes: Vec<u8>) -> Result<Vec<u8>, ModFsError> {
        Ok(bytes)
    }

    fn size_multiplier(&self) -> u32 {
        10
    }

    fn finalize(&self, _config: &mut ModConfig) {}
}

#[derive(Default)]
pub struct HandlerRegistry {
    handlers: Vec<Box<dyn FileHandler>>,
    by_extension: HashMap<&'static str, HandlerId>,
    by_filename: HashMap<&'static str, HandlerId>,
    by_hash: HashMap<Hash40, Vec<HandlerId>>,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<H: FileHandler>(&mut self, handler: H) -> HandlerId {
        let id = HandlerId(self.handlers.len());
        for ext in handler.extensions() {
            self.by_extension.insert(*ext, id);
        }
        for name in handler.filenames() {
            self.by_filename.insert(*name, id);
        }
        self.handlers.push(Box::new(handler));
        id
    }

    pub fn lookup(&self, local: &Path) -> Option<HandlerId> {
        if let Some(name) = local.file_name().and_then(|n| n.to_str()) {
            if let Some(id) = self.by_filename.get(name) {
                return Some(*id);
            }
        }
        if let Some(ext) = local.extension().and_then(|e| e.to_str()) {
            if let Some(id) = self.by_extension.get(ext) {
                return Some(*id);
            }
        }
        None
    }

    pub fn handler_mut(&mut self, id: HandlerId) -> &mut dyn FileHandler {
        &mut *self.handlers[id.0]
    }

    pub fn handler_name(&self, id: HandlerId) -> &'static str {
        self.handlers[id.0].name()
    }

    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    pub fn bind_hash(&mut self, hash: Hash40, id: HandlerId) {
        self.by_hash.entry(hash).or_default().push(id);
    }

    pub fn handlers_for_hash(&self, hash: Hash40) -> &[HandlerId] {
        self.by_hash.get(&hash).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn bound_hashes(&self) -> impl Iterator<Item = Hash40> + '_ {
        self.by_hash.keys().copied()
    }

    pub fn has_load_patchers(&self, hash: Hash40) -> bool {
        self.handlers_for_hash(hash).iter().any(|id| self.handlers[id.0].patches_load())
    }

    pub fn size_multiplier_for_hash(&self, hash: Hash40) -> u32 {
        self.handlers_for_hash(hash)
            .iter()
            .map(|id| self.handlers[id.0].size_multiplier())
            .max()
            .unwrap_or(0)
    }

    pub fn apply_chain(&self, hash: Hash40, mut bytes: Vec<u8>) -> Result<Vec<u8>, ModFsError> {
        for id in self.handlers_for_hash(hash) {
            bytes = self.handlers[id.0].apply(hash, bytes)?;
        }
        Ok(bytes)
    }

    pub fn finalize(&self, config: &mut ModConfig) {
        for handler in &self.handlers {
            handler.finalize(config);
        }
    }
}
