use std::{
    fs::{create_dir_all, read_dir},
    path::{Path, PathBuf},
};

use crate::config::CONFIG;
use log::info;
use owo_colors::OwoColorize;
use percent_encoding::percent_decode_str;
use skyline::nn::web::OfflineExitReason;
use skyline_web::PageResult;

static HTML_TEXT: &str = include_str!("../../resources/templates/selector.html");
static JAVASCRIPT_TEXT: &str = include_str!("../../resources/js/selector.js");
static CSS_TEXT: &str = include_str!("../../resources/css/selector.css");
static CHECK_ICON: &[u8] = include_bytes!("../../resources/img/check.svg");
static WORKSPACES_LOCATION: &str = "sd:/ultimate/";

const LOCALHOST: &str = "http://localhost/";

#[derive(ramhorns::Content)]
pub struct Workspaces {
    pub workspace: Vec<Workspace>,
}

#[derive(ramhorns::Content)]
pub struct Workspace {
    pub index: u8,
    pub name: String,
    pub in_use: Option<String>,
    pub umm_exists: bool,
    pub arc_exists: bool
}

fn get_workspaces() -> Vec<Workspace> {
    // TODO: Move this in some sort of initial check method on boot
    create_dir_all(WORKSPACES_LOCATION).unwrap();

    let config = CONFIG.read();

    let mut actual_index = 0;

    read_dir(WORKSPACES_LOCATION)
        .unwrap()
        .enumerate()
        .filter_map(|(index, entry)| {
            let entry = entry.unwrap();
            let name = String::from(entry.path().file_name().unwrap().to_str().unwrap());

            if entry.file_type().unwrap().is_file() {
                None
            }else if name == "mods" {
                None
            }else {
                let ws = Some(Workspace {
                    index: actual_index,
                    name: name.clone(),
                    in_use: {
                        let arc_used =
                            PathBuf::from(&format!("{}/{}/arc", WORKSPACES_LOCATION, name)) == config.paths.arc;
                        let umm_used =
                            PathBuf::from(&format!("{}/{}/umm", WORKSPACES_LOCATION, name)) == config.paths.umm;

                        if arc_used && umm_used {
                            Some(String::from("both"))
                        } else if arc_used {
                            Some(String::from("arc"))
                        } else if umm_used {
                            Some(String::from("umm"))
                        } else {
                            None
                        }
                    },
                    umm_exists: Path::new(&format!("{}/{}/umm", WORKSPACES_LOCATION, name)).exists(),
                    arc_exists: Path::new(&format!("{}/{}/arc", WORKSPACES_LOCATION, name)).exists(),
                });

                actual_index += 1;
                ws
            }
        })
        .collect()
}

fn show_selector(workspaces: &Workspaces) -> PageResult {
    let tpl = ramhorns::Template::new(HTML_TEXT).unwrap();

    let render = tpl.render(&workspaces);

    skyline_web::Webpage::new()
                        .htdocs_dir("contents")
                        .file("index.html", &render)
                        .file("selector.css", CSS_TEXT)
                        .file("selector.js", JAVASCRIPT_TEXT)
                        .file("check.svg", CHECK_ICON)
                        .open()
                        .unwrap()
}

// Please don't judge too hard I'm in a rush :'D
// I'm judging pretty hard rn - csk
pub fn workspace_selector() {
    let mut workspaces = Workspaces { workspace: vec![] };

    workspaces.workspace = get_workspaces();

    // Sort workspaces alphabatically
    workspaces.workspace.sort_by(|a, b| {
        a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase())
    });


    if workspaces.workspace.is_empty() {
        skyline_web::DialogOk::ok("Your directory does not contain any modpack.");
        return;
    }

    let response = show_selector(&workspaces);

    // If the user picked a modpack
    if response.get_exit_reason() == OfflineExitReason::LastUrl {
        let mut config = CONFIG.write();
        let mut config_changed = false;
        let mut workspace_name = String::from("Default");

        match response.get_last_url().unwrap() {
            "http://localhost/" => {
                return;
            }
            "http://localhost/default" => {
                config.paths.arc = PathBuf::from("rom:/arc");
                config.paths.umm = PathBuf::from("sd:/ultimate/mods");
                config_changed = true;
            }
            url => {
                let res = percent_decode_str(&url[LOCALHOST.len()..])
                    .decode_utf8_lossy()
                    .into_owned();

                let split = res.split("|").collect::<Vec<&str>>();

                // If someone manages to have this many workspaces they honestly deserve the panic
                let workspace_index = split[1 as usize].parse::<u8>().unwrap() as usize;
                let selected_type = split[0 as usize];

                let mut selector_workspace = std::path::PathBuf::from(WORKSPACES_LOCATION);
                selector_workspace.push(workspaces.workspace[workspace_index].name.to_owned());

                info!(
                    "[Menu | Workspace Selector] Selected workspace: '{}'",
                    selector_workspace.display().red()
                );

                workspace_name = String::from(selector_workspace.to_str().unwrap());

                let path = selector_workspace.to_str().unwrap();

                match selected_type {
                    "arc" => {
                        // Set Arc path in config
                        if Path::new(&format!("{}/{}", path, "arc")).exists() {
                            config.paths.arc = PathBuf::from(format!("{}/{}", path, "arc"));
                            config_changed = true;
                        }
                    },
                    "umm" => {
                        // Set UMM path in config
                        if Path::new(&format!("{}/{}", path, "umm")).exists() {
                            config.paths.umm = PathBuf::from(format!("{}/{}", path, "umm"));
                            config_changed = true;
                        }
                    },
                    "both" => {
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
                    _ => {
                        skyline_web::DialogOk::ok(
                            "How the hell did you break this???????",
                        );
                        return;
                    }
                }


            }
        }

        if config_changed {
            skyline_web::DialogOk::ok(format!(
                "Workspace {} has been applied.<br>
                Please reboot the game to apply your changes.",
                workspace_name
            ));
        }
    }
}
