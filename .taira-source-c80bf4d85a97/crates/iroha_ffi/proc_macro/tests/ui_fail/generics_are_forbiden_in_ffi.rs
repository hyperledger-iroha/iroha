use getset::Getters;
use iroha_ffi::{FfiType, ffi_export};

#[ffi_export]
pub fn freestanding<T>(v: T) -> T {
    v
}

#[ffi_export]
#[derive(Getters, FfiType)]
#[getset(get = "pub")]
pub struct FfiStruct<T> {
    inner: T,
}

fn main() {}
