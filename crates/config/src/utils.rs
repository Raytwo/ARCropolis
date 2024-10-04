pub mod env {
    use semver::Version;
    use std::str::FromStr;

    pub fn get_arcropolis_version() -> Version {
        Version::from_str(env!("CARGO_PKG_VERSION")).expect("ARCropolis' version should follow proper semver.")
    }
}

pub mod paths {
    use camino::Utf8PathBuf;

    pub fn mods() -> Utf8PathBuf {
        Utf8PathBuf::from("sd:/ultimate/mods")
    }
}
