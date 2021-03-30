// #![feature(proc_macro_hygiene)]

use crate::config::CONFIG;
use percent_encoding::percent_decode_str;
use serde::Deserialize;
use skyline::nn;
use skyline_web::Webpage;
use std::ffi::CString;
use std::io::prelude::*;
use std::path::Path;

use log::info;

#[derive(Debug, ramhorns::Content)]
pub struct Entries {
    workspace: String,
    entries: Vec<Entry>,
}

#[derive(Debug, PartialEq, PartialOrd, Deserialize, ramhorns::Content)]
pub struct Entry {
    id: Option<u32>,
    folder_name: Option<String>,
    is_disabled: Option<bool>,
    display_name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    category: Option<String>,
    image: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModStatues {
    is_disabled: Vec<bool>,
}

static HTML_TEXT: &str = include_str!("../../resources/templates/arcadia.html");
static CSS_TEXT: &str = include_str!("../../resources/css/arcadia.css");
static ARCADIA_JAVASCRIPT_TEXT: &str = include_str!("../../resources/js/arcadia.js");
static JQUERY_LIB_JAVASCRIPT_TEXT: &str = include_str!("../../resources/js/jquery.textfill.min.js");
static MISSING_ICON: &[u8] = include_bytes!("../../resources/img/missing.webp");
static CHECK_ICON: &[u8] = include_bytes!("../../resources/img/check.svg");

const LOCALHOST: &str = "http://localhost/";

pub fn rename_folder(src: &Path, dest: &Path) -> u32 {
    let old_cstr = CString::new(src.to_str().unwrap().as_bytes() as &[u8]).unwrap();

    let new_cstr = CString::new(dest.to_str().unwrap().as_bytes() as &[u8]).unwrap();

    unsafe { nn::fs::RenameDirectory(old_cstr.as_ptr() as _, new_cstr.as_ptr() as _) as u32 }
}

pub fn get_mods(workspace: &str) -> Vec<Entry> {
    std::fs::read_dir(workspace)
        .unwrap()
        .enumerate()
        .filter_map(|(i, path)| {
            let path_to_be_used;
            let disabled;
            
            let path = path.unwrap();

            let parent_path = format!("{}", path.path().parent().unwrap().display());
            let original_folder_name = format!("{}", path.file_name().to_os_string().into_string().unwrap());

            
            let original = format!("{}", path.path().display());
            let counter_part = match original_folder_name.chars().next() {
                Some('.') => {
                    disabled = true;
                    format!("{}/{}", parent_path, &original_folder_name[1..])
                }
                _ => {
                    disabled = false;
                    format!("{}/.{}", parent_path, &original_folder_name)
                }
            };

            if std::fs::metadata(&original).is_ok()
            & std::fs::metadata(&counter_part).is_ok()
            {
                path_to_be_used = format!("{} (2)", &counter_part);
                rename_folder(Path::new(&counter_part), Path::new(&path_to_be_used));
            } else {
                path_to_be_used = original.to_owned();
            }
            
            let mut folder_name = Path::new(&path_to_be_used)
                .file_name()
                .unwrap()
                .to_os_string()
                .into_string()
                .unwrap();

            folder_name = if folder_name.starts_with('.') {
                folder_name[1..].to_string()
            } else {
                folder_name
            };

            let info_path = format!("{}/info.toml", path_to_be_used);
            info!("Info Path: {}", info_path);
            let mod_info: Entry = if std::fs::metadata(&info_path).is_ok() {
                let mut res: Entry =
                    toml::from_str(&std::fs::read_to_string(&info_path).unwrap()).unwrap();
                res.id = Some(i as u32);
                res.folder_name = Some(folder_name);
                res.is_disabled = Some(disabled);
                res.image = Some(format!("{}/preview.webp", path_to_be_used));
                res.description = Some(res.description.unwrap_or("".to_string()).replace("\n", "<br>"));
                res
            } else {
                Entry {
                    id: Some(i as u32),
                    folder_name: Some(folder_name.clone()),
                    is_disabled: Some(disabled),
                    display_name: Some(folder_name),
                    version: Some("???".to_string()),
                    description: Some("".to_string()),
                    category: Some("Misc".to_string()),
                    image: Some(format!("{}/preview.webp", path_to_be_used)),
                }
            };

            Some(mod_info)
        })
        .collect()
}

pub fn show_arcadia() {
    let workspace = CONFIG.read().paths.umm.to_str().unwrap().to_string();

    let mut mods: Entries = Entries {
        entries: get_mods(&workspace),
        workspace: Path::new(&workspace)
            .file_name()
            .unwrap()
            .to_os_string()
            .into_string()
            .unwrap(),
    };

    // Sort mods alphabatically
    mods.entries.sort_by(|a, b| {
        a.display_name
            .as_ref()
            .unwrap()
            .to_ascii_lowercase()
            .cmp(&b.display_name.as_ref().unwrap().to_ascii_lowercase())
    });

    //region Setup Preview Images
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();
    for item in &mods.entries {
        if std::fs::metadata(item.image.as_ref().unwrap()).is_ok() {
            images.push((
                format!("img/{}", item.id.unwrap().to_string()),
                std::fs::read(item.image.as_ref().unwrap()).unwrap(),
            ));
        };
    }

    let img_cache =
        "sd:/atmosphere/contents/01006A800016E000/manual_html/html-document/contents.htdocs/img";

    if std::fs::metadata(&img_cache).is_ok() {
        std::fs::remove_dir_all(&img_cache).unwrap();
    };

    std::fs::create_dir_all(&img_cache).unwrap();
    //endregion

    // let mut file = std::fs::File::open("sd:/atmosphere/contents/01006A800016E000/manual_html/html-document/contents.htdocs/arcropolis/resources/templates/arcadia.html").unwrap();
    // let mut page_content: String = String::new();
    // file.read_to_string(&mut page_content).unwrap();

    let tpl = ramhorns::Template::new(HTML_TEXT).unwrap();

    let render = tpl.render(&mods);

    let response = Webpage::new()
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
        .open()
        .unwrap();

    match response.get_last_url().unwrap() {
        "http://localhost/" => (),
        url => {
            let res = percent_decode_str(&url[LOCALHOST.len()..])
                .decode_utf8_lossy()
                .into_owned();

            let webpage_res: ModStatues = toml::from_str(&res).unwrap();

            for (id, disabled) in webpage_res.is_disabled.into_iter().enumerate() {
                let folder_name = &mods.entries[id as usize].folder_name.as_ref().unwrap();

                let enabled_path = Path::new(&workspace).join(&folder_name);
                let disabled_path = Path::new(&workspace).join(&format!(".{}", &folder_name));

                if disabled {
                    if std::fs::metadata(&enabled_path).is_ok() {
                        info!("[menus::show_arcadia] Disabling {}", folder_name);
                        let res = rename_folder(&enabled_path, &disabled_path);
                        info!(
                            "[menus::show_arcadia] RenameFolder Result: {:?}",
                            res
                        );
                    }
                } else if std::fs::metadata(&disabled_path).is_ok() {
                    info!("[menus::show_arcadia] Enabling {}", folder_name);
                    let res = rename_folder(&disabled_path, &enabled_path);
                    info!(
                        "[menus::show_arcadia] RenameFolder Result: {:?}",
                        res
                    );
                }

                info!("[menus::show_arcadia] ---------------------------");
            }
        }
    }
}
