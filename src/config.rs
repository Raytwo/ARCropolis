use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    str::FromStr,
};

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use semver::Version;
use skyline::nn;
use skyline_config::*;
use smash_arc::{Hash40, Region};
use walkdir::WalkDir;

use crate::utils;

pub static GLOBAL_CONFIG: Lazy<RwLock<StorageHolder<ArcStorage>>> = Lazy::new(|| {
    let mut storage = StorageHolder::new(ArcStorage::new());

    let version: Result<Version, _> = storage.get_field("version");

    if let Ok(config_version) = version {
        let curr_version = utils::get_arcropolis_version();

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
            storage.set_field("version", utils::get_arcropolis_version().to_string()).unwrap();
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

    storage.set_field("version", utils::get_arcropolis_version().to_string())?;
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

    // TODO: Turn this into a map and use Collect
    for entry in WalkDir::new(utils::paths::mods()).max_depth(1).into_iter().flatten() {
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

    presets
}

#[cfg(feature = "web")]
pub fn prompt_for_region() {
    if first_boot() {
        if skyline_web::Dialog::yes_no("A default configuration for ARCropolis has been created.<br>It is important that your region matches your console's and the language matches the one in Smash.<br>By default, it is set to American English. Would you like to adjust it?") {
            crate::menus::show_config_editor(&mut GLOBAL_CONFIG.write());
        }
        set_first_boot(false);
    }
}

pub mod workspaces {
    use super::*;
    use std::collections::HashMap;

    use skyline_config::ConfigError;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum WorkspaceError {
        #[error("a configuration error happened: {0}")]
        ConfigError(#[from] ConfigError),
        #[error("a workspace with this name already exists")]
        AlreadyExists,
        #[error("failed to find workspace with name: {0}")]
        MissingWorkspace(String)
        // #[error("failed to call from_str for the desired type")]
        // FromStrErr,
    }

    pub fn get_list() -> Result<HashMap<String, String>, WorkspaceError> {
        GLOBAL_CONFIG.read().get_field_json("workspace_list").map_err(WorkspaceError::ConfigError)
    }

    pub fn create_new_workspace(name: String) -> Result<(), WorkspaceError> {
        let mut list = get_list()?;

        if let std::collections::hash_map::Entry::Vacant(e) = list.entry(name) {
            todo!("Implement code to generate a preset name");
            e.insert("temp".to_string());
            GLOBAL_CONFIG.write().set_field_json("workspace_list", &list).map_err(WorkspaceError::ConfigError)
        } else {
            Err(WorkspaceError::AlreadyExists)
        }
    }

    pub fn set_active_workspace(name: String) -> Result<(), WorkspaceError> {
        let workspace_list = get_list()?;
        // Make sure the workspace actually exists before setting it
        if workspace_list.contains_key(&name) {
            // If we couldn't write the new active workspace, return an error
            GLOBAL_CONFIG.write().set_field("workspace", name).map_err(WorkspaceError::ConfigError)
        } else {
            // Couldn't find the workspace in our list, something is wrong
            Err(WorkspaceError::MissingWorkspace(name))
        }
    }

    pub fn get_active_workspace() -> Result<String, WorkspaceError> {
        let workspace_list = get_list()?;
        let workspace_name: String = GLOBAL_CONFIG.read().get_field("workspace")?;
        workspace_list.get(&workspace_name).map(|x| x.to_owned()).ok_or(WorkspaceError::MissingWorkspace(workspace_name))
    }

    fn get_workspace_by_name(name: String) -> Result<String, WorkspaceError> {
        let workspace_list = get_list()?;
        workspace_list.get(&name).map(|x| x.to_owned()).ok_or(WorkspaceError::MissingWorkspace(name))
    }

    pub fn rename_workspace(from: &str, to: &str) -> Result<(), WorkspaceError> {
        let mut workspace_list = get_list()?;
        // Remove the workspace if we find it and get back the associate preset name, but if we don't, return an error.
        let preset_name = workspace_list.remove(from).ok_or_else(|| WorkspaceError::MissingWorkspace(from.to_string()))?;
        // Reinsert the preset name with the new workspace name
        workspace_list.insert(to.to_string(), preset_name);
        // Overwrite the list with the changes
        GLOBAL_CONFIG.write().set_field_json("workspace_list", &workspace_list).map_err(WorkspaceError::ConfigError)
    }
}

pub mod presets {
    use super::*;
    use std::collections::HashSet;

    use once_cell::sync::Lazy;
    use skyline_config::ConfigError;
    use smash_arc::Hash40;
    use thiserror::Error;

    use super::workspaces::WorkspaceError;

    #[derive(Debug, Error)]
    pub enum PresetError {
        #[error("a configuration error happened: {0}")]
        ConfigError(#[from] ConfigError),
        #[error("a workspace error happened: {0}")]
        WorkspaceError(#[from] WorkspaceError),
        #[error("failed to find the preset file for this workspace")]
        MissingPreset,
        // #[error("failed to call from_str for the desired type")]
        // FromStrErr,
    }

    pub fn get_active_preset() -> Result<HashSet<Hash40>, PresetError> {
        let preset_name = workspaces::get_active_workspace()?;
        GLOBAL_CONFIG.read().get_field_json(preset_name).map_err(PresetError::ConfigError)
    }

    pub fn replace_active_preset(preset: &HashSet<Hash40>) -> Result<(), PresetError> {
        let preset_name = workspaces::get_active_workspace()?;
        GLOBAL_CONFIG.write().set_field_json(preset_name, preset).map_err(PresetError::ConfigError)
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
    let region: String = GLOBAL_CONFIG.read().get_field("region").unwrap_or_else(|_| String::from("us_en"));
    region
}

pub fn version() -> String {
    let version: String = GLOBAL_CONFIG
        .read()
        .get_field("version")
        .unwrap_or_else(|_| utils::get_arcropolis_version().to_string());
    version
}

pub fn logger_level() -> String {
    let level: String = GLOBAL_CONFIG.read().get_field("logging_level").unwrap_or_else(|_| String::from("Warn"));
    level
}

pub fn file_logging_enabled() -> bool {
    GLOBAL_CONFIG.read().get_flag("log_to_file")
}

pub fn legacy_discovery() -> bool {
    GLOBAL_CONFIG.read().get_flag("legacy_discovery")
}

pub fn first_boot() -> bool {
    GLOBAL_CONFIG.read().get_flag("first_boot")
}

pub fn set_first_boot(enabled: bool) {
    GLOBAL_CONFIG.write().set_flag("first_boot", enabled).unwrap();
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
        utils::paths::config().into_std_path_buf()
    }

    fn storage_path(&self) -> PathBuf {
        self.root_path().join(&self.0)
    }
}
