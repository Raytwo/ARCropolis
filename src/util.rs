pub mod env {
    #[non_exhaustive]
    pub enum RunEnvironment {
        Switch,
        Ryujinx,
        // Yuzu
    }

    lazy_static! {
        static ref PLATFORM: RunEnvironment = {
            if unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64 } == 0x8004000 {
                RunEnvironment::Ryujinx
            } else {
                RunEnvironment::Switch
            }
        };
    }

    pub fn get_running_env() -> &'static RunEnvironment {
        &PLATFORM
    }

    pub fn is_emulator() -> bool {
        match get_running_env() {
            RunEnvironment::Switch => false,
            _ => true,
        }
    }
}