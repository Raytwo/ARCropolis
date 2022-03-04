use crate::{chainloader::*, PathExtension};
use smash_arc::Hash40;
use skyline::nn::{self, ro::*};
use std::path::{Path, PathBuf};

use orbits::{Tree, FileLoader, LaunchPad, StandardLoader, ConflictHandler, ConflictKind};
use crate::config;

use std::collections::{ HashMap, HashSet };

lazy_static! {
    static ref PRESET_HASHES: HashSet<Hash40> = {
        let mut storage = skyline_config::acquire_storage("arcropolis").unwrap();

        let presets = match storage.get_field_json("presets") {
            Ok(presets) => presets,
            Err(_) => {
                let empty_presets: HashSet<Hash40> = HashSet::new();
                storage.set_field_json("presets", &empty_presets);
                empty_presets
            },
        };
        presets
    };
}

pub fn perform_discovery() -> LaunchPad<StandardLoader> {
    let is_emulator = unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64 } == 0x8004000;

    if is_emulator {
        println!("Emulator usage detected in perform_discovery, reverting to old behavior.");
    }

    let filter = |path: &Path| {
        // If we're not running on emulator
        if !is_emulator && !config::legacy_discovery() {
            // If it's not in the presets, don't load
            trace!("New discovery system");
            PRESET_HASHES.contains(&Hash40::from(path.to_str().unwrap()))
        } else {
            // Legacy filter, load the mod except if it has a period at the start of the name
            trace!("Legacy discovery system");

            path
                .file_name()
                .map(|name| name.to_str())
                .flatten()
                .map(|name| !name.starts_with("."))
                .unwrap_or(false)
        }
        
    };

    let ignore = |path: &Path| {
        let name = if let Some(name) = path.file_name().map(|x| x.to_str()).flatten() {
            name
        } else {
            return false;
        };

        let is_root = path
            .parent()
            .map(|parent| parent.as_os_str().is_empty())
            .unwrap_or(true);

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
                RESERVED_NAMES.contains(&name) || name.ends_with("prcx") || name.ends_with("prctxt") || name.ends_with("xmsbt")
            },
            _ => false
        }
    };

    let mut launchpad = LaunchPad::new(StandardLoader, ConflictHandler::NoRoot);

    let arc_path = config::arc_path();

    launchpad.collecting(collect);
    launchpad.ignoring(ignore);

    let mut conflicts = if std::fs::try_exists(arc_path).unwrap_or(false) {
        launchpad.discover_in_root(config::arc_path())
    } else {
        Vec::new()
    };

    let umm_path = config::umm_path();
    if std::fs::try_exists(umm_path).unwrap_or(false) {
        conflicts.extend(
            launchpad
                .discover_roots(config::umm_path(), 1, filter),
        );
    }

    for path in config::extra_paths() {
        if std::fs::try_exists(&path).unwrap_or(false) {
            conflicts.extend(launchpad.discover_roots(&path, 1, filter));
        }
    }

    let should_prompt = !conflicts.is_empty();

    for conflict in conflicts.into_iter() {
        match conflict {
            ConflictKind::StandardConflict { error_root, source_root, local } => warn!(
                "File '{}' was rejected for file '{}' during discovery.",
                error_root.join(&local).display(),
                source_root.join(local).display()
            ),
            ConflictKind::RootConflict(root_path, kept) => warn!(
                "Mod root '{}' was rejected for a file conflict with '{}' during discovery.",
                root_path.display(),
                kept.display()
            )
        }
    }

    if should_prompt {
        if skyline_web::Dialog::yes_no("During file discovery, ARCropolis encountered file conflicts.<br>Do you want to run it again to list all file conflicts?") {
            let mut launchpad = LaunchPad::new(StandardLoader, ConflictHandler::First);

            let arc_path = config::arc_path();

            launchpad.collecting(collect);
            launchpad.ignoring(ignore);

            let mut conflicts = if std::fs::try_exists(arc_path).unwrap_or(false) {
                launchpad.discover_in_root(config::arc_path())
            } else {
                Vec::new()
            };
        
            let umm_path = config::umm_path();
            if std::fs::try_exists(umm_path).unwrap_or(false) {
                conflicts.extend(
                    launchpad
                        .discover_roots(config::umm_path(), 1, filter),
                );
            }
        
            for path in config::extra_paths() {
                if std::fs::try_exists(&path).unwrap_or(false) {
                    conflicts.extend(launchpad.discover_roots(&path, 1, filter));
                }
            }

            let mut conflict_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

            for conflict in conflicts.into_iter() {
                match conflict {
                    ConflictKind::StandardConflict { error_root, local, source_root } => {
                        if let Some(conflicting_mods) = conflict_map.get_mut(&local) {
                            conflicting_mods.push(error_root);
                        } else {
                            conflict_map.insert(local, vec![source_root, error_root]);
                        }
                    },
                    _ => {}
                }
            }

            let should_log = match serde_json::to_string_pretty(&conflict_map) {
                Ok(json) => match std::fs::write("sd:/ultimate/arcropolis/conflicts.json", json.as_bytes()) {
                    Ok(_) => {
                        crate::dialog_error("Please check sd:/ultimate/arcropolis/conflicts.json for all of the file conflicts.");
                        false
                    },
                    Err(e) => {
                        crate::dialog_error(format!("Failed to write conflict map to sd:/ultimate/arcropolis/conflicts.json<br>{:?}", e));
                        true
                    }
                },
                Err(e) => {
                    crate::dialog_error(format!("Failed to serialize conflict map to JSON. {:?}", e));
                    true
                }
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
    }

    match mount_prebuilt_nrr(launchpad.tree()) {
        Ok(Some(_)) => info!("Successfully registered fighter modules."),
        Ok(_) => info!("No fighter modules found to register."),
        Err(e) => {
            error!("{:?}", e);
            crate::dialog_error("ARCropolis failed to register module information for fighter modules.<br>You may experience infinite loading on some fighters.");
        }
    }

    load_and_run_plugins(launchpad.collected_paths());

    launchpad
}

fn mount_prebuilt_nrr<A: FileLoader>(tree: &Tree<A>) -> Result<Option<RegistrationInfo>, NrrRegistrationFailedError>
where <A as FileLoader>::ErrorType: std::fmt::Debug {
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
            _ => {}
        }
    });

    fighter_nro_nrr.register()
}



pub fn load_and_run_plugins(plugins: &Vec<(PathBuf, PathBuf)>) {
    let mut plugin_nrr = NrrBuilder::new();

    let modules: Vec<NroBuilder> = plugins.iter().filter_map(|(root, local)| {
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
                }
            }
        } else {
            error!("File discovery collected path '{}' but it does not exist and/or is invalid!", full_path.display());
            None
        }
    }).collect();

    if modules.is_empty() {
        info!("No plugins found for chainloading.");
        return;
    }

    let mut registration_info = match plugin_nrr.register() {
        Ok(Some(info)) => info,
        Ok(_) => return,
        Err(e) => {
            error!("{:?}", e);
            crate::dialog_error("ARCropolis failed to register plugin module info.");
            return;
        }
    };

    let modules: Vec<Module> = modules.into_iter().filter_map(|x| {
        match x.mount() {
            Ok(module) => Some(module),
            Err(e) => {
                error!("Failed to mount chainloaded plugin. {:?}", e);
                None
            }
        }
    }).collect();

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