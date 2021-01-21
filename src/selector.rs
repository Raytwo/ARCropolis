use std::{fs::{File, create_dir_all, read_dir}, path::Path};
use std::io::prelude::*;

use log::info;
use owo_colors::OwoColorize;
use skyline::nn::web::OfflineExitReason;
use skyline_web::PageResult;

use crate::config::Config;

use crate::config::CONFIG;

// Thanks jugeeya :^)
pub fn get_arguments_from_url(s: &str) -> String{
    let base_url_len = "http://localhost/".len();
    let total_len = s.len();

    s.chars().skip(base_url_len).take(total_len - base_url_len).collect()
}

#[derive(ramhorns::Content)]
pub struct Workspaces {
    pub workspace: Vec<Workspace>
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

    read_dir("rom:/arcropolis/workspaces").unwrap().enumerate().filter_map(|(index, entry)| {
        let entry = entry.unwrap();

        if entry.file_type().unwrap().is_file() {
            return None;
        }

        let workspace = Workspace {
            index: index as u8,
            name: String::from(entry.path().file_name().unwrap().to_str().unwrap()),
            in_use: false,
        };

        Some(workspace)
    }).collect()
}

fn show_selector(workspaces: &Workspaces) -> PageResult {
    let mut file = std::fs::File::open("sd:/atmosphere/contents/01006A800016E000/manual_html/html-document/contents.htdocs/arcropolis/selector/templates/index.html").unwrap();
    let mut page_content: String = String::new();
    file.read_to_string(&mut page_content);

    let tpl = ramhorns::Template::new(page_content).unwrap();

    let render = tpl.render(&workspaces);

    let mut webpage = skyline_web::Webpage::new();
    webpage.htdocs_dir("contents");
    webpage.file("index.html", &render);
    webpage.open().unwrap()
}

// Please don't judge too hard I'm in a rush :'D
pub fn workspace_selector() {
    let mut workspaces = Workspaces {
        workspace: vec![],
    };

    workspaces.workspace = get_workspaces();

    if workspaces.workspace.len() == 0 {
        skyline_web::DialogOk::ok("Your directory does not contain any modpack.");
        return;
    }

    let response = show_selector(&workspaces);

    // If the user picked a modpack
    if response.get_exit_reason() == OfflineExitReason::LastUrl {
        // If someone manages to have this many workspaces they honestly deserve the panic
        let modpack_index = get_arguments_from_url(response.get_last_url().unwrap()).parse::<u8>().unwrap() as usize;

        let mut selector_workspace = std::path::PathBuf::from("rom:/arcropolis/workspaces");
        selector_workspace.push(workspaces.workspace[modpack_index].name.to_owned());

        info!("[Menu | Workspace Selector] Selected workspace: '{}'", selector_workspace.display().red());

        let path = selector_workspace.to_str().unwrap();

        let mut config = Config::open().unwrap();

        let mut config_changed = false;

        // Set Arc path in config
        if Path::new(&format!("{}/{}", path, "arc")).exists() == true {
            config.paths.arc = format!("{}/{}", path, "arc");
            config_changed = true;
        }

        // Set UMM path in config
        if Path::new(&format!("{}/{}", path, "umm")).exists() == true {
            config.paths.umm = format!("{}/{}", path, "umm");
            config_changed = true;
        }

        if config_changed {
            config.save();
            skyline_web::DialogOk::ok(
                "Your changes have been applied.  
                        Consider rebooting the game to apply your changes.");
        } else {
            skyline_web::DialogOk::ok("The workspace your selected does not contain either a /arc or /umm directory.");
        }
    }
}