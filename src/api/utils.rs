#[no_mangle]
pub extern "C" fn arcrop_show_mod_manager() {
    debug!("arcrop_show_mod_manager -> Function called");
    crate::menus::show_arcadia(None);
}

#[no_mangle]
pub extern "C" fn arcrop_show_config_editor() {
    debug!("arcrop_show_config_editor -> Function called");
    crate::menus::show_config_editor(&mut crate::config::GLOBAL_CONFIG.lock().unwrap());
}

#[no_mangle]
pub extern "C" fn arcrop_show_main_menu() {
    debug!("arcrop_show_main_menu -> Function called");
    crate::menus::show_main_menu();
}
