use std::fmt;
use std::sync::atomic::AtomicU32;

use skyline::hooks::{getRegionAddress, Region};
use skyline::nn;

pub static mut LOADED_TABLES_OFFSET: usize = 0x4ed7200; // 8.0.0 offset

pub fn offset_to_addr(offset: usize) -> *const () {
    unsafe { (getRegionAddress(Region::Text) as *const u8).offset(offset as isize) as _ }
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
    arc: &'static mut LoadedArc,
    search: &'static mut LoadedSearch,
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
    pub file_info_idx: *const FileInfoIndex,
    pub dir_hash_group: *const (),
    pub dir_list: *const (),
    pub dir_offset: *const (),
    pub dir_child_hash_group: *const (),
    // FileInfoV2
    pub file_info: *const FileInfo,
    pub file_info_sub_index: *const FileInfoSubIndex,
    pub sub_files: *const SubFile,
}

#[repr(C)]
pub struct CppVector<T> {
    start: *const T,
    end: *const T,
    eos: *const T,
}

#[repr(C)]
pub struct LoadedSearch {
    pub header: *const (),
    pub body: *const FsSearchBody,
    pub file_path_to_idx: *const HashIndexGroup,
    pub idx_to_group: *const HashGroup,
    pub path_to_idx: *const HashIndexGroup,
    pub idx_to_path_group_idx: *const (),
    pub path_group: *const HashGroup,
}

#[repr(C)]
pub struct FsSearchBody {
    pub file_path_length: u32,
    pub idx_length: u32,
    pub path_group_length: u32,
}

impl LoadedArc {
    pub fn get_subfile_by_t1_index(&self, t1_index: u32) -> &mut SubFile {
        let mut file_info = self.lookup_file_information_by_t1_index(t1_index);
        let file_index = self.lookup_fileinfoindex_by_t1_index(t1_index);

        // Redirect
        if (file_info.flags & 0x00000010) == 0x10 {
            // Try to change its shared status, infinite loadings so far
            // file_info.flags ^= 0x10;

            // let loaded_tables = LoadedTables::get_instance();
            // unsafe { nn::os::LockMutex(loaded_tables.mutex); }

            // let count: [u8; 4] = unsafe { transmute((loaded_tables.table2_len -1).to_le()) };
            // self.lookup_fileinfopath_by_t1_index(t1_index).path.index.0[0] = count[0];
            // self.lookup_fileinfopath_by_t1_index(t1_index).path.index.0[1] = count[1];
            // self.lookup_fileinfopath_by_t1_index(t1_index).path.index.0[2] = count[2];
            // loaded_tables.get_t1_mut(t1_index).unwrap().table2_index = loaded_tables.table2_len - 1;

            // unsafe { nn::os::UnlockMutex(loaded_tables.mutex); }

            // println!("Table2_len: {:#x}", loaded_tables.table2_len);
            // println!("Flags after: {:#x}, PathIndex: {:#x}", file_info.flags, self.lookup_fileinfopath_by_t1_index(t1_index).path.index.as_u32());

            //let file_index = self.lookup_fileinfoindex_by_t1_index(t1_index);
            file_info = self.lookup_file_information_by_t1_index(file_index.file_info_index);
        }

        let mut sub_index = self.lookup_fileinfosubindex_by_index(file_info.sub_index_index);

        // Regional
        if (file_info.flags & 0x00008000) == 0x8000 {
            sub_index = self.lookup_fileinfosubindex_by_index(file_info.sub_index_index + 2);
        }

        let sub_file =
            unsafe { self.sub_files.offset(sub_index.sub_file_index as isize) as *mut SubFile };

        unsafe { &mut *sub_file }
    }

    pub fn lookup_fileinfopath_by_t1_index(&self, t1_index: u32) -> &mut FileInfoPath {
        let file_info_path_table = self.file_info_path;
        let file_info =
            unsafe { file_info_path_table.offset(t1_index as isize) as *mut FileInfoPath };
        unsafe { &mut *file_info }
    }

    pub fn lookup_fileinfoindex_by_t1_index(&self, t1_index: u32) -> &mut FileInfoIndex {
        let file_info_path = self.lookup_fileinfopath_by_t1_index(t1_index);
        let file_info_idx = unsafe {
            self.file_info_idx
                .offset(file_info_path.path.index.as_u32() as isize)
                as *mut FileInfoIndex
        };
        unsafe { &mut *file_info_idx }
    }

    pub fn lookup_file_information_by_t1_index(&self, t1_index: u32) -> &mut FileInfo {
        let file_info_idx = self.lookup_fileinfoindex_by_t1_index(t1_index);
        let file_info_table = self.file_info as *mut FileInfo;
        let file_info =
            unsafe { file_info_table.offset((*file_info_idx).file_info_index as isize) };
        unsafe { &mut (*file_info) }
    }

    pub fn lookup_fileinfosubindex_by_index(&self, sub_index_index: u32) -> &mut FileInfoSubIndex {
        let file_info_sub_index_table = self.file_info_sub_index as *mut FileInfoSubIndex;
        let file_info_sub_index =
            unsafe { &mut *file_info_sub_index_table.offset(sub_index_index as isize) };
        file_info_sub_index
    }
}

#[repr(C)]
pub struct FileInfo {
    pub path_index: u32,
    pub index_index: u32,
    pub sub_index_index: u32,
    pub flags: u32,
}

#[repr(C)]
pub struct SubFile {
    pub offset: u32,
    pub compressed_size: u32,
    pub decompressed_size: u32,
    pub flags: u32,
}

#[repr(C)]
pub struct FileInfoPath {
    pub path: HashIndexGroup,
    pub extension: HashIndexGroup,
    pub parent: HashIndexGroup,
    pub file_name: HashIndexGroup,
}

#[repr(packed)]
pub struct FileInfoIndex {
    pub dir_offset_index: u32,
    pub file_info_index: u32,
}

#[repr(packed)]
pub struct FileInfoSubIndex {
    pub folder_offset_index: u32,
    pub sub_file_index: u32,
    pub file_info_index_and_flag: u32,
}

#[repr(packed)]
#[derive(Copy, Clone)]
pub struct HashIndexGroup {
    pub hash40: Hash40,
    pub index: U24,
}

#[repr(C)]
pub struct HashGroup {
    pub path: HashIndexGroup,
    pub parent: HashIndexGroup,
    pub file_name: HashIndexGroup,
    pub extension: HashIndexGroup,
}

impl fmt::Debug for HashIndexGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:x}", self.hash40.as_u64())
    }
}

#[repr(packed)]
#[derive(Copy, Clone)]
pub struct Hash40 {
    pub crc32: u32,
    pub len: u8,
}

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct U24(pub [u8; 3]);

impl U24 {
    pub fn as_u32(&self) -> u32 {
        u32::from_le_bytes([self.0[0], self.0[1], self.0[2], 0])
    }
}

impl fmt::Debug for U24 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_u32())
    }
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

    #[allow(dead_code)]
    pub fn get_search(&self) -> &LoadedSearch {
        self.loaded_data.search
    }

    #[allow(dead_code)]
    pub fn get_arc_mut(&mut self) -> &mut LoadedArc {
        &mut self.loaded_data.arc
    }

    #[allow(dead_code)]
    pub fn get_search_mut(&mut self) -> &LoadedSearch {
        &mut self.loaded_data.search
    }

    pub fn get_instance() -> &'static mut Self {
        unsafe {
            let instance_ptr: *mut &'static mut Self =
                std::mem::transmute(offset_to_addr(LOADED_TABLES_OFFSET));
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
    pub data_arc_filenx: *mut FileNX,
    pub buffer_size: usize,
    pub buffer_array: [*const skyline::libc::c_void; 2],
    pub buffer_array_idx: u32,
    unk12: *const (),
    pub data_ptr: *const skyline::libc::c_void,
    pub offset_into_read: u64,
    //Still need to add some
}
