use std::fmt;
use std::sync::atomic::AtomicU32;

use skyline::{
    hooks::{getRegionAddress, Region},
    nn,
};

use smash_arc::{
    ArcLookup, FileData, FileInfo, FileInfoIndiceIdx, FilePath, FilePathIdx, LoadedArc,
};

use smash_arc::LoadedSearchSection;

use crate::replacement_files::get_region_id;

use crate::config::CONFIG;

use crate::cpp_vector::CppVector;

use log::info;
use owo_colors::OwoColorize;

// 9.0.1 offsets
pub static mut LOADED_TABLES_OFFSET: usize = 0x50567a0;
pub static mut RES_SERVICE_OFFSET: usize = 0x50567a8;

pub fn offset_to_addr(offset: usize) -> *const () {
    unsafe { (getRegionAddress(Region::Text) as *const u8).offset(offset as isize) as _ }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
#[allow(dead_code)]
pub enum FileState {
    Unloaded = 0,
    Unused = 1,
    Unk2 = 2,
    Loaded = 3,
}

impl fmt::Display for FileState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[repr(C)]
#[repr(packed)]
pub struct Table1Entry {
    pub table2_index: u32,
    pub in_table_2: u32,
}

impl Table1Entry {
    #[allow(dead_code)]
    pub fn get_t2_entry(&self) -> Result<&Table2Entry, LoadError> {
        LoadedTables::get_instance()
            .table_2()
            .get(self.table2_index as usize)
            .ok_or(LoadError::NoTable2)
    }
}

impl fmt::Display for Table1Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            write!(
                f,
                "Table2 index: {} (In Table2: {})",
                self.table2_index,
                self.in_table_2 != 0
            )
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct Table2Entry {
    pub data: *const u8,
    pub ref_count: AtomicU32,
    pub is_used: bool,
    pub state: FileState,
    pub file_flags2: bool,
    pub flags: u8,
    pub version: u32,
    pub unk: u8,
}

impl fmt::Display for Table2Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "State: {}, Flags: {}, RefCount: {:x?}, Data loaded: {}, Version: {:#x}, Unk: {}",
            self.state,
            self.flags,
            self.ref_count,
            !self.data.is_null(),
            self.version,
            self.unk
        )
    }
}

#[repr(C)]
pub struct LoadedTables {
    pub mutex: *mut nn::os::MutexType,
    pub table1: *mut Table1Entry,
    pub table2: *mut Table2Entry,
    pub table1_len: u32,
    pub table2_len: u32,
    pub table1_count: u32,
    pub table2_count: u32,
    pub table1_list: CppVector<u32>,
    pub loaded_directory_table: *const LoadedDirectory,
    pub loaded_directory_table_size: u32,
    pub unk2: u32,
    pub unk3: CppVector<u32>,
    pub unk4: u8,
    pub unk5: [u8; 7],
    pub addr: *const (),
    pub loaded_data: &'static mut LoadedData,
    pub version: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct LoadedDirectory {
    pub directory_offset_index: u32,
    pub dir_count: u32,
    unk: u64,
    pub child_files_indexes: CppVector<u32>,
    pub child_folders: CppVector<*mut LoadedDirectory>,
    pub redirection_dir: *const LoadedDirectory,
}

#[repr(C)]
pub struct LoadedData {
    pub arc: &'static mut LoadedArc,
    pub search: &'static mut LoadedSearchSection,
}

#[repr(C)]
#[allow(dead_code)]
pub struct FsSearchBody {
    pub file_path_length: u32,
    pub idx_length: u32,
    pub path_group_length: u32,
}

pub struct TableGuard {
    tables: &'static mut LoadedTables
}

impl std::ops::Deref for TableGuard {
    type Target = LoadedTables;

    fn deref(&self) -> &LoadedTables {
        self.tables
    }
}

impl std::ops::DerefMut for TableGuard {
    fn deref_mut(&mut self) -> &mut LoadedTables {
        self.tables
    }
}

impl TableGuard {
    pub fn new() -> Self {
        let instance: &'static mut LoadedTables = LoadedTables::get_instance();
        instance.lock();
        Self {
            tables: instance
        }
    }
}

impl Drop for TableGuard {
    fn drop(&mut self) {
        self.tables.unlock();
    }
}


use smash_arc::*;
#[derive(Debug)]
pub struct ArrayLengths {
    pub dir_infos: u32,
    pub file_paths: u32,
    pub file_info_indices: u32,
    pub file_infos: u32,
    pub file_info_to_datas: u32,
    pub file_datas: u32,
    pub folder_offsets: u32
}

impl ArrayLengths {
    pub fn new() -> Self {
        let arc = LoadedTables::get_arc();
        let fs = unsafe { *arc.fs_header };
        Self {
            dir_infos: fs.folder_count,
            file_paths: fs.file_info_path_count,
            file_info_indices: fs.file_info_index_count,
            file_infos: fs.file_info_count + fs.file_data_count_2 + fs.extra_count,
            file_info_to_datas: fs.file_info_sub_index_count + fs.file_data_count_2 + fs.extra_count_2,
            file_datas: fs.file_data_count + fs.file_data_count_2 + fs.extra_count,
            folder_offsets: fs.folder_offset_count_1 + fs.folder_offset_count_2 // + fs.extra_folder
        }
    }
}

impl LoadedTables {

    #[inline]
    pub fn lock(&mut self) {
        unsafe { nn::os::LockMutex(self.mutex); }
    }

    #[inline]
    pub fn unlock(&mut self) {
        unsafe { nn::os::UnlockMutex(self.mutex); }
    }

    unsafe fn recreate_array<T: Sized>(start: *const T, length: usize, new_entries: &Vec<T>) -> *mut T {
        let arr_layout = std::alloc::Layout::from_size_align((length + new_entries.len()) * std::mem::size_of::<T>(), 0x10).unwrap();
        let new_ptr = std::alloc::alloc(arr_layout) as *mut T;
        std::ptr::copy_nonoverlapping(start, new_ptr, length);
        std::ptr::copy_nonoverlapping(new_entries.as_ptr(), new_ptr.offset(length as isize), new_entries.len());
        new_ptr
    }

    unsafe fn extend_table<T: Sized>(start: *const T, length: usize, new_entries: usize) -> *mut T {
        let arr_layout = std::alloc::Layout::from_size_align((length + new_entries) * std::mem::size_of::<T>(), 0x10).unwrap();
        let new_ptr = std::alloc::alloc(arr_layout) as *mut T;
        std::ptr::copy_nonoverlapping(start, new_ptr, length);
        new_ptr
    }
    
    // pub unsafe fn deduplicate_mass_loading_group2<Hash: Into<Hash40> + Clone>(path: Hash) -> Result<(), LookupError> {
    //     use std::collections::HashMap;
    //     let path: Hash40 = path.clone().into();
    //     let mut instance = Self::acquire_instance();
    //     let arc = Self::get_arc_mut();
    //     let fs: &'static mut FileSystemHeader = std::mem::transmute(arc.fs_header);
    //     let mut lengths = ArrayLengths::new();

    //     let dir_infos: &[LoadedDirInfo] = std::slice::from_raw_parts(arc.dir_infos, lengths.dir_infos as usize);
    //     let folder_offsets: &mut [DirectoryOffset]        = std::slice::from_raw_parts_mut(arc.folder_offsets as *mut DirectoryOffset, lengths.folder_offsets as usize);
    //     let file_paths: &mut [FilePath]                   = std::slice::from_raw_parts_mut(arc.file_paths as *mut FilePath, lengths.file_paths as usize);
    //     let file_info_indices: &mut [FileInfoIndex]       = std::slice::from_raw_parts_mut(arc.file_info_indices as *mut FileInfoIndex, lengths.file_info_indices as usize);
    //     let file_infos: &mut [FileInfo]                   = std::slice::from_raw_parts_mut(arc.file_infos, lengths.file_infos as usize);
    //     let file_info_to_datas: &mut [FileInfoToFileData] = std::slice::from_raw_parts_mut(arc.file_info_to_datas, lengths.file_info_to_datas as usize);
    //     let file_datas: &mut [FileData]                   = std::slice::from_raw_parts_mut(arc.file_datas, lengths.file_datas as usize);

    //     let mass_load_group = arc.get_dir_info_from_hash(path)?;
    //     let intermediate_load_data = &mut folder_offsets[(mass_load_group.dir_offset_index >> 8) as usize];
    //     let old_res_idx = intermediate_load_data.resource_index;
    //     intermediate_load_data.resource_index = lengths.folder_offsets;
    //     drop(intermediate_load_data);
    //     let shared_load_data = &folder_offsets[old_res_idx as usize];

    //     let mass_load_group_infos = std::slice::from_raw_parts_mut(arc.file_infos.offset(mass_load_group.file_info_start_index as isize), mass_load_group.file_info_count as usize);
    //     let shared_load_data_infos = std::slice::from_raw_parts_mut(arc.file_infos.offset(shared_load_data.file_info_start_index as isize), shared_load_data.file_info_count as usize);

    //     let mut index_to_data_hash_map = HashMap::new();
    //     for info in shared_load_data_infos.iter() {
    //         let hash = file_paths[info.file_path_index.0 as usize].path.hash40();
    //         let path_idx = file_infos[file_info_indices[info.file_info_indice_index.0 as usize].file_info_index.0 as usize].file_path_index.0;
    //         index_to_data_hash_map.insert(path_idx, hash);
    //     }
    //     let mut data_to_group_hash_map = HashMap::new();
    //     let mut group_to_index_hash_map = HashMap::new();
    //     for (idx, info) in mass_load_group_infos.iter().enumerate() {
    //         let path_idx = file_infos[file_info_indices[info.file_info_indice_index.0 as usize].file_info_index.0 as usize].file_path_index.0;
    //         if let Some(data_hash) = index_to_data_hash_map.get(&path_idx) {
    //             let group_hash = file_paths[info.file_path_index.0 as usize].path.hash40();
    //             if *data_hash == group_hash {
    //                 return Err(LookupError::Missing);
    //             }
    //             data_to_group_hash_map.insert(*data_hash, group_hash);
    //             group_to_index_hash_map.insert(group_hash, idx);
    //         }
    //     }
    //     drop(index_to_data_hash_map);

    //     let default_data = file_infos[0];
    //     let mut new_info_indices = Vec::new();
    //     let mut new_info_to_datas = Vec::new();
    //     let mut new_infos = Vec::new();
    //     let mut new_datas = Vec::new();
    //     new_infos.resize_with(shared_load_data_infos.len(), || default_data);
    //     for (idx, info) in shared_load_data_infos.iter_mut().enumerate() {
    //         let source_info_hash = file_paths[info.file_path_index.0 as usize].path.hash40();
    //         let new_info_hash = data_to_group_hash_map.get(&source_info_hash).expect(&format!("Could not find new hash for source hash {:X}", source_info_hash.0));
    //         let info_idx = group_to_index_hash_map.get(&new_info_hash).expect(&format!("Could not find info index for new hash {:X}", new_info_hash.0));
    //         let original_indice_idx = mass_load_group_infos[*info_idx].file_info_indice_index;
    //         let original_info_idx = file_info_indices[original_indice_idx.0 as usize];
    //         let mut original_data = file_datas[file_info_to_datas[file_infos[original_info_idx.file_info_index.0 as usize].info_to_data_index.0 as usize].file_data_index.0 as usize];
    //         let new_path_idx = mass_load_group_infos[*info_idx as usize].file_path_index;
    //         mass_load_group_infos[*info_idx as usize].file_info_indice_index = FileInfoIndiceIdx(lengths.file_info_indices + new_info_indices.len() as u32);
    //         mass_load_group_infos[*info_idx as usize].info_to_data_index = InfoToDataIdx(lengths.file_info_to_datas + new_info_to_datas.len() as u32);

    //         new_infos[idx] = info.clone();
    //         new_infos[idx].file_path_index = new_path_idx;
    //         new_infos[idx].file_info_indice_index = FileInfoIndiceIdx(lengths.file_info_indices + new_info_indices.len() as u32);
    //         new_infos[idx].info_to_data_index = InfoToDataIdx(lengths.file_info_to_datas + new_info_to_datas.len() as u32);

    //         let mut redirect_info_idx = shared_load_data.file_info_start_index + idx as u32;
    //         let mut redirect_info = info;
    //         let mut redirect_info_to_data = &mut file_info_to_datas[redirect_info.info_to_data_index.0 as usize];
    //         let shared_path_idx = file_infos[file_info_indices[redirect_info.file_info_indice_index.0 as usize].file_info_index.0 as usize].file_path_index;
    //         let mut is_first = true;
    //         loop {
    //             if !is_first {
    //                 let mut new_info = redirect_info.clone();
    //                 new_info.info_to_data_index = InfoToDataIdx(lengths.file_info_to_datas + new_info_to_datas.len() as u32);
    //                 new_info.file_path_index = new_path_idx;
    //                 new_info.file_info_indice_index = FileInfoIndiceIdx(lengths.file_info_indices + new_info_indices.len() as u32);
    //                 println!("{:X}", file_paths[new_path_idx.0 as usize].path.hash40().0);
    //                 new_infos.push(new_info);
    //             } else {
    //                 is_first = false;
    //             }
    //             redirect_info_to_data = &mut file_info_to_datas[redirect_info.info_to_data_index.0 as usize];
    //             let new_info_idx = redirect_info_to_data.file_info_index_and_flag & 0xFFFFFF;
    //             redirect_info = &mut file_infos[new_info_idx as usize];
    //             let (is_chain, idx_and_flag) = if redirect_info.file_path_index != shared_path_idx {
    //                 let flag = redirect_info_to_data.file_info_index_and_flag & 0xFF000000;
    //                 let info_idx = lengths.file_infos + new_infos.len() as u32;
    //                 (false, redirect_info_to_data.file_info_index_and_flag)
    //             } else {
    //                 let flag = redirect_info_to_data.file_info_index_and_flag;
    //                 (true, flag | flag)
    //             };
    //             if is_chain {
    //                 let mut new_info_to_data = redirect_info_to_data.clone();
    //                 new_info_to_data.file_info_index_and_flag = idx_and_flag;
    //                 new_info_to_data.file_data_index = FileDataIdx(lengths.file_datas + new_datas.len() as u32);
    //                 new_info_to_data.folder_offset_index = lengths.folder_offsets;
    //                 new_info_to_datas.push(new_info_to_data);
    //             } else { 
    //                 let mut new_info_to_data = redirect_info_to_data.clone();
    //                 new_info_to_data.file_info_index_and_flag = idx_and_flag;
    //                 new_info_to_data.file_data_index = FileDataIdx(lengths.file_datas + new_datas.len() as u32);
    //                 new_info_to_data.folder_offset_index = lengths.folder_offsets;
    //                 new_info_to_datas.push(new_info_to_data);
    //                 // let mut new_info = redirect_info.clone();
    //                 // new_info.file_path_index = new_path_idx;
    //                 // new_info.file_info_indice_index = FileInfoIndiceIdx(lengths.file_info_indices + new_info_indices.len() as u32);
    //                 // new_infos.push(new_info);
    //                 break;
    //             }
    //         }
    //         file_paths[new_path_idx.0 as usize].path.set_index(lengths.file_info_indices + new_info_indices.len() as u32);
    //         // file_info_indices[original_indice_idx.0 as usize].file_info_index = FileInfoIdx(lengths.file_infos + idx as u32);
    //         new_datas.push(original_data);
    //         let mut new_info_index = original_info_idx;
    //         new_info_index.dir_offset_index = lengths.folder_offsets;
    //         new_info_index.file_info_index = FileInfoIdx(lengths.file_infos + idx as u32);
    //         new_info_indices.push(new_info_index);
    //         // info.info_to_data_index = InfoToDataIdx(0xFFFFFF);

    //     }

    //     let deduplicated_load_data = DirectoryOffset {
    //         offset: shared_load_data.offset,
    //         size: shared_load_data.size,
    //         decomp_size: shared_load_data.decomp_size,
    //         file_info_start_index: lengths.file_infos,
    //         file_info_count: shared_load_data.file_info_count,
    //         resource_index: lengths.folder_offsets
    //     };

    //     arc.folder_offsets = Self::recreate_array(arc.folder_offsets, lengths.folder_offsets as usize, &vec![deduplicated_load_data.clone()]);
    //     arc.file_info_indices = Self::recreate_array(arc.file_info_indices, lengths.file_info_indices as usize, &new_info_indices);
    //     arc.file_infos = Self::recreate_array(arc.file_infos, lengths.file_infos as usize, &new_infos);
    //     arc.file_info_to_datas = Self::recreate_array(arc.file_info_to_datas, lengths.file_info_to_datas as usize, &new_info_to_datas);
    //     arc.file_datas = Self::recreate_array(arc.file_datas, lengths.file_datas as usize, &new_datas);
    //     instance.table2 = Self::extend_table(instance.table2, instance.table2_len as usize, new_info_indices.len());
    //     instance.loaded_directory_table = Self::extend_table(instance.loaded_directory_table, instance.loaded_directory_table_size as usize, 1);
    //     instance.loaded_directory_table_size += 1;
    //     fs.folder_offset_count_1 += 1;
    //     fs.file_info_index_count += new_info_indices.len() as u32;
    //     fs.file_info_sub_index_count += new_info_to_datas.len() as u32;
    //     fs.file_info_count += new_infos.len() as u32;
    //     fs.file_data_count += new_datas.len() as u32;
    //     instance.table2_len += new_info_indices.len() as u32;

    //     Ok(())
    // }

    fn duplicate_file_structure(new_file_path_idx: FilePathIdx, new_info_indice_idx: FileInfoIndiceIdx, start_info_idx: FileInfoIdx, new_data_index: FileDataIdx, new_mass_load_data_index: u32, start_info: &FileInfo, info_to_datas: &mut Vec<FileInfoToFileData>, infos: &mut Vec<FileInfo>, lengths: &ArrayLengths) {
        let arc = Self::get_arc();
        let file_info_indices = arc.get_file_info_indices();
        let file_infos = arc.get_file_infos();
        let file_info_to_datas = arc.get_file_info_to_datas();

        let mut redirect_info_idx = start_info_idx;
        let mut redirect_info = start_info;
        let mut redirect_info_to_data = &file_info_to_datas[usize::from(redirect_info.info_to_data_index)];
        let idx = file_info_indices[usize::from(redirect_info.file_info_indice_index)].file_info_index;
        let shared_path_idx = file_infos[usize::from(idx)].file_path_index;
        let mut is_first = true;
        loop {
            if is_first { is_first = false }
            else {
                let new_info = FileInfo {
                    file_info_indice_index: new_info_indice_idx,
                    file_path_index: new_file_path_idx,
                    info_to_data_index: InfoToDataIdx(lengths.file_info_to_datas + info_to_datas.len() as u32),
                    flags: redirect_info.flags
                };
                infos.push(new_info);
            }
            redirect_info_to_data = &file_info_to_datas[usize::from(redirect_info.info_to_data_index)];
            let new_info_idx = redirect_info_to_data.file_info_index_and_flag & 0xFFFFFF;
            redirect_info = &file_infos[new_info_idx as usize];
            let (is_chain, idx_and_flag) = if redirect_info.file_path_index != shared_path_idx {
                (false, redirect_info_to_data.file_info_index_and_flag)
            } else {
                let flag = redirect_info_to_data.file_info_index_and_flag & 0xFF000000;
                let info_idx = lengths.file_infos + infos.len() as u32;
                (true, flag | info_idx)
            };
            let new_info_to_data = FileInfoToFileData {
                file_info_index_and_flag: idx_and_flag,
                file_data_index: new_data_index,
                folder_offset_index: new_mass_load_data_index
            };
            info_to_datas.push(new_info_to_data);
            if !is_chain { break; }
        }
    }

    pub fn unshare_mass_loading_groups<Hash: Into<Hash40> + Clone>(paths: &Vec<Hash>) -> Result<(), LookupError> {
        lazy_static::lazy_static! {
            static ref BANNED_FILENAMES: Vec<Hash40> = vec![
                Hash40::from("model.xmb")
            ];
        }
        use std::collections::HashMap;
        let paths: Vec<Hash40> = paths.iter().map(|x| {
            x.clone().into()
        }).collect();
        unsafe {
            // get the loaded structures
            let mut instance = Self::acquire_instance(); // acquiring will lock the mutex and unlock on drop
            let arc = Self::get_arc_mut();
            let fs: &'static mut FileSystemHeader = std::mem::transmute(arc.fs_header);
            let uncompressed_fs: &'static mut FileSystemHeader = std::mem::transmute(arc.uncompressed_fs);
            let lengths = ArrayLengths::new(); // get array lengths as u32 values, simplifies making the indices
            println!("{:?}", lengths);

            let folder_offsets = std::slice::from_raw_parts_mut(arc.folder_offsets as *mut DirectoryOffset, lengths.folder_offsets as usize);
            let file_paths = std::slice::from_raw_parts_mut(arc.file_paths as *mut FilePath, lengths.file_paths as usize);
            let file_info_indices = std::slice::from_raw_parts(arc.file_info_indices, lengths.file_info_indices as usize);
            let file_infos = std::slice::from_raw_parts_mut(arc.file_infos, lengths.file_infos as usize);
            let file_info_to_datas = std::slice::from_raw_parts(arc.file_info_to_datas, lengths.file_info_to_datas as usize);
            let file_datas = std::slice::from_raw_parts(arc.file_datas, lengths.file_datas as usize);

            let default_info = file_infos[0]; // default data to reserve sizes with

            // declare the arrays to be filled in later
            let mut new_info_indices = Vec::new();
            let mut new_infos = Vec::new();
            let mut new_info_to_datas = Vec::new();
            let mut new_datas = Vec::new();
            let mut new_mass_load_datas = Vec::new();

            let mut mass_load_data_start_offset = 0;
            for path in paths.iter() {
                // start by changing the index of the load data
                let mass_load_group = arc.get_dir_info_from_hash(*path)?;
                let intermediate_load_data = &mut folder_offsets[(mass_load_group.dir_offset_index >> 8) as usize]; // ideally change this in smash-arc
                let old_res_idx = intermediate_load_data.resource_index;
                intermediate_load_data.resource_index = lengths.folder_offsets + new_mass_load_datas.len() as u32;
                drop(intermediate_load_data); // can't mutably borrow twice at once, so drop
                let shared_load_data = &folder_offsets[old_res_idx as usize];

                let mass_load_group_infos = std::slice::from_raw_parts_mut(
                    arc.file_infos.offset(mass_load_group.file_info_start_index as isize),
                    mass_load_group.file_info_count as usize
                );
                let shared_load_data_infos = std::slice::from_raw_parts_mut(
                    arc.file_infos.offset(shared_load_data.file_info_start_index as isize),
                    shared_load_data.file_info_count as usize
                );

                // get the file path index and map it to a hash
                // the FileInfoIndex for a group file (i.e. "fighter/roy/model/body/c00/model.numshb")
                // is the same as one for the shared data file. In the case of Roy, they all point to the c07 variants
                // However, it goes deeper than this, as some files are shared even further. For example,
                // "fighter/roy/model/body/c00/model.xmb" redirects to "fighter/roy/model/body/c07/model.xmb"
                // which further redirects to fox's model.xmb.
                // ^^ lol this doesn't work
                // In order to unshare these files we have to go two redirections deep to be 100% sure we map the right hashes together
                let mut index_to_data_hash_map = HashMap::new();
                for info in shared_load_data_infos.iter() {
                    let hash = file_paths[usize::from(info.file_path_index)].path.hash40();
                    let idx = file_info_indices[usize::from(info.file_info_indice_index)].file_info_index;
                    let idx = file_infos[usize::from(idx)].file_path_index;
                    index_to_data_hash_map.insert(idx, hash);
                }
                // we do this to be able to go from shared data hash -> mass load group hash
                // this is important so that the file path indices in our new MassLoadData point
                // to the original filepaths
                let mut data_to_group_hash_map = HashMap::new();
                let mut group_to_index_hash_map = HashMap::new();
                for (offset, info) in mass_load_group_infos.iter().enumerate() {
                    let idx = file_info_indices[usize::from(info.file_info_indice_index)].file_info_index;
                    let idx = file_infos[usize::from(idx)].file_path_index;
                    if let Some(data_hash) = index_to_data_hash_map.get(&idx) {
                        let group_hash = file_paths[usize::from(info.file_path_index)].path.hash40();
                        if *data_hash == group_hash {
                            return Err(LookupError::Missing); // TODO: Change this to a better error code or smthn
                        }
                        data_to_group_hash_map.insert(*data_hash, group_hash);
                        group_to_index_hash_map.insert(group_hash, offset);
                    }
                }
                drop(index_to_data_hash_map);

                let mut path_idx_to_info_indice = HashMap::new();

                // since we also have to include our redirection chain, we have to make sure we get contigious FileInfos
                // for our new MassLoadData
                let current_mld_offset = new_infos.len();
                new_infos.resize_with(current_mld_offset + shared_load_data_infos.len(), || default_info);
                for (offset, info) in shared_load_data_infos.iter().enumerate() {
                    // get the shared hash
                    let source_info_hash = file_paths[usize::from(info.file_path_index)].path.hash40();
                    let new_info_hash = data_to_group_hash_map.get(&source_info_hash).expect(&format!("Could not find new hash for source {:#x?}", source_info_hash));
                    // extract the MassLoadGroup's child info for this
                    let child_info_offset = group_to_index_hash_map.get(&new_info_hash).expect(&format!("Could not find info index for new hash {:#x?}", new_info_hash));
                    let child_info = &mut mass_load_group_infos[*child_info_offset];
                    // get the data this pointed to
                    let idx_ = child_info.file_info_indice_index;
                    let idx_ = file_info_indices[usize::from(idx_)].file_info_index;
                    let info_to_data_idx = file_infos[usize::from(idx_)].info_to_data_index;
                    let idx_ = file_info_to_datas[usize::from(info_to_data_idx)].file_data_index;
                    let original_data = file_datas[usize::from(idx_)];

                    // Create our indices for repeated use
                    let new_info_indice_index = FileInfoIndiceIdx(lengths.file_info_indices + new_info_indices.len() as u32);
                    let new_info_index_start = FileInfoIdx(lengths.file_infos + (current_mld_offset + offset) as u32);
                    let new_info_to_data_index_start = InfoToDataIdx(lengths.file_info_to_datas + new_info_to_datas.len() as u32);
                    let new_data_index = FileDataIdx(lengths.file_datas + new_datas.len() as u32);
                    let new_mass_load_data_index = lengths.folder_offsets + new_mass_load_datas.len() as u32;

                    // get the FilePathIdx for the group file
                    let group_path_idx = child_info.file_path_index;
                    let file_name = file_paths[usize::from(group_path_idx)].file_name.hash40();
                    let is_banned = BANNED_FILENAMES.contains(&file_name);

                    // manufacture the index which goes into the contiguous data section
                    new_infos[current_mld_offset + offset] = info.clone();
                    if !is_banned {
                        child_info.file_info_indice_index = new_info_indice_index; // points to a yet to be created FileInfoIndex
                        child_info.info_to_data_index = new_info_to_data_index_start; // yet to be created file data
                        new_infos[current_mld_offset + offset].file_info_indice_index = new_info_indice_index;
                        new_infos[current_mld_offset + offset].file_path_index = group_path_idx;
                        new_infos[current_mld_offset + offset].info_to_data_index = new_info_to_data_index_start;

                        Self::duplicate_file_structure(
                            group_path_idx,
                            new_info_indice_index,
                            FileInfoIdx(shared_load_data.file_info_start_index + offset as u32),
                            new_data_index,
                            new_mass_load_data_index,
                            info,
                            &mut new_info_to_datas,
                            &mut new_infos,
                            &lengths
                        );
                        file_paths[usize::from(group_path_idx)].path.set_index(new_info_indice_index.0);
                        path_idx_to_info_indice.insert(group_path_idx, (new_info_indice_index, new_info_to_data_index_start));
                        new_datas.push(original_data);
                    }
                    let new_info_index = FileInfoIndex {
                        dir_offset_index: new_mass_load_data_index,
                        file_info_index: new_info_index_start
                    };
                    new_info_indices.push(new_info_index);
                }
                let new_mass_load_data = DirectoryOffset {
                    offset: shared_load_data.offset,
                    size: shared_load_data.size,
                    decomp_size: shared_load_data.decomp_size,
                    file_info_start_index: lengths.file_infos + current_mld_offset as u32,
                    file_info_count: shared_load_data.file_info_count,
                    resource_index: lengths.folder_offsets + new_mass_load_datas.len() as u32
                };
                new_mass_load_datas.push(new_mass_load_data);

                continue;

                // unshare the rest of the chads
                // still WIP
                for (offset, info) in mass_load_group_infos.iter_mut().enumerate() {
                    if BANNED_FILENAMES.contains(&file_paths[usize::from(info.file_path_index)].file_name.hash40()) { 
                        continue;
                    }
                    if let Some((info_indice_idx, info_to_data_idx)) = path_idx_to_info_indice.get(&info.file_path_index) {
                        info.file_info_indice_index = *info_indice_idx;
                        info.info_to_data_index = *info_to_data_idx;
                    } else {
                        let child_info_index = mass_load_group.file_info_start_index + offset as u32;
                        let current_hash = file_paths[usize::from(info.file_path_index)].path.hash40();
                        let original_folder_offset_idx = file_info_to_datas[usize::from(info.info_to_data_index)].folder_offset_index;
                        let idx = info.file_info_indice_index;
                        let idx = file_info_indices[usize::from(idx)].file_info_index;
                        let idx = file_infos[usize::from(idx)].file_path_index;
                        let other_hash = file_paths[usize::from(idx)].path.hash40();
                        if current_hash == other_hash || folder_offsets[original_folder_offset_idx as usize].resource_index == 0xFFFFFF { continue; }
                        println!("{}", crate::hashes::get(file_paths[info.file_path_index.0 as usize].path.hash40()).unwrap_or(&"Unknown"));
                        let idx = file_info_indices[usize::from(info.file_info_indice_index)].file_info_index;
                        let idx = file_infos[usize::from(idx)].info_to_data_index;
                        let idx = file_info_to_datas[usize::from(idx)].file_data_index;
                        let original_data = file_datas[usize::from(idx)];

                        let new_info_indice_index = FileInfoIndiceIdx(lengths.file_info_indices + new_info_indices.len() as u32);
                        let new_info_index_start = FileInfoIdx(lengths.file_infos + new_infos.len() as u32);
                        let new_info_to_data_index_start = InfoToDataIdx(lengths.file_info_to_datas + new_info_to_datas.len() as u32);
                        let new_data_index = FileDataIdx(lengths.file_datas + new_datas.len() as u32);

                        let mut new_info = info.clone();
                        new_info.file_info_indice_index = new_info_indice_index;
                        new_info.file_path_index = info.file_path_index;
                        new_info.info_to_data_index = new_info_to_data_index_start;
                        new_infos.push(new_info);

                        Self::duplicate_file_structure(
                            info.file_path_index,
                            new_info_indice_index,
                            FileInfoIdx(child_info_index),
                            new_data_index,
                            mass_load_group.dir_offset_index >> 8,
                            info,
                            &mut new_info_to_datas,
                            &mut new_infos,
                            &lengths
                        );

                        file_paths[usize::from(info.file_path_index)].path.set_index(new_info_indice_index.0);
                        info.file_info_indice_index = new_info_indice_index;
                        info.info_to_data_index = new_info_to_data_index_start;
                        new_datas.push(original_data);
                        let new_info_index = FileInfoIndex {
                            dir_offset_index: mass_load_group.dir_offset_index >> 8,
                            file_info_index: new_info_index_start
                        };
                        new_info_indices.push(new_info_index);
                    }
                }
            }
            
            arc.folder_offsets = Self::recreate_array(arc.folder_offsets, lengths.folder_offsets as usize, &new_mass_load_datas);
            arc.file_info_indices = Self::recreate_array(arc.file_info_indices, lengths.file_info_indices as usize, &new_info_indices);
            arc.file_infos = Self::recreate_array(arc.file_infos, lengths.file_infos as usize, &new_infos);
            arc.file_info_to_datas = Self::recreate_array(arc.file_info_to_datas, lengths.file_info_to_datas as usize, &new_info_to_datas);
            arc.file_datas = Self::recreate_array(arc.file_datas, lengths.file_datas as usize, &new_datas);
            instance.table1 = Self::extend_table(instance.table1, instance.table1_len as usize, new_info_indices.len());
            instance.table2 = Self::extend_table(instance.table2, instance.table2_len as usize, new_info_indices.len());
            instance.loaded_directory_table = Self::extend_table(instance.loaded_directory_table, instance.loaded_directory_table_size as usize, new_mass_load_datas.len());
            fs.folder_offset_count_1 += new_mass_load_datas.len() as u32;
            fs.file_info_index_count += new_info_indices.len() as u32;
            fs.file_info_sub_index_count += new_info_to_datas.len() as u32;
            fs.file_info_count += new_infos.len() as u32;
            fs.file_data_count += new_datas.len() as u32;
            instance.table1_len += new_info_indices.len() as u32;
            instance.table2_len += new_info_indices.len() as u32;
            instance.loaded_directory_table_size += new_mass_load_datas.len() as u32;
            // println!("{:#x} {:#x}", arc.get_file_paths()[usize::from(arc.get_file_path_index_from_hash(Hash40::from("fighter/ganon/model/sword/c01/model.numdlb")).unwrap())].path.index(), arc.get_file_paths()[usize::from(arc.get_file_path_index_from_hash(Hash40::from("fighter/ganon/model/sword/c00/model.numdlb")).unwrap())].path.index());
        }
        Ok(())
    }



    pub fn acquire_instance() -> TableGuard {
        TableGuard::new()
    }

    pub fn get_arc() -> &'static LoadedArc {
        LoadedTables::get_instance().loaded_data.arc
    }

    #[allow(dead_code)]
    pub fn get_search() -> &'static LoadedSearchSection {
        LoadedTables::get_instance().loaded_data.search
    }

    #[allow(dead_code)]
    pub fn get_arc_mut() -> &'static mut LoadedArc {
        &mut LoadedTables::get_instance().loaded_data.arc
    }

    #[allow(dead_code)]
    pub fn get_search_mut() -> &'static LoadedSearchSection {
        &mut LoadedTables::get_instance().loaded_data.search
    }

    pub fn get_instance() -> &'static mut Self {
        unsafe {
            let instance_ptr: *mut &'static mut Self =
                std::mem::transmute(offset_to_addr(LOADED_TABLES_OFFSET));
            *instance_ptr
        }
    }

    #[allow(dead_code)]
    pub fn get_loaded_directories(&self) -> &[LoadedDirectory] {
        unsafe {
            std::slice::from_raw_parts(
                self.loaded_directory_table,
                self.loaded_directory_table_size as usize,
            )
        }
    }

    pub fn table_1(&self) -> &[Table1Entry] {
        unsafe { std::slice::from_raw_parts(self.table1, self.table1_len as usize) }
    }

    pub fn table_1_mut(&mut self) -> &mut [Table1Entry] {
        unsafe { std::slice::from_raw_parts_mut(self.table1, self.table1_len as usize) }
    }

    pub fn table_2(&self) -> &[Table2Entry] {
        unsafe { std::slice::from_raw_parts(self.table2, self.table2_len as usize) }
    }

    pub fn table_2_mut(&mut self) -> &mut [Table2Entry] {
        unsafe { std::slice::from_raw_parts_mut(self.table2, self.table2_len as usize) }
    }

    #[allow(dead_code)]
    pub fn get_t1_mut(&mut self, t1_index: u32) -> Result<&mut Table1Entry, LoadError> {
        self.table_1_mut()
            .get_mut(t1_index as usize)
            .ok_or(LoadError::NoTable1)
    }

    #[allow(dead_code)]
    pub fn get_t2(&self, t1_index: FilePathIdx) -> Result<&Table2Entry, LoadError> {
        let t1 = self
            .table_1()
            .get(usize::from(t1_index))
            .ok_or(LoadError::NoTable1)?;
        let t2_index = t1.table2_index as usize;
        self.table_2().get(t2_index).ok_or(LoadError::NoTable2)
    }

    pub fn get_t2_mut(
        &mut self,
        t2_index: FileInfoIndiceIdx,
    ) -> Result<&mut Table2Entry, LoadError> {
        self.table_2_mut()
            .get_mut(usize::from(t2_index))
            .ok_or(LoadError::NoTable2)
    }
}

/// Set of functions to extend and patch the various tables at runtime
pub trait LoadedArcEx {
    /// Provides every FileInfo that refers to the FilePath
    fn get_shared_fileinfos(&self, file_path: &FilePath) -> Vec<FileInfo>;
    fn patch_filedata(&mut self, fileinfo: &FileInfo, size: u32) -> FileData;
    fn is_unshareable_group(&self, group_hash: Hash40) -> bool;
    fn get_mass_load_group_hash_from_file_hash(&self, file_hash: Hash40) -> Result<Hash40, LookupError>;
}

impl LoadedArcEx for LoadedArc {
    fn get_shared_fileinfos(&self, file_path: &FilePath) -> Vec<FileInfo> {
        self.get_file_infos()
            .iter()
            .filter_map(|entry| {
                if entry.file_info_indice_index == FileInfoIndiceIdx(file_path.path.index()) {
                    Some(*entry)
                } else {
                    None
                }
            })
            .collect()
    }

    fn patch_filedata(&mut self, fileinfo: &FileInfo, size: u32) -> FileData {
        let file_path = self.get_file_paths()[usize::from(fileinfo.file_path_index)];

        let region = if fileinfo.flags.is_regional() {
            smash_arc::Region::from(
                get_region_id(CONFIG.read().misc.region.as_ref().unwrap()).unwrap() + 1,
            )
        } else {
            smash_arc::Region::None
        };

        // To check if the file is shared
        let folder_offset = self.get_folder_offset(fileinfo, region);
        let orig_filedata = *self.get_file_data_mut(fileinfo, region);
        let offset = folder_offset
            + self.get_file_section_offset()
            + ((orig_filedata.offset_in_folder as u64) << 2);

        if self.get_shared_section_offset() < offset {
            // Get every FileInfo that shares the same FileInfoIndice index
            let shared_fileinfos = self.get_shared_fileinfos(&file_path);

            shared_fileinfos.iter().for_each(|info| {
                let mut filedata = self.get_file_data_mut(info, region);

                if filedata.decomp_size < size {
                    filedata.decomp_size = size;
                    info!(
                        "[ARC::Patching] File '{}' has a new patched decompressed size: {:#x}",
                        "temp",
                        filedata.decomp_size.bright_red()
                    );
                }
            });
        } else {
            let mut filedata = self.get_file_data_mut(fileinfo, region);

            if filedata.decomp_size < size {
                filedata.decomp_size = size;
                info!(
                    "[ARC::Patching] File '{}' has a new patched decompressed size: {:#x}",
                    "temp",
                    filedata.decomp_size.bright_red()
                );
            }
        }

        orig_filedata
    }

    fn is_unshareable_group(&self, group_hash: Hash40) -> bool {
        let group_info = match self.get_dir_info_from_hash(group_hash) {
            Ok(info) => info,
            _ => {
                return false;
            }
        };
        let folder_offsets = self.get_folder_offsets();
        let file_infos = self.get_file_infos();
        let file_paths = self.get_file_paths();
        let intermediate_idx = group_info.dir_offset_index >> 8;
        if intermediate_idx == 0xFFFFFF { return false; }
        let shared_idx = folder_offsets[intermediate_idx as usize].resource_index;
        if shared_idx == 0xFFFFFF { return false; }
        let shared_data = &folder_offsets[shared_idx as usize];
        // this can probably (?) be optimized, but basically we get the first info and check it's hash
        // against the hash of every file in the group. If we get one match, then we return false
        let test_info = file_infos[shared_data.file_info_start_index as usize];
        let test_path_hash = file_paths[usize::from(test_info.file_path_index)].path.hash40();
        let group_infos = file_infos.iter().skip(group_info.file_info_start_index as usize).take(group_info.file_info_count as usize);
        for info in group_infos {
            if file_paths[usize::from(info.file_path_index)].path.hash40() == test_path_hash { return false; }
        }
        true
    }

    fn get_mass_load_group_hash_from_file_hash(&self, file_hash: Hash40) -> Result<Hash40, LookupError> {
        let dir_infos = self.get_dir_infos();
        let file_infos = self.get_file_infos();
        let path_idx = self.get_file_path_index_from_hash(file_hash)?;
        for dir_info in dir_infos.iter() {
            let child_infos = file_infos.iter().skip(dir_info.file_info_start_index as usize).take(dir_info.file_info_count as usize);
            for child_info in child_infos {
                if child_info.file_path_index == path_idx {
                    return Ok(Hash40((dir_info.path_hash as u64) | (((dir_info.dir_offset_index & 0xFF) as u64) << 32)));
                }
            }
        }
        Err(LookupError::Missing)
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum LoadError {
    NoTable1,
    NoTable2,
}

#[repr(C)]
#[allow(dead_code)]
pub struct FileNX {
    vtable: *const (),
    unk1: *const (),
    unk2: u32,
    pub is_open: u32,
    pub file_handle: *mut nn::fs::FileHandle,
    unk3: u32,
    pub position: u64,
    pub filename_fixedstring: [u8; 516],
    unk4: u32,
}

#[repr(u32)]
#[allow(dead_code)]
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum LoadingType {
    Directory = 0,
    // Character/Stage directory related
    Unk1 = 1,
    // Character/Stage directory related
    Unk2 = 2,
    Unk3 = 3,
    File = 4,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ResServiceState {
    pub mutex: *mut nn::os::MutexType,
    pub res_update_event: *mut nn::os::EventType,
    unk1: *const (),
    pub io_swap_event: *mut nn::os::EventType,
    unk2: *const (),
    pub semaphore1: *const (),
    pub semaphore2: *const (),
    pub res_update_thread: *mut nn::os::ThreadType,
    pub res_loading_thread: *mut nn::os::ThreadType,
    pub res_inflate_thread: *mut nn::os::ThreadType,
    unk4: *const (),
    unk5: [CppVector<CppVector<u32>>; 4],
    unk6: *const (),
    unk7: *const (),
    unk8: *const (),
    pub loaded_tables: *mut LoadedTables,
    pub unk_region_idx: u32,
    pub game_region_idx: u32,
    pub unk9: u32,
    pub state: i16,
    pub is_loader_thread_running: bool,
    unk10: u8,
    pub data_arc_string: [u8; 256],
    unk11: *const (),
    pub data_arc_filenx: *mut *mut FileNX,
    pub buffer_size: usize,
    pub buffer_array: [*const u8; 2],
    pub buffer_array_idx: u32,
    unk12: u32,
    pub data_ptr: *const u8,
    pub offset_into_read: u64,
    pub processing_file_idx_curr: u32,
    pub processing_file_idx_count: u32,
    pub processing_file_idx_start: u32,
    pub processing_type: LoadingType,
    pub processing_dir_idx_start: u32,
    pub processing_dir_idx_single: u32,
    pub current_index: u32,
    pub current_dir_index: u32,
    //Still need to add some
}

impl ResServiceState {
    pub fn get_region_id() -> u32 {
        ResServiceState::get_instance().game_region_idx
    }

    pub fn get_instance() -> &'static mut Self {
        unsafe {
            let instance_ptr: *mut &'static mut Self =
                std::mem::transmute(offset_to_addr(RES_SERVICE_OFFSET));
            *instance_ptr
        }
    }
}
