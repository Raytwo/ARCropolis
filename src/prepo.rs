
use super::logging;
use std::{time::SystemTime, path::PathBuf, fs::DirEntry};
use nnsdk::prepo::PlayReport;
use nnsdk as nn;

#[skyline::hook(replace = nn::prepo::PlayReport_SaveWithUserId)]
fn prepo_save(prepo: &PlayReport, uid: &nn::account::Uid) {
    // log the memory address of the PlayReport object
    skyline::logging::hex_dump_ptr(prepo as *const PlayReport);

    // retrieve the event_id field from the PlayReport object and convert it to a Rust string
    let event_id = unsafe { skyline::from_c_str(prepo.event_id.as_ptr()) };
    println!("Event id: {}", event_id);

    // log the memory address of the buffer field in the PlayReport object
    skyline::logging::hex_dump_ptr(prepo.buffer);

    // read and decode the value stored in the buffer field of the PlayReport object
    unsafe {
        let mut buffer = std::io::Cursor::new(std::slice::from_raw_parts(prepo.buffer, prepo.position));
        let test = rmpv::decode::read_value(&mut buffer).unwrap();

        // convert the decoded value to a pretty-printed JSON string
        let json_data = serde_json::to_string_pretty(&test).unwrap();
        println!("{}", json_data);

        // get the current time in seconds since the Unix epoch
        let seconds = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Clock may have gone backwards!");

        // create a file path with the event id and current time in the format "sd:/ultimate/arcropolis/prepo/{event_id}-{time}.json"
        let file_path = format!("sd:/ultimate/arcropolis/prepo/{}-{}.json", event_id, logging::format_time_string(seconds.as_secs()));

        // write the JSON data to the file
        std::fs::write(file_path, json_data).unwrap();
    }

    // if the event id does not start with "arc_", call the original function with the PlayReport object and user id
    if !event_id.starts_with("arc_") {
        dbg!(event_id);
        call_original!(dbg!(prepo), uid);
    }
    // if the event id does start with "arc_", only log the PlayReport object
    else {
        dbg!(prepo);
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