//! Actual original compiler identities for outgoing-operation and redemption frames.
//!
//! Captured records are metadata only. Current frame roundtrips exercise shape and codec
//! boundaries using existing structural fixtures; they do not establish proof or hardware authority.

use norito::{NoritoSchema, core::NoritoDeserialize, core::NoritoSerialize, json::Value};

fn observed_identity<T: NoritoSchema>(identifier: &str) {
    let rows: Vec<Value> = norito::json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/core/outgoing_frame_identity_observations.v1.json"
    )))
    .expect("immutable original compiler observations");
    assert_eq!(rows.len(), 12);
    let mut directions = std::collections::BTreeSet::new();
    for row in rows
        .iter()
        .filter(|row| row.get("identifier").and_then(Value::as_str) == Some(identifier))
    {
        let field = |key| row.get(key).and_then(Value::as_str).expect("captured text");
        assert!(directions.insert(field("direction")));
        assert_eq!(T::nominal_name(), field("nominal"));
        assert_eq!(std::any::type_name::<T>(), field("nominal"));
        assert_eq!(T::frame_name(), field("root_hint"));
        let hash = norito::schema::identity::frame_hash::<T>();
        let observed: Vec<u8> = field("schema_hash")
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(hash.as_slice(), observed);
    }
    assert_eq!(
        directions,
        ["deserialize", "serialize"].into_iter().collect()
    );
}

fn canonical_roundtrip<T>(value: &T)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let expected = norito::encode_canonical(value).expect("canonical frame");
    let view = norito::core::from_bytes_view(&expected).expect("valid frame and checksum");
    assert_eq!(view.schema(), norito::schema::identity::frame_hash::<T>());
    let ambient = norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _ambient = norito::core::DecodeFlagsGuard::enter(ambient);
    assert_eq!(norito::encode_canonical(value).unwrap(), expected);
    let decoded: T = norito::decode_canonical(&expected).expect("canonical typed roundtrip");
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), expected);
    assert_eq!(norito::core::get_decode_flags(), ambient);

    let mut trailing = expected.clone();
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    let mut wrong_root = expected.clone();
    wrong_root[6] ^= 1;
    assert!(norito::decode_canonical::<T>(&wrong_root).is_err());
    let mut corrupt_payload = expected.clone();
    *corrupt_payload.last_mut().unwrap() ^= 1;
    assert!(norito::decode_canonical::<T>(&corrupt_payload).is_err());
    assert!(norito::decode_canonical::<T>(&expected[..expected.len() - 1]).is_err());
    assert_eq!(norito::core::get_decode_flags(), ambient);
}

fn shapes<T>(value: &T)
where
    T: Clone + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    canonical_roundtrip(value);
    canonical_roundtrip(&Option::<T>::None);
    canonical_roundtrip(&Some(value.clone()));
    canonical_roundtrip(&Vec::<T>::new());
    canonical_roundtrip(&vec![value.clone(), value.clone()]);
}

/// Check the observed root identity and current complete container frames.
pub(in super::super) fn check<T>(identifier: &str, value: &T)
where
    T: Clone + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    observed_identity::<T>(identifier);
    shapes(value);
}
