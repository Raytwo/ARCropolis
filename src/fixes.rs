use skyline::{hook, hooks::InlineCtx, install_hooks, from_offset, patching::Patch};
use crate::offsets::offset_to_addr;

// OFFSETS IN THIS FILE ARE CURRENTLY HARDCODED TO VERSION 13.0.1

// Patches to get Inkling c08+ working
fn install_inkling_patches(){
    // All offsets related to Inkling stuff here
    static INKLING_1_PATCH: usize = 0x35baed4;
    static INKLING_2_PATCH: usize = 0x35baed8;
    static CLEAR_INK_PATCH_OFFSET: usize = 0x35bb960;

    #[skyline::hook(offset = CLEAR_INK_PATCH_OFFSET, inline)]
    unsafe fn clear_ink_patch(ctx: &mut InlineCtx) {
        let res = (*ctx.registers[24].w.as_ref() as u32) % 8;
        *ctx.registers[24].w.as_mut() = res;
    }

    // Inkling Patches here nop some branches so it can work with more than
    // 8 players
    Patch::in_text(INKLING_1_PATCH).nop().expect("Failed to patch inkling 1 cmp");
    Patch::in_text(INKLING_2_PATCH).nop().expect("Failed to patch inkling 1 b.cs");
    
    install_hooks!(clear_ink_patch);
}

// Patches to get Aegis c08+ working
fn install_aegis_patches(){
    // Offsets and Instructions that need to be patched in array so we don't repeat
    // same code over and over.
    // Format: (offset, instruction)
    static AEGIS_PATCHES: &[(usize, u32)] = &[
        (0x1834b0c, 0xF104027F), // cmp x19, #256
        (0x18347bc, 0xF104027F), // cmp x19, #256
        (0x1834b28, 0xF104011F), // cmp x8, #256
        (0x1834ef8, 0xF10402FF), // cmp x23, #256
        (0x183538c, 0xF104011F), // cmp x8, #256
        (0x1835ae4, 0xF104027F), // cmp x19, #256
        (0x1835e00, 0xF104013F), // cmp x9, #256
        (0x1a1c2f0, 0xF104011F), // cmp x8, #256
        (0x1a1c334, 0xF104011F), // cmp x8, #256
    ];

    for entry in AEGIS_PATCHES {
        let (offset, value) = entry;
        Patch::in_text(*offset).data(*value).expect(&format!("Failed to run Aegis Patch! Offset: {:#x} - Data: {:#x}", offset, value));
    }
}

fn install_lazy_loading_patches(){
    // All offsets related to lazy loading
    static PARAMATERS_CACHE_OFFSET: usize = 0x532d730;
    static LOAD_CHARA_1_FOR_ALL_COSTUMES_OFFSET: usize = 0x18465cc;
    static LOAD_UI_FILE_OFFSET: usize = 0x323b290;
    static GET_UI_CHARA_PATH_FROM_HASH_COLOR_AND_TYPE_OFFSET: usize = 0x3237820;
    static GET_COLOR_NUM_FROM_HASH: usize = 0x32621a0;
    static LOAD_STOCK_ICON_FOR_PORTRAIT_MENU_OFFSET: usize = 0x19e784c;
    static CSS_SET_SELECTED_CHARARACTER_UI_OFFSET: usize = 0x19fc790;
    static CHARA_SELECT_SCENE_DESTRUCTOR_OFFSET: usize = 0x18467c0;
    
    // Cache of variables we reuse later for loading UI + getting the character database
    static mut PARAM_1: u64 = 0x0;
    static mut PARAM_4: u64 = 0x0;
    static mut PARAMATERS_CACHE: *const u64 = 0x0 as *const u64;
    
    // This function is what's responsible for loading the UI File.
    #[from_offset(LOAD_UI_FILE_OFFSET)]
    pub fn load_ui_file(param_1: *const u64, ui_path_hash: *const u64, unk1: u64, unk2: u64);
    
    // This function is the function that takes the ui_chara_hash, color_slot, and
    // the type of UI to load and converts them to a hash40 that represents the path
    // it needs to load
    #[from_offset(GET_UI_CHARA_PATH_FROM_HASH_COLOR_AND_TYPE_OFFSET)]
    pub fn get_ui_chara_path_from_hash_color_and_type(ui_chara_hash: u64, color_slot: u32, ui_type: u32) -> u64;
    
    // This takes the character_database and the ui_chara_hash to get the color_num
    #[from_offset(GET_COLOR_NUM_FROM_HASH)]
    pub fn get_color_num_from_hash(character_database: u64, ui_chara_hash: u64) -> u8;
    
    #[hook(offset = LOAD_CHARA_1_FOR_ALL_COSTUMES_OFFSET, inline)]
    pub unsafe fn original_load_chara_1_ui_for_all_colors(ctx: &mut InlineCtx){
        // Save the first and fourth paramater for reference when we load the
        // file ourselves
        PARAM_1 = *ctx.registers[0].x.as_ref();
        PARAM_4 = *ctx.registers[3].x.as_ref();
    }
    
    #[hook(offset = LOAD_STOCK_ICON_FOR_PORTRAIT_MENU_OFFSET, inline)]
    pub unsafe fn load_stock_icon_for_portrait_menu(ctx: &mut InlineCtx){
        // If both of these params are valid, then most likely we're in the
        // CharaSelectMenu, which means we should be pretty safe loading the CSPs
        if PARAM_1 != 0 && PARAM_4 != 0 {
            let ui_chara_hash = *ctx.registers[1].x.as_ref();
            let color = *ctx.registers[2].w.as_ref();
            let path = get_ui_chara_path_from_hash_color_and_type(ui_chara_hash, color, 1);
            load_ui_file(PARAM_1 as *const u64, &path, 0, PARAM_4);
        }
    }
    
    #[hook(offset = CSS_SET_SELECTED_CHARARACTER_UI_OFFSET)]
    pub unsafe fn css_set_selected_chararacter_ui(
        param_1: *const u64,
        chara_hash_1: u64,
        chara_hash_2: u64,
        color: u32,
        unk1: u32,
        unk2: u32,
    ){
        // If we have the first and fourth param in our cache, then we're in the
        // character select screen and can load the files manually
        if PARAM_1 != 0 && PARAM_4 != 0 {
            // Get the color_num for smooth loading between different CSPs
            let max_color: u32 = {
                // Get the character database for the color num function
                let character_database = {
                    let databases = *((*PARAMATERS_CACHE + 0x8) as *const u64);
                    *((databases + 360) as *const u64) as u64
                };
                get_color_num_from_hash(character_database, chara_hash_1) as u32
            };
    
            let path = get_ui_chara_path_from_hash_color_and_type(chara_hash_1, color, 1);
            load_ui_file(PARAM_1 as *const u64, &path, 0, PARAM_4);
    
            // Set next color to 0 if it's going to end up past the max, else just be
            // the current color + 1
            let next_color = {
                let mut res = color + 1;
                if res >= max_color {
                    res = 0;
                }
                res
            };
    
            // Set the previous color to max_color - 1 (so 8 - 1 = 7) if it's gonna be
            // the u32::MAX (aka underflowed to max), else just be the current color - 1
            let prev_color = {
                let mut res = color - 1;
                if res == u32::MAX {
                    res = max_color - 1;
                }
                res
            };
    
            // Load both next and previous color paths
            let next_color_path = get_ui_chara_path_from_hash_color_and_type(chara_hash_1, next_color, 1);
            load_ui_file(PARAM_1 as *const u64, &next_color_path, 0, PARAM_4);        
            let prev_color_path = get_ui_chara_path_from_hash_color_and_type(chara_hash_1, prev_color, 1);
            load_ui_file(PARAM_1 as *const u64, &prev_color_path, 0, PARAM_4);
        }
    
    
        call_original!(param_1, chara_hash_1, chara_hash_2, color, unk1, unk2);
    }
    
    #[hook(offset = CHARA_SELECT_SCENE_DESTRUCTOR_OFFSET)]
    pub unsafe fn chara_select_scene_destructor(
        param_1: u64,
    ){
        // Clear the first and fourth param in our cache so we don't load outside of the chara select
        PARAM_1 = 0;
        PARAM_4 = 0;
        call_original!(param_1);
    }

    // Gets the PARAMATERS_CACHE address so we can get the character database
    // later
    unsafe {
        PARAMATERS_CACHE = offset_to_addr(PARAMATERS_CACHE_OFFSET) as *const u64;
    }

    // Prevent the game from loading all chara_1 colors at once for all characters
    Patch::in_text(LOAD_CHARA_1_FOR_ALL_COSTUMES_OFFSET).nop().expect("Failed to patch chara_1 load");
    
    // Install the hooks for everything nessecary to properly load the chara_1s
    install_hooks!(original_load_chara_1_ui_for_all_colors, css_set_selected_chararacter_ui, load_stock_icon_for_portrait_menu, chara_select_scene_destructor);
}

pub fn install() {
    install_inkling_patches();
    install_aegis_patches();
    install_lazy_loading_patches();
}