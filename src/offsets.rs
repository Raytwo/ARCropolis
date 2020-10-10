use skyline::hooks::{getRegionAddress, Region};

// default 8.0.0 offsets
pub static mut LOOKUP_STREAM_HASH_OFFSET: usize = 0x324f7a0;
// default 8.1.0 offsets
pub static mut IDK_OFFSET: usize = 0x325dcc0;
pub static mut ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET: usize = 0x3258110;
pub static mut PARSE_NUTEXB_OFFSET: usize = 0x3306004;
pub static mut PARSE_EFF_OFFSET: usize = 0x32789e0;
pub static mut PARSE_EFF_NUTEXB_OFFSET: usize = 0x3278e60;
pub static mut PARSE_PARAM_OFFSET: usize = 0x3436884;
pub static mut PARSE_MODEL_XMB_OFFSET:usize = 0x32f89a8;

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
    0x0b, 0x01, 0x40, 0xf9, 0x02, 0x00, 0x00, 0x14, 0xeb, 0x03, 0x1f, 0xaa, 0x08, 0x15, 0x80, 0x52,
    0x08, 0x57, 0x08, 0x9b, 0x09, 0x02, 0x90, 0x52, 0x29, 0x00, 0xa0, 0x72,
];

static PARSE_EFF_NUTEXB_SEARCH_CODE: &[u8] = &[
    0x1a, 0x01, 0x40, 0xf9, 0x02, 0x00, 0x00, 0x14, 0xfa, 0x03, 0x1f, 0xaa, 0x68, 0xe3, 0x00, 0x90,
    0x08, 0x11, 0x41, 0xf9, 0xbf, 0x83, 0x14, 0xf8, 0xbf, 0x7f, 0x35, 0xa9, 0x09, 0x3d, 0x40, 0xf9, 
];

static PARSE_PARAM_SEARCH_CODE: &[u8] = &[
    0x28, 0x01, 0x40, 0xf9, 0x28, 0x03, 0x00, 0xb4, 0x09, 0x41, 0x00, 0x91, 0x68, 0xa6, 0x01, 0xa9,
    0x0a, 0x09, 0x80, 0xb9, 0x29, 0x01, 0x0a, 0x8b, 0x69, 0x16, 0x00, 0xf9, 0x08, 0x0d, 0x80, 0xb9,
];

static PARSE_MODEL_XMB_SEARCH_CODE: &[u8] = &[
    0x01, 0x01, 0x40, 0xf9, 0x03, 0x00, 0x00, 0x14, 0xf7, 0x17, 0x40, 0xf9, 0xe1, 0x03, 0x1f, 0xaa,
    0xe0, 0x22, 0x42, 0xf9, 0xcd, 0x07, 0x00, 0x94, 0xe8, 0x46, 0x42, 0xf9, 0x08, 0x01, 0x40, 0xf9,
];

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
                    println!("Error: no offset found for '{}'. Defaulting to 8.0.0 offset. This most likely won't work.", stringify!($out_variable));
                }
            }
        )*
    };
}

pub fn search_offsets() {
        find_offsets!(
            (IDK_OFFSET, IDK_SEARCH_CODE),
            (ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET, ADD_IDX_TO_TABLE1_AND_TABLE2_SEARCH_CODE),
            (LOOKUP_STREAM_HASH_OFFSET, LOOKUP_STREAM_HASH_SEARCH_CODE),
            (PARSE_NUTEXB_OFFSET, PARSE_NUTEXB_SEARCH_CODE),
            (PARSE_EFF_NUTEXB_OFFSET, PARSE_EFF_NUTEXB_SEARCH_CODE),
            (PARSE_EFF_OFFSET, PARSE_EFF_SEARCH_CODE),
            (PARSE_PARAM_OFFSET, PARSE_PARAM_SEARCH_CODE),
            (PARSE_MODEL_XMB_OFFSET, PARSE_MODEL_XMB_SEARCH_CODE)
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
