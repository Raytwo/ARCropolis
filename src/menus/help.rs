use skyline_web::{ramhorns, Webpage, encoding::Encoder};
use num_derive;
use num_traits::ToPrimitive;

#[repr(u8)]
#[derive(num_derive::ToPrimitive, serde::Deserialize)]
pub enum HelpTheme {
    Red = 1,
    Green,
    Blue,
    Orange,
    Pink,
}

impl ramhorns::Content for HelpTheme {
    fn is_truthy(&self) -> bool {
        true
    }

    fn capacity_hint(&self, _tpl: &skyline_web::Template) -> usize {
        1
    }

    fn render_escaped<E: skyline_web::encoding::Encoder>(&self, encoder: &mut E) -> Result<(), E::Error> {
        encoder.write_escaped(&self.to_u8().unwrap().to_string())
    }

    fn render_unescaped<E: skyline_web::encoding::Encoder>(&self, encoder: &mut E) -> Result<(), E::Error> {
        encoder.write_unescaped(&self.to_u8().unwrap().to_string())
    }

    fn render_cmark<E: skyline_web::encoding::Encoder>(&self, encoder: &mut E) -> Result<(), E::Error> {
        self.render_escaped(encoder)
    }
}

#[derive(ramhorns::Content, serde::Deserialize)]
pub struct HelpMenu {
    pub title: String,
    pub theme: HelpTheme,
    pub table_of_content: bool,
    pub chapters: Vec<HelpChapter>,
}

#[derive(ramhorns::Content, serde::Deserialize)]
pub struct HelpChapter {
    pub id: u8,
    pub title: String,
    pub pages: Vec<HelpPage>,
}

#[derive(ramhorns::Content, serde::Deserialize)]
pub struct HelpPage {
    pub id: u8,
    pub title: String,
    pub text: String,
    pub picture_url: String,
}

static HTML_TEXT: &str = include_str!("../../resources/templates/help.html");
// Change this for different pictures
static IMAGE_BYTES: &[u8] = include_bytes!("../../resources/img/note_thumbnail.png");

pub fn display_help() {
    let tpl = ramhorns::Template::new(HTML_TEXT).unwrap();

    let page = HelpMenu {
        title: "Recovery".to_string(),
        theme: HelpTheme::Blue,
        // Will only display properly for Green and Blue theme
        table_of_content: true,
        chapters: vec![HelpChapter {
            id: 1,
            title: String::from("Chapter 1"),
            pages: vec![HelpPage {
                id: 1,
                title: String::from("Placeholder title"),
                text: String::from("Placeholder text"),
                picture_url: String::from("./howto/img/all_battle_01_01.webp"),
            },
            HelpPage {
                id: 2,
                title: String::from("Placeholder title 2"),
                text: String::from("Placeholder text 2"),
                picture_url: String::from("./howto/img/all_battle_01_01.webp"),
            },]
        },
        HelpChapter {
            id: 2,
            title: String::from("Chapter 2"),
            pages: vec![HelpPage {
                id: 1,
                title: String::from("Placeholder title"),
                text: String::from("Placeholder text"),
                picture_url: String::from("./howto/img/all_battle_01_01.webp"),
            },
            HelpPage {
                id: 2,
                title: String::from("Placeholder title 2"),
                text: String::from("Placeholder text 2"),
                picture_url: String::from("./howto/img/all_battle_01_01.webp"),
            },]
        }],
    };

    let render = tpl.render(&page);

    Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", &render)
        .file("notes.png", &IMAGE_BYTES)
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open()
        .unwrap();
}
