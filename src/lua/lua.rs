use skyline::{from_offset, hooks::{getRegionAddress, Region}};

pub type LuaCfunction = ::std::option::Option<unsafe extern "C" fn(L: &mut lua_state) -> ::std::os::raw::c_int>;
pub type LMem = u64;

#[from_offset(0x38f5c30)]
fn lua_l_newmetatable(lua_state: &mut lua_state, name: *const u8);

#[from_offset(0x38f3d60)]
fn lua_setfield(lua_state: &mut lua_state, unk_1: *const u64, name: *const u8);

#[from_offset(0x38f7ee0)]
fn lua_l_setfuncs(lua_state: &mut lua_state, regs: *const u64, index: u32);

#[from_offset(0x38fd610)]
fn lua_c_step(lua_state: &mut lua_state);

#[from_offset(0x391ca20)]
fn lua_h_new(lua_state: &mut lua_state) -> *const u64;

#[from_offset(0x38f3710)]
fn lua_getfield(lua_state: &mut lua_state, lua_registry: *const TValue, name: *const u8);

#[from_offset(0x38f45d0)]
fn lua_setmetatable(lua_state: &mut lua_state, obj_idx: i32);

#[from_offset(0x38f2e10)]
fn lua_tonumberx(lua_state: &mut lua_state, idx: i32, unk: *const u64) -> f32;

#[from_offset(0x38f2f80)]
fn lua_tointegerx(lua_state: &mut lua_state, idx: i32, unk: *const u64) -> u64;

#[from_offset(0x38f3100)]
fn lua_tolstring(lua_state: &mut lua_state, idx: i32, unk: *const u64) -> *const u8;

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct TValue {
    pub udata: u64,
    pub tt: u32
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
    pub name: *mut u8,
    pub func: LuaCfunction,
}

#[repr(C)]
#[derive(Debug)]
pub struct lua_state {
    pub unk: [u8; 0xF],
    pub top_ptr: *mut TValue,
    pub global_state: &'static mut global_state,
    pub unk_2: [u8; 176]
}

#[repr(C)]
#[derive(Debug)]
pub struct global_state {
    pub unk: [u8; 0x17],
    pub gc_debt: LMem,
    pub unk_2: [u8; 0x20],
    pub l_registry: TValue,
    pub unk_3: [u8; 0xA9]
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

impl lua_state {
    pub fn get_current_top(&mut self) -> TValue {
        unsafe {
            *(self.top_ptr)
        }
    }
    pub fn get_previous_top(&mut self) -> TValue {
        unsafe {
            *(self.top_ptr).sub(1)
        }
    }
    pub fn update_current_top(&mut self, new: &TValue){
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
    pub fn set_top_field(&mut self, index: i32, field: impl AsRef<str>){
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
        unsafe {
            lua_h_new(self)
        }
    }
    pub fn step(&mut self) {
        unsafe {
            lua_c_step(self)
        }
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
        unsafe {
            lua_l_newmetatable(self, format!("{}\0", field.as_ref()).as_ptr() as _)
        }
    }
    pub fn set_funcs(&mut self, funcs: &[luaL_Reg]){
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

    pub fn add_manager(&mut self, name: impl AsRef<str>, registry: &[luaL_Reg]){
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
            top.tt = 69; // CollectableTableType
            
            self.update_current_top(&top);
            self.increment_top_address();
            
            let lua_registry = self.global_state.l_registry;
            
            self.get_field(&lua_registry, &metatable);
            self.set_metatable(-2);
            
            let mut set_field_var = std::ptr::null();
            
            let pv_var7 = lua_registry.udata;

            
            let mut pu_var4: *const unk_struct = std::ptr::null();
            let mut pi_var1: *const unk_struct = std::ptr::null();

            let lua_nil_addr = getRegionAddress(Region::Text) as u64 + 0x4860ab0;
            let lua_r_udata = lua_registry.udata as *const unk_udata_struct;

            if (*lua_r_udata).unk_2_0xc < 2 {
                panic!("lua_r_udata.unk_2_0xc is below a 2!");
                // Implementation is copied word for word (and translated) from Ghidra.
                // I have no idea what the game is doing here, but this is replicating what its doing
                // You can see this being done in the `apply_ui2d_layout_bindings` function (0x33702b0 in Smash ver 13.0.1) after the metatable is set
                pu_var4 = ((*lua_r_udata).unk_4_0x18
                        +
                        (!((0xffffffff as u64) << ((*lua_r_udata).unk_1_0xb as u64 & 0x3f)) as u64 & 2) * 0x20) as *const unk_struct;
                loop {
                    set_field_var = pu_var4 as *const u64;
                    if (*pu_var4).unk_4_0x18 == 0x13 && (*pu_var4).unk_3_0x10 == 2 {
                        break;
                    }
                    pi_var1 = (*pu_var4).unk_5_0x1c as *const unk_struct;
                    pu_var4 = ((pu_var4 as u64) + (*pi_var1).unk_1_0x0) as *const unk_struct;
                    set_field_var = lua_nil_addr as *const u64; // &LUA_NIL
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
}