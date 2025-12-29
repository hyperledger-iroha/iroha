use iroha_schema::IntoSchema;
use norito::{NoritoDeserialize, NoritoSerialize, codec::Encode, decode_bare_from_bytes};

#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct Pair(u32, String);

#[test]
fn bare_slice_no_copy_roundtrip() {
    let v = Pair(7, "hi".into());
    // Encode bare payload (headerless)
    let bare = v.encode();
    // Bare decode helper should succeed and match the value
    let out: Pair = decode_bare_from_bytes(&bare).expect("decode bytes");
    assert_eq!(v, out);
}
