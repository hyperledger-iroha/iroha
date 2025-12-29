//! Verify framing bare Norito payloads with a default header roundtrips via `from_bytes`.

use norito::{NoritoDeserialize, NoritoSerialize};

#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct Item(u32, String);

#[test]
fn frame_bare_with_default_header_roundtrip() {
    use norito::codec::Encode as _;

    let v = vec![
        Item(1, "a".into()),
        Item(2, "bb".into()),
        Item(3, "ccc".into()),
    ];

    // Produce a bare, headerless payload via codec::Encode
    let bare = v.encode();

    // Frame with default header flags for type `Vec<Item>`
    let bytes =
        norito::core::frame_bare_with_default_header::<Vec<Item>>(&bare).expect("frame header");

    // Zero-copy decode the archived payload via header-aware path
    let archived = norito::core::from_bytes::<Vec<Item>>(&bytes).expect("view");
    let got = <Vec<Item> as NoritoDeserialize>::deserialize(archived);
    assert_eq!(got, v);
}
