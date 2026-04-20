use std::collections::HashSet;
use std::path::{Path, PathBuf};

use arc_config::Config as ModConfig;
use smash_arc::{ArcLookup, Hash40};
use thiserror::Error;

use crate::{resource, PathExtension};

const CANONICAL_SLOTS: &[&str] = &["c00", "c01", "c02", "c03", "c04", "c05", "c06", "c07"];

const COSTUME_INHERITED_SUBDIRS: &[&str] = &["camera", "cmn"];

const KIRBYCOPY_INHERITED_SUBDIRS: &[&str] = &["bodymotion", "cmn", "sound"];

const KIRBY_ARTICLE_OWNERS: &[&str] = &[
    "mario", "luigi", "donkey", "link", "samus", "samusd", "yoshi", "fox",
    "pikachu", "ness", "captain", "purin", "peach", "pickel",
];

#[derive(Debug, Error)]
pub enum InferenceError {
    #[error("path is not valid UTF-8: {0}")]
    NonUtf8(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CostumeClone {
    pub fighter: String,
    pub slot: String,
    pub subtype: CostumeSubtype,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CostumeSubtype {
    Main,
    Camera,
    Result,
    Movie,
    KirbyCopy,
}

pub fn merge_into_config(entries: &[(PathBuf, PathBuf, usize)], config: &mut ModConfig) {
    let existing_infos_count = config.new_dir_infos.len();
    let existing_base_count = config.new_dir_infos_base.len();
    let existing_files_count: usize = config.new_dir_files.values().map(|v| v.len()).sum();

    let mut clones: HashSet<CostumeClone> = HashSet::new();
    let mut stage_candidate_dirs: HashSet<String> = HashSet::new();
    let mut effect_candidate_dirs: HashSet<String> = HashSet::new();
    let mut files_emitted = 0usize;

    for (_root, local, _size) in entries {
        if let Some(clone) = classify_local_path(local) {
            clones.insert(clone);
        } else if let Some(stage_dir) = classify_stage_parent(local) {
            stage_candidate_dirs.insert(stage_dir);
        }
        if let Some(effect_dir) = classify_effect_new_dir(local) {
            effect_candidate_dirs.insert(effect_dir);
        }
        if let Some(top_level) = classify_file_membership(local) {
            if let Ok(hash) = local.smash_hash() {
                let key = hash40::Hash40::new(&top_level);
                config
                    .new_dir_files
                    .entry(key)
                    .or_default()
                    .push(hash40::Hash40(hash.0));
                files_emitted += 1;
            }
        }
    }

    for info in &config.new_dir_infos {
        if let Some(clone) = parse_fighter_dir_info(info) {
            clones.insert(clone);
        }
    }

    let kirby_slots: Vec<String> = clones
        .iter()
        .filter(|c| c.fighter == "kirby" && matches!(c.subtype, CostumeSubtype::Main))
        .map(|c| c.slot.clone())
        .collect();
    for slot in kirby_slots {
        for owner in KIRBY_ARTICLE_OWNERS {
            clones.insert(CostumeClone {
                fighter: (*owner).to_string(),
                slot: slot.clone(),
                subtype: CostumeSubtype::KirbyCopy,
            });
        }
    }

    let mut existing_infos: HashSet<String> = config.new_dir_infos.iter().cloned().collect();

    for clone in &clones {
        let top = clone.top_level_dir();
        if existing_infos.insert(top.clone()) {
            config.new_dir_infos.push(top);
        }

        let (subdirs, kc_prefix) = match clone.subtype {
            CostumeSubtype::Main => (COSTUME_INHERITED_SUBDIRS, None),
            CostumeSubtype::KirbyCopy => (KIRBYCOPY_INHERITED_SUBDIRS, Some("kirbycopy")),
            _ => continue,
        };
        for sub in subdirs {
            let (new_key, base_val) = match kc_prefix {
                None => (
                    format!("fighter/{}/{}/{}", clone.fighter, clone.slot, sub),
                    format!("fighter/{}/c00/{}", clone.fighter, sub),
                ),
                Some(prefix) => (
                    format!("fighter/{}/{}/{}/{}", clone.fighter, prefix, clone.slot, sub),
                    format!("fighter/{}/{}/c00/{}", clone.fighter, prefix, sub),
                ),
            };
            config
                .new_dir_infos_base
                .entry(new_key)
                .or_insert(base_val);
        }
    }

    let arc = resource::arc();
    let mut stages_emitted = 0usize;
    let mut stages_skipped_vanilla = 0usize;
    for d in stage_candidate_dirs {
        let hash = Hash40::from(d.as_str());
        if arc.get_dir_info_from_hash(hash).is_ok() {
            stages_skipped_vanilla += 1;
            continue;
        }
        if existing_infos.insert(d.clone()) {
            config.new_dir_infos.push(d);
            stages_emitted += 1;
        }
    }

    for d in effect_candidate_dirs {
        let hash = Hash40::from(d.as_str());
        if arc.get_dir_info_from_hash(hash).is_ok() {
            continue;
        }
        if existing_infos.insert(d.clone()) {
            config.new_dir_infos.push(d);
        }
    }

    for files in config.new_dir_files.values_mut() {
        files.sort_by_key(|h| h.0);
        files.dedup_by_key(|h| h.0);
    }

    config.new_dir_infos.sort();
    config.new_dir_infos.dedup();

    info!(
        target: "std",
        "inference: {} costume clone(s), {} new stage dir(s) ({} vanilla-existing stage dirs skipped), \
         {} file memberships across {} top-level dirs; replaced config.json-provided entries \
         (new_dir_infos {} -> {}, new_dir_infos_base {} -> {}, new_dir_files total {} -> {})",
        clones.len(),
        stages_emitted,
        stages_skipped_vanilla,
        files_emitted,
        config.new_dir_files.len(),
        existing_infos_count,
        config.new_dir_infos.len(),
        existing_base_count,
        config.new_dir_infos_base.len(),
        existing_files_count,
        config.new_dir_files.values().map(|v| v.len()).sum::<usize>(),
    );
}

fn classify_file_membership(local: &Path) -> Option<String> {
    let s = local.to_str()?;
    let comps: Vec<&str> = s.split('/').collect();

    if comps.len() >= 4 && comps[0] == "fighter" {
        let fighter = comps[1];
        let subtype = comps.get(2).copied()?;

        if fighter == "kirby" && subtype == "model" && comps.len() >= 6 {
            if let Some(victim_cap) = comps.get(3).copied() {
                if let Some(victim) = victim_cap.strip_prefix("copy_").and_then(|s| s.strip_suffix("_cap")) {
                    let slot = comps[4];
                    if is_noncanonical_slot(slot) {
                        return Some(format!("fighter/{}/kirbycopy/{}", victim, slot));
                    }
                }
            }
        }

        let (slot_idx, slot) = find_slot(&comps)?;

        match subtype {
            "model" | "motion" | "sound" | "effect" | "param" => {
                if slot_idx >= 3 {
                    return Some(format!("fighter/{}/{}", fighter, slot));
                }
            }
            "camera" if slot_idx == 3 => return Some(format!("fighter/{}/camera/{}", fighter, slot)),
            "result" if slot_idx == 3 => return Some(format!("fighter/{}/result/{}", fighter, slot)),
            "movie" if slot_idx == 3 => return Some(format!("fighter/{}/movie/{}", fighter, slot)),
            "kirbycopy" => {
                if slot_idx >= 4 {
                    return Some(format!("fighter/{}/kirbycopy/{}", fighter, slot));
                }
            }
            _ => {}
        }
        return None;
    }

    if comps.len() >= 5 && comps[0] == "camera" && comps[1] == "fighter" {
        let fighter = comps[2];
        let slot = comps[3];
        if is_noncanonical_slot(slot) {
            return Some(format!("fighter/{}/camera/{}", fighter, slot));
        }
    }

    if comps.len() == 4 && comps[0] == "sound" && comps[1] == "bank" {
        let category = comps[2];
        if category == "fighter" || category == "fighter_voice" {
            let filename = comps[3];
            if let Some((stem, _ext)) = filename.rsplit_once('.') {
                if let Some((fighter, slot)) = parse_sound_bank_stem(stem) {
                    if is_noncanonical_slot(&slot) {
                        return Some(format!("fighter/{}/{}", fighter, slot));
                    }
                }
            }
        }
    }

    if s.starts_with("stage/") {
        if let Some((parent, _)) = s.rsplit_once('/') {
            if parent.matches('/').count() >= 2 {
                return Some(parent.to_string());
            }
        }
    }

    if comps.len() >= 4 && comps[0] == "effect" && comps[1] == "fighter" {
        let fighter = comps[2];

        if comps.len() >= 5 {
            let maybe_trail = comps[3];
            if let Some(slot) = maybe_trail.strip_prefix("trail_") {
                if is_slot_token(slot) {
                    return Some(format!("effect/fighter/{}/{}", fighter, maybe_trail));
                }
            }
        }

        if comps.len() == 4 {
            let filename = comps[3];
            let prefix = format!("ef_{}_", fighter);
            if let Some(rest) = filename.strip_prefix(&prefix) {
                if let Some((slot, _ext)) = rest.split_once('.') {
                    if is_slot_token(slot) {
                        return Some(format!("effect/fighter/{}", fighter));
                    }
                }
            }
        }
    }

    None
}

fn classify_effect_new_dir(local: &Path) -> Option<String> {
    let s = local.to_str()?;
    let comps: Vec<&str> = s.split('/').collect();
    if comps.len() < 4 || comps[0] != "effect" || comps[1] != "fighter" {
        return None;
    }
    let fighter = comps[2];

    if comps.len() >= 5 {
        let maybe_trail = comps[3];
        if let Some(slot) = maybe_trail.strip_prefix("trail_") {
            if is_slot_token(slot) {
                return Some(format!("effect/fighter/{}/{}", fighter, maybe_trail));
            }
        }
    }

    Some(format!("effect/fighter/{}", fighter))
}

fn is_slot_token(s: &str) -> bool {
    s.len() == 3
        && s.as_bytes()[0] == b'c'
        && s.as_bytes()[1].is_ascii_digit()
        && s.as_bytes()[2].is_ascii_digit()
}

fn parse_fighter_dir_info(s: &str) -> Option<CostumeClone> {
    let comps: Vec<&str> = s.split('/').collect();
    if comps.first().copied() != Some("fighter") {
        return None;
    }
    match comps.len() {
        3 => {
            let fighter = comps[1];
            let slot = comps[2];
            is_noncanonical_slot(slot).then(|| CostumeClone {
                fighter: fighter.to_string(),
                slot: slot.to_string(),
                subtype: CostumeSubtype::Main,
            })
        },
        4 => {
            let fighter = comps[1];
            let slot = comps[3];
            if !is_noncanonical_slot(slot) {
                return None;
            }
            let subtype = match comps[2] {
                "camera" => CostumeSubtype::Camera,
                "result" => CostumeSubtype::Result,
                "movie" => CostumeSubtype::Movie,
                "kirbycopy" => CostumeSubtype::KirbyCopy,
                _ => return None,
            };
            Some(CostumeClone {
                fighter: fighter.to_string(),
                slot: slot.to_string(),
                subtype,
            })
        },
        _ => None,
    }
}

fn parse_sound_bank_stem(stem: &str) -> Option<(String, String)> {
    let rest = stem.strip_prefix("se_").or_else(|| stem.strip_prefix("vc_"))?;
    let (name_part, slot) = rest.rsplit_once('_')?;
    if !is_noncanonical_slot_lenient(slot) {
        return None;
    }
    let name = name_part.strip_suffix("_cheer").unwrap_or(name_part);
    Some((name.to_string(), slot.to_string()))
}

fn is_noncanonical_slot_lenient(s: &str) -> bool {
    if s.len() != 3 {
        return false;
    }
    let bytes = s.as_bytes();
    bytes[0] == b'c' && bytes[1].is_ascii_digit() && bytes[2].is_ascii_digit()
}

fn classify_stage_parent(local: &Path) -> Option<String> {
    let s = local.to_str()?;
    if !s.starts_with("stage/") {
        return None;
    }
    let parent = s.rsplit_once('/').map(|(dir, _)| dir)?;
    if parent.matches('/').count() < 2 {
        return None;
    }
    Some(parent.to_string())
}

fn classify_local_path(local: &Path) -> Option<CostumeClone> {
    let s = local.to_str()?;
    let mut comps: Vec<&str> = s.split('/').collect();
    if comps.len() < 4 || comps[0] != "fighter" {
        return None;
    }
    if comps.last().is_some_and(|c| c.is_empty()) {
        comps.pop();
    }

    let fighter = comps[1].to_string();

    let Some((slot_idx, slot_str)) = find_slot(&comps) else {
        return None;
    };

    let subtype = match comps.get(2)? {
        &"model" | &"motion" | &"sound" | &"effect" | &"param" => CostumeSubtype::Main,
        &"camera" => CostumeSubtype::Camera,
        &"result" => CostumeSubtype::Result,
        &"movie" => CostumeSubtype::Movie,
        &"kirbycopy" => CostumeSubtype::KirbyCopy,
        _ => return None,
    };

    if slot_idx < 3 {
        return None;
    }

    Some(CostumeClone {
        fighter,
        slot: slot_str.to_string(),
        subtype,
    })
}

fn find_slot<'a>(comps: &[&'a str]) -> Option<(usize, &'a str)> {
    for (i, c) in comps.iter().enumerate() {
        if is_noncanonical_slot(c) {
            return Some((i, c));
        }
    }
    None
}

fn is_noncanonical_slot(s: &str) -> bool {
    if s.len() != 3 {
        return false;
    }
    let bytes = s.as_bytes();
    if bytes[0] != b'c' {
        return false;
    }
    if !bytes[1].is_ascii_digit() || !bytes[2].is_ascii_digit() {
        return false;
    }
    !CANONICAL_SLOTS.iter().any(|&canon| canon == s)
}

impl CostumeClone {
    fn top_level_dir(&self) -> String {
        match self.subtype {
            CostumeSubtype::Main => format!("fighter/{}/{}", self.fighter, self.slot),
            CostumeSubtype::Camera => format!("fighter/{}/camera/{}", self.fighter, self.slot),
            CostumeSubtype::Result => format!("fighter/{}/result/{}", self.fighter, self.slot),
            CostumeSubtype::Movie => format!("fighter/{}/movie/{}", self.fighter, self.slot),
            CostumeSubtype::KirbyCopy => format!("fighter/{}/kirbycopy/{}", self.fighter, self.slot),
        }
    }
}

use log::info;

#[derive(Debug, Clone)]
pub enum Classification {
    Uninferred,
}

#[allow(dead_code)]
pub fn classify(_root: &Path, _local: &Path, _size: usize) -> Result<Classification, InferenceError> {
    Ok(Classification::Uninferred)
}
