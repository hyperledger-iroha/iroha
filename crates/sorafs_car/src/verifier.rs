//! Trustless verification helpers for SoraFS CAR streams.
//!
//! The verifier parses CARv2 archives, recomputes chunk digests, and rebuilds
//! the Proof-of-Retrievability (PoR) tree so clients can validate responses
//! from untrusted gateways. It supports both `dag-scope=full` downloads and
//! `dag-scope=block` ranged responses.

use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
    ops::RangeInclusive,
};

use blake3::Hash;
use sorafs_manifest::ManifestV1;
use thiserror::Error;

use crate::{
    BLAKE3_256_MULTIHASH_CODE, CarBuildPlan, CarPlanError, CarWriteStats, ChunkProfile, ChunkStore,
    DAG_CBOR_CODEC, HEADER_LEN, PRAGMA, RAW_CODEC, chunker_registry, encode_cid,
};

/// Result returned after verifying a `dag-scope=full` CAR stream.
#[derive(Debug)]
pub struct CarVerificationReport {
    /// Statistics derived from the verified archive.
    pub stats: CarWriteStats,
    /// PoR-ready chunk metadata extracted from the archive.
    pub chunk_store: ChunkStore,
}

/// Outcome returned after verifying a `dag-scope=block` CAR stream.
#[derive(Debug)]
pub struct BlockVerificationReport {
    /// Indices of the chunks that were served (relative to the manifest plan).
    pub chunk_indices: Vec<usize>,
    /// Inclusive byte range covered by the response.
    pub payload_range: RangeInclusive<u64>,
    /// Total number of payload bytes carried by the response.
    pub payload_bytes: u64,
    /// BLAKE3-256 digest of the streamed payload.
    pub payload_digest: [u8; 32],
}

/// Trustless CAR verifier.
#[derive(Debug, Default)]
pub struct CarVerifier;

impl CarVerifier {
    /// Verifies a `dag-scope=full` CAR response against the supplied manifest.
    pub fn verify_full_car(
        manifest: &ManifestV1,
        car_bytes: &[u8],
    ) -> Result<CarVerificationReport, CarVerifyError> {
        Self::verify_full_car_internal(manifest, None, car_bytes)
    }

    /// Verifies a `dag-scope=full` CAR response using an existing chunk plan.
    ///
    /// The supplied plan is cross-checked against the archive before rebuilding
    /// the PoR tree so downstream tooling can reuse precomputed chunk metadata.
    pub fn verify_full_car_with_plan(
        manifest: &ManifestV1,
        plan: &CarBuildPlan,
        car_bytes: &[u8],
    ) -> Result<CarVerificationReport, CarVerifyError> {
        Self::verify_full_car_internal(manifest, Some(plan), car_bytes)
    }

    /// Verifies a `dag-scope=block` (range) CAR response.
    ///
    /// When `expected_range` is supplied the byte range derived from the plan
    /// must match it exactly; otherwise verification fails.
    pub fn verify_block_car(
        manifest: &ManifestV1,
        plan: &CarBuildPlan,
        car_bytes: &[u8],
        expected_range: Option<RangeInclusive<u64>>,
    ) -> Result<BlockVerificationReport, CarVerifyError> {
        let parsed = ParsedCar::parse(car_bytes)?;
        ensure_chunking_multihash(manifest)?;

        if parsed.chunk_sections().is_empty() {
            return Err(CarVerifyError::EmptyRange);
        }

        let mut digest_to_index: HashMap<[u8; 32], VecDeque<usize>> = HashMap::new();
        for (idx, chunk) in plan.chunks.iter().enumerate() {
            digest_to_index
                .entry(chunk.digest)
                .or_default()
                .push_back(idx);
        }

        let mut indices = Vec::with_capacity(parsed.chunk_sections().len());
        for (section_idx, section) in parsed.chunk_sections().iter().enumerate() {
            let Some(digest_indices) = digest_to_index.get_mut(&section.digest) else {
                return Err(CarVerifyError::UnknownChunkDigest {
                    section_index: section_idx,
                });
            };
            let Some(plan_index) = digest_indices.pop_front() else {
                return Err(CarVerifyError::UnknownChunkDigest {
                    section_index: section_idx,
                });
            };
            let plan_chunk = plan
                .chunks
                .get(plan_index)
                .ok_or(CarVerifyError::PlanChunkIndexOutOfRange { index: plan_index })?;
            if plan_chunk.length != section.length {
                return Err(CarVerifyError::ChunkLengthMismatch {
                    chunk_index: plan_index,
                    expected: plan_chunk.length,
                    actual: section.length,
                });
            }
            indices.push(plan_index);
        }

        indices.sort_unstable();
        indices.dedup();
        if indices.is_empty() {
            return Err(CarVerifyError::EmptyRange);
        }

        // Ensure the served chunks form a contiguous range.
        for window in indices.windows(2) {
            if window[1] != window[0] + 1 {
                return Err(CarVerifyError::NonContiguousChunkRange {
                    previous: window[0],
                    current: window[1],
                });
            }
        }

        let first_index = *indices
            .first()
            .ok_or(CarVerifyError::InternalInvariant("empty chunk range"))?;
        let last_index = *indices
            .last()
            .ok_or(CarVerifyError::InternalInvariant("empty chunk range"))?;
        let first_chunk = plan
            .chunks
            .get(first_index)
            .ok_or(CarVerifyError::PlanChunkIndexOutOfRange { index: first_index })?;
        let last_chunk = plan
            .chunks
            .get(last_index)
            .ok_or(CarVerifyError::PlanChunkIndexOutOfRange { index: last_index })?;

        let actual_start = first_chunk.offset;
        let actual_end_inclusive =
            last_chunk.offset + u64::from(last_chunk.length).saturating_sub(1);
        let actual_range = actual_start..=actual_end_inclusive;

        if let Some(expected) = expected_range
            && expected != actual_range
        {
            return Err(CarVerifyError::ExpectedRangeMismatch {
                expected_start: *expected.start(),
                expected_end: *expected.end(),
                actual_start,
                actual_end: actual_end_inclusive,
            });
        }

        if actual_end_inclusive >= manifest.content_length {
            return Err(CarVerifyError::RangeExceedsContentLength {
                content_length: manifest.content_length,
                range_end: actual_end_inclusive,
            });
        }

        let payload_digest = hash_to_array(blake3::hash(parsed.payload()));
        let payload_bytes: u64 = parsed
            .chunk_sections()
            .iter()
            .map(|section| u64::from(section.length))
            .sum();

        Ok(BlockVerificationReport {
            chunk_indices: indices,
            payload_range: actual_range,
            payload_bytes,
            payload_digest,
        })
    }

    fn verify_full_car_internal(
        manifest: &ManifestV1,
        plan_opt: Option<&CarBuildPlan>,
        car_bytes: &[u8],
    ) -> Result<CarVerificationReport, CarVerifyError> {
        let parsed = ParsedCar::parse(car_bytes)?;
        ensure_manifest_constraints(manifest, &parsed)?;
        ensure_chunking_multihash(manifest)?;

        let profile = chunk_profile_from_manifest(manifest)?;

        let plan_for_store: Cow<'_, CarBuildPlan> = match plan_opt {
            Some(plan) => {
                validate_plan(plan, &parsed)?;
                Cow::Borrowed(plan)
            }
            None => {
                let generated = CarBuildPlan::single_file_with_profile(parsed.payload(), profile)
                    .map_err(CarVerifyError::Plan)?;
                validate_plan(&generated, &parsed)?;
                Cow::Owned(generated)
            }
        };

        ensure_plan_offsets(plan_for_store.as_ref())?;

        let mut chunk_store = ChunkStore::with_profile(plan_for_store.chunk_profile);
        chunk_store.ingest_plan(parsed.payload(), plan_for_store.as_ref());

        let stats = CarWriteStats {
            payload_bytes: plan_for_store.content_length,
            chunk_count: plan_for_store.chunks.len(),
            car_size: parsed.total_len(),
            car_payload_digest: parsed.car_payload_digest(),
            car_archive_digest: parsed.car_archive_digest(),
            car_cid: encode_cid(RAW_CODEC, parsed.car_archive_digest().as_bytes()),
            root_cids: parsed.roots(),
            dag_codec: manifest.dag_codec.0,
            chunk_profile: plan_for_store.chunk_profile,
        };

        Ok(CarVerificationReport { stats, chunk_store })
    }
}

fn ensure_manifest_constraints(
    manifest: &ManifestV1,
    parsed: &ParsedCar<'_>,
) -> Result<(), CarVerifyError> {
    if manifest.car_size != parsed.total_len() {
        return Err(CarVerifyError::ManifestCarSizeMismatch {
            expected: manifest.car_size,
            actual: parsed.total_len(),
        });
    }
    if manifest.content_length != parsed.payload_len() {
        return Err(CarVerifyError::ManifestContentLengthMismatch {
            expected: manifest.content_length,
            actual: parsed.payload_len(),
        });
    }
    if manifest.car_digest != *parsed.car_archive_digest().as_bytes() {
        return Err(CarVerifyError::ManifestCarDigestMismatch);
    }
    let roots = parsed.roots();
    if roots.len() != 1 || roots[0] != manifest.root_cid {
        return Err(CarVerifyError::ManifestRootMismatch);
    }
    Ok(())
}

fn ensure_chunking_multihash(manifest: &ManifestV1) -> Result<(), CarVerifyError> {
    if manifest.chunking.multihash_code != BLAKE3_256_MULTIHASH_CODE {
        return Err(CarVerifyError::ManifestMultihashMismatch(
            manifest.chunking.multihash_code,
        ));
    }
    Ok(())
}

fn validate_plan(plan: &CarBuildPlan, parsed: &ParsedCar<'_>) -> Result<(), CarVerifyError> {
    if plan.chunks.len() != parsed.chunk_sections().len() {
        return Err(CarVerifyError::PlanChunkCountMismatch {
            expected: plan.chunks.len(),
            actual: parsed.chunk_sections().len(),
        });
    }
    for (idx, (plan_chunk, parsed_chunk)) in
        plan.chunks.iter().zip(parsed.chunk_sections()).enumerate()
    {
        if plan_chunk.digest != parsed_chunk.digest {
            return Err(CarVerifyError::ChunkDigestMismatch { chunk_index: idx });
        }
        if plan_chunk.length != parsed_chunk.length {
            return Err(CarVerifyError::ChunkLengthMismatch {
                chunk_index: idx,
                expected: plan_chunk.length,
                actual: parsed_chunk.length,
            });
        }
    }
    Ok(())
}

fn ensure_plan_offsets(plan: &CarBuildPlan) -> Result<(), CarVerifyError> {
    let mut expected_offset = 0u64;
    for (idx, chunk) in plan.chunks.iter().enumerate() {
        if chunk.offset != expected_offset {
            return Err(CarVerifyError::ChunkOffsetMismatch {
                chunk_index: idx,
                expected: expected_offset,
                actual: chunk.offset,
            });
        }
        expected_offset = expected_offset.checked_add(u64::from(chunk.length)).ok_or(
            CarVerifyError::InternalInvariant("chunk offset overflowed u64"),
        )?;
    }
    if expected_offset != plan.content_length {
        return Err(CarVerifyError::PlanContentLengthMismatch {
            expected: plan.content_length,
            actual: expected_offset,
        });
    }
    Ok(())
}

/// Verification errors surfaced while parsing or validating a CAR stream.
#[derive(Debug, Error)]
pub enum CarVerifyError {
    #[error("CAR bytes too short for header")]
    Truncated,
    #[error("invalid CAR pragma magic bytes")]
    InvalidPragma,
    #[error("invalid CARv2 header offsets")]
    InvalidHeader,
    #[error("CARv1 header invalid: {0}")]
    InvalidCarv1Header(&'static str),
    #[error("varint decoding exceeded buffer")]
    VarintOverflow,
    #[error("CARv1 header truncated")]
    HeaderTruncated,
    #[error("section truncated at index {section_index}")]
    TruncatedSection { section_index: usize },
    #[error("CID truncated within section {section_index}")]
    TruncatedCid { section_index: usize },
    #[error("unsupported digest length {length} in section {section_index}")]
    UnsupportedDigestLength { section_index: usize, length: u64 },
    #[error("unexpected multihash code {code:#x} in section {section_index}")]
    UnsupportedMultihash { section_index: usize, code: u64 },
    #[error("unexpected codec {codec:#x} in section {section_index}")]
    UnsupportedSectionCodec { section_index: usize, codec: u64 },
    #[error("chunk digest mismatch at index {chunk_index}")]
    ChunkDigestMismatch { chunk_index: usize },
    #[error("chunk payload size {len} exceeds configured maximum {max} at section {section_index}")]
    ChunkSizeExceeded {
        section_index: usize,
        len: u64,
        max: u64,
    },
    #[error("chunk length mismatch at index {chunk_index} (expected {expected}, found {actual})")]
    ChunkLengthMismatch {
        chunk_index: usize,
        expected: u32,
        actual: u32,
    },
    #[error("chunk offset mismatch at index {chunk_index} (expected {expected}, found {actual})")]
    ChunkOffsetMismatch {
        chunk_index: usize,
        expected: u64,
        actual: u64,
    },
    #[error("node digest mismatch at section {section_index}")]
    NodeDigestMismatch { section_index: usize },
    #[error("manifest car size mismatch (expected {expected}, actual {actual})")]
    ManifestCarSizeMismatch { expected: u64, actual: u64 },
    #[error("manifest content length mismatch (expected {expected}, actual {actual})")]
    ManifestContentLengthMismatch { expected: u64, actual: u64 },
    #[error("manifest car digest mismatch")]
    ManifestCarDigestMismatch,
    #[error("manifest root CID mismatch")]
    ManifestRootMismatch,
    #[error("manifest chunker multihash code mismatch ({0:#x})")]
    ManifestMultihashMismatch(u64),
    #[error("manifest chunking profile is unsupported or invalid")]
    ChunkProfileMismatch,
    #[error("plan chunk count mismatch (expected {expected}, actual {actual})")]
    PlanChunkCountMismatch { expected: usize, actual: usize },
    #[error("plan content length mismatch (expected {expected}, actual {actual})")]
    PlanContentLengthMismatch { expected: u64, actual: u64 },
    #[error(
        "expected byte range {expected_start}-{expected_end} does not match actual {actual_start}-{actual_end}"
    )]
    ExpectedRangeMismatch {
        expected_start: u64,
        expected_end: u64,
        actual_start: u64,
        actual_end: u64,
    },
    #[error(
        "byte range exceeds manifest content length (range_end={range_end}, content_length={content_length})"
    )]
    RangeExceedsContentLength { content_length: u64, range_end: u64 },
    #[error("chunk range is empty")]
    EmptyRange,
    #[error("chunk range is not contiguous (previous={previous}, current={current})")]
    NonContiguousChunkRange { previous: usize, current: usize },
    #[error("unknown chunk digest encountered in section {section_index}")]
    UnknownChunkDigest { section_index: usize },
    #[error("plan chunk index {index} out of range")]
    PlanChunkIndexOutOfRange { index: usize },
    #[error("invalid index offset in CAR (expected data index offset to follow payload region)")]
    InvalidIndexOffset,
    #[error("internal verifier invariant violated: {0}")]
    InternalInvariant(&'static str),
    #[error("failed to reconstruct plan: {0}")]
    Plan(#[from] CarPlanError),
}

pub(crate) struct ParsedCar<'a> {
    bytes: &'a [u8],
    roots: Vec<Vec<u8>>,
    payload: Vec<u8>,
    chunk_sections: Vec<ParsedChunkSection>,
    data_offset: u64,
    data_size: u64,
}

pub(crate) struct ParsedChunkSection {
    pub(crate) digest: [u8; 32],
    pub(crate) length: u32,
}

impl<'a> ParsedCar<'a> {
    pub(crate) fn parse(bytes: &'a [u8]) -> Result<Self, CarVerifyError> {
        if bytes.len() < PRAGMA.len().saturating_add(HEADER_LEN) {
            return Err(CarVerifyError::Truncated);
        }
        if bytes[..PRAGMA.len()] != PRAGMA {
            return Err(CarVerifyError::InvalidPragma);
        }

        let characteristics = &bytes[PRAGMA.len()..PRAGMA.len() + HEADER_LEN];
        let data_offset = u64::from_le_bytes(
            characteristics[16..24]
                .try_into()
                .expect("slice with fixed length"),
        );
        let data_size = u64::from_le_bytes(
            characteristics[24..32]
                .try_into()
                .expect("slice with fixed length"),
        );
        let index_offset = u64::from_le_bytes(
            characteristics[32..40]
                .try_into()
                .expect("slice with fixed length"),
        );

        let header_start = PRAGMA.len() + HEADER_LEN;
        let (header_len, header_len_bytes) =
            decode_uleb128(&bytes[header_start..]).map_err(|_| CarVerifyError::VarintOverflow)?;
        let carv1_header_start = header_start
            .checked_add(header_len_bytes)
            .ok_or(CarVerifyError::InvalidHeader)?;
        let carv1_header_end = carv1_header_start
            .checked_add(header_len as usize)
            .ok_or(CarVerifyError::InvalidHeader)?;
        if carv1_header_end > bytes.len() {
            return Err(CarVerifyError::HeaderTruncated);
        }

        let roots = parse_carv1_header(&bytes[carv1_header_start..carv1_header_end])?;

        if data_offset as usize != header_start {
            return Err(CarVerifyError::InvalidHeader);
        }
        let data_end = data_offset
            .checked_add(data_size)
            .ok_or(CarVerifyError::InvalidHeader)?;
        if data_end as usize > bytes.len() {
            return Err(CarVerifyError::Truncated);
        }
        if index_offset != data_end {
            return Err(CarVerifyError::InvalidIndexOffset);
        }

        let mut cursor = carv1_header_end;
        let mut payload = Vec::with_capacity(data_size as usize);
        let mut chunk_sections = Vec::new();
        let mut section_index = 0usize;

        while (cursor as u64) < data_end {
            let (section_len, len_bytes) =
                decode_uleb128(&bytes[cursor..]).map_err(|_| CarVerifyError::VarintOverflow)?;
            cursor = cursor
                .checked_add(len_bytes)
                .ok_or(CarVerifyError::Truncated)?;
            let section_len_usize = usize::try_from(section_len)
                .map_err(|_| CarVerifyError::TruncatedSection { section_index })?;
            let section_end = cursor
                .checked_add(section_len_usize)
                .ok_or(CarVerifyError::Truncated)?;
            if section_end as u64 > data_end {
                return Err(CarVerifyError::TruncatedSection { section_index });
            }

            let (cid, cid_len) = decode_cid(&bytes[cursor..], section_index)?;
            cursor = cursor
                .checked_add(cid_len)
                .ok_or(CarVerifyError::Truncated)?;
            let data_len = section_len_usize
                .checked_sub(cid_len)
                .ok_or(CarVerifyError::TruncatedSection { section_index })?;
            let data_slice = &bytes[cursor..cursor + data_len];
            cursor += data_len;

            let digest = blake3::hash(data_slice);
            if cid.multihash != BLAKE3_256_MULTIHASH_CODE {
                return Err(CarVerifyError::UnsupportedMultihash {
                    section_index,
                    code: cid.multihash,
                });
            }
            if digest.as_bytes() != cid.digest.as_ref() {
                if cid.codec == RAW_CODEC {
                    return Err(CarVerifyError::ChunkDigestMismatch {
                        chunk_index: chunk_sections.len(),
                    });
                } else {
                    return Err(CarVerifyError::NodeDigestMismatch { section_index });
                }
            }

            match cid.codec {
                RAW_CODEC => {
                    payload.extend_from_slice(data_slice);
                    chunk_sections.push(ParsedChunkSection {
                        digest: cid.digest,
                        length: u32::try_from(data_len)
                            .map_err(|_| CarVerifyError::TruncatedSection { section_index })?,
                    });
                }
                DAG_CBOR_CODEC => {}
                other => {
                    return Err(CarVerifyError::UnsupportedSectionCodec {
                        section_index,
                        codec: other,
                    });
                }
            }

            section_index += 1;
        }

        Ok(Self {
            bytes,
            roots,
            payload,
            chunk_sections,
            data_offset,
            data_size,
        })
    }

    pub(crate) fn total_len(&self) -> u64 {
        self.bytes.len() as u64
    }

    pub(crate) fn payload_len(&self) -> u64 {
        self.payload.len() as u64
    }

    pub(crate) fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub(crate) fn chunk_sections(&self) -> &[ParsedChunkSection] {
        &self.chunk_sections
    }

    pub(crate) fn roots(&self) -> Vec<Vec<u8>> {
        self.roots.clone()
    }

    pub(crate) fn car_payload_digest(&self) -> Hash {
        let start = self.data_offset as usize;
        let end = start + self.data_size as usize;
        blake3::hash(&self.bytes[start..end])
    }

    pub(crate) fn car_archive_digest(&self) -> Hash {
        blake3::hash(self.bytes)
    }
}

pub(crate) fn decode_cid(
    data: &[u8],
    section_index: usize,
) -> Result<(CidInfo, usize), CarVerifyError> {
    let (version, consumed_version) =
        decode_uleb128(data).map_err(|_| CarVerifyError::VarintOverflow)?;
    if version != 1 {
        return Err(CarVerifyError::UnsupportedSectionCodec {
            section_index,
            codec: version,
        });
    }
    let (codec, consumed_codec) =
        decode_uleb128(&data[consumed_version..]).map_err(|_| CarVerifyError::VarintOverflow)?;
    let (mh_code, consumed_mh) = decode_uleb128(&data[consumed_version + consumed_codec..])
        .map_err(|_| CarVerifyError::VarintOverflow)?;
    let digest_offset = consumed_version + consumed_codec + consumed_mh;
    let (digest_len, consumed_len) =
        decode_uleb128(&data[digest_offset..]).map_err(|_| CarVerifyError::VarintOverflow)?;
    let digest_start = digest_offset + consumed_len;
    let digest_end = digest_start
        .checked_add(digest_len as usize)
        .ok_or(CarVerifyError::TruncatedCid { section_index })?;
    if digest_end > data.len() {
        return Err(CarVerifyError::TruncatedCid { section_index });
    }
    if digest_len != 32 {
        return Err(CarVerifyError::UnsupportedDigestLength {
            section_index,
            length: digest_len,
        });
    }
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&data[digest_start..digest_end]);
    Ok((
        CidInfo {
            codec,
            multihash: mh_code,
            digest,
        },
        digest_end,
    ))
}

pub(crate) struct CidInfo {
    pub(crate) codec: u64,
    pub(crate) multihash: u64,
    pub(crate) digest: [u8; 32],
}

fn chunk_profile_from_manifest(manifest: &ManifestV1) -> Result<ChunkProfile, CarVerifyError> {
    if let Some(descriptor) =
        chunker_registry::lookup(crate::ProfileId(manifest.chunking.profile_id.0))
    {
        return Ok(descriptor.profile);
    }
    let profile = ChunkProfile {
        min_size: manifest.chunking.min_size as usize,
        target_size: manifest.chunking.target_size as usize,
        max_size: manifest.chunking.max_size as usize,
        break_mask: manifest.chunking.break_mask as u64,
    };
    if profile.min_size == 0
        || profile.min_size > profile.target_size
        || profile.target_size > profile.max_size
        || profile.break_mask == 0
    {
        return Err(CarVerifyError::ChunkProfileMismatch);
    }
    Ok(profile)
}

pub(crate) fn parse_carv1_header(bytes: &[u8]) -> Result<Vec<Vec<u8>>, CarVerifyError> {
    let (map_len, mut idx) = decode_cbor_map_len(bytes)?;
    if map_len < 2 {
        return Err(CarVerifyError::InvalidCarv1Header(
            "expected roots + version entries",
        ));
    }
    let mut roots: Option<Vec<Vec<u8>>> = None;
    let mut version: Option<u64> = None;

    for _ in 0..map_len {
        let (key, consumed_key) = decode_cbor_text(&bytes[idx..])?;
        idx += consumed_key;
        match key.as_str() {
            "roots" => {
                let (count, consumed) = decode_cbor_array_len(&bytes[idx..])?;
                idx += consumed;
                let mut entries = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let (value, consumed) = decode_cbor_bytes(&bytes[idx..])?;
                    idx += consumed;
                    entries.push(value);
                }
                roots = Some(entries);
            }
            "version" => {
                let (value, consumed) = decode_cbor_uint(&bytes[idx..])?;
                idx += consumed;
                version = Some(value);
            }
            _ => return Err(CarVerifyError::InvalidCarv1Header("unexpected header key")),
        }
    }

    if version != Some(1) {
        return Err(CarVerifyError::InvalidCarv1Header("unsupported version"));
    }
    roots.ok_or(CarVerifyError::InvalidCarv1Header("missing roots"))
}

pub(crate) fn decode_uleb128(data: &[u8]) -> Result<(u64, usize), ()> {
    let mut value = 0u64;
    let mut shift = 0u32;
    for (idx, byte) in data.iter().enumerate() {
        let slice = (byte & 0x7F) as u64;
        value |= slice << shift;
        if byte & 0x80 == 0 {
            return Ok((value, idx + 1));
        }
        shift += 7;
        if shift >= 64 {
            return Err(());
        }
    }
    Err(())
}

fn decode_cbor_map_len(data: &[u8]) -> Result<(u64, usize), CarVerifyError> {
    decode_cbor_len(5, data)
}

fn decode_cbor_array_len(data: &[u8]) -> Result<(u64, usize), CarVerifyError> {
    decode_cbor_len(4, data)
}

fn decode_cbor_uint(data: &[u8]) -> Result<(u64, usize), CarVerifyError> {
    decode_cbor_len(0, data)
}

fn decode_cbor_text(data: &[u8]) -> Result<(String, usize), CarVerifyError> {
    let (len, consumed) = decode_cbor_len(3, data)?;
    let start = consumed;
    let end = start
        .checked_add(len as usize)
        .ok_or(CarVerifyError::InvalidCarv1Header("text length overflow"))?;
    if end > data.len() {
        return Err(CarVerifyError::InvalidCarv1Header("text truncated"));
    }
    let text = String::from_utf8(data[start..end].to_vec())
        .map_err(|_| CarVerifyError::InvalidCarv1Header("text invalid utf8"))?;
    Ok((text, end))
}

fn decode_cbor_bytes(data: &[u8]) -> Result<(Vec<u8>, usize), CarVerifyError> {
    let (len, consumed) = decode_cbor_len(2, data)?;
    let start = consumed;
    let end = start
        .checked_add(len as usize)
        .ok_or(CarVerifyError::InvalidCarv1Header("bytes length overflow"))?;
    if end > data.len() {
        return Err(CarVerifyError::InvalidCarv1Header("bytes truncated"));
    }
    Ok((data[start..end].to_vec(), end))
}

fn decode_cbor_len(expected_major: u8, data: &[u8]) -> Result<(u64, usize), CarVerifyError> {
    if data.is_empty() {
        return Err(CarVerifyError::InvalidCarv1Header("missing CBOR data"));
    }
    let first = data[0];
    if first >> 5 != expected_major {
        return Err(CarVerifyError::InvalidCarv1Header(
            "unexpected CBOR major type",
        ));
    }
    let additional = first & 0x1F;
    match additional {
        v @ 0..=23 => Ok((v as u64, 1)),
        24 => {
            if data.len() < 2 {
                return Err(CarVerifyError::InvalidCarv1Header("truncated CBOR length"));
            }
            Ok((data[1] as u64, 2))
        }
        25 => {
            if data.len() < 3 {
                return Err(CarVerifyError::InvalidCarv1Header("truncated CBOR length"));
            }
            Ok((u16::from_be_bytes([data[1], data[2]]) as u64, 3))
        }
        26 => {
            if data.len() < 5 {
                return Err(CarVerifyError::InvalidCarv1Header("truncated CBOR length"));
            }
            Ok((
                u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as u64,
                5,
            ))
        }
        27 => {
            if data.len() < 9 {
                return Err(CarVerifyError::InvalidCarv1Header("truncated CBOR length"));
            }
            Ok((
                u64::from_be_bytes([
                    data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8],
                ]),
                9,
            ))
        }
        _ => Err(CarVerifyError::InvalidCarv1Header(
            "unsupported CBOR additional info",
        )),
    }
}

fn hash_to_array(hash: Hash) -> [u8; 32] {
    let mut arr = [0u8; 32];
    arr.copy_from_slice(hash.as_bytes());
    arr
}

#[cfg(test)]
mod tests {
    use blake3::hash as blake3_hash;
    use sorafs_manifest::{DagCodecId, GovernanceProofs, ManifestBuilder, PinPolicy, StorageClass};

    use super::*;
    use crate::{CarChunk, ChunkProfile, FilePlan};

    fn sample_payload() -> Vec<u8> {
        let total_bytes = 512 * 1024; // ensure multiple chunks under the default profile
        let mut payload = Vec::with_capacity(total_bytes);
        for idx in 0..total_bytes {
            payload.push((idx % 251) as u8);
        }
        payload
    }

    fn build_manifest(plan: &CarBuildPlan, stats: &CarWriteStats) -> ManifestV1 {
        let mut car_digest = [0u8; 32];
        car_digest.copy_from_slice(stats.car_archive_digest.as_bytes());
        ManifestBuilder::new()
            .root_cid(stats.root_cids[0].clone())
            .dag_codec(DagCodecId(stats.dag_codec))
            .chunking_from_profile(plan.chunk_profile, BLAKE3_256_MULTIHASH_CODE)
            .content_length(plan.content_length)
            .car_digest(car_digest)
            .car_size(stats.car_size)
            .pin_policy(PinPolicy {
                min_replicas: 1,
                storage_class: StorageClass::Hot,
                retention_epoch: 0,
            })
            .governance(GovernanceProofs::default())
            .build()
            .expect("manifest")
    }

    fn chunk_payload(plan: &CarBuildPlan, payload: &[u8], index: usize) -> Vec<u8> {
        let chunk = &plan.chunks[index];
        let start = chunk.offset as usize;
        let end = start + chunk.length as usize;
        payload[start..end].to_vec()
    }

    #[test]
    fn full_car_verification_with_plan_succeeds() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car_bytes = Vec::new();
        let stats = crate::CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car_bytes)
            .expect("write car");
        let manifest = build_manifest(&plan, &stats);
        let report =
            CarVerifier::verify_full_car_with_plan(&manifest, &plan, &car_bytes).expect("verify");
        assert_eq!(report.stats.chunk_count, plan.chunks.len());
        assert_eq!(report.chunk_store.payload_len(), plan.content_length);
    }

    #[test]
    fn full_car_verification_detects_length_mismatch() {
        let payload = sample_payload();
        let mut plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car_bytes = Vec::new();
        let stats = crate::CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car_bytes)
            .expect("write car");
        let manifest = build_manifest(&plan, &stats);
        plan.chunks[0].length -= 1;
        let err =
            CarVerifier::verify_full_car_with_plan(&manifest, &plan, &car_bytes).expect_err("err");
        assert!(matches!(
            err,
            CarVerifyError::ChunkLengthMismatch {
                chunk_index: 0,
                expected: _,
                actual: _
            }
        ));
    }

    #[test]
    fn block_car_verification_succeeds() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = crate::CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write car");
        let manifest = build_manifest(&plan, &stats);

        let first_chunk = chunk_payload(&plan, &payload, 0);
        let chunk = &plan.chunks[0];
        let sub_plan = CarBuildPlan {
            chunk_profile: plan.chunk_profile,
            payload_digest: blake3_hash(&first_chunk),
            content_length: chunk.length as u64,
            chunks: vec![CarChunk {
                offset: 0,
                length: chunk.length,
                digest: chunk.digest,
                taikai_segment_hint: chunk.taikai_segment_hint.clone(),
            }],
            files: vec![FilePlan {
                path: Vec::new(),
                first_chunk: 0,
                chunk_count: 1,
                size: first_chunk.len() as u64,
            }],
        };

        let mut range_car = Vec::new();
        crate::CarWriter::new(&sub_plan, &first_chunk)
            .expect("writer")
            .write_to(&mut range_car)
            .expect("write range car");

        let report =
            CarVerifier::verify_block_car(&manifest, &plan, &range_car, None).expect("verify");
        assert_eq!(report.chunk_indices, vec![0]);
        assert_eq!(*report.payload_range.start(), 0);
        assert_eq!(
            *report.payload_range.end(),
            u64::from(chunk.length).saturating_sub(1)
        );
        assert_eq!(
            report.payload_digest,
            hash_to_array(blake3_hash(&first_chunk))
        );
    }

    #[test]
    fn block_car_rejects_non_contiguous_range() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = crate::CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write car");
        let manifest = build_manifest(&plan, &stats);

        let chunk_count = plan.chunks.len();
        assert!(
            chunk_count >= 3,
            "expected at least three chunks for test setup"
        );
        let first = chunk_payload(&plan, &payload, 0);
        let third = chunk_payload(&plan, &payload, 2);
        let mut concat = Vec::with_capacity(first.len() + third.len());
        concat.extend_from_slice(&first);
        concat.extend_from_slice(&third);

        let sub_plan = CarBuildPlan {
            chunk_profile: plan.chunk_profile,
            payload_digest: blake3_hash(&concat),
            content_length: concat.len() as u64,
            chunks: vec![
                CarChunk {
                    offset: 0,
                    length: plan.chunks[0].length,
                    digest: plan.chunks[0].digest,
                    taikai_segment_hint: plan.chunks[0].taikai_segment_hint.clone(),
                },
                CarChunk {
                    offset: plan.chunks[0].length as u64,
                    length: plan.chunks[2].length,
                    digest: plan.chunks[2].digest,
                    taikai_segment_hint: plan.chunks[2].taikai_segment_hint.clone(),
                },
            ],
            files: vec![FilePlan {
                path: Vec::new(),
                first_chunk: 0,
                chunk_count: 2,
                size: concat.len() as u64,
            }],
        };

        let mut range_car = Vec::new();
        crate::CarWriter::new(&sub_plan, &concat)
            .expect("writer")
            .write_to(&mut range_car)
            .expect("write car");

        let err =
            CarVerifier::verify_block_car(&manifest, &plan, &range_car, None).expect_err("err");
        assert!(matches!(
            err,
            CarVerifyError::NonContiguousChunkRange {
                previous: 0,
                current: 2
            }
        ));
    }
}
