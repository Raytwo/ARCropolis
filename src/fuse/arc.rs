use std::{io::Write, str::FromStr};

use nn_fuse::{AccessorResult, DAccessor, DirectoryAccessor, FAccessor, FileAccessor, FileSystemAccessor, FsAccessor, FsEntryType};
use once_cell::sync::Lazy;
use smash_arc::{ArcFile, ArcLookup, Hash40, Region};

use crate::PathExtension;

pub static ARC_FILE: Lazy<ArcFile> = Lazy::new(|| ArcFile::open("rom:/data.arc").unwrap());

pub struct ArcFileAccessor(Hash40, Region);

impl FileAccessor for ArcFileAccessor {
    fn read(&mut self, mut buffer: &mut [u8], offset: usize) -> Result<usize, AccessorResult> {
        debug!("ArcFileAccessor::read - Buffer length: {:x}", buffer.len());
        let file = ARC_FILE.get_file_contents(self.0, self.1).unwrap();
        Ok(buffer.write(&file.as_slice()[offset..]).unwrap())
    }

    fn get_size(&mut self) -> Result<usize, AccessorResult> {
        debug!("ArcFileAccessor::get_size");
        Ok(ARC_FILE.get_file_data_from_hash(self.0, self.1).unwrap().decomp_size as _)
    }
}

pub struct ArcDirAccessor;

impl DirectoryAccessor for ArcDirAccessor {
    fn read(&mut self, _buffer: &mut [nn_fuse::DirectoryEntry]) -> Result<usize, AccessorResult> {
        Err(AccessorResult::Unimplemented)
    }

    fn get_entry_count(&mut self) -> Result<usize, AccessorResult> {
        Err(AccessorResult::Unimplemented)
    }
}

pub struct ArcFuse;

impl FileSystemAccessor for ArcFuse {
    fn get_entry_type(&self, path: &std::path::Path) -> Result<FsEntryType, AccessorResult> {
        debug!("Path: {}", path.display());
        if path.file_name().is_some() {
            Ok(FsEntryType::File)
        } else {
            Err(AccessorResult::Unimplemented)
        }
    }

    fn open_file(&self, path: &std::path::Path, mode: skyline::nn::fs::OpenMode) -> Result<*mut FAccessor, AccessorResult> {
        let read = mode & 1;
        let write = mode >> 1 & 1;
        let append = mode >> 2 & 1;
        debug!("Path: {}, read: {}, write: {}, append: {}", path.display(), read, write, append);
        let mut file_region = config::region();
        let mut new_path = path.display().to_string();
        for region in crate::REGIONS.iter() {
            if new_path.contains(region) {
                let region_string = format!("+{}", region);
                new_path.remove_matches(&region_string);
                file_region = Region::from_str(region).unwrap();
                let _path = std::path::Path::new(&new_path);
            }
        }

        let hash = path.smash_hash().unwrap();
        match ARC_FILE.get_file_info_from_hash(hash) {
            Ok(info) => {
                if !info.flags.is_regional() {
                    file_region = Region::None;
                }
            },
            Err(_) => file_region = Region::None,
        }
        if read != 0 {
            if ARC_FILE.get_file_path_index_from_hash(hash).is_ok() {
                Ok(FAccessor::new(ArcFileAccessor(hash, file_region), mode))
            } else {
                Err(AccessorResult::PathNotFound)
            }
        } else {
            Err(AccessorResult::Unsupported)
        }
    }

    fn open_directory(&self, _path: &std::path::Path, _mode: skyline::nn::fs::OpenDirectoryMode) -> Result<*mut DAccessor, AccessorResult> {
        Err(AccessorResult::Unimplemented)
    }
}

pub fn install_arc_fs() {
    let accessor = FsAccessor::new(ArcFuse);
    unsafe { nn_fuse::mount("arc", &mut *accessor).unwrap() };
    info!("Finished mounting arc:/");
}
