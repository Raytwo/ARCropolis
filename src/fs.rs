use orbits::{
    orbit::LaunchPad, ConflictHandler, ConflictKind, DiscoverSystem, FileEntryType, FileLoader,
    Orbit, StandardLoader,
};
use smash_arc::{ArcLookup, Hash40, LoadedArc, LookupError, Region, SearchLookup};

use crate::chainloader::{self, NrrBuilder};
use crate::config;
use std::{
    ops::Deref,
    path::Path
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

    pub fn get(&self) -> &Orbit<ArcLoader, StandardLoader, StandardLoader> {
        match self {
            Self::Initialized(loader) => loader,
            _ => panic!("Global Filesystem is not initialized!")
        }
    }

    pub fn get_mut(&mut self) -> &mut Orbit<ArcLoader, StandardLoader, StandardLoader> {
        match self {
            Self::Initialized(loader) => loader,
            _ => panic!("Global Filesystem is not initialized!")
        }
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

        is_config || x == Path::new("plugin.nro")
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

    let should_prompt = !conflicts.is_empty();

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

    if should_prompt {
        if config::file_logging_enabled() {
            skyline_web::DialogOk::ok("During file discovery, ARCropolis encountered file conflicts.<br>See the latest log for more information.");
        } else {
            skyline_web::DialogOk::ok("During file discovery, ARCropolis encountered file conflicts.<br>Enable file logging and run again for more information.");
        }
    }

    let fighter_nro_parent = Path::new("prebuilt;/nro/release");
    let mut fighter_nro_nrr = NrrBuilder::new();

    launchpad.patch.tree.walk_paths(|node, entry_type| {
        match node.get_local().parent() {
            Some(parent) if parent == fighter_nro_parent => {
                info!("Reading '{}' for module registration.", node.full_path().display());
                if let Ok(data) = std::fs::read(node.full_path()) {
                    fighter_nro_nrr.add_module(data.as_slice());
                }
            },
            _ => {}
        }
    });

    match fighter_nro_nrr.register() {
        Ok(_) => info!("Successfully registered fighter modules."),
        Err(e) => {
            error!("{:?}", e);
            crate::dialog_error("ARCropolis failed to register module information for fighter modules.<br>You may experience infinite loading on some fighters.");
        }
    }

    chainloader::load_and_run_plugins(&launchpad.patch.collected);

    launchpad
}
