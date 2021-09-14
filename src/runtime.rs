use skyline::{
    hooks::{getRegionAddress, Region},
    nn,
};
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};

use smash_arc::{ArcLookup, FileInfo, FileInfoIndiceIdx, FilePath, FilePathIdx, LoadedArc};

use smash_arc::LoadedSearchSection;

use crate::{config::REGION, hashes, res_list::ResList};

use crate::cpp_vector::CppVector;

use log::info;
use owo_colors::OwoColorize;

// 9.0.1 offsets
pub static mut LOADED_TABLES_OFFSET: usize = 0x505_67a0;
pub static mut RES_SERVICE_OFFSET: usize = 0x505_67a8;

pub fn offset_to_addr(offset: usize) -> *const () {
    unsafe { (getRegionAddress(Region::Text) as *const u8).add(offset) as _ }
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
#[derive(Debug, Copy, Clone)]
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
        write!(
            f,
            "Table2 index: {} (In Table2: {})",
            self.table2_index,
            self.in_table_2 != 0
        )
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

impl Clone for Table2Entry {
    fn clone(&self) -> Self {
        Table2Entry {
            data: self.data,
            ref_count: AtomicU32::new(self.ref_count.load(Ordering::SeqCst)),
            is_used: self.is_used,
            state: self.state,
            file_flags2: self.file_flags2,
            flags: self.flags,
            version: self.version,
            unk: self.unk
        }
    }
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
    pub ref_count: AtomicU32,
    pub flags: u8,
    pub state: FileState,
    pub incoming_request_count: AtomicU32,
    pub child_path_indices: CppVector<u32>,
    pub child_folders: CppVector<*mut LoadedDirectory>,
    pub redirection_dir: *mut LoadedDirectory,
}
// to satisfy ArcVector
impl Clone for LoadedDirectory {
    fn clone(&self) -> Self {
        LoadedDirectory {
            directory_offset_index: self.directory_offset_index,
            ref_count: AtomicU32::new(self.ref_count.load(Ordering::SeqCst)),
            flags: self.flags,
            state: self.state,
            incoming_request_count: AtomicU32::new(self.incoming_request_count.load(Ordering::SeqCst)),
            child_path_indices: self.child_path_indices,
            child_folders: self.child_folders,
            redirection_dir: self.redirection_dir
        }
    }
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
    tables: &'static mut LoadedTables,
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
        Self { tables: instance }
    }
}

impl Drop for TableGuard {
    fn drop(&mut self) {
        self.tables.unlock();
    }
}

#[derive(Debug)]
pub struct ArrayLengths {
    pub dir_infos: u32,
    pub file_paths: u32,
    pub file_info_indices: u32,
    pub file_infos: u32,
    pub file_info_to_datas: u32,
    pub file_datas: u32,
    pub folder_offsets: u32,
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
            file_info_to_datas: fs.file_info_sub_index_count
                + fs.file_data_count_2
                + fs.extra_count_2,
            file_datas: fs.file_data_count + fs.file_data_count_2 + fs.extra_count,
            folder_offsets: fs.folder_offset_count_1 + fs.folder_offset_count_2, // + fs.extra_folder
        }
    }
}

pub unsafe fn as_mutable_slice<'a, T>(pointer: *const T, length: u32) -> &'a mut [T] {
    std::slice::from_raw_parts_mut(pointer as *mut T, length as usize)
}

impl LoadedTables {
    #[inline]
    pub fn lock(&mut self) {
        unsafe {
            nn::os::LockMutex(self.mutex);
        }
    }

    #[inline]
    pub fn unlock(&mut self) {
        unsafe {
            nn::os::UnlockMutex(self.mutex);
        }
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
            let instance_ptr = offset_to_addr(LOADED_TABLES_OFFSET) as *mut &'static mut Self;
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
    fn patch_filedata(&mut self, fileinfo: &FileInfo, size: u32) -> u32;
    // fn is_unshareable_group(&self, group_hash: Hash40) -> bool;
    // fn get_mass_load_group_hash_from_file_hash(
    //     &self,
    //     file_hash: Hash40,
    // ) -> Result<Hash40, LookupError>;
    // fn get_unshared_connections(
    //     &self,
    //     mass_load_infos: &[FileInfo],
    //     shared_load_infos: &[FileInfo],
    // ) -> Option<HashMap<Hash40, (Hash40, usize)>>;
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

    fn patch_filedata(&mut self, fileinfo: &FileInfo, size: u32) -> u32 {
        let file_path = self.get_file_paths()[usize::from(fileinfo.file_path_index)];

        let region = if fileinfo.flags.is_regional() {
            *REGION
        } else {
            smash_arc::Region::None
        };

        // To check if the file is shared
        let folder_offset = self.get_folder_offset(fileinfo, region);
        let orig_filedata = *self.get_file_data_mut(fileinfo, region);
        let offset = folder_offset
            + self.get_file_section_offset()
            + ((orig_filedata.offset_in_folder as u64) << 2);

        let name = hashes::get(self.get_file_paths()[fileinfo.file_path_index].path.hash40());

        if self.get_shared_section_offset() < offset  && false {
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
                    name,
                    filedata.decomp_size.bright_red()
                );
            }
        }

        orig_filedata.decomp_size
    }

    // fn is_unshareable_group(&self, group_hash: Hash40) -> bool {
    //     let group_info = match self.get_dir_info_from_hash(group_hash) {
    //         Ok(info) => info,
    //         _ => {
    //             return false;
    //         }
    //     };
    //     let folder_offsets = self.get_folder_offsets();
    //     let file_infos = self.get_file_infos();
    //     let file_paths = self.get_file_paths();
    //     let intermediate_idx = group_info.path.index();
    //     if intermediate_idx == 0xFF_FFFF {
    //         return false;
    //     }
    //     let shared_idx = folder_offsets[intermediate_idx as usize].directory_index;
    //     if shared_idx == 0xFF_FFFF {
    //         return false;
    //     }
    //     let shared_data = &folder_offsets[shared_idx as usize];
    //     // this can probably (?) be optimized, but basically we get the first info and check it's hash
    //     // against the hash of every file in the group. If we get one match, then we return false
    //     let test_info = file_infos[shared_data.file_info_start_index as usize];
    //     let test_path_hash = file_paths[usize::from(test_info.file_path_index)]
    //         .path
    //         .hash40();
    //     let group_infos = file_infos
    //         .iter()
    //         .skip(group_info.file_info_start_index as usize)
    //         .take(group_info.file_info_count as usize);
    //     for info in group_infos {
    //         if file_paths[usize::from(info.file_path_index)].path.hash40() == test_path_hash {
    //             return false;
    //         }
    //     }
    //     true
    // }

    // fn get_mass_load_group_hash_from_file_hash(
    //     &self,
    //     file_hash: Hash40,
    // ) -> Result<Hash40, LookupError> {
    //     let dir_infos = self.get_dir_infos();
    //     let file_infos = self.get_file_infos();
    //     let path_idx = self.get_file_path_index_from_hash(file_hash)?;

    //     for dir_info in dir_infos.iter() {
    //         let child_infos = file_infos
    //             .iter()
    //             .skip(dir_info.file_start_index as usize)
    //             .take(dir_info.file_count as usize);
    //         for child_info in child_infos {
    //             if child_info.file_path_index == path_idx {
    //                 return Ok(dir_info.path.hash40());
    //             }
    //         }
    //     }

    //     Err(LookupError::Missing)
    // }

    // // Should probably return a Result because of the potential error in unsharing a source slot
    // fn get_unshared_connections(
    //     &self,
    //     mass_load_infos: &[FileInfo],
    //     shared_load_infos: &[FileInfo],
    // ) -> Option<HashMap<Hash40, (Hash40, usize)>> {
    //     let file_paths = self.get_file_paths();
    //     let file_info_indices = self.get_file_info_indices();
    //     let file_infos = self.get_file_infos();

    //     let mut path_idx_to_data: HashMap<FilePathIdx, Hash40> = shared_load_infos
    //         .iter()
    //         .map(|info| {
    //             let hash = file_paths[info.file_path_index].path.hash40();
    //             let info_idx = file_info_indices[info.file_info_indice_index].file_info_index;
    //             let path_idx = file_infos[info_idx].file_path_index;

    //             (path_idx, hash)
    //         })
    //         .collect();

    //     let connections: HashMap<Hash40, (Hash40, usize)> = mass_load_infos
    //         .iter()
    //         .enumerate()
    //         .filter_map(|(idx, info)| {
    //             let info_idx = file_info_indices[info.file_info_indice_index].file_info_index;
    //             let path_idx = file_infos[info_idx].file_path_index;

    //             if let Some(data_hash) = path_idx_to_data.get(&path_idx) {
    //                 let group_hash = file_paths[info.file_path_index].path.hash40();

    //                 if group_hash == *data_hash {
    //                     None // can't unshare a source slot
    //                 } else {
    //                     Some((*data_hash, (group_hash, idx)))
    //                 }
    //             } else {
    //                 None
    //             }
    //         })
    //         .collect();

    //     Some(connections)
    // }
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
    pub res_lists: [ResList; 5],
    pub loaded_tables: *mut LoadedTables,
    pub region_idx: u32,
    pub language_idx: u32,
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
    #[allow(dead_code)]
    pub fn get_language() -> u32 {
        ResServiceState::get_instance().language_idx + 1
    }

    pub fn get_instance() -> &'static mut Self {
        unsafe { *(offset_to_addr(RES_SERVICE_OFFSET) as *mut &'static mut Self) }
    }
}
