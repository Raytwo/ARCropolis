use std::{
    collections::{HashMap, hash_map::Entry},
    ops::{Deref, DerefMut},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use arc_config::search::{File, Folder};
use smash_arc::{
    ArcLookup, DirInfo, DirectoryOffset, FileData, FileInfo, FileInfoBucket, FileInfoFlags, FileInfoIdx, FileInfoIndex, FileInfoToFileData, FilePath,
    FilePathIdx, FileSystemHeader, FolderPathListEntry, Hash40, HashToIndex, LoadedArc, LoadedSearchSection, LookupError, PathListEntry,
    RedirectionType, Region, SearchListEntry, SearchLookup, SearchSectionBody, FileInfoIndiceIdx, InfoToDataIdx, FileDataIdx, FileDataFlags, FileInfoToFileDataBitfield,
};

use crate::{
    get_smash_hash, hashes,
    resource::{self, CppVector, FilesystemInfo, LoadedData, LoadedDirectory, LoadedFilepath},
    PathExtension, HashingError,
};

use super::addition::DirectoryAdditionError;

/// Used to keep track of added DirInfo children.
#[derive(Debug)]
pub struct InterDir {
    /// Whether or not this modifies a DirInfo in the original game.
    pub modifies_original: bool,
    /// The dir_hash_to_info_idx for each new child in the corresponding DirInfo.
    pub children: Vec<HashToIndex>,
}

pub struct AdditionContext {
    pub arc: &'static mut LoadedArc,
    pub filesystem_info: &'static FilesystemInfo,

    pub added_files: HashMap<Hash40, FilePathIdx>,

    /// Maps a DirInfo's path hash to an InterDir; which helps keep track of new children.
    pub inter_dirs: HashMap<Hash40, InterDir>,

    pub filepaths: CppVector<FilePath>,
    pub file_info_indices: CppVector<FileInfoIndex>,
    pub file_infos: CppVector<FileInfo>,
    pub info_to_datas: CppVector<FileInfoToFileData>,
    pub file_datas: CppVector<FileData>,

    pub loaded_filepaths: CppVector<LoadedFilepath>,
    pub loaded_datas: CppVector<LoadedData>,
    pub loaded_directories: CppVector<LoadedDirectory>,

    pub dir_infos_vec: CppVector<DirInfo>,
    pub dir_hash_to_info_idx: CppVector<HashToIndex>,
    pub folder_offsets_vec: CppVector<DirectoryOffset>,
    pub folder_children_hashes: CppVector<HashToIndex>,
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
    fn new_file_info_index(&mut self, file_info_index: FileInfoIdx) -> FileInfoIndiceIdx {
        let new_indice = FileInfoIndiceIdx(self.file_info_indices.len() as u32);

        let new_info_indice_idx = FileInfoIndex {
            dir_offset_index: 0xFF_FFFF,
            file_info_index,
        };

        self.file_info_indices.push(new_info_indice_idx);

        new_indice
    }

    // This could probably be cleaned and split better but one thing at a time eh
    pub fn new_fileinfo_from_path(&mut self, path: &Path, info_to_data_index: InfoToDataIdx) -> Result<(), HashingError> {
        let new_indice = FilePathIdx(self.filepaths.len() as u32);

        // Create a FilePath from the path that's passed in
        let mut file_path = FilePath::from_path(path)?;

        // Sets unknown1 flags to true if file is either a nutexb or eff
        let with_unknown1 = file_path.ext.hash40() == Hash40::from("nutexb") || file_path.ext.hash40() == Hash40::from("eff");

        let (_, file_info_indice_index) = self.new_fileinfo(new_indice, info_to_data_index, with_unknown1);
        
        // Set the FilePath's path index to be the new file_info_indice_index
        file_path.path.set_index(file_info_indice_index.0);

        self.filepaths.push(file_path);

        // Insert the added FilePath's path and it's index to the context's added_files vector
        self.added_files.insert(file_path.path.hash40(), new_indice);

        Ok(())
    }

    pub fn new_fileinfo(&mut self, file_path_index: FilePathIdx, info_to_data_index: InfoToDataIdx, unknown1: bool) -> (FileInfoIdx, FileInfoIndiceIdx) {
        let new_indice = FileInfoIdx(self.file_infos.len() as u32);

        let file_info_indice_index = self.new_file_info_index(new_indice);

        let mut new_file_info = FileInfo {
            file_path_index,
            file_info_indice_index,
            info_to_data_index,
            flags: FileInfoFlags::new().with_unknown1(unknown1),
        };

        // Set the new file to be standalone so it doesn't need to be near the other files
        new_file_info.flags.set_standalone_file(true);
        new_file_info.flags.set_unshared_nus3bank(true);
        
        self.file_infos.push(new_file_info);

        (new_indice, file_info_indice_index)
    }

    pub fn new_fileinfo_to_filedata(&mut self, folder_offset_index: u32, file_data_index: FileDataIdx) -> InfoToDataIdx {
        let new_indice = InfoToDataIdx(self.info_to_datas.len() as u32);

        let new_info_to_data = FileInfoToFileData {
            folder_offset_index,
            file_data_index,
            file_info_index_and_load_type: FileInfoToFileDataBitfield::new().with_load_type(1),
        };

        self.info_to_datas.push(new_info_to_data);

        new_indice
    }

    pub fn new_file_data(&mut self, offset_in_folder: u32, comp_size: u32, decomp_size: u32) -> FileDataIdx {
        let new_indice = FileDataIdx(self.file_datas.len() as u32);

        let new_file_data = FileData {
            offset_in_folder,
            comp_size,
            decomp_size,
            flags: FileDataFlags::new().with_compressed(false).with_use_zstd(false),
        };

        self.file_datas.push(new_file_data);

        new_indice
    }

    pub fn new_dir_info(&mut self, dir_info_path: FolderPathListEntry) -> Result<DirInfo, LookupError> {
        // Get a base
        let mut dir_info = match self.get_dir_info_from_hash_ctx(Hash40::from("fighter/luigi/c00")) {
            Ok(dir) => *dir,
            Err(_) => return Err(LookupError::Missing),
        };

        // Change the values to match the path passed in
        dir_info.path = dir_info_path.path;
        dir_info.name = dir_info_path.file_name.hash40();
        dir_info.parent = dir_info_path.parent.hash40();
        dir_info.file_info_start_index = 0;
        dir_info.file_count = 0;
        dir_info.child_dir_start_index = 0;
        dir_info.child_dir_count = 0;

        Ok(dir_info)
    }

    pub fn new_dir_hash_to_info_idx(&mut self, dir_info: &DirInfo) -> HashToIndex {
        HashToIndex::new()
            .with_hash(dir_info.path.hash())
            .with_length(dir_info.path.length())
            .with_index(self.dir_infos_vec.len() as u32)
    }

    // Right now this will take up a bit of memory if adding multiple dirs to the same dirinfo, so gonna have to change it to take a vec instead ig
    pub fn add_dir_info_to_parent(&mut self, parent_dir_info: &DirInfo, child_hash_to_index: &HashToIndex) {
        // if self.inter_dirs.contains_key(&parent_dir_info.path.hash40()) {
        //     self.inter_dirs
        //         .get_mut(&parent_dir_info.path.hash40())
        //         .unwrap()
        //         .children
        //         .push(*child_hash_to_index);
        // } else {
        //     // If parent_dir_info already has children, then it must be from the original game. (Children ranges aren't modified until later)
        //     let modifies_original = parent_dir_info.child_dir_count > 0;
        //     let inter_dir = InterDir {
        //         modifies_original,
        //         children: [*child_hash_to_index].to_vec(),
        //     };
        //     self.inter_dirs.insert(parent_dir_info.path.hash40(), inter_dir);
        // }

        if let Entry::Vacant(e) = self.inter_dirs.entry(parent_dir_info.path.hash40()) {
             // If parent_dir_info already has children, then it must be from the original game. (Children ranges aren't modified until later)
             let modifies_original = parent_dir_info.child_dir_count > 0;
             
             let inter_dir = InterDir {
                 modifies_original,
                 children: [*child_hash_to_index].to_vec(),
             };
             e.insert(inter_dir);
         } else {
             self.inter_dirs
                .get_mut(&parent_dir_info.path.hash40())
                .unwrap()
                .children
                .push(*child_hash_to_index);
         }
    }

    pub fn new_directory_offset(&mut self, dir_info: &DirInfo) -> DirectoryOffset {
        DirectoryOffset {
            offset: 0,
            decomp_size: 0,
            size: 0,
            file_start_index: dir_info.file_info_start_index,
            file_count: dir_info.file_count,
            directory_index: 0xFF_FFFF,
        }
    }

    pub fn push_dir_context(&mut self, dir_info: DirInfo, dir_hash_to_info_idx: HashToIndex, new_dir_offset: DirectoryOffset) {
        self.dir_infos_vec.push(dir_info);
        self.dir_hash_to_info_idx.push(dir_hash_to_info_idx);
        self.folder_offsets_vec.push(new_dir_offset);
        self.loaded_directories.push(LoadedDirectory::default());
    }

    // Add new entries to the resource tables so they match with the FilePath and FileData tables
    pub fn expand_file_resource_tables(&mut self) {
        self.loaded_filepaths.push(LoadedFilepath::default());
        self.loaded_datas.push(LoadedData::new());
    }

    /// Attempts to find the hash in LoadedArc, and if not, look into the added files
    pub fn get_file_info_indice_idx<H: Into<Hash40>>(&self, hash: H) -> Result<FileInfoIndiceIdx, LookupError> {
        let hash = hash.into();

        match self.get_file_info_from_hash(hash) {
            Ok(info) => Ok(info.file_info_indice_index),
            Err(_) => {
                match self.added_files.get(&hash) {
                    Some(filepath_idx) => {
                        let info_index = self.filepaths[usize::from(*filepath_idx)].path.index() as usize;
                        let info_idx = self.file_info_indices[info_index].file_info_index;
                        Ok(self.file_infos[usize::from(info_idx)].file_info_indice_index)
                    },
                    None => Err(LookupError::Missing),
                }
            },
        }
    }

    pub fn get_shared_info_index(&self, current_index: FileInfoIdx) -> FileInfoIdx {
        let shared_idx = self.file_info_indices[usize::from(self.file_infos[usize::from(current_index)].file_info_indice_index)].file_info_index;
        
        if shared_idx == current_index {
            shared_idx
        } else {
            self.get_shared_info_index(shared_idx)
        }
    }

    pub fn get_dir_info_from_hash_ctx(&self, hash: Hash40) -> Result<&DirInfo, LookupError> {
        let dir_hash_to_info_index = self.dir_hash_to_info_idx.iter().collect::<Vec<_>>();

        let mut index: Option<usize> = None;

        for (i, hashtoindex) in dir_hash_to_info_index.iter().enumerate() {
            if hashtoindex.hash40() == hash {
                index = Some(i);
                break;
            }
        }

        match index {
            Some(dir_index) => Ok(&self.dir_infos_vec[dir_index]),
            None => Err(LookupError::Missing),
        }

        // let index = dir_hash_to_info_index
        //     .binary_search_by_key(&hash, |dir| dir.hash40())
        //     .map(|index| dir_hash_to_info_index[index].index() as usize)
        //     .map_err(|_| LookupError::Missing)?;

        // Ok(&self.dir_infos_vec[index])
    }

    pub fn get_dir_info_from_hash_ctx_mut(&mut self, hash: Hash40) -> Result<&mut DirInfo, LookupError> {
        let dir_hash_to_info_index = self.dir_hash_to_info_idx.iter().collect::<Vec<_>>();

        let mut index: Option<usize> = None;

        for (i, hashtoindex) in dir_hash_to_info_index.iter().enumerate() {
            if hashtoindex.hash40() == hash {
                index = Some(i);
                break;
            }
        }

        match index {
            Some(dir_index) => Ok(&mut self.dir_infos_vec[dir_index]),
            None => Err(LookupError::Missing),
        }

        // let index = dir_hash_to_info_index
        //     .binary_search_by_key(&hash, |dir| dir.hash40())
        //     .map(|index| dir_hash_to_info_index[index].index() as usize)
        //     .map_err(|_| LookupError::Missing)?;

        // Ok(&mut self.dir_infos_vec[index])
    }

    // for resharing super shared files
    pub fn get_directory_dependency_ctx(&self, dir_info: &DirInfo) -> Option<RedirectionType> {
        if dir_info.flags.redirected() {
            let directory_index = self.folder_offsets_vec[dir_info.path.index() as usize].directory_index;

            if directory_index != 0xFFFFFF {
                if dir_info.flags.is_symlink() {
                    Some(RedirectionType::Symlink(self.dir_infos_vec[directory_index as usize]))
                } else {
                    Some(RedirectionType::Shared(self.folder_offsets_vec[directory_index as usize]))
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl SearchContext {
    pub fn get_folder_path_mut(&mut self, hash: Hash40) -> Option<&mut FolderPathListEntry> {
        match self.search.get_folder_path_index_from_hash(hash) {
            Ok(entry) => Some(&mut self.folder_paths[entry.index() as usize]),
            Err(_) => match self.new_folder_paths.get(&hash) {
                Some(index) => Some(&mut self.folder_paths[*index]),
                None => None,
            },
        }
    }

    pub fn add_folder_recursive(&mut self, path: &Path) -> Result<(), DirectoryAdditionError> {
        match self.get_folder_path_mut(path.smash_hash()?) {
            Some(_) => Ok(()),
            None => {
                self.create_folder_hierarchy(path)?;
                Ok(())
            },
        }
    }

    pub fn create_folder_hierarchy(&mut self, path: &Path) -> Result<&mut FolderPathListEntry, DirectoryAdditionError> {
        let parent = path.parent().unwrap_or_else(|| panic!("Could not get parent of {:?}!", path));
        let new_folder = FolderPathListEntry::from_path(path)?;

        // We reached the root of the filesystem
        if parent == Path::new("") {
            self.add_new_root_folder_path(new_folder);
        } else {
            let next_index = self.path_list_indices.len() as u32;
            // Considering the path for this folder was not found, this means we need to either create of find its parent first. Recursively.
            let parent_folder = self.create_folder_hierarchy(parent)?;

            // TODO: Probably reconsider this design. Logically speaking we would just be called add_new_search_folder_to_parent here.
            // I ended up having to copy paste a chunk of the implementation to make it work.

            // Add our new FolderPath as a child of the parent folder
            // self.add_new_search_folder_to_parent(new_folder, parent_folder);

            let mut new_path = new_folder.as_path_entry();
            // Set the previous head of the linked list as the child of the new path
            new_path.path.set_index(parent_folder.get_first_child_index() as u32);

            // Set the next path as the first element of the linked list
            parent_folder.set_first_child_index(next_index);

            self.add_new_search_path(new_path);
            self.add_new_folder_path(new_folder);
        }

        // return the new FolderPath here
        self.get_folder_path_mut(path.smash_hash()?).ok_or(DirectoryAdditionError::Lookup(LookupError::Missing))

    }

    pub fn add_new_search_path(&mut self, entry: PathListEntry) {
        self.new_paths.insert(entry.path.hash40(), self.path_list_indices.len());
        self.path_list_indices.push(self.paths.len() as u32);
        self.paths.push(entry);
    }

    pub fn add_new_search_path_with_parent(&mut self, mut entry: PathListEntry, parent: &mut FolderPathListEntry) {
        // Set the previous head of the linked list as the child of the new path
        entry.path.set_index(parent.get_first_child_index() as u32);

        // Set the next path as the first element of the linked list
        parent.set_first_child_index(self.path_list_indices.len() as u32);

        self.add_new_search_path(entry)
    }

    pub fn add_new_search_folder_to_parent(&mut self, entry: FolderPathListEntry, parent: &mut FolderPathListEntry) {
        let new_path = entry.as_path_entry();

        self.add_new_search_path_with_parent(new_path, parent);
        self.add_new_folder_path(entry);
    }

    /// Create a new directory that does not have a parent
    pub fn add_new_root_folder_path(&mut self, entry: FolderPathListEntry) {
        // Create a new directory that does not have child directories
        let new_path = entry.as_path_entry();

        self.add_new_search_path(new_path);
        self.add_new_folder_path(entry);
    }

    pub fn add_new_folder_path(&mut self, mut entry: FolderPathListEntry) {
        entry.set_first_child_index(0xFF_FFFF);
        self.new_folder_paths.insert(entry.path.hash40(), self.folder_paths.len());
        self.folder_paths.push(entry);
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
        let loaded_datas = CppVector::clone_from_slice(filesystem_info.get_loaded_datas());
        let loaded_directories = CppVector::clone_from_slice(filesystem_info.get_loaded_directories());
        let dir_infos_vec = CppVector::from_slice(arc.get_dir_infos());
        let dir_hash_to_info_idx = CppVector::from_slice(arc.get_dir_hash_to_info_index());
        let folder_offsets_vec = CppVector::from_slice(arc.get_folder_offsets());

        let header = unsafe { &*(arc.fs_header as *mut FileSystemHeader) };
        let folder_children_hashes =
            unsafe { CppVector::from_slice(std::slice::from_raw_parts(arc.folder_child_hashes, header.hash_folder_count as usize)) };

        AdditionContext {
            arc,
            filesystem_info,

            added_files: HashMap::new(),
            inter_dirs: HashMap::new(),

            filepaths,
            file_info_indices,
            file_infos,
            info_to_datas,
            file_datas,

            loaded_filepaths,
            loaded_datas,
            loaded_directories,

            dir_infos_vec,
            dir_hash_to_info_idx,
            folder_offsets_vec,
            folder_children_hashes,
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
            mut loaded_directories,

            mut dir_infos_vec,
            mut dir_hash_to_info_idx,
            mut folder_offsets_vec,
            mut folder_children_hashes,
            ..
        } = ctx;

        // sort hash_to_info_index here
        let mut dir_hash_to_info_index_sorted = dir_hash_to_info_idx.iter().cloned().collect::<Vec<_>>();
        dir_hash_to_info_index_sorted.sort_by_key(|a| a.hash40());
        dir_hash_to_info_idx = CppVector::from_slice(&dir_hash_to_info_index_sorted[..]);

        let (filepaths, filepath_len) = (filepaths.as_mut_ptr(), filepaths.len());
        let (file_info_indices, info_index_len) = (file_info_indices.as_mut_ptr(), file_info_indices.len());
        let (file_infos, file_info_len) = (file_infos.as_mut_ptr(), file_infos.len());
        let (info_to_datas, info_to_data_len) = (info_to_datas.as_mut_ptr(), info_to_datas.len());
        let (file_datas, file_data_len) = (file_datas.as_mut_ptr(), file_datas.len());
        let (loaded_filepaths, loaded_filepath_len) = (loaded_filepaths.as_mut_ptr(), loaded_filepaths.len());
        let (loaded_datas, loaded_data_len) = (loaded_datas.as_mut_ptr(), loaded_datas.len());
        let (loaded_directories, loaded_directory_len) = (loaded_directories.as_mut_ptr(), loaded_directories.len());

        let now = std::time::Instant::now();

        for (dir_name, info) in &ctx.inter_dirs {
            let mut dir_info = *dir_infos_vec.iter().find(|x| x.path.hash40() == *dir_name).unwrap();
            if info.modifies_original {
                let current_folder_children_hashes_len = folder_children_hashes.len() as u32;

                // Copy the old children to the end of folder_children_hashes
                folder_children_hashes.extend_from_within(dir_info.children_range());

                // Reset all the child hashes that were previously children of the original parent to an invalid value.
                let original_children = &mut folder_children_hashes[dir_info.children_range()];
                original_children.iter_mut().for_each(|x| *x = HashToIndex::new());

                dir_info.child_dir_start_index = current_folder_children_hashes_len;
                dir_info.child_dir_count += info.children.len() as u32;

                // Update the dir info
                *dir_infos_vec.iter_mut().find(|x| x.path.hash40() == *dir_name).unwrap() = dir_info;

                // Reserve space at the end of the folder_children_hashes vector for our new children.
                let reserved_hashes: Vec<HashToIndex> = std::iter::repeat(HashToIndex::new()).take(info.children.len()).collect();
                folder_children_hashes.extend_from_slice(&reserved_hashes);

                add_children_to_dir_info(&mut dir_infos_vec, &mut folder_children_hashes, &dir_info, info, &ctx.inter_dirs);
            } else {
                dir_info.child_dir_start_index = folder_children_hashes.len() as u32;
                dir_info.child_dir_count = info.children.len() as u32;

                // Update the dir info
                *dir_infos_vec.iter_mut().find(|x| x.path.hash40() == *dir_name).unwrap() = dir_info;

                // Reserve space at the end of the folder_children_hashes vector for our new children.
                let reserved_hashes: Vec<HashToIndex> = std::iter::repeat(HashToIndex::new()).take(info.children.len()).collect();
                folder_children_hashes.extend_from_slice(&reserved_hashes);

                add_children_to_dir_info(&mut dir_infos_vec, &mut folder_children_hashes, &dir_info, info, &ctx.inter_dirs);
            }
        }
        println!("Added dirinfo children took {}ms", now.elapsed().as_millis());


        fn add_children_to_dir_info(
            dir_infos_vec: &mut CppVector<DirInfo>,
            folder_children_hashes: &mut CppVector<HashToIndex>,
            dir: &DirInfo,
            info: &InterDir,
            inter_dirs: &HashMap<Hash40, InterDir>,
        ) {
            let mut base_index = dir.child_dir_start_index as usize + dir.child_dir_count as usize - info.children.len();
            for child in &info.children {
                // If we have an intermediate directory for the child, then start adding its children
                if let Some(child_inter_dir) = inter_dirs.get(&child.hash40()) {
                    let mut child_dir_info = *dir_infos_vec.iter().find(|x| x.path.hash40() == child.hash40()).unwrap();

                    child_dir_info.child_dir_start_index = folder_children_hashes.len() as u32;
                    child_dir_info.child_dir_count = child_inter_dir.children.len() as u32;

                    // Update the dir info
                    *dir_infos_vec.iter_mut().find(|x| x.path.hash40() == child.hash40()).unwrap() = child_dir_info;

                    // Reserve space at the end of the dir info vector for our new children.
                    let reserved_dirs: Vec<HashToIndex> = std::iter::repeat(HashToIndex::new())
                        .take(child_inter_dir.children.len())
                        .collect();
                    folder_children_hashes.extend_from_slice(&reserved_dirs);
                    add_children_to_dir_info(dir_infos_vec, folder_children_hashes, &child_dir_info, child_inter_dir, inter_dirs);
                }

                folder_children_hashes[base_index] = *child;
                base_index += 1;
            }
        }

        // --------------------- SETUP DIRECTORY ADDITION VARIABLES ---------------------
        let (dir_infos_vec, dir_infos_vec_len) = (dir_infos_vec.as_mut_ptr(), dir_infos_vec.len());
        let (dir_hash_to_info_idx, _dir_hash_to_info_idx_len) = (dir_hash_to_info_idx.as_mut_ptr(), dir_hash_to_info_idx.len());
        let (folder_offsets_vec, folder_offsets_vec_len) = (folder_offsets_vec.as_mut_ptr(), folder_offsets_vec.len());
        let (folder_children_hashes, folder_children_hashes_len) = (folder_children_hashes.as_mut_ptr(), folder_children_hashes.len());
        // --------------------- END DIRECTORY ADDITION VARIABLES ---------------------

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
        fs_info.loaded_filepath_len = loaded_filepath_len as u32;

        fs_info.loaded_datas = loaded_datas;
        fs_info.loaded_data_len = loaded_data_len as u32;

        fs_info.loaded_directories = loaded_directories;
        fs_info.loaded_directory_len = loaded_directory_len as u32;

        // --------------------- BEGIN MODIFY DIRECTORY RELEATED FIELDS ---------------------
        // Set folder count for DirInfos and DirHashToInfoIndex
        header.folder_count = dir_infos_vec_len as u32;

        self.dir_infos = dir_infos_vec;
        self.dir_hash_to_info_index = dir_hash_to_info_idx;

        // Set folder count for FolderChildHashes
        header.hash_folder_count = folder_children_hashes_len as u32;
        self.folder_child_hashes = folder_children_hashes;

        // Calculate extra folders and set extra folder header for FolderOffsets
        let extra_folder_count =
            (folder_offsets_vec_len as u32).wrapping_sub(header.folder_offset_count_1 + header.folder_offset_count_2 + header.extra_folder);
        header.extra_folder = header.extra_folder.wrapping_add(extra_folder_count);
        self.folder_offsets = folder_offsets_vec;
        // --------------------- END MODIFY DIRECTORY RELEATED FIELDS ---------------------

        let now = std::time::Instant::now();
        self.resort_file_hashes();
        println!("File hash sort took {}ms", now.elapsed().as_millis());
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
            bucket.as_mut_slice().sort_by_key(|a| a.hash40().as_u64());
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

pub trait SearchEx: SearchLookup {
    fn get_folder_path_to_index_mut(&mut self) -> &mut [HashToIndex];
    fn get_folder_path_list_mut(&mut self) -> &mut [FolderPathListEntry];
    fn get_path_to_index_mut(&mut self) -> &mut [HashToIndex];
    fn get_path_list_indices_mut(&mut self) -> &mut [u32];
    fn get_path_list_mut(&mut self) -> &mut [PathListEntry];

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
            return Err(LookupError::Missing);
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
            return Err(LookupError::Missing);
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

        let now = std::time::Instant::now();
        self.resort_folder_paths();
        self.resort_paths();
        println!("Sorting folder and paths took {}ms", now.elapsed().as_millis());
    }
}

pub trait FileInfoFlagsExt {
    fn standalone_file(&self) -> bool;
    fn unshared_nus3bank(&self) -> bool;
    fn new_shared_file(&self) -> bool;
    fn set_standalone_file(&mut self, x: bool);
    fn set_unshared_nus3bank(&mut self, x: bool);
    fn set_new_shared_file(&mut self, x: bool);
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

    fn new_shared_file(&self) -> bool {
        self.unused4() & 4 != 0
    }

    fn set_new_shared_file(&mut self, x: bool) {
        if x {
            self.set_unused4(self.unused4() | 4);
        } else {
            self.set_unused4(self.unused4() & !4);
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
    fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, HashingError>
    where
        Self: Sized;
}

pub trait FromSearchableFile {
    fn from_file(file: &File) -> Self;
}

pub trait FromSearchableFolder {
    fn from_folder(folder: &Folder) -> Self;
}

impl FromSearchableFile for FilePath {
    fn from_file(file: &File) -> Self {
        let mut result = FilePath {
            path: HashToIndex::default(),
            ext: HashToIndex::default(),
            parent: HashToIndex::default(),
            file_name: HashToIndex::default(),
        };

        result.path.set_hash(file.full_path.crc());
        result.path.set_length(file.full_path.str_len());
        result.ext.set_hash(file.extension.crc());
        result.ext.set_length(file.extension.str_len());
        result.file_name.set_hash(file.file_name.crc());
        result.file_name.set_length(file.file_name.str_len());
        result.parent.set_hash(file.parent.full_path.crc());
        result.parent.set_length(file.parent.full_path.str_len());

        result
    }
}

impl FromSearchableFile for PathListEntry {
    fn from_file(file: &File) -> Self {
        let mut result = PathListEntry(SearchListEntry {
            path: HashToIndex::default(),
            ext: HashToIndex::default(),
            parent: HashToIndex::default(),
            file_name: HashToIndex::default(),
        });

        result.path.set_hash(file.full_path.crc());
        result.path.set_length(file.full_path.str_len());
        result.ext.set_hash(file.extension.crc());
        result.ext.set_length(file.extension.str_len());
        result.file_name.set_hash(file.file_name.crc());
        result.file_name.set_length(file.file_name.str_len());
        result.parent.set_hash(file.parent.full_path.crc());
        result.parent.set_length(file.parent.full_path.str_len());

        result
    }
}

impl FromSearchableFolder for FolderPathListEntry {
    fn from_folder(folder: &Folder) -> Self {
        let mut result = Self(SearchListEntry {
            path: HashToIndex::default(),
            ext: HashToIndex::default(),
            parent: HashToIndex::default(),
            file_name: HashToIndex::default(),
        });

        let parent = folder.parent.as_ref().map(|folder| folder.full_path).unwrap();

        let name = folder.name.unwrap();

        result.path.set_hash(folder.full_path.crc());
        result.path.set_length(folder.full_path.str_len());
        result.path.set_index(0xFF_FFFF);
        result.ext.set_hash(0xFFFF_FFFF);
        result.file_name.set_hash(name.crc());
        result.file_name.set_length(name.str_len());
        result.parent.set_hash(parent.crc());
        result.parent.set_length(parent.str_len());
        result.parent.set_index(0x0);

        result
    }
}

impl FromPathExt for FilePath {
    fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, HashingError> {
        let path = path.as_ref();
        let path_hash =  path.smash_hash()?;

        let ext_hash =  path.extension()
            .and_then(|x| x.to_str())
            .map(get_smash_hash)
            .ok_or(HashingError::MissingExtension)??;

        let name_hash = path.file_name()
            .and_then(|x| x.to_str())
            .map(get_smash_hash)
            .expect("could not read the filename")?;

        let parent_hash = path.parent().map_or(Ok(Hash40::from("")), |x| x.smash_hash())?;

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

        Ok(result)
    }
}

impl FromPathExt for FolderPathListEntry {
    fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, HashingError>
    where
        Self: Sized,
    {
        let path = path.as_ref();
        let path_hash = path.smash_hash()?;

        let name_hash = path.file_name()
            .and_then(|x| x.to_str())
            .map(get_smash_hash)
            .expect("could not read the filename")?;

        let parent_hash = path.parent().map_or(Ok(Hash40::from("")), |x| x.smash_hash())?;

        // TODO: Move this to Smash-arc
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

        Ok(result)
    }
}

impl FromPathExt for PathListEntry {
    fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, HashingError>
    where
        Self: Sized,
    {
        let path = path.as_ref();
        let path_hash =  path.smash_hash()?;

        let ext_hash =  path.extension()
            .and_then(|x| x.to_str())
            .map(get_smash_hash)
            .ok_or(HashingError::MissingExtension)??;

        let name_hash = path.file_name()
            .and_then(|x| x.to_str())
            .map(get_smash_hash)
            .expect("could not read the filename")?;

        let parent_hash = path.parent().map_or(Ok(Hash40::from("")), |x| x.smash_hash())?;

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

        Ok(result)
    }
}

pub trait DirInfoExt {
    fn copy_from_source(&mut self, src_dir_info: &DirInfo);
}

impl DirInfoExt for DirInfo {
    fn copy_from_source(&mut self, src_dir_info: &DirInfo) {
        self.path.set_index(src_dir_info.path.index());
        self.file_info_start_index = src_dir_info.file_info_start_index;
        self.file_count = src_dir_info.file_count;
        self.child_dir_start_index = src_dir_info.child_dir_start_index;
        self.child_dir_count = src_dir_info.child_dir_count;
        self.flags = src_dir_info.flags;
    }
}