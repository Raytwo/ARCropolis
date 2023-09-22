use crate::offsets;
use skyline::{from_offset, hook, hooks::InlineCtx, install_hooks, patching::{Patch, BranchBuilder}};

// Patches to get Inkling c08+ working
fn install_inkling_patches() {
    #[skyline::hook(offset = offsets::clear_ink_patch(), inline)]
    unsafe fn clear_ink_patch(ctx: &mut InlineCtx) {
        let res = (*ctx.registers[24].w.as_ref() as u32) % 8;
        *ctx.registers[24].w.as_mut() = res;
    }

    // Inkling Patches here nop some branches so it can work with more than c08+
    Patch::in_text(offsets::inkling_patch())
        .nop()
        .expect("Failed to patch inkling 1 cmp");
    
    Patch::in_text(offsets::inkling_patch() + 4)
        .nop()
        .expect("Failed to patch inkling 1 b.cs");
    
    BranchBuilder::branch()
        .branch_offset(offsets::inkling_c10plus())
        .branch_to_offset(offsets::inkling_c10plus() + 0x38)
        .replace();

    install_hooks!(clear_ink_patch);
}

/*
Patches for:
            - Aegis c08+ working
            - Some issues reported with some other characters
            - Crashes in Classic Mode with colors greater than c08+
            - Crashes in Tourney Mode when selecting a c08+ color
*/
fn install_added_color_patches() {
    // Install inkling patches here since they're related to added color issues
    install_inkling_patches();

    #[hook(offset = offsets::set_global_color_for_classic_mode(), inline)]
    pub unsafe fn set_global_color_for_classic_mode(ctx: &mut InlineCtx) {
        *(ctx.registers[9].w.as_mut()) = *(ctx.registers[9].w.as_ref()) % 8;
    }

    skyline::install_hook!(set_global_color_for_classic_mode);

    /*
      Offsets and Instructions that need to be patched in array so we don't repeat
      same code with minor changes over and over.
      Format: (offset, instruction)
    */
    // OFFSETS ARE CURRENTLY HARDCODED TO VERSION 13.0.1
    static ADDED_COLOR_PATCHES: &[(usize, u32)] = &[
        (0x1834b0c, 0xF104027F), // cmp x19, #256 (Issue related to Aegis)
        (0x18347bc, 0xF104027F), // cmp x19, #256 (Issue related to Aegis)
        (0x1834b28, 0xF104011F), // cmp  x8, #256 (Issue related to Aegis)
        (0x1834ef8, 0xF10402FF), // cmp x23, #256 (Issue related to Aegis)
        (0x183538c, 0xF104011F), // cmp  x8, #256 (Issue related to Aegis)
        (0x1835ae4, 0xF104027F), // cmp x19, #256 (Issue related to Aegis)
        (0x1835e00, 0xF104013F), // cmp  x9, #256 (Issue related to Aegis)
        (0x1a1c2f0, 0xF104011F), // cmp  x8, #256 (Issue related to Aegis)
        (0x1a1c334, 0xF104011F), // cmp  x8, #256 (Issue related to Aegis)
        (0x14de340, 0x7104013F), // cmp  w9, #256 (Issue related to Terry)
        (0x14e1410, 0x710402FF), // cmp w23, #256 (Issue related to Terry)
        (0x14e159c, 0x7104011F), // cmp  w8, #256 (Issue related to Terry)
        (0x1a55584, 0x52800020), // mov  w0, #1   (Issue related to Tourney Mode Crash)
        (0x1a556b4, 0xF104011F), // cmp  x8, #256 (Issue related to Tourney Mode Crash)
        (0x1a54f74, 0x7104015F), // cmp w10, #256 (Issue related to Tourney Mode Crash)
        (0x1a5500c, 0x7104001F), // cmp  w0, #256 (Issue related to Tourney Mode Crash)
        (0x1a55038, 0xF104001F), // cmp  x0, #256 (Issue related to Tourney Mode Crash)
        (0x1a54fa0, 0xF104001F), // cmp  x0, #256 (Issue related to Tourney Mode Crash)
        (0x1a54fd4, 0x7104017F), // cmp w11, #256 (Issue related to Tourney Mode Crash)
    ];

    for entry in ADDED_COLOR_PATCHES {
        let (offset, value) = entry;
        Patch::in_text(*offset)
            .data(*value)
            .expect(&format!("Failed to run Aegis Patch! Offset: {:#x} - Data: {:#x}", offset, value));
    }
}

fn install_lazy_loading_patches() {
    #[repr(C)]
    struct ParametersCache {
        pub vtable: *const u64,
        pub databases: *const ParameterDatabaseTable,
    }

    #[repr(C)]
    struct ParameterDatabaseTable {
        pub unk1: [u8; 360],
        pub character: *const u64, // ParameterDatabase
    }

    impl ParametersCache {
        pub unsafe fn get_chara_db(&self) -> *const u64 {
            (*(self.databases)).character
        }
    }

    // Cache of variables we reuse later for loading UI + getting the character database
    static mut PARAM_1: u64 = 0x0;
    static mut PARAM_4: u64 = 0x0;

    // This function is what's responsible for loading the UI File.
    #[from_offset(offsets::load_ui_file())]
    pub fn load_ui_file(param_1: *const u64, ui_path_hash: *const u64, unk1: u64, unk2: u64);

    /*
      This function is the function that takes the ui_chara_hash, color_slot, and
      the type of UI to load and converts them to a hash40 that represents the path
      it needs to load
    */
    #[from_offset(offsets::get_ui_chara_path_from_hash())]
    pub fn get_ui_chara_path_from_hash_color_and_type(ui_chara_hash: u64, color_slot: u32, ui_type: u32) -> u64;

    // This takes the character_database and the ui_chara_hash to get the color_num
    #[from_offset(offsets::get_color_num_from_hash())]
    pub fn get_color_num_from_hash(character_database: u64, ui_chara_hash: u64) -> u8;

    // This takes the character_database and the ui_chara_hash to get the chara's respective echo (for loading it at the same time)
    #[from_offset(offsets::get_echo_from_hash())]
    pub fn get_ui_chara_echo(character_database: u64, ui_chara_hash: u64) -> u64;

    #[hook(offset = offsets::load_chara_1_for_all_costumes(), inline)]
    pub unsafe fn original_load_chara_1_ui_for_all_colors(ctx: &mut InlineCtx) {
        // Save the first and fourth parameter for reference when we load the file ourselves
        PARAM_1 = *ctx.registers[0].x.as_ref();
        PARAM_4 = *ctx.registers[3].x.as_ref();
    }

    #[hook(offset = offsets::load_stock_icon_for_portrait_menu(), inline)]
    pub unsafe fn load_stock_icon_for_portrait_menu(ctx: &mut InlineCtx) {
        /*
          If both of these params are valid, then most likely we're in the
          CharaSelectMenu, which means we should be pretty safe loading the CSPs
        */
        if PARAM_1 != 0 && PARAM_4 != 0 {
            let ui_chara_hash = *ctx.registers[1].x.as_ref();
            let color = *ctx.registers[2].w.as_ref();
            let path = get_ui_chara_path_from_hash_color_and_type(ui_chara_hash, color, 1);
            load_ui_file(PARAM_1 as *const u64, &path, 0, PARAM_4);
        }
    }

    pub unsafe fn load_chara_1_for_ui_chara_hash_and_num(ui_chara_hash: u64, color: u32) {
        /*
          If we have the first and fourth param in our cache, then we're in the
          character select screen and can load the files manually
        */
        if PARAM_1 != 0 && PARAM_4 != 0 {
            // Get the color_num for smooth loading between different CSPs
            // Get the character database for the color num function
            let max_color: u32 = get_color_num_from_hash(
                (*(*(offsets::offset_to_addr(offsets::parameters_cache()) as *const u64) as *const ParametersCache)).get_chara_db() as u64,
                ui_chara_hash,
            ) as u32;

            let path = get_ui_chara_path_from_hash_color_and_type(ui_chara_hash, color, 1);
            load_ui_file(PARAM_1 as *const u64, &path, 0, PARAM_4);

            /*
              Set next color to 0 if it's going to end up past the max, else just be
              the current color + 1
            */
            let next_color = {
                let mut res = color + 1;
                if res >= max_color {
                    res = 0;
                }
                res
            };

            /*
              Set the previous color to max_color - 1 (so 8 - 1 = 7) if it's gonna be
              the u32::MAX (aka underflowed to max), else just be the current color - 1
            */
            let prev_color = {
                let mut res = color - 1;
                if res == u32::MAX {
                    res = max_color - 1;
                }
                res
            };

            // Load both next and previous color paths
            let next_color_path = get_ui_chara_path_from_hash_color_and_type(ui_chara_hash, next_color, 1);
            load_ui_file(PARAM_1 as *const u64, &next_color_path, 0, PARAM_4);
            let prev_color_path = get_ui_chara_path_from_hash_color_and_type(ui_chara_hash, prev_color, 1);
            load_ui_file(PARAM_1 as *const u64, &prev_color_path, 0, PARAM_4);
        }
    }

    #[hook(offset = offsets::css_set_selected_character_ui())]
    pub unsafe fn css_set_selected_character_ui(param_1: *const u64, chara_hash_1: u64, chara_hash_2: u64, color: u32, unk1: u32, unk2: u32) {
        let echo = get_ui_chara_echo(
            (*(*(offsets::offset_to_addr(offsets::parameters_cache()) as *const u64) as *const ParametersCache)).get_chara_db() as u64,
            chara_hash_1,
        );
        load_chara_1_for_ui_chara_hash_and_num(chara_hash_1, color);
        load_chara_1_for_ui_chara_hash_and_num(echo, color);
        call_original!(param_1, chara_hash_1, chara_hash_2, color, unk1, unk2);
    }

    #[hook(offset = offsets::chara_select_scene_destructor())]
    pub unsafe fn chara_select_scene_destructor(param_1: u64) {
        // Clear the first and fourth param in our cache so we don't load outside of the chara select
        PARAM_1 = 0;
        PARAM_4 = 0;
        call_original!(param_1);
    }

    // Prevent the game from loading all chara_1 colors at once for all characters
    Patch::in_text(offsets::load_chara_1_for_all_costumes())
        .nop()
        .expect("Failed to patch chara_1 load");

    // Install the hooks for everything necessary to properly load the chara_1s
    install_hooks!(
        original_load_chara_1_ui_for_all_colors,
        css_set_selected_character_ui,
        load_stock_icon_for_portrait_menu,
        chara_select_scene_destructor
    );
}

// Patch to allow running uncompiled lua scripts
fn install_lua_magic_patch() {
    Patch::in_text(offsets::lua_magic_check())
        .nop()
        .expect("Failed to patch lua magic check cbnz");
}

pub fn install() {
    install_added_color_patches();
    install_lazy_loading_patches();
    install_lua_magic_patch();
}
