// #![feature(proc_macro_hygiene)]

use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    path::{Path, PathBuf},
};

use log::info;
use serde::{Deserialize, Serialize};
use skyline::nn;
use skyline_web::{ramhorns, Webpage};
use smash_arc::Hash40;

use crate::config;

#[derive(Debug, Serialize)]
pub struct Information {
    entries: Vec<Entry>,
    workspace: String,
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
    ChangeIndexesRequest { state: bool, indexes: Vec<usize> },
    ClosureRequest,
}

pub fn get_mods(presets: &HashSet<Hash40>) -> Vec<Entry> {
    let mut id: u32 = 0;
    std::fs::read_dir(&config::umm_path())
        .unwrap()
        .enumerate()
        .filter_map(|(i, path)| {
            let path_to_be_used = path.unwrap().path();

            if path_to_be_used.is_file() {
                return None;
            }

            let disabled = if !presets.contains(&Hash40::from(path_to_be_used.to_str().unwrap())) { true } else { false };

            let mut folder_name = Path::new(&path_to_be_used).file_name().unwrap().to_os_string().into_string().unwrap();

            let info_path = format!("{}/info.toml", path_to_be_used.display());

            let default_entry = Entry {
                id: Some(id),
                folder_name: Some(folder_name.clone()),
                is_disabled: Some(disabled),
                version: Some("???".to_string()),
                // description: Some("".to_string()),
                category: Some("Misc".to_string()),
                ..Default::default()
            };

            let mod_info = match toml::from_str::<Entry>(&std::fs::read_to_string(&info_path).unwrap_or_default()) {
                Ok(res) => Entry {
                    id: Some(id),
                    folder_name: Some(folder_name.clone()),
                    display_name: res.display_name.or(Some(folder_name.clone())),
                    authors: res.authors.or(Some(String::from("???"))),
                    is_disabled: Some(disabled),
                    version: res.version.or(Some(String::from("???"))),
                    category: res.category.map_or(Some(String::from("Misc")),
                     |cat|
                            if cat == "Music" {
                                Some("Audio".to_string())
                            }
                            else {
                                Some(cat)
                            },
                    ),
                    description: Some(res.description.unwrap_or_else(String::new).replace("\n", "<br />")),
                    ..res
                },
                Err(e) => {
                    skyline_web::DialogOk::ok(&format!("The following info.toml is not valid: \n\n* '{}'\n\nError: {}", folder_name, e,));
                    default_entry
                },
            };

            id += 1;

            Some(mod_info)
        })
        .collect()
}

pub fn show_arcadia(workspace: Option<String>) {
    let umm_path = config::umm_path();

    if !umm_path.exists() {
        skyline_web::DialogOk::ok("It seems the directory specified in your configuration does not exist.");
        return;
    }

    let mut storage = config::GLOBAL_CONFIG.lock().unwrap();
    let workspace_name: String = workspace.unwrap_or(storage.get_field("workspace").unwrap_or("Default".to_string()));
    let workspace_list: HashMap<String, String> = storage.get_field_json("workspace_list").unwrap_or_default();
    let preset_name = &workspace_list[&workspace_name];

    let presets: HashSet<Hash40> = storage.get_field_json(preset_name).unwrap_or_default();
    let mut new_presets = presets.clone();

    let mut mods: Information = Information {
        entries: get_mods(&presets),
        workspace: workspace_name.clone(),
    };

    // region Setup Preview Images
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();
    for item in &mods.entries {
        let path = &umm_path.join(item.folder_name.as_ref().unwrap()).join("preview.webp");

        if path.exists() {
            images.push((format!("img/{}", item.id.unwrap().to_string()), std::fs::read(path).unwrap()));
        };
    }

    let img_cache = "sd:/atmosphere/contents/01006A800016E000/manual_html/html-document/contents.htdocs/img";

    if std::fs::metadata(&img_cache).is_ok() {
        let _ = std::fs::remove_dir_all(&img_cache).map_err(|err| error!("Error occured in ARCadia-rs when trying to delete cache: {}", err));
    };

    std::fs::create_dir_all(&img_cache).unwrap();

    println!("Opening ARCadia...");

    let session = Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", &crate::menus::files::ARCADIA_HTML_TEXT)
        .file("arcadia.js", &crate::menus::files::ARCADIA_JS_TEXT)
        .file("common.js", &crate::menus::files::COMMON_JAVASCRIPT_TEXT)
        .file("arcadia.css", &crate::menus::files::ARCADIA_CSS_TEXT)
        .file("common.css", &crate::menus::files::COMMON_CSS_TEXT)
        .file("pagination.min.js", &crate::menus::files::PAGINATION_JS)
        .file("jquery.marquee.min.js", &crate::menus::files::MARQUEE_JS)
        .file("check.svg", &crate::menus::files::CHECK_SVG)
        .file("missing.webp", &crate::menus::files::MISSING_WEBP)
        .file("mods.json", &serde_json::to_string(&mods).unwrap())
        .files(&images)
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open_session(skyline_web::Visibility::Default)
        .unwrap();

    while let Ok(message) = session.recv_json::<ArcadiaMessage>() {
        match message {
            ArcadiaMessage::ToggleModRequest { id, state } => {
                let path = format!("{}/{}", umm_path.display(), mods.entries[id].folder_name.as_ref().unwrap());
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
                        let path = format!("{}/{}", umm_path.display(), item.folder_name.as_ref().unwrap());
                        let hash = Hash40::from(path.as_str());

                        new_presets.insert(hash);
                    }
                }
            },
            ArcadiaMessage::ChangeIndexesRequest { state, indexes } => {
                for idx in indexes {
                    let path = format!("{}/{}", umm_path.display(), mods.entries[idx].folder_name.as_ref().unwrap());
                    let hash = Hash40::from(path.as_str());
                    debug!("Setting {} to {}", path, state);

                    if state {
                        new_presets.insert(hash);
                    } else {
                        new_presets.remove(&hash);
                    }
                }
            },
            ArcadiaMessage::ClosureRequest => {
                session.exit();
                session.wait_for_exit();
                break;
            },
        }
    }

    let active_workspace: String = storage.get_field("workspace").unwrap_or("Default".to_string());

    storage.set_field_json(&preset_name, &new_presets).unwrap();
    storage.flush();

    drop(storage);

    if new_presets != presets {
        // Acquire the filesystem so we can check if it's already finished or not (for boot-time mod manager)
        if let Some(filesystem) = crate::GLOBAL_FILESYSTEM.try_read() {
            if active_workspace.eq(&workspace_name) {
                if skyline_web::Dialog::yes_no("Your preset has successfully been updated!<br>Your changes will take effect on the next boot.<br>Would you like to reboot the game to reload your mods?") {
                    unsafe { skyline::nn::oe::RequestToRelaunchApplication() };
                }
            }
        }
    }
}
