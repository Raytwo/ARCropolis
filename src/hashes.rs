use std::{collections::HashMap, fs, sync::{LazyLock, RwLock}};

use smash_arc::Hash40;

static HASH_FILEPATH: &str = "sd:/ultimate/arcropolis/hashes.txt";

static HASHES: LazyLock<RwLock<HashMap<Hash40, &'static str>>> = LazyLock::new(|| {
    let mut hashes = HashMap::default();

    let str_path = "sd:/ultimate/arcropolis/hashes.txt";

    let s = match fs::read_to_string(str_path) {
        Err(e) => {
            warn!(
                "Failed to read '{}' for hashes. Reason: {:?}. There won't be any hash lookups in this run's logs.",
                HASH_FILEPATH, e
            );
            return RwLock::new(hashes);
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
    let hashes = HASHES.read().unwrap();
    hashes.get(&hash).copied()
}

pub fn find(hash: Hash40) -> &'static str {
    try_find(hash).unwrap_or("Unknown")
}

pub fn add<S: AsRef<str>>(new_hash: S) {
    let new_hash = new_hash.as_ref();
    let mut hashes = HASHES.write().unwrap();
    let _ = hashes.try_insert(Hash40::from(new_hash), string_to_static_str(new_hash.to_string()));
}

pub fn init() {
    LazyLock::force(&HASHES);
}
