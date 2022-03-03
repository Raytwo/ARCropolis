use smash_arc::Hash40;

pub mod arc;
pub mod search;
pub mod table;

static USAGE: &'static str = 
r#"ARCropolis remote CLI server | Usage
General:
    Common flags are '-i' for passing an index and '-h' for passing a hashable item. Hashable items are either raw hashes (prefixed with 0x) or strings.
    An index can be either decimal or hexadecimal.

    It is recommended to have "hashes.txt" at rom:/skyline/hashes.txt so that any dumped information that can display that will display it.

arc: Commands to help scan data.arc tables/entries at runtime.
    Directory Lookups:
        get_directory [-i | -h] [<index> | <hashable>]
        get_directory_file_group [-i | -h] [<index> | <hashable>]
            Note: The index/hashable item in this command are for directories, not file groups
        get_directory_shared_file_group [-i | -h] [<index> | <hashable>]
            Note: The index/hashable item in this command are for directories, not file groups
        get_file_group <index>

    File Lookups:
        get_file_path [-i | -h] [<index> | <hashable>]
        get_file_info_index [-i | -h] [<index> | <hashable>]
        get_file_info [-i | -h] [<index> | <hashable>]
        get_file_info_to_data [-i | -h] [<index> | <hashable>]  <region>
            Note: The region is only necessary when passing the hashable item.
        get_file_data [-i | -h] [-i | -h] [<index> | <hashable>] <region>
            Note: The region is only necessary when passing the hashable item.
        
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
            Gets the first file data index that is shared.

table: Commands to help scan the filesystem tables for analysis/troubleshooting at runtime.
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
                  Pass --ref-count or --pointer to only see those fields of the loaded data table entries for each file (the default is the loaded state)
        freeze
            Note: Freezes ResLoadingThread
        unfreeze
            Note: Unfreeses ResLoadingThread
"#;

pub fn parse_index(arg: &str) -> Option<u32> {
    match arg.parse::<u32>() {
        Ok(val) => Some(val),
        Err(_) => {
            match u32::from_str_radix(arg.trim_start_matches("0x"), 16) {
                Ok(val) => Some(val),
                Err(_) => None
            }
        }
    }
}

pub fn parse_hash(arg: &str) -> Hash40 {
    if arg.starts_with("0x") {
        match u64::from_str_radix(arg.trim_start_matches("0x"), 16) {
            Ok(val) => Hash40(val),
            Err(_) => Hash40::from(arg)
        }
    } else {
        Hash40::from(arg)
    }
}

pub fn check_for_flag(flag: &str, args: &mut Vec<String>) -> bool {
    let mut index = 0;
    let has_flag = args.iter().enumerate()
        .any(|(idx, x)| {
            if x == flag {
                index = idx;
                true
            } else {
                false
            }
        });
    if has_flag {
        let _ = args.remove(index);
    }
    has_flag
}

pub fn get_flag_and_option(flag: &str, args: &mut Vec<String>) -> Option<String> {
    let mut index = 0;
    let has_flag = args.iter().enumerate()
        .any(|(idx, x)| {
            if x == flag {
                index = idx;
                true
            } else {
                false
            }
        });
    if has_flag && index != args.len() - 1{
        let _ = args.remove(index);
        Some(args.remove(index))
    } else {
        None
    }
}

pub fn handle_command(mut args: Vec<String>) -> String {
    if args.len() == 0 || args.get(0).unwrap() == "help" {
        return String::from(USAGE);
    }
    let category = args.remove(0);
    match category.as_str() {
        "arc" => {
            arc::handle_command(args)
        },
        "search" => {
            search::handle_command(args)
        },
        "table" => {
            table::handle_command(args)
        },
        _ => String::from(USAGE)
    }
}