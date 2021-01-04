use skyline::hooks::{getRegionAddress, Region};

// default 9.0.2 offsets
pub static mut LOADED_TABLES_ADRP_OFFSET: usize = 0x35bb1f8;
pub static mut RES_SERVICE_ADRP_OFFSET: usize = 0x335a860;

pub static mut LOOKUP_STREAM_HASH_OFFSET: usize = 0x335A7F0;
pub static mut TITLE_SCREEN_VERSION_OFFSET:usize = 0x35BAE00;

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
            (LOOKUP_STREAM_HASH_OFFSET, LOOKUP_STREAM_HASH_SEARCH_CODE),
            (TITLE_SCREEN_VERSION_OFFSET, TITLE_SCREEN_VERSION_SEARCH_CODE),
        );
    }
}
