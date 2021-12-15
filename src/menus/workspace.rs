use std::{
    fs::{create_dir_all, read_dir},
    path::{Path, PathBuf},
};

use log::info;
use owo_colors::OwoColorize;
use skyline::nn::web::OfflineExitReason;
use skyline_web::{ramhorns, PageResult};

static HTML_TEXT: &str = include_str!("../../resources/templates/selector.html");
static JAVASCRIPT_TEXT: &str = include_str!("../../resources/js/selector.js");
static CSS_TEXT: &str = include_str!("../../resources/css/arcadia.css");
static CHECK_ICON: &[u8] = include_bytes!("../../resources/img/check.svg");
static WORKSPACES_LOCATION: &str = "sd:/ultimate/";

const LOCALHOST: &str = "http://localhost/";

fn show_selector() -> PageResult {
    //let tpl = ramhorns::Template::new(HTML_TEXT).unwrap();

    //let render = tpl.render(&workspaces);

    skyline_web::Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", &HTML_TEXT)
        .file("selector.css", CSS_TEXT)
        .file("selector.js", JAVASCRIPT_TEXT)
        .file("check.svg", CHECK_ICON)
        .open()
        .unwrap()
}

pub fn workspace_selector() {
    let response = show_selector();

    // If the user picked a modpack
    if response.get_exit_reason() == OfflineExitReason::LastUrl {
        
    }
}
