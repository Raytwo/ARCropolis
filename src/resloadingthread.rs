use skyline::{nn, hook};
mod resource;
use resource::*;

#[hook(offset = 0x3251630)]
pub fn res_loading_thread_main(res_service: &mut ResServiceState) {
        // Reimplementing this will take forever :pensive:
        original!()(res_service);
}