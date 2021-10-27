use std::path::PathBuf;

use serde_derive::{Deserialize, Serialize};

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

lazy_static! {
    static ref GLOBAL_CONFIG: Config = {

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
