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
