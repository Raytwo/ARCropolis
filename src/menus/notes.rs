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

static HTML_TEXT: &str = include_str!("../../resources/templates/notes.html");
// Change this for different pictures
static IMAGE_BYTES: &[u8] = include_bytes!("../../resources/img/note_thumbnail.png");

pub fn display_update_page(info: &MainEntry) {
    let tpl = ramhorns::Template::new(HTML_TEXT).unwrap();

    let render = tpl.render(&info);

    Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", &render)
        .file("notes.png", &IMAGE_BYTES)
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open()
        .unwrap();
}
