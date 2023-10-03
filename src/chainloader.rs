use std::{cmp::Ordering, fmt, io::{self, Seek, Read}, mem::MaybeUninit, ops::Deref, path::Path};

use nn::ro::{self, Module, NrrHeader, RegistrationInfo};
use once_cell::sync::Lazy;
use skyline::{libc, nn};

macro_rules! align_up {
    ($x:expr, $a:expr) => {
        ((($x) + (($a) - 1)) & !(($a) - 1))
    };
}

struct Sha256Hash {
    hash: [u8; 0x20],
}

impl Sha256Hash {
    pub fn new(data: &[u8]) -> Self {
        let mut hash = [0u8; 0x20];
        unsafe {
            nn::crypto::GenerateSha256Hash(hash.as_mut_ptr() as _, 0x20, data.as_ptr() as _, data.len() as u64);
        }
        Self { hash }
    }
}

impl PartialEq for Sha256Hash {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Ord for Sha256Hash {
    fn cmp(&self, other: &Self) -> Ordering {
        let memcmp = unsafe { libc::memcmp(self.hash.as_ptr() as _, other.hash.as_ptr() as _, 0x20) };
        memcmp.cmp(&0)
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
    hashes: Vec<Sha256Hash>,
}

static ORIGINAL_GAME_NRO_HASHES: Lazy<Vec<Sha256Hash>> = Lazy::new(|| { 
    let mut out = Vec::new();
    let read_dir = std::fs::read_dir("rom:/.nrr/").unwrap();
    for entry in read_dir.flatten() {
         let path = entry.path();
         if path.is_file() {
             let mut file = std::fs::File::open(path).unwrap();
             file.seek(io::SeekFrom::Start(0x340)).unwrap();
             // Manually read in the hashes_offset and num_hashes members of NRR file header.
             let mut hashes_offset_bytes = [0u8; 4];
             file.read_exact(&mut hashes_offset_bytes).unwrap();
             let hashes_offset = u32::from_le_bytes(hashes_offset_bytes);
             let mut num_hashes_bytes = [0u8; 4];
             file.read_exact(&mut num_hashes_bytes).unwrap();
             let num_hashes = u32::from_le_bytes(num_hashes_bytes);
             
             // Seek to hashes_offset and read the module hashes into nrr_hashes.
             file.seek(io::SeekFrom::Start(hashes_offset as u64)).unwrap();
             let mut nrr_hashes = Vec::with_capacity(num_hashes as usize);
             for i in 0..num_hashes {
                 let mut hash_bytes = [0u8; 0x20];
                 file.read_exact(&mut hash_bytes).unwrap();
                 nrr_hashes.push(Sha256Hash { hash: hash_bytes });
             }
             // Append the module hashes from this NRR into the global vector.
             out.extend(nrr_hashes);
         }
    }
    out
    }
);

const NRR_SIZE: usize = std::mem::size_of::<NrrHeader>();

impl NrrBuilder {
    pub fn new() -> Self {
        Self { hashes: vec![] }
    }

    pub fn add_module(&mut self, data: &[u8]) {
        let hash = Sha256Hash::new(data);
        let original_hashes = Lazy::force(&ORIGINAL_GAME_NRO_HASHES);
        if !original_hashes.contains(&hash) {
            self.hashes.push(hash);
        }
    }

    pub fn register(self) -> Result<Option<RegistrationInfo>, NrrRegistrationFailedError> {
        let Self { hashes: mut module_hashes } = self;
        if module_hashes.is_empty() {
            return Ok(None);
        }
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
                Ok(Some(nrr_info.assume_init()))
            }
        }
    }
}

pub struct NroBuilder {
    data: Vec<u8>,
}

impl NroBuilder {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, io::Error> {
        Ok(Self { data: std::fs::read(path)? })
    }

    pub fn mount(self) -> Result<Module, NroMountFailedError> {
        let Self { data } = self;

        let nro_image = unsafe {
            let nro_memory = libc::memalign(0x1000, data.len()) as *mut u8;
            libc::memcpy(nro_memory as _, data.as_ptr() as _, data.len());
            nro_memory as *const u8
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

        let bss_section = unsafe { libc::memalign(0x1000, bss_size) as *mut u8 };

        let mut nro_module = MaybeUninit::uninit();
        unsafe {
            let rc = ro::LoadModule(
                nro_module.as_mut_ptr(),
                nro_image,
                bss_section,
                bss_size as u64,
                ro::BindFlag_BindFlag_Now as i32,
            );
            if rc == 0 {
                Ok(nro_module.assume_init())
            } else {
                libc::free(nro_image as *mut libc::c_void);
                libc::free(bss_section as *mut libc::c_void);
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
