use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use skyline::nn::{self, ro::*};
use smash_arc::Hash40;

use crate::{chainloader::*, config};

lazy_static! {
    static ref PRESET_HASHES: HashSet<Hash40> = {
        let presets = crate::config::presets::get_active_preset().unwrap();

        trace!("Presets count: {}", presets.len());
        presets
    };

pub fn perform_discovery() {
    let is_emulator = crate::util::env::is_emulator();

    if is_emulator {
        info!("Emulator usage detected in perform_discovery, reverting to old behavior.");
    }

    let legacy_discovery = config::legacy_discovery();

    if !is_emulator {
        // Open the ARCropolis menu if Minus is held before mod discovery
        if ninput::any::is_down(ninput::Buttons::PLUS) {
            crate::menus::show_main_menu();
        }
    }

    let filter = |path: &Path| {
        // If we're not running on emulator
        if !is_emulator && !legacy_discovery {
            // If it's not in the presets, don't load
            PRESET_HASHES.contains(&Hash40::from(path.to_str().unwrap()))
        } else {
            // Legacy filter, load the mod except if it has a period at the start of the name

            path.file_name()
                .map(|name| name.to_str())
                .flatten()
                .map(|name| !name.starts_with("."))
                .unwrap_or(false)
        }
    };

    let ignore = |path: &Path| {
        let name = if let Some(name) = path.file_name().map(|x| x.to_str()).flatten() { name } else { return false };

        let is_root = path.parent().map(|parent| parent.as_os_str().is_empty()).unwrap_or(true);

        let is_dot = name.starts_with(".");

        let is_out_of_region = if let Some(index) = name.find("+") {
            let (_, end) = name.split_at(index + 1);
            !end.starts_with(&config::region_str())
        } else {
            false
        };

        is_root || is_dot || is_out_of_region
    };

    let collect = |x: &Path| {
        match x.file_name() {
            Some(name) if let Some(name) = name.to_str() => {
                static RESERVED_NAMES: &[&'static str] = &[
                    "config.json",
                    "plugin.nro",
                ];
                static PATCH_EXTENSIONS: &[&'static str] = &[
                    "prcx",
                    "prcxml",
                    "stdatx",
                    "stdatxml",
                    "stprmx",
                    "stprmxml",

                    "xmsbt"
                ];
                RESERVED_NAMES.contains(&name) || PATCH_EXTENSIONS.iter().any(|x| name.ends_with(x))
            },
            _ => false
        }
    };

    let arc_path = config::arc_path();
    let umm_path = config::umm_path();

    // Emulators can't use presets, so don't run this logic
    if !is_emulator && !legacy_discovery {
        let mut storage = config::GLOBAL_CONFIG.write();
        // Get the mod cache from last run
        let mut mod_cache: HashSet<Hash40> = storage.get_field_json("mod_cache").unwrap_or_default();

        // Inspect the list of mods to see if some are new ones
        let new_cache: HashSet<Hash40> = std::fs::read_dir(&umm_path)
            .unwrap()
            .filter_map(|path| {
                let path = PathBuf::from(&umm_path).join(path.unwrap().path());

                if path.is_file() {
                    None
                } else {
                    Some(Hash40::from(path.to_str().unwrap()))
                }
            })
            .collect();

        // Get the workspace name and workspace list
        let workspace_name: String = storage.get_field("workspace").unwrap_or("Default".to_string());
        let workspace_list: HashMap<String, String> = storage.get_field_json("workspace_list").unwrap_or_default();

        // Get the preset name from the workspace list
        let preset_name = &workspace_list[&workspace_name];
        let mut presets: HashSet<Hash40> = storage.get_field_json(preset_name).unwrap_or_default();
        let new_mods: HashSet<&Hash40> = new_cache
            .iter()
            .filter(|cached_mod| !mod_cache.contains(cached_mod) && !presets.contains(cached_mod))
            .collect();

        // We found hashes that weren't in the cache
        if !new_mods.is_empty() {
            if skyline_web::Dialog::yes_no("New mods have been detected.\nWould you like to enable them?") {
                // Add the new mods to the presets file
                presets.extend(new_mods);
                // Save it back
                storage.set_field_json(preset_name, &presets).unwrap();
            }
        }

        // No matter what, the cache has to be updated
        storage.set_field_json("mod_cache", &new_cache).unwrap();
    }

    // I'm well aware this sucks, but the stack size in main is too small to do it there.
    let mut storage = config::GLOBAL_CONFIG.write();

    if storage.get_flag("first_boot") {
        if skyline_web::Dialog::yes_no("A default configuration for ARCropolis has been created.<br>It is important that both your region & language in this config match your Smash copy.<br>By default, it is set to American English. Would you like to adjust it?") {
            crate::menus::show_config_editor(&mut storage);
        }
        storage.set_flag("first_boot", false).unwrap();
    }

    drop(storage);

    // TODO: Discovered, conflicting, ignored file operations go here

    // TODO: Reimplement NRR stuff

    // match mount_prebuilt_nrr(launchpad.tree()) {
    //     Ok(Some(_)) => info!("Successfully registered fighter modules."),
    //     Ok(_) => info!("No fighter modules found to register."),
    //     Err(e) => {
    //         error!("{:?}", e);
    //         crate::dialog_error(
    //             "ARCropolis failed to register module information for fighter modules.<br>You may experience infinite loading on some fighters.",
    //         );
    //     },
    // }

    //load_and_run_plugins(launchpad.collected_paths());

}

// fn mount_prebuilt_nrr<A: FileLoader>(tree: &Tree<A>) -> Result<Option<RegistrationInfo>, NrrRegistrationFailedError>
// where
//     <A as FileLoader>::ErrorType: std::fmt::Debug,
// {
//     let fighter_nro_parent = Path::new("prebuilt;/nro/release");
//     let mut fighter_nro_nrr = NrrBuilder::new();

//     tree.walk_paths(|node, entry_type| match node.get_local().parent() {
//         Some(parent) if entry_type.is_file() && parent == fighter_nro_parent => {
//             info!("Reading '{}' for module registration.", node.full_path().display());
//             if let Ok(data) = std::fs::read(node.full_path()) {
//                 fighter_nro_nrr.add_module(data.as_slice());
//             }
//         },
//         _ => {},
//     });

//     fighter_nro_nrr.register()

pub fn load_and_run_plugins(plugins: &Vec<(PathBuf, PathBuf)>) {
    let mut plugin_nrr = NrrBuilder::new();

    let modules: Vec<NroBuilder> = plugins
        .iter()
        .filter_map(|(root, local)| {
            let full_path = root.join(local);

            if full_path.exists() && full_path.ends_with("plugin.nro") {
                match NroBuilder::open(&full_path) {
                    Ok(builder) => {
                        info!("Loaded plugin at '{}' for chainloading.", full_path.display());
                        plugin_nrr.add_module(&builder);
                        Some(builder)
                    },
                    Err(e) => {
                        error!("Failed to load plugin at '{}'. {:?}", full_path.display(), e);
                        None
                    },
                }
            } else {
                error!(
                    "File discovery collected path '{}' but it does not exist and/or is invalid!",
                    full_path.display()
                );
                None
            }
        })
        .collect();

    if modules.is_empty() {
        info!("No plugins found for chainloading.");
        return
    }

    let mut registration_info = match plugin_nrr.register() {
        Ok(Some(info)) => info,
        Ok(_) => return,
        Err(e) => {
            error!("{:?}", e);
            crate::dialog_error("ARCropolis failed to register plugin module info.");
            return
        },
    };

    let modules: Vec<Module> = modules
        .into_iter()
        .filter_map(|x| {
            match x.mount() {
                Ok(module) => Some(module),
                Err(e) => {
                    error!("Failed to mount chainloaded plugin. {:?}", e);
                    None
                },
            }
        })
        .collect();

    unsafe {
        // Unfortunately, without unregistering this it will cause the game to crash, cause is unknown, but likely due to page alignment I'd guess
        // It does not matter if we use only one NRR for both the prebuilt modules and the plugins, it will still cause a crash
        nn::ro::UnregisterModuleInfo(&mut registration_info);
    }

    // 3.0.0: The plugins are apparently loaded despite the mismatch in module vs plugin count, leaving it here for now
    // if modules.len() < plugins.len() {
    //     crate::dialog_error("ARCropolis failed to load/mount some plugins.");
    // } else {
    info!("Successfully chainloaded all collected plugins.");
    // }

    for module in modules.into_iter() {
        let callable = unsafe {
            let mut sym_loc = 0usize;
            let rc = nn::ro::LookupModuleSymbol(&mut sym_loc, &module, "main\0".as_ptr() as _);
            if rc != 0 {
                warn!("Failed to find symbol 'main' in chainloaded plugin.");
                None
            } else {
                Some(std::mem::transmute::<usize, extern "C" fn()>(sym_loc))
            }
        };

        if let Some(entrypoint) = callable {
            info!("Calling 'main' in chainloaded plugin");
            entrypoint();
            info!("Finished calling 'main' in chainloaded plugin");
        }
    }
}
