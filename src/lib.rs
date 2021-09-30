#![allow(incomplete_features)] // for if_let_guard
#![feature(proc_macro_hygiene)]
#![feature(if_let_guard)]
#![feature(path_try_exists)]

use std::{path::PathBuf, str::FromStr};

use log::LevelFilter;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

use parking_lot::RwLock;
use skyline::{hooks::InlineCtx, nn};

mod config;
mod fs;
mod logging;
mod offsets;
mod resource;
mod update;

use fs::GlobalFilesystem;

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

fn init_time() {
    unsafe {
        if !nn::time::IsInitialized() {
            nn::time::Initialize();
        }
    }
}

fn get_version_string() -> String {
    unsafe {
        let mut version_string = nn::oe::DisplayVersion { name: [0x00; 16] };
        nn::oe::GetDisplayVersion(&mut version_string);
        skyline::from_c_str(version_string.name.as_ptr())
    }
}

#[skyline::hook(offset = offsets::initial_loading(), inline)]
fn initial_loading(_ctx: &InlineCtx) {
    let mut filesystem = GLOBAL_FILESYSTEM.write();
    *filesystem = filesystem.take().finish(resource::arc()).unwrap();
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    
    init_time();

    if let Err(err) = logging::init(
        LevelFilter::from_str(config::logger_level()).unwrap_or(LevelFilter::Warn),
    ) {
        println!(
            "[arcropolis] Failed to initialize logger. Reason: {:?}",
            err
        );
    }

    let mut filesystem = GLOBAL_FILESYSTEM.write();

    *filesystem = GlobalFilesystem::Promised(
        std::thread::Builder::new()
            .stack_size(0x40000)
            .spawn(|| {
                fs::perform_discovery()
            })
            .unwrap(),
    );

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

    let _ = updater.join();
}
