pub mod arcadia;
pub use arcadia::*;
pub mod workspaces;
pub use workspaces::*;
pub mod config;
pub use config::*;
pub mod changelog;
pub use changelog::*;
use skyline_web::Webpage;

static HTML_TEXT: &str = include_str!("../../resources/templates/menu.html");
static CSS_TEXT: &str = include_str!("../../resources/css/configurator.css");
static ARCADIA_JAVASCRIPT_TEXT: &str = include_str!("../../resources/js/menu.js");

const LOCALHOST: &str = "http://localhost/";

pub fn show_main_menu() {
    let response = std::boxed::Box::new(
        Webpage::new()
            .htdocs_dir("contents")
            .file("index.html", &HTML_TEXT)
            .file("menu.css", CSS_TEXT)
            .file("menu.js", ARCADIA_JAVASCRIPT_TEXT)
            .background(skyline_web::Background::Default)
            .boot_display(skyline_web::BootDisplay::Default)
            .open()
            .unwrap(),
    );

    match response.get_last_url().unwrap() {
        "http://localhost/" => {},
        url => {
            match url {
                "http://localhost/arcadia" => {
                    show_arcadia();
                },
                "http://localhost/workspaces" => {
                    show_workspaces();
                },
                "http://localhost/config" => {
                    show_config_editor();
                },
                _ => {},
            }
        },
    }
}
