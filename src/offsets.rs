use skyline::hooks::{getRegionAddress, Region};

// default 9.0.2 offsets
pub static mut LOADED_TABLES_ADRP_OFFSET: usize = 0x35b_b1f8;
pub static mut RES_SERVICE_ADRP_OFFSET: usize = 0x335_a860;

pub static mut LOOKUP_STREAM_HASH_OFFSET: usize = 0x335_A7F0;
pub static mut TITLE_SCREEN_VERSION_OFFSET: usize = 0x35B_AE00;

pub static mut INFLATE_OFFSET: usize = 0x33b_71e8;
pub static mut MEMCPY_1_OFFSET: usize = 0x33b_7d08;
pub static mut MEMCPY_2_OFFSET: usize = 0x33b_78f8;
pub static mut MEMCPY_3_OFFSET: usize = 0x33b_7988;
pub static mut INFLATE_DIR_FILE_OFFSET: usize = 0x381_6230;
pub static mut MANUAL_OPEN_OFFSET: usize = 0x35c_93b0;
pub static mut INITIAL_LOADING_OFFSET: usize = 0x35c_6474;
pub static mut PROCESS_RESOURCE_NODE: usize = 0x34e_3e24; // 12.0.0
pub static mut RES_LOAD_LOOP_START: usize = 0x34e_34c4; // 12.0.0
pub static mut RES_LOAD_LOOP_REFRESH: usize = 0x34e_42f8; // 12.0.0

static LOADED_TABLES_ADRP_SEARCH_CODE: &[u8] = &[
    0xf3, 0x03, 0x00, 0xaa, 0x1f, 0x01, 0x09, 0x6b, 0xe0, 0x04, 0x00, 0x54,
];

static RES_SERVICE_ADRP_SEARCH_CODE: &[u8] = &[
    0x04, 0x01, 0x49, 0xfa, 0x21, 0x05, 0x00, 0x54, 0x5f, 0x00, 0x00, 0xf9, 0x7f, 0x00, 0x00, 0xf9,
];

static LOOKUP_STREAM_HASH_SEARCH_CODE: &[u8] = &[
    0x29, 0x58, 0x40, 0xf9, 0x28, 0x60, 0x40, 0xf9, 0x2a, 0x05, 0x40, 0xb9, 0x09, 0x0d, 0x0a, 0x8b,
    0xaa, 0x01, 0x00, 0x34, 0x5f, 0x01, 0x00, 0xf1,
];

static TITLE_SCREEN_VERSION_SEARCH_CODE: &[u8] = &[
    0xfc, 0x0f, 0x1d, 0xf8, 0xf4, 0x4f, 0x01, 0xa9, 0xfd, 0x7b, 0x02, 0xa9, 0xfd, 0x83, 0x00, 0x91,
    0xff, 0x07, 0x40, 0xd1, 0xf4, 0x03, 0x01, 0xaa, 0xf3, 0x03, 0x00, 0xaa,
];

static INFLATE_SEARCH_CODE: &[u8] = &[
    0x4b, 0x00, 0x1b, 0x0b, 0x00, 0x01, 0x1f, 0xd6, 0x68, 0x6a, 0x40, 0xf9, 0x09, 0x3d, 0x40, 0xf9,
    0x2c, 0x01, 0x40, 0xf9,
];

static MEMCPY_1_SEARCH_CODE: &[u8] = &[
    0xf5, 0x1f, 0x40, 0xb9, 0xa7, 0x00, 0x00, 0x14, 0xe2, 0xa3, 0x00, 0x91, 0xe4, 0xc3, 0x00, 0x91,
];

static MEMCPY_2_SEARCH_CODE: &[u8] = &[
    0xf8, 0x1b, 0x40, 0xf9, 0x1f, 0x03, 0x15, 0xeb, 0xa2, 0x2a, 0x00, 0x54, 0x96, 0x03, 0x18, 0x8b,
    0x68, 0x1a, 0x40, 0xf9,
];

static MEMCPY_3_SEARCH_CODE: &[u8] = &[
    0xe8, 0x03, 0x18, 0xaa, 0xf8, 0x1b, 0x40, 0xf9, 0xd6, 0x02, 0x18, 0x8b, 0xbf, 0x02, 0x18, 0xeb,
    0x88, 0xfb, 0xff, 0x54,
];

static INFLATE_DIR_FILE_SEARCH_CODE: &[u8] = &[
    0xfc, 0x6f, 0xba, 0xa9, 0xfa, 0x67, 0x01, 0xa9, 0xf8, 0x5f, 0x02, 0xa9, 0xf6, 0x57, 0x03, 0xa9,
    0xf4, 0x4f, 0x04, 0xa9, 0xfd, 0x7b, 0x05, 0xa9, 0xfd, 0x43, 0x01, 0x91, 0xff, 0x03, 0x07, 0xd1,
    0x4c, 0xb4, 0x40, 0xa9,
];

static MANUAL_OPEN_SEARCH_CODE: &[u8] = &[
    0xfc, 0x4f, 0xbe, 0xa9, 0xfd, 0x7b, 0x01, 0xa9, 0xfd, 0x43, 0x00, 0x91, 0xff, 0x0f, 0x40, 0xd1,
];

static INITIAL_LOADING_SEARCH_CODE: &[u8] = &[
    0x08, 0x3f, 0x40, 0xf9, 0x08, 0x01, 0x40, 0xf9, 0x08, 0x21, 0x40, 0xf9, 0x08, 0x3d, 0x40, 0xb9,
    0x08, 0x5d, 0x00, 0x12,
];

static PROCESS_RESOURCE_NODE_SEARCH_CODE: &[u8] = &[
    0x5f, 0x05, 0x00, 0x31, 0xea, 0x03, 0x8a, 0x1a, 0x29, 0x01, 0x0a, 0x0b, 0x29, 0x05, 0x00, 0x11,
    0x6a, 0x3f, 0x40, 0xf9, 0x29, 0x21, 0xad, 0x9b
];

static RES_LOAD_LOOP_START_SEARCH_CODE: &[u8] = &[
    0x2a, 0x05, 0x09, 0x8b, 0x6e, 0x62, 0x01, 0x91, 0xdf, 0x01, 0x1b, 0xeb, 0x4d, 0xf1, 0x7d, 0xd3,
    0xca, 0x01, 0x0d, 0x8b, 0x6d, 0x03, 0x0d, 0x8b
];

static RES_LOAD_LOOP_REFRESH_SEARCH_CODE: &[u8] = &[
    0x68, 0x32, 0x40, 0xf9, 0xee, 0x1b, 0x40, 0xf9, 0xdf, 0x01, 0x08, 0xeb, 0xec, 0x3f, 0x40, 0xf9,
    0xed, 0x37, 0x40, 0xf9
];


fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[allow(clippy::inconsistent_digit_grouping)]
fn offset_from_adrp(adrp_offset: usize) -> usize {
    unsafe {
        let adrp = *(offset_to_addr(adrp_offset) as *const u32);
        let immhi = (adrp & 0b0_00_00000_1111111111111111111_00000) >> 3;
        let immlo = (adrp & 0b0_11_00000_0000000000000000000_00000) >> 29;
        let imm = ((immhi | immlo) << 12) as i32 as usize;
        let base = adrp_offset & 0xFFFF_FFFF_FFFF_F000;
        base + imm
    }
}

#[allow(clippy::inconsistent_digit_grouping)]
fn offset_from_ldr(ldr_offset: usize) -> usize {
    unsafe {
        let ldr = *(offset_to_addr(ldr_offset) as *const u32);
        let size = (ldr & 0b11_000_0_00_00_000000000000_00000_00000) >> 30;
        let imm = (ldr & 0b00_000_0_00_00_111111111111_00000_00000) >> 10;
        (imm as usize) << size
    }
}

pub fn offset_to_addr(offset: usize) -> *const () {
    unsafe { (getRegionAddress(Region::Text) as *const u8).add(offset) as _ }
}

macro_rules! find_offsets {
    (
        $(
            ($out_variable:expr, $search_pattern:expr)
        ),*
        $(,)?
    ) => {
        $(
            let text_ptr = getRegionAddress(Region::Text) as *const u8;
            let text_size = (getRegionAddress(Region::Rodata) as usize) - (text_ptr as usize);
            let text = std::slice::from_raw_parts(text_ptr, text_size);

            if let Some(offset) = find_subsequence(text, $search_pattern) {
                $out_variable = offset
            } else {
                println!("Error: no offset found for '{}'. Defaulting to 9.0.2 offset. This most likely won't work.", stringify!($out_variable));
            }
        )*
    };
}

pub fn search_offsets() {
    unsafe {
        crate::runtime::LOADED_TABLES_OFFSET = 0x505_67a0;
        crate::runtime::RES_SERVICE_OFFSET = 0x505_67a8;

        let text_ptr = getRegionAddress(Region::Text) as *const u8;
        let text_size = (getRegionAddress(Region::Rodata) as usize) - (text_ptr as usize);

        let text = std::slice::from_raw_parts(text_ptr, text_size);

        if let Some(offset) = find_subsequence(text, LOADED_TABLES_ADRP_SEARCH_CODE) {
            LOADED_TABLES_ADRP_OFFSET = offset + 12;

            let adrp_offset = offset_from_adrp(LOADED_TABLES_ADRP_OFFSET);
            let ldr_offset = offset_from_ldr(LOADED_TABLES_ADRP_OFFSET + 4);
            crate::runtime::LOADED_TABLES_OFFSET = adrp_offset + ldr_offset;
        } else {
            println!("Error: no offset found for 'loaded_tables_adrp'. Defaulting to 9.0.2 offset. This likely won't work.");
        }

        if let Some(offset) = find_subsequence(text, RES_SERVICE_ADRP_SEARCH_CODE) {
            RES_SERVICE_ADRP_OFFSET = offset + 16;

            let adrp_offset = offset_from_adrp(RES_SERVICE_ADRP_OFFSET);
            let ldr_offset = offset_from_ldr(RES_SERVICE_ADRP_OFFSET + 4);
            crate::runtime::RES_SERVICE_OFFSET = adrp_offset + ldr_offset;
        } else {
            println!("Error: no offset found for 'res_service_adrp'. Defaulting to 9.0.2 offset. This likely won't work.");
        }

        find_offsets!(
            (LOOKUP_STREAM_HASH_OFFSET, LOOKUP_STREAM_HASH_SEARCH_CODE),
            (
                TITLE_SCREEN_VERSION_OFFSET,
                TITLE_SCREEN_VERSION_SEARCH_CODE
            ),
            (INFLATE_OFFSET, INFLATE_SEARCH_CODE),
            (MEMCPY_1_OFFSET, MEMCPY_1_SEARCH_CODE),
            (MEMCPY_2_OFFSET, MEMCPY_2_SEARCH_CODE),
            (MEMCPY_3_OFFSET, MEMCPY_3_SEARCH_CODE),
            (INFLATE_DIR_FILE_OFFSET, INFLATE_DIR_FILE_SEARCH_CODE),
            (MANUAL_OPEN_OFFSET, MANUAL_OPEN_SEARCH_CODE),
            (INITIAL_LOADING_OFFSET, INITIAL_LOADING_SEARCH_CODE),
            (PROCESS_RESOURCE_NODE, PROCESS_RESOURCE_NODE_SEARCH_CODE),
            (RES_LOAD_LOOP_START, RES_LOAD_LOOP_START_SEARCH_CODE),
            (RES_LOAD_LOOP_REFRESH, RES_LOAD_LOOP_REFRESH_SEARCH_CODE)
        );
        PROCESS_RESOURCE_NODE += 0xC;
    }
}
