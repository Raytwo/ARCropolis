use skyline::nn;
use skyline::hooks::{getRegionAddress, Region};
use std::fmt;
use std::sync::atomic::AtomicU32;

fn offset_to_addr(offset: usize) -> *const () {
    unsafe { (getRegionAddress(Region::Text) as *const u8).offset(offset as isize) as _ }
}

#[repr(C)]
#[repr(packed)]
pub struct Table1Entry {
    pub table2_index: u32,
    pub is_in_table_2: u32,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
#[allow(dead_code)]
pub enum FileState {
    Unused = 0,
    Unloaded = 1,
    Unk2 = 2,
    Loaded = 3,
}

impl fmt::Display for FileState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
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

#[repr(C)]
pub struct CppVector<T> {
    start: *const T,
    end: *const T,
    eos: *const T,
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
    arc: &'static mut LoadedArc,
}

#[repr(C)]
pub struct LoadedArc {
    pub magic: u64,
    pub music_data_offset: u64,
    pub file_data_offset: u64,
    pub file_data_offset_2: u64,
    pub fs_offset: u64,
    pub fs_search_offset: u64,
    pub unk_offset: u64,
    pub loaded_fs: *const (),
    pub loaded_fs_2: *const (),
    pub region_entry: *const (),
    pub file_path_buckets: *const (),
    pub file_path_to_index_hash_group: *const (),
    pub file_info_path: *const FileInfoPath,
    pub file_info_idx: *const (),
    pub dir_hash_group: *const (),
    pub dir_list: *const (),
    pub dir_offset: *const (),
    pub dir_child_hash_group: *const (),
    pub file_info: *const FileInfo,
}

#[repr(C)]
pub struct FileInfo {
    pub path_index: u32,
    pub index_index: u32,
    pub sub_index_index: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct FileInfoPath {
    pub path: HashIndexGroup,
    pub extension: HashIndexGroup,
    pub parent: HashIndexGroup,
    pub file_name: HashIndexGroup,
}

#[repr(packed)]
#[derive(Copy, Clone)]
pub struct HashIndexGroup {
    pub hash40: Hash40,
    pub flags: [u8; 3],
}

impl fmt::Debug for HashIndexGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:x}", self.hash40.as_u64())
    }
}

#[repr(packed)]
#[derive(Copy, Clone)]
pub struct Hash40 {
    crc32: u32,
    len: u8,
}

impl fmt::Debug for Hash40 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:x}", self.as_u64())
    }
}

impl Hash40 {
    pub fn as_u64(&self) -> u64 {
        (self.crc32 as u64) + ((self.len as u64) << 32)
    }
}

impl LoadedTables {
    pub fn get_arc(&self) -> &LoadedArc {
        self.loaded_data.arc
    }

    pub fn get_instance() -> &'static mut Self {
        unsafe {
            let instance_ptr: *mut &'static mut Self =
                std::mem::transmute(offset_to_addr(0x4ed7200));
            *instance_ptr
        }
    }

    pub fn get_hash_from_t1_index(&self, t1_index: u32) -> Hash40 {
        let arc = self.get_arc();
        let path_table = arc.file_info_path;
        let file_info = unsafe { &*path_table.offset(t1_index as isize) };
        file_info.path.hash40
    }

    pub fn table_1(&self) -> &[Table1Entry] {
        unsafe { std::slice::from_raw_parts(self.table1, self.table1_len as usize) }
    }

    pub fn table_2(&self) -> &[Table2Entry] {
        unsafe { std::slice::from_raw_parts(self.table2, self.table2_len as usize) }
    }

    pub fn table_2_mut(&mut self) -> &mut [Table2Entry] {
        unsafe { std::slice::from_raw_parts_mut(self.table2, self.table2_len as usize) }
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
