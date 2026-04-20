use std::{io::Write, path::PathBuf};

use nn_fuse::*;

use crate::modfs::{EntryType, ModFsError};

pub struct ModFileAccessor(PathBuf);

pub struct ModDirAccessor(PathBuf);

pub struct ModFsAccessor;

fn map_read_err(err: ModFsError, path: &std::path::Path) -> AccessorResult {
    if let ModFsError::Io(ref io_err) = err {
        if io_err.kind() == std::io::ErrorKind::NotFound {
            warn!(target: "std", "mods:/ stale patch entry (on-disk file gone): {}", path.display());
            return AccessorResult::PathNotFound;
        }
    }
    AccessorResult::Unexpected
}

impl FileAccessor for ModFileAccessor {
    fn read(&mut self, mut buffer: &mut [u8], offset: usize) -> Result<usize, AccessorResult> {
        debug!(target: "no-mod-path", "ModFileAccessor::read - Buffer length: {:#x}", buffer.len());

        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.get_mut().unwrap() };
        let bytes = fs.modfs().read(&self.0).map_err(|e| map_read_err(e, &self.0))?;
        buffer.write(&bytes[offset..]).map_err(|_| AccessorResult::Unexpected)
    }

    fn get_size(&mut self) -> Result<usize, AccessorResult> {
        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.get_mut().unwrap() };
        match fs.modfs().size(&self.0) {
            Some(size) => {
                debug!(target: "no-mod-path", "ModFileAccessor::get_size - Size: {:#x}", size);
                Ok(size)
            },
            None => {
                debug!(target: "no-mod-path", "ModFileAccessor::get_size - Size not found");
                Err(AccessorResult::PathNotFound)
            },
        }
    }
}

impl DirectoryAccessor for ModDirAccessor {
    fn read(&mut self, buffer: &mut [DirectoryEntry]) -> Result<usize, AccessorResult> {
        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.get_mut().unwrap() };
        let modfs = fs.modfs();
        let children = modfs.read_dir(&self.0);
        for (idx, path) in children.iter().enumerate() {
            if idx >= buffer.len() {
                break;
            }

            buffer[idx].path = path.to_path_buf();
            buffer[idx].ty = match modfs.entry_type(path).ok_or(AccessorResult::PathNotFound)? {
                EntryType::File => DirectoryEntryType::File(modfs.size(path).unwrap_or(0) as i64),
                EntryType::Directory => DirectoryEntryType::Directory,
            };
        }
        Ok(children.len())
    }

    fn get_entry_count(&mut self) -> Result<usize, AccessorResult> {
        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.get_mut().unwrap() };
        Ok(fs.modfs().read_dir(&self.0).len())
    }
}

impl FileSystemAccessor for ModFsAccessor {
    fn get_entry_type(&self, path: &std::path::Path) -> Result<FsEntryType, AccessorResult> {
        debug!(target: "no-mod-path", "ModFsAccessor::get_entry_type - Path: {}", path.display());

        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.get_mut().unwrap() };
        match fs.modfs().entry_type(path) {
            Some(EntryType::File) => Ok(FsEntryType::File),
            Some(EntryType::Directory) => Ok(FsEntryType::Directory),
            None => Err(AccessorResult::PathNotFound),
        }
    }

    fn open_file(&self, path: &std::path::Path, mode: skyline::nn::fs::OpenMode) -> Result<*mut FAccessor, AccessorResult> {
        let read = mode & 1 != 0;
        let write = mode >> 1 & 1 != 0;
        let append = mode >> 2 & 1 != 0;

        debug!(target: "no-mod-path", "ModFsAccessor::open_file - Path: {} | Read: {} | Write: {} | Append: {}", path.display(), read, write, append);

        if write || append {
            return Err(AccessorResult::Unsupported);
        }

        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.get_mut().unwrap() };
        let modfs = fs.modfs();
        if !modfs.exists(path) {
            return Err(AccessorResult::PathNotFound);
        }

        if let Some(entry) = modfs.patch().get(path) {
            let full = entry.full_path(path);
            if !full.is_file() {
                warn!(target: "std", "mods:/ stale patch entry (on-disk file gone): {}", path.display());
                return Err(AccessorResult::PathNotFound);
            }
        }

        Ok(FAccessor::new(ModFileAccessor(PathBuf::from(path)), mode))
    }

    fn open_directory(&self, path: &std::path::Path, _mode: skyline::nn::fs::OpenDirectoryMode) -> Result<*mut DAccessor, AccessorResult> {
        debug!(target: "no-mod-path", "ModFsAccessor::open_directory - Path: {}", path.display());

        let fs = unsafe { &*crate::GLOBAL_FILESYSTEM.get_mut().unwrap() };
        if fs.modfs().exists(path) {
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
