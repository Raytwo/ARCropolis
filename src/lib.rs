#![feature(proc_macro_hygiene)]
#![feature(if_let_guard)]

use std::str::FromStr;

use log::LevelFilter;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

use std::fmt;
use skyline::{hooks::InlineCtx, nn};
use orbits::{ConflictHandler, ConflictKind, DiscoverSystem, FileLoader, Orbit, StandardLoader, orbit::LaunchPad};
use parking_lot::RwLock;

mod config;
mod fs;
mod logging;
mod offsets;
mod update;

use fs::GlobalFilesystem;

use offsets::INITIAL_LOADING_OFFSET;
use smash_arc::LoadedArc;

lazy_static! {
    pub static ref GLOBAL_FILESYSTEM: RwLock<GlobalFilesystem> = RwLock::new(GlobalFilesystem::Uninitialized);
}

fn init_time() {
    unsafe {
        if !nn::time::IsInitialized() {
            nn::time::Initialize();
        }
    }
}

#[skyline::hook(offset = INITIAL_LOADING_OFFSET, inline)]
fn initial_loading(_ctx: &InlineCtx) {
    let mut filesystem = GLOBAL_FILESYSTEM.write();
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    offsets::search_offsets();
    skyline::install_hooks!(
        initial_loading
    );

    let mut filesystem = GLOBAL_FILESYSTEM.write();

    *filesystem = GlobalFilesystem::Promised(std::thread::Builder::new()
        .stack_size(0x40000)
        .spawn(|| {
            init_time();

            if let Err(err) = logging::init(
                LevelFilter::from_str(config::logger_level()).unwrap_or(LevelFilter::Warn),
            ) {
                println!(
                    "[arcropolis] Failed to initialize logger. Reason: {:?}",
                    err
                );
            }

            if config::auto_update_enabled() {
                update::check_for_updates(true, |update_kind| {
                    skyline_web::Dialog::yes_no(format!(
                        "{} has been detected. Do you want to install it?",
                        update_kind
                    ))
                });
            }

            fs::perform_discovery()
        })
        .unwrap()
    );
}
