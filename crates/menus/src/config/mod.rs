// #![feature(proc_macro_hygiene)]

use serde::Deserialize;
use skyline_config::{ConfigStorage, StorageHolder};
use skyline_web::{Visibility, Webpage};

#[derive(Debug, Deserialize)]
pub struct ConfigChanged {
    category: String,
    value: String,
}

// Is this trash? Yes
// Did I have a choice? No
pub fn show_config_editor<CS: ConfigStorage>(storage: &mut StorageHolder<CS>) {
    let reboot_required = false;

    let session = std::boxed::Box::new(
        Webpage::new()
            .htdocs_dir("contents")
            .file("index.html", &crate::files::CONFIG_HTML_TEXT)
            .file("configurator.css", &crate::files::CONFIG_CSS_TEXT)
            .file("configurator.js", &crate::files::CONFIG_JAVASCRIPT_TEXT)
            .file("check.svg", &crate::files::CHECK_SVG)
            .file("common.js", &crate::files::COMMON_JAVASCRIPT_TEXT)
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

    if storage.get_flag("skip_cutscene") {
        session.send("skip_cutscene");
    }

    if storage.get_flag("skip_title_scene") {
        session.send("skip_title_scene");
    }

    if storage.get_flag("use_folder_name") {
        session.send("use_folder_name");
    }

    let logging: String = storage.get_field("logging_level").unwrap_or(String::from("Info"));
    session.send(&logging);

    while let Ok(msg) = session.recv_json::<ConfigChanged>() {
        match msg.category.as_str() {
            "log" => {
                let curr_value: String = storage.get_field("logging_level").unwrap_or(String::from("Info"));
                session.send(&curr_value);
                storage.set_field("logging_level", &msg.value).unwrap();
                session.send(&msg.value);
                // info!("Set logger to {}", &msg.value);
            },
            // A "true" value is passed for flags, you might be wondering why.
            // If you pass ``null``, the browser closes, because Value is not a String or a Option. I think?
            // You can change it if you feel like it, I just didn't have it within me at this point
            "beta" => {
                let curr_value = !storage.get_flag("beta_updates");
                storage.set_flag("beta_updates", curr_value).unwrap();
                // info!("Set beta update flag to {}", curr_value);
                session.send("beta");
            },
            "discovery" => {
                let curr_value = !storage.get_flag("legacy_discovery");
                storage.set_flag("legacy_discovery", curr_value).unwrap();
                // info!("Set legacy_discovery flag to {}", curr_value);
                session.send("legacy_discovery");
            },
            "log_to_file" => {
                let curr_value = !storage.get_flag("log_to_file");
                storage.set_flag("log_to_file", curr_value).unwrap();
                // info!("Set log_to_file flag to {}", curr_value);
                session.send("log_to_file");
            },
            "auto_update" => {
                let curr_value = !storage.get_flag("auto_update");
                storage.set_flag("auto_update", curr_value).unwrap();
                // info!("Set auto_update flag to {}", curr_value);
                session.send("auto_update");
            },
            "skip_cutscene" => {
                let curr_value = !storage.get_flag("skip_cutscene");
                storage.set_flag("skip_cutscene", curr_value).unwrap();
                // info!("Set skip_cutscene flag to {}", curr_value);
                session.send("skip_cutscene");
            },
            "skip_title_scene" => {
                let curr_value = !storage.get_flag("skip_title_scene");
                storage.set_flag("skip_title_scene", curr_value).unwrap();
                // info!("Set title_scene flag to {}", curr_value);
                session.send("skip_title_scene");
            },
            "use_folder_name" => {
                let curr_value = !storage.get_flag("use_folder_name");
                storage.set_flag("use_folder_name", curr_value).unwrap();
                // info!("Set use_folder_name flag to {}", curr_value);
                session.send("use_folder_name");
            },
            _ => break,
        }
    }

    session.exit();
    session.wait_for_exit();

    storage.flush();

    if reboot_required {
        skyline_web::dialog_ok::DialogOk::ok(
            "Some important fields in the configuration have been changed. <br>Smash will now reboot to reload ARCropolis with the new changes.",
        );
        unsafe { skyline::nn::oe::RequestToRelaunchApplication() };
    }
}
