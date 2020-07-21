use smash::hash40;
use std::collections::HashMap;
use std::fs;

lazy_static::lazy_static! {
    pub static ref HASHES : HashMap<u64, &'static str> = {
        let mut hashes = HashMap::default();

        let str_path = "rom:/skyline/hashes.txt";

        let s = match fs::read_to_string(str_path){
            Err(why) =>  {
                println!("[HashesMgr] Failed to read \"{}\" \"({})\"", str_path, why);
                return hashes;
            },
            Ok(s) => s
        };

        for hs in string_to_static_str(s).lines() {
            hashes.insert(hash40(hs), hs);
        }

        return hashes;
    };
}

fn string_to_static_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub fn init() {
    lazy_static::initialize(&HASHES);
}
