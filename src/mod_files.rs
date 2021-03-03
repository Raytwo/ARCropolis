use std::path::Path;

mod arc_file;
mod stream_file;

pub trait ModFile {
    fn get_path(&self) -> &Path;
    fn get_size(&self) -> u32;
    //fn get_file(&self) -> &[u8];
}