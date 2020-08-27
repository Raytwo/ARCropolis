use skyline::error::show_error;
use skyline::{c_str, nn};
use std::fs;
use std::fs::File;
use std::io::prelude::*;

use crate::log;

use serde::{Deserialize, Serialize};

const CONFIG_PATH: &str = "sd:/atmosphere/contents/01006A800016E000/arcropolis.toml";
const CONFIG_CURR_VERSION: &str = "1.0.0";


lazy_static::lazy_static! {
    pub static ref CONFIG: Config = init();
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub infos: Infos,
    pub paths: Paths,
}

#[derive(Serialize, Deserialize)]
pub struct Infos {
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct Paths {
    pub arc: String,
    pub stream: String,
    pub umm: String,
}

fn generate_config() -> Result<Config, &'static str> {
    // Create a new default configuration
    let config = Config {
        infos: Infos {
            version: CONFIG_CURR_VERSION.to_string(),
        },
        paths: Paths {
            arc: "rom:/arc".to_string(),
            stream: "rom:/arc/stream".to_string(),
            umm: "sd:/umm".to_string(),
        },
    };

    let config_txt = toml::to_string(&config).unwrap();

    let mut fhandle = nn::fs::FileHandle { handle: 0 as _ };

    unsafe {
        nn::fs::CreateFile(c_str(&(CONFIG_PATH.to_owned() + "\0")), 0);
        nn::fs::OpenFile(&mut fhandle, c_str(&(CONFIG_PATH.to_owned() + "\0")), 6);
        nn::fs::WriteFile(
            fhandle,
            0,
            c_str(&config_txt) as *const skyline::libc::c_void,
            config_txt.len() as u64,
            &nn::fs::WriteOption { flags: 1 },
        );
        nn::fs::CloseFile(fhandle);
    }

    log!("[ARC::Config] Configuration file successfully created");

    Ok(config)
}

fn init() -> Config {
    let config = match fs::read_to_string(CONFIG_PATH) {
        Ok(content) => {
            let config: Config = toml::from_str(&content).unwrap();

            if config.infos.version != CONFIG_CURR_VERSION {
                log!("[ARC::Config] Configuration file version mismatch");
                show_error(69, "Configuration file version mismatch.", &format!("The version of your configuration file ({}) indicate that the file is either outdated, corrupted or in a format unfit for ARCropolis.\n\nA new configuration file will now be generated, but it might ignore your modpacks. Consider double checking.", CONFIG_PATH));
                log!("[ARC::Config] Deleting configuration file...");
                unsafe {
                    nn::fs::DeleteFile(c_str(&(CONFIG_PATH.to_owned() + "\0")));
                }
                log!("[ARC::Config] Generating configuration file...");
                let config = generate_config().unwrap();
                log!("[ARC::Config] Configuration file successfully created");
            }

            config
        }
        Err(_) => {
            log!("[ARC::Config] Configuration file not found. Generating a new one...");
            show_error(69, "Thank you for installing ARCropolis!\nConfiguration file will now be generated.", "Your installation of ARCropolis does not have a configuration file yet.\nSit tight while we create one for you!");
            match generate_config() {
                Ok(config) => config,
                Err(err) => {
                    show_error(69, "Error during configuration creation.", &format!("An attempt to generate a configuration file at location {} has been met with failure.", CONFIG_PATH));
                    panic!(err);
                }
            }
        }
    };
    config
}
