
use std::{
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    time::Duration,
};

use log::error;
use serde::{de::DeserializeOwned, Serialize};
use skyline_web::WebSession;

pub const HTDOCS: &str = "sd:/atmosphere/contents/01006A800016E000/manual_html/html-document/contents.htdocs";

pub fn path(name: &str) -> PathBuf {
    Path::new(HTDOCS).join(name)
}

pub fn write_static_assets(page: &str, assets: &[(&str, &[u8])]) {
    let mut hasher = DefaultHasher::new();
    for (name, data) in assets {
        name.hash(&mut hasher);
        data.hash(&mut hasher);
    }
    let stamp = format!("{:016x}", hasher.finish());

    let stamp_path = path(&format!("{}.stamp", page));
    if fs::read_to_string(&stamp_path).map(|current| current == stamp).unwrap_or(false) {
        return;
    }

    if let Err(err) = fs::create_dir_all(HTDOCS) {
        error!("Could not create the page folder: {}", err);
        return;
    }
    for (name, data) in assets {
        write_file(name, data);
    }
    if let Err(err) = fs::write(&stamp_path, stamp) {
        error!("Could not write the stamp file for the {} page: {}", page, err);
    }
}

pub fn write_file(name: &str, data: impl AsRef<[u8]>) {
    if let Err(err) = fs::write(path(name), data) {
        error!("Could not write '{}' for the menus: {}", name, err);
    }
}

pub fn data_script(variable: &str, value: &impl Serialize) -> String {
    let json = serde_json::to_string(value).unwrap().replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029");
    format!("var {} = {};", variable, json)
}

pub fn next_message<T: DeserializeOwned>(session: &WebSession) -> T {
    loop {
        match session.try_recv_json_max::<T>(0x4000) {
            Some(Ok(message)) => return message,
            Some(Err(err)) => error!("Could not read a message from the page: {}", err),
            None => std::thread::sleep(Duration::from_millis(8)),
        }
    }
}
