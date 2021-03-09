use smash_arc::{FileInfoIndiceIdx, Hash40};

trait ArcropCallback {
    fn callback_type(&self) -> CallbackType;

    /// Maximum number of bytes needed for the given file
    fn max_file_size(&self, file: SomeFileRepresentation) -> Option<usize>;

    // when the file is loaded
    fn on_file_load(&self, file: &mut [u8]);

    fn into_ffi_callback(self) -> FfiCallback {
        FfiCallback {
            obj: Box::leak(Box::new(self)) as &mut _ as *mut c_void,
            callback_type: callback_type_wrapper::<Self>,
            max_file_size: max_file_size_wrapper::<Self>,
            on_file_load: on_file_load_wrapper::<Self>,
            free: drop_wrapper::<Self>,
        }
    }
}

enum CallbackType {
    File(FileInfoIndiceIdx),
    Extension(Hash40),
    Directory(u32)
}