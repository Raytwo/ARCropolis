use crate::lua::lua::{lua_state, luaL_Reg_container, luaL_Reg_from_api};
use std::ffi::CString;

#[no_mangle]
pub extern "C" fn arcorp_add_lua_manager(name: *mut u8, reg_vec_ptr: *mut luaL_Reg_from_api, reg_vec_size: usize, reg_vec_cap: usize) -> bool {
    debug!("arcorp_add_lua_manager -> Function called");
    unsafe {
        match CString::from_raw(name).to_str() {
            Ok(s) => {
                let name = s.to_string();
                let registry = Vec::from_raw_parts(reg_vec_ptr, reg_vec_size, reg_vec_cap);
                
                let functions = registry.iter().map(|x| {
                    luaL_Reg_container {
                        name: CString::from_raw(x.name).to_str().unwrap().to_string(),
                        func: x.func
                    }
                }).collect::<Vec<luaL_Reg_container>>();

                crate::lua::add_lua_manager(name, functions)
            },
            Err(err) => {
                false
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn arcrop_lua_state_get_string(lua_state: &mut lua_state) -> *const u8 {
    debug!("arcrop_lua_state_get_string -> Function called");
    lua_state.get_string_arg_ptr()
}

#[no_mangle]
pub extern "C" fn arcrop_lua_state_get_number(lua_state: &mut lua_state) -> f32 {
    debug!("arcrop_lua_state_get_number -> Function called");
    lua_state.get_number_arg()
}

#[no_mangle]
pub extern "C" fn arcrop_lua_state_get_integer(lua_state: &mut lua_state) -> u64 {
    debug!("arcrop_lua_state_get_integer -> Function called");
    lua_state.get_integer_arg()
}