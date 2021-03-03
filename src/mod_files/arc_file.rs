use std::path::{Path, PathBuf};

use smash_arc::FileData;

use crate::*;

#[derive(Debug, Clone)]
pub struct ArcFile {
    pub path: PathBuf,
    pub hash: Hash40,
    pub filesize: u32,
    pub extension: Hash40,
    pub orig_subfile: FileData,
    pub index: FileInfoIndiceIdx,
}

impl ModFile for ArcFile {
    fn get_size(&self) -> u32 {
        self.filesize
    }

    fn get_path(&self) -> &Path {
        &self.path
    }
}