#![allow(incomplete_features)] // for if_let_guard
#![feature(proc_macro_hygiene)]
#![feature(if_let_guard)]
#![feature(map_try_insert)] // for not overwriting previously stored hashes
#![feature(vec_into_raw_parts)]
#![feature(string_remove_matches)]
// #![feature(fs_try_exists)]
#![feature(int_roundings)]

use std::{
    collections::HashMap,
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
mod fixes;
mod fs;
mod fuse;
mod hashes;
mod logging;
mod menus;
mod offsets;
mod replacement;
mod resource;
#[cfg(feature = "online")]
mod update;
mod utils;
mod lua;

use fs::GlobalFilesystem;
use smash_arc::{Hash40, Region};

use crate::{
    config::{GLOBAL_CONFIG, REGION},
    utils::save::{get_language_id_in_savedata, get_system_region_from_language_id, mount_save, unmount_save},
};

pub static GLOBAL_FILESYSTEM: RwLock<GlobalFilesystem> = const_rwlock(GlobalFilesystem::Uninitialized);

static mut NEWS_DATA: Lazy<HashMap<String, String>> = Lazy::new(HashMap::new);

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
    if utils::env::is_emulator() {
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
            .replace(".mp4", ".webm")
            .replace(".lua", ".lc");

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

fn init_account() {
    // It is safe to initialize multiple times
    unsafe { nn::account::Initialize() }
}

#[cfg(feature = "online")]
fn check_for_changelog() {
    if !crate::utils::env::is_emulator() {
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
}

#[cfg(feature = "online")]
fn get_news_data() {
    skyline::install_hook!(msbt_text);
    match minreq::get("https://coolsonickirby.com/arc/news").send() {
        Ok(resp) => match resp.json::<HashMap<String, String>>() {
            Ok(info) => unsafe { NEWS_DATA.extend(info) },
            Err(err) => println!("{:?}", err),
        },
        Err(err) => println!("{:?}", err),
    }
}

#[cfg(feature = "online")]
fn check_input_on_boot() {
    if !crate::utils::env::is_emulator() {
        // Open the ARCropolis menu if Minus is held before mod discovery
        if ninput::any::is_down(ninput::Buttons::PLUS) {
            crate::menus::show_main_menu();
        }
    }
}

#[cfg(feature = "online")]
fn check_for_update() {
    // Changed to pre because prerelease doesn't compile
    if !semver::Version::from_str(env!("CARGO_PKG_VERSION")).unwrap().pre.is_empty() {
        update::check_for_updates(config::beta_updates(), |_, _, _| true);
    }

    if config::auto_update_enabled() {
        update::check_for_updates(config::beta_updates(), |update_kind, date, description| {
            let (contributors, entries) = menus::get_entries_from_md(description);
            let main_entry = menus::MainEntry {
                title: format!("ARCropolis update: Ver. {}", update_kind),
                date,
                description: "A new version of ARCropolis was detected!<br/>Please read the following changelog.".to_string(),
                entries,
                contributors,
            };

            menus::display_update_page(&main_entry)
            // skyline_web::Dialog::no_yes(format!("{} has been detected. Do you want to install it?", update_kind))
        });
    }
}

#[skyline::hook(offset = offsets::initial_loading(), inline)]
fn initial_loading(_ctx: &InlineCtx) {
    // #[cfg(feature = "online")]
    // check_for_changelog();

    // Begin checking if there is an update to do. We do this in a separate thread so that we can install the hooks while we are waiting on GitHub response
    #[cfg(feature = "online")]
    let _updater = std::thread::Builder::new()
        .stack_size(0x10000)
        .spawn(|| {
            check_for_update();
        })
        .unwrap();

    // Commented out until we get an actual news server
    // #[cfg(feature = "online")]
    // get_news_data();

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

    #[cfg(feature = "online")]
    _updater.join().unwrap();
}

#[skyline::hook(offset = offsets::title_screen_version())]
fn change_version_string(arg: u64, string: *const c_char) {
    let original_str = unsafe { skyline::from_c_str(string) };

    if original_str.contains("Ver.") {
        let new_str = format!(
            "Smash {}\nARCropolis Ver. {}\0",
            original_str,
            crate::utils::env::get_arcropolis_version()
        );

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

#[skyline::hook(offset = offsets::eshop_button())]
fn show_eshop() {
    // stop_all_bgm();
    // let instance = (*(offsets::offset_to_addr(0x532d8d0) as *const u64));
    // play_bgm(instance as _, 0xd9ffff202a04c55b, false);
    menus::show_main_menu();
    // play_menu_bgm();
}

#[skyline::hook(offset = offsets::msbt_text(), inline)]
unsafe fn msbt_text(ctx: &mut InlineCtx) {
    let msbt_label = skyline::from_c_str((ctx as *const InlineCtx as *const u8).add(0x100).add(224));

    if NEWS_DATA.contains_key(&msbt_label) {
        let mut text = NEWS_DATA.get(&msbt_label).unwrap().as_str().to_string();

        text.push('\0');

        let text_vec: Vec<u16> = text.encode_utf16().collect();
        *ctx.registers[0].x.as_mut() = text_vec.as_ptr() as u64;
    }
}

#[skyline::hook(offset = offsets::packet_send(), inline)]
unsafe fn online_slot_spoof(ctx: &InlineCtx) {
    let data = *ctx.registers[3].x.as_ref() as *mut u8;

    if data.is_null() {
        return;
    }

    if *(data as *const u64).add(0x28 / 8) & 0xFFFF_0000_0000_0000 == 0xc100_0000_0000_0000 {
        // Change the slot (lower 4 bits) to slot % 8
        *data.add(0x38) &= 0xF7;
    }
}

pub fn is_online() -> bool {
    unsafe {
        *(offsets::offset_to_addr(offsets::is_online()) as *const bool)
    }
}

// Thanks to blujay for these two function hooks
#[skyline::hook(offset = offsets::change_color_r(), inline)]
unsafe fn change_fighter_color_r(ctx: &mut skyline::hooks::InlineCtx) {
    if is_online() {
        unsafe {
            if *ctx.registers[8].w.as_ref() >= 8 {
                *ctx.registers[8].w.as_mut() = 0; // Actual color
                *ctx.registers[3].w.as_mut() = 0; // UI
            }
        }
    }
}

#[skyline::hook(offset = offsets::change_color_l(), inline)]
unsafe fn change_fighter_color_l(ctx: &mut skyline::hooks::InlineCtx) {
    if is_online() {
        unsafe {
            if *ctx.registers[8].w.as_ref() >= 8 {
                // Assuming that if they can change a character's color then that means a character has at least a set of 8 colors
                *ctx.registers[8].w.as_mut() = 7; // Actual color
                *ctx.registers[3].w.as_mut() = 7; // UI
            }
        }
    }
}

#[skyline::hook(offset = offsets::skip_opening(), inline)]
unsafe fn skip_opening_cutscene(ctx: &mut InlineCtx) {
    let data = ctx.registers[8].x.as_mut();
    *data = 0;
}

// Change the next callback for the TitleSceneInfo::callbacks::Enter from "DisplayOpeningCutscene" to "HowToPlay"
#[skyline::hook(offset = offsets::title_scene_play_opening(), inline)]
unsafe fn title_scene_play_opening(ctx: &mut InlineCtx) {
    let data = ctx.registers[9].x.as_mut();
    *data = 1;
}

// Pretend the state for another state handler (OpeningCutsceneLayout?) is set to 5
#[skyline::hook(offset = offsets::title_scene_how_to_play(), inline)]
unsafe fn title_scene_show_how_to_play_fake_state_index(ctx: &mut InlineCtx) {
    let data = ctx.registers[8].x.as_mut();
    *data = 5;
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().unwrap();

        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
            },
        };

        let err_msg = format!("ARCropolis has panicked at '{}', {}", msg, location);
        skyline::error::show_error(
            69,
            "ARCropolis has panicked! Please open the details and send a screenshot to the developer, then close the game.\n\0",
            err_msg.as_str(),
        );
    }));

    if utils::env::get_game_version() != semver::Version::new(13, 0, 1) {
        skyline_web::DialogOk::ok(
            "ARCropolis cannot currently run on a Smash version lower than 13.0.1<br/>Consider updating your game or uninstalling ARCropolis.",
        );
        // Do not perform any of the hook installation and let the game proceed as normal.
        return;
    }

    // Initialize the time for the logger
    init_time();
    // Required to mount the savedata ourselves
    init_account();

    // Initialize hid
    if !utils::env::is_emulator() {
        println!("Initializing ninput");
        ninput::init();
    }

    // Make sure the paths exist before doing anything
    utils::paths::ensure_paths_exist().expect("Paths should exist on the SD");

    // Scope to drop the lock
    {
        let mut region = REGION.write();
        mount_save("save\0");
        let language_id = get_language_id_in_savedata();
        unmount_save("save\0");
        // Read the user's region + language from the game ourselves because the game hasn't done it yet
        // Default to UsEnglish if there is no Save Data on this boot
        match language_id {
            Ok(id) => *region = get_system_region_from_language_id(id),
            Err(_) => *region = Region::UsEnglish,
        }
    }

    // Force the configuration to be initialized right away, so we can be sure default files exist (hopefully)
    Lazy::force(&GLOBAL_CONFIG);

    // Attempt to initialize the logger, and if we fail we will just do a regular println
    if let Err(err) = logging::init(LevelFilter::from_str(&config::logger_level()).unwrap_or(LevelFilter::Warn)) {
        println!("[arcropolis] Failed to initialize logger. Reason: {:?}", err);
    }

    // Acquire the filesystem and promise it to the initial_loading hook
    let mut filesystem = GLOBAL_FILESYSTEM.write();

    let discovery = std::thread::Builder::new()
        .stack_size(0x10000)
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
        .stack_size(0x10000)
        .spawn(|| {
            hashes::init();
            replacement::lookup::initialize(None);
        })
        .unwrap();

    skyline::install_hooks!(initial_loading, change_version_string, show_eshop, online_slot_spoof, change_fighter_color_l, change_fighter_color_r);

    // If we skip the title scene, we obviously skip the opening cutscene with it. Well, actually not necessarily but in this case we do.
    if config::skip_title_scene() {
        skyline::install_hooks!(title_scene_play_opening, title_scene_show_how_to_play_fake_state_index);
    } else if config::skip_cutscene() {
        skyline::install_hook!(skip_opening_cutscene);
    }

    replacement::install();
    fixes::install();
    lua::install();

    // Wait on hashes/lut to finish
    let _ = resources.join();

    api::event::setup();
}
