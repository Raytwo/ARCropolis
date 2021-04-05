use smash_arc::Hash40;
use crate::replacement_files::FileBacking;

type CallbackFn = extern "C" fn(Hash40) -> bool;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Callback {
    pub callback: CallbackFn,
    pub len: u32,
    pub fallback: Box<FileBacking>
}