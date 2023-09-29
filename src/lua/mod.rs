use skyline::{hook, install_hooks};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::ffi::CString;


pub mod lua;
use crate::lua::lua::{lua_state, luaL_Reg, luaL_Reg_container};
use crate::offsets;

static LUA_MENU_MANAGERS: Lazy<RwLock<HashMap<&'static str, Vec<luaL_Reg_container>>>> = Lazy::new(|| RwLock::new(HashMap::new()));
static LUA_INGAME_MANAGERS: Lazy<RwLock<HashMap<&'static str, Vec<luaL_Reg_container>>>> = Lazy::new(|| RwLock::new(HashMap::new()));

static INSTALLED_MENU_MANAGERS: Lazy<RwLock<Vec<u64>>> = Lazy::new(|| RwLock::new(Vec::new()));
static INSTALLED_INGAME_MANAGERS: Lazy<RwLock<Vec<u64>>> = Lazy::new(|| RwLock::new(Vec::new()));

fn clean_managers(manager: &Lazy<RwLock<Vec<u64>>>){
    unsafe {
        for ptr in manager.read().iter() {
            let _ = CString::from_raw(*ptr as _);
        }
        manager.write().clear();
    }
}

fn string_to_static_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub fn add_lua_menu_manager(name: impl AsRef<str>, registry: Vec<luaL_Reg_container>) -> bool {
    let mut lua_menu_managers = LUA_MENU_MANAGERS.write();
    match lua_menu_managers.try_insert(string_to_static_str(name.as_ref().to_string()), registry) {
        Ok(_s) => true,
        Err(_err) => false
    }
}

#[hook(offset = offsets::lua_ui2d_bindings())]
fn apply_ui2d_layout_bindings(lua_state: &mut lua_state) {
    clean_managers(&INSTALLED_MENU_MANAGERS);
    original!()(lua_state);
    let lua_menu_managers = LUA_MENU_MANAGERS.read();
    for (key, value) in lua_menu_managers.iter() {
        let mut functions = value.iter().map(|x| luaL_Reg {
            name: {
                let c_str = CString::new(format!("{}", x.name)).expect(&format!("Failed to make a CString from {}!", x.name));
                let raw = c_str.into_raw();
                INSTALLED_MENU_MANAGERS.write().push(raw as _);
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

        lua_state.add_menu_manager(key, &functions);
    }
}


pub fn add_lua_ingame_manager(name: impl AsRef<str>, registry: Vec<luaL_Reg_container>) -> bool {
    let mut lua_ingame_managers = LUA_INGAME_MANAGERS.write();
    match lua_ingame_managers.try_insert(string_to_static_str(name.as_ref().to_string()), registry) {
        Ok(_s) => true,
        Err(_err) => false
    }
}

#[hook(offset = offsets::lua_ingame_bindings())]
fn apply_ingame_bindings(lua_state: &mut lua_state) {
    clean_managers(&INSTALLED_INGAME_MANAGERS);
    original!()(lua_state);
    let lua_ingame_managers = LUA_INGAME_MANAGERS.read();
    for (key, value) in lua_ingame_managers.iter() {
        let functions = value.iter().map(|x| luaL_Reg {
            name: {
                let c_str = CString::new(format!("{}", x.name)).expect(&format!("Failed to make a CString from {}!", x.name));
                let raw = c_str.into_raw();
                INSTALLED_INGAME_MANAGERS.write().push(raw as _);
                raw as _
            },
            func: x.func,
        }).collect::<Vec<luaL_Reg>>();

        lua_state.add_ingame_manager(key, &functions);
    }
}


pub fn install() {
    install_hooks!(apply_ui2d_layout_bindings, apply_ingame_bindings,);
}