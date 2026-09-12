//! Complete version diagnostic frames and bounded canonical slice reconstruction.

use std::fmt::{Debug, Write as _};

use iroha_version::{RawVersioned, UnsupportedVersion};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{DecodeAll as _, Encode as _},
    core as ncore,
    json::Value,
};

fn hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

fn raw_values() -> Vec<RawVersioned> {
    vec![
        RawVersioned::Json(String::new()),
        RawVersioned::Json("{\"version\":\"9\",\"content\":[1,true]}".to_owned()),
        RawVersioned::Json("opaque 日本語\0diagnostic".to_owned()),
        RawVersioned::NoritoBytes(Vec::new()),
        RawVersioned::NoritoBytes(vec![0, 1, 255]),
        RawVersioned::NoritoBytes((0..=255).collect()),
    ]
}

fn record<T>(rows: &mut Vec<Value>, case: &str, value: &T)
where
    T: norito::NoritoSchema + Debug + PartialEq + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let frame = norito::encode_canonical(value).expect("complete canonical frame");
    let bare = value.encode();
    let header = ncore::Header::read(frame.as_slice()).unwrap();
    let frame_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(header.schema, frame_hash);
    let decoded: T = norito::decode_canonical(&frame).unwrap();
    assert_eq!(&decoded, value);
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
    let view = ncore::from_bytes_view(&frame).unwrap();
    let archived = view
        .decode_exact_with(ncore::decode_field_canonical::<T>)
        .unwrap();
    assert_eq!(&archived, value);
    assert_eq!(view.as_bytes(), bare);
    let mut cursor = bare.as_slice();
    assert_eq!(&T::decode_all(&mut cursor).unwrap(), value);
    assert!(cursor.is_empty());
    assert_eq!(u64::try_from(bare.len()).unwrap(), header.length);
    let payload_start = frame.len() - bare.len();
    assert!(payload_start >= ncore::Header::SIZE);
    assert!(
        frame[ncore::Header::SIZE..payload_start]
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(norito::canonical_frame_len(value).unwrap(), frame.len());
    let mut wrong_owner = frame.clone();
    wrong_owner[6] ^= 1;
    assert!(norito::decode_canonical::<T>(&wrong_owner).is_err());
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    let mut trailing = frame.clone();
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    let recovered: T = norito::decode_canonical(&frame).unwrap();
    assert_eq!(&recovered, value);
    rows.push(norito::json!({
        "case": case,
        "actual_type_name": (T::nominal_name()),
        "serialize_schema_hash_hex": (hex(&frame_hash)),
        "deserialize_schema_hash_hex": (hex(&frame_hash)),
        "header_schema_hash_hex": (hex(&header.schema)),
        "header_flags": (header.flags),
        "payload_length": (header.length),
        "alignment_padding_length": (payload_start - ncore::Header::SIZE),
        "bare_hex": (hex(&bare)),
        "frame_hex": (hex(&frame)),
    }));
}

fn frame_rows() -> Vec<Value> {
    let raw = raw_values();
    let mut rows = Vec::new();
    for (index, value) in raw.iter().enumerate() {
        record(&mut rows, &format!("raw-{index}"), value);
        record(
            &mut rows,
            &format!("raw-some-{index}"),
            &Some(value.clone()),
        );
    }
    record(&mut rows, "raw-none", &None::<RawVersioned>);
    record(&mut rows, "raw-empty-vec", &Vec::<RawVersioned>::new());
    record(&mut rows, "raw-all-vec", &raw);
    let unsupported = [
        UnsupportedVersion::new(0, raw[0].clone()),
        UnsupportedVersion::new(1, raw[2].clone()),
        UnsupportedVersion::new(2, raw[3].clone()),
        UnsupportedVersion::new(255, raw[5].clone()),
    ];
    for (index, value) in unsupported.iter().enumerate() {
        record(&mut rows, &format!("unsupported-{index}"), value);
        record(
            &mut rows,
            &format!("unsupported-some-{index}"),
            &Some(value.clone()),
        );
    }
    record(&mut rows, "unsupported-none", &None::<UnsupportedVersion>);
    record(
        &mut rows,
        "unsupported-empty-vec",
        &Vec::<UnsupportedVersion>::new(),
    );
    record(&mut rows, "unsupported-all-vec", &unsupported.to_vec());
    assert_eq!(rows.len(), 26);
    rows
}

#[test]
fn raw_versioned_slice_matches_its_encoder() {
    for value in raw_values() {
        let bytes = value.encode();
        let expected_tag: u32 = match value {
            RawVersioned::Json(_) => 0,
            RawVersioned::NoritoBytes(_) => 1,
        };
        assert_eq!(&bytes[..4], expected_tag.to_le_bytes());
        let decoded = norito::codec::decode_exact_from_slice::<RawVersioned>(&bytes).expect(
            "complete canonical encoder output must reconstruct through the public slice API",
        );
        assert_eq!(decoded, value);
    }
}

#[test]
fn raw_versioned_slice_preserves_advertised_layout() {
    let default = ncore::default_encode_flags();
    for value in raw_values() {
        for flags in [default, default ^ ncore::header_flags::COMPACT_LEN] {
            let frame = {
                let _flags = ncore::DecodeFlagsGuard::enter(flags);
                norito::to_bytes(&value).unwrap()
            };
            let view = ncore::from_bytes_view(&frame).unwrap();
            assert_eq!(view.flags(), flags);
            assert_eq!(
                view.decode_exact_with(ncore::decode_field_canonical::<RawVersioned>)
                    .unwrap(),
                value
            );
            assert_eq!(
                view.decode_exact_with(<RawVersioned as ncore::DecodeFromSlice>::decode_from_slice)
                    .unwrap(),
                value
            );
            assert_eq!(
                ncore::decode_from_bytes::<RawVersioned>(&frame).unwrap(),
                value
            );
            {
                let _flags = ncore::DecodeFlagsGuard::enter(flags);
                let (decoded, used) =
                    <RawVersioned as ncore::DecodeFromSlice>::decode_from_slice(view.as_bytes())
                        .unwrap();
                assert_eq!(used, view.as_bytes().len());
                assert_eq!(decoded, value);
                let mut encoded = Vec::new();
                ncore::serialize_to_buffer(&decoded, &mut encoded).unwrap();
                assert_eq!(encoded, view.as_bytes());
                assert_eq!(ncore::effective_decode_flags(), Some(flags));
            }
            if flags != default {
                assert_ne!(frame, norito::encode_canonical(&value).unwrap());
                assert!(norito::decode_canonical::<RawVersioned>(&frame).is_err());
            }
        }
    }
}

#[test]
fn raw_versioned_slice_rejects_incomplete_unknown_and_trailing_payloads() {
    for value in raw_values() {
        let bytes = value.encode();
        assert_eq!(
            norito::codec::decode_exact_from_slice::<RawVersioned>(&bytes).unwrap(),
            value
        );
        for length in 0..bytes.len() {
            assert!(
                norito::codec::decode_exact_from_slice::<RawVersioned>(&bytes[..length]).is_err()
            );
        }
        let mut unknown = bytes.clone();
        unknown[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(norito::codec::decode_exact_from_slice::<RawVersioned>(&unknown).is_err());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(norito::codec::decode_exact_from_slice::<RawVersioned>(&trailing).is_err());
        assert_eq!(
            norito::codec::decode_exact_from_slice::<RawVersioned>(&bytes).unwrap(),
            value
        );
    }
}

#[test]
fn raw_versioned_slice_retains_resource_errors_and_restores_context() {
    let value = RawVersioned::NoritoBytes(vec![1, 2, 3]);
    let bytes = value.encode();
    let flags = ncore::default_encode_flags();
    let _flags = ncore::DecodeFlagsGuard::enter(flags);
    let limits = ncore::DecodeLimits::new(usize::MAX, bytes.len() - 1, usize::MAX, usize::MAX, 128);
    let error = ncore::with_decode_limits(limits, || {
        <RawVersioned as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
    })
    .unwrap_err();
    assert!(
        matches!(error, ncore::Error::FieldLengthExceeded { length, limit } if length == u64::try_from(bytes.len()).unwrap() && limit == length - 1)
    );
    let limits = ncore::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 0);
    let error = ncore::with_decode_limits(limits, || {
        <RawVersioned as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
    })
    .unwrap_err();
    assert!(matches!(
        error,
        ncore::Error::NestingDepthExceeded { limit: 0, .. }
    ));
    let limits = ncore::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 128);
    let error = ncore::with_decode_limits(limits, || {
        <RawVersioned as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
    })
    .unwrap_err();
    assert!(matches!(
        error,
        ncore::Error::TotalAllocationExceeded { limit: 0, .. }
    ));
    assert_eq!(ncore::effective_decode_flags(), Some(flags));
    let (decoded, used) =
        <RawVersioned as ncore::DecodeFromSlice>::decode_from_slice(&bytes).unwrap();
    assert_eq!(used, bytes.len());
    assert_eq!(decoded, value);
}

#[test]
fn declared_version_owners_preserve_captured_frames() {
    let expected: Vec<Value> =
        norito::json::from_json(include_str!("fixtures/version_identity_frames.json")).unwrap();
    assert_eq!(frame_rows(), expected);
}
