//! Schema registry for canonical Norito payload examples.
//!
//! The encoded bytes in these doctests double as executable documentation:
//! any layout changes surface immediately through the doctest failure, making
//! it clear that downstream fixtures need to be regenerated.

use norito_derive::{NoritoDeserialize, NoritoSerialize};

/// Minimal sample payload used by the docs and CLI examples.
///
/// ```rust
/// use std::convert::TryInto;
///
/// use norito::{
///     NoritoDeserialize, NoritoSerialize,
///     core::{Compression, Header, MAGIC, VERSION_MAJOR, VERSION_MINOR},
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
///     &<SamplePayload as NoritoSerialize>::schema_hash()
/// );
/// assert_eq!(header_bytes[22], Compression::None as u8);
/// let length = u64::from_le_bytes(header_bytes[23..31].try_into().unwrap());
/// assert_eq!(length as usize, body.len());
/// let checksum = u64::from_le_bytes(header_bytes[31..39].try_into().unwrap());
/// assert_eq!(checksum, crc64_fallback(body));
/// let flags = header_bytes[39];
/// assert_eq!(flags, 0);
///
/// const EXPECTED_BODY: &[u8] = &[
///     // Archived `String` header: length and spare capacity (`Vec` layout)
///     0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
///     // `version` (u32) and `enabled` (bool stored as u32)
///     0x07, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
///     // Archived string pointer metadata
///     0x00, 0x00, 0x00, 0x00, 0x01, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
///     0x00, // UTF-8 bytes for "demo" followed by padding
///     0x00, 0x00, 0x00, 0x00, 0x00, 0x64, 0x65, 0x6D, 0x6F, 0x2C, 0x00, 0x00, 0x00, 0x00, 0x00,
///     0x00, // Archived `Vec<u32>` header (len/capacity)
///     0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00,
///     0x00, // Item payload (`1`, `2`, `3`) stored as u32 values
///     0x00, 0x01, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00,
///     0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
/// ];
/// assert_eq!(body, EXPECTED_BODY);
///
/// let archived = from_bytes::<SamplePayload>(&encoded).expect("decode sample payload");
/// let decoded = <SamplePayload as NoritoDeserialize>::deserialize(archived);
/// assert_eq!(decoded, payload);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
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
