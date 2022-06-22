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

pub fn discover_in_mods<P: AsRef<Utf8Path>>(root: P) -> ModDir {
    let root = root.as_ref();

    let files = WalkDir::new(root).min_depth(1).into_iter().flatten().filter_map(|entry| {
        // Ignore the directories, only care about the files
        if entry.file_type().is_file() && entry.path().extension().is_some() {
            //let (path, _) = crate::strip_region_from_path(Utf8Path::from_path(entry.path()).unwrap());
            let path = Utf8PathBuf::from_path_buf(entry.path().into()).unwrap();
            // TODO: Replace by a smash_path equivalent
            Some(ModFile { hash: hash40(path.as_str()), path, size: entry.metadata().unwrap().len() })
        } else {
            None
        }
    }).collect();

    ModDir { root: root.to_owned(), files }
}

pub fn discover_mods<P: AsRef<Utf8Path>>(root: P) -> Modpack {
    let root = root.as_ref();

    let presets = crate::config::presets::get_active_preset().unwrap();

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
            discover_in_mods(Utf8Path::from_path(entry.path()).unwrap())
        }).collect();
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
