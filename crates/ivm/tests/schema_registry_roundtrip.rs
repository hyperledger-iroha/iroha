//! SchemaRegistry encode/decode sanity tests

use ivm::schema_registry::{DefaultRegistry, SchemaRegistry};

#[test]
fn order_encode_decode_roundtrip() {
    let reg = DefaultRegistry::new();
    let json = br#"{"qty":10, "side":"buy"}"#;
    let bytes = reg.encode_json("Order", json).expect("encode Order");
    assert!(!bytes.is_empty());
    let back = reg.decode_to_json("Order", &bytes).expect("decode Order");
    assert!(!back.is_empty());
    let s = std::str::from_utf8(&back).unwrap();
    assert!(s.contains("qty") && s.contains("side"));
}
