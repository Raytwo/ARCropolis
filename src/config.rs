use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use smash_arc::Region;

use skyline_config::*;

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
fn default_arc_path() -> String { "rom:/arc".to_string() }
fn default_umm_path() -> String { "sd:/ultimate/mods".to_string() }
fn default_logger_level() -> String { "Warn".to_string() }
fn default_region() -> String { "us_en".to_string() }

lazy_static! {
    static ref GLOBAL_CONFIG: Config = {
        // TODO: Write something to convert the current config to skyline-config while still in Beta
        let config = match std::fs::read_to_string(&*CONFIG_PATH) {
            Ok(toml) => match toml::de::from_str(toml.as_str()) {
                Ok(config) => config,
                Err(_) => {
                    error!("Unable to read config file, generating new one.");
                    Config::new()
                }
            },
            Err(_) => {
                error!("Unable to read config file, generating new one.");
                Config::new()
            }
        };

        match toml::ser::to_string_pretty(&config) {
            Ok(string) => match std::fs::write(&*CONFIG_PATH, string.as_bytes()) {
                Err(_) => error!("Unable to write config file."),
                _ => {}
            },
            Err(_) => error!("Failed to serialize config data."),
        }

        // let mut storage = acquire_storage("arcropolis").unwrap();
        // storage.clear_storage();
        // storage.set_flag("beta_updates", true);
        // storage.set_field("logging_level", "Info").unwrap();

        // let get_set_flag = dbg!(storage.get_flag("beta_updates"));
        // let get_unset_flag = dbg!(storage.get_flag("debug"));
        // let get_existing_field: String = dbg!(storage.get_field("logging_level").unwrap());

        // storage.set_field_json("config", &config).unwrap();
        // let deserialized_config: Config = storage.get_field_json("config").unwrap();
        // storage.read_dir().unwrap().for_each(|dir| println!("{}", dir.unwrap().path().display()));

        config
    };
}

pub trait FromIntermediate<I> {
    fn from_intermediate(int: I) -> Self;
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
    #[serde(default = "default_arc_path")]
    pub arc: String,

    #[serde(default = "default_umm_path")]
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
    GLOBAL_CONFIG.auto_update
}

pub fn debug_enabled() -> bool {
    GLOBAL_CONFIG.debug
}

pub fn beta_updates() -> bool {
    GLOBAL_CONFIG.beta_updates
}

pub fn no_web_menus() -> bool {
    GLOBAL_CONFIG.no_web_menus
}

pub fn region() -> Region {
    const REGIONS: &[&str] = &[
        "jp_ja", "us_en", "us_fr", "us_es", "eu_en", "eu_fr", "eu_es", "eu_de", "eu_nl", "eu_it",
        "eu_ru", "kr_ko", "zh_cn", "zh_tw",
    ];

    Region::from(REGIONS.iter().position(|x| x == &GLOBAL_CONFIG.region).map(|x| (x + 1) as u32).unwrap_or(0))
}

pub fn region_str() -> &'static str {
    GLOBAL_CONFIG.region.as_str()
}

pub fn version() -> &'static str {
    GLOBAL_CONFIG.version.as_str()
}

pub fn arc_path() -> &'static str {
    GLOBAL_CONFIG.paths.arc.as_str()
}

pub fn umm_path() -> &'static str {
    GLOBAL_CONFIG.paths.umm.as_str()
}

pub fn extra_paths() -> &'static Vec<String> {
    &GLOBAL_CONFIG.paths.extra_paths
}

pub fn logger_level() -> &'static str {
    GLOBAL_CONFIG.logger.logger_level.as_str()
}

pub fn file_logging_enabled() -> bool {
    GLOBAL_CONFIG.logger.log_to_file
}
