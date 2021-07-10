use arc_vector::ArcVector;
use smash_arc::*;
use crate::runtime::*;
trait LoadedArcAdditions {
    fn get_dir_infos_as_vec(&self) -> ArcVector<DirInfo>;
    fn get_file_paths_as_vec(&self) -> ArcVector<FilePath>;
    fn get_file_info_indices_as_vec(&self) -> ArcVector<FileInfoIndex>;
    fn get_file_infos_as_vec(&self) -> ArcVector<FileInfo>;
    fn get_file_info_to_datas_as_vec(&self) -> ArcVector<FileInfoToFileData>;
    fn get_file_datas_as_vec(&self) -> ArcVector<FileData>;
    fn get_file_groups_as_vec(&self) -> ArcVector<DirectoryOffset>;
    fn get_path_to_index_as_vec(&self) -> ArcVector<HashToIndex>;
    fn recreate_file_buckets(&mut self, bucket_count: usize);
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

    fn recreate_file_buckets(&mut self, count: usize) {
        use skyline::libc::*;
        // currently unimplemented, but since we access via index I'm not gonna worry *yet*
        let file_info_buckets = unsafe { malloc((count + 1) * std::mem::size_of::<FileInfoBucket>()) as *mut FileInfoBucket};
        let file_info_buckets = unsafe { std::slice::from_raw_parts_mut(file_info_buckets, count + 1) };
        let path_to_hashes = self.get_path_to_index_as_vec();
        let mut new_path_to_hashes = Vec::with_capacity(path_to_hashes.len());
        file_info_buckets[0].count = count as u32;
        let mut start = 0;
        for x in 0..count {
            for pth in path_to_hashes.iter() {
                if pth.hash40().as_u64() % (count as u64) == (x as u64) {
                    new_path_to_hashes.push(pth.clone());
                }
            }
            file_info_buckets[x + 1].start = start;
            file_info_buckets[x + 1].count = (new_path_to_hashes.len() as u32) - start;
            start = new_path_to_hashes.len() as u32;
        }
        drop(path_to_hashes);
        let new_path_to_hashes = new_path_to_hashes.leak();
        self.file_hash_to_path_index = new_path_to_hashes.as_ptr();
        self.file_info_buckets = file_info_buckets.as_ptr();
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