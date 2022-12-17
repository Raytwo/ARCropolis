#[repr(C)]
#[derive(Debug)]
pub struct PlayReport {
    pub event_id: [u8;32],
    pub buffer: *const u8,
    pub size: usize,
    pub position: usize
}

#[link_name = "_ZN2nn5prepo10PlayReport4SaveERKNS_7account3UidE"]
extern "C" fn prepo_save(prepo: &PlayReport, uid: &nn::account::Uid);

#[skyline::hook(replace = prepo_save)]
fn prepo_save_hook(prepo: &PlayReport, uid: &nn::account::Uid) {
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

pub fn install() {
    skyline::install_hook!(prepo_save_hook)
}