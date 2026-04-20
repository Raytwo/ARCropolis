#[derive(Copy, Clone)]
pub enum ApiCallback {
    None,
    GenericCallback(arcropolis_api::CallbackFn),
    StreamCallback(arcropolis_api::StreamCallbackFn),
}
