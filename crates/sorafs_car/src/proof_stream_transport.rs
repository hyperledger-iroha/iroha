//! Helpers for decoding proof stream responses delivered over compressed transports.
//!
//! Gateways may apply HTTP content encodings (e.g., `gzip`, `zstd`) when
//! streaming NDJSON proof items. This module centralises the decompression
//! logic so callers (CLI, orchestrator, tests) can share a hardened
//! implementation.

use std::{
    fmt,
    io::{BufRead, Cursor, Read},
    str,
};

use flate2::read::{DeflateDecoder, GzDecoder};
use thiserror::Error;

use crate::proof_stream::ProofStreamItem;

/// Maximum compressed or decoded bytes accepted for one proof-stream response.
pub const MAX_PROOF_STREAM_TRANSPORT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum bytes accepted for one proof-stream NDJSON record, excluding newline.
pub const MAX_PROOF_STREAM_LINE_BYTES: usize = 256 * 1024;
/// Maximum proof records accepted in one response.
pub const MAX_PROOF_STREAM_ITEMS: usize = 1_024;

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
        match label {
            "identity" => Ok(Self::Identity),
            "gzip" => Ok(Self::Gzip),
            "deflate" => Ok(Self::Deflate),
            "zstd" => Ok(Self::Zstd),
            other => Err(ProofStreamTransportError::UnsupportedEncoding {
                encoding: other.to_string(),
            }),
        }
    }
}

/// Errors surfaced while decoding proof stream transport payloads.
#[derive(Debug, Error)]
pub enum ProofStreamTransportError {
    /// Reading the decoded transport failed.
    #[error("failed to read proof stream transport: {source}")]
    Read {
        /// Underlying IO failure.
        #[source]
        source: std::io::Error,
    },
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
    /// Compressed or decoded payload exceeded the fixed transport bound.
    #[error("proof stream payload exceeds the {limit}-byte transport bound")]
    PayloadTooLarge {
        /// Fixed maximum accepted byte length.
        limit: usize,
    },
    /// One NDJSON item exceeded the fixed line bound.
    #[error("proof stream item at line {line} exceeds the {limit}-byte line bound")]
    ItemTooLarge {
        /// One-based line number.
        line: usize,
        /// Fixed maximum accepted line length.
        limit: usize,
    },
    /// Stream contained more items than the request protocol permits.
    #[error("proof stream contains more than {limit} items")]
    TooManyItems {
        /// Fixed maximum accepted item count.
        limit: usize,
    },
    /// A line used noncanonical surrounding whitespace.
    #[error("proof stream item at line {line} contains noncanonical surrounding whitespace")]
    NonCanonicalLine {
        /// One-based line number.
        line: usize,
    },
    /// Canonical NDJSON streams must terminate the final item with a newline.
    #[error("proof stream payload is missing its final newline")]
    MissingFinalNewline,
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
    fn new(line: usize, message: &str) -> Self {
        Self {
            message: format!("failed to decode proof stream item at line {line}: {message}"),
        }
    }
}

impl fmt::Display for ItemDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ItemDecodeError {}

/// Incremental, resource-bounded decoder for canonical proof-stream NDJSON.
///
/// The reader consumes an already decoded transport. It rejects oversized
/// responses and records before allocating beyond their fixed bounds, requires
/// exactly one non-empty canonical JSON record per newline, and never includes
/// response bytes in its errors.
pub struct ProofStreamNdjsonReader<R> {
    inner: R,
    total_bytes: usize,
    item_count: usize,
    line_number: usize,
    finished: bool,
}

impl<R: BufRead> ProofStreamNdjsonReader<R> {
    /// Wrap an already decoded proof-stream response.
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            total_bytes: 0,
            item_count: 0,
            line_number: 0,
            finished: false,
        }
    }

    fn read_next(&mut self) -> Result<Option<ProofStreamItem>, ProofStreamTransportError> {
        let mut line_bytes = Vec::new();
        let next_line = self.line_number + 1;

        loop {
            let (consumed, found_newline) = {
                let available = self
                    .inner
                    .fill_buf()
                    .map_err(|source| ProofStreamTransportError::Read { source })?;
                if available.is_empty() {
                    self.finished = true;
                    if line_bytes.is_empty() {
                        return Ok(None);
                    }
                    return Err(ProofStreamTransportError::MissingFinalNewline);
                }

                let newline = available.iter().position(|byte| *byte == b'\n');
                let content_len = newline.unwrap_or(available.len());
                let consumed = newline.map_or(available.len(), |index| index + 1);
                let prospective_total = self.total_bytes.checked_add(consumed).ok_or(
                    ProofStreamTransportError::PayloadTooLarge {
                        limit: MAX_PROOF_STREAM_TRANSPORT_BYTES,
                    },
                )?;
                if prospective_total > MAX_PROOF_STREAM_TRANSPORT_BYTES {
                    return Err(ProofStreamTransportError::PayloadTooLarge {
                        limit: MAX_PROOF_STREAM_TRANSPORT_BYTES,
                    });
                }
                let prospective_line = line_bytes.len().checked_add(content_len).ok_or(
                    ProofStreamTransportError::ItemTooLarge {
                        line: next_line,
                        limit: MAX_PROOF_STREAM_LINE_BYTES,
                    },
                )?;
                if prospective_line > MAX_PROOF_STREAM_LINE_BYTES {
                    return Err(ProofStreamTransportError::ItemTooLarge {
                        line: next_line,
                        limit: MAX_PROOF_STREAM_LINE_BYTES,
                    });
                }
                line_bytes.extend_from_slice(&available[..content_len]);
                (consumed, newline.is_some())
            };

            self.total_bytes += consumed;
            self.inner.consume(consumed);

            if !found_newline {
                continue;
            }
            self.line_number = next_line;
            if line_bytes.is_empty() {
                return Err(ProofStreamTransportError::NonCanonicalLine { line: next_line });
            }
            if self.item_count >= MAX_PROOF_STREAM_ITEMS {
                return Err(ProofStreamTransportError::TooManyItems {
                    limit: MAX_PROOF_STREAM_ITEMS,
                });
            }

            let text = str::from_utf8(&line_bytes)
                .map_err(|source| ProofStreamTransportError::InvalidUtf8 { source })?;
            let trimmed = text.trim_matches(|ch: char| ch.is_ascii_whitespace());
            if trimmed != text {
                return Err(ProofStreamTransportError::NonCanonicalLine { line: next_line });
            }
            let item = ProofStreamItem::from_ndjson(text.as_bytes()).map_err(|message| {
                ProofStreamTransportError::ItemDecode {
                    source: ItemDecodeError::new(next_line, &message),
                }
            })?;
            self.item_count += 1;
            return Ok(Some(item));
        }
    }
}

impl<R: BufRead> Iterator for ProofStreamNdjsonReader<R> {
    type Item = Result<ProofStreamItem, ProofStreamTransportError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.read_next() {
            Ok(Some(item)) => Some(Ok(item)),
            Ok(None) => None,
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}

/// Decompress a proof stream payload according to the advertised encoding.
pub fn decode_transport_payload(
    encoding: Option<&str>,
    payload: &[u8],
) -> Result<Vec<u8>, ProofStreamTransportError> {
    if payload.len() > MAX_PROOF_STREAM_TRANSPORT_BYTES {
        return Err(ProofStreamTransportError::PayloadTooLarge {
            limit: MAX_PROOF_STREAM_TRANSPORT_BYTES,
        });
    }
    match encoding {
        None => Ok(payload.to_vec()),
        Some(label) => match ContentEncoding::parse(label)? {
            ContentEncoding::Identity => Ok(payload.to_vec()),
            ContentEncoding::Gzip => {
                let decoder = GzDecoder::new(payload);
                let mut buffer = Vec::with_capacity(payload.len());
                decoder
                    .take((MAX_PROOF_STREAM_TRANSPORT_BYTES + 1) as u64)
                    .read_to_end(&mut buffer)
                    .map_err(|source| ProofStreamTransportError::Gzip { source })?;
                checked_decoded_payload(buffer)
            }
            ContentEncoding::Deflate => {
                let decoder = DeflateDecoder::new(payload);
                let mut buffer = Vec::with_capacity(payload.len());
                decoder
                    .take((MAX_PROOF_STREAM_TRANSPORT_BYTES + 1) as u64)
                    .read_to_end(&mut buffer)
                    .map_err(|source| ProofStreamTransportError::Deflate { source })?;
                checked_decoded_payload(buffer)
            }
            ContentEncoding::Zstd => {
                let decoder = zstd::stream::read::Decoder::new(payload).map_err(|source| {
                    ProofStreamTransportError::Zstd {
                        message: source.to_string(),
                    }
                })?;
                let mut buffer = Vec::with_capacity(payload.len());
                decoder
                    .take((MAX_PROOF_STREAM_TRANSPORT_BYTES + 1) as u64)
                    .read_to_end(&mut buffer)
                    .map_err(|source| ProofStreamTransportError::Zstd {
                        message: source.to_string(),
                    })?;
                checked_decoded_payload(buffer)
            }
        },
    }
}

fn checked_decoded_payload(payload: Vec<u8>) -> Result<Vec<u8>, ProofStreamTransportError> {
    if payload.len() > MAX_PROOF_STREAM_TRANSPORT_BYTES {
        return Err(ProofStreamTransportError::PayloadTooLarge {
            limit: MAX_PROOF_STREAM_TRANSPORT_BYTES,
        });
    }
    Ok(payload)
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
    ProofStreamNdjsonReader::new(Cursor::new(decompressed)).collect()
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
                manifest_digest_hex: "aa".repeat(32),
                provider_id_hex: "bb".repeat(32),
                challenge_id_hex: None,
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
                trace_id: Some("ee".repeat(16)),
                por_proof: None,
                potr_receipt: None,
                recorded_at_ms: Some(1_701_000_000),
            },
            ProofStreamItem {
                manifest_digest_hex: "cc".repeat(32),
                provider_id_hex: "dd".repeat(32),
                challenge_id_hex: Some("ef".repeat(32)),
                proof_kind: ProofKind::Pdp,
                status: VerificationStatus::Pending,
                failure_reason: None,
                latency_ms: None,
                deadline_ms: None,
                sample_index: None,
                chunk_index: None,
                segment_index: None,
                leaf_index: None,
                tier: Some(crate::proof_stream::ProofTier::Hot),
                trace_id: None,
                por_proof: None,
                potr_receipt: None,
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

    #[test]
    fn content_encoding_rejects_retired_aliases_and_normalization() {
        for invalid in ["", "x-gzip", "zst", "GZIP", " gzip", "gzip "] {
            assert!(
                ContentEncoding::parse(invalid).is_err(),
                "accepted noncanonical content encoding `{invalid}`"
            );
        }
    }

    #[test]
    fn decoded_compression_bomb_is_rejected_at_the_fixed_bound() {
        let expanded = vec![b'x'; MAX_PROOF_STREAM_TRANSPORT_BYTES + 1];
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&expanded).expect("write bomb payload");
        let compressed = encoder.finish().expect("finish bomb payload");
        assert!(compressed.len() < MAX_PROOF_STREAM_TRANSPORT_BYTES);

        let error = decode_transport_payload(Some("gzip"), &compressed)
            .expect_err("decoded transport bomb must be rejected");
        assert!(matches!(
            error,
            ProofStreamTransportError::PayloadTooLarge { .. }
        ));
    }

    #[test]
    fn transport_rejects_noncanonical_lines_and_missing_final_newline() {
        let item = encode_items_ndjson(&sample_items()[..1]);
        let without_newline = &item[..item.len() - 1];
        assert!(matches!(
            decode_transport_items(None, without_newline),
            Err(ProofStreamTransportError::MissingFinalNewline)
        ));

        let mut padded = vec![b' '];
        padded.extend_from_slice(&item);
        assert!(matches!(
            decode_transport_items(None, &padded),
            Err(ProofStreamTransportError::NonCanonicalLine { line: 1 })
        ));
    }

    #[test]
    fn transport_rejects_item_and_count_bombs() {
        let oversized_line = vec![b'x'; MAX_PROOF_STREAM_LINE_BYTES + 1];
        let mut oversized_payload = oversized_line;
        oversized_payload.push(b'\n');
        assert!(matches!(
            decode_transport_items(None, &oversized_payload),
            Err(ProofStreamTransportError::ItemTooLarge { line: 1, .. })
        ));

        let one_item = encode_items_ndjson(&sample_items()[..1]);
        let mut too_many = Vec::with_capacity(one_item.len() * (MAX_PROOF_STREAM_ITEMS + 1));
        for _ in 0..=MAX_PROOF_STREAM_ITEMS {
            too_many.extend_from_slice(&one_item);
        }
        assert!(matches!(
            decode_transport_items(None, &too_many),
            Err(ProofStreamTransportError::TooManyItems { .. })
        ));
    }

    #[test]
    fn item_decode_errors_do_not_echo_response_payloads() {
        let payload = b"{\"secret\":\"do-not-log-this\"}\n";
        let error = decode_transport_items(None, payload)
            .expect_err("malformed item must fail")
            .to_string();
        assert!(error.contains("line 1"));
        assert!(!error.contains("do-not-log-this"));
    }
}
