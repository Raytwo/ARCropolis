use std::fmt;
use std::sync::atomic::AtomicU32;

use skyline::{
    nn,
    hooks::{
        Region,
        getRegionAddress,
    },
};

use smash_arc::LoadedArc;
use smash_arc::LoadedSearchSection;
use smash_arc::HashToIndex;

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
    pub loaded_directory_table: *const (),
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
pub struct LoadedData {
    pub arc: &'static mut LoadedArc,
    pub search: &'static mut LoadedSearchSection,
}

#[repr(C)]
pub struct CppVector<T> {
    start: *const T,
    end: *const T,
    eos: *const T,
}

#[repr(C)]
pub struct FsSearchBody {
    pub file_path_length: u32,
    pub idx_length: u32,
    pub path_group_length: u32,
}

impl LoadedTables {
    pub fn get_arc(&self) -> &LoadedArc {
        &self.loaded_data.arc
    }

    #[allow(dead_code)]
    pub fn get_search(&self) -> &LoadedSearchSection {
        self.loaded_data.search
    }

    #[allow(dead_code)]
    pub fn get_arc_mut(&mut self) -> &mut LoadedArc {
        &mut self.loaded_data.arc
    }

    #[allow(dead_code)]
    pub fn get_search_mut(&mut self) -> &LoadedSearchSection {
        &mut self.loaded_data.search
    }

    pub fn get_instance() -> &'static mut Self {
        unsafe {
            let instance_ptr: *mut &'static mut Self =
                std::mem::transmute(offset_to_addr(LOADED_TABLES_OFFSET));
            *instance_ptr
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

    pub fn get_t1_mut(&mut self, t1_index: u32) -> Result<&mut Table1Entry, LoadError> {
        self.table_1_mut()
            .get_mut(t1_index as usize)
            .ok_or(LoadError::NoTable1)
    }

    pub fn get_t2(&self, t1_index: u32) -> Result<&Table2Entry, LoadError> {
        let t1 = self
            .table_1()
            .get(t1_index as usize)
            .ok_or(LoadError::NoTable1)?;
        let t2_index = t1.table2_index as usize;
        self.table_2().get(t2_index).ok_or(LoadError::NoTable2)
    }

    pub fn get_t2_mut(&mut self, t1_index: u32) -> Result<&mut Table2Entry, LoadError> {
        let t1 = self
            .table_1()
            .get(t1_index as usize)
            .ok_or(LoadError::NoTable1)?;
        let t2_index = t1.table2_index as usize;
        self.table_2_mut()
            .get_mut(t2_index)
            .ok_or(LoadError::NoTable2)
    }
}

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
    pub regular_region_idx: u32,
    unk9: u32,
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
    pub fn get_instance() -> &'static mut Self {
        unsafe {
            let instance_ptr: *mut &'static mut Self =
                std::mem::transmute(offset_to_addr(RES_SERVICE_OFFSET));
            *instance_ptr
        }
    }
}
