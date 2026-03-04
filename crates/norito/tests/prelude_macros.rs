#![allow(clippy::manual_div_ceil)]
use norito::{
    codec::{Decode as _, Encode as _},
    prelude::*,
};

#[derive(Encode, Decode, PartialEq, Debug, iroha_schema::IntoSchema)]
struct ViaPrelude(u64);

#[test]
fn prelude_exports_macros() {
    let value = ViaPrelude(42);
    let bytes = value.encode();
    let decoded = ViaPrelude::decode(&mut &bytes[..]).expect("decode");
    assert_eq!(value, decoded);
}
