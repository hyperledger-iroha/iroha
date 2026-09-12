//! Captured frame/signature bytes and reference identity checks for P2P owners.
use norito::{
    NoritoSchema,
    core::{NoritoDeserialize, NoritoSerialize},
    json::Value,
};
use std::sync::OnceLock;
fn frames() -> &'static [Value] {
    static RECORDS: OnceLock<Vec<Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        norito::json::from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/p2p/frame_identity_observations.v1.json"
        )))
        .expect("immutable original P2P frame observations")
    })
}
fn preimages() -> &'static [Value] {
    static RECORDS: OnceLock<Vec<Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        norito::json::from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/p2p/signature_preimages.v1.json"
        )))
        .expect("immutable original P2P signature preimages")
    })
}
fn field<'a>(record: &'a Value, key: &str) -> &'a str {
    record
        .get(key)
        .and_then(Value::as_str)
        .expect("captured text field")
}
fn bytes(record: &Value, key: &str) -> Vec<u8> {
    let text = field(record, key);
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let part = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(part, 16).expect("captured hexadecimal bytes")
        })
        .collect()
}
fn frame_record(owner: &str, variant: &str, shape: &str) -> &'static Value {
    let mut matches = frames().iter().filter(|row| {
        field(row, "owner") == owner
            && field(row, "variant") == variant
            && field(row, "shape") == shape
    });
    let value = matches.next().expect("captured frame row");
    assert!(matches.next().is_none(), "frame observation is unique");
    value
}
fn record<T>(owner: &str, variant: &str, shape: &str, value: &T)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let observed = frame_record(owner, variant, shape);
    assert_eq!(
        <T as NoritoSchema>::nominal_name(),
        field(observed, "nominal")
    );
    let hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(hash.as_slice(), bytes(observed, "serialize_hash"));
    assert_eq!(hash.as_slice(), bytes(observed, "deserialize_hash"));
    let expected = bytes(observed, "frame_hex");
    assert_eq!(&expected[6..22], hash.as_slice());
    assert_eq!(
        expected.len() as u64,
        observed.get("frame_len").and_then(Value::as_u64).unwrap()
    );
    let ambient = norito::core::header_flags::COMPACT_LEN;
    let _ambient = norito::core::DecodeFlagsGuard::enter(ambient);
    let frame = norito::encode_canonical(value).expect("current canonical frame");
    assert_eq!(
        frame, expected,
        "whole original frame for {owner}/{variant}/{shape}"
    );
    assert_eq!(norito::core::get_decode_flags(), ambient);
    let decoded: T = norito::decode_canonical(&expected).expect("decode original complete frame");
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), expected);
    assert_eq!(norito::core::get_decode_flags(), ambient);
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    assert_eq!(
        norito::codec::Encode::encode(value),
        bytes(observed, "bare_hex")
    );
    let mut trailing = expected.clone();
    trailing.push(0);
    assert!(
        norito::decode_canonical::<T>(&trailing).is_err(),
        "canonical boundary rejects suffix"
    );
    let mut wrong_root = expected.clone();
    wrong_root[6] ^= 1;
    assert!(
        norito::decode_canonical::<T>(&wrong_root).is_err(),
        "frame root is exact"
    );
    assert!(
        norito::decode_canonical::<T>(&expected[..expected.len() - 1]).is_err(),
        "truncated frame rejected"
    );
}
pub(crate) fn shapes<T>(owner: &str, variant: &str, value: &T)
where
    T: Clone + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    record(owner, variant, "root", value);
    record(owner, variant, "option_none", &Option::<T>::None);
    record(owner, variant, "option_some", &Some(value.clone()));
    record(owner, variant, "vec_empty", &Vec::<T>::new());
    record(
        owner,
        variant,
        "vec_two",
        &vec![value.clone(), value.clone()],
    );
}
pub(crate) fn preimage(owner: &str, variant: &str, signed: &[u8]) {
    let mut matches = preimages()
        .iter()
        .filter(|row| field(row, "owner") == owner && field(row, "variant") == variant);
    let value = matches.next().expect("captured signature preimage");
    assert!(matches.next().is_none(), "signature observation is unique");
    assert_eq!(signed, bytes(value, "signed_bytes_hex"));
}

/// Check both typed codec contracts against an independently hashed identity vector.
pub(crate) fn test_payload_identity<T>(expected_nominal: &str)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    static RECORDS: OnceLock<Vec<Value>> = OnceLock::new();
    let records = RECORDS.get_or_init(|| {
        norito::json::from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/p2p/payload_identities.v1.json"
        )))
        .expect("reference P2P test-payload identities")
    });
    assert_eq!(T::nominal_name(), expected_nominal);
    let rows: Vec<_> = records
        .iter()
        .filter(|row| field(row, "nominal") == expected_nominal)
        .collect();
    assert_eq!(rows.len(), 1, "each reference identity is unique");
    assert_eq!(T::frame_name(), expected_nominal);
    assert_eq!(
        norito::schema::identity::frame_hash::<T>().as_slice(),
        bytes(rows[0], "schema_hash")
    );
}

/// Check faulty test codecs without invoking their deliberately rejected serializers.
pub(crate) fn manual_test_payload_identity<T: NoritoSchema>(expected_nominal: &str) {
    static RECORDS: OnceLock<Vec<Value>> = OnceLock::new();
    let records = RECORDS.get_or_init(|| {
        norito::json::from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/p2p/manual_test_payload_identities.v1.json"
        )))
        .expect("immutable original faulty P2P test-codec identities")
    });
    assert_eq!(T::nominal_name(), expected_nominal);
    let rows: Vec<_> = records
        .iter()
        .filter(|row| field(row, "nominal") == expected_nominal)
        .collect();
    assert_eq!(rows.len(), 1, "manual owner observation is unique");
    let row = rows[0];
    assert_eq!(T::frame_name(), field(row, "nominal"));
    let hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(hash.as_slice(), bytes(row, "serialize_hash"));
    assert_eq!(hash.as_slice(), bytes(row, "deserialize_hash"));
}
