#![allow(clippy::manual_div_ceil)]
use iroha_schema::IntoSchema;
use norito::core::*;

#[derive(IntoSchema, NoritoSerialize, NoritoDeserialize)]
#[repr(C)]
struct Point {
    x: u32,
    y: bool,
}

#[test]
fn struct_roundtrip() {
    let p = Point { x: 5, y: true };
    let bytes = to_bytes(&p).unwrap();
    let archived = from_bytes::<Point>(&bytes).unwrap();
    let decoded = <Point as NoritoDeserialize>::deserialize(archived);
    assert!(decoded.x == 5 && decoded.y);
}

#[derive(IntoSchema, NoritoSerialize, NoritoDeserialize)]
#[repr(C)]
struct Tuple(u32, bool);

#[test]
fn tuple_roundtrip() {
    let t = Tuple(1, false);
    let bytes = to_bytes(&t).unwrap();
    let archived = from_bytes::<Tuple>(&bytes).unwrap();
    let decoded = <Tuple as NoritoDeserialize>::deserialize(archived);
    assert_eq!(decoded.0, 1);
    assert!(!decoded.1);
}
