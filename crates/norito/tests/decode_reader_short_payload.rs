//! Decode-from-reader coverage for short payloads vs archived size.

use std::io::Cursor;

use iroha_schema::IntoSchema;
use norito::{Archived, NoritoDeserialize, NoritoSerialize};

#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
struct LargePayload {
    bytes: [u8; 256],
    label: String,
}

#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
enum ShortPayloadEnum {
    Unit,
    Large(LargePayload),
}

#[test]
fn decode_from_reader_handles_short_payload() {
    let value = ShortPayloadEnum::Unit;
    let bytes = norito::to_bytes(&value).expect("encode");
    let archived_size = std::mem::size_of::<Archived<ShortPayloadEnum>>();
    let length_offset = 4 + 1 + 1 + 16 + 1;
    let length_bytes: [u8; 8] = bytes[length_offset..length_offset + 8]
        .try_into()
        .expect("header length bytes");
    let payload_len = usize::try_from(u64::from_le_bytes(length_bytes))
        .expect("payload length fits usize");
    assert!(
        payload_len < archived_size,
        "expected payload shorter than archived size (payload_len={payload_len}, archived_size={archived_size})"
    );
    let decoded: ShortPayloadEnum =
        norito::decode_from_reader(Cursor::new(bytes)).expect("decode");
    assert_eq!(decoded, value);
}
