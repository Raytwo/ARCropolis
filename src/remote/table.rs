use crate::resource::{self};

static USAGE: &str = r#"table: Commands to help scan the filesystem tables for analysis/troubleshooting at runtime.
Entry Lookups:
    get_filepath_table_entry [-i | -h] [<index> | <hashable>]
        Note: This is commonly referred to as Table 1
              This table uses the same indices as the FilePath table in data.arc
    get_loaded_data_table_entry [-i | -h] [<index> | <hashable>]
        Note: This is commonly referred to as Table 2
              When passing an index, make sure it is not a file path index. This table uses the same indices as the FileInfoIndex table in data.arc
    get_loaded_directory_table_entry [-i | -h] [<index> | <hashable>]
        Note: When passing a hashable, use the directory path hash. Directories in data.arc are formatted differently than you might expect.
              For example, when getting Joker's first slot, you will use the directory path of "figher/jack/c00"
              This table uses the same indices as the DirInfo table in data.arc

Utilities:
    check_directory [-i | -h] [<index> | <hashable>] [-r] [-p] [-u] [--ref-count | --pointer]
        Note: Both hashables and indices are for directories.
              Pass -r to recursively enter redirected directories (child directories are done by default)
              Pass -p for pretty printing with colors
              Pass -u to only print unloaded file infos
              Pass --ref-count or --pointer to only see those fields of the loaded data table entries for each file (the default is the loaded state)"#;

mod lookups {
    use smash_arc::ArcLookup;

    use super::super::*;
    use crate::{
        config,
        resource::{self, FilesystemInfo},
    };
    pub fn handle_get_filepath_table_entry(tables: &FilesystemInfo, mut args: Vec<String>) -> String {
        static USAGE: &str = r#"get_filepath_table_entry [-i | -h] [<index> | <hashable>]
        Note: This is commonly referred to as Table 1
              This table uses the same indices as the FilePath table in data.arc"#;

        if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(idx) => {
                    match tables.get_loaded_filepaths().get(idx as usize) {
                        Some(entry) => {
                            format!("{:#x?}", entry)
                        },
                        None => String::from("Out of bounds"),
                    }
                },
                None => String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            let arc = resource::arc();
            match arc.get_file_path_index_from_hash(hash) {
                Ok(idx) => {
                    match tables.get_loaded_filepaths().get(usize::from(idx)) {
                        Some(entry) => {
                            format!("{:#x?}", entry)
                        },
                        None => String::from("Out of bounds"),
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

    pub fn handle_get_loaded_data_table_entry(tables: &FilesystemInfo, mut args: Vec<String>) -> String {
        static USAGE: &str = r#"get_loaded_data_table_entry [-i | -h] [<index> | <hashable>]
            Note: This is commonly referred to as Table 2
                  When passing an index, make sure it is not a file path index. This table uses the same indices as the FileInfoIndex table in data.arc"#;
        if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(idx) => {
                    match tables.get_loaded_datas().get(idx as usize) {
                        Some(entry) => {
                            format!("{:#x?}", entry)
                        },
                        None => String::from("Out of bounds"),
                    }
                },
                None => String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            let arc = resource::arc();
            match arc.get_file_path_index_from_hash(hash) {
                Ok(idx) => {
                    let info_idx_idx = arc.get_file_paths()[idx].path.index() as usize;
                    match tables.get_loaded_datas().get(info_idx_idx) {
                        Some(entry) => {
                            if check_for_flag("-d", &mut args) && !entry.data.is_null() {
                                let decomp_size = arc
                                    .get_file_data(
                                        &arc.get_file_infos()[arc.get_file_info_indices()[info_idx_idx].file_info_index],
                                        config::region(),
                                    )
                                    .decomp_size;
                                let slice = unsafe { std::slice::from_raw_parts(entry.data, decomp_size as usize) };
                                format!("{:x?}", slice)
                            } else {
                                format!("{:#x?}", entry)
                            }
                        },
                        None => String::from("File path points out of bounds of loaded data table"),
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

    pub fn handle_get_loaded_directory_table_entry(tables: &FilesystemInfo, mut args: Vec<String>) -> String {
        static USAGE: &str = r#"get_loaded_directory_table_entry [-i | -h] [<index> | <hashable>]
        Note: When passing a hashable, use the directory path hash. Directories in data.arc are formatted differently than you might expect.
              For example, when getting Joker's first slot, you will use the directory path of "figher/jack/c00"
              This table uses the same indices as the DirInfo table in data.arc"#;

        if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(idx) => {
                    match tables.get_loaded_directories().get(idx as usize) {
                        Some(entry) => {
                            format!("{:#x?}", entry)
                        },
                        None => String::from("Out of bounds"),
                    }
                },
                None => String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            let arc = resource::arc();
            // gross, but idc
            let dir_hashes = arc.get_dir_hash_to_info_index();
            let mut index = None;
            for idx in dir_hashes.iter() {
                if idx.hash40() == hash {
                    index = Some(idx.index() as usize);
                    break
                }
            }
            if let Some(idx) = index {
                match tables.get_loaded_directories().get(idx) {
                    Some(entry) => {
                        format!("{:#x?}", entry)
                    },
                    None => String::from("DirInfo HashToIndex is out of bounds"),
                }
            } else {
                String::from("Missing")
            }
        } else {
            String::from(USAGE)
        }
    }
}

mod utils {
    use smash_arc::ArcLookup;

    use super::super::*;
    use crate::{
        hashes,
        resource::{self, FilesystemInfo, LoadState},
    };

    #[derive(Copy, Clone)]
    enum CheckInfoType {
        State,
        RefCount,
        Pointer,
    }

    fn check_directory(
        tables: &FilesystemInfo,
        index: u32,
        data_idx: bool,
        pretty: bool,
        unloaded: bool,
        info_type: CheckInfoType,
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
        let arc = resource::arc();
        let dir_infos = arc.get_dir_infos();
        let child_dir_to_index = unsafe { std::slice::from_raw_parts(arc.folder_child_hashes, (*arc.fs_header).folder_count as usize) };
        let file_paths = arc.get_file_paths();
        let table1 = tables.get_loaded_filepaths();
        let table2 = tables.get_loaded_datas();
        let directories = tables.get_loaded_directories();

        let loaded_dir = &directories[index as usize];
        let dir_info = dir_infos.get(index as usize);
        let mut output = if let Some(current) = current {
            current
        } else if let Some(dir_info) = dir_info {
            if pretty {
                format!(
                    "'{}' ({:#x}):\n",
                    hashes::find(dir_info.path.hash40()).bright_blue(),
                    dir_info.path.hash40().0
                )
            } else {
                format!("'{}' ({:#x}):\n", hashes::find(dir_info.path.hash40()), dir_info.path.hash40().0)
            }
        } else if pretty {
            format!("LoadedDirectory #{}:\n", format!("{:#x}", index).bright_blue())
        } else {
            format!("LoadedDirectory #{:#x}:\n", index)
        };

        write_indent(&mut output, indent + 1);
        if let Some(dir_info) = dir_info {
            let _ = writeln!(
                &mut output,
                "| Files in vector: {} / {}",
                loaded_dir.child_path_indices.len(),
                dir_info.file_count
            );
        } else {
            let _ = writeln!(&mut output, "| Files in vector: {} / ?", loaded_dir.child_path_indices.len());
        };

        for path_idx in loaded_dir.child_path_indices.iter() {
            let t2_entry = {
                if data_idx || table1[*path_idx as usize].is_loaded == 0 {
                    None
                } else {
                    table2.get(table1[*path_idx as usize].loaded_data_index as usize)
                }
            };
            if let Some(entry) = t2_entry.as_ref() {
                if entry.state == LoadState::Loaded && unloaded {
                    continue
                }
            }
            let hash = file_paths[*path_idx as usize].path.hash40();
            write_indent(&mut output, indent + 2);
            if pretty {
                match info_type {
                    CheckInfoType::State => {
                        if let Some(entry) = t2_entry {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {:?}",
                                hashes::find(hash).bright_red(),
                                hash.0,
                                entry.state
                            );
                        } else if data_idx {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {:#x}",
                                hashes::find(hash).bright_red(),
                                hash.0,
                                table1[*path_idx as usize].loaded_data_index
                            );
                        } else {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {}",
                                hashes::find(hash).bright_red(),
                                hash.0,
                                "Error! Invalid filepath table state!".red()
                            );
                        }
                    },
                    CheckInfoType::RefCount => {
                        if let Some(entry) = t2_entry {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {}",
                                hashes::find(hash).bright_red(),
                                hash.0,
                                entry.ref_count.load(std::sync::atomic::Ordering::SeqCst)
                            );
                        } else if data_idx {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {:#x}",
                                hashes::find(hash).bright_red(),
                                hash.0,
                                table1[*path_idx as usize].loaded_data_index
                            );
                        } else {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {}",
                                hashes::find(hash).bright_red(),
                                hash.0,
                                "Error! Invalid filepath table state!".red()
                            );
                        }
                    },
                    CheckInfoType::Pointer => {
                        if let Some(entry) = t2_entry {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {:x}",
                                hashes::find(hash).bright_red(),
                                hash.0,
                                entry.data as u64
                            );
                        } else if data_idx {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {:#x}",
                                hashes::find(hash).bright_red(),
                                hash.0,
                                table1[*path_idx as usize].loaded_data_index
                            );
                        } else {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {}",
                                hashes::find(hash).bright_red(),
                                hash.0,
                                "Error! Invalid filepath table state!".red()
                            );
                        }
                    },
                }
            } else {
                match info_type {
                    CheckInfoType::State => {
                        if let Some(entry) = t2_entry {
                            let _ = writeln!(&mut output, "| '{}' ({:#x}): {:?}", hashes::find(hash), hash.0, entry.state);
                        } else if data_idx {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {:#x}",
                                hashes::find(hash),
                                hash.0,
                                table1[*path_idx as usize].loaded_data_index
                            );
                        } else {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {}",
                                hashes::find(hash),
                                hash.0,
                                "Error! Invalid filepath table state!".red()
                            );
                        }
                    },
                    CheckInfoType::RefCount => {
                        if let Some(entry) = t2_entry {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {}",
                                hashes::find(hash),
                                hash.0,
                                entry.ref_count.load(std::sync::atomic::Ordering::SeqCst)
                            );
                        } else if data_idx {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {:#x}",
                                hashes::find(hash),
                                hash.0,
                                table1[*path_idx as usize].loaded_data_index
                            );
                        } else {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {}",
                                hashes::find(hash),
                                hash.0,
                                "Error! Invalid filepath table state!".red()
                            );
                        }
                    },
                    CheckInfoType::Pointer => {
                        if let Some(entry) = t2_entry {
                            let _ = writeln!(&mut output, "| '{}' ({:#x}): {:x}", hashes::find(hash), hash.0, entry.data as u64);
                        } else if data_idx {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {:#x}",
                                hashes::find(hash),
                                hash.0,
                                table1[*path_idx as usize].loaded_data_index
                            );
                        } else {
                            let _ = writeln!(
                                &mut output,
                                "| '{}' ({:#x}): {}",
                                hashes::find(hash),
                                hash.0,
                                "Error! Invalid filepath table state!".red()
                            );
                        }
                    },
                }
            }
        }
        if let Some(dir_info) = dir_info {
            for idx in dir_info.children_range() {
                let index = child_dir_to_index[idx].index() as u32;
                let child_dir_info = &dir_infos[index as usize];
                write_indent(&mut output, indent + 1);
                if pretty {
                    let _ = writeln!(
                        &mut output,
                        "| '{}' ({:#x}): {:?}",
                        hashes::find(child_dir_info.path.hash40()).bright_yellow(),
                        child_dir_info.path.hash40().0,
                        directories[index as usize].state
                    );
                } else {
                    let _ = writeln!(
                        &mut output,
                        "| Child: '{}' ({:#x}): {:?}",
                        hashes::find(child_dir_info.path.hash40()),
                        child_dir_info.path.hash40().0,
                        directories[index as usize].state
                    );
                }
                output = check_directory(tables, index, data_idx, pretty, unloaded, info_type, Some(output), indent + 1);
            }
        }
        output
    }

    pub fn handle_check_directory(tables: &FilesystemInfo, mut args: Vec<String>) -> String {
        static USAGE: &str = r#"check_directory [-i | -h] [<index> | <hashable>] [-p] [-u] [--ref-count | --pointer]
            Note: Both hashables and indices are for directories.
                  Pass -p for pretty printing with colors
                  Pass -u to only print unloaded file infos
                  Pass --ref-count or --pointer to only see those fields of the loaded data table entries for each file (the default is the loaded state)"#;
        let index = if let Some(idx) = get_flag_and_option("-i", &mut args) {
            match parse_index(idx.as_str()) {
                Some(idx) => idx,
                None => return String::from(USAGE),
            }
        } else if let Some(hash) = get_flag_and_option("-h", &mut args) {
            let hash = parse_hash(hash.as_str());
            let arc = resource::arc();
            // gross, but idc
            let dir_hashes = arc.get_dir_hash_to_info_index();
            let mut index = None;
            for idx in dir_hashes.iter() {
                if idx.hash40() == hash {
                    index = Some(idx.index() as u32);
                    break
                }
            }
            match index {
                Some(index) => index,
                None => return String::from("Missing"),
            }
        } else {
            return String::from(USAGE)
        };

        let _ = match tables.get_loaded_directories().get(index as usize) {
            Some(dir) => dir,
            None => return String::from("Out of bounds"),
        };

        let data_idx = check_for_flag("-d", &mut args);
        let pretty = check_for_flag("-p", &mut args);
        let unloaded = check_for_flag("-u", &mut args);
        let info_type = if check_for_flag("--ref-count", &mut args) {
            CheckInfoType::RefCount
        } else if check_for_flag("--pointer", &mut args) {
            CheckInfoType::Pointer
        } else {
            CheckInfoType::State
        };

        check_directory(tables, index, data_idx, pretty, unloaded, info_type, None, 0)
    }

    pub fn handle_get_broken_filepaths(tables: &FilesystemInfo, mut args: Vec<String>) -> String {
        use std::fmt::Write;
        #[derive(Debug)]
        enum BrokenReason {
            InvalidDataIdx,
            DataUnloaded,
        }
        let filter = get_flag_and_option("-f", &mut args);
        let arc = resource::arc();
        let table1 = tables.get_loaded_filepaths();
        let table2 = tables.get_loaded_datas();
        let file_paths = arc.get_file_paths();
        let broken: Vec<(Hash40, BrokenReason)> = table1
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                if entry.is_loaded != 0 {
                    if entry.loaded_data_index == 0xFF_FFFF {
                        Some((file_paths[idx].path.hash40(), BrokenReason::InvalidDataIdx))
                    } else if table2[entry.loaded_data_index as usize].state != LoadState::Loaded {
                        Some((file_paths[idx].path.hash40(), BrokenReason::DataUnloaded))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        let mut output = String::from("");
        for (hash, reason) in broken.into_iter() {
            let dehashed = hashes::find(hash);
            if let Some(filter) = filter.as_ref() {
                if !dehashed.contains(filter) {
                    continue
                }
            }
            let _ = writeln!(&mut output, "'{}' ({:#x}): {:?}", dehashed, hash.0, reason);
        }
        output
    }
}

pub fn handle_command(mut args: Vec<String>) -> String {
    let tables = resource::filesystem_info();
    if args.is_empty() {
        return String::from(USAGE)
    }
    let command = args.remove(0);
    match command.as_str() {
        "get_filepath_table_entry" => lookups::handle_get_filepath_table_entry(tables, args),
        "get_loaded_data_table_entry" => lookups::handle_get_loaded_data_table_entry(tables, args),
        "get_loaded_directory_table_entry" => lookups::handle_get_loaded_directory_table_entry(tables, args),
        "check_directory" => utils::handle_check_directory(tables, args),
        "get_broken_filepaths" => utils::handle_get_broken_filepaths(tables, args),
        _ => String::from(USAGE),
    }
}
