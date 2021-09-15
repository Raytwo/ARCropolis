use serde_derive::{Deserialize, Serialize};

static CONFIG_PATH: &'static str = "rom:/arcropolis.toml";
static CONFIG_WRITE_PATH: &'static str = "sd:/atmosphere/contents/01006A800016E000/romfs/arcropolis.toml";

lazy_static! {
    static ref GLOBAL_CONFIG: Config = {
        let config = match std::fs::read_to_string(CONFIG_PATH) {
            Ok(toml) => {
                match toml::de::from_str(toml.as_str()) {
                    Ok(config) => Config::from_intermediate(config),
                    Err(_) => {
                        println!("[arcropolis::config] Unable to read config file, generating new one.");
                        Config::new()
                    }
                }
            },
            Err(_) => {
                println!("[arcropolis::config] Unable to read config file, generating new one.");
                Config::new()
            }
        };

        match toml::ser::to_string_pretty(&config) {
            Ok(string) => {
                match std::fs::write(CONFIG_WRITE_PATH, string.as_bytes()) {
                    Err(_) => println!("[arcropolis::config] Unable to write config file."),
                    _ => {}
                }
            },
            Err(_) => println!("[arcropolis::config] Failed to serialize config data.")
        }

        config
    };
}

trait FromIntermediate<I> {
    fn from_intermediate(int: I) -> Self;
}

#[derive(Serialize)]
struct Config {
    pub version: String,
    pub debug: bool,
    pub auto_update: bool,
    pub paths: ConfigPaths,
    pub logger: ConfigLogger
}

impl Config {
    pub fn new() -> Self {
        Self {
            version: String::from(env!("CARGO_PKG_VERSION")),
            debug: false,
            auto_update: true,
            paths: ConfigPaths::new(),
            logger: ConfigLogger::new()
        }
    }
}

impl FromIntermediate<IConfig> for Config {
    fn from_intermediate(int: IConfig) -> Self {
        let version = String::from(env!("CARGO_PKG_VERSION")); // We don't care what the previous version was
        let IConfig { debug, auto_update, paths, logger, ..} = int;
        let debug = debug.unwrap_or(false);
        let auto_update = auto_update.unwrap_or(true);
        let paths = paths
            .map_or(ConfigPaths::new(), |x| ConfigPaths::from_intermediate(x));
        let logger = logger
            .map_or(ConfigLogger::new(), |x| ConfigLogger::from_intermediate(x));
        Self {
            version,
            debug,
            auto_update,
            paths,
            logger
        }
    }
}

#[derive(Deserialize)]
struct IConfig {
    pub version: Option<String>,
    pub debug: Option<bool>,
    pub auto_update: Option<bool>,
    pub paths: Option<IConfigPaths>,
    pub logger: Option<IConfigLogger>
}

#[derive(Serialize)]
struct ConfigPaths {
    pub arc: String,
    pub umm: String,
    pub extra_paths: Vec<String>
}

impl ConfigPaths {
    fn new() -> Self {
        Self {
            arc: String::from("rom:/arc"),
            umm: String::from("sd:/ultimate/mods"),
            extra_paths: Vec::new()
        }
    }
}

impl FromIntermediate<IConfigPaths> for ConfigPaths {
    fn from_intermediate(int: IConfigPaths) -> Self {
        let IConfigPaths { arc, umm, extra_paths } = int;
        let arc = arc.unwrap_or(String::from("rom:/arc"));
        let umm = umm.unwrap_or(String::from("sd:/ultimate/mods"));
        let extra_paths = extra_paths.unwrap_or(Vec::new());
        Self {
            arc,
            umm,
            extra_paths
        }
    }
}

#[derive(Deserialize)]
struct IConfigPaths {
    pub arc: Option<String>,
    pub umm: Option<String>,
    pub extra_paths: Option<Vec<String>>
}

#[derive(Serialize)]
struct ConfigLogger {
    pub logger_level: String,
    pub log_to_file: bool
}

impl ConfigLogger {
    pub fn new() -> Self {
        Self {
            logger_level: String::from("Warning"),
            log_to_file: false
        }
    }
}

impl FromIntermediate<IConfigLogger> for ConfigLogger {
    fn from_intermediate(int: IConfigLogger) -> Self {
        let IConfigLogger { logger_level, log_to_file } = int;

        let logger_level = logger_level.unwrap_or(String::from("Warning"));
        let log_to_file = log_to_file.unwrap_or(false);

        Self {
            logger_level,
            log_to_file
        }
    }
}

#[derive(Deserialize, Serialize)]
struct IConfigLogger {
    pub logger_level: Option<String>,
    pub log_to_file: Option<bool>
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