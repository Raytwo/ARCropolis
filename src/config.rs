use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Mutex,
};

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use semver::Version;
use serde::{Deserialize, Serialize};
use skyline::nn;
use skyline_config::*;
use smash_arc::{Hash40, Region};
use walkdir::WalkDir;

use crate::utils::{self, get_arcropolis_version};

#[repr(u8)]
#[derive(Debug)]
pub enum SaveLanguageId {
    Japanese = 0,
    English,
    French,
    Spanish,
    German,
    Italian,
    Dutch,
    Russian,
    Chinese,
    Taiwanese,
    Korean,
}

impl From<u8> for SaveLanguageId {
    fn from(byte: u8) -> Self {
        match byte {
            0 => Self::Japanese,
            1 => Self::English,
            2 => Self::French,
            3 => Self::Spanish,
            4 => Self::German,
            5 => Self::Italian,
            6 => Self::Dutch,
            7 => Self::Russian,
            8 => Self::Chinese,
            9 => Self::Taiwanese,
            10 => Self::Korean,
            _ => Self::English,
        }
    }
}

pub static GLOBAL_CONFIG: Lazy<Mutex<StorageHolder<ArcStorage>>> = Lazy::new(|| {
    let mut storage = StorageHolder::new(ArcStorage::new());

    let version: Result<Version, _> = storage.get_field("version");

    if let Ok(config_version) = version {
            let curr_version = get_arcropolis_version();

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
                storage.set_field("version", get_arcropolis_version().to_string()).unwrap();
            }
    }
    else // Version file does not exist
    {
        generate_default_config(&mut storage).unwrap_or_else(|err| panic!("ARCropolis encountered an error when generating the default configuration: {}", err));
    }

    Mutex::new(storage)
});

fn generate_default_config<CS: ConfigStorage>(storage: &mut StorageHolder<CS>) -> Result<(), ConfigError> {
    info!("Populating ConfigStorage with default values.");

    // Just so we don't keep outdated fields
    storage.clear_storage();

    storage.set_field("version", get_arcropolis_version().to_string())?;
    storage.set_field("logging_level", "Warn")?;
    storage.set_flag("auto_update", true)?;
    storage.set_field_json("presets", &HashSet::<Hash40>::new())?;

    let mut default_workspace = HashMap::<&str, &str>::new();
    default_workspace.insert("Default", "presets");

    storage.set_field_json("workspace_list", &default_workspace)?;
    storage.set_field("workspace", "Default")
}

fn convert_legacy_to_presets() -> HashSet<Hash40> {
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
            // TODO: Check if the destination already exists, because it'll definitely happen, and when someone opens an issue about it and you'll realize you knew ahead of time, you'll feel dumb. But right this moment, you decided not to do anything.
            std::fs::rename(path, format!("sd:/ultimate/mods/{}", &path.file_name().unwrap().to_str().unwrap()[1..])).unwrap();
        }
    }

    presets
}

pub fn auto_update_enabled() -> bool {
    GLOBAL_CONFIG.lock().unwrap().get_flag("auto_update")
}

pub fn debug_enabled() -> bool {
    GLOBAL_CONFIG.lock().unwrap().get_flag("debug")
}

pub fn beta_updates() -> bool {
    GLOBAL_CONFIG.lock().unwrap().get_flag("beta_updates")
}

pub static REGION: RwLock<Region> = RwLock::new(Region::UsEnglish);

pub fn region() -> Region {
    *REGION.read()
}

pub fn region_str() -> String {
    let region: String = GLOBAL_CONFIG
        .lock()
        .unwrap()
        .get_field("region")
        .unwrap_or_else(|_| String::from("us_en"));
    region
}

pub fn logger_level() -> String {
    let level: String = GLOBAL_CONFIG
        .lock()
        .unwrap()
        .get_field("logging_level")
        .unwrap_or_else(|_| String::from("Warn"));
    level
}

pub fn file_logging_enabled() -> bool {
    GLOBAL_CONFIG.lock().unwrap().get_flag("log_to_file")
}

pub fn legacy_discovery() -> bool {
    GLOBAL_CONFIG.lock().unwrap().get_flag("legacy_discovery")
}

pub struct ArcStorage(std::path::PathBuf);

impl ArcStorage {
    pub fn new() -> Self {
        unsafe { nn::account::Initialize(); }

        // This provides a UserHandle and sets the User in a Open state to be used.
        let handle = nn::account::try_open_preselected_user().expect("TryOpenPreselectedUser should open the current user");
        // Obtain the UID for this user
        let uid = nn::account::get_user_id(&handle).expect("GetUserId should return a valid Uid");

        nn::account::close_user(handle);

        let path = PathBuf::from(uid.id[0].to_string()).join(uid.id[1].to_string());

        Self(path)
    }
}

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
