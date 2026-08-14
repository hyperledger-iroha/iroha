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
use crate::proof_stream::{
    ProofStreamItem, ProofStreamSequenceVerifier, ProofStreamVerificationContext,
};
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
    /// The response ended early or violated its request-bound sample schedule.
    #[error("invalid proof stream sequence: {message}")]
    InvalidSequence {
        /// Payload-free description of the sequence failure.
        message: String,
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
/// exactly one non-empty canonical JSON record per newline, authenticates the
/// request-bound sequence at EOF, and never includes response bytes in its errors.
/// Callers must consume the iterator through EOF before acting on any yielded row.
pub struct ProofStreamNdjsonReader<R> {
    inner: R,
    context: ProofStreamVerificationContext,
    sequence: ProofStreamSequenceVerifier,
    total_bytes: usize,
    item_count: usize,
    line_number: usize,
    finished: bool,
}
impl<R: BufRead> ProofStreamNdjsonReader<R> {
    /// Wrap an already decoded proof-stream response in its exact verification scope.
    pub fn new(inner: R, context: &ProofStreamVerificationContext) -> Self {
        Self {
            inner,
            context: *context,
            sequence: ProofStreamSequenceVerifier::new(context),
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
                    if line_bytes.is_empty() {
                        self.finished = true;
                        return self.sequence.finish().map(|()| None).map_err(|message| {
                            ProofStreamTransportError::InvalidSequence { message }
                        });
                    }
                    self.finished = true;
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
            let request_limit = self.context.item_limit();
            if self.item_count >= request_limit {
                return Err(ProofStreamTransportError::TooManyItems {
                    limit: request_limit,
                });
            }
            let text = str::from_utf8(&line_bytes)
                .map_err(|source| ProofStreamTransportError::InvalidUtf8 { source })?;
            let trimmed = text.trim_matches(|ch: char| ch.is_ascii_whitespace());
            if trimmed != text {
                return Err(ProofStreamTransportError::NonCanonicalLine { line: next_line });
            }
            let item = ProofStreamItem::from_ndjson(text.as_bytes(), &self.context).map_err(
                |message| ProofStreamTransportError::ItemDecode {
                    source: ItemDecodeError::new(next_line, &message),
                },
            )?;
            self.sequence
                .verify_item(&item)
                .map_err(|message| ProofStreamTransportError::InvalidSequence { message })?;
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
    context: &ProofStreamVerificationContext,
) -> Result<Vec<ProofStreamItem>, ProofStreamTransportError> {
    let decompressed = decode_transport_payload(encoding, payload)?;
    ProofStreamNdjsonReader::new(Cursor::new(decompressed), context).collect()
}
#[cfg(test)]
mod tests {
    use std::io::Write;
    use flate2::{
        Compression,
        write::{DeflateEncoder, GzEncoder},
    };
    use super::*;
    use crate::proof_stream::{ProofKind, ProofStreamItem};
    use sorafs_manifest::ProofStreamRequestV1;
    fn sample_por_fixture(
        request: &ProofStreamRequestV1,
    ) -> (Vec<(usize, crate::PorProof)>, [u8; 32]) {
        let payload = (0..(crate::POR_LEAF_SIZE * 4 + 17))
            .map(|value| u8::try_from(value % 251).expect("fixture byte"))
            .collect::<Vec<_>>();
        let mut store = crate::ChunkStore::new();
        store
            .ingest_bytes(&payload)
            .expect("ingest canonical PoR transport fixture");
        let trusted_root = *store.por_tree().root();
        let seed = crate::proof_stream::por_request_sample_seed_v1(request, &trusted_root)
            .expect("derive canonical request-bound PoR seed");
        let samples = store
            .sample_leaves(
                usize::try_from(
                    request
                        .sample_count
                        .expect("PoR transport request has a sample count"),
                )
                .expect("u32 sample count fits usize"),
                seed,
                &payload,
            )
            .expect("sample canonical PoR transport fixture");
        (samples, trusted_root)
    }
    fn por_request(sample_count: u32) -> ProofStreamRequestV1 {
        ProofStreamRequestV1 {
            manifest_digest: [0xaa; 32],
            provider_id: [0xbb; 32],
            proof_kind: ProofKind::Por,
            challenge_id: None,
            sample_count: Some(sample_count),
            deadline_ms: None,
            sample_seed: Some(7),
            expected_finalized_height: Some(17),
            expected_finalized_block_hash: Some([0x66; 32]),
            nonce: [0x04; 16],
            orchestrator_job_id: None,
            tier: None,
        }
    }
    fn sample_items_for(
        request: ProofStreamRequestV1,
    ) -> (ProofStreamVerificationContext, Vec<ProofStreamItem>) {
        let (samples, trusted_root) = sample_por_fixture(&request);
        let context = ProofStreamVerificationContext::new(request, Some(trusted_root))
            .expect("canonical PoR transport verification context");
        let items = samples
            .into_iter()
            .map(|(flat_index, por_proof)| {
                let mut por = crate::por_json::sample_to_map(flat_index, &por_proof);
                por.insert(
                    "request_digest_hex".into(),
                    norito::json::Value::from(hex::encode(context.request_digest())),
                );
                por.insert(
                    "manifest_digest_hex".into(),
                    norito::json::Value::from("aa".repeat(32)),
                );
                por.insert(
                    "provider_id_hex".into(),
                    norito::json::Value::from("bb".repeat(32)),
                );
                por.insert("proof_kind".into(), norito::json::Value::from("por"));
                por.insert("result".into(), norito::json::Value::from("success"));
                por.insert("latency_ms".into(), norito::json::Value::from(42_u64));
                por.insert(
                    "finalized_block_height".into(),
                    norito::json::Value::from(17_u64),
                );
                por.insert(
                    "finalized_block_hash_hex".into(),
                    norito::json::Value::from("66".repeat(32)),
                );
                ProofStreamItem::from_json(&norito::json::Value::Object(por), &context)
                    .expect("canonical PoR transport fixture")
            })
            .collect();
        (context, items)
    }
    fn sample_items() -> (ProofStreamVerificationContext, Vec<ProofStreamItem>) {
        sample_items_for(por_request(1))
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
        let (context, items) = sample_items();
        let payload = encode_items_ndjson(&items);
        let decoded =
            decode_transport_items(None, &payload, &context).expect("decode identity stream");
        assert_eq!(decoded.len(), items.len());
    }
    #[test]
    fn gzip_encoding_roundtrips() {
        let (context, items) = sample_items();
        let payload = encode_items_ndjson(&items);
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&payload).expect("write gzip payload");
        let compressed = encoder.finish().expect("finish gzip payload");
        let decoded = decode_transport_items(Some("gzip"), &compressed, &context)
            .expect("decode gzip transport");
        assert_eq!(decoded.len(), items.len());
    }
    #[test]
    fn deflate_encoding_roundtrips() {
        let (context, items) = sample_items();
        let payload = encode_items_ndjson(&items);
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&payload).expect("write deflate payload");
        let compressed = encoder.finish().expect("finish deflate payload");
        let decoded = decode_transport_items(Some("deflate"), &compressed, &context)
            .expect("decode deflate stream");
        assert_eq!(decoded.len(), items.len());
    }
    #[test]
    fn zstd_encoding_roundtrips() {
        let (context, items) = sample_items();
        let payload = encode_items_ndjson(&items);
        let compressed =
            zstd::stream::encode_all(payload.as_slice(), 3).expect("encode zstd payload");
        let decoded = decode_transport_items(Some("zstd"), &compressed, &context)
            .expect("decode zstd transport");
        assert_eq!(decoded.len(), items.len());
    }
    #[test]
    fn por_transport_enforces_exact_request_bound_schedule() {
        let (context, items) = sample_items_for(por_request(3));
        assert_eq!(items.len(), 3);
        let payload = encode_items_ndjson(&items);
        let decoded = decode_transport_items(None, &payload, &context)
            .expect("exact deterministic PoR schedule must verify");
        assert_eq!(decoded.len(), 3);
        let truncated = encode_items_ndjson(&items[..2]);
        assert!(matches!(
            decode_transport_items(None, &truncated, &context),
            Err(ProofStreamTransportError::InvalidSequence { .. })
        ));
        assert!(matches!(
            decode_transport_items(None, b"", &context),
            Err(ProofStreamTransportError::InvalidSequence { .. })
        ));
        let mut reordered = items.clone();
        reordered.swap(0, 1);
        assert!(matches!(
            decode_transport_items(None, &encode_items_ndjson(&reordered), &context),
            Err(ProofStreamTransportError::InvalidSequence { .. })
        ));
        let mut duplicated = items;
        duplicated[1] = duplicated[0].clone();
        assert!(matches!(
            decode_transport_items(None, &encode_items_ndjson(&duplicated), &context),
            Err(ProofStreamTransportError::InvalidSequence { .. })
        ));
    }
    #[test]
    fn por_transport_uses_nonce_seed_and_authenticated_population() {
        let request = por_request(3);
        let (context, items) = sample_items_for(request);
        let payload = encode_items_ndjson(&items);
        let leaf_count = items[0]
            .por_proof()
            .expect("canonical PoR item has a proof")
            .leaf_count;
        let first_index = items[0].sample_index().expect("canonical sample index");
        let trusted_root = *context
            .trusted_por_root()
            .expect("PoR context has an authenticated root");
        let wrong_context = (1u8..=u8::MAX)
            .find_map(|nonce_byte| {
                let mut changed = request;
                changed.nonce[0] = nonce_byte;
                let candidate =
                    ProofStreamVerificationContext::new(changed, Some(trusted_root)).ok()?;
                let expected = crate::PorSampleIndices::new(
                    leaf_count,
                    1,
                    candidate.por_sample_seed().expect("PoR seed"),
                )
                .ok()?
                .next()?;
                (expected != first_index).then_some(candidate)
            })
            .expect("find a nonce that selects a different first leaf");
        assert!(matches!(
            decode_transport_items(None, &payload, &wrong_context),
            Err(ProofStreamTransportError::ItemDecode { .. })
        ));
        let mut request_by_first_index =
            vec![None; usize::try_from(leaf_count).expect("fixture population fits usize")];
        let (replayed_request, replay_context) = (1u8..=u8::MAX)
            .find_map(|nonce_byte| {
                let mut changed = request;
                changed.nonce[0] = nonce_byte;
                let candidate =
                    ProofStreamVerificationContext::new(changed, Some(trusted_root)).ok()?;
                let expected = crate::PorSampleIndices::new(
                    leaf_count,
                    1,
                    candidate.por_sample_seed().expect("PoR seed"),
                )
                .ok()?
                .next()?;
                let slot = request_by_first_index
                    .get_mut(usize::try_from(expected).ok()?)
                    .expect("sample index is inside fixture population");
                match slot.replace(changed) {
                    Some(previous) => Some((previous, candidate)),
                    None => None,
                }
            })
            .expect("pigeonhole search finds distinct nonces with the same first sample");
        let (_, replayed_items) = sample_items_for(replayed_request);
        let replay_error =
            decode_transport_items(None, &encode_items_ndjson(&replayed_items), &replay_context)
                .expect_err("request digest must reject same-schedule response replay")
                .to_string();
        assert!(replay_error.contains("request digest does not match"));
        let mut item_map = items[0]
            .to_json()
            .as_object()
            .expect("PoR item object")
            .clone();
        let mut proof_map = item_map
            .get("proof")
            .and_then(norito::json::Value::as_object)
            .expect("PoR proof object")
            .clone();
        proof_map.insert(
            "leaf_count".into(),
            norito::json::Value::from(leaf_count + 1),
        );
        item_map.insert("proof".into(), norito::json::Value::Object(proof_map));
        let mut forged = norito::json::to_vec(&norito::json::Value::Object(item_map))
            .expect("encode forged population");
        forged.push(b'\n');
        assert!(
            decode_transport_items(None, &forged, &context).is_err(),
            "unauthenticated leaf population must fail closed"
        );
    }
    #[test]
    fn por_transport_cardinality_is_minimum_of_request_and_leaf_population() {
        let (context, items) = sample_items_for(por_request(10));
        let leaf_count = usize::try_from(
            items[0]
                .por_proof()
                .expect("canonical PoR item has a proof")
                .leaf_count,
        )
        .expect("fixture leaf count fits usize");
        assert_eq!(items.len(), leaf_count);
        let decoded = decode_transport_items(None, &encode_items_ndjson(&items), &context)
            .expect("population-truncated exact PoR response");
        assert_eq!(decoded.len(), leaf_count);
    }
    #[test]
    fn transport_rejects_attacker_root_and_wrong_request_context() {
        let (context, items) = sample_items();
        let payload = encode_items_ndjson(&items);
        let mut attacker_root = *context
            .trusted_por_root()
            .expect("PoR transport context has a trusted root");
        attacker_root[0] ^= 0xff;
        let attacker_context =
            ProofStreamVerificationContext::new(*context.request(), Some(attacker_root))
                .expect("non-zero attacker root is structurally valid");
        let error = decode_transport_items(None, &payload, &attacker_context)
            .expect_err("transport must authenticate PoR proofs against the trusted root")
            .to_string();
        assert!(error.contains("trusted manifest root"));
        let mut wrong_request = *context.request();
        wrong_request.provider_id = [0xbc; 32];
        let wrong_context =
            ProofStreamVerificationContext::new(wrong_request, context.trusted_por_root().copied())
                .expect("wrong-provider request is structurally valid");
        let error = decode_transport_items(None, &payload, &wrong_context)
            .expect_err("transport must bind items to the exact request")
            .to_string();
        assert!(error.contains("provider does not match"));
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
        let (context, items) = sample_items();
        let item = encode_items_ndjson(&items);
        let without_newline = &item[..item.len() - 1];
        assert!(matches!(
            decode_transport_items(None, without_newline, &context),
            Err(ProofStreamTransportError::MissingFinalNewline)
        ));
        let mut padded = vec![b' '];
        padded.extend_from_slice(&item);
        assert!(matches!(
            decode_transport_items(None, &padded, &context),
            Err(ProofStreamTransportError::NonCanonicalLine { line: 1 })
        ));
    }
    #[test]
    fn transport_rejects_item_and_count_bombs() {
        let (context, items) = sample_items();
        let oversized_line = vec![b'x'; MAX_PROOF_STREAM_LINE_BYTES + 1];
        let mut oversized_payload = oversized_line;
        oversized_payload.push(b'\n');
        assert!(matches!(
            decode_transport_items(None, &oversized_payload, &context),
            Err(ProofStreamTransportError::ItemTooLarge { line: 1, .. })
        ));
        let one_item = encode_items_ndjson(&items);
        let mut too_many = Vec::with_capacity(one_item.len() * 2);
        for _ in 0..2 {
            too_many.extend_from_slice(&one_item);
        }
        assert!(matches!(
            decode_transport_items(None, &too_many, &context),
            Err(ProofStreamTransportError::TooManyItems { limit: 1 })
        ));
    }
    #[test]
    fn item_decode_errors_do_not_echo_response_payloads() {
        let (context, _) = sample_items();
        let payload = b"{\"secret\":\"do-not-log-this\"}\n";
        let error = decode_transport_items(None, payload, &context)
            .expect_err("malformed item must fail")
            .to_string();
        assert!(error.contains("line 1"));
        assert!(!error.contains("do-not-log-this"));
    }
}
