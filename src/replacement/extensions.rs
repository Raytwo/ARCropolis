use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use smash_arc::{
    ArcLookup, FileData, FileInfo, FileInfoBucket, FileInfoFlags, FileInfoIdx, FileInfoIndex, FileInfoToFileData, FilePath, FilePathIdx,
    FileSystemHeader, FolderPathListEntry, Hash40, HashToIndex, LoadedArc, LoadedSearchSection, LookupError, PathListEntry, Region, SearchListEntry,
    SearchLookup, SearchSectionBody,
};

use crate::{
    get_smash_hash, hashes,
    resource::{self, CppVector, FilesystemInfo, LoadedData, LoadedFilepath},
    PathExtension,
};

pub struct AdditionContext {
    pub arc: &'static mut LoadedArc,
    pub filesystem_info: &'static FilesystemInfo,

    pub added_files: HashMap<Hash40, FilePathIdx>,

    pub filepaths: CppVector<FilePath>,
    pub file_info_indices: CppVector<FileInfoIndex>,
    pub file_infos: CppVector<FileInfo>,
    pub info_to_datas: CppVector<FileInfoToFileData>,
    pub file_datas: CppVector<FileData>,

    pub loaded_filepaths: CppVector<LoadedFilepath>,
    pub loaded_datas: CppVector<LoadedData>,
}

pub struct SearchContext {
    pub search: &'static mut LoadedSearchSection,

    pub folder_paths: CppVector<FolderPathListEntry>,
    pub path_list_indices: CppVector<u32>,
    pub paths: CppVector<PathListEntry>,
    pub new_folder_paths: HashMap<Hash40, usize>,
    pub new_paths: HashMap<Hash40, usize>,
}

impl Deref for AdditionContext {
    type Target = LoadedArc;

    fn deref(&self) -> &Self::Target {
        self.arc
    }
}

impl DerefMut for AdditionContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.arc
    }
}

impl Deref for SearchContext {
    type Target = LoadedSearchSection;

    fn deref(&self) -> &Self::Target {
        self.search
    }
}

impl DerefMut for SearchContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.search
    }
}

impl AdditionContext {
    pub fn get_shared_info_index(&self, current_index: FileInfoIdx) -> FileInfoIdx {
        let shared_idx = self.file_info_indices[usize::from(self.file_infos[usize::from(current_index)].file_info_indice_index)].file_info_index;
        if shared_idx == current_index {
            shared_idx
        } else {
            self.get_shared_info_index(shared_idx)
        }
    }
}

impl SearchContext {
    pub fn get_folder_path_mut(&mut self, hash: Hash40) -> Option<&mut FolderPathListEntry> {
        match self.search.get_folder_path_index_from_hash(hash) {
            Ok(entry) => Some(&mut self.folder_paths[entry.index() as usize]),
            Err(_) => {
                match self.new_folder_paths.get(&hash) {
                    Some(index) => Some(&mut self.folder_paths[*index]),
                    None => None,
                }
            },
        }
    }
}

pub trait LoadedArcEx {
    fn get_file_hash_to_path_index_mut(&mut self) -> &mut [HashToIndex];
    fn get_bucket_for_hash_mut(&mut self, hash: Hash40) -> &mut [HashToIndex];
    fn patch_filedata(&mut self, hash: Hash40, size: u32, region: Region) -> Result<u32, LookupError>;
    fn change_hash_lookup(&mut self, hash: Hash40, index: FilePathIdx) -> Result<(), LookupError>;
    fn get_shared_file(&self, hash: Hash40) -> Result<FilePathIdx, LookupError>;
    fn resort_file_hashes(&mut self);
    fn make_addition_context() -> AdditionContext;
    fn take_context(&mut self, ctx: AdditionContext);
    fn contains_file(&self, hash: Hash40) -> bool;
}

impl LoadedArcEx for LoadedArc {
    fn get_file_hash_to_path_index_mut(&mut self) -> &mut [HashToIndex] {
        unsafe {
            let fs = *self.fs_header;
            let table_size = fs.file_info_path_count;
            std::slice::from_raw_parts_mut(self.file_hash_to_path_index as *mut HashToIndex, table_size as _)
        }
    }

    fn get_bucket_for_hash_mut(&mut self, hash: Hash40) -> &mut [HashToIndex] {
        let range = {
            let file_info_buckets = self.get_file_info_buckets();
            let bucket_index = (hash.as_u64() % (file_info_buckets.len() as u64)) as usize;
            let bucket = &file_info_buckets[bucket_index];
            (bucket.start as usize)..((bucket.start + bucket.count) as usize)
        };

        &mut self.get_file_hash_to_path_index_mut()[range]
    }

    fn patch_filedata(&mut self, hash: Hash40, size: u32, region: Region) -> Result<u32, LookupError> {
        let file_info = *self.get_file_info_from_hash(hash)?;
        let region = if file_info.flags.is_regional() {
            info!(
                "Patching file '{}' ({:#x}) and it is regional. Patching region {:?}",
                hashes::find(hash),
                hash.0,
                region
            );
            region
        } else {
            Region::None
        };

        let file_data = self.get_file_data_mut(&file_info, region);
        let old_size = file_data.decomp_size;
        file_data.decomp_size = size;
        Ok(old_size)
    }

    fn change_hash_lookup(&mut self, hash: Hash40, index: FilePathIdx) -> Result<(), LookupError> {
        let bucket = self.get_bucket_for_hash_mut(hash);

        let index_in_bucket = bucket
            .binary_search_by_key(&hash, |group| group.hash40())
            .map_err(|_| LookupError::Missing)?;

        bucket[index_in_bucket].set_index(index.0);
        Ok(())
    }

    fn get_shared_file(&self, hash: Hash40) -> Result<FilePathIdx, LookupError> {
        let file_info = self.get_file_info_from_hash(hash)?;
        let new_hash = self.get_file_paths()[file_info.file_path_index].path.hash40();
        if new_hash != hash {
            self.get_shared_file(new_hash)
        } else {
            Ok(file_info.file_path_index)
        }
    }

    fn make_addition_context() -> AdditionContext {
        let arc = resource::arc_mut();
        let filesystem_info = resource::filesystem_info();

        let filepaths = CppVector::from_slice(arc.get_file_paths());
        let file_info_indices = CppVector::from_slice(arc.get_file_info_indices());
        let file_infos = CppVector::from_slice(arc.get_file_infos());
        let info_to_datas = CppVector::from_slice(arc.get_file_info_to_datas());
        let file_datas = CppVector::from_slice(arc.get_file_datas());

        let loaded_filepaths = CppVector::from_slice(filesystem_info.get_loaded_filepaths());
        let loaded_datas = unsafe {
            let loaded_datas = filesystem_info.get_loaded_datas();
            let mut vec = CppVector::with_capacity(loaded_datas.len());
            std::ptr::copy_nonoverlapping(loaded_datas.as_ptr(), vec.as_mut_ptr(), loaded_datas.len());
            vec
        };

        AdditionContext {
            arc,
            filesystem_info,

            added_files: HashMap::new(),

            filepaths,
            file_info_indices,
            file_infos,
            info_to_datas,
            file_datas,

            loaded_filepaths,
            loaded_datas,
        }
    }

    fn take_context(&mut self, ctx: AdditionContext) {
        let AdditionContext {
            mut filepaths,
            mut file_info_indices,
            mut file_infos,
            mut info_to_datas,
            mut file_datas,

            mut loaded_filepaths,
            mut loaded_datas,
            ..
        } = ctx;
        let (filepaths, filepath_len) = (filepaths.as_mut_ptr(), filepaths.len());
        let (file_info_indices, info_index_len) = (file_info_indices.as_mut_ptr(), file_info_indices.len());
        let (file_infos, file_info_len) = (file_infos.as_mut_ptr(), file_infos.len());
        let (info_to_datas, info_to_data_len) = (info_to_datas.as_mut_ptr(), info_to_datas.len());
        let (file_datas, file_data_len) = (file_datas.as_mut_ptr(), file_datas.len());
        let (loaded_filepaths, _) = (loaded_filepaths.as_mut_ptr(), loaded_filepaths.len());
        let (loaded_datas, _) = (loaded_datas.as_mut_ptr(), loaded_datas.len());

        let header = unsafe { &mut *(self.fs_header as *mut FileSystemHeader) };

        self.file_paths = filepaths;
        header.file_info_path_count = filepath_len as u32;

        self.file_info_indices = file_info_indices;
        header.file_info_index_count = info_index_len as u32;

        self.file_infos = file_infos;
        header.file_info_count = file_info_len as u32;

        self.file_info_to_datas = info_to_datas;
        header.file_info_sub_index_count = info_to_data_len as u32;

        self.file_datas = file_datas;
        header.file_data_count = file_data_len as u32;

        let fs_info = resource::filesystem_info_mut();

        fs_info.loaded_filepaths = loaded_filepaths;
        fs_info.loaded_filepath_len = filepath_len as u32;

        fs_info.loaded_datas = loaded_datas;
        fs_info.loaded_data_len = info_index_len as u32;

        self.resort_file_hashes();
    }

    fn resort_file_hashes(&mut self) {
        static NEEDS_FREE: AtomicBool = AtomicBool::new(false);
        let bucket_count = unsafe { (*self.file_info_buckets).count as usize };

        let mut buckets = vec![Vec::with_capacity(0x50_0000); bucket_count];

        for (idx, file_path) in self.get_file_paths().iter().enumerate() {
            let bucket_idx = (file_path.path.hash40().as_u64() as usize) % bucket_count;
            let mut index = HashToIndex::default();
            index.set_hash(file_path.path.hash());
            index.set_length(file_path.path.length());
            index.set_index(idx as u32);
            buckets[bucket_idx].push(index);
        }

        let mut start_count = Vec::with_capacity(buckets.len());
        let mut start = 0usize;
        for bucket in buckets.iter_mut() {
            start_count.push((start, bucket.len()));
            start += bucket.len();
            bucket.as_mut_slice().sort_by(|a, b| a.hash40().as_u64().cmp(&b.hash40().as_u64()));
        }

        let mut new_hash_to_index = Vec::with_capacity(self.get_file_paths().len());
        for bucket in buckets.iter() {
            new_hash_to_index.extend_from_slice(bucket.as_slice());
        }

        let tmp = self.file_hash_to_path_index as _;

        self.file_hash_to_path_index = new_hash_to_index.leak().as_ptr();

        std::thread::sleep(std::time::Duration::from_millis(100));

        for (idx, _) in start_count.iter().enumerate().take(bucket_count) {
            unsafe {
                *(self.file_info_buckets as *mut FileInfoBucket).add(1 + idx) = FileInfoBucket {
                    start: start_count[idx].0 as u32,
                    count: start_count[idx].1 as u32,
                };
            }
        }

        if NEEDS_FREE.swap(true, Ordering::SeqCst) {
            unsafe {
                skyline::libc::free(tmp);
            }
        }
        assert!(self
            .get_file_path_index_from_hash(Hash40::from("fighter/common/param/fighter_param.prc"))
            .is_ok());
    }

    fn contains_file(&self, hash: Hash40) -> bool {
        self.get_file_path_index_from_hash(hash).is_ok()
    }
}

pub enum DirectoryChild {
    Folder(FolderPathListEntry),
    File(PathListEntry),
}

pub trait SearchEx: SearchLookup {
    fn get_folder_path_to_index_mut(&mut self) -> &mut [HashToIndex];
    fn get_folder_path_list_mut(&mut self) -> &mut [FolderPathListEntry];
    fn get_path_to_index_mut(&mut self) -> &mut [HashToIndex];
    fn get_path_list_indices_mut(&mut self) -> &mut [u32];
    fn get_path_list_mut(&mut self) -> &mut [PathListEntry];

    fn walk_directory(&self, hash: impl Into<Hash40>, mut f: impl FnMut(DirectoryChild, usize)) -> Result<(), LookupError> {
        fn internal(
            search: &(impl SearchLookup + ?Sized),
            hash: impl Into<Hash40>,
            f: &mut impl FnMut(DirectoryChild, usize),
            depth: usize,
        ) -> Result<(), LookupError> {
            let path_entry = search.get_path_list_entry_from_hash(hash)?;
            if !path_entry.is_directory() {
                return Ok(())
            }

            let mut child = search.get_first_child_in_folder(path_entry.path.hash40());
            while let Ok(child_entry) = child {
                if child_entry.is_directory() {
                    let folder_entry = search.get_folder_path_entry_from_hash(child_entry.path.hash40())?;
                    f(DirectoryChild::Folder(*folder_entry), depth);
                    internal(search, folder_entry.path.hash40(), f, depth + 1)?;
                } else {
                    f(DirectoryChild::File(*child_entry), depth);
                }
                child = search.get_next_child_in_folder(child_entry);
            }
            Ok(())
        }
        internal(self, hash, &mut f, 0)
    }

    fn get_folder_path_index_from_hash_mut(&mut self, hash: impl Into<Hash40>) -> Result<&mut HashToIndex, LookupError> {
        let folder_path_to_index = self.get_folder_path_to_index_mut();
        match folder_path_to_index.binary_search_by_key(&hash.into(), |h| h.hash40()) {
            Ok(idx) => Ok(&mut folder_path_to_index[idx]),
            Err(_) => Err(LookupError::Missing),
        }
    }

    fn get_folder_path_entry_from_hash_mut(&mut self, hash: impl Into<Hash40>) -> Result<&mut FolderPathListEntry, LookupError> {
        let index = *self.get_folder_path_index_from_hash(hash)?;
        if index.index() != 0xFF_FFFF {
            Ok(&mut self.get_folder_path_list_mut()[index.index() as usize])
        } else {
            Err(LookupError::Missing)
        }
    }

    fn get_path_index_from_hash_mut(&mut self, hash: impl Into<Hash40>) -> Result<&mut HashToIndex, LookupError> {
        let path_to_index = self.get_path_to_index_mut();
        match path_to_index.binary_search_by_key(&hash.into(), |h| h.hash40()) {
            Ok(idx) => Ok(&mut path_to_index[idx]),
            Err(_) => Err(LookupError::Missing),
        }
    }

    fn get_path_list_index_from_hash_mut(&mut self, hash: impl Into<Hash40>) -> Result<&mut u32, LookupError> {
        let index = *self.get_path_index_from_hash(hash)?;
        if index.index() != 0xFF_FFFF {
            Ok(&mut self.get_path_list_indices_mut()[index.index() as usize])
        } else {
            Err(LookupError::Missing)
        }
    }

    fn get_path_list_entry_from_hash_mut(&mut self, hash: impl Into<Hash40>) -> Result<&mut PathListEntry, LookupError> {
        let index = self.get_path_list_index_from_hash(hash)?;
        if index != 0xFF_FFFF {
            Ok(&mut self.get_path_list_mut()[index as usize])
        } else {
            Err(LookupError::Missing)
        }
    }

    fn get_first_child_in_folder_mut(&mut self, hash: impl Into<Hash40>) -> Result<&mut PathListEntry, LookupError> {
        let folder_path = self.get_folder_path_entry_from_hash(hash)?;
        let index_idx = folder_path.get_first_child_index();

        if index_idx == 0xFF_FFFF {
            return Err(LookupError::Missing)
        }

        let path_entry_index = self.get_path_list_indices()[index_idx];
        if path_entry_index != 0xFF_FFFF {
            Ok(&mut self.get_path_list_mut()[path_entry_index as usize])
        } else {
            Err(LookupError::Missing)
        }
    }

    fn get_next_child_in_folder_mut(&mut self, current_child: &PathListEntry) -> Result<&mut PathListEntry, LookupError> {
        let index_idx = current_child.path.index() as usize;
        if index_idx == 0xFF_FFFF {
            return Err(LookupError::Missing)
        }

        let path_entry_index = self.get_path_list_indices()[index_idx];
        if path_entry_index != 0xFF_FFFF {
            Ok(&mut self.get_path_list_mut()[path_entry_index as usize])
        } else {
            Err(LookupError::Missing)
        }
    }

    fn resort_folder_paths(&mut self);
    fn resort_paths(&mut self);
    fn make_context() -> SearchContext;
    fn take_context(&mut self, ctx: SearchContext);
}

impl SearchEx for LoadedSearchSection {
    fn get_folder_path_to_index_mut(&mut self) -> &mut [HashToIndex] {
        unsafe {
            let table_size = (*self.body).folder_path_count;
            std::slice::from_raw_parts_mut(self.folder_path_index as _, table_size as usize)
        }
    }

    fn get_folder_path_list_mut(&mut self) -> &mut [FolderPathListEntry] {
        unsafe {
            let table_size = (*self.body).folder_path_count;
            std::slice::from_raw_parts_mut(self.folder_path_list as _, table_size as usize)
        }
    }

    fn get_path_to_index_mut(&mut self) -> &mut [HashToIndex] {
        unsafe {
            let table_size = (*self.body).path_indices_count;
            std::slice::from_raw_parts_mut(self.path_index as _, table_size as usize)
        }
    }

    fn get_path_list_indices_mut(&mut self) -> &mut [u32] {
        unsafe {
            let table_size = (*self.body).path_indices_count;
            std::slice::from_raw_parts_mut(self.path_list_indices as _, table_size as usize)
        }
    }

    fn get_path_list_mut(&mut self) -> &mut [PathListEntry] {
        unsafe {
            let table_size = (*self.body).path_count;
            std::slice::from_raw_parts_mut(self.path_list as _, table_size as usize)
        }
    }

    fn resort_folder_paths(&mut self) {
        static NEEDS_FREE: AtomicBool = AtomicBool::new(false);
        let paths = self.get_folder_path_list();
        let mut indices = Vec::with_capacity(paths.len());
        for (idx, path) in paths.iter().enumerate() {
            let mut index = HashToIndex::default();
            index.set_hash(path.path.hash());
            index.set_length(path.path.length());
            index.set_index(idx as u32);
            indices.push(index);
        }
        indices.sort_by_key(|a| a.hash40());

        let tmp = self.folder_path_index;

        self.folder_path_index = indices.leak().as_ptr();

        if NEEDS_FREE.swap(true, Ordering::SeqCst) {
            unsafe {
                skyline::libc::free(tmp as _);
            }
        }
    }

    fn resort_paths(&mut self) {
        static NEEDS_FREE: AtomicBool = AtomicBool::new(false);
        let paths = self.get_path_list();
        let mut index_link = HashMap::new();
        for (idx, index) in self.get_path_list_indices().iter().enumerate() {
            if *index != 0xFFFF_FFFF && *index != 0xFF_FFFF {
                index_link.insert(paths[*index as usize].path.hash40(), idx);
            }
        }
        let mut indices = Vec::with_capacity(paths.len());
        for path in paths.iter() {
            let mut index = HashToIndex::default();
            index.set_hash(path.path.hash());
            index.set_length(path.path.length());
            if let Some(idx) = index_link.get(&path.path.hash40()) {
                index.set_index(*idx as u32);
            } else {
                index.set_index(0xFF_FFFF);
            }
            indices.push(index);
        }
        indices.sort_by_key(|a| a.hash40());

        let tmp = self.path_index;

        self.path_index = indices.leak().as_ptr();

        if NEEDS_FREE.swap(true, Ordering::SeqCst) {
            unsafe {
                skyline::libc::free(tmp as _);
            }
        }
    }

    fn make_context() -> SearchContext {
        let search = resource::search_mut();

        let folder_paths = CppVector::from_slice(search.get_folder_path_list());
        let paths = CppVector::from_slice(search.get_path_list());
        let path_list_indices = CppVector::from_slice(search.get_path_list_indices());
        SearchContext {
            search,

            folder_paths,
            path_list_indices,
            paths,

            new_folder_paths: HashMap::new(),
            new_paths: HashMap::new(),
        }
    }

    fn take_context(&mut self, ctx: SearchContext) {
        let SearchContext {
            folder_paths,
            path_list_indices,
            paths,
            ..
        } = ctx;

        let (folder_paths, folder_paths_len) = (folder_paths.as_ptr(), folder_paths.len());
        let (path_list_indices, path_list_indices_len) = (path_list_indices.as_ptr(), path_list_indices.len());
        let (paths, paths_len) = (paths.as_ptr(), paths.len());

        unsafe {
            self.folder_path_list = folder_paths as _;
            (*(self.body as *mut SearchSectionBody)).folder_path_count = folder_paths_len as u32;

            self.path_list_indices = path_list_indices as _;
            (*(self.body as *mut SearchSectionBody)).path_indices_count = path_list_indices_len as u32;

            self.path_list = paths as _;
            (*(self.body as *mut SearchSectionBody)).path_count = paths_len as u32;
        }

        self.resort_folder_paths();
        self.resort_paths();
    }
}

pub trait FileInfoFlagsExt {
    fn standalone_file(&self) -> bool;
    fn unshared_nus3bank(&self) -> bool;
    fn set_standalone_file(&mut self, x: bool);
    fn set_unshared_nus3bank(&mut self, x: bool);
}

impl FileInfoFlagsExt for FileInfoFlags {
    fn standalone_file(&self) -> bool {
        self.unused4() & 1 != 0
    }

    fn set_standalone_file(&mut self, x: bool) {
        if x {
            self.set_unused4(self.unused4() | 1);
        } else {
            self.set_unused4(self.unused4() & !1);
        }
    }

    fn unshared_nus3bank(&self) -> bool {
        self.unused4() & 2 != 0
    }

    fn set_unshared_nus3bank(&mut self, x: bool) {
        if x {
            self.set_unused4(self.unused4() | 2);
        } else {
            self.set_unused4(self.unused4() & !2);
        }
    }
}

pub trait FromPathExt {
    fn from_path<P: AsRef<Path>>(path: P) -> Option<Self>
    where
        Self: Sized;
}

impl FromPathExt for FilePath {
    fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        let path = path.as_ref();
        let path_hash = match path.smash_hash() {
            Ok(hash) => hash,
            Err(_) => return None,
        };

        let ext_hash = match path.extension().and_then(|x| x.to_str()) {
            Some(str) => {
                match get_smash_hash(str) {
                    Ok(hash) => hash,
                    Err(_) => return None,
                }
            },
            None => return None,
        };

        let name_hash = match path.file_name().and_then(|x| x.to_str()).map(get_smash_hash) {
            Some(Ok(hash)) => hash,
            _ => return None,
        };

        let parent_hash = match path.parent().map_or(Ok(Hash40::from("")), |x| x.smash_hash()) {
            Ok(hash) => hash,
            Err(_) => return None,
        };

        let mut result = FilePath {
            path: HashToIndex::default(),
            ext: HashToIndex::default(),
            parent: HashToIndex::default(),
            file_name: HashToIndex::default(),
        };

        result.path.set_hash(path_hash.crc32());
        result.path.set_length(path_hash.len());
        result.ext.set_hash(ext_hash.crc32());
        result.ext.set_length(ext_hash.len());
        result.file_name.set_hash(name_hash.crc32());
        result.file_name.set_length(name_hash.len());
        result.parent.set_hash(parent_hash.crc32());
        result.parent.set_length(parent_hash.len());

        Some(result)
    }
}

impl FromPathExt for FolderPathListEntry {
    fn from_path<P: AsRef<Path>>(path: P) -> Option<Self>
    where
        Self: Sized,
    {
        let path = path.as_ref();
        let path_hash = match path.smash_hash() {
            Ok(hash) => hash,
            Err(_) => return None,
        };

        let name_hash = match path.file_name().and_then(|x| x.to_str()).map(get_smash_hash) {
            Some(Ok(hash)) => hash,
            _ => return None,
        };

        let parent_hash = match path.parent().map_or(Ok(Hash40::from("")), |x| x.smash_hash()) {
            Ok(hash) => hash,
            Err(_) => return None,
        };

        let mut result = Self(SearchListEntry {
            path: HashToIndex::default(),
            ext: HashToIndex::default(),
            parent: HashToIndex::default(),
            file_name: HashToIndex::default(),
        });

        result.path.set_hash(path_hash.crc32());
        result.path.set_length(path_hash.len());
        result.path.set_index(0xFF_FFFF);
        result.ext.set_hash(0xFFFF_FFFF);
        result.file_name.set_hash(name_hash.crc32());
        result.file_name.set_length(name_hash.len());
        result.parent.set_hash(parent_hash.crc32());
        result.parent.set_length(parent_hash.len());
        result.parent.set_index(0x40_0000);

        Some(result)
    }
}

impl FromPathExt for PathListEntry {
    fn from_path<P: AsRef<Path>>(path: P) -> Option<Self>
    where
        Self: Sized,
    {
        let path = path.as_ref();
        let path_hash = match path.smash_hash() {
            Ok(hash) => hash,
            Err(_) => return None,
        };

        let ext_hash = match path.extension().and_then(|x| x.to_str()) {
            Some(str) => {
                match get_smash_hash(str) {
                    Ok(hash) => hash,
                    Err(_) => return None,
                }
            },
            None => return None,
        };

        let name_hash = match path.file_name().and_then(|x| x.to_str()).map(get_smash_hash) {
            Some(Ok(hash)) => hash,
            _ => return None,
        };

        let parent_hash = match path.parent().map_or(Ok(Hash40::from("")), |x| x.smash_hash()) {
            Ok(hash) => hash,
            Err(_) => return None,
        };

        let mut result = Self(SearchListEntry {
            path: HashToIndex::default(),
            ext: HashToIndex::default(),
            parent: HashToIndex::default(),
            file_name: HashToIndex::default(),
        });

        result.path.set_hash(path_hash.crc32());
        result.path.set_length(path_hash.len());
        result.ext.set_hash(ext_hash.crc32());
        result.ext.set_length(ext_hash.len());
        result.file_name.set_hash(name_hash.crc32());
        result.file_name.set_length(name_hash.len());
        result.parent.set_hash(parent_hash.crc32());
        result.parent.set_length(parent_hash.len());

        Some(result)
    }
}
