use std::collections::BTreeMap;

use log::error;
use serde::{Deserialize, Serialize};
use skyline_config::{ConfigStorage, StorageHolder};
use skyline_web::{Visibility, Webpage};

use crate::page;

const FLAGS: &[&str] = &[
    "beta_updates",
    "legacy_discovery",
    "debug",
    "log_to_file",
    "auto_update",
    "skip_cutscene",
    "skip_title_scene",
    "use_folder_name",
];

const READ_AT_BOOT: &[&str] = &["legacy_discovery", "logging_level"];

#[derive(Debug, Serialize)]
struct ConfigData<'a> {
    flags: BTreeMap<&'a str, bool>,
    logging_level: String,
}

#[derive(Debug, Deserialize)]
pub enum ConfigMessage {
    ToggleFlag { name: String },
    SetLoggingLevel { level: String },
    Closure,
}

pub fn show_config_editor<CS: ConfigStorage>(storage: &mut StorageHolder<CS>) {
    page::write_static_assets(
        "config",
        &[
            ("config.html", crate::files::CONFIG_HTML_TEXT.as_bytes()),
            ("configurator.css", crate::files::CONFIG_CSS_TEXT.as_bytes()),
            ("configurator.js", crate::files::CONFIG_JAVASCRIPT_TEXT.as_bytes()),
            ("check.svg", crate::files::CHECK_SVG),
            ("common.js", crate::files::COMMON_JAVASCRIPT_TEXT.as_bytes()),
        ],
    );

    let data = ConfigData {
        flags: FLAGS.iter().map(|flag| (*flag, storage.get_flag(flag))).collect(),
        logging_level: storage.get_field("logging_level").unwrap_or_else(|_| String::from("Warn")),
    };
    page::write_file("config_data.js", page::data_script("CONFIG_DATA", &data));

    let session = Webpage::new()
        .htdocs_dir("contents")
        .start_page("config.html")
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open_session(Visibility::Default)
        .unwrap();

    let mut reboot_required = false;

    loop {
        match page::next_message::<ConfigMessage>(&session) {
            ConfigMessage::ToggleFlag { name } => {
                if !FLAGS.contains(&name.as_str()) {
                    error!("The config page asked to toggle '{}' which is not a setting", name);
                    continue;
                }
                let value = !storage.get_flag(&name);
                if let Err(err) = storage.set_flag(&name, value) {
                    error!("Could not save the '{}' setting: {}", name, err);
                }
                reboot_required |= READ_AT_BOOT.contains(&name.as_str());
            },
            ConfigMessage::SetLoggingLevel { level } => {
                let current: String = storage.get_field("logging_level").unwrap_or_default();
                if current == level {
                    continue;
                }
                if let Err(err) = storage.set_field("logging_level", &level) {
                    error!("Could not save the logging level: {}", err);
                }
                reboot_required |= READ_AT_BOOT.contains(&"logging_level");
            },
            ConfigMessage::Closure => break,
        }
    }

    session.exit();
    session.wait_for_exit();
    storage.flush();

    if reboot_required
        && skyline_web::dialog::Dialog::yes_no(
            "Some of the settings you changed only apply after a restart.<br>Would you like to reboot the game now?",
        )
    {
        unsafe { skyline::nn::oe::RequestToRelaunchApplication() };
    }
}
