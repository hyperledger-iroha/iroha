//! Schema registry for canonical Norito payload examples.
//!
//! The encoded bytes in these doctests double as executable documentation:
//! any layout changes surface immediately through the doctest failure, making
//! it clear that downstream fixtures need to be regenerated.
pub mod identity;

use norito_derive::{NoritoDeserialize, NoritoSerialize};
/// Minimal sample payload used by the docs and CLI examples.
///
/// ```rust
/// use std::convert::TryInto;
///
/// use norito::{
///     DeserializePayload,
///     core::{Compression, Header, MAGIC, VERSION_MAJOR, VERSION_MINOR, header_flags},
///     crc64_fallback, from_bytes,
///     schema::SamplePayload,
///     to_bytes,
/// };
///
/// let payload = SamplePayload {
///     version: 7,
///     enabled: true,
///     label: "demo".into(),
///     items: vec![1, 2, 3],
/// };
///
/// let encoded = to_bytes(&payload).expect("encode sample payload");
/// let (header_bytes, body) = encoded.split_at(Header::SIZE);
///
/// assert_eq!(&header_bytes[0..4], &MAGIC);
/// assert_eq!(header_bytes[4], VERSION_MAJOR);
/// assert_eq!(header_bytes[5], VERSION_MINOR);
/// assert_eq!(
///     &header_bytes[6..22],
///     &norito::schema::identity::frame_hash::<SamplePayload>()
/// );
/// assert_eq!(header_bytes[22], Compression::None as u8);
/// let length = u64::from_le_bytes(header_bytes[23..31].try_into().unwrap());
/// assert_eq!(length as usize, body.len());
/// let checksum = u64::from_le_bytes(header_bytes[31..39].try_into().unwrap());
/// assert_eq!(checksum, crc64_fallback(body));
/// let flags = header_bytes[39];
/// assert_eq!(flags, header_flags::COMPACT_LEN);
///
/// const EXPECTED_BODY: &[u8] = &[
///     // Compact-length v1 default layout for the sample payload.
///     0x04, 0x07, 0x00, 0x00, 0x00, 0x01, 0x01, 0x05, 0x04, 0x64, 0x65, 0x6D, 0x6F, 0x17, 0x03,
///     0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x01, 0x00, 0x00, 0x00, 0x04, 0x02, 0x00,
///     0x00, 0x00, 0x04, 0x03, 0x00, 0x00, 0x00,
/// ];
/// assert_eq!(body, EXPECTED_BODY);
///
/// let archived = from_bytes::<SamplePayload>(&encoded).expect("decode sample payload");
/// let decoded = <SamplePayload as DeserializePayload>::deserialize(archived);
/// assert_eq!(decoded, payload);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, crate::NoritoSchema, NoritoSerialize, NoritoDeserialize)]
#[norito_schema(name = "norito::schema::SamplePayload")]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct SamplePayload {
    /// Payload schema version.
    pub version: u32,
    /// Feature toggle flag copied by higher-level docs.
    pub enabled: bool,
    /// Human-readable label.
    pub label: String,
    /// Sample numeric items.
    pub items: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::SamplePayload;

    #[test]
    fn sample_frame_matches_captured_protocol_bytes() {
        let value = SamplePayload {
            version: 7,
            enabled: true,
            label: "demo".into(),
            items: vec![1, 2, 3],
        };
        let expected = include_bytes!("../../tests/fixtures/sample_payload_frame.norito");
        assert_eq!(
            crate::encode_canonical(&value).unwrap(),
            expected.as_slice()
        );
        assert_eq!(
            crate::schema::identity::frame_hash::<SamplePayload>(),
            expected[6..22]
        );
        assert_eq!(
            crate::decode_from_bytes::<SamplePayload>(expected).unwrap(),
            value
        );
    }
}
