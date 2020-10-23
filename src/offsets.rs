use skyline::hooks::{getRegionAddress, Region};

// default 9.0.1 offsets
pub static mut LOOKUP_STREAM_HASH_OFFSET: usize = 0x335a350;
pub static mut IDK_OFFSET: usize = 0x335f150;
pub static mut ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET: usize = 0x33595a0;
pub static mut PARSE_EFF_OFFSET: usize = 0x3379e14;
pub static mut PARSE_EFF_NUTEXB_OFFSET: usize = 0x337a2f0;
pub static mut PARSE_PARAM_OFFSET: usize = 0x3539714;
pub static mut PARSE_MODEL_XMB_OFFSET:usize = 0x33fad28;
pub static mut PARSE_ARC_FILE_OFFSET:usize = 0x3588f3c;
pub static mut PARSE_FONT_FILE_OFFSET:usize = 0x3576f28;
pub static mut PARSE_NUMSHB_FILE_OFFSET:usize = 0x33e1d50;
pub static mut PARSE_NUMATB_NUTEXB_OFFSET:usize = 0x3408384;
pub static mut PARSE_NUMSHEXB_FILE_OFFSET:usize = 0x33e3c44;
pub static mut PARSE_NUMATB_FILE_OFFSET:usize = 0x340791c;
pub static mut PARSE_NUMDLB_FILE_OFFSET:usize = 0x33dc6a8;
pub static mut PARSE_LOG_XMB_OFFSET:usize = 0x33fadf4;
pub static mut PARSE_MODEL_XMB_2_OFFSET:usize = 0x3406f44;
pub static mut TITLE_SCREEN_VERSION_OFFSET:usize = 0x35ba960;
pub static mut PARSE_NUS3BANK_FILE_OFFSET:usize = 0x35528f4;

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

macro_rules! find_offsets {
    (
        $(
            ($out_variable:expr, $search_pattern:expr)
        ),*
        $(,)?
    ) => {
        $(
            unsafe {
                let text_ptr = getRegionAddress(Region::Text) as *const u8;
                let text_size = (getRegionAddress(Region::Rodata) as usize) - (text_ptr as usize);
                let text = std::slice::from_raw_parts(text_ptr, text_size);

                if let Some(offset) = find_subsequence(text, $search_pattern) {
                    $out_variable = offset
                } else {
                    println!("Error: no offset found for '{}'. Defaulting to 8.1.0 offset. This most likely won't work.", stringify!($out_variable));
                }
            }
        )*
    };
}

pub fn search_offsets() {
    unsafe {
        smash::resource::LOADED_TABLES_OFFSET = 0x50567a0;
        smash::resource::RES_SERVICE_OFFSET = 0x50567a8;
    }
        find_offsets!(
            //(IDK_OFFSET, IDK_SEARCH_CODE),
            //(ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET, ADD_IDX_TO_TABLE1_AND_TABLE2_SEARCH_CODE),
            //(LOOKUP_STREAM_HASH_OFFSET, LOOKUP_STREAM_HASH_SEARCH_CODE),
            // (PARSE_EFF_NUTEXB_OFFSET, PARSE_EFF_NUTEXB_SEARCH_CODE),
            // (PARSE_EFF_OFFSET, PARSE_EFF_SEARCH_CODE),
            // (PARSE_PARAM_OFFSET, PARSE_PARAM_SEARCH_CODE),
            // (PARSE_MODEL_XMB_OFFSET, PARSE_MODEL_XMB_SEARCH_CODE),
            // (PARSE_ARC_FILE_OFFSET, PARSE_ARC_FILE_SEARCH_CODE),
            // (PARSE_FONT_FILE_OFFSET, PARSE_FONT_FILE_SEARCH_CODE),
            // (PARSE_NUMATB_NUTEXB_OFFSET, PARSE_NUMATB_NUTEXB_SEARCH_CODE),
            // (PARSE_NUMSHEXB_FILE_OFFSET, PARSE_NUMSHEXB_FILE_SEARCH_CODE),
            // (PARSE_NUMATB_FILE_OFFSET, PARSE_NUMATB_FILE_SEARCH_CODE),
            // (PARSE_NUMDLB_FILE_OFFSET, PARSE_NUMDLB_FILE_SEARCH_CODE),
            // (PARSE_LOG_XMB_OFFSET, PARSE_LOG_XMB_SEARCH_CODE),
            // (PARSE_MODEL_XMB_2_OFFSET, PARSE_MODEL_XMB_2_SEARCH_CODE),
            //(TITLE_SCREEN_VERSION_OFFSET, TITLE_SCREEN_VERSION_SEARCH_CODE),
        );
}

// #[allow(dead_code)]
// pub fn expand_table2() {
//     let loaded_tables = LoadedTables::get_instance();

//     unsafe {
//         nn::os::LockMutex(loaded_tables.mutex);
//     }

//     let mut table2_vec = loaded_tables.table_2().to_vec();

//     table2_vec.push(Table2Entry {
//         data: 0 as *const u8,
//         ref_count: AtomicU32::new(0),
//         is_used: false,
//         state: FileState::Unused,
//         file_flags2: false,
//         flags: 45,
//         version: 0xFFFF,
//         unk: 0,
//     });

//     loaded_tables.table2_len = table2_vec.len() as u32;
//     let mut table2_array = table2_vec.into_boxed_slice();
//     loaded_tables.table2 = table2_array.as_ptr() as *mut Table2Entry;

//     unsafe {
//         nn::os::UnlockMutex(loaded_tables.mutex);
//     }
// }

// pub fn shared_redirection() {
//     let str_path = "rom:/skyline/redirect.txt";

//     let s = match fs::read_to_string(str_path) {
//         Err(why) => {
//             println!("[HashesMgr] Failed to read \"{}\" \"({})\"", str_path, why);
//             return;
//         }
//         Ok(s) => s,
//     };

//     for entry in string_to_static_str(s).lines() {
//         let mut values = entry.split_whitespace();

//         let loaded_tables = LoadedTables::get_instance();
//         let arc = loaded_tables.get_arc();
//         let path = values.next().unwrap();
//         println!("Path to replace: {}", path);
//         let hash = hash40(path);

//         unsafe {
//             let hashindexgroup_slice =
//                 slice::from_raw_parts(arc.file_info_path, (*loaded_tables).table1_len as usize);

//             let t1_index = match hashindexgroup_slice
//                 .iter()
//                 .position(|x| x.path.hash40.as_u64() == hash)
//             {
//                 Some(index) => index as u32,
//                 None => {
//                     println!(
//                         "[ARC::Patching] Hash {} not found in table1, skipping",
//                         hash
//                     );
//                     continue;
//                 }
//             };
//             println!("T1 index found: {}", t1_index);

//             let file_info = arc.lookup_file_information_by_t1_index(t1_index);
//             println!("Path index: {}", file_info.path_index);

//             let mut file_index = arc.lookup_fileinfoindex_by_t1_index(t1_index);
//             println!("File_info_index: {}", file_index.file_info_index);

//             // Make sure it is flagged as a shared file
//             if (file_info.flags & 0x00000010) == 0x10 {
//                 let path = values.next().unwrap();
//                 println!("Replacing path: {}", path);
//                 let hash = hash40(path);

//                 let t1_index = match hashindexgroup_slice
//                     .iter()
//                     .position(|x| x.path.hash40.as_u64() == hash)
//                 {
//                     Some(index) => index as u32,
//                     None => {
//                         println!(
//                             "[ARC::Patching] Hash {} not found in table1, skipping",
//                             hash
//                         );
//                         continue;
//                     }
//                 };

//                 println!("T1 index found: {}", t1_index);
//                 file_index.file_info_index = t1_index;
//                 file_index.file_info_index = t1_index;
//                 println!("New file_info_index: {}", file_index.file_info_index);
//             }
//         }
//     }

//     //hashes.insert(hash40(hs), hs);
// }
