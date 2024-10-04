use skyline::nn;

pub mod env {
    use semver::Version;
    use std::{str::FromStr, sync::LazyLock};

    use super::*;

    #[non_exhaustive]
    pub enum RunEnvironment {
        Switch,
        Emulator,
    }

    static PLATFORM: LazyLock<RunEnvironment> = LazyLock::new(|| {
        let base_addr = unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64 };

        if base_addr == 0x8004000 || base_addr == 0x8504000 {
            RunEnvironment::Emulator
        } else {
            RunEnvironment::Switch
        }
    });

    pub fn get_running_env() -> &'static RunEnvironment {
        &PLATFORM
    }

    pub fn is_hardware() -> bool {
        matches!(get_running_env(), RunEnvironment::Switch)
    }

    pub fn is_emulator() -> bool {
        matches!(get_running_env(), RunEnvironment::Emulator)
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
    use super::env::get_game_version;
    use camino::Utf8PathBuf;
    use std::io;

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
