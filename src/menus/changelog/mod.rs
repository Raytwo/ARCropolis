use skyline_web::Webpage;

#[derive(serde::Deserialize)]
pub struct NotesEntry {
    pub section_title: String,
    pub contents: String,
}

#[derive(serde::Deserialize)]
pub struct MainEntry {
    pub title: String,
    pub date: String,
    pub description: String,
    pub entries: Vec<NotesEntry>,
}

pub fn build_html(info: &MainEntry) -> String {
    let mut rendered = crate::menus::files::CHANGELOG_HTML_TEXT.to_string();
    rendered = rendered.replace("{{title}}", &info.title);
    rendered = rendered.replace("{{date}}", &info.date);
    rendered = rendered.replace("{{description}}", &info.description);
    
    let entries = info.entries.iter().map(|entry| {
        format!("
        <div class=\"section\">
            <h2 class=\"section-header\">
                <div>{}</div>
            </h2>
            <div>
                {}
            </div>
        </div>
        ", entry.section_title, entry.contents)
    }).collect::<Vec<String>>().join("\n");
    
    rendered = rendered.replace("{{entries}}", &entries);

    rendered
}

pub fn display_update_page(info: &MainEntry) {
    Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", &build_html(info))
        .file("notes.png", &crate::menus::files::CHANGELOG_IMAGE_BYTES)
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open()
        .unwrap();
}
