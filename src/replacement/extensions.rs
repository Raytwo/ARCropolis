use smash_arc::{ArcLookup, Hash40, LoadedArc, LookupError, Region};

pub trait LoadedArcEx {
    fn patch_filedata(&mut self, hash: Hash40, size: u32, region: Region) -> Result<u32, LookupError>;
}

impl LoadedArcEx for LoadedArc {
    fn patch_filedata(&mut self, hash: Hash40, size: u32, region: Region) -> Result<u32, LookupError> {
        let file_info = *self.get_file_info_from_hash(hash)?;
        let region = if file_info.flags.is_localized() {
            region
        } else {
            Region::None
        };

        let file_data = self.get_file_data_mut(&file_info, region);
        let old_size = file_data.decomp_size;
        file_data.decomp_size = size;
        Ok(old_size)
    }
}