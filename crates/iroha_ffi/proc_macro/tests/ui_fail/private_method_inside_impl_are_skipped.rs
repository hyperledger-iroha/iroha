use iroha_ffi::{FfiConvert, FfiType, ffi_export};

/// FfiStruct
#[derive(Clone, FfiType)]
pub struct FfiStruct;

#[ffi_export]
impl FfiStruct {
    /// Private methods are skipped
    fn private(self) {}
}

fn main() {
    let s = FfiStruct;
    unsafe {
        // Function not found
        FfiStruct__private(FfiConvert::into_ffi(s, &mut ()));
    }
}
