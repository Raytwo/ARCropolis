use serde::Deserialize;
use skyline_web::Webpage;

use crate::page;

const REQUEST_TIMEOUT: u64 = 5;

#[derive(Deserialize, Clone)]
pub struct NotesEntry {
    pub section_title: String,
    pub contents: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Contributor {
    pub login: Option<String>,
    pub id: i64,
    #[serde(rename = "node_id")]
    pub node_id: Option<String>,
    #[serde(rename = "avatar_url")]
    pub avatar_url: Option<String>,
    #[serde(rename = "gravatar_id")]
    pub gravatar_id: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "html_url")]
    pub html_url: Option<String>,
    #[serde(rename = "followers_url")]
    pub followers_url: Option<String>,
    #[serde(rename = "following_url")]
    pub following_url: Option<String>,
    #[serde(rename = "gists_url")]
    pub gists_url: Option<String>,
    #[serde(rename = "starred_url")]
    pub starred_url: Option<String>,
    #[serde(rename = "subscriptions_url")]
    pub subscriptions_url: Option<String>,
    #[serde(rename = "organizations_url")]
    pub organizations_url: Option<String>,
    #[serde(rename = "repos_url")]
    pub repos_url: Option<String>,
    #[serde(rename = "events_url")]
    pub events_url: Option<String>,
    #[serde(rename = "received_events_url")]
    pub received_events_url: Option<String>,
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    #[serde(rename = "site_admin")]
    pub site_admin: bool,
    pub name: Option<String>,
    pub company: Option<String>,
    pub blog: Option<String>,
    pub location: Option<String>,
    pub email: Option<String>,
    pub hireable: Option<bool>,
    pub bio: Option<String>,
    #[serde(rename = "twitter_username")]
    pub twitter_username: Option<String>,
    #[serde(rename = "public_repos")]
    pub public_repos: i64,
    #[serde(rename = "public_gists")]
    pub public_gists: i64,
    pub followers: i64,
    pub following: i64,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "updated_at")]
    pub updated_at: String,
}

impl Contributor {
    fn make_contributor_name_only(name: &str) -> Contributor {
        Contributor {
            login: Some(name.to_string()),
            ..Contributor::default()
        }
    }

    fn get_contributor_from_git(username: &str) -> Contributor {
        match minreq::get(format!("https://api.github.com/users/{}", username))
            .with_header("Accept", "application/vnd.github.v3+json")
            .with_header("User-Agent", "ARCropolis")
            .with_timeout(REQUEST_TIMEOUT)
            .send()
        {
            Ok(resp) => match resp.json::<Contributor>() {
                Ok(contributor) => contributor,
                Err(_) => Contributor::make_contributor_name_only(username),
            },
            Err(_) => Contributor::make_contributor_name_only(username),
        }
    }

    fn get_contributor_image(&self) -> Vec<u8> {
        let Some(url) = &self.avatar_url else {
            return vec![];
        };
        match minreq::get(url).with_header("User-Agent", "ARCropolis").with_timeout(REQUEST_TIMEOUT).send() {
            Ok(resp) => resp.as_bytes().to_vec(),
            Err(err) => {
                println!("Failed getting contributor avatar! Reason: {:?}", err);
                vec![]
            },
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum NotesMessage {
    UpdateState { state: bool },
    Closure,
}

#[derive(Deserialize)]
pub struct MainEntry {
    pub title: String,
    pub date: String,
    pub description: String,
    pub entries: Vec<NotesEntry>,
    pub contributors: Vec<Contributor>,
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn build_html(info: &MainEntry) -> String {
    let mut entries = info
        .entries
        .iter()
        .map(|entry| {
            format!(
                "
        <div class=\"section\">
            <h2 class=\"section-header\">
                <div>{}</div>
            </h2>
            <div>
                {}
            </div>
        </div>
        ",
                entry.section_title, entry.contents
            )
        })
        .collect::<Vec<String>>();

    if !info.contributors.is_empty() {
        let mut contributors = String::new();
        for (i, contributor) in info.contributors.iter().enumerate() {
            let mut details = vec![format!(
                "<li class=\"contributor-name\">{} {}</li>",
                escape_html(contributor.login.as_deref().unwrap_or("???")),
                contributor
                    .name
                    .as_ref()
                    .map(|name| format!("<span style=\"font-size: 20px;\">({})</span>", escape_html(name)))
                    .unwrap_or_default()
            )];
            if let Some(twitter) = &contributor.twitter_username {
                details.push(format!("<li class=\"contributor-twitter\">Twitter: @{}</li>", escape_html(twitter)));
            }
            if let Some(blog) = &contributor.blog {
                details.push(format!("<li class=\"contributor-blog\">{}</li>", escape_html(blog)));
            }
            if let Some(bio) = &contributor.bio {
                details.push(format!("<li class=\"contributor-bio\">{}</li>", escape_html(bio)));
            }

            contributors.push_str(&format!(
                "
                <li class=\"contributor\">
                    <div class=\"contributor-image\" style=\"background-image: url('./contributor{}.png');\"></div>
                    <ul class=\"contributor-detail\">
                        {}
                    </ul>
                </li>",
                i,
                details.join("\n")
            ));
        }

        entries.push(format!(
            "
        <div class=\"section\">
            <h2 class=\"section-header\">
                <div>Contributors</div>
            </h2>
            <div>
                <ul class=\"contributors-holder\">
                    {}
                </ul>
            </div>
        </div>
        ",
            contributors
        ));
    }

    crate::files::CHANGELOG_HTML_TEXT
        .replace("{{title}}", &info.title)
        .replace("{{date}}", &info.date)
        .replace("{{description}}", &info.description)
        .replace("{{entries}}", &entries.join("\n"))
}

pub fn get_entries_from_md(text: &str) -> (Vec<Contributor>, Vec<NotesEntry>) {
    let mut entries: Vec<NotesEntry> = vec![];
    let mut found_contributors: Vec<&str> = vec![];
    let data = text.lines().collect::<Vec<&str>>();
    let mut i = 0;
    while i < data.len() {
        if data[i].starts_with("### ") {
            let heading = escape_html(data[i].strip_prefix("### ").unwrap().trim());
            let mut bullet_points: Vec<String> = vec![];
            let mut y = i + 1;
            while y != data.len() && !data[y].is_empty() {
                match data[y].strip_prefix("* ") {
                    Some(mut line) => {
                        if line.contains("(@") {
                            line = line.split("(@").collect::<Vec<&str>>()[0].trim();
                        }
                        bullet_points.push(format!("<li>{}</li>", escape_html(line)));
                    },
                    None => {
                        break;
                    },
                }

                if data[y].contains('@') {
                    let split = data[y].split('@').collect::<Vec<&str>>();
                    for part in split.iter().skip(1) {
                        static EOC: &[char] = &[' ', '/', ')', '\\'];
                        let contributor = &part[..part.find(EOC).unwrap_or(part.len())];

                        if !found_contributors.contains(&contributor) {
                            found_contributors.push(contributor);
                        }
                    }
                }

                y += 1;
            }
            i = y;
            entries.push(NotesEntry {
                section_title: heading,
                contents: format!("<ul>{}</ul>", bullet_points.join("")),
            })
        } else {
            i += 1;
        }
    }

    let contributors = found_contributors.into_iter().map(Contributor::get_contributor_from_git).collect();

    (contributors, entries)
}

pub fn display_update_page(info: &MainEntry) -> bool {
    page::write_static_assets("notes", &[("notes.png", crate::files::CHANGELOG_IMAGE_BYTES)]);
    page::write_file("notes.html", build_html(info));
    for (i, contributor) in info.contributors.iter().enumerate() {
        page::write_file(&format!("contributor{}.png", i), contributor.get_contributor_image());
    }

    let session = Webpage::new()
        .htdocs_dir("contents")
        .start_page("notes.html")
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open_session(skyline_web::Visibility::Default)
        .unwrap();

    let mut update = false;

    loop {
        match page::next_message::<NotesMessage>(&session) {
            NotesMessage::UpdateState { state } => {
                update = state;
            },
            NotesMessage::Closure => {
                session.exit();
                session.wait_for_exit();
                break;
            },
        }
    }

    update
}
