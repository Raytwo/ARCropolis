#![feature(lazy_cell)]

pub mod arcadia;
pub use arcadia::*;
pub mod workspaces;
pub use workspaces::*;
pub mod config;
pub use config::*;
pub mod changelog;
pub use changelog::*;
pub mod files;
pub use files::*;
use skyline_web::Webpage;

mod utils;

pub fn show_main_menu() {
    let response = std::boxed::Box::new(
        Webpage::new()
            .htdocs_dir("contents")
            .file("index.html", &crate::files::MENU_HTML_TEXT)
            .file("menu.css", &crate::files::MENU_CSS_TEXT)
            .file("menu.js", &crate::files::MENU_JAVASCRIPT_TEXT)
            .file("common.js", &crate::files::COMMON_JAVASCRIPT_TEXT)
            .background(skyline_web::Background::Default)
            .boot_display(skyline_web::BootDisplay::Default)
            .open()
            .unwrap(),
    );

    match response.get_last_url().unwrap() {
        "http://localhost/" => {},
        url => match url {
            "http://localhost/arcadia" => {
                show_arcadia(None);
            },
            "http://localhost/workspaces" => {
                show_workspaces();
            },
            "http://localhost/config" => {
                show_config_editor(&mut ::config::GLOBAL_CONFIG.lock().unwrap());
            },
            _ => {},
        },
    }
}
