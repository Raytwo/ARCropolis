use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{Cursor, Read},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use hash40::diff::Diff;
use msbt::{builder::MsbtBuilder, Msbt};
use nus3audio::*;
use serde::*;
use serde_xml_rs;
use serde_yaml::from_str;
use smash_bgm_property::BgmPropertyFile;
use xml::common::Position;

use super::*;

#[derive(Debug, Deserialize)]
pub struct Xmsbt {
    #[serde(rename = "entry")]
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
pub struct Entry {
    label: String,
    base64: Option<bool>,
    #[serde(rename = "text")]
    text: Text,
}

#[derive(Debug, Deserialize)]
pub struct Text {
    #[serde(rename = "$value")]
    value: String,
}

pub enum TextType {
    Text(String),
    Data(Vec<u8>),
}

#[derive(Debug, Error)]
pub enum ApiLoaderError {
    #[error("Error loading file from the data.arc.")]
    Arc(#[from] LookupError),

    #[error("Unable to generate hash from path.")]
    Hash(#[from] crate::InvalidOsStrError),

    #[error("Invalid serde_yaml found.")]
    InvalidSerde(#[from] serde_yaml::Error),

    #[error("Invalid callback type found.")]
    InvalidCb,

    #[error("Failed to find next virtual file!")]
    NoVirtFile,

    #[error("IO Error")]
    IO(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Copy)]
enum ApiLoadType {
    Nus3bankPatch,
    PrcPatch,
    MsbtPatch,
    Nus3audioPatch,
    MotionlistPatch,
    BgmPropertyPatch,
    Generic,
    Stream,
    Extension,
}

impl ApiLoadType {
    pub fn from_root(root: &Path) -> Result<Self, ApiLoaderError> {
        if root.ends_with("patch-nus3bank") {
            Ok(ApiLoadType::Nus3bankPatch)
        } else if root.ends_with("patch-prc") {
            Ok(ApiLoadType::PrcPatch)
        } else if root.ends_with("patch-msbt") {
            Ok(ApiLoadType::MsbtPatch)
        } else if root.ends_with("patch-nus3audio") {
            Ok(ApiLoadType::Nus3audioPatch)
        } else if root.ends_with("patch-motionlist") {
            Ok(ApiLoadType::MotionlistPatch)
        } else if root.ends_with("patch-bgm_property") {
            Ok(ApiLoadType::BgmPropertyPatch)
        } else if root.ends_with("generic-cb") {
            Ok(ApiLoadType::Generic)
        } else if root.ends_with("stream-cb") {
            Ok(ApiLoadType::Stream)
        } else if root.ends_with("extension-cb") {
            Ok(ApiLoadType::Extension)
        } else {
            Err(ApiLoaderError::Other(format!("Cannot find ApiLoadType for root {}", root.display())))
        }
    }

    pub fn path_exists(self, _local: &Path) -> bool {
        matches!(self, ApiLoadType::Nus3bankPatch)
    }

    pub fn get_file_size(self, local: &Path) -> Option<usize> {
        match self {
            ApiLoadType::Nus3bankPatch => {
                let arc = resource::arc();
                crate::get_smash_hash(local)
                    .ok()
                    .and_then(|hash| arc.get_file_data_from_hash(hash, config::region()).ok())
                    .map(|x| x.decomp_size as usize)
            },
            _ => None,
        }
    }

    pub fn get_path_type(self, local: &Path) -> Result<FileEntryType, ApiLoaderError> {
        match self {
            ApiLoadType::Nus3bankPatch => {
                let search = resource::search();
                let hash = crate::get_smash_hash(local)?;
                if search.get_path_list_entry_from_hash(hash)?.is_directory() {
                    Ok(FileEntryType::Directory)
                } else {
                    Ok(FileEntryType::File)
                }
            },
            _ => Err(ApiLoaderError::Other("Unimplemented ApiLoadType!".to_string())),
        }
    }

    pub fn load_path(self, local: &Path, usr_fn: ApiCallback) -> Result<(usize, Vec<u8>), ApiLoaderError> {
        println!("[ARCropolis::loader] Patching {:#?}", local.as_os_str());

        match self {
            ApiLoadType::Nus3bankPatch => {
                let data = ApiLoader::handle_load_vanilla_file(local)?;
                Ok((data.len(), data))
            },
            ApiLoadType::PrcPatch => {
                let patches = if let Some(patches) = ApiLoader::get_prc_patches_for_hash(local.smash_hash()?) {
                    patches
                } else {
                    return Err(ApiLoaderError::Other("[ARCropolis::loader] No patches found for file of type PRC!".to_string()));
                };

                let data = ApiLoader::handle_load_base_file(local)?;
                let mut param_data = prcx::read_stream(&mut Cursor::new(data))
                    .map_err(|_| ApiLoaderError::Other("Unable to parse param data!".to_string()))?;

                for patch_path in patches.iter() {
                    let patch = if let Ok(patch) = prcx::open(patch_path) {
                        patch
                    } else {
                        let file = File::open(patch_path)?;
                        let mut reader = std::io::BufReader::new(file);

                        prcx::read_xml(&mut reader).map_err(|_| ApiLoaderError::Other("Unable to parse param patch data!".to_string()))?
                    };

                    prcx::apply_patch(&patch, &mut param_data).map_err(|_| ApiLoaderError::Other("Unable to patch param data!".to_string()))?;
                }

                let mut writer = Cursor::new(Vec::new());
                prcx::write_stream(&mut writer, &param_data)?;
                let data = writer.into_inner();
                Ok((data.len(), data))
            },
            ApiLoadType::MsbtPatch => {
                let patches = if let Some(patches) = ApiLoader::get_msbt_patches_for_hash(local.smash_hash()?) {
                    patches
                } else {
                    return Err(ApiLoaderError::Other("No patches found for file of type MSBT!".to_string()));
                };

                let mut labels: HashMap<String, TextType> = HashMap::new();

                for patch_path in patches.iter() {
                    let mut reader = Cursor::new(fs::read(patch_path)?);
                    let xmsbt: Xmsbt = match serde_xml_rs::from_reader(&mut reader) {
                        Ok(xmsbt) => xmsbt,
                        Err(err) => {
                            match err {
                                serde_xml_rs::Error::Syntax { source }  => {
                                    let position = source.position();
                                    warn!("XMSBT file `{}` could not be read due to the following syntax error at line {}, column {}: `{}`, skipping.", patch_path.display(), position.row + 1, position.column, source.msg())
                                },
                                _ => warn!("XMSBT file `{}` is malformed, skipping.", patch_path.display()),
                            }

                            continue;
                        },
                    };

                    for entry in &xmsbt.entries {
                        if entry.base64.unwrap_or(false) {
                            match BASE64_STANDARD.decode::<String>(entry.text.value.to_owned()) {
                                Ok(mut decoded) => {
                                    // Pushing these 0s to ensure that the end of the text is marked clearly
                                    decoded.push(0);
                                    decoded.push(0);
                                    labels.insert(entry.label.to_owned(), TextType::Data(decoded));
                                },
                                Err(err) => error!("XMSBT Label {} could not be base64 decoded. Reason: {}", entry.label, err),
                            }
                        } else {
                            labels.insert(entry.label.to_owned(), TextType::Text(entry.text.value.to_owned()));
                        }
                    }
                }

                let data = ApiLoader::handle_load_base_file(local)?;
                let mut msbt = Msbt::from_reader(Cursor::new(&data)).unwrap();

                for lbl in msbt.lbl1_mut().unwrap().labels_mut() {
                    let lbl_name = &lbl.name().to_owned();

                    if labels.contains_key(lbl_name) {
                        let text_data = match &labels[lbl_name] {
                            TextType::Text(text) => {
                                let mut str_val: Vec<u16> = text.encode_utf16().collect();
                                str_val.push(0);

                                let slice_u8: &[u8] = unsafe {
                                    std::slice::from_raw_parts(
                                        str_val.as_ptr() as *const u8,
                                        str_val.len() * std::mem::size_of::<u16>(),
                                    )
                                };

                                slice_u8
                            },
                            TextType::Data(data) => &data,
                        };

                        lbl.set_value_raw(text_data).unwrap();
                        labels.remove(lbl_name);
                    }
                }

                let mut builder = MsbtBuilder::from(msbt);

                for lbl in labels {
                    let text_data = match &lbl.1 {
                        TextType::Text(text) => {
                            let mut str_val: Vec<u16> = text
                                    .encode_utf16()
                                    .collect();

                            str_val.push(0);

                            let slice_u8: &[u8] = unsafe {
                                std::slice::from_raw_parts(
                                    str_val.as_ptr() as *const u8,
                                    str_val.len() * std::mem::size_of::<u16>(),
                                )
                            };

                            slice_u8
                        },
                        TextType::Data(data) => &data,
                    };

                    builder = builder.add_label(lbl.0, text_data);
                }

                let out_msbt = builder.build();
                let mut cursor = Cursor::new(Vec::new());
                out_msbt.write_to(&mut cursor).unwrap();
                let data = cursor.into_inner();
                Ok((data.len(), data))
            },
            ApiLoadType::Nus3audioPatch => {
                let patches = if let Some(patches) = ApiLoader::get_nus3audio_patches_for_hash(local.smash_hash()?) {
                    patches
                } else {
                    return Err(ApiLoaderError::Other("No patches found for file of type NUS3AUDIO!".to_string()));
                };

                // Initialize the `original_file` variable, which parses the pre patch file into the nus3audio type
                let mut original_file = Nus3audioFile::from_bytes(&ApiLoader::handle_load_base_file(local).unwrap()[..]);

                // This is a little weird imo, but it's the only good solution I could come up with
                // Basically what it's doing past this point is:
                //     ~ looping through the original file's audiofiles to get their names and insert the name
                //     and itself into the HashMap
                //     ~ looping through the patches, and then applying them to the HashMap
                //     ~ setting the base file's AudioFile vec to the values of the HashMap
                let mut known_audiofiles: HashMap<String, AudioFile> = original_file.files.iter().map(|audio_file| (audio_file.name.clone(), audio_file.clone())).collect();

                // Iterate through the patches
                for patch_path in patches.iter() {
                    // Reads the patch file data and parses it into the nus3audio type
                    let patch_data = &std::fs::read(patch_path).unwrap()[..];
                    let modified_file = Nus3audioFile::from_bytes(patch_data);

                    // Iterate through the AudioFiles of the modified file
                    for mut audio_file in modified_file.files {
                        // Check if the known AudioFiles HashMap contains the name of the current AudioFile
                        if known_audiofiles.contains_key(&audio_file.name) {
                            // If it does, set the already made AudioFile's data to the modified one/
                            println!("Found {}! Patching...", &audio_file.name);
                            known_audiofiles.get_mut(&audio_file.name).unwrap().data = audio_file.data.clone();
                        }
                        else {
                            // If it doesn't, insert it into the known_audiofiles HashMap
                            println!("Not found {}! Adding...", &audio_file.name);
                            audio_file.id = (known_audiofiles.len() + 1) as u32;
                            known_audiofiles.try_insert(audio_file.name.clone(), audio_file.clone()).unwrap();
                        }
                    }
                }

                // Initialize the `new_audio_files` Vec, which takes in the values of the known_audiofiles HashMap
                let mut new_audio_files: Vec<AudioFile> = known_audiofiles.iter().map(|(_, audio_file)| audio_file.clone()).collect();

                // Sort the new_audio_files vec by ID, because if we don't, the game loads the wrong AudioFiles on request.
                new_audio_files.sort_by(|a, b| a.id.cmp(&b.id));

                // Set the original file's AudioFile vec to the new_audio_files vec
                original_file.files = new_audio_files;

                let mut contents: Vec<u8> = Vec::new();

                // Write the contents of the original file to the contents vec
                original_file.write(&mut contents);

                // Return the length of the contents and the contents
                Ok((contents.len(), contents))

            },
            ApiLoadType::MotionlistPatch => {
                let patches = if let Some(patches) = ApiLoader::get_motionlist_patches_for_hash(local.smash_hash()?) {
                    patches
                } else {
                    return Err(ApiLoaderError::Other("[ARCropolis::loader] No patches found for files motion_list.bin!".to_string()));
                };

                let mut yml_patches = Vec::new();
                let mut diff_patches = Vec::new();

                for patch_path in patches.iter() {
                    if patch_path.has_extension("motdiff") {
                        diff_patches.push(patch_path.clone());
                    } else if patch_path.ends_with("motion_list.yml") {
                        yml_patches.push(patch_path.clone());
                    } else {
                        return Err(ApiLoaderError::Other("This isn't a motion list patch file!".to_string()));
                    }
                }

                let data = ApiLoader::handle_load_base_file(local)?;
                let mut reader = Cursor::new(data);
                let mut motion_list = motion_lib::read_stream(&mut reader)?;

                if !yml_patches.is_empty() {
                    println!("[ARCropolis::loader] motion_list.yml file(s) found!");
                    let mut full_patches = 0;

                    for full_patch in yml_patches.iter() {
                        println!("[ARCropolis::loader] Replacing motion_list.bin with {}.", full_patch.to_str().unwrap());
                        let mut contents: String = String::default();
                        File::open(full_patch)?.read_to_string(&mut contents)?;
                        if let Some(full) = from_str(&contents)? {
                            motion_list = full;
                            full_patches += 1;
                        }
                    }

                    if full_patches > 1 {
                        println!("[ARCropolis::loader] Multiple motion_list.yml files found for {}.", local.to_str().unwrap());
                        println!("                     The last applied .yml file will be used.");
                    }
                }

                if !diff_patches.is_empty() {
                    for patch_path in diff_patches.iter() {
                        let mut contents: String = String::default();
                        File::open(patch_path)?.read_to_string(&mut contents)?;
                        if let Some(diff) = from_str(&contents)? {
                            motion_list.apply(&diff);
                        }
                        else {
                            return Err(ApiLoaderError::Other("This isn't a motion list patch file!".to_string()));
                        }
                    }
                }

                println!("[ARCropolis::loader] 'motion_list.bin' patching finished!");
                let mut writer = Cursor::new(Vec::new());
                motion_lib::write_stream(&mut writer, &motion_list)?;
                let data = writer.into_inner();
                Ok((data.len(), data))
            },
            ApiLoadType::BgmPropertyPatch => {
                let patches = if let Some(patches) = ApiLoader::get_bgm_property_patches_for_hash(local.smash_hash()?) {
                    patches
                } else {
                    return Err(ApiLoaderError::Other("[ARCropolis::loader] No patches found for file bgm_property.bin!".to_string()));
                };

                let data = ApiLoader::handle_load_base_file(local)?;
                let mut reader = Cursor::new(&data[..]);
                let mut bgm_property = BgmPropertyFile::read(&mut reader).unwrap();

                for patch_path in patches.iter() {
                    let mut patch_file = BgmPropertyFile::from_file(patch_path).unwrap();

                    bgm_property.entries.append(&mut patch_file.entries);
                }

                let mut writer = Cursor::new(Vec::new());
                bgm_property.write(&mut writer).unwrap();
                let data = writer.into_inner();
                Ok((data.len(), data))
            },
            ApiLoadType::Generic if let ApiCallback::GenericCallback(cb) = usr_fn => {
                let hash = local.smash_hash()?;
                let mut size = 0;

                if !crate::api::file::arcrop_get_decompressed_size(hash, &mut size) {
                    return Err(ApiLoaderError::Other("Unable to create buffer!".to_string()));
                }

                let mut vec = Vec::with_capacity(size);

                unsafe {
                    let mut new_len = size;

                    if !cb(hash.0, vec.as_mut_ptr(), size, &mut new_len) {
                        return Err(ApiLoaderError::Other("Callback did not load file!".to_string()));
                    }

                    vec.set_len(new_len);
                }

                Ok((size, vec))
            },
            ApiLoadType::Generic => Err(ApiLoaderError::InvalidCb),
            ApiLoadType::Stream if let ApiCallback::StreamCallback(cb) = usr_fn => {
                let hash = local.smash_hash()?;
                let mut vec = Vec::with_capacity(0x100);
                let mut file_size = 0;

                unsafe {
                    if !cb(hash.0, vec.as_mut_ptr(), &mut file_size) {
                        return Err(ApiLoaderError::Other("Callback did not provide a valid path!".to_string()));
                    }

                    vec.set_len(0x100);
                }

                Ok((file_size, vec))
            },
            ApiLoadType::Stream => Err(ApiLoaderError::InvalidCb),
            _ => Err(ApiLoaderError::Other("Unimplemented ApiLoadType!".to_string()))
        }
    }
}

#[derive(Copy, Clone)]
pub enum ApiCallback {
    None,
    GenericCallback(arcropolis_api::CallbackFn),
    StreamCallback(arcropolis_api::StreamCallbackFn),
}

#[repr(transparent)]
struct UnsafeSize(UnsafeCell<usize>);

unsafe impl Send for UnsafeSize {}
unsafe impl Sync for UnsafeSize {}

struct ApiFunctionEntry {
    pub function_index: usize,
    pub functions: VecDeque<(PathBuf, ApiCallback)>,
}

#[derive(Default)]
pub struct ApiLoader {
    function_map: HashMap<Hash40, UnsafeCell<ApiFunctionEntry>>,
    stream_size_map: UnsafeCell<HashMap<PathBuf, usize>>,
    param_patches: HashMap<Hash40, Vec<PathBuf>>,
    msbt_patches: HashMap<Hash40, Vec<PathBuf>>,
    nus3audio_patches: HashMap<Hash40, Vec<PathBuf>>,
    motionlist_patches: HashMap<Hash40, Vec<PathBuf>>,
    bgm_property_patches: HashMap<Hash40, Vec<PathBuf>>,
}

unsafe impl Send for ApiLoader {}
unsafe impl Sync for ApiLoader {}

impl ApiLoader {
    pub fn push_entry(&mut self, hash: Hash40, root: &Path, cb: ApiCallback) {
        if let Some(list) = self.function_map.get_mut(&hash) {
            list.get_mut().functions.push_front((root.to_path_buf(), cb));
        } else {
            let mut vdq = VecDeque::new();
            vdq.push_front((root.to_path_buf(), cb));

            self.function_map.insert(
                hash,
                UnsafeCell::new(ApiFunctionEntry {
                    function_index: 0,
                    functions: vdq,
                }),
            );
        }
    }

    fn use_virtual_file(&self, local: &Path) -> Option<(&Path, ApiCallback)> {
        local.smash_hash().ok().and_then(|x| self.function_map.get(&x)).and_then(|entry| {
            let data = entry.get();

            unsafe {
                if let Some((vroot, func)) = (*data).functions.get((*data).function_index) {
                    (*data).function_index += 1;
                    Some((vroot.as_path(), *func))
                } else {
                    None
                }
            }
        })
    }

    fn release_virtual_file(&self, local: &Path) {
        let _ = local.smash_hash().ok().and_then(|x| self.function_map.get(&x)).map(|entry| {
            let data = entry.get();

            unsafe {
                (*data).function_index = ((*data).function_index - 1).min(0);
            }
        });
    }

    pub fn handle_load_vanilla_file(local: &Path) -> Result<Vec<u8>, ApiLoaderError> {
        let arc = resource::arc();
        let hash = crate::get_smash_hash(local)?;

        Ok(arc.get_file_contents(hash, config::region())?)
    }

    pub fn handle_load_base_file(local: &Path) -> Result<Vec<u8>, ApiLoaderError> {
        let filesystem = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };
        let cached = filesystem.get();

        if cached.get_patch_entry_type(local).is_ok() {
            cached.load_patch(local).map_err(|x| ApiLoaderError::Other(format!("{:?}", x)))
        } else {
            Self::handle_load_vanilla_file(local)
        }
    }

    pub fn get_prc_patches_for_hash(hash: Hash40) -> Option<&'static Vec<PathBuf>> {
        let filesystem = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };
        let cached = filesystem.get();

        cached.virt().loader.param_patches.get(&hash)
    }

    pub fn get_msbt_patches_for_hash(hash: Hash40) -> Option<&'static Vec<PathBuf>> {
        let filesystem = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };
        let cached = filesystem.get();

        cached.virt().loader.msbt_patches.get(&hash)
    }

    pub fn get_nus3audio_patches_for_hash(hash: Hash40) -> Option<&'static Vec<PathBuf>> {
        let filesystem = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };
        let cached = filesystem.get();

        cached.virt().loader.nus3audio_patches.get(&hash)
    }

    pub fn get_motionlist_patches_for_hash(hash: Hash40) -> Option<&'static Vec<PathBuf>> {
        let filesystem = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };
        let cached = filesystem.get();

        cached.virt().loader.motionlist_patches.get(&hash)
    }

    pub fn get_bgm_property_patches_for_hash(hash: Hash40) -> Option<&'static Vec<PathBuf>> {
        let filesystem = unsafe { &*crate::GLOBAL_FILESYSTEM.data_ptr() };
        let cached = filesystem.get();

        cached.virt().loader.bgm_property_patches.get(&hash)
    }

    pub fn insert_prc_patch(&mut self, hash: Hash40, path: &Path) {
        if let Some(list) = self.param_patches.get_mut(&hash) {
            list.push(path.to_path_buf())
        } else {
            self.param_patches.insert(hash, vec![path.to_path_buf()]);
        }
    }

    pub fn insert_msbt_patch(&mut self, hash: Hash40, path: &Path) {
        if let Some(list) = self.msbt_patches.get_mut(&hash) {
            list.push(path.to_path_buf())
        } else {
            self.msbt_patches.insert(hash, vec![path.to_path_buf()]);
        }
    }

    pub fn insert_nus3audio_patch(&mut self, hash: Hash40, path: &Path) {
        if let Some(list) = self.nus3audio_patches.get_mut(&hash) {
            list.push(path.to_path_buf())
        } else {
            self.nus3audio_patches.insert(hash, vec![path.to_path_buf()]);
        }
    }

    pub fn insert_motionlist_patch(&mut self, hash: Hash40, path: &Path) {
        if let Some(list) = self.motionlist_patches.get_mut(&hash) {
            list.push(path.to_path_buf())
        } else {
            self.motionlist_patches.insert(hash, vec![path.to_path_buf()]);
        }
    }

    pub fn insert_bgm_property_patch(&mut self, hash: Hash40, path: &Path) {
        if let Some(list) = self.bgm_property_patches.get_mut(&hash) {
            list.push(path.to_path_buf())
        } else {
            self.bgm_property_patches.insert(hash, vec![path.to_path_buf()]);
        }
    }

    fn get_stream_cb_path(&self, local: &Path) -> Option<String> {
        if let Some((root_path, callback)) = self.use_virtual_file(local) {
            let result = match ApiLoadType::from_root(root_path) {
                Ok(ApiLoadType::Stream) => match ApiLoadType::Stream.load_path(local, callback) {
                    Ok((sz, data)) => unsafe {
                        if let Some(prev_size) = (*self.stream_size_map.get()).get_mut(local) {
                            *prev_size = sz;
                        } else {
                            (*self.stream_size_map.get()).insert(local.to_path_buf(), sz);
                        }
                        Some(skyline::from_c_str(data.as_ptr()))
                    },
                    _ => self.get_stream_cb_path(local),
                },
                _ => self.get_stream_cb_path(local),
            };
            self.release_virtual_file(local);
            result
        } else {
            None
        }
    }
}

impl FileLoader for ApiLoader {
    type ErrorType = ApiLoaderError;

    fn path_exists(&self, _root_path: &Path, local_path: &Path) -> bool {
        if let Some((root_path, _)) = self.use_virtual_file(local_path) {
            let result = ApiLoadType::from_root(root_path).map(|x| x.path_exists(local_path)).unwrap_or(false);
            let result = if !result { self.path_exists(root_path, local_path) } else { result };
            self.release_virtual_file(local_path);
            result
        } else {
            false
        }
    }

    fn get_file_size(&self, _root_path: &Path, local_path: &Path) -> Option<usize> {
        if let Some(sz) = unsafe { (*self.stream_size_map.get()).get(local_path) } {
            return Some(*sz);
        }
        if let Some((root_path, _)) = self.use_virtual_file(local_path) {
            let result = ApiLoadType::from_root(root_path)
                .ok()
                .and_then(|x| x.get_file_size(local_path))
                .or_else(|| self.get_file_size(root_path, local_path));
            self.release_virtual_file(local_path);
            result
        } else {
            None
        }
    }

    fn get_path_type(&self, _root_path: &Path, local_path: &Path) -> Result<FileEntryType, Self::ErrorType> {
        if let Some((root_path, _)) = self.use_virtual_file(local_path) {
            let result = ApiLoadType::from_root(root_path);
            let result = if let Ok(result) = result {
                result
                    .get_path_type(local_path)
                    .map_or_else(|_| self.get_path_type(root_path, local_path), Ok)
            } else {
                Err(result.unwrap_err())
            };
            self.release_virtual_file(local_path);
            result
        } else {
            Err(ApiLoaderError::NoVirtFile)
        }
    }

    fn load_path(&self, _root_path: &Path, local_path: &Path) -> Result<Vec<u8>, Self::ErrorType> {
        if let Some((root_path, callback)) = self.use_virtual_file(local_path) {
            let result = match ApiLoadType::from_root(root_path) {
                Ok(ty) => ty
                    .load_path(local_path, callback)
                    .map_or_else(|_| self.load_path(root_path, local_path), |(_, data)| Ok(data)),
                Err(e) => Err(e),
            };
            self.release_virtual_file(local_path);
            result
        } else {
            Err(ApiLoaderError::NoVirtFile)
        }
    }

    fn get_actual_path(&self, root_path: &Path, local_path: &Path) -> Option<PathBuf> {
        if root_path.ends_with("stream-cb") {
            Some(self.get_stream_cb_path(local_path).map_or(root_path.join(local_path), PathBuf::from))
        } else {
            Some(root_path.join(local_path))
        }
    }
}

#[repr(transparent)]
pub struct ArcLoader(pub(super) &'static LoadedArc);

unsafe impl Send for ArcLoader {}
unsafe impl Sync for ArcLoader {}

impl Deref for ArcLoader {
    type Target = LoadedArc;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl FileLoader for ArcLoader {
    type ErrorType = LookupError;

    fn path_exists(&self, _: &Path, local_path: &Path) -> bool {
        match crate::get_smash_hash(local_path) {
            Ok(hash) => self.get_file_path_index_from_hash(hash).is_ok(),
            _ => false,
        }
    }

    fn get_file_size(&self, _: &Path, local_path: &Path) -> Option<usize> {
        match crate::get_smash_hash(local_path) {
            Ok(hash) => self
                .get_file_data_from_hash(hash, config::region())
                .map_or_else(|_| None, |data| Some(data.decomp_size as usize)),
            Err(_) => None,
        }
    }

    fn get_path_type(&self, _: &Path, local_path: &Path) -> Result<FileEntryType, Self::ErrorType> {
        match crate::get_smash_hash(local_path) {
            Ok(hash) => match self.get_path_list_entry_from_hash(hash)?.is_directory() {
                true => Ok(FileEntryType::Directory),
                false => Ok(FileEntryType::File),
            },
            _ => Err(LookupError::Missing),
        }
    }

    #[track_caller]
    fn load_path(&self, _root_path: &Path, local_path: &Path) -> Result<Vec<u8>, Self::ErrorType> {
        let hash = if local_path.to_str().unwrap().is_empty() {
            Ok(Hash40(u64::from_str_radix(&local_path.to_str().unwrap()[2..], 16).unwrap()))
        } else {
            crate::get_smash_hash(local_path)
        };

        match hash {
            Ok(path) => self.get_file_contents(path, config::region()),
            Err(_) => Err(LookupError::Missing),
        }
    }
}
