use std::{
    collections::{HashSet, HashMap},
    fs
};

use skyline_web::Webpage;
use serde::{Deserialize, Serialize};
use crate::config;

#[derive(Serialize, Deserialize, Debug)]
pub struct Information {
    workspaces: Vec<String>,
    active_workspace: String
}

#[derive(Debug, Deserialize)]
pub enum WorkspacesMessage {
    Create { name: String },
    SetActive { name: String },
    Edit { name: String },
    Rename { source_name: String, target_name: String },
    Remove { name: String },
    WriteItDown { text: String },
    ClosureRequest,
}

pub fn save_workspaces(workspace_list: &HashMap<String, String>){
    // let mut storage = config::GLOBAL_CONFIG.lock().unwrap();
    // storage.set_field_json("workspace_list", workspace_list).unwrap_or_default();
}

pub fn show_workspaces() {

    let mut storage = config::GLOBAL_CONFIG.lock().unwrap();
    let active_workspace: String = storage.get_field("workspace").unwrap_or("Default".to_string());
    let mut workspace_list: HashMap<String, String> = storage.get_field_json("workspace_list").unwrap_or_default();

    let info: Information = Information {
        workspaces: workspace_list.iter().map(|(k, v)| k.clone()).collect(),
        active_workspace: active_workspace
    };

    let mut workspace_to_edit: Option<String> = None; 

    let session = Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", &crate::menus::files::WORKSPACES_HTML_TEXT)
        .file("workspaces.css", &crate::menus::files::WORKSPACES_CSS_TEXT)
        .file("workspaces.js", &crate::menus::files::WORKSPACES_JAVASCRIPT_TEXT)
        .file("check.svg", &crate::menus::files::CHECK_SVG)
        .file("common.js", &crate::menus::files::COMMON_JAVASCRIPT_TEXT)
        .file("workspaces.json", &serde_json::to_string(&info).unwrap())
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open_session(skyline_web::Visibility::Default)
        .unwrap();

    while let Ok(message) = session.recv_json::<WorkspacesMessage>() {
        match message {
            WorkspacesMessage::Create { name } => {
                workspace_list.insert(name, format!("preset{}", workspace_list.len() + 1));
            }
            WorkspacesMessage::WriteItDown { text } => {
                fs::write("sd:/nx.info", text).expect("Unable to write file");
            }
            WorkspacesMessage::SetActive { name } => {
                storage.set_field("workspace", name).unwrap();
            }
            WorkspacesMessage::Edit { name } => {
                session.wait_for_exit();
                session.exit();
                save_workspaces(&workspace_list);
                workspace_to_edit = Some(name);
                break;
            }
            WorkspacesMessage::Rename { source_name, target_name } => {
                let preset_name = workspace_list[&source_name].clone();
                workspace_list.remove(&source_name);
                workspace_list.insert(target_name, preset_name);
            }
            WorkspacesMessage::Remove { name } => {
                workspace_list.remove(&name);
            }
            WorkspacesMessage::ClosureRequest => {
                session.wait_for_exit();
                session.exit();
                save_workspaces(&workspace_list);
                break;
            }
        }
    }

    println!("Opening ARCadia from Workspaces.rs...");

    match workspace_to_edit {
        Some(s) => {            
           crate::menus::arcadia::show_arcadia(Some(s))
        },
        None => {}
    }

}