use skyline::{nn, hook};
mod resource;
use resource::*;

#[hook(offset = 0x3251630)]
pub fn res_loading_thread_main(res_service: &mut ResServiceState)
{
        original!()(res_service);
}