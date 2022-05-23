use std::{collections::HashMap, fs};

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use smash_arc::Hash40;

static HASH_FILEPATH: &'static str = "sd:/ultimate/arcropolis/hashes.txt";

static HASHES: Lazy<RwLock<HashMap<Hash40, &'static str>>> = Lazy::new(|| {
    let mut hashes = HashMap::default();

    let str_path = "sd:/ultimate/arcropolis/hashes.txt";

    let s = match fs::read_to_string(str_path) {
        Err(e) => {
            warn!(
                "Failed to read '{}' for hashes. Reason: {:?}. There won't be any hash lookups in this run's logs.",
                HASH_FILEPATH, e
            );
            return RwLock::new(hashes)
        },
        Ok(s) => s,
    };

    for hs in string_to_static_str(s).lines() {
        hashes.insert(Hash40::from(hs), hs);
    }

    RwLock::new(hashes)
});

fn string_to_static_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub fn try_find(hash: Hash40) -> Option<&'static str> {
    let hashes = HASHES.read();
    hashes.get(&hash).map(|x| *x)
}

pub fn find(hash: Hash40) -> &'static str {
    try_find(hash).unwrap_or("Unknown")
}

pub fn add<S: AsRef<str>>(new_hash: S) {
    let new_hash = new_hash.as_ref();
    let mut hashes = HASHES.write();
    let _ = hashes.try_insert(Hash40::from(new_hash), string_to_static_str(new_hash.to_string()));
}

pub fn init() {
    Lazy::force(&HASHES);
}
