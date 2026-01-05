//! Regression tests for `ArchiveView` validation behaviour.

use iroha_schema::IntoSchema;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{DecodeFromSlice, Error, from_bytes_view, to_bytes},
};

#[derive(IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct DummyPayload(Vec<u8>);

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
fn archive_view_rejects_schema_mismatch() {
    let bytes = to_bytes(&42u32).expect("serialize payload");
    let view = from_bytes_view(&bytes).expect("view");
    let err = view
        .decode::<i32>()
        .expect_err("schema mismatch must error");
    assert!(matches!(err, Error::SchemaMismatch));
}
