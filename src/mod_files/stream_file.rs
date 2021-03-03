use std::path::{Path, PathBuf};

use crate::*;

pub struct StreamFile {
    pub path: PathBuf,
    pub filesize: u32,
}

impl ModFile for StreamFile {
    fn get_path(&self) -> &Path {
        &self.path
    }

    fn get_size(&self) -> u32 {
        self.filesize
    }
}