// #![feature(proc_macro_hygiene)]

use crate::config;
use log::info;
use percent_encoding::percent_decode_str;
use serde::{Serialize, Deserialize};
use skyline::nn;
use skyline_web::{ramhorns, Webpage};
use smash_arc::Hash40;
use std::collections::HashSet;
use std::ffi::CString;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, ramhorns::Content)]
pub struct Entries {
    entries: Vec<Entry>,
}

#[derive(Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize, ramhorns::Content)]
pub struct Entry {
    id: Option<u32>,
    folder_name: Option<String>,
    is_disabled: Option<bool>,
    display_name: Option<String>,
    authors: Option<Vec<String>>,
    version: Option<String>,
    description: Option<String>,
    category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    image: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigChanged {
    category: String,
    value: String,
}

#[derive(Debug, Deserialize)]
pub enum ArcadiaMessage {
    DescriptionRequest { id: usize },
    ToggleModRequest { id: usize, state: bool },
    ClosureRequest,
}

static HTML_TEXT: &str = include_str!("../../../resources/templates/arcadia.html");
static CSS_TEXT: &str = include_str!("../../../resources/css/arcadia.css");
static ARCADIA_JAVASCRIPT_TEXT: &str = include_str!("../../../resources/js/arcadia.js");
static JQUERY_LIB_JAVASCRIPT_TEXT: &str = include_str!("../../../resources/js/jquery.textfill.min.js");
static MISSING_ICON: &[u8] = include_bytes!("../../../resources/img/missing.webp");
static CHECK_ICON: &[u8] = include_bytes!("../../../resources/img/check.svg");

pub fn get_mods(workspace: &str) -> Vec<Entry> {
    let mut storage = config::GLOBAL_CONFIG.lock().unwrap();
    let mut presets: HashSet<Hash40> = storage.get_field_json("presets").unwrap_or_default();

    std::fs::read_dir(workspace)
        .unwrap()
        .enumerate()
        .map(|(i, path)| {
            let path_to_be_used = path.unwrap().path();

            //let path_to_be_used = format!("{}", path.path().display());

            let disabled = if !presets.contains(&Hash40::from(path_to_be_used.to_str().unwrap())) {
                true
            } else {
                false
            };

            let mut folder_name = Path::new(&path_to_be_used)
                .file_name()
                .unwrap()
                .to_os_string()
                .into_string()
                .unwrap();

            let info_path = format!("{}/info.toml", path_to_be_used.display());

            let default_entry = Entry {
                    id: Some(i as u32),
                    folder_name: Some(folder_name.clone()),
                    is_disabled: Some(disabled),
                    version: Some("???".to_string()),
                    // description: Some("".to_string()),
                    category: Some("Misc".to_string()),
                    .. Default::default()
            };

            let mod_info = match toml::from_str::<Entry>(&std::fs::read_to_string(&info_path).unwrap_or_default()) {
                    Ok(res) => {
                        Entry {
                            id: Some(i as u32),
                            folder_name: Some(folder_name.clone()),
                            display_name: res.display_name.or(Some(folder_name.clone())),
                            is_disabled: Some(disabled),
                            version: res.version.or(Some(String::from("???"))),
                            category: res.category.or(Some(String::from("Misc"))),
                            // TODO: Yes this is trash, not in the mood to learn serde attributes rn
                            image: if PathBuf::from("sd:/ultimate/mods").join(&path_to_be_used).join("preview.webp").exists() {
                                Some(format!("img/{}", i))
                            } else {
                                Some("missing.webp".to_string())
                            },
                            description: Some(res.description
                                .unwrap_or_else(String::new)
                                .replace("\n", "<br>")),
                            ..res
                        }
                    }
                    Err(e) => {
                        skyline_web::DialogOk::ok(&format!(
                            "The following info.toml is not valid: \n\n* '{}'\n\nError: {}",
                            folder_name,
                            e,
                        ));
                        default_entry
                    }
                };

            mod_info
        })
        .collect()
}

pub fn show_arcadia() {
    let workspace = PathBuf::from(config::umm_path());

    if !workspace.exists() {
        skyline_web::DialogOk::ok("It seems the directory specified in your configuration does not exist.");
        return;
    }

    let mut mods: Entries = Entries {
        entries: get_mods(&workspace.to_str().unwrap()),
    };

    // Causes some trouble when sending the Id from the javascript side

    // Sort mods alphabatically
    // mods.entries.sort_by(|a, b| {
    //     a.display_name
    //         .as_ref()
    //         .unwrap_or(a.folder_name.as_ref().unwrap())
    //         .to_ascii_lowercase()
    //         .cmp(&b.display_name.as_ref().unwrap().to_ascii_lowercase())
    // });

    //region Setup Preview Images
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();
    for item in &mods.entries {
        let path = PathBuf::from("sd:/ultimate/mods").join(item.folder_name.as_ref().unwrap()).join("preview.webp");

        if path.exists() {
            images.push((
                format!("img/{}", item.id.unwrap().to_string()),
                std::fs::read(path).unwrap(),
            ));
        };
    }

    let img_cache =
        "sd:/atmosphere/contents/01006A800016E000/manual_html/html-document/contents.htdocs/img";

    if std::fs::metadata(&img_cache).is_ok() {
        let _ = std::fs::remove_dir_all(&img_cache).map_err(|err| println!("Error occured in ARCadia-rs when trying to delete cache: {}", err));
    };

    std::fs::create_dir_all(&img_cache).unwrap();

    let tpl = ramhorns::Template::new(HTML_TEXT).unwrap();

    let render = tpl.render(&mods);

    let session = Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", &render)
        .file("arcadia.css", CSS_TEXT)
        .file("arcadia.js", ARCADIA_JAVASCRIPT_TEXT)
        .file("jquery.textfill.min.js", JQUERY_LIB_JAVASCRIPT_TEXT)
        .file("missing.webp", MISSING_ICON)
        .file("check.svg", CHECK_ICON)
        .files(&images)
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open_session(skyline_web::Visibility::Default)
        .unwrap();

    let mut storage = config::GLOBAL_CONFIG.lock().unwrap();
    let mut presets: HashSet<Hash40> = storage.get_field_json("presets").unwrap();
    let mut modified_detected = false;
    
    while let Ok(message) = session.recv_json::<ArcadiaMessage>() {
        match message {
            ArcadiaMessage::DescriptionRequest { id } => {
                session.send_json(&mods.entries[id]);
            },
            ArcadiaMessage::ToggleModRequest { id, state} => {
                let path = format!("sd:/ultimate/mods/{}", mods.entries[id].folder_name.as_ref().unwrap());
                let hash = Hash40::from(path.as_str());

                if state {
                    presets.insert(hash);
                } else {
                    presets.remove(&hash);
                }
                
            },
            ArcadiaMessage::ClosureRequest => {
                session.exit();
                session.wait_for_exit();
                break;
            }
        }
    }

    storage.set_field_json("presets", &presets).unwrap();
    storage.flush();

    if modified_detected {
        if skyline_web::Dialog::yes_no("Your preset has been changed!<br>Would you like to reboot the game to reload your mods?") {
            unsafe { skyline::nn::oe::RequestToRelaunchApplication() };
        }
    }
}
