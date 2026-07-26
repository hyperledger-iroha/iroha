//! Structs without an explicit representation are intentionally opaque.

use iroha_ffi::{
    FfiType,
    ir::{Ir, Opaque},
};

#[derive(Clone, Copy, FfiType)]
pub struct MissingRepr {
    field: u32,
}

fn assert_opaque<T>()
where
    T: Ir<Type = Opaque> + FfiType,
{
}

fn main() {
    assert_opaque::<MissingRepr>();
}
