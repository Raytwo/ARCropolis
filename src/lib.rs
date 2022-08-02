#![allow(incomplete_features)] // for if_let_guard
#![feature(proc_macro_hygiene)]
#![feature(if_let_guard)]
#![feature(path_try_exists)]
#![feature(map_try_insert)] // for not overwriting previously stored hashes
#![feature(vec_into_raw_parts)]
#![allow(unaligned_references)]
#![feature(string_remove_matches)]
#![feature(let_else)]

use std::{
    fmt,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

use arcropolis_api::Event;
use log::LevelFilter;
use thiserror::Error;

#[macro_use]
extern crate log;

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
mod menus;
mod offsets;
mod replacement;
mod resource;
#[cfg(feature = "updater")]
mod update;

use fs::GlobalFilesystem;
use smash_arc::Hash40;

use crate::config::GLOBAL_CONFIG;

pub static GLOBAL_FILESYSTEM: RwLock<GlobalFilesystem> = const_rwlock(GlobalFilesystem::Uninitialized);

pub static CACHE_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let version_string = get_version_string();
    let path = PathBuf::from("sd:/ultimate/arcropolis/cache").join(version_string);

    if let Err(e) = std::fs::create_dir_all(&path) {
        panic!("Unable to create cache directory! Reason: {:?}", e)
    }

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

/// Basic code for displaying an ARCropolis dialog error informing the user to check their logs, or enable them if they don't currently.
fn dialog_error<S: AsRef<str>>(msg: S) {
    let is_emulator = unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64 } == 0x8004000;

    if is_emulator {
        if config::file_logging_enabled() {
            error!("{}<br>See the latest log for more information.", msg.as_ref());
        } else {
            error!("{}<br>Enable file logging and run again for more information.", msg.as_ref());
        }
    } else if config::file_logging_enabled() {
        skyline_web::DialogOk::ok(format!("{}<br>See the latest log for more information.", msg.as_ref()));
    } else {
        skyline_web::DialogOk::ok(format!("{}<br>Enable file logging and run again for more information.", msg.as_ref()));
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
    fn to_str(&self) -> Option<&str>;
    fn is_stream(&self) -> bool;
    fn has_extension<S: AsRef<str>>(&self, ext: S) -> bool;
    fn smash_hash(&self) -> Result<Hash40, InvalidOsStrError>;
}

impl PathExtension for Path {
    fn to_str(&self) -> Option<&str> {
        self.as_os_str().to_str()
    }

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
                return Ok(hash);
            }
        }
        let mut path = self
            .as_os_str()
            .to_str()
            .map_or(Err(InvalidOsStrError), Ok)?
            .to_lowercase()
            .replace(';', ":")
            .replace(".mp4", ".webm");

        if let Some(regional_idx) = path.find('+') {
            path.replace_range(regional_idx..regional_idx + 6, "")
        }

        Ok(Hash40::from(path.trim_start_matches('/')))
    }
}

/// Basic code for getting a hash40 from a path, ignoring things like if it exists
fn get_smash_hash<P: AsRef<Path>>(path: P) -> Result<Hash40, InvalidOsStrError> {
    path.as_ref().smash_hash()
}

fn get_path_from_hash(hash: Hash40) -> PathBuf {
    if let Some(string) = hashes::try_find(hash) {
        PathBuf::from(string)
    } else {
        PathBuf::from(format!("{:#x}", hash.0))
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

/// Wrapper function for getting the version string of the game from nnSdk
fn get_version_string() -> String {
    unsafe {
        let mut version_string = nn::oe::DisplayVersion { name: [0x00; 16] };
        nn::oe::GetDisplayVersion(&mut version_string);
        skyline::from_c_str(version_string.name.as_ptr())
    }
}

fn check_for_changelog() {
    if let Ok(changelog) = std::fs::read_to_string("sd:/ultimate/arcropolis/changelog.toml") {
        match toml::from_str(&changelog) {
            Ok(changelog) => {
                menus::display_update_page(&changelog);
                std::fs::remove_file("sd:/ultimate/arcropolis/changelog.toml").unwrap();
            },
            Err(_) => {
                warn!("Changelog could not be parsed. Is the file malformed?");
            },
        }
    }
}

#[skyline::hook(offset = offsets::initial_loading(), inline)]
fn initial_loading(_ctx: &InlineCtx) {
    check_for_changelog();

    // menus::show_arcadia();
    let arc = resource::arc();
    fuse::arc::install_arc_fs();
    api::event::send_event(Event::ArcFilesystemMounted);
    replacement::lookup::initialize(Some(arc));
    let mut filesystem = GLOBAL_FILESYSTEM.write();
    *filesystem = filesystem.take().finish(arc).unwrap();
    filesystem.process_mods();
    filesystem.share_hashes();
    filesystem.patch_files();

    if config::debug_enabled() {
        let mut output = BufWriter::new(std::fs::File::create("sd:/ultimate/arcropolis/filesystem_dump.txt").unwrap());
        filesystem.get().walk_patch(|node, entry_type| {
            let depth = node.get_local().components().count() - 1;
            for _ in 0..depth {
                let _ = write!(output, "    ");
            }
            if entry_type.is_dir() {
                let _ = writeln!(output, "{}", node.get_local().display());
            } else {
                let _ = writeln!(output, "{}", node.full_path().display());
            }
        });
    }
    drop(filesystem);
    fuse::mods::install_mod_fs();
    api::event::send_event(Event::ModFilesystemMounted);
}

#[skyline::hook(offset = offsets::title_screen_version())]
fn change_version_string(arg: u64, string: *const c_char) {
    let original_str = unsafe { skyline::from_c_str(string) };

    if original_str.contains("Ver.") {
        let new_str = format!("Smash {}\nARCropolis Ver. {}\0", original_str, env!("CARGO_PKG_VERSION"));

        original!()(arg, skyline::c_str(&new_str))
    } else {
        original!()(arg, string)
    }
}

// pub struct UiSoundManager {
//     vtable: *const u8,
//     pub unk: *const u8,
// }

// #[skyline::from_offset(0x33135f0)]
// pub fn play_bgm(unk1: *const u8, some_hash: u64, unk3: bool);

// #[skyline::from_offset(0x336d810)]
// pub fn play_menu_bgm();

// #[skyline::from_offset(0x336d890)]
// pub fn stop_all_bgm();

#[skyline::hook(offset = offsets::eshop_show())]
fn show_eshop() {
    // stop_all_bgm();
    // let instance = (*(offsets::offset_to_addr(0x532d8d0) as *const u64));
    // play_bgm(instance as _, 0xd9ffff202a04c55b, false);
    menus::show_main_menu();
    // play_menu_bgm();
}

#[skyline::hook(offset = offsets::packet_send(), inline)]
unsafe fn packet_send(ctx: &InlineCtx) {
    let data = *ctx.registers[3].x.as_ref() as *mut u8;

    if data.is_null() {
        return;
    }

    if *(data as *const u64).add(0x28 / 8) & 0xFFFF0000_00000000 == 0xc1000000_00000000 {
        let slot = *data.add(0x38) & 0xF;
        if slot > 8 {
            *data.add(0x38) &= 0xF0;
        }
    }
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    // Initialize the time for the logger
    init_time();

    // Force the configuration to be initialized right away, so we can be sure default files exist (hopefully)
    Lazy::force(&GLOBAL_CONFIG);

    // Initialize hid
    let is_emulator = unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64 } == 0x8004000;

    if !is_emulator {
        ninput::init();
    }

    // Attempt to initialize the logger, and if we fail we will just do a regular println
    if let Err(err) = logging::init(LevelFilter::from_str(&config::logger_level()).unwrap_or(LevelFilter::Warn)) {
        println!("[arcropolis] Failed to initialize logger. Reason: {:?}", err);
    }

    // Acquire the filesystem and promise it to the initial_loading hook
    let mut filesystem = GLOBAL_FILESYSTEM.write();

    let discovery = std::thread::Builder::new()
        .stack_size(0x40000)
        .spawn(|| {
            unsafe {
                let curr_thread = nn::os::GetCurrentThread();
                nn::os::ChangeThreadPriority(curr_thread, 0);
            }
            std::thread::sleep(std::time::Duration::from_millis(5000));
            fs::perform_discovery()
        })
        .unwrap();

    *filesystem = GlobalFilesystem::Promised(discovery);

    let resources = std::thread::Builder::new()
        .stack_size(0x40000)
        .spawn(|| {
            hashes::init();
            replacement::lookup::initialize(None);
        })
        .unwrap();

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
                        // skyline_web::Dialog::yes_no(format!(
                        //     "{} has been detected. Do you want to install it?",
                        //     update_kind
                        // ))

                        // This didn't compile
                        skyline_web::Dialog::no_yes(format!("{} has been detected. Do you want to install it?", update_kind))

                        // match skyline_web::Dialog::yes_no(format!("{} has been detected. Do you want to install it?", update_kind)) {
                        //     true => true,
                        //     false => false,
                        // }
                    });
                }
            })
            .unwrap();
    }

    skyline::install_hooks!(initial_loading, change_version_string, show_eshop, packet_send);
    replacement::install();

    std::panic::set_hook(Box::new(|info| {
        let location = info.location().unwrap();

        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
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
    let _ = resources.join();

    api::event::setup();
}
