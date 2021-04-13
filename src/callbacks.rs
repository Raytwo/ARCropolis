use smash_arc::Hash40;
use crate::replacement_files::FileBacking;

// Hash, out_buffer, length
pub type CallbackFn = extern "C" fn(*mut usize, u64, *mut u8, usize) -> bool;

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Callback {
    pub callback: CallbackFn,
    pub len: u32,
    pub previous: Box<FileBacking>
}