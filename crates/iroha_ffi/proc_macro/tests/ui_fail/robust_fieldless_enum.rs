use iroha_ffi::FfiType;
#[derive(FfiType)]
#[ffi_type(unsafe {robust})]
#[repr(u8)]
pub enum Fieldless {
    A,
    B,
}
fn main() {}
