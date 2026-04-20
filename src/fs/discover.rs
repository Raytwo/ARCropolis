use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use camino::Utf8Path;
use serde::{Deserialize, Serialize};
use skyline::nn::{self, ro::*};
use smash_arc::{Hash40, Region};

use crate::{chainloader::*, utils};

struct RootWalk {
    staged_tree: Vec<(PathBuf, usize)>,
    staged_collected: Vec<(PathBuf, usize)>,
}

#[derive(Serialize, Deserialize)]
pub struct DiscoveryResult {
    pub entries: Vec<(PathBuf, PathBuf, usize)>,
    pub collected: Vec<(PathBuf, PathBuf)>,
}

fn should_ignore(local: &Path, region: Region) -> bool {
    let Some(name) = local.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    let is_root = local.parent().map(|parent| parent.as_os_str().is_empty()).unwrap_or(true);
    let is_dot = name.starts_with('.');
    let is_out_of_region = if let Some(index) = name.find('+') {
        let (_, end) = name.split_at(index + 1);
        !end.starts_with(&region.to_string())
    } else {
        false
    };

    is_root || is_dot || is_out_of_region
}

fn should_collect(local: &Path, region: Region) -> bool {
    let Some(name) = local.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    static RESERVED_NAMES: &[&str] = &["config.json", "plugin.nro", "bgm_property.bin"];
    static PATCH_EXTENSIONS: &[&str] =
        &["prcx", "prcxml", "stdatx", "stdatxml", "stprmx", "stprmxml", "xmsbt", "patch3audio", "motdiff", "yml"];

    if RESERVED_NAMES.contains(&name) {
        return true;
    }

    let is_out_of_region = if let Some(index) = name.find('+') {
        let (_, end) = name.split_at(index + 1);
        !end.starts_with(&region.to_string())
    } else {
        false
    };

    PATCH_EXTENSIONS.iter().any(|ext| name.ends_with(ext)) && !is_out_of_region
}

fn walk_one_root(mod_root: &Path, region: Region) -> RootWalk {
    let mut staged_tree: Vec<(PathBuf, usize)> = Vec::new();
    let mut staged_collected: Vec<(PathBuf, usize)> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![mod_root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let iter = match std::fs::read_dir(&dir) {
            Ok(iter) => iter,
            Err(e) => {
                warn!("Failed to read directory {}: {:?}", dir.display(), e);
                continue;
            },
        };
        for entry in iter.filter_map(|e| e.ok()) {
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_dir() {
                stack.push(entry.path());
                continue;
            }
            if !ft.is_file() {
                continue;
            }

            let full_path = entry.path();
            let local = match full_path.strip_prefix(mod_root) {
                Ok(rel) => rel.to_path_buf(),
                Err(_) => continue,
            };

            let size = entry.metadata().map(|m| m.len() as usize).unwrap_or(0);

            if should_collect(&local, region) {
                staged_collected.push((local, size));
                continue;
            }

            if should_ignore(&local, region) {
                continue;
            }

            staged_tree.push((local, size));
        }
    }

    RootWalk { staged_tree, staged_collected }
}

pub fn perform_discovery() -> DiscoveryResult {
    let is_emulator = utils::env::is_emulator();

    if is_emulator {
        info!("Emulator usage detected in perform_discovery, reverting to old behavior.");
    }

    let mods_path = utils::paths::mods();

    let legacy_discovery = config::legacy_discovery();

    let mut presets = config::presets::get_active_preset().unwrap();

    // Emulators can't use presets, so don't run this logic
    if !is_emulator && !legacy_discovery {
        // Get the mod cache from last run
        let mod_cache: HashSet<Hash40> = config::get_mod_cache().unwrap_or_default();

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

        let new_mods: HashSet<&Hash40> = new_cache
            .iter()
            .filter(|cached_mod| !mod_cache.contains(cached_mod) && !presets.contains(cached_mod))
            .collect();

        // We found hashes that weren't in the cache
        if !new_mods.is_empty() {
            // Add the new mods to the presets file
            presets.extend(new_mods);
            // Save it back
            config::presets::replace_active_preset(&presets).unwrap();
        }

        // No matter what, the cache has to be updated
        config::set_mod_cache(&new_cache).unwrap();
    }

    #[cfg(feature = "ui")]
    crate::check_input_on_boot();

    let presets = config::presets::get_active_preset().unwrap();

    let is_active_root = |path: &Path| {
        if !is_emulator && !legacy_discovery {
            presets.contains(&Hash40::from(path.to_str().unwrap()))
        } else {
            Utf8Path::from_path(path)
                .unwrap()
                .file_name()
                .map(|name| !name.starts_with('.'))
                .unwrap_or(false)
        }
    };

    let mut entries: Vec<(PathBuf, PathBuf, usize)> = Vec::new();
    let mut collected: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut claimed_tree_files: HashMap<PathBuf, PathBuf> = HashMap::new();
    let mut conflict_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    let mut active_roots: Vec<PathBuf> = match std::fs::read_dir(&mods_path) {
        Ok(iter) => iter
            .filter_map(|e| e.ok())
            .filter(|e| matches!(e.file_type(), Ok(ft) if ft.is_dir()))
            .map(|e| e.path())
            .filter(|p| is_active_root(p))
            .collect(),
        Err(e) => {
            error!("Could not read mods directory {}: {:?}", mods_path, e);
            return DiscoveryResult { entries, collected };
        },
    };
    active_roots.sort();

    super::cache::ensure_cache_dir();
    let cache_key = super::cache::discovery_key(&active_roots);
    if let Some(cached) = super::cache::load_discovery(cache_key) {
        info!(
            "discovery cache hit ({} entries, {} collected) — skipping walk",
            cached.entries.len(),
            cached.collected.len()
        );
        let DiscoveryResult { entries, collected } = cached;

        match mount_prebuilt_nrr(&entries) {
            Ok(Some(_)) => info!("Successfully registered fighter modules."),
            Ok(_) => info!("No fighter modules found to register."),
            Err(e) => {
                error!("{:?}", e);
                crate::dialog_error(
                    "ARCropolis failed to register module information for fighter modules.<br>You may experience infinite loading on some fighters.",
                );
            },
        }
        load_and_run_plugins(&collected);
        return DiscoveryResult { entries, collected };
    }

    let region = config::region();
    let walked: Vec<(PathBuf, RootWalk)> = active_roots
        .into_iter()
        .map(|root| {
            let walk = walk_one_root(&root, region);
            (root, walk)
        })
        .collect();

    for (mod_root, RootWalk { staged_tree, staged_collected }) in walked {
        let conflict_local = staged_tree.iter().find_map(|(local, _)| {
            claimed_tree_files.get(local).map(|first| (local.clone(), first.clone()))
        });

        if let Some((local, first_root)) = conflict_local {
            conflict_map
                .entry(local)
                .or_insert_with(|| vec![first_root])
                .push(mod_root.clone());
            warn!("Mod root '{}' was rejected due to a file conflict during discovery.", mod_root.display());
            continue;
        }

        for (local, size) in staged_tree {
            claimed_tree_files.insert(local.clone(), mod_root.clone());
            entries.push((mod_root.clone(), local, size));
        }
        for (local, size) in staged_collected {
            let is_chainload_only = local.file_name().and_then(|n| n.to_str()) == Some("plugin.nro");
            collected.push((mod_root.clone(), local.clone()));
            if !is_chainload_only {
                entries.push((mod_root.clone(), local, size));
            }
        }
    }

    if !conflict_map.is_empty() {
        match serde_json::to_string_pretty(&conflict_map) {
            Ok(json) => match std::fs::write("sd:/ultimate/arcropolis/conflicts.json", json.as_bytes()) {
                Ok(_) => {
                    crate::dialog_error(
                        "Conflict file created at sd:/ultimate/arcropolis/conflicts.json. Please open this file in a text editor to preview what mods are conflicting with one another and take the necessary changes to resolve them by either reslotting or removing these mods."
                    );
                },
                Err(e) => {
                    crate::dialog_error(format!(
                        "Failed to write conflict map to sd:/ultimate/arcropolis/conflicts.json<br>{:?}",
                        e
                    ));
                    for (local, roots) in &conflict_map {
                        error!("The file {} is used by the following roots:", local.display());
                        for root in roots {
                            error!("{}", root.display());
                        }
                    }
                },
            },
            Err(e) => {
                crate::dialog_error(format!("Failed to serialize conflict map to JSON. {:?}", e));
                for (local, roots) in &conflict_map {
                    error!("The file {} is used by the following roots:", local.display());
                    for root in roots {
                        error!("{}", root.display());
                    }
                }
            },
        }
    }

    match mount_prebuilt_nrr(&entries) {
        Ok(Some(_)) => info!("Successfully registered fighter modules."),
        Ok(_) => info!("No fighter modules found to register."),
        Err(e) => {
            error!("{:?}", e);
            crate::dialog_error(
                "ARCropolis failed to register module information for fighter modules.<br>You may experience infinite loading on some fighters.",
            );
        },
    }

    load_and_run_plugins(&collected);

    let result = DiscoveryResult { entries, collected };
    // Persist the fresh walk under the key we computed up front. Any mod
    // root change (add/remove/rename) flips that key, so the next boot
    // will skip the cache and regenerate here.
    super::cache::save_discovery(cache_key, &result);
    result
}

fn mount_prebuilt_nrr(entries: &[(PathBuf, PathBuf, usize)]) -> Result<Option<RegistrationInfo>, NrrRegistrationFailedError> {
    let fighter_nro_parent = Path::new("prebuilt;/nro/release");
    let mut fighter_nro_nrr = NrrBuilder::new();

    for (root, local, _size) in entries {
        if local.parent() == Some(fighter_nro_parent) {
            let full_path = root.join(local);
            debug!("Reading '{}' for module registration.", full_path.display());
            if let Ok(data) = std::fs::read(&full_path) {
                fighter_nro_nrr.add_module(data.as_slice());
            }
        }
    }

    fighter_nro_nrr.register()
}

pub fn load_and_run_plugins(plugins: &[(PathBuf, PathBuf)]) {
    let mut plugin_nrr = NrrBuilder::new();

    let modules: Vec<(PathBuf, NroBuilder)> = plugins
        .iter()
        .filter(|(_, local)| local.file_name().and_then(|n| n.to_str()) == Some("plugin.nro"))
        .filter_map(|(root, local)| {
            let full_path = root.join(local);

            if !full_path.exists() {
                error!("Plugin file '{}' does not exist.", full_path.display());
                return None;
            }

            match NroBuilder::open(&full_path) {
                Ok(builder) => {
                    debug!("Loaded plugin at '{}' for chainloading.", full_path.display());
                    plugin_nrr.add_module(&builder);
                    Some((full_path, builder))
                },
                Err(e) => {
                    error!("Failed to load plugin at '{}'. {:?}", full_path.display(), e);
                    None
                },
            }
        })
        .collect();

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
        },
    };

    // we have to do it this way
    // i'm sorry ray, but it literally does not work without collecting here
    // i don't know
    // i didn't write hos
    let modules: Vec<(PathBuf, Module)> = modules
        .into_iter()
        .filter_map(|(path, x)| match x.mount() {
            Ok(module) => Some((path, module)),
            Err(e) => {
                error!("Failed to mount chainloaded plugin '{}'. {:?}", path.display(), e);
                None
            },
        })
        .collect();

    unsafe {
        // Unfortunately, without unregistering this it will cause the game to crash, cause is unknown, but likely due to page alignment I'd guess
        // It does not matter if we use only one NRR for both the prebuilt modules and the plugins, it will still cause a crash
        nn::ro::UnregisterModuleInfo(&mut registration_info);
    }

    info!("Successfully chainloaded all collected plugins.");

    for (path, module) in modules {
        let callable = unsafe {
            let mut sym_loc = 0usize;
            let rc = nn::ro::LookupModuleSymbol(&mut sym_loc, &module, "main\0".as_ptr() as _);
            if rc != 0 {
                warn!("Failed to find symbol 'main' in chainloaded plugin '{}'.", path.display());
                None
            } else {
                Some(std::mem::transmute::<usize, extern "C" fn()>(sym_loc))
            }
        };

        if let Some(entrypoint) = callable {
            entrypoint();
        }
    }
}
