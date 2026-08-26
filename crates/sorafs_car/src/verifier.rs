//! Trustless verification helpers for SoraFS CAR streams.
//!
//! The verifier parses CARv2 archives, recomputes chunk digests, and rebuilds the
//! Proof-of-Retrievability (PoR) tree so clients can validate responses from untrusted gateways. It
//! supports both `dag-scope=full` downloads and `dag-scope=block` ranged responses.
use crate::{
    BLAKE3_256_MULTIHASH_CODE, CarBuildPlan, CarChunk, CarPlanError, CarStreamingWriter,
    CarWriteError, CarWriteStats, ChunkProfile, ChunkStore, ChunkStoreError, DAG_CBOR_CODEC,
    FilePlan, HEADER_LEN, PRAGMA, RAW_CODEC, chunker_registry,
};
use blake3::Hash;
use sorafs_manifest::ManifestV1;
use std::{
    borrow::Cow,
    io::{self, Read, Write},
    ops::{Range, RangeInclusive},
};
use thiserror::Error;
pub(crate) const MAX_CARV1_HEADER_SIZE: usize = 64 * 1024;
/// Result returned after verifying a `dag-scope=full` CAR stream.
#[derive(Debug)]
pub struct CarVerificationReport {
    /// Statistics derived from the verified archive.
    pub stats: CarWriteStats,
    /// PoR-ready chunk metadata extracted from the archive.
    pub chunk_store: ChunkStore,
}
/// Canonical CAR whose complete plan, container bytes, and payload sections were verified.
///
/// The retained parsed view lets security-sensitive consumers make repeated bounded passes over
/// the authenticated raw payload without cloning it out of the CAR. No instance is returned until
/// the supplied multi-file plan has reproduced the exact canonical container and roots.
pub struct VerifiedCanonicalCarV1<'a> {
    parsed: ParsedCar<'a>,
    stats: CarWriteStats,
}
impl VerifiedCanonicalCarV1<'_> {
    /// Return statistics reproduced from the verified plan and canonical CAR.
    #[must_use]
    pub const fn stats(&self) -> &CarWriteStats {
        &self.stats
    }
    /// Consume the retained view and return its reproduced writer statistics.
    #[must_use]
    pub fn into_stats(self) -> CarWriteStats {
        self.stats
    }
    /// Open a fresh reader over the authenticated raw payload sections.
    ///
    /// Each call starts at payload byte zero and borrows the immutable verified CAR bytes. The
    /// returned stream excludes CAR headers, DAG nodes, and the index.
    pub fn payload_reader(&self) -> impl Read + '_ {
        self.parsed.payload_reader()
    }
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
    /// Verifies that `car_bytes` are the exact canonical CARv2 encoding of
    /// `plan` and returns statistics derived from the retained archive.
    ///
    /// Unlike [`Self::verify_full_car_with_plan`], this entry point does not
    /// require a manifest. It is intended for callers that already retain and
    /// authenticate the complete build plan alongside the archive.
    pub fn verify_canonical_car_with_plan(
        plan: &CarBuildPlan,
        car_bytes: &[u8],
    ) -> Result<CarWriteStats, CarVerifyError> {
        Self::verify_canonical_car_with_plan_retained(plan, car_bytes)
            .map(VerifiedCanonicalCarV1::into_stats)
    }
    /// Verify a canonical multi-file CAR while retaining a zero-copy authenticated payload view.
    ///
    /// This performs the same complete plan, root, and byte-for-byte canonical-container checks as
    /// [`Self::verify_canonical_car_with_plan`]. It is intended for consumers that must reproduce
    /// higher-level commitments or parse bundle members after the container has been authenticated.
    pub fn verify_canonical_car_with_plan_retained<'a>(
        plan: &CarBuildPlan,
        car_bytes: &'a [u8],
    ) -> Result<VerifiedCanonicalCarV1<'a>, CarVerifyError> {
        let parsed = ParsedCar::parse(car_bytes)?;
        validate_plan(plan, &parsed)?;
        ensure_plan_offsets(plan)?;
        let mut canonical_car = CanonicalCarComparator::new(car_bytes);
        let mut payload_reader = parsed.payload_reader();
        let stats = CarStreamingWriter::new(plan)
            .write_from_reader(&mut payload_reader, &mut canonical_car)
            .map_err(CarVerifyError::CanonicalCar)?;
        if stats.root_cids != parsed.roots() {
            return Err(CarVerifyError::PlanRootMismatch);
        }
        if !canonical_car.matches_exactly() {
            return Err(CarVerifyError::NonCanonicalCar);
        }
        Ok(VerifiedCanonicalCarV1 { parsed, stats })
    }
    /// Verifies a canonical single-file `dag-scope=full` CAR response against
    /// the supplied manifest.
    ///
    /// Multi-file archives must use [`Self::verify_full_car_with_plan`] so the
    /// canonical file and directory DAG can be reconstructed exactly.
    pub fn verify_full_car(
        manifest: &ManifestV1,
        car_bytes: &[u8],
    ) -> Result<CarVerificationReport, CarVerifyError> {
        Self::verify_full_car_internal(manifest, None, car_bytes)
    }
    /// Verifies a `dag-scope=full` CAR response using an existing chunk plan.
    ///
    /// The supplied plan is cross-checked against the archive before rebuilding
    /// the PoR tree and the complete canonical CAR encoding.
    pub fn verify_full_car_with_plan(
        manifest: &ManifestV1,
        plan: &CarBuildPlan,
        car_bytes: &[u8],
    ) -> Result<CarVerificationReport, CarVerifyError> {
        Self::verify_full_car_internal(manifest, Some(plan), car_bytes)
    }
    /// Verifies a `dag-scope=block` (range) CAR response.
    ///
    /// The byte range derived from the plan must match `expected_range` exactly; otherwise
    /// verification fails. The raw chunk order, range-specific root, DAG nodes, index, and complete
    /// container encoding must also equal the canonical CAR produced for that plan slice.
    pub fn verify_block_car(
        manifest: &ManifestV1,
        plan: &CarBuildPlan,
        car_bytes: &[u8],
        expected_range: RangeInclusive<u64>,
    ) -> Result<BlockVerificationReport, CarVerifyError> {
        let parsed = ParsedCar::parse(car_bytes)?;
        ensure_chunking_multihash(manifest)?;
        let manifest_profile = chunk_profile_from_manifest(manifest)?;
        if plan.chunk_profile != manifest_profile {
            return Err(CarVerifyError::ChunkProfileMismatch);
        }
        for (chunk_index, chunk) in plan.chunks.iter().enumerate() {
            let length = usize::try_from(chunk.length).map_err(|_| {
                CarVerifyError::InvalidPlanChunkLength {
                    chunk_index,
                    length: chunk.length,
                    max: manifest_profile.max_size,
                }
            })?;
            if length == 0 || length > manifest_profile.max_size {
                return Err(CarVerifyError::InvalidPlanChunkLength {
                    chunk_index,
                    length: chunk.length,
                    max: manifest_profile.max_size,
                });
            }
        }
        ensure_plan_offsets(plan)?;
        if plan.content_length != manifest.content_length {
            return Err(CarVerifyError::PlanContentLengthMismatch {
                expected: manifest.content_length,
                actual: plan.content_length,
            });
        }
        if parsed.chunk_sections().is_empty() {
            return Err(CarVerifyError::EmptyRange);
        }
        // Match the raw section sequence directly. Reordering the resulting
        // indices would turn a reversed or duplicated wire sequence into an
        // apparently valid contiguous range.
        let indices = match_ordered_chunk_range(plan, parsed.chunk_sections(), &expected_range)?;
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
        let actual_end_inclusive = last_chunk
            .offset
            .checked_add(u64::from(last_chunk.length))
            .and_then(|end| end.checked_sub(1))
            .ok_or(CarVerifyError::InternalInvariant(
                "block range end overflowed u64",
            ))?;
        let actual_range = actual_start..=actual_end_inclusive;
        if expected_range != actual_range {
            return Err(CarVerifyError::ExpectedRangeMismatch {
                expected_start: *expected_range.start(),
                expected_end: *expected_range.end(),
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
        let payload_bytes = parsed
            .chunk_sections()
            .iter()
            .try_fold(0u64, |total, section| {
                total.checked_add(u64::from(section.length))
            })
            .ok_or(CarVerifyError::InternalInvariant(
                "block payload length overflowed u64",
            ))?;
        if payload_bytes != parsed.payload_len() {
            return Err(CarVerifyError::InternalInvariant(
                "block payload length disagrees with raw sections",
            ));
        }
        let canonical_plan = canonical_block_plan(
            plan,
            &indices,
            parsed.payload_digest(),
            parsed.payload_len(),
        )?;
        let mut canonical_car = CanonicalCarComparator::new(car_bytes);
        let mut payload_reader = parsed.payload_reader();
        let canonical_stats = CarStreamingWriter::new(&canonical_plan)
            .write_from_reader(&mut payload_reader, &mut canonical_car)
            .map_err(CarVerifyError::CanonicalCar)?;
        if parsed.roots() != canonical_stats.root_cids {
            return Err(CarVerifyError::BlockRootMismatch);
        }
        if !canonical_car.matches_exactly() {
            return Err(CarVerifyError::NonCanonicalCar);
        }
        let payload_digest = hash_to_array(parsed.payload_digest());
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
                if plan.chunk_profile != profile {
                    return Err(CarVerifyError::ChunkProfileMismatch);
                }
                validate_plan(plan, &parsed)?;
                Cow::Borrowed(plan)
            }
            None => {
                let generated = plan_from_parsed_payload(&parsed, profile)?;
                validate_plan(&generated, &parsed)?;
                Cow::Owned(generated)
            }
        };
        ensure_plan_offsets(plan_for_store.as_ref())?;
        let mut canonical_car = CanonicalCarComparator::new(car_bytes);
        let mut canonical_payload = parsed.payload_reader();
        let canonical_stats = CarStreamingWriter::new(plan_for_store.as_ref())
            .write_from_reader(&mut canonical_payload, &mut canonical_car)
            .map_err(CarVerifyError::CanonicalCar)?;
        if canonical_stats.root_cids != parsed.roots() {
            return Err(CarVerifyError::ManifestRootMismatch);
        }
        if !canonical_car.matches_exactly() {
            return Err(CarVerifyError::NonCanonicalCar);
        }
        let mut chunk_store = ChunkStore::with_profile(plan_for_store.chunk_profile);
        let mut payload_reader = parsed.payload_reader();
        chunk_store
            .ingest_plan_stream(plan_for_store.as_ref(), &mut payload_reader)
            .map_err(CarVerifyError::ChunkStore)?;
        Ok(CarVerificationReport {
            stats: canonical_stats,
            chunk_store,
        })
    }
}
fn match_ordered_chunk_range(
    plan: &CarBuildPlan,
    sections: &[ParsedChunkSection],
    expected_range: &RangeInclusive<u64>,
) -> Result<Vec<usize>, CarVerifyError> {
    let section_count = sections.len();
    if section_count == 0 || expected_range.is_empty() {
        return Err(CarVerifyError::EmptyRange);
    }
    let expected_start = *expected_range.start();
    let start = plan
        .chunks
        .binary_search_by_key(&expected_start, |chunk| chunk.offset)
        .map_err(|_| CarVerifyError::ExpectedRangeNotChunkAligned {
            start: expected_start,
            end: *expected_range.end(),
        })?;
    let end = start
        .checked_add(section_count)
        .filter(|end| *end <= plan.chunks.len())
        .ok_or(CarVerifyError::UnexpectedChunkOrder)?;
    for (relative_index, (chunk, section)) in
        plan.chunks[start..end].iter().zip(sections).enumerate()
    {
        let chunk_index = start + relative_index;
        if chunk.digest != section.digest {
            return Err(CarVerifyError::UnexpectedChunkOrder);
        }
        if chunk.length != section.length {
            return Err(CarVerifyError::ChunkLengthMismatch {
                chunk_index,
                expected: chunk.length,
                actual: section.length,
            });
        }
    }
    let actual = block_range_for_indices(plan, start, section_count).ok_or(
        CarVerifyError::InternalInvariant("matched block range could not be reconstructed"),
    )?;
    if &actual != expected_range {
        return Err(CarVerifyError::ExpectedRangeMismatch {
            expected_start,
            expected_end: *expected_range.end(),
            actual_start: *actual.start(),
            actual_end: *actual.end(),
        });
    }
    let mut indices = Vec::new();
    try_reserve_verifier(&mut indices, section_count, "matched chunk indices")?;
    indices.extend(start..end);
    Ok(indices)
}
fn block_range_for_indices(
    plan: &CarBuildPlan,
    start: usize,
    count: usize,
) -> Option<RangeInclusive<u64>> {
    let first = plan.chunks.get(start)?;
    let last = plan.chunks.get(start.checked_add(count)?.checked_sub(1)?)?;
    let end = last
        .offset
        .checked_add(u64::from(last.length))?
        .checked_sub(1)?;
    Some(first.offset..=end)
}
fn canonical_block_plan(
    plan: &CarBuildPlan,
    indices: &[usize],
    payload_digest: Hash,
    payload_len: u64,
) -> Result<CarBuildPlan, CarVerifyError> {
    let mut relative_offset = 0u64;
    let mut chunks = Vec::new();
    try_reserve_verifier(&mut chunks, indices.len(), "canonical block chunks")?;
    for &index in indices {
        let chunk = plan
            .chunks
            .get(index)
            .ok_or(CarVerifyError::PlanChunkIndexOutOfRange { index })?;
        chunks.push(CarChunk {
            offset: relative_offset,
            length: chunk.length,
            digest: chunk.digest,
            taikai_segment_hint: chunk
                .taikai_segment_hint
                .as_ref()
                .map(crate::try_clone_taikai_hint)
                .transpose()
                .map_err(CarVerifyError::Plan)?,
        });
        relative_offset = relative_offset.checked_add(u64::from(chunk.length)).ok_or(
            CarVerifyError::InternalInvariant("canonical block plan length overflowed u64"),
        )?;
    }
    if relative_offset != payload_len {
        return Err(CarVerifyError::InternalInvariant(
            "canonical block plan length disagrees with payload",
        ));
    }
    Ok(CarBuildPlan {
        chunk_profile: plan.chunk_profile,
        payload_digest,
        content_length: payload_len,
        chunks,
        files: vec![FilePlan {
            path: Vec::new(),
            first_chunk: 0,
            chunk_count: indices.len(),
            size: payload_len,
        }],
    })
}
fn plan_from_parsed_payload(
    parsed: &ParsedCar<'_>,
    profile: ChunkProfile,
) -> Result<CarBuildPlan, CarVerifyError> {
    if parsed.payload_len() == 0 {
        return Err(CarVerifyError::Plan(CarPlanError::EmptyInput));
    }
    let mut chunker = sorafs_chunker::Chunker::try_with_profile(profile)
        .map_err(|_| CarVerifyError::ChunkProfileMismatch)?;
    let expected_chunk_count = parsed.chunk_sections().len();
    let mut derived_chunks = Vec::new();
    try_reserve_verifier(
        &mut derived_chunks,
        expected_chunk_count,
        "derived verifier chunks",
    )?;
    let mut emitted_too_many = false;
    for section in parsed.chunk_sections() {
        chunker.feed(parsed.chunk_payload(section), |chunk| {
            if derived_chunks.len() < expected_chunk_count {
                derived_chunks.push(chunk);
            } else {
                emitted_too_many = true;
            }
        });
    }
    chunker.finish(|chunk| {
        if derived_chunks.len() < expected_chunk_count {
            derived_chunks.push(chunk);
        } else {
            emitted_too_many = true;
        }
    });
    if emitted_too_many || derived_chunks.len() != expected_chunk_count {
        return Err(CarVerifyError::PlanChunkCountMismatch {
            expected: if emitted_too_many {
                expected_chunk_count.saturating_add(1)
            } else {
                derived_chunks.len()
            },
            actual: expected_chunk_count,
        });
    }
    let mut chunks = Vec::new();
    try_reserve_verifier(&mut chunks, derived_chunks.len(), "verified CAR chunks")?;
    for (index, (derived, parsed_chunk)) in derived_chunks
        .iter()
        .zip(parsed.chunk_sections())
        .enumerate()
    {
        let offset = u64::try_from(derived.offset)
            .map_err(|_| CarVerifyError::InternalInvariant("chunk offset exceeds u64"))?;
        let length = u32::try_from(derived.length)
            .map_err(|_| CarVerifyError::InternalInvariant("chunk length exceeds u32"))?;
        if length != parsed_chunk.length {
            return Err(CarVerifyError::ChunkLengthMismatch {
                chunk_index: index,
                expected: length,
                actual: parsed_chunk.length,
            });
        }
        chunks.push(CarChunk {
            offset,
            length,
            digest: parsed_chunk.digest,
            taikai_segment_hint: None,
        });
    }
    Ok(CarBuildPlan {
        chunk_profile: profile,
        payload_digest: parsed.payload_digest(),
        content_length: parsed.payload_len(),
        chunks,
        files: vec![FilePlan {
            path: Vec::new(),
            first_chunk: 0,
            chunk_count: derived_chunks.len(),
            size: parsed.payload_len(),
        }],
    })
}
struct CanonicalCarComparator<'a> {
    expected: &'a [u8],
    written: usize,
    exact: bool,
}
impl<'a> CanonicalCarComparator<'a> {
    fn new(expected: &'a [u8]) -> Self {
        Self {
            expected,
            written: 0,
            exact: true,
        }
    }
    fn matches_exactly(&self) -> bool {
        self.exact && self.written == self.expected.len()
    }
}
impl Write for CanonicalCarComparator<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let end = self
            .written
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("canonical CAR comparison offset overflow"))?;
        if self.expected.get(self.written..end) != Some(buf) {
            self.exact = false;
        }
        self.written = end;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
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
    #[error("varint uses a non-canonical encoding")]
    NonCanonicalVarint,
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
    #[error("CAR root CID does not match the canonical supplied plan")]
    PlanRootMismatch,
    #[error("block CAR root CID does not match the canonical requested range root")]
    BlockRootMismatch,
    #[error("CAR contains non-canonical DAG, index, header, or trailing bytes")]
    NonCanonicalCar,
    #[error("failed to reconstruct canonical CAR: {0}")]
    CanonicalCar(#[source] CarWriteError),
    #[error("failed to ingest verified CAR payload: {0}")]
    ChunkStore(#[source] ChunkStoreError),
    #[error("manifest chunker multihash code mismatch ({0:#x})")]
    ManifestMultihashMismatch(u64),
    #[error("manifest chunking profile is unsupported or invalid")]
    ChunkProfileMismatch,
    #[error("plan chunk count mismatch (expected {expected}, actual {actual})")]
    PlanChunkCountMismatch { expected: usize, actual: usize },
    #[error("plan content length mismatch (expected {expected}, actual {actual})")]
    PlanContentLengthMismatch { expected: u64, actual: u64 },
    #[error("plan chunk {chunk_index} has invalid length {length}; expected 1..={max}")]
    InvalidPlanChunkLength {
        chunk_index: usize,
        length: u32,
        max: usize,
    },
    #[error(
        "expected byte range {expected_start}-{expected_end} does not match actual {actual_start}-{actual_end}"
    )]
    ExpectedRangeMismatch {
        expected_start: u64,
        expected_end: u64,
        actual_start: u64,
        actual_end: u64,
    },
    #[error("expected byte range {start}-{end} is not aligned to a chunk boundary")]
    ExpectedRangeNotChunkAligned { start: u64, end: u64 },
    #[error(
        "byte range exceeds manifest content length (range_end={range_end}, content_length={content_length})"
    )]
    RangeExceedsContentLength { content_length: u64, range_end: u64 },
    #[error("chunk range is empty")]
    EmptyRange,
    #[error("chunk range is not contiguous (previous={previous}, current={current})")]
    NonContiguousChunkRange { previous: usize, current: usize },
    #[error("raw chunk sections do not match plan order")]
    UnexpectedChunkOrder,
    #[error("unknown chunk digest encountered in section {section_index}")]
    UnknownChunkDigest { section_index: usize },
    #[error("plan chunk index {index} out of range")]
    PlanChunkIndexOutOfRange { index: usize },
    #[error("invalid index offset in CAR (expected data index offset to follow payload region)")]
    InvalidIndexOffset,
    #[error("internal verifier invariant violated: {0}")]
    InternalInvariant(&'static str),
    #[error("failed to reserve {requested} entries/bytes for {context}")]
    AllocationFailed {
        context: &'static str,
        requested: usize,
    },
    #[error("failed to reconstruct plan: {0}")]
    Plan(#[from] CarPlanError),
}
#[derive(Debug)]
pub(crate) struct ParsedCar<'a> {
    bytes: &'a [u8],
    roots: Vec<Vec<u8>>,
    chunk_sections: Vec<ParsedChunkSection>,
    payload_len: u64,
    payload_digest: Hash,
    total_len: u64,
}
#[derive(Debug)]
pub(crate) struct ParsedChunkSection {
    pub(crate) digest: [u8; 32],
    pub(crate) length: u32,
    data_range: Range<usize>,
}
fn try_reserve_verifier<T>(
    values: &mut Vec<T>,
    additional: usize,
    context: &'static str,
) -> Result<(), CarVerifyError> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| CarVerifyError::AllocationFailed {
            context,
            requested: additional,
        })
}
impl<'a> ParsedCar<'a> {
    pub(crate) fn parse(bytes: &'a [u8]) -> Result<Self, CarVerifyError> {
        let total_len = u64::try_from(bytes.len()).map_err(|_| CarVerifyError::InvalidHeader)?;
        if bytes.len() < PRAGMA.len().saturating_add(HEADER_LEN) {
            return Err(CarVerifyError::Truncated);
        }
        if bytes[..PRAGMA.len()] != PRAGMA {
            return Err(CarVerifyError::InvalidPragma);
        }
        let characteristics = &bytes[PRAGMA.len()..PRAGMA.len() + HEADER_LEN];
        let data_offset = u64::from_le_bytes(
            characteristics
                .get(16..24)
                .ok_or(CarVerifyError::InvalidHeader)?
                .try_into()
                .map_err(|_| CarVerifyError::InvalidHeader)?,
        );
        let data_size = u64::from_le_bytes(
            characteristics
                .get(24..32)
                .ok_or(CarVerifyError::InvalidHeader)?
                .try_into()
                .map_err(|_| CarVerifyError::InvalidHeader)?,
        );
        let index_offset = u64::from_le_bytes(
            characteristics
                .get(32..40)
                .ok_or(CarVerifyError::InvalidHeader)?
                .try_into()
                .map_err(|_| CarVerifyError::InvalidHeader)?,
        );
        let header_start = PRAGMA.len() + HEADER_LEN;
        let (header_len, header_len_bytes) =
            decode_uleb128(&bytes[header_start..]).map_err(map_uleb128_error)?;
        let header_len = usize::try_from(header_len).map_err(|_| CarVerifyError::InvalidHeader)?;
        if header_len > MAX_CARV1_HEADER_SIZE {
            return Err(CarVerifyError::InvalidCarv1Header(
                "header exceeds maximum size",
            ));
        }
        let carv1_header_start = header_start
            .checked_add(header_len_bytes)
            .ok_or(CarVerifyError::InvalidHeader)?;
        let carv1_header_end = carv1_header_start
            .checked_add(header_len)
            .ok_or(CarVerifyError::InvalidHeader)?;
        if carv1_header_end > bytes.len() {
            return Err(CarVerifyError::HeaderTruncated);
        }
        let roots = parse_carv1_header(&bytes[carv1_header_start..carv1_header_end])?;
        if data_offset != u64::try_from(header_start).map_err(|_| CarVerifyError::InvalidHeader)? {
            return Err(CarVerifyError::InvalidHeader);
        }
        let data_end = data_offset
            .checked_add(data_size)
            .ok_or(CarVerifyError::InvalidHeader)?;
        let data_end = usize::try_from(data_end).map_err(|_| CarVerifyError::InvalidHeader)?;
        if data_end > bytes.len() {
            return Err(CarVerifyError::Truncated);
        }
        if carv1_header_end > data_end {
            return Err(CarVerifyError::HeaderTruncated);
        }
        if index_offset
            != u64::try_from(data_end).map_err(|_| CarVerifyError::InvalidIndexOffset)?
        {
            return Err(CarVerifyError::InvalidIndexOffset);
        }
        let mut cursor = carv1_header_end;
        let mut chunk_sections = Vec::new();
        let mut payload_len = 0u64;
        let mut payload_hasher = blake3::Hasher::new();
        let mut section_index = 0usize;
        while cursor < data_end {
            let (section_len, len_bytes) =
                decode_uleb128(&bytes[cursor..]).map_err(map_uleb128_error)?;
            cursor = cursor
                .checked_add(len_bytes)
                .ok_or(CarVerifyError::Truncated)?;
            let section_len_usize = usize::try_from(section_len)
                .map_err(|_| CarVerifyError::TruncatedSection { section_index })?;
            let section_end = cursor
                .checked_add(section_len_usize)
                .ok_or(CarVerifyError::Truncated)?;
            if section_end > data_end {
                return Err(CarVerifyError::TruncatedSection { section_index });
            }
            let (cid, cid_len) = decode_cid(&bytes[cursor..], section_index)?;
            cursor = cursor
                .checked_add(cid_len)
                .ok_or(CarVerifyError::Truncated)?;
            let data_len = section_len_usize
                .checked_sub(cid_len)
                .ok_or(CarVerifyError::TruncatedSection { section_index })?;
            if cid.codec == RAW_CODEC && data_len > crate::CHUNK_STORE_MAX_CHUNK_BYTES as usize {
                return Err(CarVerifyError::ChunkSizeExceeded {
                    section_index,
                    len: u64::try_from(data_len).map_err(|_| {
                        CarVerifyError::InternalInvariant("raw CAR section length exceeds u64")
                    })?,
                    max: u64::from(crate::CHUNK_STORE_MAX_CHUNK_BYTES),
                });
            }
            let data_start = cursor;
            let data_end = cursor
                .checked_add(data_len)
                .ok_or(CarVerifyError::TruncatedSection { section_index })?;
            let data_slice = &bytes[data_start..data_end];
            cursor = data_end;
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
                    let length = u32::try_from(data_len)
                        .map_err(|_| CarVerifyError::TruncatedSection { section_index })?;
                    payload_len = payload_len.checked_add(u64::from(length)).ok_or(
                        CarVerifyError::InternalInvariant("parsed payload length overflowed u64"),
                    )?;
                    payload_hasher.update(data_slice);
                    try_reserve_verifier(&mut chunk_sections, 1, "parsed CAR chunk sections")?;
                    chunk_sections.push(ParsedChunkSection {
                        digest: cid.digest,
                        length,
                        data_range: data_start..data_end,
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
            section_index =
                section_index
                    .checked_add(1)
                    .ok_or(CarVerifyError::InternalInvariant(
                        "CAR section count overflowed usize",
                    ))?;
        }
        Ok(Self {
            bytes,
            roots,
            chunk_sections,
            payload_len,
            payload_digest: payload_hasher.finalize(),
            total_len,
        })
    }
    pub(crate) fn total_len(&self) -> u64 {
        self.total_len
    }
    pub(crate) fn payload_len(&self) -> u64 {
        self.payload_len
    }
    pub(crate) fn payload_digest(&self) -> Hash {
        self.payload_digest
    }
    pub(crate) fn payload_reader(&self) -> ParsedPayloadReader<'_, 'a> {
        ParsedPayloadReader {
            parsed: self,
            section_index: 0,
            section_offset: 0,
        }
    }
    #[cfg(feature = "cli")]
    pub(crate) fn payload_bytes(&self) -> Result<Vec<u8>, CarVerifyError> {
        let capacity = usize::try_from(self.payload_len)
            .map_err(|_| CarVerifyError::InternalInvariant("payload length exceeds host width"))?;
        let mut payload = Vec::new();
        payload
            .try_reserve_exact(capacity)
            .map_err(|_| CarVerifyError::AllocationFailed {
                context: "parsed CAR payload",
                requested: capacity,
            })?;
        for section in &self.chunk_sections {
            payload.extend_from_slice(self.chunk_payload(section));
        }
        if payload.len() != capacity {
            return Err(CarVerifyError::InternalInvariant(
                "parsed payload sections do not match payload length",
            ));
        }
        Ok(payload)
    }
    fn chunk_payload(&self, section: &ParsedChunkSection) -> &[u8] {
        &self.bytes[section.data_range.clone()]
    }
    pub(crate) fn chunk_sections(&self) -> &[ParsedChunkSection] {
        &self.chunk_sections
    }
    pub(crate) fn roots(&self) -> &[Vec<u8>] {
        &self.roots
    }
    pub(crate) fn car_archive_digest(&self) -> Hash {
        blake3::hash(self.bytes)
    }
}
pub(crate) struct ParsedPayloadReader<'parsed, 'bytes> {
    parsed: &'parsed ParsedCar<'bytes>,
    section_index: usize,
    section_offset: usize,
}
impl Read for ParsedPayloadReader<'_, '_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut written = 0usize;
        while written < buffer.len() {
            let Some(section) = self.parsed.chunk_sections.get(self.section_index) else {
                break;
            };
            let payload = self.parsed.chunk_payload(section);
            if self.section_offset == payload.len() {
                self.section_index += 1;
                self.section_offset = 0;
                continue;
            }
            let available = payload.len() - self.section_offset;
            let requested = buffer.len() - written;
            let count = available.min(requested);
            let payload_end = self.section_offset + count;
            let buffer_end = written + count;
            buffer[written..buffer_end].copy_from_slice(&payload[self.section_offset..payload_end]);
            self.section_offset = payload_end;
            written = buffer_end;
        }
        Ok(written)
    }
}
pub(crate) fn decode_cid(
    data: &[u8],
    section_index: usize,
) -> Result<(CidInfo, usize), CarVerifyError> {
    let (version, consumed_version) = decode_uleb128(data).map_err(map_uleb128_error)?;
    if version != 1 {
        return Err(CarVerifyError::UnsupportedSectionCodec {
            section_index,
            codec: version,
        });
    }
    let (codec, consumed_codec) =
        decode_uleb128(&data[consumed_version..]).map_err(map_uleb128_error)?;
    let multihash_offset = consumed_version
        .checked_add(consumed_codec)
        .ok_or(CarVerifyError::TruncatedCid { section_index })?;
    let (mh_code, consumed_mh) =
        decode_uleb128(&data[multihash_offset..]).map_err(map_uleb128_error)?;
    let digest_offset = multihash_offset
        .checked_add(consumed_mh)
        .ok_or(CarVerifyError::TruncatedCid { section_index })?;
    let (digest_len, consumed_len) =
        decode_uleb128(&data[digest_offset..]).map_err(map_uleb128_error)?;
    if digest_len != 32 {
        return Err(CarVerifyError::UnsupportedDigestLength {
            section_index,
            length: digest_len,
        });
    }
    let digest_start = digest_offset
        .checked_add(consumed_len)
        .ok_or(CarVerifyError::TruncatedCid { section_index })?;
    let digest_end = digest_start
        .checked_add(32)
        .ok_or(CarVerifyError::TruncatedCid { section_index })?;
    if digest_end > data.len() {
        return Err(CarVerifyError::TruncatedCid { section_index });
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
    if manifest.chunking.profile_id.0 != 0 {
        let descriptor = chunker_registry::lookup(crate::ProfileId(manifest.chunking.profile_id.0))
            .ok_or(CarVerifyError::ChunkProfileMismatch)?;
        let profile = descriptor.profile;
        let geometry_matches = u32::try_from(profile.min_size).ok()
            == Some(manifest.chunking.min_size)
            && u32::try_from(profile.target_size).ok() == Some(manifest.chunking.target_size)
            && u32::try_from(profile.max_size).ok() == Some(manifest.chunking.max_size)
            && u32::try_from(profile.break_mask).ok() == Some(manifest.chunking.break_mask);
        let identity_matches = manifest.chunking.namespace == descriptor.namespace
            && manifest.chunking.name == descriptor.name
            && manifest.chunking.semver == descriptor.semver
            && manifest.chunking.multihash_code == descriptor.multihash_code;
        let aliases_match = manifest.chunking.aliases.len() == descriptor.aliases.len()
            && manifest
                .chunking
                .aliases
                .iter()
                .zip(descriptor.aliases.iter())
                .all(|(provided, expected)| provided == *expected);
        if !geometry_matches || !identity_matches || !aliases_match {
            return Err(CarVerifyError::ChunkProfileMismatch);
        }
        return Ok(profile);
    }
    if manifest.chunking.namespace != "inline"
        || manifest.chunking.name != "inline"
        || manifest.chunking.semver != "0.0.0"
        || manifest.chunking.aliases.as_slice() != ["inline.inline@0.0.0"]
    {
        return Err(CarVerifyError::ChunkProfileMismatch);
    }
    let profile = ChunkProfile {
        min_size: manifest.chunking.min_size as usize,
        target_size: manifest.chunking.target_size as usize,
        max_size: manifest.chunking.max_size as usize,
        break_mask: manifest.chunking.break_mask as u64,
    };
    if profile.validate().is_err() || profile.max_size > crate::CHUNK_STORE_MAX_CHUNK_BYTES as usize
    {
        return Err(CarVerifyError::ChunkProfileMismatch);
    }
    Ok(profile)
}
pub(crate) fn parse_carv1_header(bytes: &[u8]) -> Result<Vec<Vec<u8>>, CarVerifyError> {
    let (map_len, mut idx) = decode_cbor_map_len(bytes)?;
    if map_len != 2 {
        return Err(CarVerifyError::InvalidCarv1Header(
            "expected exactly roots + version entries",
        ));
    }
    let mut roots: Option<Vec<Vec<u8>>> = None;
    let mut version: Option<u64> = None;
    for _ in 0..map_len {
        let (key, consumed_key) = decode_cbor_text(&bytes[idx..])?;
        idx += consumed_key;
        match key {
            "roots" => {
                if roots.is_some() {
                    return Err(CarVerifyError::InvalidCarv1Header("duplicate roots entry"));
                }
                let (count, consumed) = decode_cbor_array_len(&bytes[idx..])?;
                idx += consumed;
                let count = usize::try_from(count)
                    .map_err(|_| CarVerifyError::InvalidCarv1Header("roots count overflow"))?;
                if count > bytes.len().saturating_sub(idx) {
                    return Err(CarVerifyError::InvalidCarv1Header(
                        "roots count exceeds header bytes",
                    ));
                }
                let mut entries = Vec::new();
                try_reserve_verifier(&mut entries, count, "CARv1 header roots")?;
                for _ in 0..count {
                    let (value, consumed) = decode_cbor_bytes(&bytes[idx..])?;
                    idx += consumed;
                    entries.push(value);
                }
                roots = Some(entries);
            }
            "version" => {
                if version.is_some() {
                    return Err(CarVerifyError::InvalidCarv1Header(
                        "duplicate version entry",
                    ));
                }
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
    if idx != bytes.len() {
        return Err(CarVerifyError::InvalidCarv1Header(
            "trailing CARv1 header bytes",
        ));
    }
    roots.ok_or(CarVerifyError::InvalidCarv1Header("missing roots"))
}
/// Failure modes for canonical unsigned LEB128 decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Uleb128Error {
    /// The input ended before a terminating byte was found.
    Truncated,
    /// The encoded value exceeds the `u64` representation.
    Overflow,
    /// The value used more bytes than its shortest representation.
    NonCanonical,
}
fn map_uleb128_error(error: Uleb128Error) -> CarVerifyError {
    match error {
        Uleb128Error::NonCanonical => CarVerifyError::NonCanonicalVarint,
        Uleb128Error::Truncated | Uleb128Error::Overflow => CarVerifyError::VarintOverflow,
    }
}
pub(crate) fn decode_uleb128(data: &[u8]) -> Result<(u64, usize), Uleb128Error> {
    let mut value = 0u64;
    for (idx, byte) in data.iter().enumerate() {
        if idx >= 10 {
            return Err(Uleb128Error::Overflow);
        }
        let slice = (byte & 0x7F) as u64;
        if idx == 9 && (*byte & 0xFE) != 0 {
            return Err(Uleb128Error::Overflow);
        }
        value |= slice << (idx * 7);
        if byte & 0x80 == 0 {
            if idx > 0 && slice == 0 {
                return Err(Uleb128Error::NonCanonical);
            }
            return Ok((value, idx + 1));
        }
    }
    Err(Uleb128Error::Truncated)
}
pub(super) fn decode_cbor_map_len(data: &[u8]) -> Result<(u64, usize), CarVerifyError> {
    decode_cbor_len(5, data)
}
pub(super) fn decode_cbor_array_len(data: &[u8]) -> Result<(u64, usize), CarVerifyError> {
    decode_cbor_len(4, data)
}
pub(super) fn decode_cbor_uint(data: &[u8]) -> Result<(u64, usize), CarVerifyError> {
    decode_cbor_len(0, data)
}
pub(super) fn decode_cbor_text(data: &[u8]) -> Result<(&str, usize), CarVerifyError> {
    let (len, consumed) = decode_cbor_len(3, data)?;
    let start = consumed;
    let len = usize::try_from(len)
        .map_err(|_| CarVerifyError::InvalidCarv1Header("text length overflow"))?;
    let end = start
        .checked_add(len)
        .ok_or(CarVerifyError::InvalidCarv1Header("text length overflow"))?;
    if end > data.len() {
        return Err(CarVerifyError::InvalidCarv1Header("text truncated"));
    }
    let text = std::str::from_utf8(&data[start..end])
        .map_err(|_| CarVerifyError::InvalidCarv1Header("text invalid utf8"))?;
    Ok((text, end))
}
pub(super) fn decode_cbor_bytes(data: &[u8]) -> Result<(Vec<u8>, usize), CarVerifyError> {
    let (len, consumed) = decode_cbor_len(2, data)?;
    let start = consumed;
    let len = usize::try_from(len)
        .map_err(|_| CarVerifyError::InvalidCarv1Header("bytes length overflow"))?;
    let end = start
        .checked_add(len)
        .ok_or(CarVerifyError::InvalidCarv1Header("bytes length overflow"))?;
    if end > data.len() {
        return Err(CarVerifyError::InvalidCarv1Header("bytes truncated"));
    }
    let mut value = Vec::new();
    try_reserve_verifier(&mut value, len, "CARv1 header byte string")?;
    value.extend_from_slice(&data[start..end]);
    Ok((value, end))
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
            let value = u64::from(data[1]);
            if value < 24 {
                return Err(CarVerifyError::InvalidCarv1Header(
                    "non-canonical CBOR length",
                ));
            }
            Ok((value, 2))
        }
        25 => {
            if data.len() < 3 {
                return Err(CarVerifyError::InvalidCarv1Header("truncated CBOR length"));
            }
            let value = u64::from(u16::from_be_bytes([data[1], data[2]]));
            if value <= u64::from(u8::MAX) {
                return Err(CarVerifyError::InvalidCarv1Header(
                    "non-canonical CBOR length",
                ));
            }
            Ok((value, 3))
        }
        26 => {
            if data.len() < 5 {
                return Err(CarVerifyError::InvalidCarv1Header("truncated CBOR length"));
            }
            let value = u64::from(u32::from_be_bytes([data[1], data[2], data[3], data[4]]));
            if value <= u64::from(u16::MAX) {
                return Err(CarVerifyError::InvalidCarv1Header(
                    "non-canonical CBOR length",
                ));
            }
            Ok((value, 5))
        }
        27 => {
            if data.len() < 9 {
                return Err(CarVerifyError::InvalidCarv1Header("truncated CBOR length"));
            }
            let value = u64::from_be_bytes([
                data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8],
            ]);
            if value <= u64::from(u32::MAX) {
                return Err(CarVerifyError::InvalidCarv1Header(
                    "non-canonical CBOR length",
                ));
            }
            Ok((value, 9))
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
    use super::*;
    use crate::{CarChunk, CarWriter, ChunkProfile, FileEntry, FilePlan, encode_cid};
    use blake3::hash as blake3_hash;
    use sorafs_manifest::{DagCodecId, GovernanceProofs, ManifestBuilder, PinPolicy, StorageClass};
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
        let payload = sample_payload();
        ManifestBuilder::new()
            .root_cid(stats.root_cids[0].clone())
            .dag_codec(DagCodecId(stats.dag_codec))
            .chunking_from_profile(plan.chunk_profile, BLAKE3_256_MULTIHASH_CODE)
            .chunk_digest_sha3_256(crate::compute_chunk_plan_digest_sha3(&plan.chunks))
            .por_root(
                crate::compute_por_root(&payload, plan).expect("derive canonical fixture PoR root"),
            )
            .content_length(plan.content_length)
            .car_digest(car_digest)
            .car_size(stats.car_size)
            .pin_policy(PinPolicy {
                min_replicas: 1,
                storage_class: StorageClass::Hot,
                retention_epoch: 1,
            })
            .governance(GovernanceProofs::default())
            .build()
            .expect("manifest")
    }
    fn rebind_manifest_archive(manifest: &mut ManifestV1, car: &[u8]) {
        manifest.car_size = u64::try_from(car.len()).expect("fixture CAR length fits u64");
        manifest
            .car_digest
            .copy_from_slice(blake3_hash(car).as_bytes());
    }
    fn swap_carv1_header_entries(car: &mut [u8]) {
        let length_offset = PRAGMA.len() + HEADER_LEN;
        let (header_len, length_bytes) =
            decode_uleb128(&car[length_offset..]).expect("CARv1 header length");
        let header_len = usize::try_from(header_len).expect("header length fits usize");
        let header_start = length_offset + length_bytes;
        let header_end = header_start + header_len;
        let header = &car[header_start..header_end];
        assert_eq!(header[0], 0xa2, "fixture must use a two-entry map");
        const VERSION_KEY: &[u8] = b"\x67version";
        let version_offset = header
            .windows(VERSION_KEY.len())
            .rposition(|window| window == VERSION_KEY)
            .expect("version map entry");
        assert_eq!(
            version_offset + VERSION_KEY.len() + 1,
            header.len(),
            "version entry must terminate the canonical fixture header"
        );
        let roots_entry = header[1..version_offset].to_vec();
        let version_entry = header[version_offset..].to_vec();
        let mut swapped = Vec::with_capacity(header.len() - 1);
        swapped.extend_from_slice(&version_entry);
        swapped.extend_from_slice(&roots_entry);
        car[header_start + 1..header_end].copy_from_slice(&swapped);
    }
    fn chunk_payload(plan: &CarBuildPlan, payload: &[u8], index: usize) -> Vec<u8> {
        let chunk = &plan.chunks[index];
        let start = chunk.offset as usize;
        let end = start + chunk.length as usize;
        payload[start..end].to_vec()
    }
    fn block_car_for_indices(
        plan: &CarBuildPlan,
        payload: &[u8],
        indices: &[usize],
        file_path: Vec<String>,
    ) -> Vec<u8> {
        assert!(!indices.is_empty(), "block CAR fixture needs a chunk");
        let mut block_payload = Vec::new();
        let mut chunks = Vec::with_capacity(indices.len());
        for &index in indices {
            let source = &plan.chunks[index];
            let bytes = chunk_payload(plan, payload, index);
            let offset = u64::try_from(block_payload.len()).expect("block payload offset");
            block_payload.extend_from_slice(&bytes);
            chunks.push(CarChunk {
                offset,
                length: source.length,
                digest: source.digest,
                taikai_segment_hint: source.taikai_segment_hint.clone(),
            });
        }
        let block_len = u64::try_from(block_payload.len()).expect("block payload length");
        let block_plan = CarBuildPlan {
            chunk_profile: plan.chunk_profile,
            payload_digest: blake3_hash(&block_payload),
            content_length: block_len,
            chunks,
            files: vec![FilePlan {
                path: file_path,
                first_chunk: 0,
                chunk_count: indices.len(),
                size: block_len,
            }],
        };
        let mut car = Vec::new();
        CarWriter::new(&block_plan, &block_payload)
            .expect("block writer")
            .write_to(&mut car)
            .expect("write block CAR");
        car
    }
    fn first_distinct_adjacent_chunks(plan: &CarBuildPlan) -> usize {
        plan.chunks
            .windows(2)
            .position(|window| {
                window[0].digest != window[1].digest || window[0].length != window[1].length
            })
            .expect("fixture must contain adjacent distinct chunks")
    }
    fn bump_carv2_data_size_and_index(car: &mut [u8], delta: u64) {
        for field_offset in [24usize, 32] {
            let start = PRAGMA.len() + field_offset;
            let end = start + 8;
            let mut encoded = [0u8; 8];
            encoded.copy_from_slice(&car[start..end]);
            let updated = u64::from_le_bytes(encoded)
                .checked_add(delta)
                .expect("fixture CAR offset update");
            car[start..end].copy_from_slice(&updated.to_le_bytes());
        }
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
    fn canonical_car_verification_with_retained_plan_succeeds() {
        let payload = sample_payload();
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let mut car = Vec::new();
        let expected = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        let actual = CarVerifier::verify_canonical_car_with_plan(&plan, &car)
            .expect("retained plan and CAR must verify");
        assert_eq!(actual, expected);
    }
    #[test]
    fn retained_canonical_car_reopens_the_exact_multifile_payload_without_cloning() {
        let (plan, payload) = CarBuildPlan::from_files(vec![
            FileEntry {
                path: vec!["src".to_owned(), "lib.ko".to_owned()],
                data: b"fn main() {}\n".to_vec(),
            },
            FileEntry {
                path: vec!["tests".to_owned(), "basic.ko".to_owned()],
                data: b"test main\n".to_vec(),
            },
        ])
        .expect("multi-file plan");
        let mut car = Vec::new();
        let expected = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        let verified = CarVerifier::verify_canonical_car_with_plan_retained(&plan, &car)
            .expect("retain verified CAR");
        assert_eq!(verified.stats(), &expected);
        for _ in 0..2 {
            let mut reopened = Vec::new();
            verified
                .payload_reader()
                .read_to_end(&mut reopened)
                .expect("read authenticated payload");
            assert_eq!(reopened, payload);
        }
    }
    #[test]
    fn canonical_car_verification_rejects_substituted_plan_identity() {
        let payload = sample_payload();
        let mut plan = CarBuildPlan::single_file(&payload).expect("plan");
        let mut car = Vec::new();
        CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        plan.payload_digest = blake3_hash(b"substituted payload identity");
        let error = CarVerifier::verify_canonical_car_with_plan(&plan, &car)
            .expect_err("a substituted plan identity must fail");
        assert!(matches!(
            error,
            CarVerifyError::CanonicalCar(CarWriteError::PayloadDigestMismatch)
        ));
    }
    #[test]
    fn full_car_verification_reconstructs_plan_without_payload_clone() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        let manifest = build_manifest(&plan, &stats);
        let report = CarVerifier::verify_full_car(&manifest, &car).expect("verify full CAR");
        assert_eq!(report.stats, stats);
        assert_eq!(report.chunk_store.payload_len(), plan.content_length);
    }
    #[test]
    fn full_car_rejects_tampered_registered_profile_geometry() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        let mut manifest = build_manifest(&plan, &stats);
        manifest.chunking.max_size = manifest.chunking.max_size.saturating_sub(1);
        let error = CarVerifier::verify_full_car_with_plan(&manifest, &plan, &car)
            .expect_err("registered profile geometry mismatch must fail");
        assert!(matches!(error, CarVerifyError::ChunkProfileMismatch));
    }
    #[test]
    fn full_car_rejects_unknown_non_inline_profile_id() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        let mut manifest = build_manifest(&plan, &stats);
        manifest.chunking.profile_id = sorafs_manifest::ProfileId(u32::MAX);
        let error = CarVerifier::verify_full_car_with_plan(&manifest, &plan, &car)
            .expect_err("unknown registered profile must fail");
        assert!(matches!(error, CarVerifyError::ChunkProfileMismatch));
    }
    #[test]
    fn full_car_rejects_noncanonical_inline_profile_identity() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        let mut manifest = build_manifest(&plan, &stats);
        manifest.chunking.profile_id = sorafs_manifest::ProfileId(0);
        manifest.chunking.namespace = "untrusted".to_owned();
        let error = CarVerifier::verify_full_car_with_plan(&manifest, &plan, &car)
            .expect_err("noncanonical inline identity must fail");
        assert!(matches!(error, CarVerifyError::ChunkProfileMismatch));
    }
    #[test]
    fn full_car_rejects_noncanonical_header_key_order_even_when_manifest_binds_it() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        let mut manifest = build_manifest(&plan, &stats);
        swap_carv1_header_entries(&mut car);
        rebind_manifest_archive(&mut manifest, &car);
        let err = CarVerifier::verify_full_car_with_plan(&manifest, &plan, &car)
            .expect_err("noncanonical header ordering must fail");
        assert!(matches!(err, CarVerifyError::NonCanonicalCar));
    }
    #[test]
    fn full_car_rejects_noncanonical_reserved_header_bytes_even_when_manifest_binds_it() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        let mut manifest = build_manifest(&plan, &stats);
        car[PRAGMA.len() + 1] ^= 1;
        rebind_manifest_archive(&mut manifest, &car);
        let err = CarVerifier::verify_full_car_with_plan(&manifest, &plan, &car)
            .expect_err("noncanonical reserved characteristics must fail");
        assert!(matches!(err, CarVerifyError::NonCanonicalCar));
    }
    #[test]
    fn full_car_rejects_modified_index_even_when_manifest_binds_it() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        let mut manifest = build_manifest(&plan, &stats);
        let index_field = PRAGMA.len() + 32;
        let index_offset = usize::try_from(u64::from_le_bytes(
            car[index_field..index_field + 8]
                .try_into()
                .expect("index offset bytes"),
        ))
        .expect("index offset fits usize");
        car[index_offset] ^= 1;
        rebind_manifest_archive(&mut manifest, &car);
        let err = CarVerifier::verify_full_car_with_plan(&manifest, &plan, &car)
            .expect_err("modified canonical index must fail");
        assert!(matches!(err, CarVerifyError::NonCanonicalCar));
    }
    #[test]
    fn full_car_rejects_trailing_junk_even_when_manifest_binds_it() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        let mut manifest = build_manifest(&plan, &stats);
        car.extend_from_slice(b"manifest-bound-trailing-junk");
        rebind_manifest_archive(&mut manifest, &car);
        let err = CarVerifier::verify_full_car_with_plan(&manifest, &plan, &car)
            .expect_err("manifest-bound trailing bytes must fail");
        assert!(matches!(err, CarVerifyError::NonCanonicalCar));
    }
    #[test]
    fn parsed_payload_reader_reassembles_raw_sections() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car_bytes = Vec::new();
        CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car_bytes)
            .expect("write CAR");
        let parsed = ParsedCar::parse(&car_bytes).expect("parse CAR");
        let mut reader = parsed.payload_reader();
        let mut reconstructed = Vec::new();
        reader
            .read_to_end(&mut reconstructed)
            .expect("read parsed payload");
        assert_eq!(reconstructed, payload);
        assert_eq!(parsed.payload_digest(), blake3_hash(&payload));
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
        let expected_range = block_range_for_indices(&plan, 0, 1).expect("expected range");
        let report = CarVerifier::verify_block_car(&manifest, &plan, &range_car, expected_range)
            .expect("verify");
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
    fn block_car_rejects_unaligned_expected_range() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write full CAR");
        let manifest = build_manifest(&plan, &stats);
        let block_car = block_car_for_indices(&plan, &payload, &[0], Vec::new());
        let aligned = block_range_for_indices(&plan, 0, 1).expect("aligned range");
        let unaligned = aligned.start().checked_add(1).expect("unaligned start")..=*aligned.end();
        let err = CarVerifier::verify_block_car(&manifest, &plan, &block_car, unaligned)
            .expect_err("unaligned expected range must fail");
        assert!(matches!(
            err,
            CarVerifyError::ExpectedRangeNotChunkAligned { .. }
        ));
    }
    #[test]
    fn block_car_rejects_zero_length_plan_chunk() {
        let payload = sample_payload();
        let mut plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write full CAR");
        let manifest = build_manifest(&plan, &stats);
        let block_car = block_car_for_indices(&plan, &payload, &[0], Vec::new());
        let expected_range = block_range_for_indices(&plan, 0, 1).expect("expected range");
        plan.chunks[0].length = 0;
        let err = CarVerifier::verify_block_car(&manifest, &plan, &block_car, expected_range)
            .expect_err("zero-length plan chunk must fail");
        assert!(matches!(
            err,
            CarVerifyError::InvalidPlanChunkLength {
                chunk_index: 0,
                length: 0,
                ..
            }
        ));
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
        let expected_range = block_range_for_indices(&plan, 0, 3).expect("expected range");
        let err = CarVerifier::verify_block_car(&manifest, &plan, &range_car, expected_range)
            .expect_err("err");
        assert!(matches!(err, CarVerifyError::UnexpectedChunkOrder));
    }
    #[test]
    fn block_car_rejects_wrong_root() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write full CAR");
        let manifest = build_manifest(&plan, &stats);
        let mut block_car = block_car_for_indices(&plan, &payload, &[0], Vec::new());
        let root = ParsedCar::parse(&block_car).expect("parse fixture").roots()[0].clone();
        let root_offset = block_car
            .windows(root.len())
            .position(|window| window == root.as_slice())
            .expect("header root bytes");
        let last_root_byte = root_offset + root.len() - 1;
        block_car[last_root_byte] ^= 1;
        let expected_range = block_range_for_indices(&plan, 0, 1).expect("expected range");
        let err = CarVerifier::verify_block_car(&manifest, &plan, &block_car, expected_range)
            .expect_err("wrong root must fail");
        assert!(matches!(err, CarVerifyError::BlockRootMismatch));
    }
    #[test]
    fn block_car_rejects_reversed_raw_chunks() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write full CAR");
        let manifest = build_manifest(&plan, &stats);
        let start = first_distinct_adjacent_chunks(&plan);
        let block_car = block_car_for_indices(&plan, &payload, &[start + 1, start], Vec::new());
        let expected_range = block_range_for_indices(&plan, start, 2).expect("expected range");
        let err = CarVerifier::verify_block_car(&manifest, &plan, &block_car, expected_range)
            .expect_err("reversed chunks must fail");
        assert!(matches!(err, CarVerifyError::UnexpectedChunkOrder));
    }
    #[test]
    fn block_car_rejects_duplicate_raw_chunks() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write full CAR");
        let manifest = build_manifest(&plan, &stats);
        let start = first_distinct_adjacent_chunks(&plan);
        let block_car = block_car_for_indices(&plan, &payload, &[start, start], Vec::new());
        let expected_range = block_range_for_indices(&plan, start, 2).expect("expected range");
        let err = CarVerifier::verify_block_car(&manifest, &plan, &block_car, expected_range)
            .expect_err("duplicate chunks must fail");
        assert!(matches!(err, CarVerifyError::UnexpectedChunkOrder));
    }
    #[test]
    fn block_car_rejects_unauthorized_dag_shape() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write full CAR");
        let manifest = build_manifest(&plan, &stats);
        let mut block_car = block_car_for_indices(&plan, &payload, &[0], Vec::new());
        let index_field = PRAGMA.len() + 32;
        let index_offset = usize::try_from(u64::from_le_bytes(
            block_car[index_field..index_field + 8]
                .try_into()
                .expect("index offset bytes"),
        ))
        .expect("index offset fits usize");
        let dag_data = [0xa0]; // canonical empty CBOR map
        let dag_digest = hash_to_array(blake3_hash(&dag_data));
        let dag_cid = encode_cid(DAG_CBOR_CODEC, &dag_digest);
        let dag_section_len =
            u64::try_from(dag_cid.len() + dag_data.len()).expect("DAG section length fits u64");
        let mut extra_section = crate::encode_uleb128_vec(dag_section_len);
        extra_section.extend_from_slice(&dag_cid);
        extra_section.extend_from_slice(&dag_data);
        let extra_len = u64::try_from(extra_section.len()).expect("extra section length fits u64");
        block_car.splice(index_offset..index_offset, extra_section);
        bump_carv2_data_size_and_index(&mut block_car, extra_len);
        let expected_range = block_range_for_indices(&plan, 0, 1).expect("expected range");
        let err = CarVerifier::verify_block_car(&manifest, &plan, &block_car, expected_range)
            .expect_err("extra DAG section must fail");
        assert!(matches!(err, CarVerifyError::NonCanonicalCar));
    }
    #[test]
    fn block_car_rejects_modified_index() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write full CAR");
        let manifest = build_manifest(&plan, &stats);
        let mut block_car = block_car_for_indices(&plan, &payload, &[0], Vec::new());
        let index_field = PRAGMA.len() + 32;
        let index_offset = usize::try_from(u64::from_le_bytes(
            block_car[index_field..index_field + 8]
                .try_into()
                .expect("index offset bytes"),
        ))
        .expect("index offset fits usize");
        assert!(
            index_offset < block_car.len(),
            "fixture must contain an index"
        );
        block_car[index_offset] ^= 1;
        let expected_range = block_range_for_indices(&plan, 0, 1).expect("expected range");
        let err = CarVerifier::verify_block_car(&manifest, &plan, &block_car, expected_range)
            .expect_err("modified index must fail");
        assert!(matches!(err, CarVerifyError::NonCanonicalCar));
    }
    #[test]
    fn block_car_rejects_trailing_junk() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write full CAR");
        let manifest = build_manifest(&plan, &stats);
        let mut block_car = block_car_for_indices(&plan, &payload, &[0], Vec::new());
        block_car.extend_from_slice(b"trailing-junk");
        let expected_range = block_range_for_indices(&plan, 0, 1).expect("expected range");
        let err = CarVerifier::verify_block_car(&manifest, &plan, &block_car, expected_range)
            .expect_err("trailing bytes must fail");
        assert!(matches!(err, CarVerifyError::NonCanonicalCar));
    }
    #[test]
    fn block_car_rejects_nonminimal_header_varint() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write full CAR");
        let manifest = build_manifest(&plan, &stats);
        let mut block_car = block_car_for_indices(&plan, &payload, &[0], Vec::new());
        let header_len_offset = PRAGMA.len() + HEADER_LEN;
        let header_len = block_car[header_len_offset];
        assert_eq!(
            header_len & 0x80,
            0,
            "fixture header length must be one byte"
        );
        block_car[header_len_offset] = header_len | 0x80;
        block_car.insert(header_len_offset + 1, 0);
        bump_carv2_data_size_and_index(&mut block_car, 1);
        let expected_range = block_range_for_indices(&plan, 0, 1).expect("expected range");
        let err = CarVerifier::verify_block_car(&manifest, &plan, &block_car, expected_range)
            .expect_err("nonminimal varint must fail");
        assert!(matches!(err, CarVerifyError::NonCanonicalVarint));
    }
    #[test]
    fn block_car_rejects_overflowing_header_varint() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write full CAR");
        let manifest = build_manifest(&plan, &stats);
        let mut block_car = block_car_for_indices(&plan, &payload, &[0], Vec::new());
        let header_len_offset = PRAGMA.len() + HEADER_LEN;
        let mut overflow = vec![0xff; 9];
        overflow.push(0x02);
        block_car.splice(header_len_offset..=header_len_offset, overflow);
        bump_carv2_data_size_and_index(&mut block_car, 9);
        let expected_range = block_range_for_indices(&plan, 0, 1).expect("expected range");
        let err = CarVerifier::verify_block_car(&manifest, &plan, &block_car, expected_range)
            .expect_err("overflowing varint must fail");
        assert!(matches!(err, CarVerifyError::VarintOverflow));
    }
    #[test]
    fn block_car_rejects_nonminimal_cbor_length() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut full_car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut full_car)
            .expect("write full CAR");
        let manifest = build_manifest(&plan, &stats);
        let mut block_car = block_car_for_indices(&plan, &payload, &[0], Vec::new());
        let header_len_offset = PRAGMA.len() + HEADER_LEN;
        let header_body_offset = header_len_offset + 1;
        assert_eq!(block_car[header_body_offset], 0xa2);
        block_car.splice(header_body_offset..=header_body_offset, [0xb8, 0x02]);
        block_car[header_len_offset] = block_car[header_len_offset]
            .checked_add(1)
            .expect("fixture header length");
        bump_carv2_data_size_and_index(&mut block_car, 1);
        let expected_range = block_range_for_indices(&plan, 0, 1).expect("expected range");
        let err = CarVerifier::verify_block_car(&manifest, &plan, &block_car, expected_range)
            .expect_err("nonminimal CBOR length must fail");
        assert!(matches!(
            err,
            CarVerifyError::InvalidCarv1Header("non-canonical CBOR length")
        ));
    }
    #[test]
    fn parser_rejects_oversized_carv1_header_before_buffering() {
        let payload = sample_payload();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car = Vec::new();
        CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        let header_len_offset = PRAGMA.len() + HEADER_LEN;
        let (_, encoded_len) = decode_uleb128(&car[header_len_offset..]).expect("header length");
        let oversized = crate::encode_uleb128_vec(
            u64::try_from(MAX_CARV1_HEADER_SIZE + 1).expect("header bound fits u64"),
        );
        car.splice(
            header_len_offset..header_len_offset + encoded_len,
            oversized,
        );
        let err = ParsedCar::parse(&car).expect_err("oversized header must fail");
        assert!(matches!(
            err,
            CarVerifyError::InvalidCarv1Header("header exceeds maximum size")
        ));
    }
    #[test]
    fn parser_rejects_oversized_raw_section_before_hashing() {
        let payload = b"canonical CAR template";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut car = Vec::new();
        CarWriter::new(&plan, payload)
            .expect("writer")
            .write_to(&mut car)
            .expect("write CAR");
        let data_offset = PRAGMA.len() + HEADER_LEN;
        let (header_len, header_len_bytes) =
            decode_uleb128(&car[data_offset..]).expect("CARv1 header length");
        let section_start = data_offset
            + header_len_bytes
            + usize::try_from(header_len).expect("header length fits usize");
        let old_index_offset = usize::try_from(u64::from_le_bytes(
            car[PRAGMA.len() + 32..PRAGMA.len() + 40]
                .try_into()
                .expect("index offset bytes"),
        ))
        .expect("index offset fits usize");
        let index = car[old_index_offset..].to_vec();
        let raw_len = crate::CHUNK_STORE_MAX_CHUNK_BYTES as usize + 1;
        let cid = encode_cid(RAW_CODEC, &[0u8; 32]);
        let section_len = cid.len().checked_add(raw_len).expect("section length");
        let mut section =
            crate::encode_uleb128_vec(u64::try_from(section_len).expect("section length fits u64"));
        section.extend_from_slice(&cid);
        section.resize(section.len() + raw_len, 0);
        car.truncate(section_start);
        car.extend_from_slice(&section);
        let new_index_offset = car.len();
        car.extend_from_slice(&index);
        let data_size = new_index_offset
            .checked_sub(data_offset)
            .expect("data size");
        car[PRAGMA.len() + 24..PRAGMA.len() + 32].copy_from_slice(
            &u64::try_from(data_size)
                .expect("data size fits u64")
                .to_le_bytes(),
        );
        car[PRAGMA.len() + 32..PRAGMA.len() + 40].copy_from_slice(
            &u64::try_from(new_index_offset)
                .expect("index offset fits u64")
                .to_le_bytes(),
        );

        let err = ParsedCar::parse(&car).expect_err("oversized raw section must fail");

        assert!(matches!(
            err,
            CarVerifyError::ChunkSizeExceeded {
                section_index: 0,
                len,
                max,
            } if len == u64::from(crate::CHUNK_STORE_MAX_CHUNK_BYTES) + 1
                && max == u64::from(crate::CHUNK_STORE_MAX_CHUNK_BYTES)
        ));
    }
    #[test]
    fn integer_decoders_reject_nonminimal_and_overflow_lengths() {
        assert_eq!(
            decode_uleb128(&[0x80, 0x00]),
            Err(Uleb128Error::NonCanonical)
        );
        assert_eq!(
            decode_uleb128(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02]),
            Err(Uleb128Error::Overflow)
        );
        assert_eq!(
            decode_uleb128(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01]),
            Ok((u64::MAX, 10))
        );
        for encoded in [
            vec![0x18, 0x17],
            vec![0x19, 0x00, 0xff],
            vec![0x1a, 0x00, 0x00, 0xff, 0xff],
            vec![0x1b, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff],
        ] {
            assert!(matches!(
                decode_cbor_uint(&encoded),
                Err(CarVerifyError::InvalidCarv1Header(
                    "non-canonical CBOR length"
                ))
            ));
        }
        let mut overflowing_bytes = vec![0x5b];
        overflowing_bytes.extend_from_slice(&u64::MAX.to_be_bytes());
        assert!(matches!(
            decode_cbor_bytes(&overflowing_bytes),
            Err(CarVerifyError::InvalidCarv1Header("bytes length overflow"))
        ));
    }
}
