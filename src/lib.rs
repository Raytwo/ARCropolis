#![allow(incomplete_features)] // for if_let_guard
#![feature(proc_macro_hygiene)]
#![feature(if_let_guard)]
#![feature(path_try_exists)]

use std::{fmt, path::{Path, PathBuf}, str::FromStr};

use log::LevelFilter;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

use parking_lot::RwLock;
use skyline::{hooks::InlineCtx, nn};

mod chainloader;
mod config;
mod fs;
mod logging;
mod offsets;
mod resource;
mod replacement;
mod update;

use fs::GlobalFilesystem;
use smash_arc::Hash40;

lazy_static! {
    pub static ref GLOBAL_FILESYSTEM: RwLock<GlobalFilesystem> =
        RwLock::new(GlobalFilesystem::Uninitialized);
    
    pub static ref CACHE_PATH: PathBuf = {
        let version_string = get_version_string();
        let path = PathBuf::from("sd:/ultimate/arcropolis/cache").join(version_string);
        match std::fs::create_dir_all(&path) {
            Err(e) => panic!("Unable to create cache directory! Reason: {:?}", e),
            _ => {}
        }
        path
    };
}

/// Basic code for displaying an ARCropolis dialog error informing the user to check their logs, or enable them if they don't currently.
fn dialog_error<S: AsRef<str>>(msg: S) {
    if config::file_logging_enabled() {
        skyline_web::DialogOk::ok(format!("{}<br>See the latest log for more information.", msg.as_ref()));
    } else {
        skyline_web::DialogOk::ok(format!("{}<br>Enable file logging and run again for more information.", msg.as_ref()));
    }
}

pub struct InvalidOsStrError;

impl fmt::Debug for InvalidOsStrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to convert from OsStr to &str")
    }
}

/// Basic code for getting a hash40 from a path, ignoring things like if it exists
fn get_smash_hash<P: AsRef<Path>>(path: P) -> Result<Hash40, InvalidOsStrError> {
    let path = path
        .as_ref()
        .as_os_str()
        .to_str()
        .map_or(Err(InvalidOsStrError), |x| Ok(x))?
        .to_lowercase()
        .replace(";", ":");

    Ok(Hash40::from(path.trim_start_matches("/")))
}

/// Initializes the `nn::time` library, for creating a log file based off of the current time. For some reason Smash does not initialize this
fn init_time() {
    unsafe {
        if !nn::time::IsInitialized() {
            nn::time::Initialize();
        }
    }
}

/// Wrapper function for getting the version string of the game from nnSdk
fn get_version_string() -> String {
    unsafe {
        let mut version_string = nn::oe::DisplayVersion { name: [0x00; 16] };
        nn::oe::GetDisplayVersion(&mut version_string);
        skyline::from_c_str(version_string.name.as_ptr())
    }
}

/// The core functionality of ARCropolis, this is where we ensure the filesystem has finished being loaded and combine it with the data.arc
/// to create one cohesive Orbits layeredfs.
#[skyline::hook(offset = offsets::initial_loading(), inline)]
fn initial_loading(_ctx: &InlineCtx) {
    let mut filesystem = GLOBAL_FILESYSTEM.write();
    *filesystem = filesystem.take().finish(resource::arc()).unwrap();
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    
    // Initialize the time for the logger
    init_time();

    // Attempt to initialize the logger, and if we fail we will just do a regular println
    if let Err(err) = logging::init(
        LevelFilter::from_str(config::logger_level()).unwrap_or(LevelFilter::Warn),
    ) {
        println!(
            "[arcropolis] Failed to initialize logger. Reason: {:?}",
            err
        );
    }

    // Acquire the filesystem and promise it to the initial_loading hook
    let mut filesystem = GLOBAL_FILESYSTEM.write();

    *filesystem = GlobalFilesystem::Promised(
        std::thread::Builder::new()
            .stack_size(0x40000)
            .spawn(|| {
                fs::perform_discovery()
            })
            .unwrap(),
    );

    // Begin checking if there is an update to do. We do this in a separate thread so that we can install the hooks while we are waiting on GitHub response
    let updater = std::thread::Builder::new()
    .stack_size(0x40000)
    .spawn(|| {
        if config::auto_update_enabled() {
            update::check_for_updates(true, |update_kind| {
                skyline_web::Dialog::yes_no(format!(
                    "{} has been detected. Do you want to install it?",
                    update_kind
                ))
            });
        }
    })
    .unwrap();
    

    skyline::install_hooks!(initial_loading);
    replacement::install();

    // Wait on updater since we don't want to crash if we have to restart (can happen I suppose)
    let _ = updater.join();
}
