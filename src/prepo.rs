
use super::logging;
use std::{time::SystemTime, path::PathBuf, fs::DirEntry};
use nnsdk::prepo::PlayReport;
use nnsdk as nn;

#[skyline::hook(replace = nn::prepo::PlayReport_SaveWithUserId)]
fn prepo_save(prepo: &PlayReport, uid: &nn::account::Uid) {
    skyline::logging::hex_dump_ptr(prepo as *const PlayReport);
    let event_id = unsafe { skyline::from_c_str(prepo.event_id.as_ptr()) };
    println!("Event id: {}", event_id);
    skyline::logging::hex_dump_ptr(prepo.buffer);
    unsafe {
        let mut buffer = std::io::Cursor::new(std::slice::from_raw_parts(prepo.buffer, prepo.position));
        let test = rmpv::decode::read_value(&mut buffer).unwrap();
        let json_data = serde_json::to_string_pretty(&test).unwrap();
        println!("{}", json_data);

        let seconds = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Clock may have gone backwards!");

        let file_path = format!("sd:/ultimate/arcropolis/prepo/{}-{}.json", event_id, logging::format_time_string(seconds.as_secs()));

        std::fs::write(file_path, json_data).unwrap();
    }
    if !event_id.starts_with("arc_") {
        dbg!(event_id);
        call_original!(dbg!(prepo), uid);
    }
}

#[skyline::hook(offset = 0x39c4a60)]
fn immediate_transmission() {
    let prepo_reader = std::fs::read_dir("sd:/ultimate/arcropolis/prepo").expect("Dir missing?");
    let mut prepo_logs: Vec<String> = prepo_reader.into_iter().map(|p| p.unwrap().path().display().to_string()).collect();
    send_logs_to_api(prepo_logs);
    call_original!()
}

fn send_logs_to_api(prepo_logs: Vec<String>) {
    unimplemented!()
}


pub fn install() {
    skyline::install_hooks!(prepo_save, immediate_transmission);
}