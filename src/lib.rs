#![feature(proc_macro_hygiene)]
#![feature(str_strip)]
#![feature(asm)]

use std::io::Write;
use std::ffi::CStr;
use std::path::Path;
use std::net::IpAddr;
use std::sync::atomic::Ordering;

use skyline::hooks::InlineCtx;
use skyline::{nn, hook, install_hooks};

mod config;
use config::CONFIG;
mod hashes;
mod stream;

mod replacement_files;
use replacement_files::{ FileCtx, ARC_FILES, ARC_CALLBACKS, QUEUE_HANDLED, CB_QUEUE };

mod offsets;
use offsets::{ ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET, IDK_OFFSET, PARSE_EFF_NUTEXB_OFFSET, PARSE_EFF_OFFSET, PARSE_PARAM_OFFSET, PARSE_MODEL_XMB_OFFSET, PARSE_ARC_FILE_OFFSET, PARSE_FONT_FILE_OFFSET, PARSE_NUMSHB_FILE_OFFSET,PARSE_NUMATB_NUTEXB_OFFSET, PARSE_NUMSHEXB_FILE_OFFSET, PARSE_NUMATB_FILE_OFFSET, PARSE_NUMDLB_FILE_OFFSET, PARSE_LOG_XMB_OFFSET, PARSE_MODEL_XMB_2_OFFSET, TITLE_SCREEN_VERSION_OFFSET, PARSE_NUS3BANK_FILE_OFFSET };

use owo_colors::OwoColorize;

use smash::resource::{FileState, LoadedTables, ResServiceState, Table2Entry, HashIndexGroup, CppVector, FileNX};

use log::{ trace, info };
mod logging;

#[hook(offset = IDK_OFFSET)]
unsafe fn idk(res_state: *const ResServiceState, table1_idx: u32, flag_related: u32) {
    handle_file_load(table1_idx);
    original!()(res_state, table1_idx, flag_related);
}

#[hook(offset = ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET)]
unsafe fn add_idx_to_table1_and_table2(loaded_table: *const LoadedTables, table1_idx: u32) {
    handle_file_load(table1_idx);
    original!()(loaded_table, table1_idx);
}

#[hook(offset = PARSE_EFF_OFFSET, inline)]
fn parse_eff(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[10].w.as_ref());
    }
}

#[hook(offset = PARSE_PARAM_OFFSET, inline)]
fn parse_param_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*((*ctx.registers[20].x.as_ref()) as *const u32));
    }
}

#[hook(offset = PARSE_MODEL_XMB_OFFSET, inline)]
fn parse_model_xmb(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[22].w.as_ref());
    }
}

#[hook(offset = PARSE_MODEL_XMB_2_OFFSET, inline)]
fn parse_model_xmb2(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[22].w.as_ref());
    }
}

#[hook(offset = PARSE_LOG_XMB_OFFSET, inline)]
fn parse_log_xmb(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[19].w.as_ref());
    }
}

#[hook(offset = PARSE_ARC_FILE_OFFSET, inline)]
fn parse_arc_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[8].w.as_ref());
    }
}

#[hook(offset = PARSE_FONT_FILE_OFFSET, inline)]
fn parse_font_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*((*ctx.registers[19].x.as_ref()) as *const u32));
    }
}

#[hook(offset = PARSE_NUMDLB_FILE_OFFSET, inline)]
fn parse_numdlb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[1].w.as_ref());
    }
}

#[hook(offset = PARSE_NUMSHEXB_FILE_OFFSET, inline)]
fn parse_numshexb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[25].w.as_ref());
    }
}

#[hook(offset = PARSE_NUMATB_FILE_OFFSET, inline)]
fn parse_numatb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[23].w.as_ref());
    }
}

#[hook(offset = PARSE_EFF_NUTEXB_OFFSET, inline)]
fn parse_eff_nutexb(ctx: &InlineCtx) {
    unsafe {
        handle_texture_files(*ctx.registers[24].w.as_ref());
    }
}

#[hook(offset = PARSE_NUMATB_NUTEXB_OFFSET, inline)]
fn parse_numatb_nutexb(ctx: &InlineCtx) {
    unsafe {
        handle_texture_files(*ctx.registers[25].w.as_ref());
    }
}

#[hook(offset = PARSE_NUMSHB_FILE_OFFSET, inline)]
fn parse_numshb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[24].w.as_ref());
    }
}

#[hook(offset = PARSE_NUS3BANK_FILE_OFFSET, inline)]
fn parse_nus3bank_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[8].w.as_ref());
    }
}

#[hook(offset = 0x35ba800, inline)]
fn parse_bntx_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[9].w.as_ref());
    }
}

fn get_filectx_by_t1index<'a>(table1_idx: u32) -> Option<(parking_lot::MappedRwLockReadGuard<'a, FileCtx>, &'a mut Table2Entry)> {
    let loaded_tables = LoadedTables::get_instance();
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();

    let table2entry = match loaded_tables.get_t2_mut(table1_idx) {
        Ok(entry) => entry,
        Err(_) => {
            return None;
        }
    };

    trace!("[ARC::Loading | #{}] File: {}, Hash: {}, Status: {}", table1_idx.green(), hashes::get(hash).unwrap_or(&"Unknown").bright_yellow(), hash.cyan(), table2entry.bright_magenta());

    if QUEUE_HANDLED.swap(true, Ordering::SeqCst) {
        for (hash, ctx) in CB_QUEUE.write().iter_mut() {
            let found = match ARC_FILES.write().0.get_mut(&*hash) {
                Some(context) => {
                    if context.filesize < ctx.filesize {
                        context.filesize = ctx.filesize;
                        ctx.filesize_replacement();
                    }
                    true
                },
                None => false,
            };

            if !found {
                ctx.filesize_replacement();
                ARC_FILES.write().0.insert(*hash, ctx.clone());
            }
        }

        CB_QUEUE.write().clear();
    }

    match get_from_hash!(hash) {
        Ok(file_ctx) => {
            info!("[ARC::Loading | #{}] Hash matching for file: '{}'", table1_idx.green(), file_ctx.path.display().bright_yellow());
            Some((file_ctx, table2entry))
        }
        Err(_) => None,
    }
}

fn handle_file_load(table1_idx: u32) {
    // Println!() calls are on purpose so these show up no matter what.
    if let Some((file_ctx, table2entry)) = get_filectx_by_t1index(table1_idx) {
        if table2entry.state != FileState::Unloaded {
            return;
        }

        // Some formats don't appreciate me replacing the data pointer
        // if !is_file_allowed(&file_ctx.path) {
        //     return;
        // }

            // For files that are too dependent on timing, make sure the pointer is overwritten instead of swapped
            match file_ctx.path.extension().unwrap().to_str().unwrap() {
                // "bntx" | "nusktb" | "bin" | "numdlb" => {
                //     handle_file_overwrite(table1_idx);
                //     return;
                // }
                "nutexb" => {
                    handle_texture_files(table1_idx);
                    return;
                }
                &_ => {}
            }

        info!("[ARC::Replace | #{}] Replacing '{}'", table1_idx.green(), hashes::get(file_ctx.hash).unwrap_or(&"Unknown").bright_yellow());

        let hash = file_ctx.hash;

        let orig_size = file_ctx.filesize as usize;

        let file = vec![0;orig_size];
        let mut file_slice = file.into_boxed_slice();

        let cb_result = match ARC_CALLBACKS.read().get(&hash) {
            Some(cb) => {
                cb(hash, file_slice.as_mut_ptr() as *mut skyline::libc::c_void, orig_size)
            },
            None => false,
        };

        // Callback returned false or there are no callback for this file
        if !cb_result {
            // If it is a valid file_ctx
            if !file_ctx.virtual_file {
                // Load the file on the SD
                file_slice = file_ctx.get_file_content().into_boxed_slice();
            } else {
                // The file does not actually exist on the SD, so we abort here
                return;
            }
        }

        let data = Box::leak(file_slice);

        unsafe {
            if !table2entry.data.is_null() {
                skyline::libc::free(table2entry.data as *const skyline::libc::c_void);
            }
        }

        table2entry.data = data.as_ptr();
        table2entry.state = FileState::Loaded;
        table2entry.flags = 43;
    }
}

fn handle_file_overwrite(table1_idx: u32) {
    if let Some((file_ctx, table2entry)) = get_filectx_by_t1index(table1_idx) {
        if table2entry.state != FileState::Loaded {
            return;
        }

        let hash = file_ctx.hash;

        let orig_size = file_ctx.filesize as usize;

        let file = vec![0;orig_size];
        let mut file_slice = file.into_boxed_slice();

        let cb_result = match ARC_CALLBACKS.read().get(&hash) {
            Some(cb) => {
                cb(hash, file_slice.as_mut_ptr() as *mut skyline::libc::c_void, orig_size)
            },
            None => false,
        };

        if !cb_result {
            if !file_ctx.virtual_file {
                file_slice = file_ctx.get_file_content().into_boxed_slice();
            } else {
                // The file does not actually exist on the SD, so we abort here
                return;
            }
        }

        info!("[ARC::Replace | #{}] Replacing '{}'", table1_idx.green(), hashes::get(file_ctx.hash).unwrap_or(&"Unknown").bright_yellow());

        unsafe {
            let mut data_slice = std::slice::from_raw_parts_mut(table2entry.data as *mut u8, file_slice.len());
            data_slice.write(&file_slice).unwrap();
        }
    }
}

fn handle_texture_files(table1_idx: u32) {
    if let Some((file_ctx, table2entry)) = get_filectx_by_t1index(table1_idx) {
        if table2entry.state != FileState::Loaded {
            return;
        }

        let hash = file_ctx.hash;

        let orig_size = file_ctx.get_subfile(table1_idx).decompressed_size as usize;

        let file = vec![0;file_ctx.filesize as _];
        let mut file_slice = file.into_boxed_slice();

        let cb_result = match ARC_CALLBACKS.read().get(&hash) {
            Some(cb) => {
                cb(hash, file_slice.as_mut_ptr() as *mut skyline::libc::c_void, file_ctx.filesize as _)
            },
            None => false,
        };

        if !cb_result {
            if !file_ctx.virtual_file {
                file_slice = file_ctx.get_file_content().into_boxed_slice();
            } else {
                // The file does not actually exist on the SD, so we abort here and fix the texture if the size has been modified by a callback
                let new_size = file_ctx.filesize as usize;
                let orig_size = file_ctx.orig_subfile.decompressed_size as usize;
                
                if new_size > orig_size {
                    unsafe {
                        let data_slice = std::slice::from_raw_parts_mut(table2entry.data as *mut u8, new_size);
                        // Copy our footer at the end
                        let (from, to) = data_slice.split_at_mut(new_size - 0xB0);
                        to.copy_from_slice(&from[orig_size-0xb0..orig_size]);
                    }
                }

                return;
            }
        }

        info!("[ARC::Replace | #{}] Replacing '{}'", table1_idx.green(), hashes::get(file_ctx.hash).unwrap_or(&"Unknown").bright_yellow());

        unsafe {
            let mut data_slice = std::slice::from_raw_parts_mut(table2entry.data as *mut u8, orig_size);

            if orig_size > file_slice.len() {
                // Copy the content at the beginning
                data_slice[0..file_slice.len() - 0xB0].copy_from_slice(&file_slice[0..file_slice.len() - 0xB0]);
                // Copy our new footer at the end
                data_slice[orig_size - 0xB0..orig_size].copy_from_slice(&file_slice[file_slice.len() - 0xB0..file_slice.len()]);
            } else {
                data_slice.write(&file_slice).unwrap();
            }
        }
    }
}

pub fn is_file_allowed(filepath: &Path) -> bool {
    // Check extensions
    match filepath.extension().unwrap().to_str().unwrap() {
        "numshb" | "nutexb" | "eff" | "prc" | "stprm" | "stdat" | "xmb" | "arc" | "bfotf" | "bfttf" | "numatb" | "numshexb" | "nus3bank" => false,
        &_ => true,
    }
}

#[hook(offset = TITLE_SCREEN_VERSION_OFFSET)]
fn change_version_string(arg1: u64, string: *const u8) {
    unsafe {
        let original_str = CStr::from_ptr(string as _).to_str().unwrap();

        if original_str.contains("Ver.") {
            let new_str = format!(
                "Smash {}\nARCropolis Ver. {}\0",
                original_str,
                env!("CARGO_PKG_VERSION").to_string()
            );
            original!()(arg1, skyline::c_str(&new_str))
        } else {
            original!()(arg1, string)
        }
    }
}

#[repr(u32)]
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum LoadingType {
    Directory = 0,
    Unk1 = 1,
    Unk2 = 2,
    Unk3 = 3,
    File = 4,
}

#[hook(offset = 0x33b6798, inline)]
fn incoming_file(ctx: &InlineCtx) {
    unsafe {
        //handle_file_overwrite(*ctx.registers[22].w.as_ref());
        // x26 -> FileInfoStartIdx
        let loaded_tables = LoadedTables::get_instance();
        let hash = loaded_tables.get_hash_from_t1_index(*ctx.registers[25].x.as_ref() as _).as_u64();
        println!("[ResLoadingThread] File loaded: {}", hashes::get(hash).unwrap_or(&"Unknown").green());
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);
        println!("[ResLoadingThread] Loading type: {:?}, File_idx_start: {}, File_idx_current: {}, File_idx_count: {}, Dir index: {}", res_service.processing_type,res_service.processing_file_idx_start, res_service.processing_file_idx_curr,res_service.processing_file_idx_count, res_service.current_dir_index);
    }
}

#[repr(C)]
pub struct DirectoryOffset {
    pub offset: u64,
    pub decomp_size: u32,
    pub comp_size: u32,
    pub sub_data_start_index: u32,
    pub sub_data_count: u32,
    pub redirect_index: u32,
}

#[repr(C)]
pub struct DirectoryList {
    pub full_path: HashIndexGroup,
    pub name: HashIndexGroup,
    pub parent: HashIndexGroup,
    pub extra_dis_re: HashIndexGroup,
    pub file_info_start_idx: u32,
    pub file_info_count: u32,
    pub child_directory_start_idx: u32,
    pub child_directory_count: u32,
    pub flags: u32,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ResService{
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
    pub directory_idx_queue: [CppVector<CppVector<u32>>; 4],
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
    pub buffer_array: [*const skyline::libc::c_void; 2],
    pub buffer_array_idx: u32,
    unk12: u32,
    pub data_ptr: *const skyline::libc::c_void,
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

#[hook(offset = 0x33b8410, inline)]
fn incoming_dir(ctx: &InlineCtx) {
    unsafe {
        let directory_offset = &*(*ctx.registers[28].x.as_ref() as *const DirectoryOffset);
        let dir_list_idx = *ctx.registers[22].w.as_ref();
        let file_idx_start = *ctx.registers[26].w.as_ref();
        let file_idx_count = *ctx.registers[19].w.as_ref();
        //handle_file_overwrite(*ctx.registers[22].w.as_ref());
        // x26 -> FileInfoStartIdx
        let loaded_tables = LoadedTables::get_instance();

        let directory_list_table = loaded_tables.get_arc().dir_list as *mut DirectoryList;
        let dir_list = &mut *directory_list_table.offset(dir_list_idx as isize);

        //let hash = loaded_tables.get_hash_from_t1_index().as_u64();
        println!("Dir: {}, Comp size: {:x}, Decomp size: {:x}, SubData count: {}", hashes::get(dir_list.full_path.hash40.as_u64()).unwrap_or(&"Unknown"), directory_offset.comp_size, directory_offset.decomp_size, directory_offset.sub_data_count);
    }
}

// #[hook(offset = 0x33b8240, inline)]
// fn incoming_dir(ctx: &InlineCtx) {
//     unsafe {
//         let dir_list_idx = *ctx.registers[2].w.as_ref();
//         let loaded_tables = LoadedTables::get_instance();

//         let directory_list_table = loaded_tables.get_arc().dir_list as *mut DirectoryList;
//         let dir_list = &mut *directory_list_table.offset(dir_list_idx as isize);
//         println!("Dir: {}", hashes::get(dir_list.full_path.hash40.as_u64()).unwrap_or(&"Unknown"));
//     }
// }

#[hook(offset = 0x33b71ec, inline)]
fn inflate_incoming_dir_file(ctx: &InlineCtx) {
    unsafe {
        //handle_file_overwrite(*ctx.registers[22].w.as_ref());
        // x26 -> FileInfoStartIdx
        let loaded_tables = LoadedTables::get_instance();
        let file_infos = loaded_tables.get_arc().file_info;
        let file_info = &*file_infos.offset(*ctx.registers[11].x.as_ref() as isize);
        let hash = loaded_tables.get_hash_from_t1_index(file_info.path_index).as_u64();
        println!("[ResInflateThread] File loaded: {}", hashes::get(hash).unwrap_or(&"Unknown").bright_yellow());
    }
}

#[hook(offset = 0x33b71e8, inline)]
fn inflate_sniff(ctx: &InlineCtx) {
    unsafe {
        //handle_file_overwrite(*ctx.registers[22].w.as_ref());
        // x26 -> FileInfoStartIdx
        let loaded_tables = LoadedTables::get_instance();
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);
        // let processing_type: LoadingType = match *ctx.registers[21].x.as_ref() {
        //     0 => LoadingType::Directory,
        //     // Fighter, Stage
        //     1 => LoadingType::Unk1,
        //     // Fighter
        //     2 => LoadingType::Unk2,
        //     // Unseen
        //     3 => LoadingType::Unk3,
        //     4 => LoadingType::File,
        //     _ => unreachable!(),
        // };

        // let index_count = *ctx.registers[1].x.as_ref();
        // let first_index = *ctx.registers[2].x.as_ref();
        let current_index = *ctx.registers[27].x.as_ref() as u32;
        let file_infos = loaded_tables.get_arc().file_info;
        let file_info = &*file_infos.offset((res_service.processing_file_idx_start + current_index) as isize);
        let hash = loaded_tables.get_hash_from_t1_index(file_info.path_index).as_u64();
        //println!("[ResInflateThread] Loading type: {:?}, Index count: {}", processing_type, index_count);
        println!("[ResInflateThread] Loading type: {:?}, File_idx_start: {}, File_idx_curr: {}, File_idx_count: {}, Dir index: {}, File: {}", res_service.processing_type ,res_service.processing_file_idx_start, current_index, res_service.processing_file_idx_count, res_service.current_dir_index,hashes::get(hash).unwrap_or(&"Unknown").bright_yellow());

        // if processing_type == LoadingType::Unk3 {
        //     panic!("LoadingType 3 encountered! Please write down what you did right before this showed up and tell Raytwo about this.");
        // }

    }
}


#[hook(offset = 0x33b6508, inline)]
fn loading_file_nx(ctx: &InlineCtx) {
    unsafe {
        // let loaded_tables = LoadedTables::get_instance();
        // let file_infos = loaded_tables.get_arc().file_info;
        // let file_info = &*file_infos.offset(*ctx.registers[20].w.as_ref() as isize);
        // let hash = loaded_tables.get_hash_from_t1_index(file_info.path_index).as_u64();
        // println!("[ResLoadingThread::FileNX] File loaded: {}", hashes::get(hash).unwrap_or(&"Unknown").bright_yellow());
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);
        println!("[ResLoadingThread::FileNX] File_idx_start: {}, File_idx_count: {}", res_service.processing_file_idx_start, res_service.processing_file_idx_count);
    }
}

//
#[hook(offset = 0x33b88e0, inline)]
fn dir_file_nx_1(ctx: &InlineCtx) {
    unsafe {
        let loaded_tables = LoadedTables::get_instance();
        //let file_infos = loaded_tables.get_arc().file_info;
        //let file_info = &*file_infos.offset(*ctx.registers[9].w.as_ref() as isize);
        //let hash = loaded_tables.get_hash_from_t1_index(file_info.path_index).as_u64();

        //println!("[LoadDirectory::FileNX1] File loaded: {}", hashes::get(hash).unwrap_or(&"Unknown").bright_yellow());
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);
        println!("[LoadDirectory::FileNX1] File_idx_start: {}, File_idx_current: {}, File_idx_count: {}, Into_read: {:08x}", res_service.processing_file_idx_start, res_service.processing_file_idx_curr,res_service.processing_file_idx_count, res_service.offset_into_read);
    }
}

#[hook(offset = 0x33b8528, inline)]
fn dir_file_nx_2(ctx: &InlineCtx) {
    unsafe {
        let loaded_tables = LoadedTables::get_instance();
        //let file_infos = loaded_tables.get_arc().file_info;
        //let file_info = &*file_infos.offset(*ctx.registers[9].w.as_ref() as isize);
        //let hash = loaded_tables.get_hash_from_t1_index(file_info.path_index).as_u64();

        //println!("[LoadDirectory::FileNX2] File loaded: {}", hashes::get(hash).unwrap_or(&"Unknown").bright_yellow());
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);
        println!("[LoadDirectory::FileNX2] File_idx_start: {}, File_idx_current: {}, File_idx_count: {}", res_service.processing_file_idx_start, res_service.processing_file_idx_curr,res_service.processing_file_idx_count);
    }
}

#[hook(offset = 0x33b6508, inline)]
fn loading_filenx_read(ctx: &InlineCtx) {
    unsafe {
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);
        println!("[ResLoadingThread::FileNX] File_idx_start: {}, File_idx_count: {}", res_service.processing_file_idx_start, res_service.processing_file_idx_count);
    }
}

#[hook(offset = 0x33b88e0, inline)]
fn loaddir_filenx_read_1(ctx: &InlineCtx) {
    unsafe {
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);
        println!("[LoadDirectory::FileNX1] Loading type: {:?}, File_idx_start: {}, File_idx_current: {}, File_idx_count: {}, Dir index: {}", res_service.processing_type.cyan(), res_service.processing_file_idx_start, res_service.processing_file_idx_curr,res_service.processing_file_idx_count, res_service.current_dir_index);
    }
}



#[hook(offset = 0x3638ab0, inline)]
fn filenx_read(ctx: &InlineCtx) {
    unsafe {
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);
        println!("[FileNX::Read] Loading type: {:?}, File_idx_start: {}, File_idx_current: {}, File_idx_count: {}, Dir index: {}", res_service.processing_type.cyan(), res_service.processing_file_idx_start, res_service.processing_file_idx_curr,res_service.processing_file_idx_count, res_service.current_dir_index);
    }
}

#[hook(offset = 0x33b8528, inline)]
fn loaddir_filenx_read_2(ctx: &InlineCtx) {
    unsafe {
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);
        println!("[LoadDirectory::FileNX2] Loading type: {:?}, File_idx_start: {}, File_idx_current: {}, File_idx_count: {}, Dir index: {}", res_service.processing_type.cyan(), res_service.processing_file_idx_start, res_service.processing_file_idx_curr,res_service.processing_file_idx_count, res_service.current_dir_index);
    }
}

/// Uncompressed directory files smaller than the buffer's size
#[hook(offset = 0x33b7d04, inline)]
fn memcpy1(ctx: &InlineCtx) {
    unsafe {
        println!("[ResInflateThread] Reading uncompressed file from directory, size: {:x}", *ctx.registers[2].x.as_ref());
    }
}

#[hook(offset = 0x33b7fbc, inline)]
fn state_change(ctx: &InlineCtx) {
    unsafe {
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);
        println!("[ResInflateThread] State change");
        handle_file_overwrite_test(res_service.processing_file_idx_curr);
    }
}

fn handle_file_overwrite_test(table1_idx: u32) {
    if let Some((file_ctx, table2entry)) = get_filectx_by_t1index(table1_idx) {
        if table2entry.state != FileState::Unloaded {
            return;
        }

        let hash = file_ctx.hash;

        let orig_size = file_ctx.filesize as usize;

        let file_slice = file_ctx.get_file_content().into_boxed_slice();

        info!("[ARC::Replace | #{}] Replacing '{}'", table1_idx.green(), hashes::get(file_ctx.hash).unwrap_or(&"Unknown").bright_yellow());

        unsafe {
            let mut data_slice = std::slice::from_raw_parts_mut(table2entry.data as *mut u8, file_slice.len());
            data_slice.write(&file_slice).unwrap();
        }
    }
}

/// Uncompressed directory files larger than the buffer's size
#[hook(offset = 0x33b78f4, inline)]
fn memcpy2(ctx: &InlineCtx) {
    unsafe {
        println!("[ResInflateThread] Reading uncompressed file from directory, size: {:x}", *ctx.registers[2].x.as_ref());
    }
}

/// Used to copy the rest of the file that memcpy2 couldn't
#[hook(offset = 0x33b7984, inline)]
fn memcpy3(ctx: &InlineCtx) {
    unsafe {
        println!("[ResInflateThread] Reading uncompressed file from directory, size: {:x}", *ctx.registers[2].x.as_ref());
    }
}

#[hook(offset = 0x33b7ec4, inline)]
fn waitevent(ctx: &InlineCtx) {
    unsafe {
        println!("[ResInflateThread::Compressed] WaitEvent");
    }
}

#[hook(offset = 0x33b7c3c, inline)]
fn waitevent2(ctx: &InlineCtx) {
    unsafe {
        println!("[ResInflateThread::WaitEvent] Waiting for rest of directory to be read");
    }
}

#[hook(offset = 0x3816230, inline)]
fn voodoo(ctx: &InlineCtx) {
    unsafe {
        println!("[ResInflateThread] Reading file from directory");
    }
}

/// STRATUS

#[hook(offset = 0x33b71e8, inline)]
fn inflate_incoming(ctx: &InlineCtx) {
    unsafe {
        let loaded_tables = LoadedTables::get_instance();
        let arc = loaded_tables.get_arc();
        let res_service = &mut *(ResServiceState::get_instance() as *mut ResServiceState as *mut ResService);

        // let index_count = *ctx.registers[1].x.as_ref();
        // let first_index = *ctx.registers[2].x.as_ref();
        let current_index = *ctx.registers[27].x.as_ref() as u32;
        let file_infos = arc.file_info;
        let file_info = &*file_infos.offset((res_service.processing_file_idx_start + current_index) as isize);
        let t1_idx = file_info.path_index;
        let hash = loaded_tables.get_hash_from_t1_index(t1_idx).as_u64();
        //println!("[ResInflateThread] Loading type: {:?}, Index count: {}", processing_type, index_count);
        println!("[ResInflateThread] Loading type: {:?}, File_idx_start: {}, File_idx_curr: {}, File_idx_count: {}, Dir index: {}, File: {}", res_service.processing_type ,res_service.processing_file_idx_start, current_index, res_service.processing_file_idx_count, res_service.current_dir_index,hashes::get(hash).unwrap_or(&"Unknown").bright_yellow());
        res_service.processing_file_idx_curr = t1_idx;

        match ARC_FILES.write().0.get_mut(&hash) {
            Some(context) => {
                context.filesize_replacement();
                println!("[ResInflateThread] Replaced FileData");
                //panic!();
            },
            None => {},
        }

        //handle_file_load(t1_idx);

        // if processing_type == LoadingType::Unk3 {
        //     panic!("LoadingType 3 encountered! Please write down what you did right before this showed up and tell Raytwo about this.");
        // }

    }
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    logging::init(CONFIG.logger.as_ref().unwrap().logger_level.into()).unwrap();

    // Check if an update is available
    if skyline_update::check_update(IpAddr::V4(CONFIG.updater.as_ref().unwrap().server_ip), "ARCropolis", env!("CARGO_PKG_VERSION"), CONFIG.updater.as_ref().unwrap().beta_updates) {
        skyline::nn::oe::RestartProgramNoArgs();
    }

    // Load hashes from rom:/skyline/hashes.txt if the file is present
    hashes::init();
    // Look for the offset of the various functions to hook
    offsets::search_offsets();

    // Originals
    install_hooks!(
    //     idk,
    //     add_idx_to_table1_and_table2,
         stream::lookup_by_stream_hash,
    //     parse_eff_nutexb,
    //     parse_eff,
    //     parse_param_file,
    //     parse_model_xmb,
    //     parse_model_xmb2,
    //     parse_log_xmb,
    //     parse_arc_file,
    //     parse_font_file,
    //     parse_numdlb_file,
         // parse_numshb_file,
    //     parse_numshexb_file,
    //     parse_numatb_file,
         // parse_numatb_nutexb,
    //     // parse_bntx_file,
    //     parse_nus3bank_file,
         change_version_string,
    );

    // Testing
    install_hooks!(
    //     incoming_file,
    //     incoming_dir,
    //     inflate_sniff,
    //     loaddir_filenx_read_1,
    //     loaddir_filenx_read_2,
    //     loading_file_nx,
    //     filenx_read,
        // memcpy1,
        // memcpy2,
        // memcpy3,
    //     waitevent,
    //     waitevent2,
    //     voodoo,
    );

    // Stratus
    install_hooks!(
        inflate_incoming,
        state_change,
    );

    println!(
        "ARCropolis v{} - File replacement plugin is now installed",
        env!("CARGO_PKG_VERSION")
    );
}
