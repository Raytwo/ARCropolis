use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use skyline::hooks::{getRegionAddress, Region};

static OFFSETS: Lazy<Offsets> = Lazy::new(|| {
    let path = crate::CACHE_PATH.join("offsets.toml");
    let offsets = match std::fs::read_to_string(&path) {
        Ok(string) => match toml::de::from_str(string.as_str()) {
            Ok(offsets) => offsets,
            Err(err) => {
                error!("Unable to parse 'offsets.toml'. Reason: {:?}", err);
                Offsets::new()
            },
        },
        Err(err) => {
            error!("Unable to read 'offsets.toml'. Reason: {:?}", err);
            Offsets::new()
        },
    };

    match toml::ser::to_string_pretty(&offsets) {
        Ok(string) => {
            if std::fs::write(path, string.as_bytes()).is_err() { error!("Unable to write 'offsets.toml'.") }
        },
        Err(_) => error!("Failed to serialize offsets."),
    }
    offsets
});

static FILESYSTEM_INFO_ADRP_SEARCH_CODE: &[u8] = &[0xf3, 0x03, 0x00, 0xaa, 0x1f, 0x01, 0x09, 0x6b, 0xe0, 0x04, 0x00, 0x54];

static RES_SERVICE_ADRP_SEARCH_CODE: &[u8] = &[
    0x04, 0x01, 0x49, 0xfa, 0x21, 0x05, 0x00, 0x54, 0x5f, 0x00, 0x00, 0xf9, 0x7f, 0x00, 0x00, 0xf9,
];

static LOOKUP_STREAM_HASH_SEARCH_CODE: &[u8] = &[
    0x29, 0x58, 0x40, 0xf9, 0x28, 0x60, 0x40, 0xf9, 0x2a, 0x05, 0x40, 0xb9, 0x09, 0x0d, 0x0a, 0x8b, 0xaa, 0x01, 0x00, 0x34, 0x5f, 0x01, 0x00, 0xf1,
];

static TITLE_SCREEN_VERSION_SEARCH_CODE: &[u8] = &[
    0xfc, 0x0f, 0x1d, 0xf8, 0xf4, 0x4f, 0x01, 0xa9, 0xfd, 0x7b, 0x02, 0xa9, 0xfd, 0x83, 0x00, 0x91, 0xff, 0x07, 0x40, 0xd1, 0xf4, 0x03, 0x01, 0xaa,
    0xf3, 0x03, 0x00, 0xaa,
];

static ESHOPMANAGER_SHOW_SEARCH_CODE: &[u8] = &[
    0x08, 0xe1, 0x43, 0xf9, 0x14, 0x05, 0x40, 0xf9, 0x88, 0x22, 0x44, 0x39, 0x08, 0x04, 0x00, 0x35,
];

static INFLATE_SEARCH_CODE: &[u8] = &[
    0x4b, 0x00, 0x1b, 0x0b, 0x00, 0x01, 0x1f, 0xd6, 0x68, 0x6a, 0x40, 0xf9, 0x09, 0x3d, 0x40, 0xf9, 0x2c, 0x01, 0x40, 0xf9,
];

static MEMCPY_1_SEARCH_CODE: &[u8] = &[
    0xf5, 0x1f, 0x40, 0xb9, 0xa7, 0x00, 0x00, 0x14, 0xe2, 0xa3, 0x00, 0x91, 0xe4, 0xc3, 0x00, 0x91,
];

static MEMCPY_2_SEARCH_CODE: &[u8] = &[
    0xf8, 0x1b, 0x40, 0xf9, 0x1f, 0x03, 0x15, 0xeb, 0xa2, 0x2a, 0x00, 0x54, 0x96, 0x03, 0x18, 0x8b, 0x68, 0x1a, 0x40, 0xf9,
];

static MEMCPY_3_SEARCH_CODE: &[u8] = &[
    0xe8, 0x03, 0x18, 0xaa, 0xf8, 0x1b, 0x40, 0xf9, 0xd6, 0x02, 0x18, 0x8b, 0xbf, 0x02, 0x18, 0xeb, 0x88, 0xfb, 0xff, 0x54,
];

static INFLATE_DIR_FILE_SEARCH_CODE: &[u8] = &[
    0xfc, 0x6f, 0xba, 0xa9, 0xfa, 0x67, 0x01, 0xa9, 0xf8, 0x5f, 0x02, 0xa9, 0xf6, 0x57, 0x03, 0xa9, 0xf4, 0x4f, 0x04, 0xa9, 0xfd, 0x7b, 0x05, 0xa9,
    0xfd, 0x43, 0x01, 0x91, 0xff, 0x03, 0x07, 0xd1, 0x4c, 0xb4, 0x40, 0xa9,
];

static MANUAL_OPEN_SEARCH_CODE: &[u8] = &[
    0xfc, 0x4f, 0xbe, 0xa9, 0xfd, 0x7b, 0x01, 0xa9, 0xfd, 0x43, 0x00, 0x91, 0xff, 0x0f, 0x40, 0xd1,
];

static INITIAL_LOADING_SEARCH_CODE: &[u8] = &[
    0x08, 0x3f, 0x40, 0xf9, 0x08, 0x01, 0x40, 0xf9, 0x08, 0x21, 0x40, 0xf9, 0x08, 0x3d, 0x40, 0xb9, 0x08, 0x5d, 0x00, 0x12,
];

static PROCESS_RESOURCE_NODE_SEARCH_CODE: &[u8] = &[
    0x5f, 0x05, 0x00, 0x31, 0xea, 0x03, 0x8a, 0x1a, 0x29, 0x01, 0x0a, 0x0b, 0x29, 0x05, 0x00, 0x11, 0x6a, 0x3f, 0x40, 0xf9, 0x29, 0x21, 0xad, 0x9b,
];

static RES_LOAD_LOOP_START_SEARCH_CODE: &[u8] = &[
    0x2a, 0x05, 0x09, 0x8b, 0x6e, 0x62, 0x01, 0x91, 0xdf, 0x01, 0x1b, 0xeb, 0x4d, 0xf1, 0x7d, 0xd3, 0xca, 0x01, 0x0d, 0x8b, 0x6d, 0x03, 0x0d, 0x8b,
];

static RES_LOAD_LOOP_REFRESH_SEARCH_CODE: &[u8] = &[
    0x68, 0x32, 0x40, 0xf9, 0xee, 0x1b, 0x40, 0xf9, 0xdf, 0x01, 0x08, 0xeb, 0xec, 0x3f, 0x40, 0xf9, 0xed, 0x37, 0x40, 0xf9,
];

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

#[allow(clippy::inconsistent_digit_grouping)]
fn offset_from_adrp(adrp_offset: usize) -> usize {
    unsafe {
        let adrp = *(offset_to_addr(adrp_offset) as *const u32);
        let immhi = (adrp & 0b0000_0000_1111_1111_1111_1111_1110_0000) >> 3;
        let immlo = (adrp & 0b0110_0000_0000_0000_0000_0000_0000_0000) >> 29;
        let imm = ((immhi | immlo) << 12) as i32 as usize;
        let base = adrp_offset & 0xFFFF_FFFF_FFFF_F000;
        base + imm
    }
}

#[allow(clippy::inconsistent_digit_grouping)]
fn offset_from_ldr(ldr_offset: usize) -> usize {
    unsafe {
        let ldr = *(offset_to_addr(ldr_offset) as *const u32);
        let size = (ldr & 0b1100_0000_0000_0000_0000_0000_0000_0000) >> 30;
        let imm = (ldr & 0b0000_0000_0011_1111_1111_1100_0000_0000) >> 10;
        (imm as usize) << size
    }
}

pub fn offset_to_addr(offset: usize) -> *const () {
    unsafe { (getRegionAddress(Region::Text) as *const u8).add(offset) as _ }
}

fn get_text() -> &'static [u8] {
    unsafe {
        let ptr = getRegionAddress(Region::Text) as *const u8;
        let size = (getRegionAddress(Region::Rodata) as usize) - (ptr as usize);
        std::slice::from_raw_parts(ptr, size)
    }
}

#[derive(Serialize, Deserialize)]
struct Offsets {
    pub lookup_stream_hash: usize,
    pub inflate: usize,
    pub memcpy_1: usize,
    pub memcpy_2: usize,
    pub memcpy_3: usize,
    pub inflate_dir_file: usize,
    pub manual_open: usize,
    pub initial_loading: usize,
    pub process_resource_node: usize,
    pub res_load_loop_start: usize,
    pub res_load_loop_refresh: usize,
    pub title_screen_version: usize,
    pub eshop_button: usize,

    pub filesystem_info: usize,
    pub res_service: usize,
}

impl Offsets {
    pub fn new() -> Self {
        let text = get_text();
        let lookup_stream_hash = find_subsequence(text, LOOKUP_STREAM_HASH_SEARCH_CODE).expect("Unable to find subsequence");
        let inflate = find_subsequence(text, INFLATE_SEARCH_CODE).expect("Unable to find subsequence");
        let memcpy_1 = find_subsequence(text, MEMCPY_1_SEARCH_CODE).expect("Unable to find subsequence") - 4;
        let memcpy_2 = find_subsequence(text, MEMCPY_2_SEARCH_CODE).expect("Unable to find subsequence") - 4;
        let memcpy_3 = find_subsequence(text, MEMCPY_3_SEARCH_CODE).expect("Unable to find subsequence") - 4;
        let inflate_dir_file = find_subsequence(text, INFLATE_DIR_FILE_SEARCH_CODE).expect("Unable to find subsequence");
        let manual_open = find_subsequence(text, MANUAL_OPEN_SEARCH_CODE).expect("Unable to find subsequence");
        let initial_loading = find_subsequence(text, INITIAL_LOADING_SEARCH_CODE).expect("Unable to find subsequence");
        let process_resource_node = find_subsequence(text, PROCESS_RESOURCE_NODE_SEARCH_CODE).expect("Unable to find subsequence") + 0xC;
        let res_load_loop_start = find_subsequence(text, RES_LOAD_LOOP_START_SEARCH_CODE).expect("Unable to find subsequence");
        let res_load_loop_refresh = find_subsequence(text, RES_LOAD_LOOP_REFRESH_SEARCH_CODE).expect("Unable to find subsequence");
        let title_screen_version = find_subsequence(text, TITLE_SCREEN_VERSION_SEARCH_CODE).expect("Unable to find subsequence!");
        let eshop_button = find_subsequence(text, ESHOPMANAGER_SHOW_SEARCH_CODE).expect("Unable to find subsequence!") - 16;

        let filesystem_info = {
            let adrp = find_subsequence(text, FILESYSTEM_INFO_ADRP_SEARCH_CODE).expect("Unable to find subsequence") + 12;
            let adrp_offset = offset_from_adrp(adrp);
            let ldr_offset = offset_from_ldr(adrp + 4);
            adrp_offset + ldr_offset
        };
        let res_service = {
            let adrp = find_subsequence(text, RES_SERVICE_ADRP_SEARCH_CODE).expect("Unable to find subsequence") + 16;
            let adrp_offset = offset_from_adrp(adrp);
            let ldr_offset = offset_from_ldr(adrp + 4);
            adrp_offset + ldr_offset
        };

        Self {
            lookup_stream_hash,
            inflate,
            memcpy_1,
            memcpy_2,
            memcpy_3,
            inflate_dir_file,
            manual_open,
            initial_loading,
            process_resource_node,
            res_load_loop_start,
            res_load_loop_refresh,
            title_screen_version,
            eshop_button,

            filesystem_info,
            res_service,
        }
    }
}

pub fn initial_loading() -> usize {
    OFFSETS.initial_loading
}

pub fn filesystem_info() -> usize {
    OFFSETS.filesystem_info
}

pub fn res_service() -> usize {
    OFFSETS.res_service
}

pub fn inflate() -> usize {
    OFFSETS.inflate
}

pub fn inflate_dir_file() -> usize {
    OFFSETS.inflate_dir_file
}

pub fn memcpy_1() -> usize {
    OFFSETS.memcpy_1
}

pub fn memcpy_2() -> usize {
    OFFSETS.memcpy_2
}

pub fn memcpy_3() -> usize {
    OFFSETS.memcpy_3
}

pub fn res_load_loop_start() -> usize {
    OFFSETS.res_load_loop_start
}

pub fn res_load_loop_refresh() -> usize {
    OFFSETS.res_load_loop_refresh
}

pub fn title_screen_version() -> usize {
    OFFSETS.title_screen_version
}

pub fn eshop_show() -> usize {
    OFFSETS.eshop_button
}

pub fn lookup_stream_hash() -> usize {
    OFFSETS.lookup_stream_hash
}
