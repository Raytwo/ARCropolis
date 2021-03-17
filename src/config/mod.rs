use std::{fs, vec};
use std::path::PathBuf;
use std::fs::File;
use std::io::Write;
use std::net::Ipv4Addr;
use std::convert::From;
use log::LevelFilter;

use skyline::error::show_error;

use semver::Version;

use serde::{Deserialize, Serialize};

use parking_lot::RwLock;

mod readable;
mod writeable;
use readable::*;
use writeable::*;

const CONFIG_PATH: &str = "sd:/atmosphere/contents/01006A800016E000/romfs/arcropolis.toml";

lazy_static::lazy_static! {
    pub static ref CONFIG: Configuration = Configuration::new();
}

pub struct Configuration(RwLock<Config>);

impl<'rwlock> Configuration {
    pub fn new() -> Self {
        Self(RwLock::new(Config::open().unwrap()))
    }

    pub fn read(&self) -> ReadableConfig<'_> {
        ReadableConfig::new(self.0.read())
    }

    pub fn write(&self) -> WriteableConfig<'_> {
        WriteableConfig::new(self.0.write())
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub infos: Infos,
    pub paths: Paths,
    pub updater: Option<Updater>,
    pub logger: Option<Logger>,
    pub misc: Miscellaneous,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Infos {
    pub version: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Paths {
    pub arc: PathBuf,
    pub umm: PathBuf,
    pub extra_paths: Option<Vec<PathBuf>>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Updater {
    pub server_ip: Ipv4Addr,
    pub beta_updates: bool,
}

impl Updater {
    pub fn new() -> Updater {
        Updater {
            server_ip: "178.62.31.147".parse().unwrap(),
            beta_updates: false,
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum LoggerLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LoggerLevel> for LevelFilter {
    fn from(item: LoggerLevel) -> Self {
        match item {
            LoggerLevel::Off => LevelFilter::Off,
            LoggerLevel::Error => LevelFilter::Error,
            LoggerLevel::Warn => LevelFilter::Warn,
            LoggerLevel::Info => LevelFilter::Info,
            LoggerLevel::Debug => LevelFilter::Debug,
            LoggerLevel::Trace => LevelFilter::Trace,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Logger {
    pub logger_level: LoggerLevel,
}

impl Logger {
    pub fn new() -> Logger {
        Logger {
            logger_level: LoggerLevel::Info,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Miscellaneous {
    pub debug: bool,
    pub region: Option<String>,
}

impl Config {
    pub fn new() -> Self {
        Config {
            infos: Infos {
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            paths: Paths {
                arc: PathBuf::from("rom:/arc"),
                umm: PathBuf::from("sd:/ultimate/mods"),
                extra_paths: Some(vec![]),
            },
            updater: Some(Updater::new()),
            logger: Some(Logger::new()),
            misc: Miscellaneous {
                debug: false,
                region: Some(String::from("us_en")),
            },
        }
    }

    pub fn open() -> Result<Config, String> {
        match fs::read_to_string(CONFIG_PATH) {
            // File exists
            Ok(content) => {
                // Try deserializing
                let mut config = match toml::from_str(&content) {
                    // Deserialized properly
                    Ok(conf) => {
                        conf
                    },
                    // Something happened when deserializing
                    Err(_) => {
                        println!("[ARC::Config] Configuration file could not be deserialized");
                        show_error(69, "Configuration file could not be deserialized", &format!("Your configuration file ({}) is either poorly manually edited, outdated, corrupted or in a format unfit for ARCropolis.\n\nA new configuration file will now be generated, but it might ignore your modpacks. Consider double checking.", CONFIG_PATH));
                        println!("[ARC::Config] Generating configuration file...");
                        Config::new()
                    }
                };
    
                // Make sure the version matches with the current release
                if Version::parse(&config.infos.version) < Version::parse(&env!("CARGO_PKG_VERSION").to_string()) {
                    println!("[ARC::Config] Configuration file version mismatch");
                    skyline_web::DialogOk::ok("Updating configuration file to latest format");
                    println!("[ARC::Config] Changing version number...");

                    config.infos.version = env!("CARGO_PKG_VERSION").to_string();
                    config.update();
                    config.save().unwrap();

                    Ok(config)
                } else {
                    config.update();
                    config.save().unwrap();
                    Ok(config)
                }
            }
            // File does not exist, generate it
            Err(_) => {
                skyline_web::DialogOk::ok("Thank you for installing ARCropolis!\n\nConfiguration file will now be generated");
                println!("[ARC::Config] Configuration file not found. Generating a new one...");

                let config = Config::new();
                config.save().unwrap();

                Ok(config)
            }
        }
    }

    /// Should initialize missing fields in the struct when they get added
    fn update(&mut self) {
        self.infos.version = env!("CARGO_PKG_VERSION").to_string();

        match &self.paths.extra_paths {
            Some(_) => {},
            None => self.paths.extra_paths = Some(vec![]),
        }

        match &self.updater {
            Some(_) => {},
            None => self.updater = Some(Updater::new()),
        }

        match &self.logger {
            Some(_) => {},
            None => self.logger = Some(Logger::new()),
        }

        match &self.misc.region {
            Some(_) => {},
            None => self.misc.region = Some(String::from("us_en")),
        }
    }

    /// Automatically called when the WriteableConfig gets out of scope
    fn save(&self) -> Result<(), std::io::Error> {
        let config_txt = toml::to_vec(&self).unwrap();

        let mut file = match File::create(CONFIG_PATH) {
            Ok(file) => file,
            Err(err) => return Err(err),
        };

        match file.write_all(&config_txt) {
            Ok(_) => {},
            Err(err) => return Err(err),
        }

        println!("[ARC::Config] Configuration file successfully created");
        Ok(())
    }
}