use skyline::{hook, install_hook};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::ffi::CString;


pub mod lua;
use crate::lua::lua::{lua_state, luaL_Reg, luaL_Reg_container};

static LUA_MANAGERS: Lazy<RwLock<HashMap<&'static str, Vec<luaL_Reg_container>>>> = Lazy::new(|| RwLock::new(HashMap::new()));

static INSTALLED_MANAGERS: Lazy<RwLock<Vec<u64>>> = Lazy::new(|| RwLock::new(Vec::new()));

fn clean_installed_managers(){
    unsafe {
        for ptr in INSTALLED_MANAGERS.read().iter() {
            let _ = CString::from_raw(*ptr as _);
        }
        INSTALLED_MANAGERS.write().clear();
    }
}

fn string_to_static_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub fn add_lua_manager(name: impl AsRef<str>, registry: Vec<luaL_Reg_container>) -> bool {
    unsafe {
        let mut lua_managers = LUA_MANAGERS.write();
        match lua_managers.try_insert(string_to_static_str(name.as_ref().to_string()), registry) {
            Ok(_s) => true,
            Err(_err) => false
        }
    }
}

#[hook(offset = 0x33702b0)]
fn apply_ui2d_layout_bindings(lua_state: &mut lua_state) {
    clean_installed_managers();
    original!()(lua_state);
    let lua_managers = LUA_MANAGERS.read();
    for (key, value) in lua_managers.iter() {
        let mut functions = value.iter().map(|x| luaL_Reg {
            name: {
                let c_str = CString::new(format!("{}", x.name)).expect(&format!("Failed to make a CString from {}!", x.name));
                let raw = c_str.into_raw();
                INSTALLED_MANAGERS.write().push(raw as _);
                raw as _
            },
            func: x.func,
        }).collect::<Vec<luaL_Reg>>();

        functions.push(
            luaL_Reg {
                name: std::ptr::null(),
                func: None,
            }
        );

        lua_state.add_manager(key, &functions);
    }
}


pub fn install() {
    install_hook!(apply_ui2d_layout_bindings);
}