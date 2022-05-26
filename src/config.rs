use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    str::FromStr,
};

use camino::Utf8PathBuf;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use semver::Version;
use serde::{Deserialize, Serialize};
use skyline::nn;
use skyline_config::*;
use smash_arc::{Hash40, Region};
use walkdir::WalkDir;

use crate::util::env;

fn arcropolis_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn default_logger_level() -> String {
    "Warn".to_string()
}

fn default_region() -> String {
    "us_en".to_string()
}

pub static GLOBAL_CONFIG: Lazy<RwLock<StorageHolder<ArcStorage>>> = Lazy::new(|| {
    let mut storage = StorageHolder::new(ArcStorage::new());

    let version: Result<Version, _> = storage.get_field("version");

    if let Ok(config_version) = version {
        let curr_version = Version::parse(&arcropolis_version())
        .expect("Parsing of ARCropolis' version string failed. Please open an issue on www.arcropolis.com to let us know!");

        // Check if the configuration is from a previous version
        if curr_version > config_version {
            // TODO: Code to perform changes for each version
            if Version::new(3, 2, 0) > config_version {
                let mut default_workspace = HashMap::<&str, &str>::new();
                default_workspace.insert("Default", "presets");
                storage.set_field_json("workspace_list", &default_workspace).unwrap();
                storage.set_field("workspace", "Default").unwrap();
            }
            // Update the version in the config
            storage.set_field("version", arcropolis_version()).unwrap();
        }
    }
    else // Version file does not exist
    {
        generate_default_config(&mut storage).unwrap_or_else(|err| panic!("ARCropolis encountered an error when generating the default configuration: {}", err));
    }

    RwLock::new(storage)
});

// TODO: Find a way to finally get rid of this.
static REGION: Lazy<Region> = Lazy::new(|| Region::from_str(&region_str()).unwrap_or(Region::None));

fn generate_default_config<CS: ConfigStorage>(storage: &mut StorageHolder<CS>) -> Result<(), ConfigError> {
    info!("Populating ConfigStorage with default values.");

    // Just so we don't keep outdated fields
    storage.clear_storage();

    storage.set_field("version", arcropolis_version())?;
    storage.set_field("region", "us_en")?;
    storage.set_field("logging_level", "Warn")?;
    storage.set_field_json("extra_paths", &Vec::<String>::new())?;
    storage.set_flag("auto_update", true)?;
    storage.set_field_json("presets", &HashSet::<Hash40>::new())?;

    let mut default_workspace = HashMap::<&str, &str>::new();
    default_workspace.insert("Default", "presets");

    storage.set_field_json("workspace_list", &default_workspace)?;
    storage.set_field("workspace", "Default")?;

    storage.set_flag("first_boot", true)
}

fn convert_legacy_to_presets() -> HashSet<Hash40> {
    todo!("Rewrite this to take workspaces into account");
    let mut presets: HashSet<Hash40> = HashSet::new();

    if umm_path().exists() {
        // TODO: Turn this into a map and use Collect
        for entry in WalkDir::new(umm_path()).max_depth(1).into_iter().flatten() {
                let path = entry.path();

                // If the mod isn't disabled, add it to the preset
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| !name.starts_with('.'))
                    .unwrap_or(false)
                {
                    presets.insert(Hash40::from(path.to_str().unwrap()));
                } else {
                    todo!("Check if the destination already exists, because it'll definitely happen, and when someone opens an issue about it and you'll realize you knew ahead of time, you'll feel dumb. But right this moment, you decided not to do anything.");
                    std::fs::rename(
                        path,
                        format!("sd:/ultimate/mods/{}", &path.file_name().unwrap().to_str().unwrap()[1..]),
                    )
                    .unwrap();
                }
        }
    }

    presets
}

pub mod workspaces {
    use std::collections::HashMap;

    use skyline_config::ConfigError;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum WorkspaceError {
        #[error("a configuration error happened: {0}")]
        ConfigError(#[from] ConfigError),
        #[error("a workspace with this name already exists")]
        AlreadyExists,
        #[error("failed to find the preset file for this workspace")]
        MissingPreset,
        // #[error("failed to call from_str for the desired type")]
        // FromStrErr,
    }

    pub fn get_list() -> Result<HashMap<String, String>, WorkspaceError> {
        let storage = super::GLOBAL_CONFIG.read();
        storage.get_field_json("workspace_list").map_err(WorkspaceError::ConfigError)
    }

    pub fn create_new_workspace(name: String) -> Result<(), WorkspaceError> {
        let mut list = get_list()?;

        if let std::collections::hash_map::Entry::Vacant(e) = list.entry(name) {
            todo!("Implement code to generate a preset name");
            e.insert("temp".to_string());
            let mut storage = super::GLOBAL_CONFIG.write();
            storage.set_field_json("workspace_list", &list).map_err(WorkspaceError::ConfigError)
        } else {
            Err(WorkspaceError::AlreadyExists)
        }
    }

    pub fn set_active_workspace(_name: String) -> Result<(), WorkspaceError> {
        // Make sure that the preset file actually exists and return a custom error if it doesn't
        todo!()
    }

    pub fn get_active_workspace() -> Result<String, WorkspaceError> {
        let storage = super::GLOBAL_CONFIG.read();
        let _workspace_list = get_list();
        let _workspace_name = storage.get_field("workspace").unwrap_or("Default".to_string());
        // TODO: Make sure that the preset file exists and return a custom error if it doesn't
        Ok("lol, lmao".to_string())
    }
}

pub mod presets {
    use std::collections::{HashMap, HashSet};

    use once_cell::sync::Lazy;
    use skyline_config::ConfigError;
    use smash_arc::Hash40;

    static PRESET: Lazy<HashSet<Hash40>> = Lazy::new(HashSet::new);

    pub fn get_active_preset() -> Result<HashSet<Hash40>, ConfigError> {
        let storage = super::GLOBAL_CONFIG.read();
        let workspace_name: String = storage.get_field("workspace").unwrap_or("Default".to_string());
        let workspace_list: HashMap<String, String> = storage.get_field_json("workspace_list").unwrap_or_default();
        let preset_name = &workspace_list[&workspace_name];
        storage.get_field_json(preset_name)
    }

    pub fn set_active_preset(preset: &HashSet<Hash40>) -> Result<(), skyline_config::ConfigError> {
        let mut storage = super::GLOBAL_CONFIG.write();
        let workspace_name: String = storage.get_field("workspace").unwrap_or("Default".to_string());
        let workspace_list: HashMap<String, String> = storage.get_field_json("workspace_list").unwrap_or_default();
        let preset_name = &workspace_list[&workspace_name];
        storage.set_field_json(preset_name, preset)
    }
}

pub fn auto_update_enabled() -> bool {
    GLOBAL_CONFIG.read().get_flag("auto_update")
}

pub fn debug_enabled() -> bool {
    GLOBAL_CONFIG.read().get_flag("debug")
}

pub fn beta_updates() -> bool {
    GLOBAL_CONFIG.read().get_flag("beta_updates")
}

pub fn region() -> Region {
    *REGION
}

pub fn region_str() -> String {
    let region: String = GLOBAL_CONFIG.read().get_field("region").unwrap_or(String::from("us_en"));
    region
}

pub fn version() -> String {
    let version: String = GLOBAL_CONFIG
        .read()
        .get_field("version")
        .unwrap_or(String::from(env!("CARGO_PKG_VERSION")));
    version
}

pub fn umm_path() -> Utf8PathBuf {
    let path = Utf8PathBuf::from("sd:/ultimate/mods");

    if !path.exists() {
        std::fs::create_dir_all("sd:/ultimate/mods").unwrap();
    }

    path
}

pub fn logger_level() -> String {
    let level: String = GLOBAL_CONFIG.read().get_field("logging_level").unwrap_or(String::from("Warn"));
    level
}

pub fn file_logging_enabled() -> bool {
    GLOBAL_CONFIG.read().get_flag("log_to_file")
}

pub fn legacy_discovery() -> bool {
    GLOBAL_CONFIG.read().get_flag("legacy_discovery")
}

pub struct ArcStorage(std::path::PathBuf);

// TODO: Improve this by moving the account stuff in nnsdk in a module
impl ArcStorage {
    pub fn new() -> Self {
        let mut uid = nn::account::Uid { id: [0; 2] };
        let mut handle = UserHandle::new();

        unsafe {
            // It is safe to initialize multiple times.
            nn::account::Initialize();

            // This provides a UserHandle and sets the User in a Open state to be used.
            if !open_preselected_user(&mut handle) {
                panic!("OpenPreselectedUser returned false");
            }

            // Obtain the UID for this user
            get_user_id(&mut uid, &handle);
            // This closes the UserHandle, making it unusable, and sets the User in a Closed state.
            close_user(&handle);
            // Make sure we can't use Handle from here
            drop(handle);
        }

        let path = PathBuf::from(uid.id[0].to_string()).join(uid.id[1].to_string());

        Self(path)
    }
}

// Move to arcropolis-api so API users can read the configuration?
impl ConfigStorage for ArcStorage {
    fn initialize(&self) -> Result<(), ConfigError> {
        // TODO: Check if the SD is mounted or something
        let path = self.storage_path();

        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }

        Ok(())
    }

    fn root_path(&self) -> PathBuf {
        PathBuf::from("sd:/ultimate/arcropolis/config/")
    }

    fn storage_path(&self) -> PathBuf {
        self.root_path().join(&self.0)
    }
}
