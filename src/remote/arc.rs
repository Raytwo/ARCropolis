//use crate::resource;

use crate::resource;

static USAGE: &'static str = r#"arc: Commands to help scan data.arc tables/entries at runtime.
Directory Lookups:
    get_directory [-i | -h] [<index> | <hashable>]
    get_directory_file_group [-i | -h] [<index> | <hashable>]
        Note: The index/hashable item in this command are for directories, not file groups
    get_directory_redirect [-i | -h] [<index> | <hashable>]
        Note: The index/hashable item in this command are for directories, not file groups
              This can either be a DirInfo or a DirectoryOffset
    get_file_group <index>

File Lookups:
    get_file_path [-i | -h] [<index> | <hashable>]
    get_file_info_index [-i | -h] [<index> | <hashable>]
    get_file_info [-i | -h] [<index> | <hashable>]
    get_file_info_to_data [-i | -h] [<index> | <hashable>] [-r <region>]
        Note: The region is only used when passing the hashable item and is assumed to be Region::None by default
    get_file_data [-i | -h] [-i | -h] [<index> | <hashable>] [-r <region>]
        Note: The region is only used when passing the hashable item and is assumed to be Region::None by default
    
Utilities:
    walk_directory [-i | -h] [<index> | <hashable>] [-r] [-p]
        Note: The index/hashable item in this command are for directories, not file groups.
              Pass -r if you want to pass recursively.
              Pass -p if you want to pretty print with colors.
    is_file_shared [-i | -h] [<index> | <hashable>]
        Note: If passing by index, pass the file path index
    get_shared_file [-i | -h] [<index> | <hashable>] [-r]
        Note: If passing by index, pass the file path index
              Pass -r if you want to get the lowest level shared file
    get_shared_data_index
        Gets the first file data index that is shared."#;

mod directory {
    use smash_arc::{ArcLookup, LoadedArc};

    use super::super::*;

    pub static USAGE: &'static str = r#"Directory Lookups:
    get_directory [-i | -h] [<index> | <hashable>]
    get_directory_file_group [-i | -h] [<index> | <hashable>]
        Note: The index/hashable item in this command are for directories, not file groups
    get_directory_redirect [-i | -h] [<index> | <hashable>]
        Note: The index/hashable item in this command are for directories, not file groups
              This can either be a DirInfo or a DirectoryOffset
    get_file_group <index>"#;

    pub fn handle_get_directory(arc: &LoadedArc, mut args: Vec<String>) -> String {
        static USAGE: &'static str = "Usage: get_directory [-i | -h] [<index> | <hashable>]";
        if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(idx) => {
                    match arc.get_dir_infos().get(idx as usize) {
                        Some(info) => {
                            format!("{:#x?}", info)
                        },
                        None => String::from("Out of bounds"),
                    }
                },
                None => String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            match arc.get_dir_info_from_hash(hash) {
                Ok(info) => {
                    format!("{:#x?}", info)
                },
                Err(e) => {
                    format!("{:#?}", e)
                },
            }
        } else {
            String::from(USAGE)
        }
    }

    pub fn handle_get_directory_file_group(arc: &LoadedArc, mut args: Vec<String>) -> String {
        static USAGE: &'static str = "get_directory_file_group [-i | -h] [<index> | <hashable>]\n    Note: The index/hashable item in this command are for directories, not file groups";
        if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(idx) => {
                    match arc.get_dir_infos().get(idx as usize) {
                        Some(info) => {
                            if info.path.index() != 0xFF_FFFF {
                                match arc.get_folder_offsets().get(info.path.index() as usize) {
                                    Some(folder) => {
                                        format!("{:#x?}", folder)
                                    },
                                    None => String::from("DirInfo file group index is out of bounds"),
                                }
                            } else {
                                String::from("DirInfo does not have associated file group")
                            }
                        },
                        None => String::from("Out of bounds"),
                    }
                },
                None => String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            match arc.get_dir_info_from_hash(hash) {
                Ok(info) => {
                    if info.path.index() != 0xFF_FFFF {
                        match arc.get_folder_offsets().get(info.path.index() as usize) {
                            Some(folder) => {
                                format!("{:#x?}", folder)
                            },
                            None => String::from("DirInfo file group index is out of bounds"),
                        }
                    } else {
                        String::from("DirInfo does not have associated file group")
                    }
                },
                Err(e) => {
                    format!("{:#?}", e)
                },
            }
        } else {
            String::from(USAGE)
        }
    }

    pub fn handle_get_directory_redirect(arc: &LoadedArc, mut args: Vec<String>) -> String {
        use smash_arc::RedirectionType;
        static USAGE: &'static str = "get_directory_redirect [-i | -h] [<index> | <hashable>]\n    Note: The index/hashable item in this command are for directories, not file groups\n          This can either be a DirInfo or a DirectoryOffset";
        if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(idx) => {
                    match arc.get_dir_infos().get(idx as usize) {
                        Some(info) => {
                            if info.flags.redirected() {
                                match arc.get_directory_dependency(info) {
                                    Some(RedirectionType::Symlink(redirection)) => {
                                        format!("{:#x?}", redirection)
                                    },
                                    Some(RedirectionType::Shared(redirection)) => {
                                        format!("{:#x?}", redirection)
                                    },
                                    None => String::from("Directory redirection index is in valid"),
                                }
                            } else {
                                String::from("Directory does not redirect")
                            }
                        },
                        None => String::from("Out of bounds"),
                    }
                },
                None => String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            match arc.get_dir_info_from_hash(hash) {
                Ok(info) => {
                    if info.path.index() != 0xFF_FFFF {
                        match arc.get_folder_offsets().get(info.path.index() as usize) {
                            Some(folder) => {
                                format!("{:#x?}", folder)
                            },
                            None => String::from("DirInfo file group index is out of bounds"),
                        }
                    } else {
                        String::from("DirInfo does not have associated file group")
                    }
                },
                Err(e) => {
                    format!("{:#?}", e)
                },
            }
        } else {
            String::from(USAGE)
        }
    }

    pub fn handle_get_file_group(arc: &LoadedArc, args: Vec<String>) -> String {
        static USAGE: &'static str = "get_file_group <index>";
        for arg in args.into_iter() {
            if let Some(idx) = parse_index(arg.as_str()) {
                if let Some(file_group) = arc.get_folder_offsets().get(idx as usize) {
                    return format!("{:#x?}", file_group)
                } else {
                    return String::from("Out of bounds")
                }
            } else {
                return String::from(USAGE)
            }
        }
        String::from(USAGE)
    }
}

mod files {
    use smash_arc::{ArcLookup, LoadedArc, Region};

    use super::super::*;
    pub static USAGE: &'static str = r#"File Lookups:
    get_file_path [-i | -h] [<index> | <hashable>]
    get_file_info_index [-i | -h] [<index> | <hashable>]
    get_file_info [-i | -h] [<index> | <hashable>]
    get_file_info_to_data [-i | -h] [<index> | <hashable>] [-r <region>]
        Note: The region is only used when passing the hashable item and is assumed to be Region::None by default
    get_file_data [-i | -h] [-i | -h] [<index> | <hashable>] [-r <region>]
        Note: The region is only used when passing the hashable item and is assumed to be Region::None by default
    "#;
    pub fn handle_get_file_path(arc: &LoadedArc, mut args: Vec<String>) -> String {
        static USAGE: &'static str = "get_file_path [-i | -h] [<index> | <hashable>]";
        if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(val) => {
                    match arc.get_file_paths().get(val as usize) {
                        Some(path) => {
                            format!("{:#x?}", path)
                        },
                        None => String::from("Out of bounds"),
                    }
                },
                None => String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            match arc.get_file_path_index_from_hash(hash) {
                Ok(idx) => {
                    match arc.get_file_paths().get(usize::from(idx)) {
                        Some(path) => {
                            format!("{:#x?}", path)
                        },
                        None => String::from("File path index is out of bounds"),
                    }
                },
                Err(e) => {
                    format!("{:#x?}", e)
                },
            }
        } else {
            String::from(USAGE)
        }
    }

    pub fn handle_get_file_info_index(arc: &LoadedArc, mut args: Vec<String>) -> String {
        static USAGE: &'static str = "get_file_info_index [-i | -h] [<index> | <hashable>]";
        if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(val) => {
                    match arc.get_file_info_indices().get(val as usize) {
                        Some(idx) => {
                            format!("{:#x?}", idx)
                        },
                        None => String::from("Out of bounds"),
                    }
                },
                None => String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            match arc.get_file_path_index_from_hash(hash) {
                Ok(idx) => {
                    match arc.get_file_paths().get(usize::from(idx)) {
                        Some(path) => {
                            match arc.get_file_info_indices().get(path.path.index() as usize) {
                                Some(idx) => {
                                    format!("{:#x?}", idx)
                                },
                                None => String::from("FileInfoIndex index is out of bounds"),
                            }
                        },
                        None => String::from("File path index is out of bounds"),
                    }
                },
                Err(e) => {
                    format!("{:#x?}", e)
                },
            }
        } else {
            String::from(USAGE)
        }
    }

    pub fn handle_get_file_info(arc: &LoadedArc, mut args: Vec<String>) -> String {
        static USAGE: &'static str = "get_file_info [-i | -h] [<index> | <hashable>]";
        if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(val) => {
                    match arc.get_file_infos().get(val as usize) {
                        Some(info) => {
                            format!("{:#x?}", info)
                        },
                        None => String::from("Out of bounds"),
                    }
                },
                None => String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            match arc.get_file_info_from_hash(hash) {
                Ok(info) => {
                    format!("{:#x?}", info)
                },
                Err(e) => {
                    format!("{:#x?}", e)
                },
            }
        } else {
            String::from(USAGE)
        }
    }

    pub fn handle_get_file_info_to_data(arc: &LoadedArc, mut args: Vec<String>) -> String {
        static USAGE: &'static str = "get_file_info_to_data [-i | -h] [<index> | <hashable>] [-r <region>]\n    Note: The region is only used when passing the hashable item and is assumed to be Region::None by default";
        if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(idx) => {
                    match arc.get_file_info_to_datas().get(idx as usize) {
                        Some(info) => {
                            format!("{:#x?}", info)
                        },
                        None => String::from("Out of bounds"),
                    }
                },
                None => String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            let region = match get_flag_and_option("-r", &mut args) {
                Some(region) => {
                    if region == "none" {
                        Region::None
                    } else {
                        Region::None
                    }
                },
                None => Region::None,
            };
            match arc.get_file_info_from_hash(hash) {
                Ok(info) => {
                    format!("{:#x?}", arc.get_file_in_folder(info, region))
                },
                Err(e) => {
                    format!("{:#?}", e)
                },
            }
        } else {
            String::from(USAGE)
        }
    }

    pub fn handle_get_file_data(arc: &LoadedArc, mut args: Vec<String>) -> String {
        static USAGE: &'static str = "get_file_data [-i | -h] [-i | -h] [<index> | <hashable>] [-r <region>]\n    Note: The region is only used when passing the hashable item and is assumed to be Region::None by default";
        if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(idx) => {
                    match arc.get_file_datas().get(idx as usize) {
                        Some(data) => {
                            format!("{:#x?}", data)
                        },
                        None => String::from("Out of bounds"),
                    }
                },
                None => String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            let region = match get_flag_and_option("-r", &mut args) {
                Some(region) => {
                    if region == "none" {
                        Region::None
                    } else {
                        Region::None
                    }
                },
                None => Region::None,
            };
            match arc.get_file_info_from_hash(hash) {
                Ok(info) => {
                    format!("{:#x?}", arc.get_file_data(info, region))
                },
                Err(e) => {
                    format!("{:#?}", e)
                },
            }
        } else {
            String::from(USAGE)
        }
    }
}

mod utils {
    use smash_arc::{ArcLookup, DirInfo, FilePathIdx, LoadedArc, RedirectionType, Region};

    use super::super::*;
    use crate::{hashes, replacement::LoadedArcEx};

    pub static USAGE: &'static str = r#"Utilities:
    walk_directory [-i | -h] [<index> | <hashable>] [-r] [-p] [-s]
        Note: The index/hashable item in this command are for directories, not file groups.
              Pass -r if you want to walk recursively.
              Pass -p if you want to pretty print with colors.
              Pass -s if you want to see which files are shared
    is_file_shared [-i | -h] [<index> | <hashable>]
        Note: If passing by index, pass the file path index
    get_shared_file [-i | -h] [<index> | <hashable>] [-r]
        Note: If passing by index, pass the file path index
              Pass -r if you want to get the lowest level shared file
    get_shared_data_index
        Gets the first file data index that is shared."#;

    fn walk_directory(
        arc: &LoadedArc,
        info: &DirInfo,
        pretty: bool,
        recursive: bool,
        shared: bool,
        current: Option<String>,
        indent: usize,
    ) -> String {
        use std::fmt::Write;
        fn write_indent(output: &mut String, indent: usize) {
            for _ in 0..indent {
                let _ = write!(output, "-");
            }
        }

        use owo_colors::OwoColorize;

        let mut output = if let Some(current) = current {
            current
        } else {
            if pretty {
                format!("'{}' ({:#x}):\n", hashes::find(info.path.hash40()).bright_blue(), info.path.hash40().0)
            } else {
                format!("'{}' ({:#x}):\n", hashes::find(info.path.hash40()), info.path.hash40().0)
            }
        };

        // print out files
        let file_paths = arc.get_file_paths();
        let file_infos = arc.get_file_infos();
        let shared_index = arc.get_shared_data_index();
        for x in info.file_info_range() {
            let shared_str = if !shared {
                "".to_string()
            } else {
                let shared_file_path = arc
                    .get_shared_file(file_paths[file_infos[x].file_path_index].path.hash40())
                    .unwrap_or(FilePathIdx(0xFF_FFFF));
                if shared_file_path.0 == 0xFF_FFFF {
                    "Invalid".to_string()
                } else if shared_file_path == file_infos[x].file_path_index {
                    "Unshared".to_string()
                } else {
                    let hash = file_paths[shared_file_path].path.hash40();
                    format!("Shared with '{}' ({:#x})", hashes::find(hash), hash.0)
                }
            };
            write_indent(&mut output, indent + 1);
            let hash = file_paths[file_infos[x].file_path_index].path.hash40();
            if pretty {
                let _ = write!(&mut output, "| '{}' ({:#x})", hashes::find(hash).bright_red(), hash.0);
            } else {
                let _ = write!(&mut output, "| '{}' ({:#x})", hashes::find(hash), hash.0);
            }
            if shared {
                let _ = write!(&mut output, ": {}\n", shared_str);
            } else {
                let _ = write!(&mut output, "\n");
            }
        }

        if recursive {
            match arc.get_directory_dependency(info) {
                Some(RedirectionType::Symlink(dir_info)) => {
                    write_indent(&mut output, indent + 1);
                    let hash = dir_info.path.hash40();
                    if pretty {
                        let _ = write!(&mut output, "| '{}' ({:#x}):\n", hashes::find(hash).bright_green(), hash.0);
                    } else {
                        let _ = write!(&mut output, "| Redirect: '{}' ({:#x}):\n", hashes::find(hash), hash.0);
                    }
                    output = walk_directory(arc, &dir_info, pretty, recursive, shared, Some(output), indent + 1);
                },
                Some(RedirectionType::Shared(folder)) => {
                    write_indent(&mut output, indent + 1);
                    if pretty {
                        let _ = write!(&mut output, "| {}:\n", "Unnamed".bright_green());
                    } else {
                        let _ = write!(&mut output, "| Redirect: Unnamed:\n");
                    }
                    for x in folder.range() {
                        let shared_str = if !shared {
                            "".to_string()
                        } else {
                            let shared_file_path = arc
                                .get_shared_file(file_paths[file_infos[x].file_path_index].path.hash40())
                                .unwrap_or(FilePathIdx(0xFF_FFFF));
                            if shared_file_path == file_infos[x].file_path_index {
                                "Unshared".to_string()
                            } else {
                                let hash = file_paths[shared_file_path].path.hash40();
                                format!("Shared with '{}' ({:#x})", hashes::find(hash), hash.0)
                            }
                        };
                        write_indent(&mut output, indent + 2);
                        let hash = file_paths[file_infos[x].file_path_index].path.hash40();
                        if pretty {
                            let _ = write!(&mut output, "| '{}' ({:#x})", hashes::find(hash).bright_red(), hash.0);
                        } else {
                            let _ = write!(&mut output, "| '{}' ({:#x})", hashes::find(hash), hash.0);
                        }
                        if shared {
                            let _ = write!(&mut output, ": {}\n", shared_str);
                        } else {
                            let _ = write!(&mut output, "\n");
                        }
                    }
                },
                None => {},
            }

            let child_dir_index = unsafe { std::slice::from_raw_parts(arc.folder_child_hashes, (*arc.fs_header).folder_count as usize) };
            for x in info.children_range() {
                let dir_index = child_dir_index[x].index();
                let child_info = &arc.get_dir_infos()[dir_index as usize];
                let hash = child_info.path.hash40();
                write_indent(&mut output, indent + 1);
                if pretty {
                    let _ = write!(&mut output, "| '{}' ({:#x}):\n", hashes::find(hash).bright_yellow(), hash.0);
                } else {
                    let _ = write!(&mut output, "| Child: '{}' ({:#x}):\n", hashes::find(hash), hash.0);
                }
                output = walk_directory(arc, child_info, pretty, recursive, shared, Some(output), indent + 1);
            }
        }
        output
    }

    pub fn handle_walk_directory(arc: &LoadedArc, mut args: Vec<String>) -> String {
        static USAGE: &'static str = "walk_directory [-i | -h] [<index> | <hashable>] [-r] [-p] [-s]\n    Note: The index/hashable item in this command are for directories, not file groups.\n          Pass -r if you want to walk recursively.\n          Pass -p if you want to pretty print with colors.\n          Pass -s if you want to see which files are shared";
        let dir_info = if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(idx) => {
                    match arc.get_dir_infos().get(idx as usize) {
                        Some(info) => info,
                        None => return String::from("Out of bounds"),
                    }
                },
                None => return String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            match arc.get_dir_info_from_hash(hash) {
                Ok(info) => info,
                Err(e) => return format!("{:#?}", e),
            }
        } else {
            return String::from(USAGE)
        };
        let pretty_print = check_for_flag("-p", &mut args);
        let recursive = check_for_flag("-r", &mut args);
        let shared = check_for_flag("-s", &mut args);
        walk_directory(arc, dir_info, pretty_print, recursive, shared, None, 0)
    }

    pub fn handle_is_file_shared(arc: &LoadedArc, mut args: Vec<String>) -> String {
        static USAGE: &'static str = "is_file_shared [-i | -h] [<index> | <hashable>]\n    Note: If passing by index, pass the file path index";
        if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(idx) => {
                    if (idx as usize) < arc.get_file_paths().len() {
                        let info = arc.get_file_info_from_path_index(FilePathIdx(idx));
                        let info_to_data = arc.get_file_in_folder(info, Region::None);
                        let shared_data_index = arc.get_shared_data_index();
                        if info_to_data.file_data_index.0 >= shared_data_index {
                            String::from("Yes")
                        } else {
                            String::from("No")
                        }
                    } else {
                        String::from("Out of bounds")
                    }
                },
                None => String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            match arc.get_file_info_from_hash(hash) {
                Ok(info) => {
                    let info_to_data = arc.get_file_in_folder(info, Region::None);
                    let shared_data_index = arc.get_shared_data_index();
                    if info_to_data.file_data_index.0 >= shared_data_index {
                        String::from("Yes")
                    } else {
                        String::from("No")
                    }
                },
                Err(e) => {
                    format!("{:#?}", e)
                },
            }
        } else {
            String::from(USAGE)
        }
    }

    pub fn handle_get_shared_file(arc: &LoadedArc, mut args: Vec<String>) -> String {
        static USAGE: &'static str = "get_shared_file [-i | -h] [<index> | <hashable>] [-r]\n    Note: If passing by index, pass the file path index\n      Pass -r if you want to get the lowest level shared file";
        let file_path = if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(idx) => {
                    match arc.get_file_paths().get(idx as usize) {
                        Some(path) => path,
                        None => return String::from("Out of bounds"),
                    }
                },
                None => return String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            match arc.get_file_path_index_from_hash(hash) {
                Ok(idx) => &arc.get_file_paths()[idx],
                Err(e) => return format!("{:#?}", e),
            }
        } else {
            return String::from(USAGE)
        };

        let recursive = check_for_flag("-r", &mut args);

        let file_info = arc.get_file_info_from_hash(file_path.path.hash40()).unwrap(); // Fine to unwrap here since we already know it's there
        let shared_data_index = arc.get_shared_data_index();
        let data_index = arc.get_file_in_folder(file_info, Region::None).file_data_index.0;
        if data_index >= shared_data_index || true {
            if !recursive {
                let shared_hash = arc.get_file_paths()[file_info.file_path_index].path.hash40();
                if shared_hash == file_path.path.hash40() {
                    String::from("File is the source file")
                } else {
                    format!("File is shared with '{}' ({:#x})", hashes::find(shared_hash), shared_hash.0)
                }
            } else {
                let file_paths = arc.get_file_paths();
                let mut current_file_path = &file_paths[file_info.file_path_index];
                if current_file_path.path.hash40() == file_path.path.hash40() {
                    String::from("File is the source file")
                } else {
                    loop {
                        let next_file_path = &file_paths[arc.get_file_info_from_hash(current_file_path.path.hash40()).unwrap().file_path_index];
                        if next_file_path.path.hash40() == current_file_path.path.hash40() {
                            break
                        }
                        current_file_path = next_file_path;
                    }
                    format!(
                        "File is shared with '{}' ({:#x})",
                        hashes::find(current_file_path.path.hash40()),
                        current_file_path.path.hash40().0
                    )
                }
            }
        } else {
            String::from("File is not shared")
        }
    }

    pub fn handle_get_shared_data_index(arc: &LoadedArc) -> String {
        format!("{:#x}", arc.get_shared_data_index())
    }
}

pub fn handle_command(mut args: Vec<String>) -> String {
    let arc = resource::arc();
    if args.len() == 0 {
        return String::from(USAGE)
    }
    let command = args.remove(0);
    match command.as_str() {
        "directory_help" => String::from(directory::USAGE),
        "get_directory" => directory::handle_get_directory(arc, args),
        "get_directory_file_group" => directory::handle_get_directory_file_group(arc, args),
        "get_directory_redirect" => directory::handle_get_directory_redirect(arc, args),
        "get_file_group" => directory::handle_get_file_group(arc, args),
        "file_help" => String::from(files::USAGE),
        "get_file_path" => files::handle_get_file_path(arc, args),
        "get_file_info_index" => files::handle_get_file_info_index(arc, args),
        "get_file_info" => files::handle_get_file_info(arc, args),
        "get_file_info_to_data" => files::handle_get_file_info_to_data(arc, args),
        "get_file_data" => files::handle_get_file_data(arc, args),
        "util_help" => String::from(utils::USAGE),
        "walk_directory" => utils::handle_walk_directory(arc, args),
        "is_file_shared" => utils::handle_is_file_shared(arc, args),
        "get_shared_file" => utils::handle_get_shared_file(arc, args),
        "get_shared_data_index" => utils::handle_get_shared_data_index(arc),
        _ => String::from(USAGE),
    }
}
