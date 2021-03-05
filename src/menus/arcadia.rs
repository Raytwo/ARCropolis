// // #![feature(proc_macro_hygiene)]

// use crate::config::CONFIG;
// use percent_encoding::percent_decode_str;
// use serde::Deserialize;
// use skyline::nn;
// use skyline_web::Webpage;
// use std::io::prelude::*;
// use std::ffi::CString;
// use std::path::Path;

// use log::info;

// #[derive(Debug, ramhorns::Content)]
// pub struct Entries {
//     workspace: String,
//     entries: Vec<Entry>,
// }

// #[derive(Debug, PartialEq, PartialOrd, Deserialize, ramhorns::Content)]
// pub struct Entry {
//     id: Option<u32>,
//     folder_name: Option<String>,
//     is_disabled: Option<bool>,
//     display_name: Option<String>,
//     version: Option<String>,
//     description: Option<String>,
//     category: Option<String>,
//     image: Option<String>,
// }

// #[derive(Debug, Deserialize)]
// pub struct ModStatues {
//     is_disabled: Vec<bool>,
// }

// const LOCALHOST: &str = "http://localhost/";

// pub fn RenameFolder(src: &Path, dest: &Path) -> u32 {
//     let old_cstr = CString::new(src.to_str().unwrap().as_bytes() as &[u8]).unwrap();

//     let new_cstr = CString::new(dest.to_str().unwrap().as_bytes() as &[u8]).unwrap();

//     unsafe { nn::fs::RenameDirectory(old_cstr.as_ptr() as _, new_cstr.as_ptr() as _) as u32 }
// }

// pub fn get_mods(workspace: &str) -> Vec<Entry> {
//     let mut i = 0;
//     std::fs::read_dir(workspace)
//         .unwrap()
//         .filter_map(|path| {
//             let path_to_be_used;
//             let disabled;

//             let original = format!("{}", path.unwrap().path().display());
//             let counter_part = match original.chars().next() {
//                 Some('.') => {
//                     disabled = true;
//                     original[1..].to_owned()
//                 }
//                 _ => {
//                     disabled = false;
//                     format!(".{}", &original)
//                 }
//             };

//             let original_path = format!("{}/{}", workspace, &original);
//             let counter_part_path = format!("{}/{}", workspace, &counter_part);

//             if std::fs::metadata(&original_path).is_ok()
//                 & std::fs::metadata(&counter_part_path).is_ok()
//             {
//                 path_to_be_used = format!("{} (2)", &counter_part_path);
//                 RenameFolder(Path::new(&counter_part_path), Path::new(&path_to_be_used));
//             } else {
//                 path_to_be_used = original_path;
//             }

//             let mut folder_name = Path::new(&path_to_be_used)
//                 .file_name()
//                 .unwrap()
//                 .to_os_string()
//                 .into_string()
//                 .unwrap();

//             folder_name = if folder_name.chars().next().unwrap() == '.' {
//                 folder_name[1..].to_string()
//             } else {
//                 folder_name
//             };

//             let info_path = format!("{}/info.toml", path_to_be_used);

//             let mod_info: Entry = if std::fs::metadata(&info_path).is_ok() {
//                 let mut res: Entry =
//                     toml::from_str(&std::fs::read_to_string(&info_path).unwrap()).unwrap();
//                 res.id = Some(i);
//                 res.folder_name = Some(folder_name.to_string());
//                 res.is_disabled = Some(disabled);
//                 res.image = Some(format!("{}/preview.webp", path_to_be_used).to_string());
//                 res
//             } else {
//                 Entry {
//                     id: Some(i),
//                     folder_name: Some(folder_name.to_string()),
//                     is_disabled: Some(disabled),
//                     display_name: Some(folder_name.to_string()),
//                     version: Some("???".to_string()),
//                     description: Some("".to_string()),
//                     category: Some("Misc".to_string()),
//                     image: Some(format!("{}/preview.webp", path_to_be_used).to_string()),
//                 }
//             };

//             i += 1;

//             Some(mod_info)
//         })
//         .collect()
// }

// pub fn show_arcadia() {
//     let workspace = CONFIG.read().paths.umm.to_str().unwrap().to_string();

//     let mut mods: Entries = Entries {
//         entries: get_mods(&workspace),
//         workspace: Path::new(&workspace)
//             .file_name()
//             .unwrap()
//             .to_os_string()
//             .into_string()
//             .unwrap(),
//     };

//     // Sort mods alphabatically
//     mods.entries.sort_by(|a, b| {
//         a.display_name
//             .as_ref()
//             .unwrap()
//             .to_ascii_lowercase()
//             .cmp(&b.display_name.as_ref().unwrap().to_ascii_lowercase())
//     });

//     //region Setup Preview Images
//     let mut images: Vec<(String, Vec<u8>)> = Vec::new();
//     for item in &mods.entries {
//         if std::fs::metadata(item.image.as_ref().unwrap()).is_ok() {
//             images.push((
//                 format!("img/{}", item.id.unwrap().to_string()),
//                 std::fs::read(item.image.as_ref().unwrap()).unwrap(),
//             ));
//         };
//     }

//     let img_cache = "sd:/atmosphere/contents/01006A800016E000/manual_html/html-document/contents.htdocs/img";

//     if std::fs::metadata(&img_cache).is_ok(){
//         std::fs::remove_dir_all(&img_cache);
//     };

//     std::fs::create_dir_all(&img_cache);
//     //endregion

//     let mut file = std::fs::File::open("sd:/atmosphere/contents/01006A800016E000/manual_html/html-document/contents.htdocs/arcropolis/resources/templates/arcadia.html").unwrap();
//     let mut page_content: String = String::new();
//     file.read_to_string(&mut page_content).unwrap();

//     let tpl = ramhorns::Template::new(page_content).unwrap();

//     let render = tpl.render(&mods);

//     let response = Webpage::new()
//         .htdocs_dir("contents")
//         .file("index.html", &render)
//         .files(&images)
//         .background(skyline_web::Background::Default)
//         .boot_display(skyline_web::BootDisplay::Default)
//         .open()
//         .unwrap();

//     match response.get_last_url().unwrap() {
//         "http://localhost/" => (),
//         url => {
//             let res = format!(
//                 "{}",
//                 percent_decode_str(&url[LOCALHOST.len()..])
//                     .decode_utf8_lossy()
//                     .into_owned()
//                     .to_string()
//                     .replace("BREAKTHISLINE", "\n")
//             );

//             let webpage_res: ModStatues = toml::from_str(&res).unwrap();

//             let mut id = 0;
//             for disabled in webpage_res.is_disabled {
//                 let folder_name = &mods.entries[id as usize].folder_name.as_ref().unwrap();

//                 let enabled_path = Path::new(&workspace).join(&folder_name);
//                 let disabled_path = Path::new(&workspace).join(&format!(".{}", &folder_name));

//                 info!(
//                     "[menus::show_arcadia] ID: {}\nMod Name: {}\nIs Disabled?: {}",
//                     id, folder_name, disabled
//                 );

//                 if disabled {
//                     if std::fs::metadata(&enabled_path).is_ok() {
//                         info!("[menus::show_arcadia] Disabling {}", folder_name);
//                         info!("[menus::show_arcadia] RenameFolder Result: {:?}", RenameFolder(&enabled_path, &disabled_path));
//                     }
//                 } else {
//                     if std::fs::metadata(&disabled_path).is_ok() {
//                         info!("[menus::show_arcadia] Enabling {}", folder_name);
//                         info!(
//                             "[menus::show_arcadia] RenameFolder Result: {:?}",
//                             RenameFolder(&disabled_path, &enabled_path)
//                         );
//                     }
//                 }
//                 id += 1;
//                 info!("[menus::show_arcadia] ---------------------------");
//             }
//         }
//     }
// }