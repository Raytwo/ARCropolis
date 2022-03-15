use std::{path::PathBuf, collections::HashSet};

use serde::{Deserialize, Serialize};
use smash_arc::{Region, Hash40};

use skyline_config::*;
use walkdir::WalkDir;

use std::sync::Mutex;
use skyline::nn;


lazy_static! {
    static ref CONFIG_PATH: PathBuf = {
        let path = PathBuf::from("sd:/ultimate/arcropolis");
        match std::fs::create_dir_all(&path) {
            Err(_) => panic!("ARCropolis failed to find/create required directory 'sd:/ultimate/arcropolis'"),
            _ => {}
        }
        path.join("config.toml")
    };
}


fn arcropolis_version() -> String { env!("CARGO_PKG_VERSION").to_string() }
const fn always_true() -> bool { true }
const fn always_false() -> bool { false }
fn default_logger_level() -> String { "Warn".to_string() }
fn default_region() -> String { "us_en".to_string() }

lazy_static! {
    pub static ref GLOBAL_CONFIG: Mutex<StorageHolder<ArcStorage>> = {
        //let mut storage = acquire_storage("arcropolis").unwrap();
        let mut storage = StorageHolder::new(ArcStorage::new());

        let version: Result<semver::Version, _> = storage.get_field("version");

        // Version file does not exist
        if version.is_err() {
        // Check if a legacy configuration does exist
        match std::fs::read_to_string(&*CONFIG_PATH) {
            // Legacy configuration exists, try parsing it
            Ok(toml) => match toml::de::from_str::<Config>(toml.as_str()) {
                // Parsing successful, migrate everything to the new system
                Ok(config) => {
                    info!("Convert legacy config file to new system.");
                    storage.set_field("version", arcropolis_version()).unwrap();
                    storage.set_field("region", &config.region).unwrap();
                    storage.set_field("logging_level", &config.logger.logger_level).unwrap();
                    storage.set_field_json("extra_paths", &config.paths.extra_paths).unwrap();

                    storage.set_flag("auto_update", config.auto_update);
                    storage.set_flag("beta_updates", config.beta_updates);
                    storage.set_flag("debug", config.debug);
                    storage.set_flag("log_to_file", config.logger.log_to_file);

                    if &config.paths.arc != &arc_path(){
                        skyline::error::show_error(69, "Usage of custom ARC paths is deprecated. Please press details.", "Starting from ARCropolis 3.0.0, custom ARC paths have been deprecated in an effort to reduce user error.\nConsider moving your modpack to rom:/arc to keep using it.");
                    }

                    if &config.paths.umm != &umm_path(){
                        skyline::error::show_error(69, "Usage of custom UMM paths is deprecated. Please press details.", "Starting from ARCropolis 3.0.0, custom UMM paths have been deprecated in an effort to reduce user error.\nConsider moving your modpack to sd:/ultimate/mods to keep using it.");
                        // TODO: Offer to move it for the user if the default umm path doesn't already exist
                    }

                    let _ = std::fs::remove_file("sd:/ultimate/arcropolis/config.toml").ok();

                    let is_emulator = unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64 } == 0x8004000;

                    // Ryujinx cannot show the web browser, and a second check is performed during file discovery
                    if !is_emulator {
                        if skyline_web::Dialog::yes_no("Would you like to migrate your modpack to the new system?\nIt offers advantages such as:\nMod manager on the eShop button\nSeparate enabled mods per user profile\n\nIf you accept, disabled mods will be renamed to strip the period.") {
                            storage.set_field_json("presets", &convert_legacy_to_presets());
                        } else {
                            storage.set_flag("legacy_discovery", true);
                        }
                    }
                },
                // Parsing unsuccessful, generate default files and delete the broken config
                Err(_) => {
                    error!("Unable to parse legacy config file, generating new one.");
                    generate_default_config(&mut storage);
                    let _ = std::fs::remove_file("sd:/ultimate/arcropolis/config.toml").ok();
                }
            },
            // Could not read or find a legacy configuration
            Err(_) => {
                // Mount the debug storage (3.0.0-beta.3.2 up to 3.0.0)
                let mut debug_storage = StorageHolder::new(DebugSavedataStorage::new("arcropolis"));
                // Try to get the version field
                let version: Result<semver::Version, _> = debug_storage.get_field("version");

                match version {
                    // A configuration was found in the debug storage, move everything to the SD
                    Ok(version) => {
                        debug!("Detected debug savedata config from version {}. Migrating to the SD.", version);

                        // Go through each file in the debug storage
                        debug_storage.read_dir().unwrap().for_each(|file| {
                            let file = file.unwrap();
                            
                            // If the size is 0, we've found a flag
                            if file.metadata().unwrap().len() == 0 {
                                storage.set_flag(file.file_name(), true);
                            }
                            // A field was found
                            else {
                                let content: String = debug_storage.get_field(file.file_name()).unwrap();
                                storage.set_field(file.file_name(), &content).unwrap();
                            }
                        });

                        // Overwrite the migrated version field with the current one
                        storage.set_field("version", arcropolis_version()).unwrap();

                        // Delete what's on the storage
                        debug_storage.delete_storage();
                    },
                    Err(_) => {
                        println!("Unable to read legacy config file, generating default values.");
                        generate_default_config(&mut storage);
                    },
                }
            }
        }
    }
    

        storage.flush();
        Mutex::new(storage)
    };

    static ref REGION: Region = {
        const REGIONS: &[&str] = &[
            "jp_ja", "us_en", "us_fr", "us_es", "eu_en", "eu_fr", "eu_es", "eu_de", "eu_nl", "eu_it",
            "eu_ru", "kr_ko", "zh_cn", "zh_tw",
        ];

        Region::from(REGIONS.iter().position(|&x| {
            x == &region_str()
        }).map(|x| (x + 1) as u32).unwrap_or(0))
    };
}

fn generate_default_config<CS: ConfigStorage>(storage: &mut StorageHolder<CS>) {
    // Just so we don't keep outdated fields
    storage.clear_storage();

    storage.set_field("version", arcropolis_version()).unwrap();
    storage.set_field("region", "us_en").unwrap();
    storage.set_field("logging_level", "Warn").unwrap();
    storage.set_field_json("extra_paths", &Vec::<String>::new()).unwrap();
    storage.set_flag("auto_update", true);
    storage.set_field_json("presets", &HashSet::<Hash40>::new());
}

fn convert_legacy_to_presets() -> HashSet<Hash40> {
    let mut presets: HashSet<Hash40> = HashSet::new();

    if std::path::PathBuf::from(umm_path()).exists() {
        // TODO: Turn this into a map and use Collect
        for entry in WalkDir::new(umm_path()).max_depth(1).into_iter() {
                if let Ok(entry) = entry {
                    let path = entry.path();

                    // If the mod isn't disabled, add it to the preset
                    if path.file_name().map(|name| name.to_str()).flatten().map(|name| !name.starts_with(".")).unwrap_or(false) {
                        presets.insert(Hash40::from(path.to_str().unwrap()));
                    } else {
                        // TODO: Check if the destination already exists, because it'll definitely happen, and when someone opens an issue about it and you'll realize you knew ahead of time, you'll feel dumb. But right this moment, you decided not to do anything.
                        std::fs::rename(path, format!("sd:/ultimate/mods/{}", path.file_name().unwrap().to_str().unwrap()[1..].to_string())).unwrap();
                    }
                }
        }
    }

    presets
}

#[derive(Serialize, Deserialize)]
struct Config {
    #[serde(skip_deserializing)]
    #[serde(default = "arcropolis_version")]
    pub version: String,

    #[serde(default = "always_false")]
    pub debug: bool,
    
    #[serde(default = "always_true")]
    pub auto_update: bool,

    #[serde(default = "always_true")]
    pub beta_updates: bool,

    #[serde(default = "always_false")]
    pub no_web_menus: bool,

    #[serde(default = "default_region")]
    pub region: String,
    
    #[serde(default = "ConfigPaths::new")]
    pub paths: ConfigPaths,

    #[serde(default = "ConfigLogger::new")]
    pub logger: ConfigLogger,
}

impl Config {
    pub fn new() -> Self {
        Self {
            version: String::from(env!("CARGO_PKG_VERSION")),
            debug: false,
            auto_update: true,
            beta_updates: true,
            no_web_menus: false,
            region: String::from("us_en"),
            paths: ConfigPaths::new(),
            logger: ConfigLogger::new(),
        }
    }
}
#[derive(Serialize, Deserialize)]
struct ConfigPaths {
    pub arc: String,
    pub umm: String,

    #[serde(default)]
    pub extra_paths: Vec<String>,
}

impl ConfigPaths {
    fn new() -> Self {
        Self {
            arc: String::from("rom:/arc"),
            umm: String::from("sd:/ultimate/mods"),
            extra_paths: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ConfigLogger {
    #[serde(default = "default_logger_level")]
    pub logger_level: String,

    #[serde(default = "always_true")]
    pub log_to_file: bool,
}

impl ConfigLogger {
    pub fn new() -> Self {
        Self {
            logger_level: String::from("Warn"),
            log_to_file: false,
        }
    }
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

// Why? We can't really avoid it. Probably remove this after confirming.
pub fn no_web_menus() -> bool {
    GLOBAL_CONFIG.lock().unwrap().get_flag("no_web_menus")
}

pub fn region() -> Region {
    *REGION
}

pub fn region_str() -> String {
    let region: String = GLOBAL_CONFIG.lock().unwrap().get_field("region").unwrap_or(String::from("us_en"));
    region
}

pub fn version() -> String {
    let version: String = GLOBAL_CONFIG.lock().unwrap().get_field("version").unwrap_or(String::from(env!("CARGO_PKG_VERSION")));
    version
}

pub fn arc_path() -> String {
    String::from("rom:/arc")
}

pub fn umm_path() -> String {
    String::from("sd:/ultimate/mods")
}

pub fn extra_paths() -> Vec<String> {
    GLOBAL_CONFIG.lock().unwrap().get_field_json("extra_paths").unwrap_or(vec![])
}

pub fn logger_level() -> String {
    let level: String = GLOBAL_CONFIG.lock().unwrap().get_field("logging_level").unwrap_or(String::from("Warn"));
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

        Self(path.into())
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