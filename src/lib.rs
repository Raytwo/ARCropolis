#![feature(proc_macro_hygiene)]

use std::str::FromStr;

use log::LevelFilter;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

use skyline::nn;

mod config;
mod logging;
mod update;

#[skyline::main(name = "arcropolis")]
pub fn main() {
    std::thread::Builder::new()
        .stack_size(0x40000)
        .spawn(|| {
            unsafe {
                if !nn::time::IsInitialized() {
                    nn::time::Initialize();
                }
            }
            if let Err(err) = logging::init(LevelFilter::from_str(config::logger_level()).unwrap_or(LevelFilter::Warn)) {
                println!("[arcropolis] Failed to initialize logger. Reason: {:?}", err);
            }
            
            if config::auto_update_enabled() {
                update::check_for_updates(true, |update_kind| {
                    skyline_web::Dialog::yes_no(format!("{} has been detected. Do you want to install it?", update_kind))
                });
                error!("Test log to std and file!");
                error!(target: "std", "Test log to only std!");
                error!(target: "file", "Test log to only file!");
            }
            log::logger().flush();
        })
        .unwrap()
        .join()
        .unwrap();
}
