use smash_arc::Hash40;

#[no_mangle]
pub extern "C" fn arcapi_load_file(hash: Hash40, buffer: *mut FfiBytes) {
    unimplemented!()
}

// Following struct taken from smash-arc/ffi_bindings.rs
type FfiBytes = FfiVec<u8>;

/// An owned slice of bytes
#[repr(C)]
pub struct FfiVec<T: Sized> {
    /// May be null on error
    ptr: *mut T,
    size: usize,
}

impl<T: Sized> From<Option<Vec<T>>> for FfiVec<T> {
    fn from(list: Option<Vec<T>>) -> Self {
        match list {
            Some(list) => {
                let size = list.len();
                let ptr = Box::leak(list.into_boxed_slice()).as_mut_ptr();

                Self { ptr, size }
            }

            None => Self { ptr: std::ptr::null_mut(), size: 0 }
        }
    }
}