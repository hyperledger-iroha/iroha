//! Ensures the FFI implementation helper rejects ignored arguments.

struct Example;

#[iroha_derive::ffi_impl_opaque(unexpected)]
impl Example {}

fn main() {}
