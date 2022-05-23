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
        match get_running_env() {
            RunEnvironment::Switch => false,
            _ => true,
        }
    }

    pub fn is_ryujinx() -> bool {
        match get_running_env() {
            RunEnvironment::Ryujinx => true,
            _ => false,
        }
    }
}
