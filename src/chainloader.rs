use std::path::PathBuf;
use std::{cmp::Ordering, path::Path};
use std::{fmt, io};
use std::ops::Deref;
use skyline::{nn, libc};
use nn::ro::{self, NrrHeader, RegistrationInfo, Module};
use std::mem::MaybeUninit;

macro_rules! align_up {
    ($x:expr, $a:expr) => {
        ((($x) + (($a) - 1)) & !(($a) - 1))
    }
}

struct Sha256Hash {
    hash: [u8; 0x20]
}

impl Sha256Hash {
    pub fn new(data: &[u8]) -> Self {
        let mut hash = [0u8; 0x20];
        unsafe {
            nn::crypto::GenerateSha256Hash(hash.as_mut_ptr() as _, 0x20, data.as_ptr() as _, data.len() as u64);
        }
        Self {
            hash
        }
    }
}

impl PartialEq for Sha256Hash {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Ord for Sha256Hash {
    fn cmp(&self, other: &Self) -> Ordering {
        let memcmp = unsafe {
            libc::memcmp(self.hash.as_ptr() as _, other.hash.as_ptr() as _, 0x20)
        };
        if memcmp < 0 {
            Ordering::Less
        }
        else if memcmp > 0 {
            Ordering::Greater
        }
        else { 
            Ordering::Equal
        }
    }
}

impl PartialOrd for Sha256Hash {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Sha256Hash {}

pub struct NrrRegistrationFailedError(u32);

impl fmt::Debug for NrrRegistrationFailedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to register module info! Reason: {:#x}", self.0)
    }
}

pub struct NroMountFailedError(u32);

impl fmt::Debug for NroMountFailedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to mount module! Reason: {:#x}", self.0)
    }
}

pub struct NrrBuilder {
    hashes: Vec<Sha256Hash>
}

const NRR_SIZE: usize = std::mem::size_of::<NrrHeader>();

impl NrrBuilder {
    pub fn new() -> Self {
        Self {
            hashes: vec![]
        }
    }

    pub fn add_module(&mut self, data: &[u8]) {
        self.hashes.push(Sha256Hash::new(data));
    }

    pub fn register(self) -> Result<RegistrationInfo, NrrRegistrationFailedError> {
        let Self { hashes: mut module_hashes } = self;
        module_hashes.sort();

        let nrr_image_size = align_up!(NRR_SIZE + module_hashes.len() * 0x20, 0x1000);
        let nrr_image = unsafe {
            let mem = libc::memalign(0x1000, nrr_image_size) as *mut u8;
            libc::memset(mem as _, 0x0, nrr_image_size);
            mem
        };

        let program_id = skyline::info::get_program_id();

        unsafe {
            let nrr_header = &mut *(nrr_image as *mut NrrHeader);
            nrr_header.magic = 0x3052524E;
            nrr_header.program_id = ro::ProgramId { value: program_id };
            nrr_header.size = nrr_image_size as u32;

            nrr_header.type_ = 0;
            nrr_header.hashes_offset = NRR_SIZE as u32;
            nrr_header.num_hashes = module_hashes.len() as u32;
        }

        for (idx, hash) in module_hashes.into_iter().enumerate() {
            unsafe {
                libc::memcpy(nrr_image.add(NRR_SIZE + idx * 0x20) as _, hash.hash.as_ptr() as _, 0x20);
            }
        }

        let mut nrr_info = MaybeUninit::uninit();
        unsafe {
            let rc = ro::RegisterModuleInfo(nrr_info.as_mut_ptr(), nrr_image as _);
            if rc != 0 {
                libc::free(nrr_image as _);
                Err(NrrRegistrationFailedError(rc))
            } else {
                Ok(nrr_info.assume_init())
            }
        }
    }
}

pub struct NroBuilder {
    data: Vec<u8>
}

impl NroBuilder {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, io::Error> {
        Ok(Self {
            data: std::fs::read(path)?
        })
    }

    pub fn mount(self) -> Result<Module, NroMountFailedError> {
        let Self { data } = self;

        let nro_image = unsafe {
            let nro_memory = libc::memalign(0x1000, data.len()) as *mut u8;
            libc::memcpy(nro_memory as _, data.as_ptr() as _, data.len());
            nro_memory as *const libc::c_void
        };

        let bss_size = unsafe {
            let mut size = 0;
            let rc = nn::ro::GetBufferSize(&mut size, nro_image);
            if rc != 0 {
                Err(NroMountFailedError(rc))
            } else {
                Ok(size as usize)
            }
        }?;

        let bss_section = unsafe {
            libc::memalign(0x1000, bss_size)
        };

        let mut nro_module = MaybeUninit::uninit();
        unsafe {
            let rc = ro::LoadModule(nro_module.as_mut_ptr(), nro_image, bss_section, bss_size as u64, ro::BindFlag_BindFlag_Now as i32);
            if rc == 0 {
                Ok(nro_module.assume_init())
            } else {
                libc::free(nro_image as *mut libc::c_void);
                libc::free(bss_section);
                Err(NroMountFailedError(rc))
            }
        }
    }
}

impl Deref for NroBuilder {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.data.as_slice()
    }
}

pub fn load_and_run_plugins(plugins: &Vec<(PathBuf, PathBuf)>) {
    let mut plugin_nrr = NrrBuilder::new();

    let modules: Vec<NroBuilder> = plugins.iter().filter_map(|(root, local)| {
        let full_path = root.join(local);

        if full_path.exists() && full_path.ends_with("plugin.nro") {
            match NroBuilder::open(&full_path) {
                Ok(builder) => {
                    info!("Loaded plugin at '{}' for chainloading.", full_path.display());
                    plugin_nrr.add_module(&builder);
                    Some(builder)
                },
                Err(e) => {
                    error!("Failed to load plugin at '{}'. {:?}", full_path.display(), e);
                    None
                }
            }
        } else {
            error!("File discovery collected path '{}' but it does not exist and/or is invalid!", full_path.display());
            None
        }
    }).collect();

    if modules.is_empty() {
        info!("No plugins found for chainloading.");
        return;
    }

    if let Err(e) = plugin_nrr.register() {
        error!("{:?}", e);
        crate::dialog_error("ARCropolis failed to register plugin module info.");
        return;
    }

    let modules: Vec<Module> = modules.into_iter().filter_map(|x| {
        match x.mount() {
            Ok(module) => Some(module),
            Err(e) => {
                error!("Failed to mount chainloaded plugin. {:?}", e);
                None
            }
        }
    }).collect();

    if modules.len() < plugins.len() {
        crate::dialog_error("ARCropolis failed to load/mount some plugins.");
    } else {
        info!("Successfully chainloaded all collected plugins.");
    }

    for module in modules.into_iter() {
        let callable = unsafe {
            let mut sym_loc = 0usize;
            let rc = nn::ro::LookupModuleSymbol(&mut sym_loc, &module, "main\0".as_ptr() as _);
            if rc != 0 {
                warn!("Failed to find symbol 'main' in chainloaded plugin.");
                None
            } else {
                Some(std::mem::transmute::<usize, extern "C" fn()>(sym_loc))
            }
        };

        if let Some(entrypoint) = callable {
            info!("Calling 'main' in chainloaded plugin"); 
            entrypoint();
            info!("Finished calling 'main' in chainloaded plugin");
        }
    }
}