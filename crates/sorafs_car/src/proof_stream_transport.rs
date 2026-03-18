//! Helpers for decoding proof stream responses delivered over compressed transports.
//!
//! Gateways may apply HTTP content encodings (e.g., `gzip`, `zstd`) when
//! streaming NDJSON proof items. This module centralises the decompression
//! logic so callers (CLI, orchestrator, tests) can share a hardened
//! implementation.

use std::{fmt, io::Read, str};

use flate2::read::{DeflateDecoder, GzDecoder};
use thiserror::Error;

use crate::proof_stream::ProofStreamItem;

/// Supported HTTP content-encoding values exposed by proof stream transports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentEncoding {
    /// Identity (no compression applied).
    Identity,
    /// `gzip` compression.
    Gzip,
    /// `deflate` compression (zlib wrapper).
    Deflate,
    /// `zstd` compression.
    Zstd,
}

impl ContentEncoding {
    /// Parse a raw `Content-Encoding` header value into a [`ContentEncoding`].
    pub fn parse(label: &str) -> Result<Self, ProofStreamTransportError> {
        match label.trim().to_ascii_lowercase().as_str() {
            "" | "identity" => Ok(Self::Identity),
            "gzip" | "x-gzip" => Ok(Self::Gzip),
            "deflate" => Ok(Self::Deflate),
            "zstd" | "zst" => Ok(Self::Zstd),
            other => Err(ProofStreamTransportError::UnsupportedEncoding {
                encoding: other.to_string(),
            }),
        }
    }
}

/// Errors surfaced while decoding proof stream transport payloads.
#[derive(Debug, Error)]
pub enum ProofStreamTransportError {
    /// Compression scheme advertised by the transport is not supported.
    #[error("unsupported content encoding `{encoding}`")]
    UnsupportedEncoding { encoding: String },
    /// `gzip` payload decompression failed.
    #[error("failed to decompress gzip payload: {source}")]
    Gzip {
        /// Underlying IO error reported by the decoder.
        #[source]
        source: std::io::Error,
    },
    /// `deflate` payload decompression failed.
    #[error("failed to decompress deflate payload: {source}")]
    Deflate {
        /// Underlying IO error reported by the decoder.
        #[source]
        source: std::io::Error,
    },
    /// `zstd` payload decompression failed.
    #[error("failed to decompress zstd payload: {message}")]
    Zstd {
        /// Human-readable decoder error.
        message: String,
    },
    /// Payload did not contain valid UTF-8 text after decompression.
    #[error("proof stream payload contained invalid UTF-8: {source}")]
    InvalidUtf8 {
        /// UTF-8 validation failure.
        #[source]
        source: std::str::Utf8Error,
    },
    /// Individual proof stream item failed to decode.
    #[error("{source}")]
    ItemDecode {
        /// Detailed decoding error (includes context).
        #[source]
        source: ItemDecodeError,
    },
}

/// Wrapper reporting per-item decoding errors with context.
#[derive(Debug)]
pub struct ItemDecodeError {
    message: String,
}

impl ItemDecodeError {
    fn new(line: &str, message: &str) -> Self {
        Self {
            message: format!("failed to decode proof stream item `{line}`: {message}"),
        }
    }
}

impl fmt::Display for ItemDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ItemDecodeError {}

/// Decompress a proof stream payload according to the advertised encoding.
pub fn decode_transport_payload(
    encoding: Option<&str>,
    payload: &[u8],
) -> Result<Vec<u8>, ProofStreamTransportError> {
    match encoding {
        None => Ok(payload.to_vec()),
        Some(label) => match ContentEncoding::parse(label)? {
            ContentEncoding::Identity => Ok(payload.to_vec()),
            ContentEncoding::Gzip => {
                let mut decoder = GzDecoder::new(payload);
                let mut buffer = Vec::with_capacity(payload.len());
                decoder
                    .read_to_end(&mut buffer)
                    .map_err(|source| ProofStreamTransportError::Gzip { source })?;
                Ok(buffer)
            }
            ContentEncoding::Deflate => {
                let mut decoder = DeflateDecoder::new(payload);
                let mut buffer = Vec::with_capacity(payload.len());
                decoder
                    .read_to_end(&mut buffer)
                    .map_err(|source| ProofStreamTransportError::Deflate { source })?;
                Ok(buffer)
            }
            ContentEncoding::Zstd => zstd::stream::decode_all(payload).map_err(|source| {
                ProofStreamTransportError::Zstd {
                    message: source.to_string(),
                }
            }),
        },
    }
}

/// Decode proof stream items from a (possibly compressed) transport payload.
///
/// The payload is decompressed according to `encoding`, split on newline
/// boundaries, and each NDJSON record is parsed into a [`ProofStreamItem`].
pub fn decode_transport_items(
    encoding: Option<&str>,
    payload: &[u8],
) -> Result<Vec<ProofStreamItem>, ProofStreamTransportError> {
    let decompressed = decode_transport_payload(encoding, payload)?;
    let mut items = Vec::new();
    for raw_line in decompressed.split(|byte| *byte == b'\n') {
        let text = str::from_utf8(raw_line)
            .map_err(|source| ProofStreamTransportError::InvalidUtf8 { source })?;
        let trimmed = text.trim_matches(|ch: char| ch.is_ascii_whitespace());
        if trimmed.is_empty() {
            continue;
        }
        let item = ProofStreamItem::from_ndjson(trimmed.as_bytes()).map_err(|message| {
            ProofStreamTransportError::ItemDecode {
                source: ItemDecodeError::new(trimmed, &message),
            }
        })?;
        items.push(item);
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::{
        Compression,
        write::{DeflateEncoder, GzEncoder},
    };

    use super::*;
    use crate::proof_stream::{ProofKind, ProofStreamItem, VerificationStatus};

    fn sample_items() -> Vec<ProofStreamItem> {
        vec![
            ProofStreamItem {
                manifest_digest_hex: Some("abc123".into()),
                provider_id_hex: Some("feedbeef".into()),
                proof_kind: ProofKind::Por,
                status: VerificationStatus::Success,
                failure_reason: None,
                latency_ms: Some(42),
                deadline_ms: None,
                sample_index: Some(3),
                chunk_index: Some(1),
                segment_index: Some(0),
                leaf_index: Some(7),
                tier: None,
                trace_id: Some("trace-1".into()),
                por_proof: None,
                recorded_at_ms: Some(1_701_000_000),
            },
            ProofStreamItem {
                manifest_digest_hex: None,
                provider_id_hex: None,
                proof_kind: ProofKind::Potr,
                status: VerificationStatus::Failure,
                failure_reason: Some("timeout".into()),
                latency_ms: Some(1_200),
                deadline_ms: Some(900),
                sample_index: None,
                chunk_index: None,
                segment_index: None,
                leaf_index: None,
                tier: Some(crate::proof_stream::ProofTier::Hot),
                trace_id: Some("trace-2".into()),
                por_proof: None,
                recorded_at_ms: None,
            },
        ]
    }

    fn encode_items_ndjson(items: &[ProofStreamItem]) -> Vec<u8> {
        let mut buffer = Vec::new();
        for item in items {
            let json = item.to_json();
            let encoded = norito::json::to_vec(&json).expect("encode item");
            buffer.extend_from_slice(&encoded);
            buffer.push(b'\n');
        }
        buffer
    }

    #[test]
    fn identity_encoding_roundtrips() {
        let items = sample_items();
        let payload = encode_items_ndjson(&items);
        let decoded = decode_transport_items(None, &payload).expect("decode identity stream");
        assert_eq!(decoded.len(), items.len());
    }

    #[test]
    fn gzip_encoding_roundtrips() {
        let items = sample_items();
        let payload = encode_items_ndjson(&items);
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&payload).expect("write gzip payload");
        let compressed = encoder.finish().expect("finish gzip payload");
        let decoded =
            decode_transport_items(Some("gzip"), &compressed).expect("decode gzip transport");
        assert_eq!(decoded.len(), items.len());
    }

    #[test]
    fn deflate_encoding_roundtrips() {
        let items = sample_items();
        let payload = encode_items_ndjson(&items);
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&payload).expect("write deflate payload");
        let compressed = encoder.finish().expect("finish deflate payload");
        let decoded =
            decode_transport_items(Some("deflate"), &compressed).expect("decode deflate stream");
        assert_eq!(decoded.len(), items.len());
    }

    #[test]
    fn zstd_encoding_roundtrips() {
        let items = sample_items();
        let payload = encode_items_ndjson(&items);
        let compressed =
            zstd::stream::encode_all(payload.as_slice(), 3).expect("encode zstd payload");
        let decoded =
            decode_transport_items(Some("zstd"), &compressed).expect("decode zstd transport");
        assert_eq!(decoded.len(), items.len());
    }
}
