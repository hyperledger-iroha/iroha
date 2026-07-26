use iroha_ffi::FfiType;

#[derive(FfiType)]
#[ffi_type(unsafe {robust})]
#[repr(C)]
pub struct ReprC(u32);

fn main() {}
