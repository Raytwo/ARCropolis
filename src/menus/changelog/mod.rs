use skyline_web::{ramhorns, Webpage};

#[derive(ramhorns::Content, serde::Deserialize)]
pub struct NotesEntry {
    pub section_title: String,
    pub contents: String,
}

#[derive(ramhorns::Content, serde::Deserialize)]
pub struct MainEntry {
    pub title: String,
    pub date: String,
    pub description: String,
    pub entries: Vec<NotesEntry>,
}

pub fn display_update_page(info: &MainEntry) {
    let tpl = ramhorns::Template::new(crate::menus::files::CHANGELOG_HTML_TEXT).unwrap();

    let render = tpl.render(&info);

    Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", &render)
        .file("notes.png", &crate::menus::files::CHANGELOG_IMAGE_BYTES)
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open()
        .unwrap();
}

fn check_for_changelog() {
    if let Ok(changelog) = std::fs::read_to_string("sd:/ultimate/arcropolis/changelog.toml") {
        match toml::from_str(&changelog) {
            Ok(changelog) => {
                menus::display_update_page(&changelog);
                std::fs::remove_file("sd:/ultimate/arcropolis/changelog.toml").unwrap();
            },
            Err(_) => {
                warn!("Changelog could not be parsed. Is the file malformed?");
            },
        }
    }
}