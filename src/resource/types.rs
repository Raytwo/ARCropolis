use skyline::nn;
use smash_arc::{LoadedArc, LoadedSearchSection, Region};
use std::{
    ops::{Index, IndexMut},
    sync::atomic::AtomicU32,
};

use super::containers::{CppVector, ResList};

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum LoadState {
    Unused = 0,
    Unloaded = 1,
    Unknown = 2,
    Loaded = 3,
}

#[repr(packed)]
#[derive(Copy, Clone, Debug)]
pub struct LoadedFilepath {
    pub loaded_data_index: u32,
    pub is_loaded: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct LoadedData {
    pub data: *const u8,
    pub ref_count: AtomicU32,
    pub is_used: bool,
    pub state: LoadState,
    pub file_flags2: bool,
    pub flags: u8,
    pub version: u32,
    pub unk: u8,
}

#[derive(Debug)]
#[repr(C)]
pub struct LoadedDirectory {
    pub file_group_index: u32,
    pub ref_count: AtomicU32,
    pub flags: u8,
    pub state: LoadState,
    pub incoming_request_count: AtomicU32, // note, could be wrong
    pub child_path_indices: CppVector<u32>,
    pub child_folders: CppVector<*mut LoadedDirectory>,
    pub redirection_directory: *mut LoadedDirectory,
}

#[repr(C)]
pub struct PathInformation {
    pub arc: &'static mut LoadedArc,
    pub search: &'static mut LoadedSearchSection,
}

#[repr(C)]
pub struct FilesystemInfo {
    pub mutex: *mut nn::os::MutexType,
    pub loaded_filepaths: *mut LoadedFilepath,
    pub loaded_datas: *mut LoadedData,
    pub loaded_filepath_len: u32,
    pub loaded_data_len: u32,
    pub loaded_filepath_count: u32,
    pub loaded_data_count: u32,
    pub loaded_filepath_list: CppVector<u32>,
    pub loaded_directories: *const LoadedDirectory,
    pub loaded_directory_len: u32,
    pub unk: u32,
    pub unk2: CppVector<u32>,
    pub unk3: u8,
    pub unk4: [u8; 7],
    pub addr: *const (),
    pub path_info: &'static mut PathInformation,
    pub version: u32,
}

impl FilesystemInfo {
    pub fn get_loaded_filepaths(&self) -> &[LoadedFilepath] {
        unsafe {
            std::slice::from_raw_parts(self.loaded_filepaths, self.loaded_filepath_len as usize)
        }
    }

    pub fn get_loaded_filepaths_mut(&mut self) -> &mut [LoadedFilepath] {
        unsafe {
            std::slice::from_raw_parts_mut(self.loaded_filepaths, self.loaded_filepath_len as usize)
        }
    }

    pub fn get_loaded_datas(&self) -> &[LoadedData] {
        unsafe {
            std::slice::from_raw_parts(self.loaded_datas, self.loaded_data_len as usize)
        }
    }

    pub fn get_loaded_datas_mut(&mut self) -> &mut [LoadedData] {
        unsafe {
            std::slice::from_raw_parts_mut(self.loaded_datas, self.loaded_data_len as usize)
        }
    }

    pub fn get_loaded_directories(&self) -> &[LoadedDirectory] {
        unsafe {
            std::slice::from_raw_parts(self.loaded_directories, self.loaded_directory_len as usize)
        }
    }
}

#[repr(C)]
pub struct FileNX {
    vtable: *const (),
    unk1: *const (),
    unk2: u32,
    pub is_open: u32,
    pub file_handle: *mut nn::fs::FileHandle,
    pub unk3: u32,
    pub position: u64,
    pub filename_fixedstring: [u8; 516],
    unk4: u32,
}

#[allow(dead_code)]
#[repr(u32)]
pub enum LoadingType {
    Directory = 0,
    Unk1 = 1,
    Unk2 = 2,
    Unk3 = 3,
    File = 4,
}

#[allow(dead_code)]
#[repr(C)]
pub struct ResServiceNX {
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
    unk3: *const (),
    pub res_lists: [ResList; 5],
    pub filesystem_info: *mut FilesystemInfo,
    pub region_idx: u32,
    pub language_idx: Region,
    unk4: u32,
    pub state: i16,
    pub is_loader_thread_running: bool,
    unk5: u8,
    pub data_arc_string: [u8; 256],
    unk6: *const (),
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

#[repr(C)]
pub struct InflateFile {
    pub content: *mut u8,
    pub size: usize
}

impl InflateFile {
    pub fn len(&self) -> usize {
        self.size
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self.content, self.size)
        }
    }
    
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(self.content, self.size)
        }
    }
}

impl Index<LoadedFilepath> for [LoadedData] {
    type Output = LoadedData;

    fn index(&self, index: LoadedFilepath) -> &Self::Output {
        &self[index.loaded_data_index as usize]
    }
}

impl Index<&LoadedFilepath> for [LoadedData] {
    type Output = LoadedData;

    fn index(&self, index: &LoadedFilepath) -> &Self::Output {
        &self[index.loaded_data_index as usize]
    }
}

impl IndexMut<LoadedFilepath> for [LoadedData] {
    fn index_mut(&mut self, index: LoadedFilepath) -> &mut Self::Output {
        &mut self[index.loaded_data_index as usize]
    }
}

impl IndexMut<&LoadedFilepath> for [LoadedData] {
    fn index_mut(&mut self, index: &LoadedFilepath) -> &mut Self::Output {
        &mut self[index.loaded_data_index as usize]
    }
}
