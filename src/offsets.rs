use skyline::hooks::{getRegionAddress, Region};

// default 8.0.0 offsets
pub static mut IDK_OFFSET: usize = 0x32545a0;
pub static mut ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET: usize = 0x324e9f0;
pub static mut LOOKUP_STREAM_HASH_OFFSET: usize = 0x324f7a0;
// default 8.1.0 offsets
pub static mut PARSE_NUTEXB_OFFSET: usize = 0x330615c;
pub static mut PARSE_EFF_OFFSET: usize = 0x3278984;

static IDK_SEARCH_CODE: &[u8] = &[
    0xf8, 0x5f, 0xbc, 0xa9, 0xf6, 0x57, 0x01, 0xa9, 0xf4, 0x4f, 0x02, 0xa9, 0xfd, 0x7b, 0x03, 0xa9,
    0xfd, 0xc3, 0x00, 0x91, 0xe8, 0x5f, 0x00, 0x32, 0x3f, 0x00, 0x08, 0x6b,
];

static ADD_IDX_TO_TABLE1_AND_TABLE2_SEARCH_CODE: &[u8] = &[
    0xf6, 0x57, 0xbd, 0xa9, 0xf4, 0x4f, 0x01, 0xa9, 0xfd, 0x7b, 0x02, 0xa9, 0xfd, 0x83, 0x00, 0x91,
    0x08, 0x18, 0x40, 0xb9, 0x1f, 0x01, 0x01, 0x6b,
];

static LOOKUP_STREAM_HASH_SEARCH_CODE: &[u8] = &[
    0x29, 0x58, 0x40, 0xf9, 0x28, 0x60, 0x40, 0xf9, 0x2a, 0x05, 0x40, 0xb9, 0x09, 0x0d, 0x0a, 0x8b,
    0xaa, 0x01, 0x00, 0x34, 0x5f, 0x01, 0x00, 0xf1,
];

static PARSE_NUTEXB_SEARCH_CODE: &[u8] = &[
    0xe8, 0x3f, 0x00, 0x32, 0xe8, 0xfb, 0x00, 0xb9, 0xe8, 0x0f, 0x40, 0xf9, 0xea, 0x4b, 0x40, 0xf9,
    0xe9, 0x07, 0x40, 0xf9, 0xf3, 0x03, 0x00, 0xaa,
];

static PARSE_EFF_SEARCH_CODE: &[u8] = &[
    0x09, 0x19, 0x40, 0xb9, 0x3f, 0x01, 0x0a, 0x6b, 0xfb, 0x03, 0x16, 0xaa, 0xc9, 0x02, 0x00, 0x54,
    0x09, 0x05, 0x40, 0xf9, 0x2b, 0x0d, 0x0a, 0x8b,
];

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

pub fn search_offsets() {
    unsafe {
        let text_ptr = getRegionAddress(Region::Text) as *const u8;
        let text_size = (getRegionAddress(Region::Rodata) as usize) - (text_ptr as usize);
        let text = std::slice::from_raw_parts(text_ptr, text_size);

        if let Some(offset) = find_subsequence(text, IDK_SEARCH_CODE) {
            IDK_OFFSET = offset
        } else {
            println!("Error: no offset found for function 'idk'. Defaulting to 8.0.0 offset. This likely won't work.");
        }

        if let Some(offset) = find_subsequence(text, ADD_IDX_TO_TABLE1_AND_TABLE2_SEARCH_CODE) {
            ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET = offset
        } else {
            println!("Error: no offset found for function 'add_idx_to_table1_and_table2'. Defaulting to 8.0.0 offset. This likely won't work.");
        }

        if let Some(offset) = find_subsequence(text, LOOKUP_STREAM_HASH_SEARCH_CODE) {
            LOOKUP_STREAM_HASH_OFFSET = offset
        } else {
            println!("Error: no offset found for function 'add_idx_to_table1_and_table2'. Defaulting to 8.0.0 offset. This likely won't work.");
        }

        if let Some(offset) = find_subsequence(text, PARSE_NUTEXB_SEARCH_CODE) {
            PARSE_NUTEXB_OFFSET = offset - 8
        } else {
            println!("Error: no offset found for function 'parse_fighter_nutexb'. Defaulting to 8.1.0 offset. This likely won't work.");
        }

        if let Some(offset) = find_subsequence(text, PARSE_EFF_SEARCH_CODE) {
            PARSE_EFF_OFFSET = offset
        } else {
            println!("Error: no offset found for function 'parse_eff'. Defaulting to 8.1.0 offset. This likely won't work.");
        }
    }
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
