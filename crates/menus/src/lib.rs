pub mod arcadia;
pub use arcadia::show_arcadia;
pub mod workspaces;
pub use workspaces::show_workspaces;
pub mod config;
pub use config::show_config_editor;
pub mod changelog;
pub use changelog::{display_update_page, get_entries_from_md, MainEntry};
pub mod files;
use skyline_web::Webpage;

mod page;
mod utils;

pub fn show_main_menu() {
    page::write_static_assets(
        "menu",
        &[
            ("menu.html", files::MENU_HTML_TEXT.as_bytes()),
            ("configurator.css", files::CONFIG_CSS_TEXT.as_bytes()),
            ("common.js", files::COMMON_JAVASCRIPT_TEXT.as_bytes()),
            ("menu.js", files::MENU_JAVASCRIPT_TEXT.as_bytes()),
        ],
    );

    let response = Webpage::new()
        .htdocs_dir("contents")
        .start_page("menu.html")
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open()
        .unwrap();

    match response.get_last_url().unwrap_or_default() {
        "http://localhost/arcadia" => show_arcadia(None),
        "http://localhost/workspaces" => show_workspaces(),
        "http://localhost/config" => show_config_editor(&mut ::config::GLOBAL_CONFIG.lock().unwrap()),
        _ => {},
    }
}
