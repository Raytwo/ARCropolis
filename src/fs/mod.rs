pub mod visit;
pub use visit::*;

// use std::vec;

// // Add a dependency to this-error and make custom error types?
use smash_arc::{
    ArcLookup,
    FileData,
    // FileInfo,
    // FileInfoFlags,
    // FilePath,
    Hash40,
    LookupError,
    Region,
};

use crate::runtime::LoadedTables;

pub struct Metadata(Hash40);

#[allow(dead_code)]
pub fn metadata<H: Into<Hash40>>(hash: H) -> Result<Metadata, String> {
    let hash = hash.into();
    match LoadedTables::get_arc().get_file_path_index_from_hash(hash) {
        Ok(_) => Ok(Metadata(hash)),
        Err(_) => Err("No FilePath found for this hash".to_string()),
    }
}

impl Metadata {
    #[allow(dead_code)]
    pub fn file_data(&self) -> Result<&FileData, LookupError> {
        // Assume it exists because you can't instantiate a Metadata if the hash does not exist to begin with
        LoadedTables::get_arc().get_file_data_from_hash(self.0, Region::UsEnglish)
    }
}

// pub struct DirInfoEntry(FileInfo);

// impl DirInfoEntry {
//     pub fn path(&self) -> FilePath {
//         let arc = LoadedTables::get_arc();
//         arc.get_file_paths()[usize::from(self.0.file_path_index)]
//     }

//     pub fn metadata(&self) -> &FileData {
//         let arc = LoadedTables::get_arc();
//         arc.get_file_data(&self.0, Region::UsEnglish)
//     }

//     pub fn flags(&self) -> FileInfoFlags {
//         self.0.flags
//     }
// }

// pub struct ReadDirInfo {
//     inner_iter: vec::IntoIter<FileInfo>,
// }

// impl Iterator for ReadDirInfo {
//     type Item = Result<DirInfoEntry, ()>;

//     fn next(&mut self) -> Option<Self::Item> {
//         let val = self.inner_iter.next()?;

//         Some(Ok(DirInfoEntry(val)))
//     }
// }

// pub fn read_dir_info(index: u32) -> Result<ReadDirInfo, ()> {
//     let arc = LoadedTables::get_arc();
//     let dir_info = &arc.get_dir_infos()[index as usize];

//     let start = dir_info.file_info_start_index as usize;
//     let end = start + dir_info.file_info_count as usize;

//     let infos = &arc.get_file_infos()[start..end];

//     Ok(ReadDirInfo {
//         inner_iter: infos.to_vec().into_iter(),
//     })
// }
