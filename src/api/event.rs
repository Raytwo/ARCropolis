use std::sync::{LazyLock, RwLock};

use arcropolis_api::{Event, EventCallbackFn};

pub struct EventCallbacks {
    arc_fs_mounted: Vec<EventCallbackFn>,
    mod_fs_mounted: Vec<EventCallbackFn>,
}

impl EventCallbacks {
    pub fn new() -> Self {
        EventCallbacks {
            arc_fs_mounted: vec![],
            mod_fs_mounted: vec![],
        }
    }
}

pub static EVENT_CALLBACKS: LazyLock<RwLock<EventCallbacks>> = LazyLock::new(|| RwLock::new(EventCallbacks::new()));
pub static EVENT_QUEUE: LazyLock<RwLock<Vec<Event>>> = LazyLock::new(|| RwLock::new(Vec::new()));

impl std::ops::Index<Event> for EventCallbacks {
    type Output = Vec<EventCallbackFn>;

    fn index(&self, index: Event) -> &Self::Output {
        match index {
            Event::ArcFilesystemMounted => &self.arc_fs_mounted,
            Event::ModFilesystemMounted => &self.mod_fs_mounted,
        }
    }
}

impl std::ops::IndexMut<Event> for EventCallbacks {
    fn index_mut(&mut self, index: Event) -> &mut Self::Output {
        match index {
            Event::ArcFilesystemMounted => &mut self.arc_fs_mounted,
            Event::ModFilesystemMounted => &mut self.mod_fs_mounted,
        }
    }
}

#[no_mangle]
pub extern "C" fn arcrop_register_event_callback(ty: Event, callback: EventCallbackFn) {
    let mut cbs = EVENT_CALLBACKS.write().unwrap();
    cbs[ty].push(callback);
}

fn event_loop() {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut events = Vec::new();
        let mut full_events = EVENT_QUEUE.write().unwrap();
        std::mem::swap(&mut events, &mut full_events);
        drop(full_events);

        let cbs = EVENT_CALLBACKS.read().unwrap();

        for e in events.into_iter() {
            for cb in cbs[e].iter() {
                cb(e);
            }
        }
    }
}

pub fn send_event(e: Event) {
    EVENT_QUEUE.write().unwrap().push(e);
}

pub fn setup() {
    let _ = std::thread::spawn(event_loop);
}
