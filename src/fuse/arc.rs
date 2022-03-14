use std::io::Write;

use nn_fuse::{FileAccessor, FileSystemAccessor, FAccessor, FsAccessor, DAccessor, AccessorResult, FsEntryType, DirectoryAccessor};
use smash_arc::{ArcLookup, Hash40, ArcFile};

use crate::PathExtension;

lazy_static! {
    static ref ARC_FILE: ArcFile = { ArcFile::open("rom:/data.arc").unwrap() };
}

pub struct ArcFileAccessor(Hash40);

impl FileAccessor for ArcFileAccessor {
    fn read(&mut self, mut buffer: &mut [u8], offset: usize) -> Result<usize, AccessorResult> {
        println!("ArcFileAccessor::read - Buffer length: {:x}", buffer.len());
        let file = ARC_FILE.get_file_contents(self.0, smash_arc::Region::UsEnglish).unwrap();
        Ok(buffer.write(&file.as_slice()[offset..]).unwrap())
    }

    fn get_size(&mut self) -> Result<usize, AccessorResult> {
        println!("ArcFileAccessor::get_size");
        Ok(ARC_FILE.get_file_data_from_hash(self.0, smash_arc::Region::UsEnglish).unwrap().decomp_size as _)
    }
}

pub struct ArcDirAccessor;

impl DirectoryAccessor for ArcDirAccessor {
    fn read(&mut self, buffer: &mut [nn_fuse::DirectoryEntry]) -> Result<usize, AccessorResult> {
        Err(AccessorResult::Unimplemented)
    }

    fn get_entry_count(&mut self) -> Result<usize, AccessorResult> {
        Err(AccessorResult::Unimplemented)
    }
}

pub struct ArcFuse;

impl FileSystemAccessor for ArcFuse {
    fn get_entry_type(&self, path: &std::path::Path) -> Result<FsEntryType, AccessorResult> {
        println!("Path: {}", path.display());
        if path.file_name().is_some() {
            Ok(FsEntryType::File)
        } else {
            Err(AccessorResult::Unimplemented)
        }
    }

    fn open_file(&self, path: &std::path::Path, mode: skyline::nn::fs::OpenMode) -> Result<*mut FAccessor, AccessorResult> {
        let read = mode >> 0 & 1;
        let write = mode >> 1 & 1;
        let append = mode >> 2 & 1;

        println!("Path: {}, read: {}, write: {}, append: {}", path.display(), read, write, append);

        let hash = path.smash_hash().unwrap();

        if read != 0 {
            if ARC_FILE.get_file_path_index_from_hash(hash).is_ok() {
                Ok(FAccessor::new(ArcFileAccessor(hash), mode))
            } else {
                Err(AccessorResult::PathNotFound)
            }
        } else {
            Err(AccessorResult::Unsupported)
        }
    }

    fn open_directory(&self, path: &std::path::Path, mode: skyline::nn::fs::OpenDirectoryMode) -> Result<*mut DAccessor, AccessorResult> {
        Err(AccessorResult::Unimplemented)
    }
}

pub fn install_arc_fs() {
    let accessor = FsAccessor::new(ArcFuse);
    unsafe { nn_fuse::mount("arc", &mut *accessor).unwrap() };
    println!("Finished mounting arc:/");
}