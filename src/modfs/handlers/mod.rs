use std::path::{Path, PathBuf};

use smash_arc::Hash40;

use super::HandlerRegistry;

pub mod bgm;
pub mod config;
pub mod motionlist;
pub mod msbt;
pub mod nus3audio;
pub mod prc;

pub fn strip_regional(path: &Path) -> (PathBuf, Option<String>) {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return (path.to_path_buf(), None);
    };
    let Some(idx) = name.find('+') else {
        return (path.to_path_buf(), None);
    };
    if name.len() < idx + 6 {
        return (path.to_path_buf(), None);
    }
    let region = name[idx + 1..idx + 6].to_string();
    let mut new_name = name.to_string();
    new_name.replace_range(idx..idx + 6, "");
    (path.with_file_name(new_name), Some(region))
}

pub fn try_smash_hash(path: &Path) -> Option<Hash40> {
    match crate::PathExtension::smash_hash(path) {
        Ok(h) => Some(h),
        Err(e) => {
            warn!("modfs: could not hash {}: {:?}", path.display(), e);
            None
        },
    }
}

pub fn register_builtins(registry: &mut HandlerRegistry) {
    registry.register(config::ConfigHandler::default());
    registry.register(prc::PrcHandler::default());
    registry.register(msbt::MsbtHandler::default());
    registry.register(nus3audio::Nus3audioHandler::default());
    registry.register(motionlist::MotionListHandler::default());
    registry.register(bgm::BgmHandler::default());
}
