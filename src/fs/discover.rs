use std::{
    collections::HashMap,
};

use walkdir::WalkDir;
use camino::{Utf8Path, Utf8PathBuf};

use smash_arc::{Hash40, hash40};

use crate::fs::{ModDir, Modpack};

use super::ModFile;

// pub const MAX_COMPONENT_COUNT: usize = 10;

// pub static INTERNER: Lazy<RwLock<Interner>> = Lazy::new(|| RwLock::new(Interner::new()));

pub fn perform_discovery() -> Modpack {
    // Maybe have some sort of FileWalker trait and two implementations for both legacy and modern? Sounds a bit overengineered but it'd allow for more fine-tuning per system.
    let umm_path = crate::utils::paths::mods();

    // if !crate::utils::env::is_ryujinx() {
    //     let mut storage = config::GLOBAL_CONFIG.write();
    //     // Get the mod cache from last run
    //     let mod_cache: HashSet<Hash40> = storage.get_field_json("mod_cache").unwrap_or_default();

    //     // Inspect the list of mods to see if some are new ones
    //     let new_cache: HashSet<Hash40> = std::fs::read_dir(&umm_path)
    //         .unwrap()
    //         .filter_map(|path| {
    //             let path = PathBuf::from(&umm_path).join(path.unwrap().path());

    //             if path.is_file() {
    //                 None
    //             } else {
    //                 Some(Hash40::from(path.to_str().unwrap()))
    //             }
    //         })
    //         .collect();

    //     // Get the workspace name and workspace list
    //     let workspace_name: String = storage.get_field("workspace").unwrap_or("Default".to_string());
    //     let workspace_list: HashMap<String, String> = storage.get_field_json("workspace_list").unwrap_or_default();

    //     // Get the preset name from the workspace list
    //     let preset_name = &workspace_list[&workspace_name];
    //     let presets: HashSet<Hash40> = storage.get_field_json(preset_name).unwrap_or_default();
    //     let _new_mods: HashSet<&Hash40> = new_cache
    //         .iter()
    //         .filter(|cached_mod| !mod_cache.contains(cached_mod) && !presets.contains(cached_mod))
    //         .collect();

    //     // We found hashes that weren't in the cache
    //     #[cfg(feature = "web")]
    //     if !new_mods.is_empty() {
    //         if skyline_web::Dialog::yes_no("New mods have been detected.\nWould you like to enable them?") {
    //             todo!("Reimplement new mod discovery so it takes workspaces into account");
    //             // Add the new mods to the presets file
    //             presets.extend(new_mods);
    //             // Save it back
    //             storage.set_field_json(preset_name, &presets).unwrap();
    //         }
    //     }

    //     // No matter what, the cache has to be updated
    //     storage.set_field_json("mod_cache", &new_cache).unwrap();
    // }
    let _fs = crate::GLOBAL_FILESYSTEM.write();

    discover_mods(umm_path)

    // let interner = INTERNER.read();

    // for path in paths {
    //     println!("{}", path.to_string(&interner));
    // }

    // TODO: Reimplement NRR stuff, but move it further in the process

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

    // load_and_run_plugins(launchpad.collected_paths());
    
}

/// Utility method to know if a path shouldn't be checked for conflicts
pub fn is_collectable(x: &Utf8Path) -> bool {
    match x.file_name() {
        Some(name) => {
            static RESERVED_NAMES: &[&str] = &["config.json", "plugin.nro"];

            static PATCH_EXTENSIONS: &[&str] = &["prcx", "prcxml", "stdatx", "stdatxml", "stprmx", "stprmxml", "xmsbt"];

            RESERVED_NAMES.contains(&name) || PATCH_EXTENSIONS.iter().any(|x| name.ends_with(x))
        },
        _ => false,
    }
}

pub fn discover_in_mods<P: AsRef<Utf8Path>>(root: P) -> ModDir {
    let root = root.as_ref();

    let mut files: Vec<ModFile> = Vec::new();
    let mut patches: Vec<Utf8PathBuf> = Vec::new();

    WalkDir::new(root).min_depth(1).into_iter().flatten().for_each(|entry| {
        // Ignore the directories, only care about the files
        if entry.file_type().is_file() && entry.path().extension().is_some() {
            let path = Utf8Path::from_path(entry.path()).unwrap();

            // Maybe move this later?
            // Is it one of the paths that we need to keep track of? (plugin, config, patches, ...)
            if is_collectable(path) {
                patches.push(path.into());
            } else {
                let (path, _) = crate::strip_region_from_path(path);
                files.push(ModFile { path, size: entry.metadata().unwrap().len() });
            }
        }
    });

    ModDir { root: root.to_owned(), files, patches }
}

pub fn discover_mods<P: AsRef<Utf8Path>>(root: P) -> Modpack {
    let root = root.as_ref();

    let presets = crate::config::presets::get_active_preset().unwrap();

    // let mut conflict_list: HashMap<Conflict, Vec<Utf8PathBuf>> = HashMap::new();

    let mods = WalkDir::new(root)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_entry(|entry| {
            // Make sure we ignore files if they are in the same directory where mods are stored.
            // Also make sure they are in the active presets of the user.
            entry.file_type().is_dir() && presets.contains(&hash40(entry.path().to_str().unwrap()))
        }).flatten()
        .map(|entry| {
            let path = Utf8Path::from_path(entry.path()).unwrap();

            discover_in_mods(path)

            // // Remove the conflicting files from mod_files and store them
            // let conflicts: HashMap<Hash40, Utf8PathBuf> = mod_files.files.drain_filter(|hash, _| files.contains_key(hash)).collect();

            // // TODO: Move this in initial_loading so we can display the conflict handler before the game loads files?
            // // If any file is conflicting with what we already have found, discard this mod and warn the user.
            // if !conflicts.is_empty() {
            //     conflicts.iter().for_each(|(hash, full_path)| {
            //         // The part of the path that is used to navigate data.arc
            //         let local_path = full_path.strip_prefix(path).unwrap();
            //         // Get the root of the mod we're conflicting with
            //         let first_mod_root = files.get(hash).unwrap().as_str().strip_suffix(local_path.as_str()).unwrap();

            //         let conflict = Conflict {
            //             conflicting_mod: path.strip_prefix("sd:/ultimate/mods/").unwrap().into(),
            //             conflict_with: first_mod_root.strip_prefix("sd:/ultimate/mods/").unwrap().trim_end_matches('/').into(),
            //         };

            //         match conflict_list.get_mut(&conflict) {
            //             // We already have an existing conflict for these two mods, so add the file to that list
            //             Some(entries) => entries.push(local_path.into()),
            //             // There wasn't an existing conflict yet, add it to the list
            //             None => {
            //                 conflict_list.insert(conflict, vec![local_path.into()]);
            //             },
            //         }
            //     });
            // } else {

                // files.extend(mod_files.files);
                // patches.extend(mod_files.patches);
            // }

            // for path in paths {
            //     println!("{}", path.to_string(&interner));
            // }

            // if path.components().count() <= MAX_COMPONENT_COUNT {
            //     interner.add_path::<MAX_COMPONENT_COUNT>(path);
            // }
        }).collect();

    // dbg!(conflict_list);
    // let yaml = serde_yaml::to_string(&Vec::from_iter(conflict_list.iter())).unwrap();
    // std::fs::write("sd:/ultimate/arcropolis/conflicts.txt", yaml.as_bytes()).unwrap();
    // dbg!(patches);
    //dbg!(conflict_list);

    Modpack {
        mods
    }
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

// pub fn load_and_run_plugins(plugins: &Vec<(PathBuf, PathBuf)>) {
//     let mut plugin_nrr = NrrBuilder::new();

//     let modules: Vec<NroBuilder> = plugins
//         .iter()
//         .filter_map(|(root, local)| {
//             let full_path = root.join(local);

//             if full_path.exists() && full_path.ends_with("plugin.nro") {
//                 match NroBuilder::open(&full_path) {
//                     Ok(builder) => {
//                         info!("Loaded plugin at '{}' for chainloading.", full_path.display());
//                         plugin_nrr.add_module(&builder);
//                         Some(builder)
//                     },
//                     Err(e) => {
//                         error!("Failed to load plugin at '{}'. {:?}", full_path.display(), e);
//                         None
//                     },
//                 }
//             } else {
//                 error!(
//                     "File discovery collected path '{}' but it does not exist and/or is invalid!",
//                     full_path.display()
//                 );
//                 None
//             }
//         })
//         .collect();

//     if modules.is_empty() {
//         info!("No plugins found for chainloading.");
//         return;
//     }

//     let mut registration_info = match plugin_nrr.register() {
//         Ok(Some(info)) => info,
//         Ok(_) => return,
//         Err(e) => {
//             error!("{:?}", e);
//             crate::dialog_error("ARCropolis failed to register plugin module info.");
//             return;
//         },
//     };

//     let modules: Vec<Module> = modules
//         .into_iter()
//         .filter_map(|x| match x.mount() {
//             Ok(module) => Some(module),
//             Err(e) => {
//                 error!("Failed to mount chainloaded plugin. {:?}", e);
//                 None
//             },
//         })
//         .collect();

//     unsafe {
//         // Unfortunately, without unregistering this it will cause the game to crash, cause is unknown, but likely due to page alignment I'd guess
//         // It does not matter if we use only one NRR for both the prebuilt modules and the plugins, it will still cause a crash
//         nn::ro::UnregisterModuleInfo(&mut registration_info);
//     }

//     // 3.0.0: The plugins are apparently loaded despite the mismatch in module vs plugin count, leaving it here for now
//     // if modules.len() < plugins.len() {
//     //     crate::dialog_error("ARCropolis failed to load/mount some plugins.");
//     // } else {
//     info!("Successfully chainloaded all collected plugins.");
//     // }

//     for module in modules.into_iter() {
//         let callable = unsafe {
//             let mut sym_loc = 0usize;
//             let rc = nn::ro::LookupModuleSymbol(&mut sym_loc, &module, "main\0".as_ptr() as _);
//             if rc != 0 {
//                 warn!("Failed to find symbol 'main' in chainloaded plugin.");
//                 None
//             } else {
//                 Some(std::mem::transmute::<usize, extern "C" fn()>(sym_loc))
//             }
//         };

//         if let Some(entrypoint) = callable {
//             info!("Calling 'main' in chainloaded plugin");
//             entrypoint();
//             info!("Finished calling 'main' in chainloaded plugin");
//         }
//     }
// }
