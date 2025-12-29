//! Compile-fail test that validates the missing `#[repr(C)]` diagnostic.

use iroha_ffi::FfiType;

#[derive(Clone, Copy, FfiType)]
pub struct MissingRepr {
    field: u32,
}

fn main() {}
