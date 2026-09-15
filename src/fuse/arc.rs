use std::str::FromStr;

use nn_fuse::{AccessorResult, DAccessor, FAccessor, FileAccessor, FileSystemAccessor, FsAccessor, FsEntryType};
use smash_arc::{ArcLookup, Hash40, Region};

use crate::{replacement::extensions::original_decomp_size, resource, PathExtension};

pub struct ArcFileAccessor {
    hash: Hash40,
    region: Region,
    contents: Option<Vec<u8>>,
}

impl ArcFileAccessor {
    fn contents(&mut self) -> Result<&[u8], AccessorResult> {
        if self.contents.is_none() {
            let data = resource::arc().get_file_contents(self.hash, self.region).map_err(|err| {
                error!("ArcFileAccessor: failed to read {:#x} from data.arc: {:?}", self.hash.0, err);
                AccessorResult::Unexpected
            })?;
            self.contents = Some(data);
        }
        Ok(self.contents.as_deref().unwrap_or_default())
    }
}

impl FileAccessor for ArcFileAccessor {
    fn read(&mut self, buffer: &mut [u8], offset: usize) -> Result<usize, AccessorResult> {
        debug!("ArcFileAccessor::read - Buffer length: {:x}", buffer.len());
        let file = self.contents()?;
        let offset = offset.min(file.len());
        let count = buffer.len().min(file.len() - offset);
        buffer[..count].copy_from_slice(&file[offset..offset + count]);
        Ok(count)
    }

    fn get_size(&mut self) -> Result<usize, AccessorResult> {
        debug!("ArcFileAccessor::get_size");
        if let Some(size) = original_decomp_size(self.hash, self.region) {
            return Ok(size as usize);
        }
        resource::arc()
            .get_file_data_from_hash(self.hash, self.region)
            .map(|data| data.decomp_size as usize)
            .map_err(|_| AccessorResult::PathNotFound)
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

        let arc = resource::arc();
        let hash = path.smash_hash().unwrap();
        match arc.get_file_info_from_hash(hash) {
            Ok(info) => {
                if !info.flags.is_regional() {
                    file_region = Region::None;
                }
            },
            Err(_) => file_region = Region::None,
        }
        if read != 0 {
            if arc.get_file_path_index_from_hash(hash).is_ok() {
                Ok(FAccessor::new(
                    ArcFileAccessor {
                        hash,
                        region: file_region,
                        contents: None,
                    },
                    mode,
                ))
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
