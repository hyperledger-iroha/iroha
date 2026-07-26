use iroha_ffi::FfiType;

#[derive(FfiType)]
#[ffi_type(unsafe {robust})]
pub struct ImplicitlyOpaque(u32);

fn main() {}
