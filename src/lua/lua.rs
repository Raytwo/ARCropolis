use std::ffi::CString;

use crate::offsets;
use skyline::from_offset;

pub type LuaCfunction = ::std::option::Option<unsafe extern "C" fn(L: &mut lua_state) -> ::std::os::raw::c_int>;
pub type LMem = u64;

#[from_offset(offsets::lua_l_newmetatable())]
fn lua_l_newmetatable(lua_state: &mut lua_state, name: *const u8);

#[from_offset(offsets::lua_setfield())]
fn lua_setfield(lua_state: &mut lua_state, unk_1: *const u64, name: *const u8);

#[from_offset(offsets::lua_l_setfuncs())]
fn lua_l_setfuncs(lua_state: &mut lua_state, regs: *const u64, index: u32);

#[from_offset(offsets::lua_c_step())]
fn lua_c_step(lua_state: &mut lua_state);

#[from_offset(offsets::lua_h_new())]
fn lua_h_new(lua_state: &mut lua_state) -> *const u64;

#[from_offset(offsets::lua_getfield())]
fn lua_getfield(lua_state: &mut lua_state, lua_registry: *const TValue, name: *const u8);

#[from_offset(offsets::lua_setmetatable())]
fn lua_setmetatable(lua_state: &mut lua_state, obj_idx: i32);

#[from_offset(0x38f6cb0)]
fn lua_tonumberx(lua_state: &mut lua_state, idx: i32, unk: *const u64) -> f32;

#[from_offset(0x38f4000)]
fn lua_tointegerx(lua_state: &mut lua_state, idx: i32, unk: *const u64) -> u64;

#[from_offset(0x38f4180)]
fn lua_tolstring(lua_state: &mut lua_state, idx: i32, unk: *const u64) -> *const u8;

#[from_offset(offsets::declare_namespace())]
fn declare_namespace(enum_builder: &mut LuaEnumBuilder, lua_state: Option<&mut lua_state>, enum_name: *const u8, table_index: i32);

#[from_offset(offsets::add_method())]
fn add_method(enum_builder: &mut LuaEnumBuilder, enum_name: *const u8, function: LuaCfunction);

#[from_offset(offsets::lua_pushstring())]
fn lua_pushstring(lua_state: &mut lua_state, name: *const u8);

// #[from_offset(0x38f3fa0)] 13.0.1 offset
// fn lua_gettable(lua_state: *mut lua_state, idx: i32);

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct TValue {
    pub udata: u64,
    pub tt: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct luaL_Reg {
    pub name: *const u8,
    pub func: LuaCfunction,
}

#[repr(C)]
#[derive(Debug)]
pub struct luaL_Reg_container {
    pub name: String,
    pub func: LuaCfunction,
}

#[repr(C)]
#[derive(Debug)]
pub struct luaL_Reg_from_api {
    pub name: *mut i8,
    pub func: LuaCfunction,
}

#[repr(C)]
#[derive(Debug)]
pub struct lua_state {
    pub unk: [u8; 0xF],
    pub top_ptr: *mut TValue,
    pub global_state: &'static mut global_state,
    pub unk_2: [u8; 176],
}

#[repr(C)]
#[derive(Debug)]
pub struct global_state {
    pub unk: [u8; 0x17],
    pub gc_debt: LMem,
    pub unk_2: [u8; 0x20],
    pub l_registry: TValue,
    pub unk_3: [u8; 0xA9],
}

#[repr(C)]
#[derive(Debug)]
pub struct unk_udata_struct {
    pub buf: [u8; 0xA],
    pub unk_1_0xb: u8,
    pub unk_2_0xc: u32,
    pub unk_3_0x10: u64,
    pub unk_4_0x18: u64,
}

#[repr(C)]
#[derive(Debug)]
pub struct unk_struct {
    pub unk_1_0x0: u64,
    pub unk_2_0x8: u32,
    pub buf: [u8; 0x4],
    pub unk_3_0x10: u64,
    pub unk_4_0x18: u32,
    pub unk_5_0x1c: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct LuaEnumBuilder {
    pub lua_state: *mut lua_state,
    pub type_name: *const u8,
    pub table_index: i32,
}

#[repr(i32)]
pub enum LuaTagType {
    NilType = 0x0,
    BoolType = 0x1,
    LightUserDataType = 0x2,
    NumberType = 0x3,
    StringType = 0x4,
    TableType = 0x5,
    FunctionType = 0x6,
    UserDataType = 0x7,
    ThreadType = 0x8,
    IntType = 0x13,
    LongStringType = 0x14,
    LightCFunctionType = 0x16,
    CClosureType = 0x26,
    CollectableNilType = 0x40,
    CollectableBoolType = 0x41,
    CollectableLightUserDataType = 0x42,
    CollectableNumberType = 0x43,
    CollectableStringType = 0x44,
    CollectableTableType = 0x45,
    CollectableFunctionType = 0x46,
    CollectableUserDataType = 0x47,
    CollectableThreadType = 0x48,
}

impl LuaEnumBuilder {
    pub fn new() -> LuaEnumBuilder {
        LuaEnumBuilder {
            lua_state: std::ptr::null_mut(),
            type_name: std::ptr::null(),
            table_index: 0,
        }
    }

    pub fn declare_namespace(&mut self, lua_state: Option<&mut lua_state>, name: impl AsRef<str>) {
        unsafe {
            declare_namespace(
                self,
                lua_state,
                CString::new(name.as_ref())
                    .expect(&format!("Failed to make CString from {}!", name.as_ref()))
                    .into_raw() as _,
                -3,
            );
        }
    }

    pub fn add_method(&mut self, reg: &luaL_Reg) {
        unsafe { add_method(self, reg.name, reg.func) }
    }
}

impl lua_state {
    pub fn get_current_top(&mut self) -> TValue {
        unsafe { *(self.top_ptr) }
    }
    pub fn get_previous_top(&mut self) -> TValue {
        unsafe { *(self.top_ptr).sub(1) }
    }
    pub fn update_current_top(&mut self, new: &TValue) {
        unsafe {
            let top_ptr = self.top_ptr;
            (*top_ptr).tt = new.tt;
            (*top_ptr).udata = new.udata;
        }
    }
    pub fn increment_top_address(&mut self) {
        unsafe {
            self.top_ptr = (self.top_ptr).add(1);
        }
    }
    pub fn decrement_top_address(&mut self) {
        unsafe {
            self.top_ptr = (self.top_ptr).sub(1);
        }
    }
    pub fn set_top_field(&mut self, index: i32, field: impl AsRef<str>) {
        unsafe {
            let ptr = {
                if index < 0 {
                    self.top_ptr.sub((index * -1) as usize)
                } else {
                    self.top_ptr.add(index as usize)
                }
            };
            lua_setfield(self, ptr as _, format!("{}\0", field.as_ref()).as_ptr() as _);
        }
    }
    pub fn new_table(&mut self) -> *const u64 {
        unsafe { lua_h_new(self) }
    }
    pub fn step(&mut self) {
        unsafe { lua_c_step(self) }
    }
    pub fn get_field(&mut self, registry: &TValue, field: impl AsRef<str>) {
        unsafe {
            lua_getfield(self, registry, format!("{}\0", field.as_ref()).as_ptr() as _);
        }
    }
    pub fn set_field(&mut self, registry: *const u64, field: impl AsRef<str>) {
        unsafe {
            lua_setfield(self, registry, format!("{}\0", field.as_ref()).as_ptr() as _);
        }
    }
    pub fn set_metatable(&mut self, idx: i32) {
        unsafe {
            lua_setmetatable(self, idx);
        }
    }
    pub fn new_metatable(&mut self, field: impl AsRef<str>) {
        unsafe { lua_l_newmetatable(self, format!("{}\0", field.as_ref()).as_ptr() as _) }
    }
    pub fn set_funcs(&mut self, funcs: &[luaL_Reg]) {
        unsafe {
            lua_l_setfuncs(self, funcs.as_ptr() as _, 0);
        }
    }

    // This variant is specifically for the API
    pub fn get_string_arg_ptr(&mut self) -> *const u8 {
        unsafe {
            let string = lua_tolstring(self, -1, std::ptr::null());
            self.decrement_top_address();
            string
        }
    }

    pub fn get_string_arg(&mut self) -> String {
        unsafe {
            let string = skyline::from_c_str(lua_tolstring(self, -1, std::ptr::null()));
            self.decrement_top_address();
            string
        }
    }

    pub fn get_number_arg(&mut self) -> f32 {
        unsafe {
            let num = lua_tonumberx(self, -1, std::ptr::null());
            self.decrement_top_address();
            num
        }
    }

    pub fn get_integer_arg(&mut self) -> u64 {
        unsafe {
            let num = lua_tointegerx(self, -1, std::ptr::null());
            self.decrement_top_address();
            num
        }
    }

    pub fn push_integer(&mut self, int: u64) {
        unsafe {
            (*self.top_ptr).tt = LuaTagType::IntType as _;
            (*self.top_ptr).udata = int;
        }
        self.increment_top_address();
    }

    pub fn push_number(&mut self, float: f32) {
        unsafe {
            (*self.top_ptr).tt = LuaTagType::NumberType as _;
            // We need to get udata as a mutable pointer to a f32, so we first get the pointer of the udata as a *const u64
            // since the compiler won't allow us to directly cast it into a *mut f32 pointer, then we dereference the f32 pointer and give it
            // our own float.
            // Basically: v = u64 -> v = &v -> v = &v as *const u64 -> v = v as *mut f32 -> *v = float
            *((&((*self.top_ptr).udata) as *const u64) as *mut f32) = float;
        }
        self.increment_top_address();
    }

    pub fn push_string(&mut self, string: impl AsRef<str>) {
        let ptr = format!("{}\0", string.as_ref()).as_ptr();
        unsafe {
            lua_pushstring(self, ptr as _);
        }
        // self.increment_top_address(); // This is done by the native smash function
    }

    pub fn push_bool(&mut self, state: bool) {
        unsafe {
            (*self.top_ptr).tt = LuaTagType::BoolType as _;
            // Same logic as the number type, where we need the udata as a pointer to a specific type, then give it our value
            *((&((*self.top_ptr).udata) as *const u64) as *mut u32) = state as _;
        }
        self.increment_top_address();
    }

    pub fn push_nil(&mut self) {
        unsafe {
            (*self.top_ptr).tt = LuaTagType::NilType as _;
        }
        self.increment_top_address();
    }

    pub fn add_menu_manager(&mut self, name: impl AsRef<str>, registry: &[luaL_Reg]) {
        unsafe {
            // Replicates the code used by the game to insert a new lua singleton
            let normal = format!("{}", name.as_ref());
            let metatable = format!("Metatable{}", name.as_ref());
            self.new_metatable(&metatable);

            let mut top = self.get_current_top();
            let prev_top = self.get_previous_top();

            top.tt = prev_top.tt;
            top.udata = prev_top.udata;
            self.update_current_top(&top);

            self.increment_top_address();
            self.set_top_field(-1, "__index");
            self.set_funcs(registry);
            self.decrement_top_address();

            let gc_debt = self.global_state.gc_debt;
            if 0 < gc_debt {
                self.step();
            }

            let tbl_ptr = self.new_table();

            let mut top = self.get_current_top();
            top.udata = tbl_ptr as u64;
            top.tt = LuaTagType::CollectableTableType as _;

            self.update_current_top(&top);
            self.increment_top_address();

            let lua_registry = self.global_state.l_registry;

            self.get_field(&lua_registry, &metatable);
            self.set_metatable(-2);

            let mut set_field_var = std::ptr::null();

            let pv_var7 = lua_registry.udata;

            let mut pu_var4: *const unk_struct = std::ptr::null();
            let mut pi_var1: *const unk_struct = std::ptr::null();

            let lua_nil_addr = offsets::lua_nil() as *const u64;
            let lua_r_udata = lua_registry.udata as *const unk_udata_struct;

            if (*lua_r_udata).unk_2_0xc < 2 {
                panic!("lua_r_udata.unk_2_0xc is below a 2!");
                // Implementation is copied word for word (and translated) from Ghidra.
                // I have no idea what the game is doing here, but this is replicating what its doing
                // You can see this being done in the `apply_ui2d_layout_bindings` function (0x33702b0 in Smash ver 13.0.1) after the metatable is set
                pu_var4 = ((*lua_r_udata).unk_4_0x18 + (!((0xffffffff as u64) << ((*lua_r_udata).unk_1_0xb as u64 & 0x3f)) as u64 & 2) * 0x20)
                    as *const unk_struct;
                loop {
                    set_field_var = pu_var4 as *const u64;
                    if (*pu_var4).unk_4_0x18 == 0x13 && (*pu_var4).unk_3_0x10 == 2 {
                        break;
                    }
                    pi_var1 = (*pu_var4).unk_5_0x1c as *const unk_struct;
                    pu_var4 = ((pu_var4 as u64) + (*pi_var1).unk_1_0x0) as *const unk_struct;
                    set_field_var = lua_nil_addr; // &LUA_NIL
                    if *(pi_var1 as *const u32) == 0 {
                        break;
                    }
                }
            } else {
                set_field_var = ((*lua_r_udata).unk_3_0x10 + 0x10) as *const u64;
            }

            self.set_field(set_field_var, &normal);
        }
    }

    pub fn add_ingame_manager(&mut self, name: impl AsRef<str>, registry: &[luaL_Reg]) {
        let mut enum_builder = LuaEnumBuilder::new();
        enum_builder.declare_namespace(Some(self), name);
        for reg in registry.iter() {
            enum_builder.add_method(reg);
        }
    }
}
