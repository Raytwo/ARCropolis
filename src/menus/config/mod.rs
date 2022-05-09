// #![feature(proc_macro_hygiene)]

use std::{
    collections::HashSet,
    ffi::CString,
    path::{Path, PathBuf},
};

use log::info;
use serde::Deserialize;
use skyline::nn;
use skyline_config::{ConfigStorage, StorageHolder};
use skyline_web::{ramhorns, Visibility, Webpage};
use smash_arc::Hash40;

use crate::config;

#[derive(Debug, Deserialize)]
pub struct ConfigChanged {
    category: String,
    value: String,
}

// Is this trash? Yes
// Did I have a choice? No
pub fn show_config_editor<CS: ConfigStorage>(storage: &mut StorageHolder<CS>) {
    let mut reboot_required = false;

    let session = std::boxed::Box::new(
        Webpage::new()
            .htdocs_dir("contents")
            .file("index.html", &crate::menus::files::CONFIG_HTML_TEXT)
            .file("configurator.css", &crate::menus::files::CONFIG_CSS_TEXT)
            .file("configurator.js", &crate::menus::files::CONFIG_JAVASCRIPT_TEXT)
            .file("check.svg", &crate::menus::files::CHECK_SVG)
            .file("common.js", &crate::menus::files::COMMON_JAVASCRIPT_TEXT)
            .background(skyline_web::Background::Default)
            .boot_display(skyline_web::BootDisplay::Default)
            .open_session(Visibility::Default)
            .unwrap(),
    );

    // Loaded
    let _ = session.recv();

    if storage.get_flag("beta_updates") {
        session.send("beta");
    }

    if storage.get_flag("legacy_discovery") {
        session.send("legacy_discovery");
    }

    if storage.get_flag("debug") {
        session.send("debug");
    }

    if storage.get_flag("log_to_file") {
        session.send("log_to_file");
    }

    if storage.get_flag("auto_update") {
        session.send("auto_update");
    }

    let region: String = storage.get_field("region").unwrap();
    session.send(&region);

    let logging: String = storage.get_field("logging_level").unwrap();
    session.send(&logging);

    while let Ok(msg) = session.recv_json::<ConfigChanged>() {
        match msg.category.as_str() {
            "lang" => {
                let curr_value: String = storage.get_field("region").unwrap();
                session.send(&curr_value);
                storage.set_field("region", &msg.value).unwrap();
                session.send(&msg.value);
                info!("Set region to {}", &msg.value);
                reboot_required = true;
            },
            "log" => {
                let curr_value: String = storage.get_field("logging_level").unwrap();
                session.send(&curr_value);
                storage.set_field("logging_level", &msg.value).unwrap();
                session.send(&msg.value);
                info!("Set logger to {}", &msg.value);
            },
            // A "true" value is passed for flags, you might be wondering why.
            // If you pass ``null``, the browser closes, because Value is not a String or a Option. I think?
            // You can change it if you feel like it, I just didn't have it within me at this point
            "beta" => {
                let curr_value = !storage.get_flag("beta_updates");
                storage.set_flag("beta_updates", curr_value).unwrap();
                info!("Set beta update flag to {}", curr_value);
                session.send("beta");
            },
            "discovery" => {
                let curr_value = !storage.get_flag("legacy_discovery");
                storage.set_flag("legacy_discovery", curr_value).unwrap();
                info!("Set legacy_discovery flag to {}", curr_value);
                session.send("legacy_discovery");
            },
            "log_to_file" => {
                let curr_value = !storage.get_flag("log_to_file");
                storage.set_flag("log_to_file", curr_value).unwrap();
                info!("Set log_to_file flag to {}", curr_value);
                session.send("log_to_file");
            },
            "auto_update" => {
                let curr_value = !storage.get_flag("auto_update");
                storage.set_flag("auto_update", curr_value).unwrap();
                info!("Set auto_update flag to {}", curr_value);
                session.send("auto_update");
            },
            _ => break,
        }
    }

    session.exit();
    session.wait_for_exit();

    storage.flush();

    if reboot_required {
        skyline_web::DialogOk::ok(
            "Some important fields in the configuration have been changed. <br>Smash will now reboot to reload ARCropolis with the new changes.",
        );
        unsafe { skyline::nn::oe::RequestToRelaunchApplication() };
    }
}
