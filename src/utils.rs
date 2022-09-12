use std::str::FromStr;

use skyline::nn;
use semver::Version;

pub mod env {
    use once_cell::sync::Lazy;

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
}

pub mod paths {
    use std::io;

    use super::*;
    use camino::Utf8PathBuf;

    pub fn ensure_paths_exist() -> io::Result<()> {
        std::fs::create_dir_all(&mods())?;
        std::fs::create_dir_all(&config())?;
        std::fs::create_dir_all(&logs())?;
        std::fs::create_dir_all(&cache())?;
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

/// Wrapper function for getting the version string of the game from nnSdk
pub fn get_game_version() -> Version {
    unsafe {
        let mut version_string = nn::oe::DisplayVersion { name: [0x00; 16] };
        nn::oe::GetDisplayVersion(&mut version_string);
        Version::from_str(&skyline::from_c_str(version_string.name.as_ptr())).expect("Smash's version should parse as a proper semver.")
    }
}

pub fn get_arcropolis_version() -> Version {
    Version::from_str(env!("CARGO_PKG_VERSION")).expect("ARCropolis' version should follow proper semver.")
}