use arc_vector::ArcVector;
use smash_arc::*;
use crate::runtime::*;
use parking_lot::Mutex;
use std::collections::HashMap;
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

trait LoadedTableAdditions {
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
}

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
            }
        }
        file_groups[shared_file_group.directory_index].file_start_index = og_fi_len as u32;
        loaded_tables.get_filepath_table_as_vec().set_len(file_paths.len());
        loaded_tables.get_loaded_data_table_as_vec().set_len(file_paths.len());
        already_reshared.push(shared_file_group.directory_index);
    }

    let reshared = RESHARED_FILEPATHS.lock();

    // let arc = LoadedTables::get_arc();
    let shared_start = arc.get_shared_data_index();
    for file_info in file_infos.iter_mut().skip(dir_info.file_info_start_index as usize).take(dir_info.file_count as usize) {
        // Check if the FileData is past the "shared offset". If so, then we need to search the FilePaths for the right information
        if arc.get_file_in_folder(file_info, Region::None).file_data_index.0 >= shared_start {
            // Get the deeper shared FilePath hash from this.
            let shared_file_hash = get_shared_hash(file_info, arc);
            // Change it to the new hash we created for accessing the reshared files
            let backup_hash = Hash40((shared_file_hash.as_u64() & 0xFFFF_FFFF) | 0x69_0000_0000);
            if let Some(path_idx) = reshared.get(&backup_hash) {
                cli::send(format!("b: {:#x}", backup_hash.0).as_str());
                // The reshared FilePath was found, change this FileInfo's information to reflect it
                let new_ii_index = file_paths[usize::from(*path_idx)].path.index();
                file_paths[usize::from(file_info.file_path_index)].path.set_index(new_ii_index);
                file_info.file_info_indice_index = FileInfoIndiceIdx(new_ii_index);
            } else {
                cli::send(format!("ub: {:#x}", backup_hash.0).as_str());
            }
        }
    }
}