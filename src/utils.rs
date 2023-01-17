use skyline::nn;

pub mod env {
    use std::str::FromStr;
    use semver::Version;
    use once_cell::sync::Lazy;

    use super::*;

    #[non_exhaustive]
    pub enum RunEnvironment {
        Switch,
        Ryujinx,
        // Yuzu
    }

    static PLATFORM: Lazy<RunEnvironment> = Lazy::new(|| {
        if unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64 } == 0x8004000 {
            RunEnvironment::Ryujinx
        } else {
            RunEnvironment::Switch
        }
    });

    pub fn get_running_env() -> &'static RunEnvironment {
        &PLATFORM
    }

    pub fn is_emulator() -> bool {
        matches!(get_running_env(), RunEnvironment::Switch)
    }

    pub fn is_ryujinx() -> bool {
        matches!(get_running_env(), RunEnvironment::Ryujinx)
    }

    /// Wrapper function for getting the version string of the game from nnSdk
    pub fn get_game_version() -> Version {
        unsafe {
            // TODO: Implement this in nnsdk-rs
            let mut version_string = nn::oe::DisplayVersion { name: [0x00; 16] };
            nn::oe::GetDisplayVersion(&mut version_string);
            Version::from_str(&skyline::from_c_str(version_string.name.as_ptr())).expect("Smash's version should parse as a proper semver.")
        }
    }

    pub fn get_arcropolis_version() -> Version {
        Version::from_str(env!("CARGO_PKG_VERSION")).expect("ARCropolis' version should follow proper semver.")
    }
}

pub mod paths {
    use std::io;
    use camino::Utf8PathBuf;
    use super::env::get_game_version;

    pub fn ensure_paths_exist() -> io::Result<()> {
        std::fs::create_dir_all(mods())?;
        std::fs::create_dir_all(config())?;
        std::fs::create_dir_all(logs())?;
        std::fs::create_dir_all(cache())?;
        Ok(())
    }

    pub fn mods() -> Utf8PathBuf {
        Utf8PathBuf::from("sd:/ultimate/mods")
    }

    pub fn config() -> Utf8PathBuf {
        Utf8PathBuf::from("sd:/ultimate/arcropolis/config")
    }

    pub fn logs() -> Utf8PathBuf {
        Utf8PathBuf::from("sd:/ultimate/arcropolis/logs")
    }

    pub fn cache() -> Utf8PathBuf {
        Utf8PathBuf::from("sd:/ultimate/arcropolis/cache").join(get_game_version().to_string())
    }
}

pub mod save {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Result};
    use smash_arc::Region;

    #[repr(u8)]
    #[derive(Debug)]
    pub enum SaveLanguageId {
        Japanese = 0,
        English,
        French,
        Spanish,
        German,
        Italian,
        Dutch,
        Russian,
        Chinese,
        Taiwanese,
        Korean,
    }

    impl From<u8> for SaveLanguageId {
        fn from(byte: u8) -> Self {
            match byte {
                0 => Self::Japanese,
                1 => Self::English,
                2 => Self::French,
                3 => Self::Spanish,
                4 => Self::German,
                5 => Self::Italian,
                6 => Self::Dutch,
                7 => Self::Russian,
                8 => Self::Chinese,
                9 => Self::Taiwanese,
                10 => Self::Korean,
                _ => Self::English,
            }
        }
    }

    pub fn mount_save(mount_path: &str) {
        // TODO: Call nn::fs::CheckMountName
        // This provides a UserHandle and sets the User in a Open state to be used.
        let handle = nn::account::try_open_preselected_user().expect("OpenPreselectedUser should not return false");
        // Obtain the UID for this user
        let uid = nn::account::get_user_id(&handle).expect("GetUserId should return a valid Uid");

        unsafe { nn::fs::MountSaveData(skyline::c_str(&format!("{}\0", mount_path)), &uid) };

        // This closes the UserHandle, making it unusable, and sets the User in a Closed state.
        // Smash will crash if you don't do it.
        nn::account::close_user(handle);
    }

    pub fn unmount_save(mount_path: &str) {
        // TODO: Call nn::fs::CheckMountName
        unsafe { nn::fs::Unmount(skyline::c_str(&format!("{}\0", mount_path))) };
    }

    pub fn get_language_id_in_savedata() -> Result<SaveLanguageId> {
        let mut file = std::fs::File::open("save:/save_data/system_data.bin")?;
        file.seek(SeekFrom::Start(0x3c6098)).unwrap();
        let mut language_code = [0u8];
        file.read_exact(&mut language_code).unwrap();
        drop(file);

        Ok(SaveLanguageId::from(language_code[0]))
    }

    pub fn get_system_region_from_language_id(language: SaveLanguageId) -> Region {
        let system_locale_id = unsafe { *(skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as *const u8).add(0x523b00c) };
    
        let system_region_map = unsafe {
            std::slice::from_raw_parts(
                (skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as *const u32).add(0x4740f90 / 4),
                14,
            )
        };
    
        let system_region = system_region_map[system_locale_id as usize];
    
        match language {
            SaveLanguageId::Japanese => Region::Japanese,
            SaveLanguageId::English => {
                if system_region == 1 {
                    // US
                    Region::UsEnglish
                } else {
                    Region::EuEnglish
                }
            },
            SaveLanguageId::French => {
                if system_region == 1 {
                    // US
                    Region::UsFrench
                } else {
                    Region::EuFrench
                }
            },
            SaveLanguageId::Spanish => {
                if system_region == 1 {
                    // US
                    Region::UsSpanish
                } else {
                    Region::EuSpanish
                }
            },
            SaveLanguageId::German => Region::EuGerman,
            SaveLanguageId::Dutch => Region::EuDutch,
            SaveLanguageId::Italian => Region::EuItalian,
            SaveLanguageId::Russian => Region::EuRussian,
            SaveLanguageId::Chinese => Region::ChinaChinese,
            SaveLanguageId::Taiwanese => Region::TaiwanChinese,
            SaveLanguageId::Korean => Region::Korean,
        }
    }
}