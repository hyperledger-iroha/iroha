#![allow(clippy::manual_div_ceil)]
use iroha_schema::IntoSchema;
use norito::{CompressionConfig, NoritoDeserialize, NoritoSerialize, decode_from_bytes};

#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct Foo {
    a: u32,
    b: String,
}

#[test]
fn decode_helper_uncompressed() {
    let v = Foo {
        a: 7,
        b: "x".into(),
    };
    let bytes = norito::to_bytes(&v).expect("encode");
    let got: Foo = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(v, got);
}

#[test]
fn decode_helper_compressed() {
    let v = Foo {
        a: 42,
        b: "zz".into(),
    };
    let cfg = CompressionConfig { level: 1 };
    let bytes = norito::to_compressed_bytes(&v, Some(cfg)).expect("encode");
    let got: Foo = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(v, got);
}

#[test]
fn decode_helper_reader() {
    use std::io::Cursor;
    let v = Foo {
        a: 99,
        b: "rd".into(),
    };
    let bytes = norito::to_bytes(&v).expect("encode");
    let got: Foo = norito::decode_from_reader(Cursor::new(bytes)).expect("decode");
    assert_eq!(v, got);
}
