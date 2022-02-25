// #![feature(proc_macro_hygiene)]

use crate::config;
use log::info;
use percent_encoding::percent_decode_str;
use serde::Deserialize;
use skyline::nn;
use skyline_web::{ramhorns, Webpage, Visibility};
use smash_arc::Hash40;
use std::collections::HashSet;
use std::ffi::CString;
use std::path::Path;
use std::path::PathBuf;

static HTML_TEXT: &str = include_str!("../../../resources/templates/selector.html");
static CSS_TEXT: &str = include_str!("../../../resources/css/selector.css");
static ARCADIA_JAVASCRIPT_TEXT: &str = include_str!("../../../resources/js/selector.js");

const LOCALHOST: &str = "http://localhost/";

#[derive(Debug, Deserialize)]
pub struct ConfigChanged {
    category: String,
    value: String,
}



extern "C" {
    #[link_name = "\u{1}_ZN2nn3web26RequestExitOfflineHtmlPageEv"]
    pub fn request_exit();
}


pub fn show_config_editor() {
    //endregion

    //let tpl = ramhorns::Template::new(HTML_TEXT).unwrap();

    //let render = tpl.render(&mods);

    let session = std::boxed::Box::new(Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", HTML_TEXT)
        .file("selector.css", CSS_TEXT)
        .file("selector.js", ARCADIA_JAVASCRIPT_TEXT)
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open_session(Visibility::Default)
        .unwrap());

        let mut storage = skyline_config::acquire_storage("arcropolis").unwrap();

        while let Ok(msg) = session.recv_json::<ConfigChanged>() {
            match msg.category.as_str() {
                "lang" => {
                    storage.set_field("region", &msg.value).unwrap();
                    println!("Set region to {}", &msg.value);
                },
                "log" => {
                     storage.set_field("logging_level", &msg.value).unwrap();
                    println!("Set logger to {}", &msg.value);
                },
                _ => {
                    break;
                },
            }
        }
        
        session.closure();
        session.wait_for_exit();
}
