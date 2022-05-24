

use owo_colors::OwoColorize;
use smash_arc::{LoadedSearchSection, *};

use super::*;
use crate::{hashes, replacement::SearchEx, resource};

fn handle_get_folder_path_index(search: &LoadedSearchSection, mut args: Vec<String>) -> String {
    if let Some(index) = get_flag_and_option("-i", &mut args) {
        if let Some(index) = parse_index(index.as_str()) {
            if let Some(index) = search.get_folder_path_to_index().get(index as usize) {
                format!("{:#x?}", index)
            } else {
                String::from("Out of bounds")
            }
        } else {
            String::from("")
        }
    } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
        let hash = parse_hash(hash.as_str());
        match search.get_folder_path_index_from_hash(hash) {
            Ok(index) => format!("{:#x?}", index),
            Err(err) => format!("{:?}", err),
        }
    } else {
        String::from("")
    }
}

fn handle_get_folder_path(search: &LoadedSearchSection, mut args: Vec<String>) -> String {
    if let Some(index) = get_flag_and_option("-i", &mut args) {
        if let Some(index) = parse_index(index.as_str()) {
            if let Some(index) = search.get_folder_path_list().get(index as usize) {
                format!("{:#x?}", index)
            } else {
                String::from("Out of bounds")
            }
        } else {
            String::from("")
        }
    } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
        let hash = parse_hash(hash.as_str());
        match search.get_folder_path_entry_from_hash(hash) {
            Ok(index) => format!("{:#x?}", index),
            Err(err) => format!("{:?}", err),
        }
    } else {
        String::from("")
    }
}

fn handle_get_path_index(search: &LoadedSearchSection, mut args: Vec<String>) -> String {
    if let Some(index) = get_flag_and_option("-i", &mut args) {
        if let Some(index) = parse_index(index.as_str()) {
            if let Some(index) = search.get_path_to_index().get(index as usize) {
                format!("{:#x?}", index)
            } else {
                String::from("Out of bounds")
            }
        } else {
            String::from("")
        }
    } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
        let hash = parse_hash(hash.as_str());
        match search.get_path_index_from_hash(hash) {
            Ok(index) => format!("{:#x?}", index),
            Err(err) => format!("{:?}", err),
        }
    } else {
        String::from("")
    }
}

fn handle_get_path_entry_index(search: &LoadedSearchSection, mut args: Vec<String>) -> String {
    if let Some(index) = get_flag_and_option("-i", &mut args) {
        if let Some(index) = parse_index(index.as_str()) {
            if let Some(index) = search.get_path_list_indices().get(index as usize) {
                format!("{:#x}", index)
            } else {
                String::from("Out of bounds")
            }
        } else {
            String::from("")
        }
    } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
        let hash = parse_hash(hash.as_str());
        match search.get_path_list_index_from_hash(hash) {
            Ok(index) => format!("{:#x}", index),
            Err(err) => format!("{:?}", err),
        }
    } else {
        String::from("")
    }
}

fn handle_get_path(search: &LoadedSearchSection, mut args: Vec<String>) -> String {
    if let Some(index) = get_flag_and_option("-i", &mut args) {
        if let Some(index) = parse_index(index.as_str()) {
            if let Some(index) = search.get_path_list().get(index as usize) {
                format!("{:#x?}", index)
            } else {
                String::from("Out of bounds")
            }
        } else {
            String::from("")
        }
    } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
        let hash = parse_hash(hash.as_str());
        match search.get_path_list_entry_from_hash(hash) {
            Ok(index) => format!("{:#x?}", index),
            Err(err) => format!("{:?}", err),
        }
    } else {
        String::from("")
    }
}

fn handle_walk_directory(search: &LoadedSearchSection, mut args: Vec<String>) -> String {
    
    fn write_indent(indent: usize) {
        for _ in 0..indent {
            print!("-");
        }
    }
    let hash = if let Some(hash) = get_flag_and_option("-h", &mut args) {
        parse_hash(hash.as_str())
    } else {
        return String::from("")
    };

    let is_pretty = check_for_flag("-p", &mut args);
    let min_depth = parse_index(get_flag_and_option("--min-depth", &mut args).unwrap_or(String::from("0")).as_str()).unwrap_or(0) as usize;
    let max_depth = parse_index(
        get_flag_and_option("--max-depth", &mut args)
            .unwrap_or(String::from("0xFFFFFFFF"))
            .as_str(),
    )
    .unwrap_or(0xFFFFFFFF) as usize;

    let _ = search.walk_directory(hash, |child, depth| {
        if !(min_depth..max_depth + 1).contains(&depth) {
            return
        }
        write_indent(depth - min_depth);
        print!("-| ");
        use crate::replacement::extensions::DirectoryChild;
        match child {
            DirectoryChild::File(file) => {
                if is_pretty {
                    println!("{} ({:#x})", hashes::find(file.path.hash40()).bright_green(), file.path.hash40().0);
                } else {
                    println!("File: {} ({:#x})", hashes::find(file.path.hash40()), file.path.hash40().0);
                }
            },
            DirectoryChild::Folder(folder) => {
                if is_pretty {
                    println!("{} ({:#x})", hashes::find(folder.path.hash40()).bright_yellow(), folder.path.hash40().0);
                } else {
                    println!("Folder: {} ({:#x})", hashes::find(folder.path.hash40()), folder.path.hash40().0);
                }
            },
        }
    });

    String::from("Check the skyline logger")
}

pub fn handle_command(mut args: Vec<String>) -> String {
    let search = resource::search();
    if args.is_empty() {
        return String::from("")
    }
    let command = args.remove(0);
    match command.as_str() {
        "get_folder_path_index" => handle_get_folder_path_index(search, args),
        "get_folder_path" => handle_get_folder_path(search, args),
        "get_path_index" => handle_get_path_index(search, args),
        "get_path_entry_index" => handle_get_path_entry_index(search, args),
        "get_path" => handle_get_path(search, args),
        "walk_directory" => handle_walk_directory(search, args),
        _ => String::from(""),
    }
}
