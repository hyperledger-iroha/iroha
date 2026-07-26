use iroha_ffi::FfiType;

#[derive(FfiType)]
#[ffi_type(unsafe {robust})]
#[repr(transparent)]
pub struct RobustTransparent(u32);

fn main() {}
