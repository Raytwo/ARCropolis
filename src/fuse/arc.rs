use std::{slice, str::FromStr, sync::OnceLock};

use nn_fuse::{AccessorResult, DAccessor, FAccessor, FileAccessor, FileSystemAccessor, FsAccessor, FsEntryType};
use smash_arc::{
    ArcLookup, DirInfo, DirectoryOffset, FileData, FileInfo, FileInfoBucket, FileInfoIndex, FileInfoToFileData, FilePath, FileSystemHeader, Hash40,
    HashToIndex, LoadedArc, QuickDir, Region, SeekRead, StreamData, StreamEntry,
};

use crate::PathExtension;

// arc:/ must only ever serve vanilla files. Patching never writes the game's own table arrays, it works on
// copies and then moves the pointers, so reading through the original pointers is vanilla by construction.
// Only the header counts and the hash buckets get rewritten in place, those are copied here before patching
struct VanillaArc {
    arc: &'static LoadedArc,
    header: FileSystemHeader,
    buckets: Vec<FileInfoBucket>,
    file_hash_to_path_index: *const HashToIndex,
    dir_hash_to_info_index: *const HashToIndex,
    dir_infos: *const DirInfo,
    file_paths: *const FilePath,
    file_info_indices: *const FileInfoIndex,
    file_infos: *const FileInfo,
    file_info_to_datas: *const FileInfoToFileData,
    file_datas: *const FileData,
    folder_offsets: *const DirectoryOffset,
}

// nothing writes to the arrays behind these pointers once the copy is taken
unsafe impl Send for VanillaArc {}
unsafe impl Sync for VanillaArc {}

static VANILLA: OnceLock<VanillaArc> = OnceLock::new();

/// Has to run before any patching, initial_loading does it through install_arc_fs
pub fn capture_vanilla(arc: &'static LoadedArc) {
    VANILLA.get_or_init(|| unsafe {
        let bucket_count = (*arc.file_info_buckets).count as usize;
        VanillaArc {
            arc,
            header: *arc.fs_header,
            buckets: slice::from_raw_parts(arc.file_info_buckets, bucket_count + 1).to_vec(),
            file_hash_to_path_index: arc.file_hash_to_path_index,
            dir_hash_to_info_index: arc.dir_hash_to_info_index,
            dir_infos: arc.dir_infos,
            file_paths: arc.file_paths,
            file_info_indices: arc.file_info_indices,
            file_infos: arc.file_infos,
            file_info_to_datas: arc.file_info_to_datas,
            file_datas: arc.file_datas,
            folder_offsets: arc.folder_offsets,
        }
    });
}

fn vanilla() -> Result<&'static VanillaArc, AccessorResult> {
    VANILLA.get().ok_or(AccessorResult::Unexpected)
}

// table sizes mirror smash-arc's LoadedArc lookup, just against the copied header
impl ArcLookup for VanillaArc {
    fn get_file_info_buckets(&self) -> &[FileInfoBucket] {
        &self.buckets[1..]
    }

    fn get_file_hash_to_path_index(&self) -> &[HashToIndex] {
        unsafe { slice::from_raw_parts(self.file_hash_to_path_index, self.header.file_info_path_count as usize) }
    }

    fn get_dir_hash_to_info_index(&self) -> &[HashToIndex] {
        unsafe { slice::from_raw_parts(self.dir_hash_to_info_index, self.header.folder_count as usize) }
    }

    fn get_dir_infos(&self) -> &[DirInfo] {
        unsafe { slice::from_raw_parts(self.dir_infos, self.header.folder_count as usize) }
    }

    fn get_file_paths(&self) -> &[FilePath] {
        unsafe { slice::from_raw_parts(self.file_paths, self.header.file_info_path_count as usize) }
    }

    fn get_file_info_indices(&self) -> &[FileInfoIndex] {
        unsafe { slice::from_raw_parts(self.file_info_indices, self.header.file_info_index_count as usize) }
    }

    fn get_file_infos(&self) -> &[FileInfo] {
        let fs = &self.header;
        unsafe { slice::from_raw_parts(self.file_infos, (fs.file_info_count + fs.file_data_count_2 + fs.extra_count) as usize) }
    }

    fn get_file_info_to_datas(&self) -> &[FileInfoToFileData] {
        let fs = &self.header;
        unsafe {
            slice::from_raw_parts(
                self.file_info_to_datas,
                (fs.file_info_sub_index_count + fs.file_data_count_2 + fs.extra_count_2) as usize,
            )
        }
    }

    fn get_file_datas(&self) -> &[FileData] {
        let fs = &self.header;
        unsafe { slice::from_raw_parts(self.file_datas, (fs.file_data_count + fs.file_data_count_2 + fs.extra_count) as usize) }
    }

    fn get_folder_offsets(&self) -> &[DirectoryOffset] {
        let fs = &self.header;
        unsafe { slice::from_raw_parts(self.folder_offsets, (fs.folder_offset_count_1 + fs.folder_offset_count_2) as usize) }
    }

    // streams are never patched, the live tables are still vanilla there
    fn get_stream_entries(&self) -> &[StreamEntry] {
        self.arc.get_stream_entries()
    }

    fn get_stream_file_indices(&self) -> &[u32] {
        self.arc.get_stream_file_indices()
    }

    fn get_stream_datas(&self) -> &[StreamData] {
        self.arc.get_stream_datas()
    }

    fn get_quick_dirs(&self) -> &[QuickDir] {
        self.arc.get_quick_dirs()
    }

    fn get_stream_hash_to_entries(&self) -> &[HashToIndex] {
        self.arc.get_stream_hash_to_entries()
    }

    fn get_file_reader<'a>(&'a self) -> Box<dyn SeekRead + 'a> {
        self.arc.get_file_reader()
    }

    fn get_file_section_offset(&self) -> u64 {
        self.arc.get_file_section_offset()
    }

    fn get_stream_section_offset(&self) -> u64 {
        self.arc.get_stream_section_offset()
    }

    fn get_shared_section_offset(&self) -> u64 {
        self.arc.get_shared_section_offset()
    }

    fn get_file_infos_mut(&mut self) -> &mut [FileInfo] {
        unreachable!("the vanilla view is read only")
    }

    fn get_dir_infos_mut(&mut self) -> &mut [DirInfo] {
        unreachable!("the vanilla view is read only")
    }

    fn get_file_datas_mut(&mut self) -> &mut [FileData] {
        unreachable!("the vanilla view is read only")
    }

    fn get_file_info_to_datas_mut(&mut self) -> &mut [FileInfoToFileData] {
        unreachable!("the vanilla view is read only")
    }

    fn get_folder_offsets_mut(&mut self) -> &mut [DirectoryOffset] {
        unreachable!("the vanilla view is read only")
    }
}

pub struct ArcFileAccessor {
    hash: Hash40,
    region: Region,
    // read once per open handle, readers call read in chunks
    contents: Option<Vec<u8>>,
}

impl ArcFileAccessor {
    fn contents(&mut self) -> Result<&[u8], AccessorResult> {
        if self.contents.is_none() {
            let data = vanilla()?.get_file_contents(self.hash, self.region).map_err(|err| {
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
        vanilla()?
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

        let arc = vanilla()?;
        let hash = path.smash_hash().map_err(|_| AccessorResult::PathNotFound)?;
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
    capture_vanilla(crate::resource::arc());
    let accessor = FsAccessor::new(ArcFuse);
    unsafe { nn_fuse::mount("arc", &mut *accessor).unwrap() };
    info!("Finished mounting arc:/");
}
