use arc_vector::ArcVector;
use smash_arc::*;
use crate::runtime::*;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};

lazy_static! {
    pub static ref UNSHARED_FILES: Mutex<HashMap<u32, HashSet<u32>>> = Mutex::new(HashMap::new());
    pub static ref ALREADY_UNSHARED: Mutex<HashSet<u32>> = Mutex::new(HashSet::new());
}
trait LoadedArcAdditions {
    fn get_dir_infos_as_vec(&self) -> ArcVector<DirInfo>;
    fn get_file_paths_as_vec(&self) -> ArcVector<FilePath>;
    fn get_file_info_indices_as_vec(&self) -> ArcVector<FileInfoIndex>;
    fn get_file_infos_as_vec(&self) -> ArcVector<FileInfo>;
    fn get_file_info_to_datas_as_vec(&self) -> ArcVector<FileInfoToFileData>;
    fn get_file_datas_as_vec(&self) -> ArcVector<FileData>;
    fn get_file_groups_as_vec(&self) -> ArcVector<DirectoryOffset>;
    fn get_path_to_index_as_vec(&self) -> ArcVector<HashToIndex>;
}

pub trait LoadedTableAdditions {
    fn get_filepath_table_as_vec(&self) -> ArcVector<Table1Entry>;
    fn get_loaded_data_table_as_vec(&self) -> ArcVector<Table2Entry>;
    fn get_loaded_directories_as_vec(&self) -> ArcVector<LoadedDirectory>;
}

impl LoadedArcAdditions for LoadedArc {
    fn get_dir_infos_as_vec(&self) -> ArcVector<DirInfo> {
        let fs = unsafe { &mut *(self.fs_header as *mut FileSystemHeader) };
        let ptr = &self.dir_infos as *const *const DirInfo;
        let ptr_size = &mut fs.folder_count as *mut u32;
        ArcVector::new(
            ptr as *mut *mut DirInfo,
            ptr_size,
            None
        )
    }

    fn get_file_paths_as_vec(&self) -> ArcVector<FilePath> {
        let fs = unsafe { &mut *(self.fs_header as *mut FileSystemHeader) };
        let ptr = &self.file_paths as *const *const FilePath;
        let ptr_size = &mut fs.file_info_path_count as *mut u32;
        ArcVector::new(
            ptr as *mut *mut FilePath,
            ptr_size,
            None
        )
    }

    fn get_file_info_indices_as_vec(&self) -> ArcVector<FileInfoIndex> {
        let fs = unsafe { &mut *(self.fs_header as *mut FileSystemHeader) };
        let ptr = &self.file_info_indices as *const *const FileInfoIndex;
        let ptr_size = &mut fs.file_info_index_count as *mut u32;
        ArcVector::new(
            ptr as *mut *mut FileInfoIndex,
            ptr_size,
            None
        )
    }

    fn get_file_infos_as_vec(&self) -> ArcVector<FileInfo> {
        let fs = unsafe { &mut *(self.fs_header as *mut FileSystemHeader) };
        let ptr = &self.file_infos as *const *mut FileInfo;
        let ptr_size = &mut fs.file_info_count as *mut u32;
        let ptr_size2 = &mut fs.file_data_count_2 as *mut u32;
        ArcVector::new(
            ptr as *mut *mut FileInfo,
            ptr_size,
            Some(ptr_size2)
        )
    }

    fn get_file_info_to_datas_as_vec(&self) -> ArcVector<FileInfoToFileData> {
        let fs = unsafe { &mut *(self.fs_header as *mut FileSystemHeader) };
        let ptr = &self.file_info_to_datas as *const *mut FileInfoToFileData;
        let ptr_size = &mut fs.file_info_sub_index_count as *mut u32;
        let ptr_size2 = &mut fs.file_data_count_2 as *mut u32;
        ArcVector::new(
            ptr as *mut *mut FileInfoToFileData,
            ptr_size,
            Some(ptr_size2)
        )
    }

    fn get_file_datas_as_vec(&self) -> ArcVector<FileData> {
        let fs = unsafe { &mut *(self.fs_header as *mut FileSystemHeader) };
        let ptr = &self.file_datas as *const *mut FileData;
        let ptr_size = &mut fs.file_data_count as *mut u32;
        let ptr_size2 = &mut fs.file_data_count_2 as *mut u32;
        ArcVector::new(
            ptr as *mut *mut FileData,
            ptr_size,
            Some(ptr_size2)
        )
    }

    fn get_file_groups_as_vec(&self) -> ArcVector<DirectoryOffset> {
        let fs = unsafe { &mut *(self.fs_header as *mut FileSystemHeader) };
        let ptr = &self.folder_offsets as *const *const DirectoryOffset;
        let ptr_size = &mut fs.folder_offset_count_2 as *mut u32;
        let ptr_size2 = &mut fs.folder_offset_count_1 as *mut u32;
        ArcVector::new(
            ptr as *mut *mut DirectoryOffset,
            ptr_size,
            Some(ptr_size2)
        )
    }

    fn get_path_to_index_as_vec(&self) -> ArcVector<HashToIndex> {
        let fs = unsafe { &mut *(self.fs_header as *mut FileSystemHeader) };
        let ptr = &self.file_hash_to_path_index as *const *const HashToIndex as *mut *mut HashToIndex;
        let ptr_size = &mut fs.file_info_path_count as *mut u32;
        ArcVector::new(
            ptr,
            ptr_size,
            None
        )
    }
}

impl LoadedTableAdditions for LoadedTables {
    fn get_filepath_table_as_vec(&self) -> ArcVector<Table1Entry> {
        let ptr = &self.table1 as *const *mut Table1Entry as *mut *mut Table1Entry;
        let ptr_size = &self.table1_len as *const u32 as *mut u32;
        ArcVector::new(
            ptr,
            ptr_size,
            None
        )
    }

    fn get_loaded_data_table_as_vec(&self) -> ArcVector<Table2Entry> {
        let ptr = &self.table2 as *const *mut Table2Entry as *mut *mut Table2Entry;
        let ptr_size = &self.table2_len as *const u32 as *mut u32;
        ArcVector::new(
            ptr,
            ptr_size,
            None
        )
    }

    fn get_loaded_directories_as_vec(&self) -> ArcVector<LoadedDirectory> {
        let ptr = &self.loaded_directory_table as *const *const LoadedDirectory as *mut *mut LoadedDirectory;
        let ptr_size = &self.loaded_directory_table_size as *const u32 as *mut u32;
        ArcVector::new(
            ptr,
            ptr_size,
            None
        )
    }
}

lazy_static! {
    static ref ALREADY_RESHARED: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    static ref RESHARED_FILEPATHS: Mutex<HashMap<Hash40, FilePathIdx>> = Mutex::new(HashMap::new());
    pub static ref TO_UNSHARE_ON_DISCOVERY: Mutex<HashMap<Hash40, (u32, FileInfoIdx)>> = Mutex::new(HashMap::new());
}

static mut FILE_PATH_CAPACITY: Option<usize> = None;
static mut INFO_INDICE_CAPACITY: Option<usize> = None;
static mut FILE_INFO_CAPACITY: Option<usize> = None;
static mut INFO_TO_DATA_CAPACITY: Option<usize> = None;
static mut DATA_CAPACITY: Option<usize> = None;
static mut LOADED_DATA_CAPACITY: Option<usize> = None;

// Old, unused functions being left in for reference
#[allow(dead_code)]
#[allow(unused_variables)]
#[allow(unused_mut)]
pub fn reshare_dir_info(hash: Hash40) {
    fn is_lowest_shared_file(info: &FileInfo, arc: &LoadedArc) -> bool {
        let file_paths = arc.get_file_paths();
        let info_indices = arc.get_file_info_indices();
        let file_infos = arc.get_file_infos();
        let cur_file_path_hash = file_paths[info.file_path_index].path.hash40();
        let next_file_path_hash = file_paths[file_infos[info_indices[file_paths[info.file_path_index].path.index() as usize].file_info_index].file_path_index].path.hash40();
        cur_file_path_hash == next_file_path_hash
    }

    fn get_shared_hash(info: &FileInfo, arc: &LoadedArc) -> Hash40 {
        let file_paths = arc.get_file_paths();
        let info_indices = arc.get_file_info_indices();
        let file_infos = arc.get_file_infos();
        file_paths[file_infos[info_indices[file_paths[info.file_path_index].path.index() as usize].file_info_index].file_path_index].path.hash40()
    }
    
    let loaded_tables = LoadedTables::acquire_instance();
    let mut already_reshared = ALREADY_RESHARED.lock();
    let arc = LoadedTables::get_arc_mut();

    // Acquire the necessary vectors
    let mut file_paths = arc.get_file_paths_as_vec();
    let mut info_indices = arc.get_file_info_indices_as_vec();
    let mut file_infos = arc.get_file_infos_as_vec();
    let mut file_groups = arc.get_file_groups_as_vec();

    // get the hash's dir info
    let dir_info = if let Ok(info) = arc.get_dir_info_from_hash(hash) {
        info.clone()
    } else {
        return;
    };

    // get the DirInfo's shared filegroup (if it exists)
    let shared_file_group = if let Some(RedirectionType::Shared(group)) = arc.get_directory_dependency(&dir_info) {
        group
    } else {
        return;
    };
    let mut info_to_datas = arc.get_file_info_to_datas_as_vec();

    if !already_reshared.contains(&shared_file_group.directory_index) {
        // backup current FileInfo vec length
        let og_fi_len = file_infos.len();

        // Get the reshared filepaths hashmap
        let mut reshared = RESHARED_FILEPATHS.lock();

        // duplicate the file infos all at once so we aren't storing invalid references when we realloc
        file_infos.extend_from_within(shared_file_group.file_start_index as usize, shared_file_group.file_count as usize);
        for (current_offset, file_info) in file_infos.iter_mut().skip(og_fi_len).enumerate() {
            // Duplicate the filepath
            // In order to eliminate the source slot problem, we have to recreate all of the filepaths with something unique that we can detect.
            // To do this, we take the CRC32 and assign it a length of 0x69
            file_paths.push_from_within(usize::from(file_info.file_path_index));
            let new_fp = file_paths.last_mut().unwrap();
            new_fp.path.set_length(0x69);

            // Create the new FileInfoIndex before we drop the FilePath reference, since we need to change the FileInfoIndex it points to
            info_indices.push_from_within(new_fp.path.index() as usize);
            new_fp.path.set_index((info_indices.len() - 1) as u32);

            // Add our reshared file path to our HashMap
            // This would be much, much nicer if we could add it to the FileInfoBuckets and let smash-arc search it, but for some reason
            // it is not properly handling it
            let fp_hash = new_fp.path.hash40();
            drop(new_fp);
            let fp_index = FilePathIdx((file_paths.len() - 1) as u32);
            let _ = reshared.insert(fp_hash, fp_index);

            // Change the FileInfo index on the new FileInfoIndex
            // Note: On files shared with other fighters, this doesn't actually matter.
            let new_ii = info_indices.last_mut().unwrap();
            new_ii.file_info_index = FileInfoIdx((og_fi_len + current_offset) as u32);
            drop(new_ii);

            // Here, we are checking to see if this file is shared with another fighter, or really anything else that isn't itself
            // This is important, because if this is the "source file" then the InfoToData/FileData will be correct. If it isn't, then they will
            // be wrong and cause an infinite load
            if is_lowest_shared_file(file_info, arc) {
                file_info.file_path_index = fp_index;
                file_info.file_info_indice_index = FileInfoIndiceIdx((info_indices.len() - 1) as u32);
                // info_to_datas[file_info.info_to_data_index.0].folder_offset_index = 0xca6;
                // info_to_datas[file_info.info_to_data_index.0].file_info_index_and_flag = 0x0100_0000;
            }
        }
        file_groups[shared_file_group.directory_index].file_start_index = og_fi_len as u32;
        loaded_tables.get_filepath_table_as_vec().set_len(file_paths.len());
        loaded_tables.get_loaded_data_table_as_vec().set_len(file_paths.len());
        already_reshared.push(shared_file_group.directory_index);
    }

    // file_groups[dir_info.path.index() as usize].directory_index = 0xFF_FFFF;

    let reshared = RESHARED_FILEPATHS.lock();
    let shared_start = arc.get_shared_data_index();
    for file_info in file_infos.iter_mut().skip(dir_info.file_info_start_index as usize).take(dir_info.file_count as usize) {
        // Check if the FileData is past the "shared offset". If so, then we need to search the FilePaths for the right information
        if arc.get_file_in_folder(file_info, Region::None).file_data_index.0 >= shared_start {
            // Get the deeper shared FilePath hash from this.
            let shared_file_hash = get_shared_hash(file_info, arc);
            // Change it to the new hash we created for accessing the reshared files
            let backup_hash = Hash40((shared_file_hash.as_u64() & 0xFFFF_FFFF) | 0x69_0000_0000);
            if let Some(path_idx) = reshared.get(&backup_hash) {
                // The reshared FilePath was found, change this FileInfo's information to reflect it
                let new_ii_index = file_paths[usize::from(*path_idx)].path.index();
                file_paths[usize::from(file_info.file_path_index)].path.set_index(new_ii_index);
                file_info.file_info_indice_index = FileInfoIndiceIdx(new_ii_index);
            }
        }
    }
}

pub fn unshare_files(directory: Hash40) {
    let loaded_tables = LoadedTables::acquire_instance();
    let mut unshared_files = UNSHARED_FILES.lock();
    let mut to_unshare = TO_UNSHARE_ON_DISCOVERY.lock();
    unshare_recursively(directory, &loaded_tables, &mut unshared_files, &mut to_unshare);
    unsafe {
        let mut loaded_datas = loaded_tables.get_loaded_data_table_as_vec();
        if let Some(cap) = LOADED_DATA_CAPACITY.clone() {
            loaded_datas.set_capacity(cap);
        }
        loaded_datas.set_len(LoadedTables::get_arc().get_file_info_indices().len());
    }
}

fn get_shared_file(info: &FileInfo, arc: &LoadedArc) -> FilePathIdx {
    let file_paths = arc.get_file_paths();
    let info_indices = arc.get_file_info_indices();
    let file_infos = arc.get_file_infos();
    let mut last_file_path = file_infos[info_indices[file_paths[info.file_path_index].path.index() as usize].file_info_index].file_path_index;
    while file_infos[info_indices[file_paths[last_file_path].path.index() as usize].file_info_index].file_path_index != last_file_path {
        last_file_path = file_infos[info_indices[file_paths[last_file_path].path.index() as usize].file_info_index].file_path_index;
    }
    last_file_path
}

pub fn unshare_recursively(directory: Hash40, loaded_tables: &LoadedTables, unshared_files: &mut HashMap<u32, HashSet<u32>>, to_unshare: &mut HashMap<Hash40, (u32, FileInfoIdx)>) {
    fn get_self_index(info: &DirInfo, arc: &LoadedArc) -> u32 {
        let hash_index = arc.get_dir_hash_to_info_index();
        let mut counter = 0;
        for hash in hash_index.iter() {
            if hash.hash40() == info.path.hash40() {
                return hash.index();
            }
            if counter == 100 {
                counter += 1;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        0xFF_FFFF
    }

    fn unshare_children(info: &DirInfo, arc: &LoadedArc, loaded_tables: &LoadedTables, unshared_files: &mut HashMap<u32, HashSet<u32>>, to_unshare: &mut HashMap<Hash40, (u32, FileInfoIdx)>) {
        for idx in info.children_range() {
            let next_hash = unsafe { (*arc.folder_child_hashes.add(idx)).hash40() };
            unshare_recursively(next_hash, loaded_tables, unshared_files, to_unshare);
        }
    }

    let arc = LoadedTables::get_arc();
    
    let dir_info = if let Ok(info) = arc.get_dir_info_from_hash(directory) {
        info.clone()
    } else {
        return;
    };

    if !dir_info.flags.redirected() || dir_info.flags.is_symlink() {
        unshare_children(&dir_info, arc, loaded_tables, unshared_files, to_unshare);
        return;
    }

    arc.get_file_groups_as_vec()[dir_info.path.index() as usize].directory_index = 0xFF_FFFF;


    let self_index = get_self_index(&dir_info, arc);

    let unshared_filepaths = if let Some(filepaths) = unshared_files.get_mut(&self_index) {
        filepaths
    } else {
        unshared_files.insert(self_index, HashSet::new());
        unshared_files.get_mut(&self_index).unwrap()
    };

    let mut file_paths = arc.get_file_paths_as_vec();
    if let Some(cap) = unsafe { FILE_PATH_CAPACITY.clone() } {
        file_paths.set_capacity(cap);
    }
    let mut info_indices = arc.get_file_info_indices_as_vec();
    if let Some(cap) = unsafe { INFO_INDICE_CAPACITY.clone() } {
        info_indices.set_capacity(cap);
    }
    let mut file_infos = arc.get_file_infos_as_vec();
    if let Some(cap) = unsafe { FILE_INFO_CAPACITY.clone() } {
        file_infos.set_capacity(cap);
    }
    let mut info_to_datas = arc.get_file_info_to_datas_as_vec();
    if let Some(cap) = unsafe { INFO_TO_DATA_CAPACITY.clone() } {
        info_to_datas.set_capacity(cap);
    }
    let mut datas = arc.get_file_datas_as_vec();
    if let Some(cap) = unsafe { DATA_CAPACITY.clone() } {
        datas.set_capacity(cap);
    }

    let shared_data_idx = unsafe { crate::ORIGINAL_SHARED_INDEX };
    for current_index in dir_info.file_info_range() {
        let current_file_path = file_infos[current_index].file_path_index;
        if !unshared_filepaths.contains(&current_file_path.0) {
            let shared_file_path = get_shared_file(&file_infos[current_index], arc);
            let extension = file_paths[current_file_path.0].ext.hash40();
            if shared_file_path != current_file_path || arc.get_file_in_folder(&file_infos[current_index], Region::None).file_data_index.0 >= shared_data_idx {
                if crate::UNSHARE_ON_DISCOVERY.contains(&extension) {
                    to_unshare.insert(file_paths[current_file_path.0].path.hash40(), (self_index, FileInfoIdx(current_index as u32)));
                    continue;
                }
                file_infos.extend_from_within(info_indices[file_paths[shared_file_path.0].path.index() as usize].file_info_index.0 as usize, 1);
                info_indices.push_from_within(file_paths[shared_file_path.0].path.index() as usize);
                let new_ii = info_indices.last_mut().unwrap();
                new_ii.file_info_index = FileInfoIdx((file_infos.len() - 1) as u32);
                drop(new_ii);
                file_paths[file_infos[current_index].file_path_index.0].path.set_index((info_indices.len() - 1) as u32);
                file_infos[current_index].file_info_indice_index = FileInfoIndiceIdx((info_indices.len() - 1) as u32);
                let current_path_idx = file_infos[current_index].file_path_index;
                let new_fi = file_infos.last_mut().unwrap();
                new_fi.file_path_index = current_path_idx;
                new_fi.file_info_indice_index = FileInfoIndiceIdx((info_indices.len() - 1) as u32);
                if extension == Hash40::from("nutexb") {
                    info_to_datas.extend_from_within(new_fi.info_to_data_index.0 as usize, 1);
                    new_fi.info_to_data_index = InfoToDataIdx((info_to_datas.len() - 1) as u32);
                    let new_itd = info_to_datas.last_mut().unwrap();
                    datas.extend_from_within(new_itd.file_data_index.0 as usize, 1);
                    new_itd.file_data_index = FileDataIdx((datas.len() - 1) as u32);
                }
                
            }
            unshared_filepaths.insert(current_file_path.0);
        }
    }

    unsafe {
        FILE_PATH_CAPACITY = Some(file_paths.capacity());
        INFO_INDICE_CAPACITY = Some(info_indices.capacity());
        FILE_INFO_CAPACITY = Some(file_infos.capacity());
        INFO_TO_DATA_CAPACITY = Some(info_to_datas.capacity());
        DATA_CAPACITY = Some(datas.capacity());
    }

    unshare_children(&dir_info, arc, loaded_tables, unshared_files, to_unshare);
}

pub fn unshare_file(dir_index: u32, index: FileInfoIdx) {
    let loaded_tables = LoadedTables::get_instance();
    let arc = LoadedTables::get_arc();

    let mut unshared_files = UNSHARED_FILES.lock();

    let unshared_filepaths = if let Some(filepaths) = unshared_files.get_mut(&dir_index) {
        filepaths
    } else {
        unshared_files.insert(dir_index, HashSet::new());
        unshared_files.get_mut(&dir_index).unwrap()
    };

    let mut file_paths = arc.get_file_paths_as_vec();
    if let Some(cap) = unsafe { FILE_PATH_CAPACITY.clone() } {
        file_paths.set_capacity(cap);
    }
    let mut info_indices = arc.get_file_info_indices_as_vec();
    if let Some(cap) = unsafe { INFO_INDICE_CAPACITY.clone() } {
        info_indices.set_capacity(cap);
    }
    let mut file_infos = arc.get_file_infos_as_vec();
    if let Some(cap) = unsafe { FILE_INFO_CAPACITY.clone() } {
        file_infos.set_capacity(cap);
    }
    let mut info_to_datas = arc.get_file_info_to_datas_as_vec();
    if let Some(cap) = unsafe { INFO_TO_DATA_CAPACITY.clone() } {
        info_to_datas.set_capacity(cap);
    }
    let mut datas = arc.get_file_datas_as_vec();
    if let Some(cap) = unsafe { DATA_CAPACITY.clone() } {
        datas.set_capacity(cap);
    }
    let mut loaded_datas = loaded_tables.get_loaded_data_table_as_vec();
    if let Some(cap) = unsafe { LOADED_DATA_CAPACITY.clone() } {
        loaded_datas.set_capacity(cap);
    }
    let current_file_path = file_infos[index.0].file_path_index;
    let shared_data_idx = unsafe { crate::ORIGINAL_SHARED_INDEX };
    if !unshared_filepaths.contains(&current_file_path.0) {
        let shared_file_path = get_shared_file(&file_infos[index.0], arc);
        let extension = file_paths[current_file_path.0].ext.hash40();
        if shared_file_path != current_file_path || arc.get_file_in_folder(&file_infos[index.0], Region::None).file_data_index.0 >= shared_data_idx {
            file_infos.extend_from_within(info_indices[file_paths[shared_file_path.0].path.index() as usize].file_info_index.0 as usize, 1);
            info_indices.push_from_within(file_paths[shared_file_path.0].path.index() as usize);
            let new_ii = info_indices.last_mut().unwrap();
            new_ii.file_info_index = FileInfoIdx((file_infos.len() - 1) as u32);
            drop(new_ii);
            file_paths[file_infos[index.0].file_path_index.0].path.set_index((info_indices.len() - 1) as u32);
            file_infos[index.0].file_info_indice_index = FileInfoIndiceIdx((info_indices.len() - 1) as u32);
            let current_path_idx = file_infos[index.0].file_path_index;
            let new_fi = file_infos.last_mut().unwrap();
            new_fi.file_path_index = current_path_idx;
            new_fi.file_info_indice_index = FileInfoIndiceIdx((info_indices.len() - 1) as u32);
            if extension == Hash40::from("nutexb") {
                info_to_datas.extend_from_within(new_fi.info_to_data_index.0 as usize, 1);
                new_fi.info_to_data_index = InfoToDataIdx((info_to_datas.len() - 1) as u32);
                let new_itd = info_to_datas.last_mut().unwrap();
                datas.extend_from_within(new_itd.file_data_index.0 as usize, 1);
                new_itd.file_data_index = FileDataIdx((datas.len() - 1) as u32);
            }
        }
    }
    unshared_filepaths.insert(current_file_path.0);
    loaded_datas.set_len(info_indices.len());
    unsafe {
        FILE_PATH_CAPACITY = Some(file_paths.capacity());
        INFO_INDICE_CAPACITY = Some(info_indices.capacity());
        FILE_INFO_CAPACITY = Some(file_infos.capacity());
        INFO_TO_DATA_CAPACITY = Some(info_to_datas.capacity());
        DATA_CAPACITY = Some(datas.capacity());
        LOADED_DATA_CAPACITY = Some(loaded_datas.capacity());
    }
}

// Old, unused functions being left in for reference
#[allow(dead_code)]
#[allow(unused_variables)]
#[allow(unused_mut)]
pub fn unshare_files_in_directory(directory: Hash40, files: Vec<Hash40>) {
    fn get_shared_hash(info: &FileInfo, arc: &LoadedArc) -> Hash40 {
        let file_paths = arc.get_file_paths();
        let info_indices = arc.get_file_info_indices();
        let file_infos = arc.get_file_infos();
        file_paths[file_infos[info_indices[file_paths[info.file_path_index].path.index() as usize].file_info_index].file_path_index].path.hash40()
    }

    fn get_self_index(info: &DirInfo, arc: &LoadedArc) -> u32 {
        let hash_index = arc.get_dir_hash_to_info_index();
        for hash in hash_index.iter() {
            if hash.hash40() == info.path.hash40() {
                return hash.index();
            }
        }
        0xFF_FFFF
    }

    let files: HashSet<Hash40> = files.into_iter().map(|x| x).collect();

    let loaded_tables = LoadedTables::acquire_instance();
    let arc = LoadedTables::get_arc();

    let dir_info = if let Ok(info) = arc.get_dir_info_from_hash(directory) {
        info.clone()
    } else {
        return;
    };

    arc.get_file_groups_as_vec()[dir_info.path.index()].directory_index = 0xFF_FFFF;

    let dir_infos = arc.get_dir_infos();
    let mut unshared_files = UNSHARED_FILES.lock();
    let mut unshared_filepaths = None;
    for (idx, filepaths) in unshared_files.iter_mut() {
        if dir_infos[*idx as usize].path.hash40() == dir_info.path.hash40() {
            unshared_filepaths = Some(filepaths);
            break;
        }
    } 
    let unshared_filepaths = if let Some(filepaths) = unshared_filepaths {
        filepaths
    } else {
        let self_index = get_self_index(&dir_info, arc);
        if self_index == 0xFF_FFFF {
            return;
        }
        unshared_files.insert(self_index, HashSet::new());
        unshared_files.get_mut(&self_index).unwrap()
    };

    let mut file_paths = arc.get_file_paths_as_vec();
    let mut info_indices = arc.get_file_info_indices_as_vec();
    let mut file_infos = arc.get_file_infos_as_vec();
    let mut info_to_datas = arc.get_file_info_to_datas_as_vec();
    
    let file_info_range = dir_info.file_info_range();
    for current_index in file_info_range {
        if !unshared_filepaths.contains(&file_infos[current_index].file_path_index.0) {
            let file_hash = file_paths[file_infos[current_index].file_path_index.0].path.hash40();
            if files.contains(&file_hash) {
                let shared_hash = get_shared_hash(&file_infos[current_index], arc);
                if shared_hash != file_hash {
                    file_infos.extend_from_within(info_indices[file_paths[file_infos[current_index].file_path_index.0].path.index()].file_info_index.0 as usize, 1);
                    info_indices.push_from_within(file_paths[file_infos[current_index].file_path_index.0].path.index() as usize);
                    let new_ii = info_indices.last_mut().unwrap();
                    new_ii.file_info_index = FileInfoIdx((file_infos.len() - 1) as u32);
                    drop(new_ii);
                    file_paths[file_infos[current_index].file_path_index.0].path.set_index((info_indices.len() - 1) as u32);
                    let current_path_idx = file_infos[current_index].file_path_index;
                    let new_fi = file_infos.last_mut().unwrap();
                    new_fi.file_path_index = current_path_idx;
                    new_fi.file_info_indice_index = FileInfoIndiceIdx((info_indices.len() - 1) as u32);
                    unshared_filepaths.insert(new_fi.file_path_index.0);
                    info_to_datas[new_fi.info_to_data_index.0].file_info_index_and_flag = 0x0100_0000;
                    drop(new_fi);
                    file_infos[current_index].file_info_indice_index = FileInfoIndiceIdx((info_indices.len() - 1) as u32);
                }
            }
        }
    }
    loaded_tables.get_loaded_data_table_as_vec().set_len(info_indices.len());
}

// Old, unused functions being left in for reference
#[allow(dead_code)]
#[allow(unused_variables)]
#[allow(unused_mut)]
pub fn unshare_file_in_directory(directory: Hash40, file: Hash40) {
    fn get_shared_hash(info: &FileInfo, arc: &LoadedArc) -> Hash40 {
        let file_paths = arc.get_file_paths();
        let info_indices = arc.get_file_info_indices();
        let file_infos = arc.get_file_infos();
        file_paths[file_infos[info_indices[file_paths[info.file_path_index].path.index() as usize].file_info_index].file_path_index].path.hash40()
    }

    let loaded_tables = LoadedTables::acquire_instance();
    let arc = LoadedTables::get_arc_mut();

    let dir_info = if let Ok(info) = arc.get_dir_info_from_hash(directory) {
        info.clone()
    } else {
        return;
    };

    let mut file_paths = arc.get_file_paths_as_vec();
    let mut info_indices = arc.get_file_info_indices_as_vec();
    let mut file_infos = arc.get_file_infos_as_vec();
    let mut info_to_datas = arc.get_file_info_to_datas_as_vec();
    let mut file_groups = arc.get_file_groups_as_vec();
    file_groups[dir_info.path.index()].directory_index = 0xFFFFFF;

    let file_info_range = dir_info.file_info_range();
    for current_index in file_info_range.clone() {
        if file_paths[file_infos[current_index].file_path_index.0].path.hash40() == file {
            cli::send("inside of first test");
            let shared_hash = get_shared_hash(&file_infos[current_index], arc);
            if shared_hash != file {
                cli::send("inside of second test");
                // let file_info = file_infos[current_index];
                // file_infos.push(file_info);
                file_infos.extend_from_within(info_indices[file_paths[file_infos[current_index].file_path_index.0].path.index()].file_info_index.0 as usize, 1);
                // file_infos.push_from_within(current_index);
                info_indices.push_from_within(file_paths[file_infos[current_index].file_path_index.0].path.index() as usize);
                let new_ii = info_indices.last_mut().unwrap();
                new_ii.file_info_index = FileInfoIdx((file_infos.len() - 1) as u32);
                drop(new_ii);
                let new_fi = file_infos.last_mut().unwrap();
                file_paths[new_fi.file_path_index.0].path.set_index((info_indices.len() - 1) as u32);
                new_fi.file_info_indice_index = FileInfoIndiceIdx((info_indices.len() - 1) as u32);
            }
        }
    }
    loaded_tables.get_loaded_data_table_as_vec().set_len(info_indices.len());
}