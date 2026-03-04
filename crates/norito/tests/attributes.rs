#![allow(clippy::manual_div_ceil)]
use iroha_schema::IntoSchema;
use norito::core::*;

#[derive(IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct Rename {
    #[norito(rename = "z")]
    x: u32,
}

#[test]
fn rename_roundtrip() {
    let r = Rename { x: 42 };
    let bytes = to_bytes(&r).expect("serialize");
    let archived = from_bytes::<Rename>(&bytes).expect("deserialize");
    let decoded = <Rename as NoritoDeserialize>::deserialize(archived);
    assert_eq!(decoded.x, 42);
}

#[derive(IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct SkipDefault {
    a: u32,
    #[norito(skip)]
    b: u32,
    #[norito(default)]
    c: u32,
}

#[test]
fn skip_and_default() {
    let s = SkipDefault { a: 5, b: 7, c: 9 };
    let bytes = to_bytes(&s).unwrap();
    let archived = from_bytes::<SkipDefault>(&bytes).unwrap();
    let decoded = <SkipDefault as NoritoDeserialize>::deserialize(archived);
    assert_eq!(decoded.a, 5);
    assert_eq!(decoded.b, 0);
    assert_eq!(decoded.c, 9);
}
