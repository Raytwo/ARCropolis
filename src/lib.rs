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
#![feature(drain_filter)] // Removing additional paths from Modfile vec

use std::{
    fmt,
    path::Path,
    str::FromStr, collections::HashMap,
};

use arcropolis_api::Event;
use camino::{Utf8Path, Utf8PathBuf};
use log::LevelFilter;
use thiserror::Error;

#[macro_use]
extern crate log;

use once_cell::sync::{Lazy, OnceCell};
use parking_lot::{const_rwlock, RwLock};
use skyline::{hooks::InlineCtx, libc::c_char, nn};

mod api;
mod chainloader;
mod config;
mod fs;
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

use fs::LoadingState;
use smash_arc::{Hash40, Region, ArcLookup, hash40, LoadedArc, LoadedSearchSection};


use crate::{config::GLOBAL_CONFIG, replacement::{config::ModConfig, LoadedArcEx, SearchEx}, fs::ModFile};

// TODO: Use the interner instead of a Utf8PathBuf
pub static FILESYSTEM: OnceCell<HashMap<Hash40, Utf8PathBuf>> = OnceCell::new();

pub static LOADING_STATIC: Lazy<RwLock<LoadingState>> = Lazy::new(|| const_rwlock(LoadingState::default()));

static mut NEWS_DATA: Lazy<HashMap<String, String>> = Lazy::new(|| HashMap::new());

pub static CACHE_PATH: Lazy<Utf8PathBuf> = Lazy::new(|| {
    let path = utils::paths::cache().join(utils::get_game_version().to_string());

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

#[skyline::hook(replace = nn::fs::MountCacheStorage)]
fn mount_mod_cache_storage(_mountpoint: *const u8) -> u64 {
    0
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
    // Ryujinx crashes on user input initialization
    if !crate::utils::env::is_ryujinx() {
        // Open the ARCropolis menu if Minus is held before mod discovery
        if ninput::any::is_down(ninput::Buttons::PLUS) {
            crate::menus::show_main_menu();
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

#[skyline::hook(offset = offsets::initial_loading(), inline)]
fn initial_loading(_ctx: &InlineCtx) {
    #[cfg(feature = "online")]
    get_news_data();

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

    // Will be needed to store patched files
    // nn::fs::mount_cache_storage("cache");
    // skyline::install_hook!(mount_mod_cache_storage);
    // fuse::arc::install_arc_fs();
    api::event::send_event(Event::ArcFilesystemMounted);

    // Judging by observation, waiting 5 seconds for file discovery to start in a thread followed by joining here is actually a waste of time, as this function is called within 2 seconds and then has to wait anyways.
    let discovery_time = std::time::Instant::now();
    println!("Starting file discovery");
    let modpack = fs::perform_discovery();
    println!("File discovery took  {}s for {} mods", discovery_time.elapsed().as_secs_f32(), modpack.mods.len());

    // TODO: 1. Perform the conflict check here and display a web page
    // Remove all of the conflicting mods from the modpack
    let conflict_time = std::time::Instant::now();

    // Disabled for now
    let (modpack, conflicts) = fs::check_for_conflicts(modpack);
    println!("Conflict checks took {}s", conflict_time.elapsed().as_secs_f32());

    // TODO: Probably move this in the appropriate menu when the time comes
    // Walk through every conflict, removing them from the manager until there are none left
    // while let Some(conflict) = conflicts.next() {
    //     // TODO: Force the user to pick one
    //     // Add back the selected mod in the modpack
    //     modpack.mods.push(conflict.first);
    //     // Remove every future conflict involving the disabled mod
    //     conflicts.rebase(&conflict.second);
    // }

    // TODO 2: Get all of the "collectable" filepaths (plugins, configuration, patches...)
    // Maybe have separate methods to get NROs and patches?
    // let collectable_files: Vec<Modfile> = modpack.mods.iter.map(|mods| fs::get_collectable_files(mods)).collect().flatten();
    let collect_time = std::time::Instant::now();
    let (modpack, collected) = fs::collect_files(modpack);
    println!("File collecting took {}s", collect_time.elapsed().as_secs_f32());

    let mut mod_config: ModConfig = ModConfig::default();
    let configs: Vec<&ModFile> = collected.iter().filter(|path| path.path.file_name().unwrap() == "config.json").collect();

    // From 3.4.0
    configs.iter().for_each(|&path| {
        let cfg = std::fs::read_to_string(&path.path)
                .ok()
                .and_then(|x| serde_json::from_str::<ModConfig>(x.as_str()).ok());

        if let Some(cfg) = cfg {
            mod_config.merge(cfg);
        } else {
            warn!("Could not read/parse JSO data from file {}", path.path);
        }
    });

    // Directory addition should be performed first

    // File addition right after, so we can add files to the added directories too
    let arc = resource::arc();
    let mut context = LoadedArc::make_addition_context();
    let mut search_context = LoadedSearchSection::make_context();

    for path in mod_config.new_dir_infos.iter() {
        replacement::addition::prepare_file(&mut context, &Utf8Path::new(path))
    }

    for (new, base) in mod_config.new_dir_infos_base.iter() {
        replacement::addition::prepare_file_with_base(&mut context, &Utf8Path::new(new), &Utf8Path::new(base))
    }

    let new_files: Vec<&Utf8Path> = modpack.0.mods.iter().flat_map(|mods| mods.files.iter().map(move |file| file.path.strip_prefix(&mods.root).unwrap())).filter(|file| {
        arc.get_file_path_index_from_hash(hash40(file.as_str())).is_err()
    }).collect();

    for path in new_files {
        replacement::addition::add_file(&mut context, path);
        replacement::addition::add_searchable_file_recursive(&mut search_context, path);
    }

    resource::arc_mut().take_context(context);
    resource::search_mut().take_context(search_context);

    // TODO 3: Get what we need to build the ModFileSystem from the Modpack

    // What we need for this step: The modpack with conflicting files removed and collectable files taken away
    // let new_files = fs::get_additional_files();
    // replacement::perform_file_addition_idk(new_files).unwrap();

    let patching_time = std::time::Instant::now();
    let modpack = fs::patch_sizes(modpack);
    println!("File patching took {}s", patching_time.elapsed().as_secs_f32());

    let fs_time = std::time::Instant::now();
    let files = fs::acquire_filesystem(modpack);
    println!("Filesystem took {}s", fs_time.elapsed().as_secs_f32());
    println!("Total time is {}s", discovery_time.elapsed().as_secs_f32());

    FILESYSTEM.set(files).unwrap();

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
    // TODO: Set the is_busy variable and all
    #[cfg(feature = "web")]
    menus::show_main_menu();
}

#[skyline::hook(offset = 0x3778bf4, inline)]
unsafe fn msbt_text(ctx: &mut InlineCtx) {
    let msbt_label = skyline::from_c_str((ctx as *const InlineCtx as *const u8).add(0x100).add(224));

    if NEWS_DATA.contains_key(&msbt_label) {
        let mut text = NEWS_DATA.get(&msbt_label).unwrap().as_str().to_string();

        text.push_str("\0");

        let text_vec: Vec<u16> = text.encode_utf16().collect();
        *ctx.registers[0].x.as_mut() = text_vec.as_ptr() as u64;
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct PlayReport {
    pub event_id: [u8;32],
    pub buffer: *const u8,
    pub size: usize,
    pub position: usize
}

#[skyline::hook(offset = 0x39C4980)]
fn prepo_save(prepo: &PlayReport, uid: &nn::account::Uid) {
    skyline::logging::hex_dump_ptr(prepo as *const PlayReport);
    println!("Event id: {}", unsafe { skyline::from_c_str(prepo.event_id.as_ptr()) });
    skyline::logging::hex_dump_ptr(prepo.buffer);
    unsafe { 
        let mut buffer = std::io::Cursor::new(std::slice::from_raw_parts(prepo.buffer, prepo.position));
        let test = rmpv::decode::read_value(&mut buffer).unwrap();
        println!("{}", serde_json::to_string_pretty(&test).unwrap());
    }
    call_original!(dbg!(prepo), uid);
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

    // let resources = std::thread::Builder::new()
    //     .stack_size(0x40000)
    //     .spawn(|| {
    //         hashes::init();
    //         replacement::lookup::initialize(None);
    //     })
    //     .unwrap();

    // Begin checking if there is an update to do. We do this in a separate thread so that we can install the hooks while we are waiting on GitHub response
    #[cfg(feature = "online")]
    {
        std::thread::Builder::new()
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

    skyline::install_hooks!(initial_loading, change_version_string, show_eshop, prepo_save);
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
            "Skyline plugin as panicked! Please open the details and send a screenshot to the developer, then close the game.\0",
            err_msg.as_str(),
        );
    }));

    // Wait on hashes/lut to finish
    // let _ = resources.join();

    api::event::setup();
}
