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

pub fn show_main_menu() {
    let response = std::boxed::Box::new(
        Webpage::new()
            .htdocs_dir("contents")
            .file("index.html", &crate::menus::files::MENU_HTML_TEXT)
            .file("menu.css", &crate::menus::files::MENU_CSS_TEXT)
            .file("menu.js", &crate::menus::files::MENU_JAVASCRIPT_TEXT)
            .file("common.js", &crate::menus::files::COMMON_JAVASCRIPT_TEXT)
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
                show_config_editor(&mut crate::config::GLOBAL_CONFIG.read());
            },
            _ => {},
        },
    }
}
