use skyline::nn;

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
        matches!(get_running_env(),RunEnvironment::Switch)
    }

    pub fn is_ryujinx() -> bool {
        matches!(get_running_env(), RunEnvironment::Ryujinx)
    }
}

/// Wrapper function for getting the version string of the game from nnSdk
pub fn get_version_string() -> String {
    unsafe {
        let mut version_string = nn::oe::DisplayVersion { name: [0x00; 16] };
        nn::oe::GetDisplayVersion(&mut version_string);
        skyline::from_c_str(version_string.name.as_ptr())
    }
}