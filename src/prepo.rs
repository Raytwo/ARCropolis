#[repr(C)]
#[derive(Debug)]
pub struct PlayReport {
    pub event_id: [u8;32],
    pub buffer: *const u8,
    pub size: usize,
    pub position: usize
}

#[skyline::hook(offset = 0x39C4980)]
fn prepo_save(prepo: &PlayReport, uid: &nn::account::Uid) {
    skyline::logging::hex_dump_ptr(prepo as *const PlayReport);
    println!("Event id: {}", unsafe { skyline::from_c_str(prepo.event_id.as_ptr()) });
    skyline::logging::hex_dump_ptr(prepo.buffer);
    unsafe { 
        let mut buffer = std::io::Cursor::new(std::slice::from_raw_parts(prepo.buffer, prepo.position));
        let test = rmpv::decode::read_value(&mut buffer).unwrap();
        println!("{}", serde_json::to_string_pretty(&test).unwrap());
    }
    call_original!(dbg!(prepo), uid);
}