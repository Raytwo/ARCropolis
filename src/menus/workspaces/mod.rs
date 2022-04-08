use skyline_web::Webpage;

static HTML_TEXT: &str = include_str!("../../../resources/templates/workspaces.html");

pub fn show_workspaces() {
    Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", HTML_TEXT)
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open()
        .unwrap();
}
