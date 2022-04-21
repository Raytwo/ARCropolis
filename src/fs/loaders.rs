use std::collections::VecDeque;

use msbt::{builder::MsbtBuilder, Msbt};
use serde::*;
use serde_xml_rs;
use xml::common::Position;

use super::*;

#[derive(Deserialize, Debug)]
pub struct XMSBT {
    #[serde(rename = "entry")]
    entries: Vec<Entry>,
}

#[derive(Deserialize, Debug)]
pub struct Entry {
    label: String,
    #[serde(rename = "text")]
    text: Text,
}

#[derive(Deserialize, Debug)]
pub struct Text {
    #[serde(rename = "$value")]
    value: String,
}

#[derive(Error, Debug)]
pub enum ApiLoaderError {
    #[error("Error loading file from the data.arc.")]
    Arc(#[from] LookupError),
    #[error("Unable to generate hash from path.")]
    Hash(#[from] crate::InvalidOsStrError),
    #[error("Invalid callback type found.")]
    InvalidCb,
    #[error("Failed to find next virtual file!")]
    NoVirtFile,
    #[error("IO Error")]
    IO(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

#[derive(Copy, Clone, Debug)]
enum ApiLoadType {
    Nus3bankPatch,
    PrcPatch,
    MsbtPatch,
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
        match self {
            ApiLoadType::Nus3bankPatch => true,
            _ => false,
        }
    }

    pub fn get_file_size(self, local: &Path) -> Option<usize> {
        match self {
            ApiLoadType::Nus3bankPatch => {
                let arc = resource::arc();
                crate::get_smash_hash(local)
                    .ok()
                    .map(|hash| arc.get_file_data_from_hash(hash, config::region()).ok())
                    .flatten()
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
        match self {
            ApiLoadType::Nus3bankPatch => {
                let data = ApiLoader::handle_load_vanilla_file(local)?;
                Ok((data.len(), data))
            },
            ApiLoadType::PrcPatch => {
                let patches = if let Some(patches) = ApiLoader::get_prc_patches_for_hash(local.smash_hash()?) {
                    patches
                } else {
                    return Err(ApiLoaderError::Other("No patches found for file in PRC patch!".to_string()));
                };

                let data = ApiLoader::handle_load_base_file(local)?;

                let mut param_data = prcx::read_stream(&mut std::io::Cursor::new(data))
                    .map_err(|_| ApiLoaderError::Other("Unable to parse param data!".to_string()))?;

                for patch_path in patches.iter() {
                    let patch = if let Ok(patch) = prcx::open(patch_path) {
                        patch
                    } else {
                        let file = std::fs::File::open(patch_path)?;
                        let mut reader = std::io::BufReader::new(file);
                        prcx::read_xml(&mut reader).map_err(|_| ApiLoaderError::Other("Unable to parse param patch data!".to_string()))?
                    };
                    prcx::apply_patch(&patch, &mut param_data).map_err(|_| ApiLoaderError::Other("Unable to patch param data!".to_string()))?;
                }

                let mut cursor = std::io::Cursor::new(vec![]);
                prcx::write_stream(&mut cursor, &param_data)?;
                let vec = cursor.into_inner();
                Ok((vec.len(), vec))
            },
            ApiLoadType::MsbtPatch => {
                let patches = if let Some(patches) = ApiLoader::get_msbt_patches_for_hash(local.smash_hash()?) {
                    patches
                } else {
                    return Err(ApiLoaderError::Other("No patches found for file in MSBT patch!".to_string()));
                };

                let mut labels: HashMap<String, String> = HashMap::new();

                for patch_path in patches.iter() {
                    let data = &std::fs::read(patch_path).unwrap()[2..];

                    let slice_u16: &[u16] = unsafe {
                        std::slice::from_raw_parts(
                            data.as_ptr() as *const u16,
                            data.len() / std::mem::size_of::<u16>(),
                        )
                    };

                    let mut xml = String::from_utf16(slice_u16).unwrap();

                    let xmsbt: XMSBT = match serde_xml_rs::from_str(&xml) {
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
                        labels.insert(entry.label.to_owned(), entry.text.value.to_owned());
                    }
                }

                let data = ApiLoader::handle_load_base_file(local)?;

                let mut msbt = Msbt::from_reader(std::io::Cursor::new(&data)).unwrap();

                for lbl in msbt.lbl1_mut().unwrap().labels_mut() {
                    if labels.contains_key(&lbl.name().to_owned()) {
                        let mut str_val: Vec<u16> = labels[&lbl.name().to_owned()].encode_utf16().collect();
                        str_val.push(0);

                        let slice_u8: &[u8] = unsafe {
                            std::slice::from_raw_parts(
                                str_val.as_ptr() as *const u8,
                                str_val.len() * std::mem::size_of::<u16>(),
                            )
                        };

                        lbl.set_value_raw(slice_u8).unwrap();
                        labels.remove(&lbl.name().to_owned());
                    }
                }

                let mut builder = MsbtBuilder::from(msbt);

                for lbl in labels {
                    let mut str_val: Vec<u16> = lbl.1
                            .encode_utf16()
                            .collect();
                        str_val.push(0);

                        let slice_u8: &[u8] = unsafe {
                            std::slice::from_raw_parts(
                                str_val.as_ptr() as *const u8,
                                str_val.len() * std::mem::size_of::<u16>(),
                            )
                        };
                    builder = builder.add_label(lbl.0, slice_u8);
                }

                let mut out_msbt = builder.build();

                let mut cursor = std::io::Cursor::new(vec![]);
                out_msbt.write_to(&mut cursor).unwrap();
                let vec = cursor.into_inner();
                Ok((vec.len(), vec))
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

pub struct ApiLoader {
    function_map: HashMap<Hash40, UnsafeCell<ApiFunctionEntry>>,
    stream_size_map: UnsafeCell<HashMap<PathBuf, usize>>,
    param_patches: HashMap<Hash40, Vec<PathBuf>>,
    msbt_patches: HashMap<Hash40, Vec<PathBuf>>,
}

unsafe impl Send for ApiLoader {}
unsafe impl Sync for ApiLoader {}

impl ApiLoader {
    pub fn new() -> Self {
        Self {
            function_map: HashMap::new(),
            stream_size_map: UnsafeCell::new(HashMap::new()),
            param_patches: HashMap::new(),
            msbt_patches: HashMap::new(),
        }
    }

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
        local
            .smash_hash()
            .ok()
            .map(|x| self.function_map.get(&x))
            .flatten()
            .map(|entry| {
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
            .flatten()
    }

    fn release_virtual_file(&self, local: &Path) {
        let _ = local.smash_hash().ok().map(|x| self.function_map.get(&x)).flatten().map(|entry| {
            let data = entry.get();
            unsafe {
                (*data).function_index = ((*data).function_index - 1).min(0);
            }
            ()
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
                .map(|x| x.get_file_size(local_path))
                .flatten()
                .or(self.get_file_size(root_path, local_path));
            self.release_virtual_file(local_path);
            result
        } else {
            None
        }
    }

    fn get_path_type(&self, _root_path: &Path, local_path: &Path) -> Result<FileEntryType, Self::ErrorType> {
        if let Some((root_path, _)) = self.use_virtual_file(local_path) {
            let result = ApiLoadType::from_root(root_path);
            let result = if result.is_ok() {
                result
                    .unwrap()
                    .get_path_type(local_path)
                    .map_or_else(|_| self.get_path_type(root_path, local_path), |x| Ok(x))
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
            Some(
                self.get_stream_cb_path(local_path)
                    .map_or(root_path.join(local_path), |x| PathBuf::from(x)),
            )
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

    fn load_path(&self, root_path: &Path, local_path: &Path) -> Result<Vec<u8>, Self::ErrorType> {
        let hash = if local_path.to_str().unwrap().is_empty() {
            Ok(Hash40(u64::from_str_radix(&root_path.to_str().unwrap()[2..], 16).unwrap()))
        } else {
            crate::get_smash_hash(local_path)
        };

        match hash {
            Ok(path) => self.get_file_contents(path, config::region()),
            Err(_) => Err(LookupError::Missing),
        }
    }
}
