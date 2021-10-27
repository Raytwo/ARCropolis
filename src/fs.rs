use orbits::{
    orbit::LaunchPad, ConflictHandler, ConflictKind, DiscoverSystem, FileEntryType, FileLoader,
    Orbit, StandardLoader,
};
use smash_arc::{ArcLookup, Hash40, LoadedArc, LookupError, Region, SearchLookup};

use crate::config;
use std::{
    ops::Deref,
    path::Path,
};

use std::fmt;

pub struct FilesystemUninitializedError;

impl fmt::Debug for FilesystemUninitializedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Filesystem is uninitialized!")
    }
}

pub enum GlobalFilesystem {
    Uninitialized,
    Promised(std::thread::JoinHandle<LaunchPad<StandardLoader, StandardLoader>>),
    Initialized(Orbit<ArcLoader, StandardLoader, StandardLoader>),
}

impl GlobalFilesystem {
    pub fn finish(self, arc: &'static LoadedArc) -> Result<Self, FilesystemUninitializedError> {
        match self {
            Self::Uninitialized => Err(FilesystemUninitializedError),
            Self::Promised(promise) => match promise.join() {
                Ok(launchpad) => Ok(Self::Initialized(launchpad.launch(ArcLoader(arc)))),
                Err(_) => Err(FilesystemUninitializedError),
            },
            Self::Initialized(filesystem) => Ok(Self::Initialized(filesystem)),
        }
    }

    pub fn take(&mut self) -> Self {
        let mut out = GlobalFilesystem::Uninitialized;
        std::mem::swap(self, &mut out);
        out
    }
}

#[repr(transparent)]
pub struct ArcLoader(&'static LoadedArc);

unsafe impl Send for ArcLoader {}
unsafe impl Sync for ArcLoader {}

impl Deref for ArcLoader {
    type Target = LoadedArc;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}
impl FileLoader for ArcLoader {
    type ErrorType = LookupError;

    fn path_exists(&self, _: &Path, local_path: &Path) -> bool {
        if let Some(path) = local_path.as_os_str().to_str() {
            self.get_file_path_index_from_hash(Hash40::from(path))
                .is_ok()
        } else {
            false
        }
    }

    fn get_path_type(&self, _: &Path, local_path: &Path) -> Result<FileEntryType, Self::ErrorType> {
        if let Some(path) = local_path.as_os_str().to_str() {
            let path_entry = self.get_path_list_entry_from_hash(path)?;
            match path_entry.is_directory() {
                true => Ok(FileEntryType::Directory),
                false => Ok(FileEntryType::File),
            }
        } else {
            Err(LookupError::Missing)
        }
    }

    fn load_path(&self, _: &Path, local_path: &Path) -> Result<Vec<u8>, Self::ErrorType> {
        match local_path.as_os_str().to_str() {
            Some(path) => self.get_file_contents(Hash40::from(path), Region::None),
            None => Err(LookupError::Missing),
        }
    }
}

pub fn perform_discovery() -> LaunchPad<StandardLoader, StandardLoader> {
    let filter = |x: &Path| {
        match x.file_name() {
            Some(name) if let Some(name) = name.to_str() => {
                !name.starts_with(".")
            },
            _ => false
        }
    };

    let ignore = |x: &Path| {
        match x.file_name() {
            Some(name) if let Some(name) = name.to_str() => {
                static RESERVED_NAMES: &[&'static str] = &[
                    "info.toml",
                ];
                RESERVED_NAMES.contains(&name)
            },
            _ => false
        }
    };

    let collect = |x: &Path| {
        let is_config = match x.file_name() {
            Some(name) if let Some(name) = name.to_str() => {
                static RESERVED_NAMES: &[&'static str] = &[
                    "config.json"
                ];
                RESERVED_NAMES.contains(&name)
            },
            _ => false
        };

        if !is_config {
            match x.extension() {
                Some(ext) if let Some(ext) = ext.to_str() => ext == "nro",
                _ => false
            }
        } else {
            is_config
        }
    };

    let mut launchpad = LaunchPad::new(
        DiscoverSystem::new(StandardLoader, ConflictHandler::NoRoot),
        DiscoverSystem::new(StandardLoader, ConflictHandler::NoRoot),
    );

    let arc_path = config::arc_path();

    launchpad.patch.collecting(collect);
    launchpad.patch.ignoring(ignore);

    let mut conflicts = if std::fs::try_exists(arc_path).unwrap_or(false) {
        launchpad.patch.discover_in_root(config::arc_path())
    } else {
        Vec::new()
    };

    let umm_path = config::umm_path();
    if std::fs::try_exists(umm_path).unwrap_or(false) {
        conflicts.extend(
            launchpad
                .patch
                .discover_roots(config::umm_path(), 1, filter),
        );
    }

    for path in config::extra_paths() {
        if std::fs::try_exists(path).unwrap_or(false) {
            conflicts.extend(launchpad.patch.discover_roots(path, 1, filter));
        }
    }

    for conflict in conflicts.into_iter() {
        match conflict {
            ConflictKind::StandardConflict(error, kept) => warn!(
                "File '{}' was rejected for file '{}' during discovery.",
                error.display(),
                kept.display()
            ),
            ConflictKind::RootConflict(root_path, kept) => warn!(
                "Mod root '{}' was rejected for a file conflict with '{}' during discovery.",
                root_path.display(),
                kept.display()
            )
        }
    }

    launchpad
}
