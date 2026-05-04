//! Verify framing bare Norito payloads with explicit header flags roundtrips via `from_bytes`.

use norito::{NoritoDeserialize, NoritoSerialize};

#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct Item(u32, String);

#[test]
fn frame_bare_with_header_flags_roundtrip() {
    let v = vec![
        Item(1, "a".into()),
        Item(2, "bb".into()),
        Item(3, "ccc".into()),
    ];

    let (bare, flags) = norito::codec::encode_with_header_flags(&v);
    let bytes = norito::core::frame_bare_with_header_flags::<Vec<Item>>(&bare, flags)
        .expect("frame header");

    // Zero-copy decode the archived payload via header-aware path
    let archived = norito::core::from_bytes::<Vec<Item>>(&bytes).expect("view");
    let got = <Vec<Item> as NoritoDeserialize>::deserialize(archived);
    assert_eq!(got, v);
}
