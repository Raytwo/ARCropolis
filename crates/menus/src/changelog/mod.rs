use serde::Deserialize;
use skyline_web::Webpage;

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
        let mut contributor = Contributor::default();
        contributor.login = Some(name.to_string());
        contributor
    }

    fn get_contributor_from_git(username: &str) -> Contributor {
        match minreq::get(format!("https://api.github.com/users/{}", username))
            .with_header("Accept", "application/vnd.github.v3+json")
            .with_header("User-Agent", "ARCropolis")
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
        match &self.avatar_url {
            Some(url) => {
                match minreq::get(url)
                    .with_header("Accept", "application/vnd.github.v3+json")
                    .with_header("User-Agent", "ARCropolis")
                    .send()
                {
                    Ok(resp) => resp.as_bytes().to_vec(),
                    Err(err) => {
                        println!("Failed getting contributor avatar! Reason: {:?}", err);
                        vec![]
                    },
                }
            },
            None => vec![],
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum NotesMessage {
    UpdateState { state: bool },
    Prompt { state: bool },
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

pub fn build_html(info: &MainEntry) -> String {
    let mut rendered = crate::files::CHANGELOG_HTML_TEXT.to_string();
    rendered = rendered.replace("{{title}}", &info.title);
    rendered = rendered.replace("{{date}}", &info.date);
    rendered = rendered.replace("{{description}}", &info.description);

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

    if info.contributors.len() > 0 {
        let formatted = format!(
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
            {
                let mut res = String::new();
                for i in 0..info.contributors.len() {
                    let mut current_contributor: Vec<String> = vec![];

                    current_contributor.push(format!(
                        "<li class=\"contributor-name\">{} {}</li>",
                        info.contributors[i].login.as_ref().unwrap(),
                        {
                            match &info.contributors[i].name {
                                Some(name) => format!("<span style=\"font-size: 20px;\">({})</span>", name),
                                None => format!(""),
                            }
                        }
                    ));

                    match &info.contributors[i].twitter_username {
                        Some(twitter) => current_contributor.push(format!("<li class=\"contributor-twitter\">Twitter: @{}</li>", twitter)),
                        None => {},
                    }

                    match &info.contributors[i].blog {
                        Some(blog) => current_contributor.push(format!("<li class=\"contributor-blog\">{}</li>", blog)),
                        None => {},
                    }

                    match &info.contributors[i].bio {
                        Some(bio) => current_contributor.push(format!("<li class=\"contributor-bio\">{}</li>", bio)),
                        None => {},
                    }

                    res.push_str(&format!(
                        "
                <li class=\"contributor\">
                    <div class=\"contributor-image\" style=\"background-image: url('./{}.png');\"></div>
                    <ul class=\"contributor-detail\">
                        {}
                    </ul>
                </li>",
                        i,
                        current_contributor.join("\n")
                    ));
                }
                res
            }
        );

        entries.push(formatted);
    }

    rendered = rendered.replace("{{entries}}", &entries.join("\n"));

    rendered
}

pub fn get_entries_from_md(text: &String) -> (Vec<Contributor>, Vec<NotesEntry>) {
    let mut entries: Vec<NotesEntry> = vec![];
    let mut found_contributors: Vec<&str> = vec![];
    let data = text.split("\\r\\n").collect::<Vec<&str>>();
    let mut i = 0;
    while i < data.len() {
        if data[i].starts_with("### ") {
            let heading = data[i].strip_prefix("### ").unwrap().trim().to_string();
            let mut bullet_points: Vec<String> = vec![];
            let mut y = i + 1;
            while y != data.len() && data[y] != "" {
                match data[y].strip_prefix("* ") {
                    Some(mut line) => {
                        if line.contains("(@") {
                            line = line.split("(@").collect::<Vec<&str>>()[0].trim();
                        }
                        bullet_points.push(format!("<li>{}</li>", line));
                    },
                    None => {
                        break;
                    },
                }

                if data[y].contains("@") {
                    let split = data[y].split("@").collect::<Vec<&str>>();
                    for z in 1..split.len() {
                        static EOC: &[char] = &[' ', '/', ')', '\\'];
                        let contributor = &split[z][..split[z].find(EOC).unwrap_or(split[z].len())];

                        if !found_contributors.contains(&contributor) {
                            found_contributors.push(&contributor);
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

    let mut contributors: Vec<Contributor> = vec![];

    for contributor in found_contributors {
        contributors.push(Contributor::get_contributor_from_git(contributor));
    }

    (contributors, entries)
}

pub fn display_update_page(info: &MainEntry) -> bool {
    let mut user_images: Vec<(String, Vec<u8>)> = Vec::new();

    for i in 0..info.contributors.len() {
        user_images.push((format!("{}.png", i), info.contributors[i].get_contributor_image()));
    }

    let session = Webpage::new()
        .htdocs_dir("contents")
        .file("index.html", &build_html(info))
        .file("notes.png", &crate::files::CHANGELOG_IMAGE_BYTES)
        .files(&user_images)
        .background(skyline_web::Background::Default)
        .boot_display(skyline_web::BootDisplay::Default)
        .open_session(skyline_web::Visibility::Default)
        .unwrap();

    let mut update = false;

    while let Ok(message) = session.recv_json::<NotesMessage>() {
        match message {
            NotesMessage::UpdateState { state } => {
                update = state;
            },
            NotesMessage::Prompt { state } => {
                todo!()
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
