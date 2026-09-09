//! Shared assertions for independently captured public model frames.

use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{DecodeAll as _, Encode as _},
    core::Header,
    json::{JsonDeserialize, JsonSerialize, Value},
};
use std::fmt::Debug;

/// Check a declared frame using its public value invariants after every decode.
pub(crate) fn record_checked<T>(rows: &mut Vec<Value>, case: &str, value: &T, verify: impl Fn(&T))
where
    T: norito::NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let frame = norito::encode_canonical(value).expect("encode canonical capture frame");
    let header = Header::read(frame.as_slice()).expect("read capture header");
    let identity_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(header.schema, identity_hash, "{case}: frame identity");
    assert_eq!(norito::canonical_frame_len(value).unwrap(), frame.len());

    let decoded: T = norito::decode_canonical(&frame).expect("decode canonical capture frame");
    verify(&decoded);
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
    let view = norito::core::from_bytes_view(&frame).expect("validate capture archive");
    let archived: T = view
        .decode_exact_with(norito::core::decode_field_canonical::<T>)
        .expect("decode typed capture archive");
    verify(&archived);
    assert_eq!(
        archived.encode(),
        value.encode(),
        "{case}: exact archive bytes"
    );
    assert_eq!(view.schema(), header.schema);
    assert_eq!(view.flags(), header.flags);

    let bare = value.encode();
    let mut cursor = bare.as_slice();
    let decoded_bare = T::decode_all(&mut cursor).expect("decode complete bare capture");
    verify(&decoded_bare);
    assert_eq!(decoded_bare.encode(), bare, "{case}: bare re-encoding");
    assert!(cursor.is_empty());
    assert_eq!(view.as_bytes(), bare, "{case}: canonical payload bytes");
    assert_eq!(u64::try_from(bare.len()).unwrap(), header.length);
    let payload_start = frame.len().checked_sub(bare.len()).unwrap();
    assert!(payload_start >= Header::SIZE);
    // The typed archive decoder above validates the owner's actual alignment.
    // Record its observed zero padding instead of assuming every type is unpadded.
    assert!(
        frame[Header::SIZE..payload_start]
            .iter()
            .all(|byte| *byte == 0)
    );

    let mut wrong_schema = frame.clone();
    wrong_schema[6] ^= 1;
    assert!(norito::decode_canonical::<T>(&wrong_schema).is_err());
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    let mut trailing = frame.clone();
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    let recovered: T =
        norito::decode_canonical(&frame).expect("valid frame after rejected controls");
    verify(&recovered);
    assert_eq!(norito::encode_canonical(&recovered).unwrap(), frame);

    rows.push(norito::json!({
        "case": case,
        // Compare the declared nominal identity with the compiler-observed capture.
        // Physical source relocation must not change this recorded wire contract.
        "actual_type_name": (T::nominal_name()),
        "serialize_schema_hash_hex": (hex::encode(identity_hash)),
        "deserialize_schema_hash_hex": (hex::encode(identity_hash)),
        "header_schema_hash_hex": (hex::encode(header.schema)),
        "header_flags": (header.flags),
        "payload_length": (header.length),
        "alignment_padding_length": (payload_start - Header::SIZE),
        "bare_hex": (hex::encode(&bare)),
        "frame_hex": (hex::encode(&frame)),
    }));
}

/// Record a frame owner whose complete value supports direct equality checks.
pub(crate) fn record_binary<T>(rows: &mut Vec<Value>, case: &str, value: &T)
where
    T: norito::NoritoSchema + Debug + PartialEq + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    record_checked(rows, case, value, |decoded| {
        assert_eq!(decoded, value, "{case}: complete decoded value");
    });
}

/// Record a frame owner whose public contract also includes canonical JSON.
pub(crate) fn record<T>(rows: &mut Vec<Value>, case: &str, value: &T)
where
    T: norito::NoritoSchema
        + Debug
        + PartialEq
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + JsonSerialize
        + JsonDeserialize,
{
    record_binary(rows, case, value);
    let json = norito::json::to_json(value).expect("encode captured JSON");
    let decoded: T = norito::json::from_json(&json).expect("decode captured JSON");
    assert_eq!(&decoded, value, "{case}: complete JSON value");
    assert_eq!(
        decoded.encode(),
        value.encode(),
        "{case}: JSON preserves payload"
    );
    rows.last_mut()
        .expect("record_binary appended one row")
        .as_object_mut()
        .expect("captured row is an object")
        .insert("json".into(), Value::String(json));
}
