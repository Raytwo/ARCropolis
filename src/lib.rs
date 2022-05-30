#![allow(incomplete_features)] // for if_let_guard
#![feature(proc_macro_hygiene)]
#![feature(if_let_guard)]
#![feature(path_try_exists)]
#![feature(map_try_insert)] // for not overwriting previously stored hashes
#![feature(vec_into_raw_parts)]
#![allow(unaligned_references)]
#![feature(allocator_api)]
#![feature(hash_drain_filter)]
#![feature(string_remove_matches)]

use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};

use arcropolis_api::Event;
use camino::{Utf8Path, Utf8PathBuf};
use log::LevelFilter;
use thiserror::Error;

#[macro_use] extern crate log;

use once_cell::sync::Lazy;
use parking_lot::{const_rwlock, RwLock};
use skyline::{hooks::InlineCtx, libc::c_char, nn};

mod api;
mod chainloader;
mod config;
mod fs;
mod fuse;
mod hashes;
mod logging;
#[cfg(feature = "web")]
mod menus;
mod offsets;
mod replacement;
mod resource;
#[cfg(feature = "updater")]
mod update;
mod utils;

use fs::PlaceholderFs;
use smash_arc::{Hash40, Region};


use crate::config::GLOBAL_CONFIG;

pub static GLOBAL_FILESYSTEM: Lazy<RwLock<PlaceholderFs>> = Lazy::new(|| const_rwlock(PlaceholderFs::default()));

pub static CACHE_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let path = PathBuf::from("sd:/ultimate/arcropolis/cache").join(utils::get_game_version().to_string());

    if let Err(e) = std::fs::create_dir_all(&path) { panic!("Unable to create cache directory! Reason: {:?}", e) }

    path
});

#[macro_export]
macro_rules! reg_x {
    ($ctx:ident, $no:expr) => {
        unsafe { *$ctx.registers[$no].x.as_ref() }
    };
}

#[macro_export]
macro_rules! reg_w {
    ($ctx:ident, $no:expr) => {
        unsafe { *$ctx.registers[$no].w.as_ref() }
    };
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
        static VALID_PREFIXES: &[&str] = &["/stream;", "/stream:", "stream;", "stream:"];

        VALID_PREFIXES.iter().any(|x| self.starts_with(*x))
    }

    fn has_extension<S: AsRef<str>>(&self, ext: S) -> bool {
        self.extension().and_then(|x| x.to_str()).map(|x| x == ext.as_ref()).unwrap_or(false)
    }

    fn smash_hash(&self) -> Result<Hash40, InvalidOsStrError> {
        if self.extension().is_none() {
            let hash = self
                .file_name()
                .and_then(|x| x.to_str())
                .and_then(
                    |x| {
                        if x.starts_with("0x") {
                            u64::from_str_radix(x.trim_start_matches("0x"), 16).ok()
                        } else {
                            None
                        }
                    },
                )
                .map(Hash40);
            if let Some(hash) = hash {
                return Ok(hash)
            }
        }
        let path = self
            .as_os_str()
            .to_str()
            .map_or(Err(InvalidOsStrError), Ok)?
            .to_lowercase()
            .replace(';', ":")
            .replace(".mp4", ".webm");

        let (path, _) = strip_region_from_path(path);

        Ok(Hash40::from(path.to_string().trim_start_matches('/')))
    }
}

/// Basic code for getting a hash40 from a path, ignoring things like if it exists
fn get_smash_hash<P: AsRef<Utf8Path>>(path: P) -> Result<Hash40, InvalidOsStrError> {
    Ok(Hash40::from(path.as_ref().as_str()))
}

fn get_path_from_hash(hash: Hash40) -> Utf8PathBuf {
    if let Some(string) = hashes::try_find(hash) {
        Utf8PathBuf::from(string)
    } else {
        Utf8PathBuf::from(format!("{:#x}", hash.0))
    }
}

fn get_region_from_suffix(suffix: &str) -> Option<Region> {
    // In this case, having a None region is the same as saying the provided region is incorrect.
    Region::from_str(suffix)
        .ok()
        .and_then(|region| if region == Region::None { None } else { Some(region) })
}

pub fn get_region_from_path<P: AsRef<Utf8Path>>(path: P) -> Option<Region> {
    // Take the filename so we don't have to deal with the extension
    let filename = path.as_ref().file_name().unwrap();

    if let Some(index) = filename.find('+') {
        // The rest of the filename is dropped, as we don't need it here
        let (_, end) = filename.split_at(index + 1);
        get_region_from_suffix(end)
    } else {
        None
    }
}

pub fn strip_region_from_path<P: AsRef<Utf8Path>>(path: P) -> (Utf8PathBuf, Option<Region>) {
    let path = path.as_ref();
    let mut filename = path.file_name().map(String::from).unwrap();

    if let Some(index) = filename.rfind('+') {
        // TODO: Need to make sure the file has an extension. Probably return a Result instead
        let period = filename.rfind('.').unwrap();
        let region: String = filename.drain(index..period).collect();
        // Remove the +
        (path.with_file_name(filename), get_region_from_suffix(&region[1..]))
    } else {
        (path.into(), None)
    }
}

pub const REGIONS: &[&str] = &[
    "jp_ja", "us_en", "us_fr", "us_es", "eu_en", "eu_fr", "eu_es", "eu_de", "eu_nl", "eu_it", "eu_ru", "kr_ko", "zh_cn", "zh_tw",
];

/// Initializes the `nn::time` library, for creating a log file based off of the current time. For some reason Smash does not initialize this
fn init_time() {
    unsafe {
        if !nn::time::IsInitialized() {
            nn::time::Initialize();
        }
    }
}

#[cfg(feature = "web")]
fn check_input_on_boot() {
    if !crate::utils::env::is_ryujinx() {
        // Open the ARCropolis menu if Minus is held before mod discovery
        if ninput::any::is_down(ninput::Buttons::PLUS) {
            crate::menus::show_main_menu();
        }
    }
}

#[skyline::hook(offset = offsets::initial_loading(), inline)]
fn initial_loading(_ctx: &InlineCtx) {
    #[cfg(feature = "web")]
    menus::changelog::check_for_changelog();

    #[cfg(feature = "web")]
    if config::first_boot() {
        if utils::env::is_ryujinx() {
            config::prompt_for_region()
        } else {
            skyline::error::show_error(69, "The web browser could not be opened", "The web browser is not available in this environment");
        }
    }

    #[cfg(feature = "web")]
    check_input_on_boot();

    // let arc = resource::arc();
    fuse::arc::install_arc_fs();
    api::event::send_event(Event::ArcFilesystemMounted);

    // Judging by observation, waiting 5 seconds for file discovery to start in a thread followed by joining here is actually a waste of time, as this function is called within 2 seconds and then has to wait anyways.
    let discovery_time = std::time::Instant::now();
    let modpack = fs::perform_discovery();
    println!("File discovery took  {}s for {} mods", discovery_time.elapsed().as_secs_f32(), modpack.mods.len());

    // TODO: Perform the conflict check here and display a web page

    // replacement::lookup::initialize(Some(arc));
    // let filesystem = GLOBAL_FILESYSTEM.write();
    // *filesystem = filesystem.take().finish().unwrap();
    // filesystem.process_mods();
    // filesystem.share_hashes();
    // filesystem.patch_files();

    // drop(filesystem);
    // fuse::mods::install_mod_fs();
    // api::event::send_event(Event::ModFilesystemMounted);
}

// TODO: Rewrite this to make use of my Layout research. This is called every time they change a string in a layout at the moment. This needs to be turned into a inline hook.
#[skyline::hook(offset = offsets::title_screen_version())]
fn change_version_string(arg: u64, string: *const c_char) {
    let original_str = unsafe { skyline::from_c_str(string) };

    if original_str.contains("Ver.") {
        let new_str = format!("Smash {}\nARCropolis Ver. {}\0", original_str, env!("CARGO_PKG_VERSION"));

        call_original!(arg, skyline::c_str(&new_str))
    } else {
        call_original!(arg, string)
    }
}

#[skyline::hook(offset = offsets::eshop_show())]
fn show_eshop(_lua_state: *const u8) {
    // Set the is_busy variable and all
    #[cfg(feature = "web")]
    menus::show_main_menu();
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    // Initialize the time for the logger
    init_time();

    // Initialize hid
    if !crate::utils::env::is_ryujinx() {
        println!("Initializing ninput");
        ninput::init();
    }

    // Attempt to initialize the logger, and if we fail we will just do a regular println
    if let Err(err) = logging::init(LevelFilter::from_str(&config::logger_level()).unwrap_or(LevelFilter::Warn)) {
        println!("[arcropolis] Failed to initialize logger. Reason: {:?}", err);
    }

    // Make sure the paths exist before doing anything
    utils::paths::ensure_paths_exist().expect("Paths should exist on the SD");

    // Force the configuration to be initialized right away, so we can be sure default files exist (hopefully)
    Lazy::force(&GLOBAL_CONFIG);

    // lazy_static::initialize(&GLOBAL_FILESYSTEM);
    // Acquire the filesystem and promise it to the initial_loading hook
    // let mut filesystem = GLOBAL_FILESYSTEM.write();

    // *filesystem = GlobalFilesystem::Promised(
    //     std::thread::Builder::new()
    //         .stack_size(0x40000)
    //         .spawn(|| {
    //             std::thread::sleep(std::time::Duration::from_millis(5000));
    //             fs::perform_discovery()
    //         })
    //         .unwrap(),
    // );

    // std::thread::Builder::new()
    //     .stack_size(0x40000)
    //     .spawn(|| {
    //         std::thread::sleep(std::time::Duration::from_millis(5000));
    //         fs::perform_discovery()
    //     })
    //     .unwrap();

    // let resources = std::thread::Builder::new()
    //     .stack_size(0x40000)
    //     .spawn(|| {
    //         hashes::init();
    //         replacement::lookup::initialize(None);
    //     })
    //     .unwrap();

    // Begin checking if there is an update to do. We do this in a separate thread so that we can install the hooks while we are waiting on GitHub response
    #[cfg(feature = "updater")]
    {
        let _updater = std::thread::Builder::new()
            .stack_size(0x40000)
            .spawn(|| {
                // Changed to pre because prerelease doesn't compile
                if !semver::Version::from_str(env!("CARGO_PKG_VERSION")).unwrap().pre.is_empty() {
                    update::check_for_updates(config::beta_updates(), |_update_kind| true);
                }

                if config::auto_update_enabled() {
                        update::check_for_updates(config::beta_updates(), |update_kind| {
                            skyline_web::Dialog::no_yes(format!("{} has been detected. Do you want to install it?", update_kind))
                        });
                    }
            })
            .unwrap();
    }

    skyline::install_hooks!(initial_loading, change_version_string, show_eshop,);
    replacement::install();

    std::panic::set_hook(Box::new(|info| {
        let location = info.location().unwrap();

        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => {
                match info.payload().downcast_ref::<String>() {
                    Some(s) => &s[..],
                    None => "Box<Any>",
                }
            },
        };

        let err_msg = format!("thread has panicked at '{}', {}", msg, location);
        skyline::error::show_error(
            69,
            "Skyline plugin as panicked! Please open the details and send a screenshot to the developer, then close the game.\n",
            err_msg.as_str(),
        );
    }));

    // Wait on hashes/lut to finish
    // let _ = resources.join();

    api::event::setup();
}
