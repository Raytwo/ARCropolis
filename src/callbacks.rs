use std::path::PathBuf;

use smash_arc::Hash40;
use crate::replacement_files::FileBacking;
use arcropolis_api::{ CallbackFn, StreamCallbackFn };

#[repr(C)]
pub enum CallbackKind {
    Regular(Callback),
    Stream(StreamCallback),
}

#[repr(C)]
#[derive(Clone)]
pub struct Callback {
    pub callback_fn: CallbackFn,
    pub path: Option<PathBuf>,
    pub len: u32,
    pub previous: Box<FileBacking>
}

#[repr(C)]
#[derive(Clone)]
pub struct StreamCallback {
    pub callback_fn: StreamCallbackFn,
    pub path: Option<PathBuf>,
    pub len: u32,
    pub previous: Box<FileBacking>
}