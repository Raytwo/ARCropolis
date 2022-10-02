use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use once_cell::sync::Lazy;
use orbits::{ConflictHandler, ConflictKind, FileLoader, LaunchPad, StandardLoader, Tree};
use skyline::nn::{self, ro::*};
use smash_arc::Hash40;

use crate::{chainloader::*, config, utils};

pub fn perform_discovery() -> LaunchPad<StandardLoader> {
    let is_ryujinx = utils::env::is_ryujinx();

    if is_ryujinx {
        info!("Emulator usage detected in perform_discovery, reverting to old behavior.");
    }

    let legacy_discovery = config::legacy_discovery();

    if !is_ryujinx {
        // Open the ARCropolis menu if Minus is held before mod discovery
        if ninput::any::is_down(ninput::Buttons::PLUS) {
            crate::menus::show_main_menu();
        }
    }

    let presets = crate::config::presets::get_active_preset().unwrap();

    let filter = |path: &Path| {
        // If we're not running on emulator
        if !is_ryujinx && !legacy_discovery {
            // If it's not in the presets, don't load
            presets.contains(&Hash40::from(path.to_str().unwrap()))
        } else {
            // Legacy filter, load the mod except if it has a period at the start of the name

            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| !name.starts_with('.'))
                .unwrap_or(false)
        }
    };

    let ignore = |path: &Path| {
        let name = if let Some(name) = path.file_name().and_then(|x| x.to_str()) { name } else { return false };

        let is_root = path.parent().map(|parent| parent.as_os_str().is_empty()).unwrap_or(true);

        let is_dot = name.starts_with('.');

        let is_out_of_region = if let Some(index) = name.find('+') {
            let (_, end) = name.split_at(index + 1);
            !end.starts_with(&config::region().to_string())
        } else {
            false
        };

        is_root || is_dot || is_out_of_region
    };

    let collect = |x: &Path| {
        match x.file_name() {
            Some(name) if let Some(name) = name.to_str() => {
                static RESERVED_NAMES: &[&str] = &[
                    "config.json",
                    "plugin.nro",
                ];
                static PATCH_EXTENSIONS: &[&str] = &[
                    "prcx",
                    "prcxml",
                    "stdatx",
                    "stdatxml",
                    "stprmx",
                    "stprmxml",

                    "xmsbt",

                    "patch3audio",

                    "xbgm"
                ];
                RESERVED_NAMES.contains(&name) || PATCH_EXTENSIONS.iter().any(|x| name.ends_with(x))
            },
            _ => false
        }
    };

    let mods_path = utils::paths::mods();

    // Emulators can't use presets, so don't run this logic
    if !is_ryujinx && !legacy_discovery {
        let mut storage = config::GLOBAL_CONFIG.lock().unwrap();
        // Get the mod cache from last run
        let mod_cache: HashSet<Hash40> = storage.get_field_json("mod_cache").unwrap_or_default();

        // Inspect the list of mods to see if some are new ones
        let new_cache: HashSet<Hash40> = std::fs::read_dir(&mods_path)
            .unwrap()
            .filter_map(|path| {
                let path = PathBuf::from(&mods_path).join(path.unwrap().path());

                if path.is_file() {
                    None
                } else {
                    Some(Hash40::from(path.to_str().unwrap()))
                }
            })
            .collect();

        // Get the workspace name and workspace list
        let workspace_name: String = storage.get_field("workspace").unwrap_or_else(|_| "Default".to_string());
        let workspace_list: HashMap<String, String> = storage.get_field_json("workspace_list").unwrap_or_default();

        // Get the preset name from the workspace list
        let preset_name = &workspace_list[&workspace_name];
        let mut presets: HashSet<Hash40> = storage.get_field_json(preset_name).unwrap_or_default();
        let new_mods: HashSet<&Hash40> = new_cache
            .iter()
            .filter(|cached_mod| !mod_cache.contains(cached_mod) && !presets.contains(cached_mod))
            .collect();

        // We found hashes that weren't in the cache
        if !new_mods.is_empty() && skyline_web::Dialog::yes_no("New mods have been detected.\nWould you like to enable them?") {
            // Add the new mods to the presets file
            presets.extend(new_mods);
            // Save it back
            storage.set_field_json(preset_name, &presets).unwrap();
        }

        // No matter what, the cache has to be updated
        storage.set_field_json("mod_cache", &new_cache).unwrap();
    }

    let mut launchpad = LaunchPad::new(StandardLoader, ConflictHandler::NoRoot);

    launchpad.collecting(collect);
    launchpad.ignoring(ignore);

    let conflicts = launchpad.discover_roots(&mods_path, 1, filter);

    let should_prompt = !conflicts.is_empty();

    for conflict in conflicts.into_iter() {
        match conflict {
            ConflictKind::StandardConflict {
                error_root,
                source_root,
                local,
            } => {
                warn!(
                    "File '{}' was rejected for file '{}' during discovery.",
                    error_root.join(&local).display(),
                    source_root.join(local).display()
                )
            },
            ConflictKind::RootConflict(root_path, kept) => {
                warn!(
                    "Mod root '{}' was rejected for a file conflict with '{}' during discovery.",
                    root_path.display(),
                    kept.display()
                )
            },
        }
    }

    if should_prompt
        && skyline_web::Dialog::yes_no(
            "During file discovery, ARCropolis encountered file conflicts.<br>Do you want to run it again to list all file conflicts?",
        )
    {
        let mut launchpad = LaunchPad::new(StandardLoader, ConflictHandler::First);

        launchpad.collecting(collect);
        launchpad.ignoring(ignore);

        let conflicts = launchpad.discover_roots(utils::paths::mods(), 1, filter);

        let mut conflict_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

        for conflict in conflicts.into_iter() {
            if let ConflictKind::StandardConflict {
                error_root,
                local,
                source_root,
            } = conflict
            {
                if let Some(conflicting_mods) = conflict_map.get_mut(&local) {
                    conflicting_mods.push(error_root);
                } else {
                    conflict_map.insert(local, vec![source_root, error_root]);
                }
            }
        }

        let should_log = match serde_json::to_string_pretty(&conflict_map) {
            Ok(json) => {
                match std::fs::write("sd:/ultimate/arcropolis/conflicts.json", json.as_bytes()) {
                    Ok(_) => {
                        crate::dialog_error("Please check sd:/ultimate/arcropolis/conflicts.json for all of the file conflicts.");
                        false
                    },
                    Err(e) => {
                        crate::dialog_error(format!(
                            "Failed to write conflict map to sd:/ultimate/arcropolis/conflicts.json<br>{:?}",
                            e
                        ));
                        true
                    },
                }
            },
            Err(e) => {
                crate::dialog_error(format!("Failed to serialize conflict map to JSON. {:?}", e));
                true
            },
        };

        if should_log {
            for (local, roots) in conflict_map {
                error!("The file {} is used by the following roots:", local.display());
                for root in roots {
                    error!("{}", root.display());
                }
            }
        }
    }

    match mount_prebuilt_nrr(launchpad.tree()) {
        Ok(Some(_)) => info!("Successfully registered fighter modules."),
        Ok(_) => info!("No fighter modules found to register."),
        Err(e) => {
            error!("{:?}", e);
            crate::dialog_error(
                "ARCropolis failed to register module information for fighter modules.<br>You may experience infinite loading on some fighters.",
            );
        },
    }

    load_and_run_plugins(launchpad.collected_paths());

    launchpad
}

fn mount_prebuilt_nrr<A: FileLoader>(tree: &Tree<A>) -> Result<Option<RegistrationInfo>, NrrRegistrationFailedError>
where
    <A as FileLoader>::ErrorType: std::fmt::Debug,
{
    let fighter_nro_parent = Path::new("prebuilt;/nro/release");
    let mut fighter_nro_nrr = NrrBuilder::new();

    tree.walk_paths(|node, entry_type| {
        match node.get_local().parent() {
            Some(parent) if entry_type.is_file() && parent == fighter_nro_parent => {
                info!("Reading '{}' for module registration.", node.full_path().display());
                if let Ok(data) = std::fs::read(node.full_path()) {
                    fighter_nro_nrr.add_module(data.as_slice());
                }
            },
            _ => {},
        }
    });

    fighter_nro_nrr.register()
}

pub fn load_and_run_plugins(plugins: &[(PathBuf, PathBuf)]) {
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

    // we have to do it this way
    // i'm sorry ray, but it literally does not work without collecting here
    // i don't know
    // i didn't write hos
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

    for module in modules {
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
