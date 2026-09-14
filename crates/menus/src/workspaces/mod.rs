use std::collections::{HashMap, HashSet};

use ::config::GLOBAL_CONFIG;
use log::error;
use serde::{Deserialize, Serialize};
use skyline_web::Webpage;
use smash_arc::Hash40;

use crate::page;

#[derive(Serialize, Debug)]
struct Information {
    workspaces: Vec<String>,
    active_workspace: String,
}

#[derive(Debug, Deserialize)]
pub enum WorkspacesMessage {
    Create { name: String },
    SetActive { name: String },
    Edit { name: String },
    Rename { source_name: String, target_name: String },
    Remove { name: String },
    Duplicate { source_name: String, target_name: String },
    ClosureRequest,
}

fn save_list(workspace_list: &HashMap<String, String>) {
    if let Err(err) = GLOBAL_CONFIG.lock().unwrap().set_field_json("workspace_list", workspace_list) {
        error!("Could not save the workspace list: {}", err);
    }
}

pub fn show_workspaces() {
    let (mut active_workspace, mut workspace_list): (String, HashMap<String, String>) = {
        let storage = GLOBAL_CONFIG.lock().unwrap();
        (
            storage.get_field("workspace").unwrap_or_else(|_| "Default".to_string()),
            storage.get_field_json("workspace_list").unwrap_or_default(),
        )
    };
    let prev_set_workspace = active_workspace.clone();
    let mut workspace_to_edit: Option<String> = None;

    page::write_static_assets(
        "workspaces",
        &[
            ("workspaces.html", crate::files::WORKSPACES_HTML_TEXT.as_bytes()),
            ("workspaces.css", crate::files::WORKSPACES_CSS_TEXT.as_bytes()),
            ("workspaces.js", crate::files::WORKSPACES_JAVASCRIPT_TEXT.as_bytes()),
            ("check.svg", crate::files::CHECK_SVG),
            ("common.js", crate::files::COMMON_JAVASCRIPT_TEXT.as_bytes()),
        ],
    );
    let info = Information {
        workspaces: workspace_list.keys().cloned().collect(),
        active_workspace: active_workspace.clone(),
    };
    page::write_file("workspaces_data.js", page::data_script("WORKSPACES_DATA", &info));

    let session = Webpage::new()
        .htdocs_dir("contents")
        .start_page("workspaces.html")
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open_session(skyline_web::Visibility::Default)
        .unwrap();

    loop {
        match page::next_message::<WorkspacesMessage>(&session) {
            WorkspacesMessage::Create { name } => {
                let preset_name = format!("{}_preset{}", name, workspace_list.len() + 1);
                workspace_list.insert(name, preset_name.clone());
                let mut storage = GLOBAL_CONFIG.lock().unwrap();
                if let Err(err) = storage.set_field_json(&preset_name, &HashSet::<Hash40>::new()) {
                    error!("Could not create the preset '{}': {}", preset_name, err);
                }
                if let Err(err) = storage.set_field_json("workspace_list", &workspace_list) {
                    error!("Could not save the workspace list: {}", err);
                }
            },
            WorkspacesMessage::SetActive { name } => {
                active_workspace = name.clone();
                if let Err(err) = GLOBAL_CONFIG.lock().unwrap().set_field("workspace", name) {
                    error!("Could not save the active workspace: {}", err);
                }
            },
            WorkspacesMessage::Edit { name } => {
                session.exit();
                session.wait_for_exit();
                workspace_to_edit = Some(name);
                break;
            },
            WorkspacesMessage::Rename { source_name, target_name } => {
                let Some(preset_name) = workspace_list.remove(&source_name) else {
                    error!("Cannot rename workspace '{}', it does not exist", source_name);
                    continue;
                };
                workspace_list.insert(target_name, preset_name);
                save_list(&workspace_list);
            },
            WorkspacesMessage::Remove { name } => {
                workspace_list.remove(&name);
                save_list(&workspace_list);
            },
            WorkspacesMessage::Duplicate { source_name, target_name } => {
                let Some(source_preset) = workspace_list.get(&source_name).cloned() else {
                    error!("Cannot duplicate workspace '{}', it does not exist", source_name);
                    continue;
                };
                let target_preset = format!("{}_preset{}", target_name, workspace_list.len() + 1);
                let mut storage = GLOBAL_CONFIG.lock().unwrap();
                let presets: HashSet<Hash40> = storage.get_field_json(&source_preset).unwrap_or_default();
                workspace_list.insert(target_name, target_preset.clone());
                if let Err(err) = storage.set_field_json(&target_preset, &presets) {
                    error!("Could not create the preset '{}': {}", target_preset, err);
                }
                if let Err(err) = storage.set_field_json("workspace_list", &workspace_list) {
                    error!("Could not save the workspace list: {}", err);
                }
            },
            WorkspacesMessage::ClosureRequest => {
                session.exit();
                session.wait_for_exit();
                break;
            },
        }
    }

    if !workspace_list.contains_key(&active_workspace) {
        active_workspace = "Default".to_string();
        if let Err(err) = GLOBAL_CONFIG.lock().unwrap().set_field("workspace", active_workspace.clone()) {
            error!("Could not reset the active workspace: {}", err);
        }
    }

    if let Some(name) = workspace_to_edit {
        println!("Opening ARCadia from workspaces.rs...");
        crate::arcadia::show_arcadia(Some(name))
    }

    if active_workspace != prev_set_workspace
        && skyline_web::dialog::Dialog::yes_no(format!(
            "Your active workspace has successfully been changed to {}!<br>Your changes will take effect on the next boot.<br>Would you like to reboot the game to reload your mods?",
            active_workspace
        ))
    {
        unsafe { skyline::nn::oe::RequestToRelaunchApplication() };
    }
}
