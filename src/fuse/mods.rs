use std::{io::Write, path::PathBuf};

use nn_fuse::*;
use orbits::FileEntryType;

pub struct ModFileAccessor(PathBuf);

pub struct ModDirAccessor(PathBuf);

pub struct ModFsAccessor;

impl FileAccessor for ModFileAccessor {
    fn read(&mut self, mut buffer: &mut [u8], offset: usize) -> Result<usize, AccessorResult> {
        debug!(target: "no-mod-path", "ModFileAccessor::read - Buffer length: {:#x}", buffer.len());

        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };
        let file = fs.get().load(&self.0).map_err(|_| AccessorResult::Unexpected)?;
        buffer.write(&file.as_slice()[offset..]).map_err(|_| AccessorResult::Unexpected)
    }

    fn get_size(&mut self) -> Result<usize, AccessorResult> {
        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };
        let size = fs.get().query_max_filesize(&self.0).map_or_else(|| Err(AccessorResult::Unexpected), Ok);
        if let Ok(size) = size {
            debug!(target: "no-mod-path", "ModFileAccessor::get_size - Size: {:#x}", size);
        } else {
            debug!(target: "no-mod-path", "ModFileAccessor::get_size - Size not found");
        }
        size
    }
}

impl DirectoryAccessor for ModDirAccessor {
    fn read(&mut self, buffer: &mut [DirectoryEntry]) -> Result<usize, AccessorResult> {
        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };
        let children = fs.get().get_children(&self.0);
        for (idx, path) in children.iter().enumerate() {
            if idx >= buffer.len() {
                break
            }

            buffer[idx].path = path.to_path_buf();
            let ty = match fs.get().get_virtual_entry_type(path) {
                Err(_) => {
                    match fs.get().get_patch_entry_type(path) {
                        Ok(ty) => ty,
                        Err(_) => return Err(AccessorResult::PathNotFound),
                    }
                },
                Ok(ty) => ty,
            };
            match ty {
                FileEntryType::File => {
                    match fs.get().query_max_filesize(path) {
                        Some(size) => buffer[idx].ty = DirectoryEntryType::File(size as i64),
                        None => return Err(AccessorResult::Unexpected),
                    }
                },
                FileEntryType::Directory => buffer[idx].ty = DirectoryEntryType::Directory,
            }
        }
        Ok(children.len())
    }

    fn get_entry_count(&mut self) -> Result<usize, AccessorResult> {
        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };
        Ok(fs.get().get_children(&self.0).len())
    }
}

impl FileSystemAccessor for ModFsAccessor {
    fn get_entry_type(&self, path: &std::path::Path) -> Result<FsEntryType, AccessorResult> {
        debug!(target: "no-mod-path", "ModFsAccessor::get_entry_type - Path: {}", path.display());

        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };
        match fs.get().get_virtual_entry_type(path) {
            Err(_) => {
                match fs.get().get_patch_entry_type(path) {
                    Ok(ty) => {
                        match ty {
                            FileEntryType::File => Ok(FsEntryType::File),
                            FileEntryType::Directory => Ok(FsEntryType::Directory),
                        }
                    },
                    Err(_) => Err(AccessorResult::PathNotFound),
                }
            },
            Ok(ty) => {
                match ty {
                    FileEntryType::File => Ok(FsEntryType::File),
                    FileEntryType::Directory => Ok(FsEntryType::Directory),
                }
            },
        }
    }

    fn open_file(&self, path: &std::path::Path, mode: skyline::nn::fs::OpenMode) -> Result<*mut FAccessor, AccessorResult> {
        let read = mode & 1 != 0;
        let write = mode >> 1 & 1 != 0;
        let append = mode >> 2 & 1 != 0;

        debug!(target: "no-mod-path", "ModFsAccessor::open_file - Path: {} | Read: {} | Write: {} | Append: {}", path.display(), read, write, append);

        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };

        if write || append {
            return Err(AccessorResult::Unsupported)
        }

        if fs.get().contains(path) {
            Ok(FAccessor::new(ModFileAccessor(PathBuf::from(path)), mode))
        } else {
            Err(AccessorResult::PathNotFound)
        }
    }

    fn open_directory(&self, path: &std::path::Path, _mode: skyline::nn::fs::OpenDirectoryMode) -> Result<*mut DAccessor, AccessorResult> {
        debug!(target: "no-mod-path", "ModFsAccessor::open_directory - Path: {}", path.display());

        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };

        if fs.get().contains(path) {
            Ok(DAccessor::new(ModDirAccessor(PathBuf::from(path))))
        } else {
            Err(AccessorResult::PathNotFound)
        }
    }
}

pub fn install_mod_fs() {
    let accessor = FsAccessor::new(ModFsAccessor);
    unsafe {
        nn_fuse::mount("mods", &mut *accessor).unwrap();
    }
    info!("Finished mounting mods:/");
}
