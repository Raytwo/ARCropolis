use crate::replacement_files::FileBacking;

type CallbackFn = extern "C" fn() -> bool;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Callback {
    callback: CallbackFn,
    len: u32,
    fallback: Box<FileBacking>
}