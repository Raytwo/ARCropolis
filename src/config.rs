use std::fs;
use std::fs::File;
use std::io::Write;
use std::net::Ipv4Addr;

use skyline::error::show_error;

use semver::Version;

use serde::{Deserialize, Serialize};

const CONFIG_PATH: &str = "sd:/atmosphere/contents/01006A800016E000/romfs/arcropolis.toml";

lazy_static::lazy_static! {
    pub static ref CONFIG: Config = Config::open().unwrap();
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub infos: Infos,
    pub paths: Paths,
    pub secret: Option<Secret>,
    pub misc: Miscellaneous,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Infos {
    pub version: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Paths {
    pub arc: String,
    pub umm: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Secret {
    pub ip: Ipv4Addr,
}

impl Secret {
    pub fn new() -> Secret {
        Secret {
            ip: "0.0.0.0".parse().unwrap(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Miscellaneous {
    pub debug: bool,
}

impl Config {
    pub fn new() -> Self {
        Config {
            infos: Infos {
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            paths: Paths {
                arc: "rom:/arc".to_string(),
                umm: "sd:/ultimate/mods".to_string(),
            },
            .. Config::default()
        }
    }

    pub fn open() -> Result<Config, String> {
        match fs::read_to_string(CONFIG_PATH) {
            // File exists
            Ok(content) => {
                // Try deserializing
                let mut config= match toml::from_str(&content) {
                    // Deserialized properly
                    Ok(conf) => conf,
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
                    show_error(420, "Configuration file version mismatch.", &format!("The version of your configuration file ({}) indicate that the file was generated with a different version of ARCropolis.\n\nThe version number in the config file will be updated to match this ARCropolis version.", CONFIG_PATH));
                    println!("[ARC::Config] Changing version number...");

                    config.infos.version = env!("CARGO_PKG_VERSION").to_string();
                    config.update();
                    config.save().unwrap();

                    Ok(config)
                } else {
                    Ok(config)
                }
            }
            // File does not exist, generate it
            Err(_) => {
                // TODO: Replace this soon-ish ( ͡° ͜ʖ ͡°)
                show_error(69, "Thank you for installing ARCropolis!\nConfiguration file will now be generated.", "Your installation of ARCropolis does not have a configuration file yet.\nSit tight while we create one for you!");
                println!("[ARC::Config] Configuration file not found. Generating a new one...");

                let config = Config::new();
                config.save().unwrap();

                Ok(config)
            }
        }
    }

    /// Should initialize missing fields in the struct when they get added
    fn update(&mut self) {
        match &self.secret {
            Some(_) => {},
            None => self.secret = Some(Secret::new()),
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
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