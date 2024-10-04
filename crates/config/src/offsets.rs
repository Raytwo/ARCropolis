use std::fmt::Write;
use skyline::hooks::{getRegionAddress, Region};

static GET_UI_CHARA_PATH_FROM_HASH_SEARCH: (&[u8], isize) = (
    &[
        0xff, 0xc3, 0x06, 0xd1, 0xfc, 0x67, 0x16, 0xa9, 0xf8, 0x5f, 0x17, 0xa9, 0xf6, 0x57, 0x18, 0xa9, 0xf4, 0x4f, 0x19, 0xa9, 0xfd, 0x7b, 0x1a,
        0xa9, 0xfd, 0x83, 0x06, 0x91, 0xf4, 0x03, 0x00, 0xaa, 0x18, 0x20, 0xf8, 0xd2, 0x9f, 0x9e, 0x40, 0xf2, 0x8a, 0x1e, 0x48, 0x92, 0xe8, 0x07,
        0x9f, 0x1a, 0x5f, 0x01, 0x18, 0xeb, 0xe0, 0x03, 0x1f, 0xaa, 0xe9, 0x17, 0x9f, 0x1a,
    ],
    0x0,
);

fn get_text() -> &'static [u8] {
    unsafe {
        let ptr = getRegionAddress(Region::Text) as *const u8;
        let size = (getRegionAddress(Region::Rodata) as usize) - (ptr as usize);
        std::slice::from_raw_parts(ptr, size)
    }
}

pub fn system_locale_id() -> usize {
    let text = get_text();

    let get_ui_chara_path_from_hash = get_offset_neon(text, GET_UI_CHARA_PATH_FROM_HASH_SEARCH);

    let system_locale_id = {
        let adrp = get_ui_chara_path_from_hash + (4 * 23); // Skip 24 instructions to get to the REGION_NUM ADRP
        let adrp_offset = offset_from_adrp(adrp);
        let ldr_offset = offset_from_ldr(adrp + 4);
        adrp_offset + ldr_offset
    };

    system_locale_id
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

pub fn get_offset_neon(data: &[u8], pattern: (&'static [u8], isize)) -> usize {
    let mut s = String::new();

    for byte in pattern.0 {
        write!(&mut s, "{:X} ", byte).expect("lmao");
    }

    write!(&mut s, "??").expect("lmao");

    ((lazysimd::find_pattern_neon(data.as_ptr(), data.len(), s).expect("lmao") as isize) + pattern.1) as usize
}