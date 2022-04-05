// #![feature(proc_macro_hygiene)]

use std::{
    collections::{HashSet, HashMap},
    ffi::CString,
    path::{Path, PathBuf},
};

use log::info;
use serde::{Deserialize, Serialize};
use skyline::nn;
use skyline_web::{ramhorns, Webpage};
use smash_arc::Hash40;

use crate::config;

#[derive(Debug)]
pub struct Entries {
    entries: Vec<Entry>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Entry {
    id: Option<u32>,
    folder_name: Option<String>,
    is_disabled: Option<bool>,
    display_name: Option<String>,
    authors: Option<String>,
    version: Option<String>,
    description: Option<String>,
    category: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigChanged {
    category: String,
    value: String,
}

#[derive(Debug, Deserialize)]
pub enum ArcadiaMessage {
    ToggleModRequest { id: usize, state: bool },
    ChangeAllRequest { state: bool },
    ClosureRequest,
}

static HTML_TEXT: &str = include_str!("../../../resources/templates/arcadia.html");
static JS_TEXT: &str = include_str!("../../../resources/js/arcadia.js");
static CSS_TEXT: &str = include_str!("../../../resources/css/arcadia.css");
static COMMON_JS_TEXT: &str = include_str!("../../../resources/js/common.js");
static COMMON_CSS_TEXT: &str = include_str!("../../../resources/css/common.css");
static MARQUEE_JS: &str = include_str!("../../../resources/js/jquery.marquee.min.js");
static PAGINATION_JS: &str = include_str!("../../../resources/js/pagination.min.js");

pub fn get_mods(workspace: &str) -> Vec<Entry> {
    let mut storage = config::GLOBAL_CONFIG.lock().unwrap();
    let workspace_name: String = storage.get_field("workspace").unwrap_or("Default".to_string());
    let workspace_list: HashMap<String, String> = storage.get_field_json("workspace_list").unwrap_or_default();
    let preset_name = &workspace_list[&workspace_name];

    let mut presets: HashSet<Hash40> = storage.get_field_json(preset_name).unwrap_or_default();

    std::fs::read_dir(workspace)
        .unwrap()
        .enumerate()
        .filter_map(|(i, path)| {
            let path_to_be_used = path.unwrap().path();

            if path_to_be_used.is_file() {
                return None
            }

            let disabled = if !presets.contains(&Hash40::from(path_to_be_used.to_str().unwrap())) { true } else { false };

            let mut folder_name = Path::new(&path_to_be_used).file_name().unwrap().to_os_string().into_string().unwrap();

            let info_path = format!("{}/info.toml", path_to_be_used.display());

            let default_entry = Entry {
                id: Some(i as u32),
                folder_name: Some(folder_name.clone()),
                is_disabled: Some(disabled),
                version: Some("???".to_string()),
                // description: Some("".to_string()),
                category: Some("Misc".to_string()),
                ..Default::default()
            };

            let mod_info = match toml::from_str::<Entry>(&std::fs::read_to_string(&info_path).unwrap_or_default()) {
                Ok(res) => {
                    Entry {
                        id: Some(i as u32),
                        folder_name: Some(folder_name.clone()),
                        display_name: res.display_name.or(Some(folder_name.clone())),
                        authors: res.authors.or(Some(String::from("???"))),
                        is_disabled: Some(disabled),
                        version: res.version.or(Some(String::from("???"))),
                        category: res.category.or(Some(String::from("Misc"))),
                        description: Some(res.description.unwrap_or_else(String::new).replace("\n", "<br />")),
                        ..res
                    }
                },
                Err(e) => {
                    skyline_web::DialogOk::ok(&format!("The following info.toml is not valid: \n\n* '{}'\n\nError: {}", folder_name, e,));
                    default_entry
                },
            };

            Some(mod_info)
        })
        .collect()
}

pub fn show_arcadia() {
    let workspace = config::umm_path();

    if !workspace.exists() {
        skyline_web::DialogOk::ok("It seems the directory specified in your configuration does not exist.");
        return
    }

    let mut mods: Entries = Entries {
        entries: get_mods(&workspace.to_str().unwrap()),
    };

    // region Setup Preview Images
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();
    for item in &mods.entries {
        let path = &workspace.join(item.folder_name.as_ref().unwrap()).join("preview.webp");

        if path.exists() {
            images.push((format!("img/{}", item.id.unwrap().to_string()), std::fs::read(path).unwrap()));
        };
    }

    let img_cache = "sd:/atmosphere/contents/01006A800016E000/manual_html/html-document/contents.htdocs/img";

    if std::fs::metadata(&img_cache).is_ok() {
        let _ = std::fs::remove_dir_all(&img_cache).map_err(|err| error!("Error occured in ARCadia-rs when trying to delete cache: {}", err));
    };

    std::fs::create_dir_all(&img_cache).unwrap();

    let session = Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", &HTML_TEXT)
        .file("arcadia.js", JS_TEXT)
        .file("common.js", &COMMON_JS_TEXT)
        .file("arcadia.css", &CSS_TEXT)
        .file("common.css", &COMMON_CSS_TEXT)
        .file("pagination.min.js", PAGINATION_JS)
        .file("jquery.marquee.min.js", MARQUEE_JS)
        .file("mods.json", &serde_json::to_string(&mods.entries).unwrap())
        .files(&images)
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open_session(skyline_web::Visibility::Default)
        .unwrap();

    let mut storage = config::GLOBAL_CONFIG.lock().unwrap();
    let workspace_name: String = storage.get_field("workspace").unwrap_or("Default".to_string());
    let workspace_list: HashMap<String, String> = storage.get_field_json("workspace_list").unwrap_or_default();
    let preset_name = &workspace_list[&workspace_name];

    let presets: HashSet<Hash40> = storage.get_field_json(preset_name).unwrap_or_default();
    let mut new_presets = presets.clone();

    while let Ok(message) = session.recv_json::<ArcadiaMessage>() {
        match message {
            ArcadiaMessage::ToggleModRequest { id, state } => {
                let path = format!("{}/{}", workspace.display(), mods.entries[id].folder_name.as_ref().unwrap());
                let hash = Hash40::from(path.as_str());
                debug!("Setting {} to {}", path, state);

                if state {
                    new_presets.insert(hash);
                } else {
                    new_presets.remove(&hash);
                }

                debug!("{} has been {}", path, state);
            },
            ArcadiaMessage::ChangeAllRequest { state } => {
                debug!("Changing all to {}", state);

                if !state {
                    new_presets.clear();
                } else {
                    for item in mods.entries.iter() {
                        let path = format!("{}/{}", workspace.display(), item.folder_name.as_ref().unwrap());
                        let hash = Hash40::from(path.as_str());

                        new_presets.insert(hash);
                    }
                }
            },
            ArcadiaMessage::ClosureRequest => {
                session.exit();
                session.wait_for_exit();
                break
            },
        }
    }

    storage.set_field_json(&preset_name, &new_presets).unwrap();
    storage.flush();

    if new_presets != presets {
        // Acquire the filesystem so we can check if it's already finished or not (for boot-time mod manager)
        if let Some(filesystem) = crate::GLOBAL_FILESYSTEM.try_read() {
            if skyline_web::Dialog::yes_no("Your preset has successfully been updated!<br>Your changes will take effect on the next boot.<br>Would you like to reboot the game to reload your mods?") {
                unsafe { skyline::nn::oe::RequestToRelaunchApplication() };
            }
        }
    }
}
