#![allow(incomplete_features)] // for if_let_guard
#![feature(proc_macro_hygiene)]
#![feature(if_let_guard)]
#![feature(path_try_exists)]
#![feature(map_try_insert)] // for not overwriting previously stored hashes
#![feature(vec_into_raw_parts)]
#![allow(unaligned_references)]

use std::{fmt, path::{Path, PathBuf}, str::FromStr};

use log::LevelFilter;
use thiserror::Error;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

use parking_lot::RwLock;
use skyline::{hooks::InlineCtx, libc::c_char, nn};

mod chainloader;
mod config;
mod fs;
mod hashes;
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

#[macro_export]
macro_rules! reg_x {
    ($ctx:ident, $no:expr) => {
        unsafe { *$ctx.registers[$no].x.as_ref() }
    }
}

#[macro_export]
macro_rules! reg_w {
    ($ctx:ident, $no:expr) => {
        unsafe { *$ctx.registers[$no].w.as_ref() }
    }
}

/// Basic code for displaying an ARCropolis dialog error informing the user to check their logs, or enable them if they don't currently.
fn dialog_error<S: AsRef<str>>(msg: S) {
    if config::no_web_menus() {
        if config::file_logging_enabled() {
            error!("{}<br>See the latest log for more information.", msg.as_ref());
        } else {
            error!("{}<br>Enable file logging and run again for more information.", msg.as_ref());
        }
    } else {
        if config::file_logging_enabled() {
            skyline_web::DialogOk::ok(format!("{}<br>See the latest log for more information.", msg.as_ref()));
        } else {
            skyline_web::DialogOk::ok(format!("{}<br>Enable file logging and run again for more information.", msg.as_ref()));
        }
    }
}

#[derive(Error, Debug)]
pub struct InvalidOsStrError;

impl fmt::Display for InvalidOsStrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to convert from OsStr to &str")
    }
}

pub trait PathExtension {
    fn is_stream(&self) -> bool;
    fn has_extension<S: AsRef<str>>(&self, ext: S) -> bool;
    fn smash_hash(&self) -> Result<Hash40, InvalidOsStrError>;
}

impl PathExtension for Path {
    fn is_stream(&self) -> bool {
        static VALID_PREFIXES: &[&str] = &[
            "/stream;",
            "/stream:",
            "stream;",
            "stream:"
        ];

        VALID_PREFIXES.iter().any(|x| self.starts_with(*x))
    }

    fn has_extension<S: AsRef<str>>(&self, ext: S) -> bool {
        self.extension()
            .map(|x| x.to_str())
            .flatten()
            .map(|x| x == ext.as_ref())
            .unwrap_or(false)
    }

    fn smash_hash(&self) -> Result<Hash40, InvalidOsStrError> {
        if self.extension().is_none() {
            let hash = self
                .file_name()
                .map(|x| x.to_str())
                .flatten()
                .map(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok())
                .flatten()
                .map(|x| Hash40(x));
            if let Some(hash) = hash {
                return Ok(hash);
            }
        }
        let mut path = self
            .as_os_str()
            .to_str()
            .map_or(Err(InvalidOsStrError), |x| Ok(x))?
            .to_lowercase()
            .replace(";", ":");

        if let Some(regional_idx) = path.find("+") {
            path.replace_range(regional_idx..regional_idx+6, "")
        }

        Ok(Hash40::from(path.trim_start_matches("/")))
    }
}

/// Basic code for getting a hash40 from a path, ignoring things like if it exists
fn get_smash_hash<P: AsRef<Path>>(path: P) -> Result<Hash40, InvalidOsStrError> {
    path.as_ref().smash_hash()
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

#[skyline::hook(offset = offsets::initial_loading(), inline)]
fn initial_loading(_ctx: &InlineCtx) {
    let arc = resource::arc();
    replacement::lookup::initialize(Some(arc));
    let mut filesystem = GLOBAL_FILESYSTEM.write();
    *filesystem = filesystem.take().finish(arc).unwrap();
    filesystem.unshare(resource::arc_mut());
    filesystem.share_hashes(arc);
    filesystem.patch_sizes(resource::arc_mut());
}

#[skyline::hook(offset = offsets::title_screen_version())]
fn change_version_string(arg: u64, string: *const c_char) {
    let original_str = unsafe { skyline::from_c_str(string) };

    if original_str.contains("Ver.") {
        let new_str = format!(
            "Smash {}\nARCropolis Ver. {}\0",
            original_str,
            env!("CARGO_PKG_VERSION")
        );

        original!()(arg, skyline::c_str(&new_str))
    } else {
        original!()(arg, string)
    }
}

// 13.0.0
#[skyline::hook(offset = offsets::eshop_show())]
fn show_eshop() {
    println!("Eshop");
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
                std::thread::sleep(std::time::Duration::from_millis(5000));
                fs::perform_discovery()
            })
            .unwrap(),
    );

    let resources = std::thread::Builder::new()
        .stack_size(0x40000)
        .spawn(|| {
            hashes::init();
            replacement::lookup::initialize(None);
        })
        .unwrap();

    // Begin checking if there is an update to do. We do this in a separate thread so that we can install the hooks while we are waiting on GitHub response
    let _updater = std::thread::Builder::new()
        .stack_size(0x40000)
        .spawn(|| {
            if config::auto_update_enabled() {
                update::check_for_updates(config::beta_updates(), |update_kind| {
                    if config::no_web_menus() {
                        false
                    } else {
                        skyline_web::Dialog::yes_no(format!(
                            "{} has been detected. Do you want to install it?",
                            update_kind
                        ))
                    }
                });
            }
        })
        .unwrap();
    

    skyline::install_hooks!(
        initial_loading,
        change_version_string,
        show_eshop
    );
    replacement::install();

    // wait on updater to finish
    // let _ = updater.join();
    // Wait on hashes/lut to finish
    let _ = resources.join();
}
