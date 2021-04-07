use smash_arc::*;
use binread::*;
use std::collections::HashMap;
use std::io::{Read, Seek, Write};
use std::path::PathBuf;
#[derive(BinRead, Debug)]
// #[br(little)]
pub struct CacheEntry {
    file: HashToIndex,
    dir_entry: HashToIndex,
    index_into_dir: u32
}
#[derive(BinRead, Debug)]
#[br(little, assert(arc_version_str.to_string() == "ARC_VER" && mod_version_str.to_string() == "MOD_VER"))]
pub struct UnshareCache {
    arc_version_str: NullString,
    pub arc_version: u32,
    mod_version_str: NullString,
    pub mod_version: u32,
    length: u32,
    #[br(count = length)]
    #[br(parse_with = cache_entry_parser)]
    pub entries: HashMap<HashToIndex, (HashToIndex, u32)>
}

fn cache_entry_parser<R: Read + Seek>(reader: &mut R, ro: &ReadOptions, _: ()) -> BinResult<HashMap<HashToIndex, (HashToIndex, u32)>> {
    let mut options = *ro;
    let count = match options.count.take() {
        Some(x) => x,
        None => { panic!("Missing count for HashMap"); }
    };

    let mut map = HashMap::new();

    for _ in 0..count {
        map.insert(
            reader.read_le().unwrap(),
            reader.read_le().unwrap()
        );
    }

    Ok(map)
}

impl UnshareCache {
    pub fn new(arc: &LoadedArc) -> HashMap<HashToIndex, (HashToIndex, u32)> {
        // let arc_str = NullString::from("ARC_VER");
        // let mod_str = NullString::from("MOD_VER");
        // let arc_ver = arc.file_system.fs_header.version;
        // let mod_ver = 0u32;

        let mut cache_map: HashMap<HashToIndex, (HashToIndex, u32)> = HashMap::new();
        let file_paths = arc.get_file_paths();
        let dir_infos = arc.get_dir_infos();
        let file_infos = arc.get_file_infos();
        for (idx, dir_info) in dir_infos.iter().enumerate() {
            let mut self_hash_to_index = dir_info.path;
            self_hash_to_index.set_index(idx as u32);
            let child_infos = file_infos.iter().skip(dir_info.file_info_start_index as usize).take(dir_info.file_info_count as usize);
            for (child_idx, child_info) in child_infos.enumerate() {
                let mut path = file_paths[usize::from(child_info.file_path_index)].path;
                path.set_index(arc.get_file_path_index_from_hash(path.hash40()).unwrap().0);
                cache_map.insert(path, (self_hash_to_index, child_idx as u32));
            }
        }
        cache_map
    }

    pub fn write(arc: &LoadedArc, cache_map: &HashMap<HashToIndex, (HashToIndex, u32)>, path: &PathBuf) -> std::io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        let out_vec: Vec<(HashToIndex, (HashToIndex, u32))> = cache_map.iter().map(|(hash, dir)| (*hash, *dir)).collect();
        let entries = out_vec.len() as u32;
        unsafe {
            writer.write_all(
                b"ARC_VER\0"
            ).unwrap();
            writer.write_all(
                std::slice::from_raw_parts(&(*arc.fs_header).version as *const u32 as *const u8, std::mem::size_of::<u32>())
            ).unwrap();
            writer.write_all(
                b"MOD_VER\0"
            ).unwrap();
            writer.write_all(
                std::slice::from_raw_parts(&0u32 as *const u32 as *const u8, std::mem::size_of::<u32>())
            ).unwrap();
            writer.write_all(
                std::slice::from_raw_parts(&entries as *const u32 as *const u8, std::mem::size_of::<u32>())
            ).unwrap();
            for (hash, dir_info) in out_vec.iter() {
                writer.write_all(
                    std::slice::from_raw_parts(hash as *const HashToIndex as *const u8, std::mem::size_of::<HashToIndex>())
                ).unwrap();
                writer.write_all(
                    std::slice::from_raw_parts(&dir_info.0 as *const HashToIndex as *const u8, std::mem::size_of::<HashToIndex>())
                ).unwrap();
                writer.write_all(
                    std::slice::from_raw_parts(&dir_info.1 as *const u32 as *const u8, std::mem::size_of::<u32>())
                ).unwrap();
            }
        }
        Ok(())
    }

    // pub fn map(self) -> HashMap<HashToIndex, (HashToIndex, u32)> {
    //     let mut map: HashMap<HashToIndex, (HashToIndex, u32)> = self.entries.into_iter().map(|entry| {
    //         (entry.file, (entry.dir_entry, entry.index_into_dir))
    //     }).collect();
    //     map
    // }
}
