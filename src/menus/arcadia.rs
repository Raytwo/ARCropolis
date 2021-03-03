#![feature(proc_macro_hygiene)]

use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use skyline_web::Webpage;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use skyline::nn;
use tinytemplate::TinyTemplate;

use std::ffi::CString;

use crate::config::CONFIG;

const LOCALHOST: &str = "http://localhost/";

static HTML_TEXT: &str = include_str!("resources/index.html");
static CSS_TEXT: &str = include_str!("resources/style.css");
static JAVASCRIPT_TEXT: &str = include_str!("resources/index.js");
static TEXTFILL_LIB: &str = include_str!("resources/jquery.textfill.min.js");
static MISSING_ICON: &[u8] = include_bytes!("resources/missing.webp");

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Holder {
    mods: Vec<ListingType>,
    images: HashMap<u32, PathBuf>,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ListingType {
    id: u32,
    name: String,
    is_enabled: bool,
    display_name: String,
    version: String,
    description: String,
    category: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Context {
    workspace: String,
    holder: Holder,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonResult {
    id: u32,
    is_enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Info {
    display_name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    category: Option<String>,
}

pub struct WebpageResult {
    json: String,
    mods: Vec<ListingType>,
}

pub fn RenameFolder(old: &Path, new: &Path) -> u32 {
    let old_cstr = CString::new(old.to_str().unwrap().as_bytes() as &[u8]).unwrap();

    let new_cstr = CString::new(new.to_str().unwrap().as_bytes() as &[u8]).unwrap();

    unsafe { nn::fs::RenameDirectory(old_cstr.as_ptr() as _, new_cstr.as_ptr() as _) as u32 }
}

pub fn get_mods(location: &str) -> Holder {
    let workspace = CONFIG.read().paths.umm.to_str().unwrap().to_string();

    let mut modsHolder = Holder {
        mods: Vec::new(),
        images: HashMap::new(),
    };

    //#region Check for duplicate folders
    let paths = std::fs::read_dir(location).unwrap();

    for path in paths {
        let original = format!("{}", path.unwrap().path().display());
        let disabled = format!(".{}", &original);
        let counter_part = if original.chars().next().unwrap() == '.' {
            &original[1..]
        } else {
            &disabled
        };

        let original_path = format!("{}/{}", workspace, &original);
        let counter_part_path = format!("{}/{}", workspace, &counter_part);

        if std::fs::metadata(&original_path).is_ok() & std::fs::metadata(&counter_part_path).is_ok()
        {
            // Nuke the disabled/enabled folder
            // Rename the disabled/enabled folder (to what?)
            RenameFolder(
                Path::new(&counter_part_path),
                Path::new(&format!("{} (2)", &counter_part_path)),
            );
        }
    }
    //#endregion

    let paths = std::fs::read_dir(location).unwrap();

    let mut i = 0;

    for path in paths {
        let original_name = format!("{}", &path.unwrap().path().display());

        let enabled = if original_name.chars().next().unwrap() == '.' {
            false
        } else {
            true
        };

        let name;

        if !enabled {
            name = &original_name[1..];
        } else {
            name = &original_name;
        }
        let info_path = Path::new(&workspace).join(&original_name).join("info.toml");

        let misc: Info = if std::fs::metadata(&info_path).is_ok() {
            toml::from_str(&std::fs::read_to_string(&info_path).unwrap()).unwrap()
        } else {
            Info {
                display_name: Some(name.to_string()),
                version: Some("???".to_string()),
                description: Some("".to_string()),
                category: Some("Misc".to_string()),
            }
        };

        let info = ListingType {
            id: i,
            name: name.to_string(),
            is_enabled: enabled,
            display_name: misc.display_name.unwrap_or(name.to_string()),
            version: misc.version.unwrap_or("???".to_string()),
            description: misc.description.unwrap_or("".to_string()).replace("\n", "<br>"),
            category: misc.category.unwrap_or("Misc".to_string()),        
        };

        let img_location = format!("{}/{}/preview.webp", workspace, original_name);
        let img_path = Path::new(&img_location);

        modsHolder.images.insert(i, img_path.to_path_buf());

        modsHolder.mods.push(info);
        i += 1;
    }

    // Sort it alphabetically
    modsHolder.mods.sort_by(|a, b| {
        a.display_name
            .to_ascii_lowercase()
            .cmp(&b.display_name.to_ascii_lowercase())
    });

    modsHolder
}

fn to_html(context: &Context) -> String {
    let mut tpl = TinyTemplate::new();
    tpl.add_template("page_listing", HTML_TEXT).unwrap();
    tpl.render("page_listing", &context).unwrap()
}

fn show_menu(context: Context) -> WebpageResult {
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();

    for item in &context.holder.mods {
        if context.holder.images.get(&item.id).unwrap().exists() {
            images.push((
                item.id.to_string(),
                std::fs::read(context.holder.images.get(&item.id).unwrap()).unwrap(),
            ));
        };
    }

    let response = Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", &to_html(&context))
        .file("style.css", CSS_TEXT)
        .file("index.js", JAVASCRIPT_TEXT)
        .file("jquery.textfill.min.js", TEXTFILL_LIB)
        .file("missing.webp", MISSING_ICON)
        .files(&images)
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open()
        .unwrap();

    match response.get_last_url().unwrap() {
        "" => WebpageResult {
            json: "".to_string(),
            mods: context.holder.mods,
        },
        url => WebpageResult {
            json: format!(
                "{}",
                percent_decode_str(&url[LOCALHOST.len()..])
                    .decode_utf8_lossy()
                    .into_owned()
                    .to_string()
            ),
            mods: context.holder.mods,
        },
    }
}

pub fn show_explorer(mut path: &String) -> WebpageResult {
    let workspace = CONFIG.read().paths.umm.to_str().unwrap().to_string();

    loop {
        let results: Holder = get_mods(&path);

        // Regex solution (look ahead doesn't work, so ¯\_(ツ)_/¯)
        // let re = Regex::new(r"\/(?:.(?!\/))+").unwrap();
        // let current_workspace = re.find(if path.chars().last().unwrap() == '/' {&path[..path.len() - 1]} else {&path} ).unwrap();

        // Hardcoded Solution (removes sd:/ultimate/)
        let current_workspace = if path.chars().last().unwrap() == '/' {
            &path[13..path.len() - 1]
        } else {
            &path[13..]
        };

        let context = Context {
            workspace: Path::new(&workspace).file_name().unwrap().to_os_string().into_string().unwrap(),
            holder: results,
        };

        let response_path = show_menu(context);

        return response_path;
    }
}

pub fn show_arcadia() {
    let workspace = CONFIG.read().paths.umm.to_str().unwrap().to_string();

    let results = show_explorer(&workspace);

    let statuses: Vec<JsonResult> = serde_json::from_str(&results.json).unwrap_or(vec![]);

    let mods = results.mods;

    for item in statuses {
        let enabled_path = Path::new(&workspace).join(&mods[item.id as usize].name);
        let disabled_path =
            Path::new(&workspace).join(&format!(".{}", &mods[item.id as usize].name));

        println!(
            "ID: {}\nMod Name: {}\nStatus: {}",
            item.id, mods[item.id as usize].name, item.is_enabled
        );
        // println!("Enabled Path: {:?}\nDoes Enabled Path Exist? {}\nDisabled Path: {:?}\nDoes Disabled Path Exist? {}", enabled_path, std::fs::metadata(&enabled_path).is_ok(), disabled_path, std::fs::metadata(&disabled_path).is_ok());

        if item.is_enabled {
            if std::fs::metadata(&disabled_path).is_ok() {
                println!("Enabling {}", mods[item.id as usize].name);
                println!(
                    "RenameFolder Result: {:?}",
                    RenameFolder(&disabled_path, &enabled_path)
                );
            }
        } else {
            if std::fs::metadata(&enabled_path).is_ok() {
                println!("Disabling {}", mods[item.id as usize].name);
                println!("{:?}", RenameFolder(&enabled_path, &disabled_path));
            }
        }

        println!("---------------------------");
    }
}
