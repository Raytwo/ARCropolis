use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use skyline_web::{dialog_ok::DialogOk, Webpage};
use smash_arc::Hash40;

use crate::{page, utils};

#[derive(Debug, Serialize)]
struct Information<'a> {
    entries: &'a [Entry],
    workspace: &'a str,
}

#[derive(Debug, Serialize)]
pub struct Entry {
    id: u32,
    display_name: String,
    authors: String,
    version: String,
    description: String,
    category: String,
    is_disabled: bool,
    image: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ModInfo {
    display_name: Option<String>,
    authors: Option<String>,
    version: Option<String>,
    description: Option<String>,
    category: Option<String>,
}

pub struct ModRoot {
    hash: Hash40,
    preview: Option<Preview>,
}

struct Preview {
    source: PathBuf,
    cached_name: String,
}

#[derive(Debug, Deserialize)]
pub enum ArcadiaMessage {
    ToggleMod { id: usize, state: bool },
    ChangeAll { state: bool },
    ChangeCategories { state: bool, categories: Vec<String> },
    DebugPrint { message: String },
    Closure,
}

pub fn get_mods(presets: &HashSet<Hash40>) -> (Vec<Entry>, Vec<ModRoot>) {
    let use_folder_name = ::config::use_folder_name();
    let dir = match fs::read_dir(utils::paths::mods()) {
        Ok(dir) => dir,
        Err(err) => {
            error!("Could not list the mods folder: {}", err);
            return (Vec::new(), Vec::new());
        },
    };

    let mut entries = Vec::new();
    let mut roots = Vec::new();

    for entry in dir.flatten() {
        if !entry.file_type().map(|ty| ty.is_dir()).unwrap_or(false) {
            continue;
        }

        let path = entry.path();
        let (Some(path_str), Ok(folder_name)) = (path.to_str(), entry.file_name().into_string()) else {
            warn!("Skipping a mod folder whose name is not valid UTF-8");
            continue;
        };

        let hash = Hash40::from(path_str);
        let info = read_info(&path, &folder_name);
        let preview = find_preview(&path, hash);

        let display_name = if use_folder_name { folder_name.clone() } else { info.display_name.unwrap_or_else(|| folder_name.clone()) };

        entries.push(Entry {
            id: entries.len() as u32,
            display_name,
            authors: info.authors.unwrap_or_else(|| "???".to_string()),
            version: info.version.unwrap_or_else(|| "???".to_string()),
            description: info.description.unwrap_or_default().replace('\n', "<br />"),
            category: match info.category {
                Some(category) if category == "Music" => "Audio".to_string(),
                Some(category) => category,
                None => "Misc".to_string(),
            },
            is_disabled: !presets.contains(&hash),
            image: preview.as_ref().map(|preview| format!("img/{}", preview.cached_name)),
        });
        roots.push(ModRoot { hash, preview });
    }

    (entries, roots)
}

fn read_info(mod_path: &Path, folder_name: &str) -> ModInfo {
    let text = match fs::read_to_string(mod_path.join("info.toml")) {
        Ok(text) => text,
        Err(_) => return ModInfo::default(),
    };

    match toml::from_str(&text) {
        Ok(info) => info,
        Err(err) => {
            DialogOk::ok(format!("The following info.toml is not valid: \n\n* '{}'\n\nError: {}", folder_name, err));
            ModInfo::default()
        },
    }
}

fn find_preview(mod_path: &Path, hash: Hash40) -> Option<Preview> {
    let source = mod_path.join("preview.webp");
    let meta = fs::metadata(&source).ok()?;

    Some(Preview {
        source,
        cached_name: format!("{:x}-{}.webp", hash.0, meta.len()),
    })
}

fn sync_previews(roots: &[ModRoot]) {
    let img_dir = page::path("img");
    if let Err(err) = fs::create_dir_all(&img_dir) {
        error!("Could not create the preview folder: {}", err);
        return;
    }

    let mut stale: HashSet<String> = fs::read_dir(&img_dir)
        .map(|dir| dir.flatten().filter_map(|entry| entry.file_name().into_string().ok()).collect())
        .unwrap_or_default();

    for preview in roots.iter().filter_map(|root| root.preview.as_ref()) {
        if stale.remove(&preview.cached_name) {
            continue;
        }

        let copied = fs::read(&preview.source).and_then(|bytes| fs::write(img_dir.join(&preview.cached_name), bytes));
        if let Err(err) = copied {
            error!("Could not copy '{}' for ARCadia: {}", preview.source.display(), err);
        }
    }

    for name in stale {
        let _ = fs::remove_file(img_dir.join(name));
    }
}

pub fn show_arcadia(workspace: Option<String>) {
    if !utils::paths::mods().exists() {
        DialogOk::ok("It seems the directory specified in your configuration does not exist.");
        return;
    }
    let workspace_name: String =
        workspace.unwrap_or_else(|| ::config::workspaces::get_active_workspace_name().unwrap_or_else(|_| String::from("Default")));

    let presets = ::config::presets::get_preset(&workspace_name).unwrap();
    let mut new_presets = presets.clone();

    let (entries, roots) = get_mods(&presets);

    page::write_static_assets(
        "arcadia",
        &[
            ("arcadia.html", crate::files::ARCADIA_HTML_TEXT.as_bytes()),
            ("arcadia.js", crate::files::ARCADIA_JS_TEXT.as_bytes()),
            ("arcadia.css", crate::files::ARCADIA_CSS_TEXT.as_bytes()),
            ("common.css", crate::files::COMMON_CSS_TEXT.as_bytes()),
            ("check.svg", crate::files::CHECK_SVG),
            ("missing.webp", crate::files::MISSING_WEBP),
        ],
    );
    let info = Information { entries: &entries, workspace: &workspace_name };
    page::write_file("mods.js", page::data_script("ARCADIA_DATA", &info));
    sync_previews(&roots);

    println!("Opening ARCadia...");

    let session = Webpage::new()
        .htdocs_dir("contents")
        .start_page("arcadia.html")
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open_session(skyline_web::Visibility::Default)
        .unwrap();

    let mut set_enabled = |root: &ModRoot, state: bool| {
        if state {
            new_presets.insert(root.hash);
        } else {
            new_presets.remove(&root.hash);
        }
    };

    loop {
        match page::next_message::<ArcadiaMessage>(&session) {
            ArcadiaMessage::ToggleMod { id, state } => match roots.get(id) {
                Some(root) => {
                    debug!("Setting mod {} to {}", id, state);
                    set_enabled(root, state);
                },
                None => error!("ARCadia asked to toggle mod {} which does not exist", id),
            },
            ArcadiaMessage::ChangeAll { state } => {
                debug!("Changing all to {}", state);
                for root in &roots {
                    set_enabled(root, state);
                }
            },
            ArcadiaMessage::ChangeCategories { state, categories } => {
                debug!("Changing {:?} to {}", categories, state);
                for (entry, root) in entries.iter().zip(&roots) {
                    if categories.is_empty() || categories.contains(&entry.category) {
                        set_enabled(root, state);
                    }
                }
            },
            ArcadiaMessage::DebugPrint { message } => {
                println!("session says: {}", message);
            },
            ArcadiaMessage::Closure => {
                session.exit();
                session.wait_for_exit();
                break;
            },
        }
    }

    if new_presets == presets {
        return;
    }

    if let Err(err) = ::config::presets::replace_preset(&workspace_name, &new_presets) {
        error!("Could not save the preset for workspace '{}': {}", workspace_name, err);
        return;
    }

    let active_workspace = ::config::workspaces::get_active_workspace_name().unwrap();
    if active_workspace == workspace_name
        && skyline_web::dialog::Dialog::yes_no(
            "Your preset has successfully been updated!<br>Your changes will take effect on the next boot.<br>Would you like to reboot the game to reload your mods?",
        )
    {
        unsafe { skyline::nn::oe::RequestToRelaunchApplication() };
    }
}
