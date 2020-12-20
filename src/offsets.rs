use skyline::hooks::{getRegionAddress, Region};

// default 9.0.2 offsets
pub static mut LOADED_TABLES_ADRP_OFFSET: usize = 0x35bb1f8;
pub static mut RES_SERVICE_ADRP_OFFSET: usize = 0x335a860;

pub static mut LOOKUP_STREAM_HASH_OFFSET: usize = 0x335A7F0;
pub static mut IDK_OFFSET: usize = 0x335F5F0;
pub static mut ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET: usize = 0x3359A40;
pub static mut PARSE_EFF_OFFSET: usize = 0x337A2B4;
pub static mut PARSE_EFF_NUTEXB_OFFSET: usize = 0x337A790;
pub static mut PARSE_PARAM_OFFSET: usize = 0x3539BB4;
pub static mut PARSE_MODEL_XMB_OFFSET:usize = 0x33FB1C8;
pub static mut PARSE_ARC_FILE_OFFSET:usize = 0x35893DC;
pub static mut PARSE_FONT_FILE_OFFSET:usize = 0x35773C8;
pub static mut PARSE_NUMSHB_FILE_OFFSET:usize = 0x33E21F0;
pub static mut PARSE_NUMATB_NUTEXB_OFFSET:usize = 0x3408824;
pub static mut PARSE_NUMSHEXB_FILE_OFFSET:usize = 0x33E40E4;
pub static mut PARSE_NUMATB_FILE_OFFSET:usize = 0x3407DBC;
pub static mut PARSE_NUMDLB_FILE_OFFSET:usize = 0x33DCB48;
pub static mut PARSE_LOG_XMB_OFFSET:usize = 0x33FB294;
pub static mut PARSE_MODEL_XMB_2_OFFSET:usize = 0x34073E4;
pub static mut TITLE_SCREEN_VERSION_OFFSET:usize = 0x35BAE00;
pub static mut PARSE_NUS3BANK_FILE_OFFSET:usize = 0x3552D94;

static LOADED_TABLES_ADRP_SEARCH_CODE: &[u8] = &[
    0xf3, 0x03, 0x00, 0xaa, 0x1f, 0x01, 0x09, 0x6b, 0xe0, 0x04, 0x00, 0x54,
];

static RES_SERVICE_ADRP_SEARCH_CODE: &[u8] = &[
    0x04, 0x01, 0x49, 0xfa, 0x21, 0x05, 0x00, 0x54, 0x5f, 0x00, 0x00, 0xf9, 0x7f, 0x00, 0x00, 0xf9,
];

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

static PARSE_EFF_SEARCH_CODE: &[u8] = &[
    0x09, 0x19, 0x40, 0xb9, 0x3f, 0x01, 0x0a, 0x6b, 0xfb, 0x03, 0x16, 0xaa, 0xc9, 0x02, 0x00, 0x54,
    0x09, 0x05, 0x40, 0xf9, 0x2b, 0x0d, 0x0a, 0x8b,
];

static PARSE_EFF_NUTEXB_SEARCH_CODE: &[u8] = &[
    0x3f, 0x01, 0x1c, 0x6b, 0xa0, 0x01, 0x00, 0x54, 0x0a, 0x1d, 0x40, 0xb9, 0x5f, 0x01, 0x09, 0x6b,
];

static PARSE_PARAM_SEARCH_CODE: &[u8] = &[
    0x28, 0x01, 0x40, 0xf9, 0x28, 0x03, 0x00, 0xb4, 0x09, 0x41, 0x00, 0x91, 0x68, 0xa6, 0x01, 0xa9,
    0x0a, 0x09, 0x80, 0xb9, 0x29, 0x01, 0x0a, 0x8b, 0x69, 0x16, 0x00, 0xf9, 0x08, 0x0d, 0x80, 0xb9,
];

static PARSE_MODEL_XMB_SEARCH_CODE: &[u8] = &[
    0x01, 0x01, 0x40, 0xf9, 0x03, 0x00, 0x00, 0x14, 0xf7, 0x17, 0x40, 0xf9, 0xe1, 0x03, 0x1f, 0xaa,
    0xe0, 0x22, 0x42, 0xf9, 0xcd, 0x07, 0x00, 0x94, 0xe8, 0x46, 0x42, 0xf9, 0x08, 0x01, 0x40, 0xf9,
];

static PARSE_MODEL_XMB_2_SEARCH_CODE: &[u8] = &[
    0x01, 0x01, 0x40, 0xf9, 0xde, 0xff, 0xff, 0x17, 0x00, 0x00, 0x00, 0x00,
];

static PARSE_LOG_XMB_SEARCH_CODE: &[u8] = &[
    0x13, 0x01, 0x40, 0xf9, 0x02, 0x00, 0x00, 0x14, 0xf3, 0x03, 0x1f, 0xaa, 0xe0, 0x03, 0x1c, 0x32,
    0xe1, 0x0b, 0x1d, 0x32,
];

static PARSE_ARC_FILE_SEARCH_CODE: &[u8] = &[
    0x8a, 0x00, 0x00, 0xb4, 0x55, 0x01, 0x40, 0xf9, 0x02, 0x00, 0x00, 0x14, 0xf5, 0x03, 0x1f, 0xaa,
];

static PARSE_FONT_FILE_SEARCH_CODE: &[u8] = &[
    0x01, 0x01, 0x40, 0xf9, 0xad, 0xff, 0xff, 0x17, 0x08, 0xc0, 0x41, 0x39, 0x48, 0x00, 0x00, 0x34,
    0xc0, 0x03, 0x5f, 0xd6, 0x00, 0x30, 0x40, 0xf9,
];

static PARSE_NUMATB_NUTEXB_SEARCH_CODE: &[u8] = &[
    0x1b, 0x01, 0x40, 0xf9, 0x02, 0x00, 0x00, 0x14, 0xfb, 0x03, 0x1f, 0xaa,
];

static PARSE_NUMSHEXB_FILE_SEARCH_CODE: &[u8] = &[
    0x49, 0x01, 0x00, 0x34, 0x28, 0x01, 0x00, 0xb4, 0x16, 0x01, 0x40, 0xf9, 0x07, 0x00, 0x00, 0x14,
];

static PARSE_NUMATB_FILE_SEARCH_CODE: &[u8] = &[
    0xea, 0x7e, 0x40, 0x92, 0x4a, 0xf1, 0x7d, 0xd3, 0x29, 0x69, 0x6a, 0xb8, 0xea, 0x5f, 0x00, 0x32,
];

static PARSE_NUMSHB_FILE_SEARCH_CODE: &[u8] = &[
    0x0a, 0x7f, 0x40, 0x92, 0x4a, 0xf1, 0x7d, 0xd3, 0x29, 0x69, 0x6a, 0xb8, 0xea, 0x5f, 0x00, 0x32, 0x3f, 0x01, 0x0a, 0x6b, 0xa0, 0x01, 0x00, 0x54
];

static PARSE_NUMDLB_FILE_SEARCH_CODE: &[u8] = &[
    0x08, 0x01, 0x40, 0xf9, 0xfb, 0xfe, 0xff, 0x17, 0xf3, 0x0f, 0x1e, 0xf8, 0xfd, 0x7b, 0x01, 0xa9,
    0xfd, 0x43, 0x00, 0x91, 0xf3, 0x03, 0x00, 0xaa,
];

static TITLE_SCREEN_VERSION_SEARCH_CODE: &[u8] = &[
    0xfc, 0x0f, 0x1d, 0xf8, 0xf4, 0x4f, 0x01, 0xa9, 0xfd, 0x7b, 0x02, 0xa9, 0xfd, 0x83, 0x00, 0x91,
    0xff, 0x07, 0x40, 0xd1, 0xf4, 0x03, 0x01, 0xaa, 0xf3, 0x03, 0x00, 0xaa,
];

static PARSE_NUS3BANK_FILE_SEARCH_CODE: &[u8] = &[
    0xf6, 0x01, 0x40, 0xf9, 0xf6, 0x10, 0x00, 0xb4, 0x8f, 0x6a, 0x7a, 0xb8, 0xf0, 0x5f, 0x00, 0x32,
    0xff, 0x01, 0x10, 0x6b, 0x60, 0x10, 0x00, 0x54,
];

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn offset_from_adrp(adrp_offset: usize) -> usize {
    unsafe {
        let adrp = *(offset_to_addr(adrp_offset) as *const u32);
        let immhi = (adrp & 0b0_00_00000_1111111111111111111_00000) >> 3;
        let immlo = (adrp & 0b0_11_00000_0000000000000000000_00000) >> 29;
        let imm = ((immhi | immlo) << 12) as i32 as usize;
        let base = adrp_offset & 0xFFFFFFFFFFFFF000;
        base + imm
    }
}

fn offset_from_ldr(ldr_offset: usize) -> usize {
    unsafe {
        let ldr = *(offset_to_addr(ldr_offset) as *const u32);
        let size = (ldr & 0b11_000_0_00_00_000000000000_00000_00000) >> 30;
        let imm = (ldr & 0b00_000_0_00_00_111111111111_00000_00000) >> 10;
        (imm as usize) << size
    }
}

pub fn offset_to_addr(offset: usize) -> *const () {
    unsafe { (getRegionAddress(Region::Text) as *const u8).offset(offset as isize) as _ }
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
                    println!("Error: no offset found for '{}'. Defaulting to 9.0.2 offset. This most likely won't work.", stringify!($out_variable));
                }
            }
        )*
    };
}

pub fn search_offsets() {
    unsafe {
        smash::resource::LOADED_TABLES_OFFSET = 0x50567a0;
        smash::resource::RES_SERVICE_OFFSET = 0x50567a8;

        let text_ptr = getRegionAddress(Region::Text) as *const u8;
        let text_size = (getRegionAddress(Region::Rodata) as usize) - (text_ptr as usize);

        let text = std::slice::from_raw_parts(text_ptr, text_size);

        if let Some(offset) = find_subsequence(text, LOADED_TABLES_ADRP_SEARCH_CODE) {
            LOADED_TABLES_ADRP_OFFSET = offset + 12;

            let adrp_offset = offset_from_adrp(LOADED_TABLES_ADRP_OFFSET);
            let ldr_offset = offset_from_ldr(LOADED_TABLES_ADRP_OFFSET + 4);
            smash::resource::LOADED_TABLES_OFFSET = adrp_offset + ldr_offset;
        } else {
            println!("Error: no offset found for 'loaded_tables_adrp'. Defaulting to 9.0.2 offset. This likely won't work.");
        }

        if let Some(offset) = find_subsequence(text, RES_SERVICE_ADRP_SEARCH_CODE) {
            RES_SERVICE_ADRP_OFFSET = offset + 16;

            let adrp_offset = offset_from_adrp(RES_SERVICE_ADRP_OFFSET);
            let ldr_offset = offset_from_ldr(RES_SERVICE_ADRP_OFFSET + 4);
            smash::resource::RES_SERVICE_OFFSET = adrp_offset + ldr_offset;
        } else {
            println!("Error: no offset found for 'res_service_adrp'. Defaulting to 9.0.2 offset. This likely won't work.");
        }

        find_offsets!(
            (IDK_OFFSET, IDK_SEARCH_CODE),
            (ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET, ADD_IDX_TO_TABLE1_AND_TABLE2_SEARCH_CODE),
            (LOOKUP_STREAM_HASH_OFFSET, LOOKUP_STREAM_HASH_SEARCH_CODE),
            (PARSE_EFF_OFFSET, PARSE_EFF_SEARCH_CODE),
            (PARSE_EFF_NUTEXB_OFFSET, PARSE_EFF_NUTEXB_SEARCH_CODE),
            (PARSE_PARAM_OFFSET, PARSE_PARAM_SEARCH_CODE),
            (PARSE_MODEL_XMB_OFFSET, PARSE_MODEL_XMB_SEARCH_CODE),
            (PARSE_ARC_FILE_OFFSET, PARSE_ARC_FILE_SEARCH_CODE),
            (PARSE_FONT_FILE_OFFSET, PARSE_FONT_FILE_SEARCH_CODE),
            (PARSE_NUMATB_NUTEXB_OFFSET, PARSE_NUMATB_NUTEXB_SEARCH_CODE),
            (PARSE_NUMSHEXB_FILE_OFFSET, PARSE_NUMSHEXB_FILE_SEARCH_CODE),
            (PARSE_NUMATB_FILE_OFFSET, PARSE_NUMATB_FILE_SEARCH_CODE),
            (PARSE_NUMSHB_FILE_OFFSET, PARSE_NUMSHB_FILE_SEARCH_CODE),
            (PARSE_NUMDLB_FILE_OFFSET, PARSE_NUMDLB_FILE_SEARCH_CODE),
            (PARSE_LOG_XMB_OFFSET, PARSE_LOG_XMB_SEARCH_CODE),
            (PARSE_MODEL_XMB_2_OFFSET, PARSE_MODEL_XMB_2_SEARCH_CODE),
            (TITLE_SCREEN_VERSION_OFFSET, TITLE_SCREEN_VERSION_SEARCH_CODE),
            (PARSE_NUS3BANK_FILE_OFFSET, PARSE_NUS3BANK_FILE_SEARCH_CODE),
        );

        PARSE_ARC_FILE_OFFSET += 4;
        PARSE_NUMSHEXB_FILE_OFFSET += 8;
        PARSE_NUMATB_FILE_OFFSET += 64;
        PARSE_NUMSHB_FILE_OFFSET += 64;
        PARSE_EFF_NUTEXB_OFFSET += 48;
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
