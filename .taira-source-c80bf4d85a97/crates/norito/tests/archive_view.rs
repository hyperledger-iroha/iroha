//! Regression tests for `ArchiveView` validation behaviour.

use iroha_schema::IntoSchema;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{DecodeFromSlice, Error, from_bytes_view, to_bytes},
};

#[derive(Debug, IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct DummyPayload(Vec<u8>);

impl<'a> DecodeFromSlice<'a> for DummyPayload {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        norito::core::decode_field_canonical::<DummyPayload>(bytes)
    }
}

#[derive(Debug)]
struct TrailingDecoder;

impl<'a> DecodeFromSlice<'a> for TrailingDecoder {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        assert!(
            !bytes.is_empty(),
            "fixture should produce non-empty payload"
        );
        Ok((Self, bytes.len() - 1))
    }
}

fn header_padding_for<T>() -> usize {
    let align = std::mem::align_of::<norito::Archived<T>>();
    if align <= 1 {
        return 0;
    }
    let rem = norito::core::Header::SIZE % align;
    if rem == 0 { 0 } else { align - rem }
}

#[test]
fn archive_view_rejects_trailing_bytes() {
    let payload = DummyPayload(vec![1, 2, 3, 4]);
    let bytes = to_bytes(&payload).expect("serialize payload");
    let view = from_bytes_view(&bytes).expect("view");
    let err = view
        .decode_unchecked::<TrailingDecoder>()
        .expect_err("trailing bytes must error");
    assert!(matches!(err, Error::LengthMismatch));
}

#[test]
fn archive_view_rejects_excess_padding() {
    let payload = DummyPayload(vec![1, 2, 3, 4]);
    let bytes = to_bytes(&payload).expect("serialize payload");
    let insert_at = norito::core::Header::SIZE + header_padding_for::<DummyPayload>();
    let mut mutated = Vec::with_capacity(bytes.len() + 1);
    mutated.extend_from_slice(&bytes[..insert_at]);
    mutated.push(0);
    mutated.extend_from_slice(&bytes[insert_at..]);

    let view = from_bytes_view(&mutated).expect("view");
    let err = view
        .decode::<DummyPayload>()
        .expect_err("excess padding must error");
    assert!(matches!(err, Error::LengthMismatch));
}

#[test]
fn archive_view_rejects_schema_mismatch() {
    let bytes = to_bytes(&42u32).expect("serialize payload");
    let view = from_bytes_view(&bytes).expect("view");
    let err = view
        .decode::<i32>()
        .expect_err("schema mismatch must error");
    assert!(matches!(err, Error::SchemaMismatch));
}
