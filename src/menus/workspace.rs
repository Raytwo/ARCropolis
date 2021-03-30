use std::{
    fs::{create_dir_all, read_dir},
    path::{Path, PathBuf},
};

use std::io::prelude::*;

use log::info;
use owo_colors::OwoColorize;
use skyline::nn::web::OfflineExitReason;
use skyline_web::PageResult;

use crate::config::CONFIG;

static HTML_TEXT: &str = include_str!("../../resources/templates/selector.html");
static CSS_TEXT: &str = include_str!("../../resources/css/selector.css");
static JAVASCRIPT_TEXT: &str = include_str!("../../resources/js/selector.js");

// Thanks jugeeya :^)
pub fn get_arguments_from_url(s: &str) -> String {
    let base_url_len = "http://localhost/".len();
    let total_len = s.len();

    s.chars()
        .skip(base_url_len)
        .take(total_len - base_url_len)
        .collect()
}

#[derive(ramhorns::Content)]
pub struct Workspaces {
    pub workspace: Vec<Workspace>,
}

#[derive(ramhorns::Content)]
pub struct Workspace {
    pub index: u8,
    pub name: String,
    pub in_use: bool,
}

fn get_workspaces() -> Vec<Workspace> {
    // TODO: Move this in some sort of initial check method on boot
    create_dir_all("sd:/atmosphere/contents/01006A800016E000/romfs/arcropolis/workspaces").unwrap();

    read_dir("rom:/arcropolis/workspaces")
        .unwrap()
        .enumerate()
        .filter_map(|(index, entry)| {
            let entry = entry.unwrap();

            if entry.file_type().unwrap().is_file() {
                None
            } else {
                Some(Workspace {
                    index: index as u8,
                    name: String::from(entry.path().file_name().unwrap().to_str().unwrap()),
                    in_use: false,
                })
            }
        })
        .collect()
}

fn show_selector(workspaces: &Workspaces) -> PageResult {
    let tpl = ramhorns::Template::new(HTML_TEXT).unwrap();

    let render = tpl.render(&workspaces);

    let mut webpage = skyline_web::Webpage::new();
    webpage.htdocs_dir("contents");
    webpage.file("index.html", &render);
    webpage.file("selector.css", CSS_TEXT);
    webpage.open().unwrap()
}

// Please don't judge too hard I'm in a rush :'D
pub fn workspace_selector() {
    let mut workspaces = Workspaces { workspace: vec![] };

    workspaces.workspace = get_workspaces();

    if workspaces.workspace.is_empty() {
        skyline_web::DialogOk::ok("Your directory does not contain any modpack.");
        return;
    }

    let response = show_selector(&workspaces);

    // If the user picked a modpack
    if response.get_exit_reason() == OfflineExitReason::LastUrl {
        let result = get_arguments_from_url(response.get_last_url().unwrap());

        let mut config = CONFIG.write();
        let mut config_changed = false;

        let mut workspace_name = String::from("Default");

        if result == "reset" {
            config.paths.arc = PathBuf::from("rom:/arc");
            config.paths.umm = PathBuf::from("sd:/ultimate/mods");
            config_changed = true;
        } else {
            // If someone manages to have this many workspaces they honestly deserve the panic
            let modpack_index = result.parse::<u8>().unwrap() as usize;

            let mut selector_workspace = std::path::PathBuf::from("rom:/arcropolis/workspaces");
            selector_workspace.push(workspaces.workspace[modpack_index].name.to_owned());

            info!(
                "[Menu | Workspace Selector] Selected workspace: '{}'",
                selector_workspace.display().red()
            );

            workspace_name = String::from(selector_workspace.to_str().unwrap());

            let path = selector_workspace.to_str().unwrap();

            // Set Arc path in config
            if Path::new(&format!("{}/{}", path, "arc")).exists() {
                config.paths.arc = PathBuf::from(format!("{}/{}", path, "arc"));
                config_changed = true;
            }

            // Set UMM path in config
            if Path::new(&format!("{}/{}", path, "umm")).exists() {
                config.paths.umm = PathBuf::from(format!("{}/{}", path, "umm"));
                config_changed = true;
            }
        }

        if config_changed {
            skyline_web::DialogOk::ok(format!(
                "Workspace {} has been applied.  
                Consider rebooting the game to apply your changes.",
                workspace_name
            ));
        } else {
            skyline_web::DialogOk::ok(
                "The workspace your selected does not contain either a /arc or /umm directory.",
            );
        }
    }
}
