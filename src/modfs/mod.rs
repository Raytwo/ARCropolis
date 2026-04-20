use std::path::{Path, PathBuf};

use arc_config::Config as ModConfig;
use smash_arc::{ArcLookup, Hash40, LookupError};
use thiserror::Error;

pub mod discovery;
pub mod handlers;
pub mod patch;
pub mod registry;
pub mod virt;

pub use discovery::DiscoveryContext;
pub use patch::PatchLayer;
pub use registry::{FileHandler, HandlerRegistry};
pub use virt::{VirtualEntry, VirtualLayer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryType {
    File,
    Directory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictMode {
    NoRoot,
    First,
    KeepAll,
}

impl Default for ConflictMode {
    fn default() -> Self {
        ConflictMode::NoRoot
    }
}

#[derive(Debug, Error)]
pub enum ModFsError {
    #[error("no entry for hash {0:#x}")]
    NotFound(u64),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("hash error: {0:?}")]
    Hash(crate::InvalidOsStrError),
    #[error("vanilla lookup error: {0:?}")]
    Vanilla(LookupError),
    #[error("handler error: {0}")]
    Handler(String),
}

impl From<crate::InvalidOsStrError> for ModFsError {
    fn from(e: crate::InvalidOsStrError) -> Self {
        ModFsError::Hash(e)
    }
}

impl From<LookupError> for ModFsError {
    fn from(e: LookupError) -> Self {
        ModFsError::Vanilla(e)
    }
}

pub struct ModFs {
    patch: PatchLayer,
    virt: VirtualLayer,
    handlers: HandlerRegistry,
    conflict_mode: ConflictMode,
}

impl Default for ModFs {
    fn default() -> Self {
        Self::new()
    }
}

impl ModFs {
    pub fn new() -> Self {
        let mut handlers = HandlerRegistry::new();
        handlers::register_builtins(&mut handlers);
        Self {
            patch: PatchLayer::new(),
            virt: VirtualLayer::new(),
            handlers,
            conflict_mode: ConflictMode::default(),
        }
    }

    pub fn register_handler<H: FileHandler>(&mut self, handler: H) {
        self.handlers.register(handler);
    }

    pub fn set_conflict_mode(&mut self, mode: ConflictMode) {
        self.conflict_mode = mode;
    }

    pub fn conflict_mode(&self) -> ConflictMode {
        self.conflict_mode
    }

    pub fn patch(&self) -> &PatchLayer {
        &self.patch
    }

    pub fn patch_mut(&mut self) -> &mut PatchLayer {
        &mut self.patch
    }

    pub fn virt(&self) -> &VirtualLayer {
        &self.virt
    }

    pub fn virt_mut(&mut self) -> &mut VirtualLayer {
        &mut self.virt
    }

    pub fn handlers(&self) -> &HandlerRegistry {
        &self.handlers
    }

    pub fn handlers_mut(&mut self) -> &mut HandlerRegistry {
        &mut self.handlers
    }

    pub fn read(&self, path: &Path) -> Result<Vec<u8>, ModFsError> {
        let bytes = if let Some(entry) = self.patch.get(path) {
            std::fs::read(entry.full_path(path))?
        } else {
            let hash = crate::PathExtension::smash_hash(path)?;
            let arc = crate::resource::arc();
            arc.get_file_contents(hash, config::region())?
        };

        let hash = crate::PathExtension::smash_hash(path)?;
        self.handlers.apply_chain(hash, bytes)
    }

    pub fn size(&self, path: &Path) -> Option<usize> {
        if let Ok(hash) = crate::PathExtension::smash_hash(path) {
            if let Some(s) = self.virt.max_size(hash) {
                return Some(s);
            }
            let base = self.patch.get(path).map(|e| e.size)?;
            let mult = self.handlers.size_multiplier_for_hash(hash);
            return Some(if mult == 0 { base } else { base * mult as usize });
        }
        self.patch.get(path).map(|e| e.size)
    }

    pub fn read_dir<'a>(&'a self, path: &Path) -> Vec<PathBuf> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();

        for (local, _entry) in self.patch.iter_files() {
            let tail = if path.as_os_str().is_empty() {
                local
            } else if let Ok(rest) = local.strip_prefix(path) {
                rest
            } else {
                continue;
            };

            let mut components = tail.components();
            let Some(first) = components.next() else { continue };

            let child = path.join(first);
            if seen.insert(child.clone()) {
                out.push(child);
            }
        }
        out
    }

    pub fn entry_type(&self, path: &Path) -> Option<EntryType> {
        if self.patch.contains(path) {
            return Some(EntryType::File);
        }
        for (local, _) in self.patch.iter_files() {
            if local.starts_with(path) && local != path {
                return Some(EntryType::Directory);
            }
        }
        None
    }

    pub fn exists(&self, path: &Path) -> bool {
        self.entry_type(path).is_some()
    }

    pub fn finalize(&self, config: &mut ModConfig) {
        self.handlers.finalize(config);
    }

    pub fn read_by_hash(&self, hash: Hash40) -> Result<Vec<u8>, ModFsError> {
        if let Some(chain) = self.virt.chain(hash) {
            if let Some(entry) = chain.take_next() {
                let result = Self::invoke_virtual(hash, entry);
                chain.release();
                match result {
                    Ok(Some(bytes)) => return Ok(bytes),
                    Ok(None) => {},
                    Err(e) => return Err(e),
                }
            }
        }
        self.read_base(hash)
    }

    fn invoke_virtual(hash: Hash40, entry: &VirtualEntry) -> Result<Option<Vec<u8>>, ModFsError> {
        use crate::fs::loaders::ApiCallback;
        match entry.callback {
            ApiCallback::GenericCallback(cb) => {
                let mut vec = Vec::with_capacity(entry.max_size);
                let mut new_len = entry.max_size;
                let success = cb(hash.0, vec.as_mut_ptr(), entry.max_size, &mut new_len);
                if success && new_len <= entry.max_size {
                    unsafe { vec.set_len(new_len) };
                    Ok(Some(vec))
                } else {
                    Ok(None)
                }
            },
            ApiCallback::StreamCallback(_) | ApiCallback::None => Ok(None),
        }
    }

    pub fn read_base(&self, hash: Hash40) -> Result<Vec<u8>, ModFsError> {
        let bytes = if let Some((local, entry)) = self.patch.entry_for_hash(hash) {
            std::fs::read(entry.full_path(local))?
        } else {
            let arc = crate::resource::arc();
            arc.get_file_contents(hash, config::region())?
        };
        self.handlers.apply_chain(hash, bytes)
    }

    pub fn resolve_stream_path(&self, hash: Hash40) -> Option<(std::path::PathBuf, usize)> {
        use crate::fs::loaders::ApiCallback;

        if let Some(chain) = self.virt.chain(hash) {
            for entry in chain.entries() {
                let ApiCallback::StreamCallback(cb) = entry.callback else { continue };
                let mut buf = vec![0u8; 0x100];
                let mut file_size = 0usize;
                let success = cb(hash.0, buf.as_mut_ptr(), &mut file_size);
                if !success {
                    break;
                }
                let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
                if let Ok(s) = std::str::from_utf8(&buf[..end]) {
                    return Some((std::path::PathBuf::from(s), file_size));
                }
                break;
            }
        }

        let (local, entry) = self.patch.entry_for_hash(hash)?;
        Some((entry.full_path(local), entry.size))
    }
}
