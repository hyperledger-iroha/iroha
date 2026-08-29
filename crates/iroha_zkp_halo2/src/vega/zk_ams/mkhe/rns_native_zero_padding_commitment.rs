//! Private zero-padding commitment prerequisite for the RNS-native proof.
//!
//! The verifier authenticates the exact padding geometry of the three partial
//! source records (`X`, `rE`, and `rW`) across all forty RNS limbs.  Each limb
//! contains 191 ordered 1,024-column Pedersen commitment chunks.  After those
//! 7,640 commitments are bound by the transcript's zero-padding root, the
//! post-root composite challenge selects a random linear combination per limb.
//! A Schnorr proof then shows that every aggregate has an opening using only
//! the fixed hiding generator.  Under commitment binding this proves that the
//! committed padding inventory is zero, with at most degree-190 cancellation
//! probability per limb.
//!
//! This prerequisite deliberately does not prove that the commitments are the
//! padding lanes of the authenticated source, global lookup, or terminal
//! materialization. The live-source replay is the padding authority; this
//! detached inventory is redundant, non-authoritative compatibility material
//! pending schema retirement. Its move-only result therefore grants no
//! composite, readiness, or release authority.

use super::{
    manifest::ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1,
        ZkAmsMkheRnsNativeFamilyV1,
    },
    rns_native_transcript::ZkAmsMkheRnsNativeChallengeSeedsV1,
    rns_native_wire::ZK_AMS_MKHE_RNS_NATIVE_ZERO_PADDING_SECTION_MAX_BYTES_V1,
};
use crate::{
    generalized_bulletproof::{ProofSuite, multiexp},
    vega::{
        VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZkAmsT256BulletproofSuiteV1},
        sponge::Keccak256,
    },
};

const CODEC_TAG_V1: [u8; 4] = *b"ZZPC";
const CODEC_VERSION_V1: u8 = 1;
const CODEC_FLAGS_V1: u8 = 0;
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const SCALAR_BYTES_V1: usize = 32;
const SEGMENT_COUNT_V1: usize = 3;
const CHUNK_COLUMNS_V1: usize = 1_024;
const X_RECORD_ORDINAL_V1: u8 = 0;
const RE_RECORD_ORDINAL_V1: u8 = 33;
const RW_RECORD_ORDINAL_V1: u8 = 42;
const X_USED_SLOTS_V1: usize = 89;
const RE_USED_SLOTS_V1: usize = 1_024;
const RW_USED_SLOTS_V1: usize = 512;
const X_PADDING_SLOTS_V1: usize = ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1 - X_USED_SLOTS_V1;
const RE_PADDING_SLOTS_V1: usize = ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1 - RE_USED_SLOTS_V1;
const RW_PADDING_SLOTS_V1: usize = ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1 - RW_USED_SLOTS_V1;
const X_CHUNKS_V1: usize = X_PADDING_SLOTS_V1.div_ceil(CHUNK_COLUMNS_V1);
const RE_CHUNKS_V1: usize = RE_PADDING_SLOTS_V1.div_ceil(CHUNK_COLUMNS_V1);
const RW_CHUNKS_V1: usize = RW_PADDING_SLOTS_V1.div_ceil(CHUNK_COLUMNS_V1);
const CHUNKS_PER_LIMB_V1: usize = X_CHUNKS_V1 + RE_CHUNKS_V1 + RW_CHUNKS_V1;
const COMMITMENT_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * CHUNKS_PER_LIMB_V1;
const SCHNORR_PROOF_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
const HEADER_DIGEST_COUNT_V1: usize = 4;
const HEADER_BYTES_V1: usize = 4 + 4 + 2 + 2 + 4 + 4 + HEADER_DIGEST_COUNT_V1 * DIGEST_BYTES_V1;
const COMMITMENTS_BYTES_V1: usize = COMMITMENT_COUNT_V1 * POINT_BYTES_V1;
const MASK_POINTS_BYTES_V1: usize = SCHNORR_PROOF_COUNT_V1 * POINT_BYTES_V1;
const RESPONSES_BYTES_V1: usize = SCHNORR_PROOF_COUNT_V1 * SCALAR_BYTES_V1;
const COMMITMENTS_OFFSET_V1: usize = HEADER_BYTES_V1;
const MASK_POINTS_OFFSET_V1: usize = COMMITMENTS_OFFSET_V1 + COMMITMENTS_BYTES_V1;
const RESPONSES_OFFSET_V1: usize = MASK_POINTS_OFFSET_V1 + MASK_POINTS_BYTES_V1;
const CODEC_DIGEST_OFFSET_V1: usize = RESPONSES_OFFSET_V1 + RESPONSES_BYTES_V1;
const EXACT_CODEC_BYTES_V1: usize = CODEC_DIGEST_OFFSET_V1 + DIGEST_BYTES_V1;
const MAX_SCALAR_ATTEMPTS_V1: u8 = 128;
const MAX_BOUND_DIGESTS_V1: usize = 256;

const LIMB_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-zero-padding.limb-commitments";
const PADDING_ROOT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-zero-padding.root";
const POINT_SET_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-zero-padding.point-set";
const CONTEXT_BINDING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-zero-padding.context";
const AGGREGATION_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-zero-padding.aggregation";
const SCHNORR_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-zero-padding.schnorr";
const SCHNORR_PROOF_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-zero-padding.schnorr-proof";
const CODEC_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-zero-padding.codec";

const _: () = {
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1 == 65_536);
    assert!(X_PADDING_SLOTS_V1 == 65_447);
    assert!(RE_PADDING_SLOTS_V1 == 64_512);
    assert!(RW_PADDING_SLOTS_V1 == 65_024);
    assert!(X_CHUNKS_V1 == 64);
    assert!(RE_CHUNKS_V1 == 63);
    assert!(RW_CHUNKS_V1 == 64);
    assert!(CHUNKS_PER_LIMB_V1 == 191);
    assert!(COMMITMENT_COUNT_V1 == 7_640);
    assert!(HEADER_BYTES_V1 == 148);
    assert!(COMMITMENTS_BYTES_V1 == 252_120);
    assert!(EXACT_CODEC_BYTES_V1 == 254_900);
    assert!(
        EXACT_CODEC_BYTES_V1 <= ZK_AMS_MKHE_RNS_NATIVE_ZERO_PADDING_SECTION_MAX_BYTES_V1 as usize
    );
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeZeroPaddingCommitmentErrorV1 {
    CapExceeded,
    InvalidEncoding,
    InvalidGeometry,
    ContextMismatch,
    AliasedDigest,
    Integrity,
    InvalidPoint,
    InvalidScalar,
    InvalidProof,
    Allocation,
}

impl core::fmt::Display for RnsNativeZeroPaddingCommitmentErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeZeroPaddingCommitmentErrorV1 {}

/// Move-only evidence that only the detached zero-padding inventory passed.
///
/// The token is redundant and non-authoritative: it carries no
/// source/global-lookup/terminal linkage and cannot authorize composite
/// verification, readiness, or release.
#[allow(
    missing_copy_implementations,
    reason = "the non-authorizing prerequisite is consumed by the future source-link stage"
)]
pub(super) struct RnsNativeZeroPaddingCommitmentPrerequisiteV1 {
    binding_digest: [u8; DIGEST_BYTES_V1],
    point_set_digest: [u8; DIGEST_BYTES_V1],
    root: [u8; DIGEST_BYTES_V1],
    proof_digest: [u8; DIGEST_BYTES_V1],
    limb_padding_digests: [[u8; DIGEST_BYTES_V1]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
}

/// Opaque evidence that the retained zero-padding prerequisite was validated
/// against one exact final transcript.
///
/// The recomputed root and final-transcript tag are deliberately private.  The
/// transcript module can consume this value only through the equality check
/// below; no raw root, tag, constructor, or aggregate proof owner is exposed.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "verified zero-root evidence is consumed exactly once by its transcript obligation"
)]
#[must_use = "verified zero-root evidence must be consumed by the exact terminal chronology"]
pub(super) struct RnsNativeVerifiedZeroPaddingRootV1 {
    root: [u8; DIGEST_BYTES_V1],
    final_transcript_tag: [u8; DIGEST_BYTES_V1],
}

impl RnsNativeVerifiedZeroPaddingRootV1 {
    /// Compare against private transcript-owned values without exposing either
    /// digest carried by this evidence.
    pub(super) fn matches_claimed_zero_padding_root_v1(
        self,
        claimed_root: [u8; DIGEST_BYTES_V1],
        expected_final_transcript_tag: [u8; DIGEST_BYTES_V1],
    ) -> bool {
        self.root != [0; DIGEST_BYTES_V1]
            && self.final_transcript_tag != [0; DIGEST_BYTES_V1]
            && self.root != self.final_transcript_tag
            && self.root == claimed_root
            && self.final_transcript_tag == expected_final_transcript_tag
    }
}

impl RnsNativeZeroPaddingCommitmentPrerequisiteV1 {
    pub(super) const fn binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.binding_digest
    }

    pub(super) const fn point_set_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.point_set_digest
    }

    pub(super) const fn root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.root
    }

    pub(super) const fn proof_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.proof_digest
    }

    pub(super) const fn limb_padding_digests(
        &self,
    ) -> &[[u8; DIGEST_BYTES_V1]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1] {
        &self.limb_padding_digests
    }

    pub(super) fn validate_context_v1(
        &self,
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<(), RnsNativeZeroPaddingCommitmentErrorV1> {
        if self.root != transcript.zero_padding_root()
            || self.binding_digest
                != context_binding_digest_v1(
                    transcript,
                    self.point_set_digest,
                    &self.limb_padding_digests,
                )?
        {
            return Err(RnsNativeZeroPaddingCommitmentErrorV1::ContextMismatch);
        }
        Ok(())
    }

    /// Mint opaque equality evidence only after revalidating this exact
    /// prerequisite against the supplied final transcript.
    pub(super) fn verified_zero_padding_root_v1(
        &self,
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<RnsNativeVerifiedZeroPaddingRootV1, RnsNativeZeroPaddingCommitmentErrorV1> {
        self.validate_context_v1(transcript)?;
        let final_transcript_tag = transcript.transcript_digest();
        if self.root == [0; DIGEST_BYTES_V1]
            || final_transcript_tag == [0; DIGEST_BYTES_V1]
            || self.root == final_transcript_tag
        {
            return Err(RnsNativeZeroPaddingCommitmentErrorV1::ContextMismatch);
        }
        Ok(RnsNativeVerifiedZeroPaddingRootV1 {
            root: self.root,
            final_transcript_tag,
        })
    }
}

#[cfg(test)]
pub(super) fn verified_zero_padding_root_fixture_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
) -> Result<RnsNativeVerifiedZeroPaddingRootV1, RnsNativeZeroPaddingCommitmentErrorV1> {
    let limb_padding_digests =
        core::array::from_fn(|limb| [(limb as u8).wrapping_add(1); DIGEST_BYTES_V1]);
    let point_set_digest = [0xe1; DIGEST_BYTES_V1];
    let prerequisite = RnsNativeZeroPaddingCommitmentPrerequisiteV1 {
        binding_digest: context_binding_digest_v1(
            transcript,
            point_set_digest,
            &limb_padding_digests,
        )?,
        point_set_digest,
        root: transcript.zero_padding_root(),
        proof_digest: [0xe2; DIGEST_BYTES_V1],
        limb_padding_digests,
    };
    prerequisite.verified_zero_padding_root_v1(transcript)
}

#[derive(Clone, Copy)]
struct PaddingSegmentV1 {
    record_ordinal: u8,
    family: ZkAmsMkheRnsNativeFamilyV1,
    first_slot: u32,
    slot_count: u32,
    chunk_count: u16,
}

const PADDING_SEGMENTS_V1: [PaddingSegmentV1; SEGMENT_COUNT_V1] = [
    PaddingSegmentV1 {
        record_ordinal: X_RECORD_ORDINAL_V1,
        family: ZkAmsMkheRnsNativeFamilyV1::X,
        first_slot: X_USED_SLOTS_V1 as u32,
        slot_count: X_PADDING_SLOTS_V1 as u32,
        chunk_count: X_CHUNKS_V1 as u16,
    },
    PaddingSegmentV1 {
        record_ordinal: RE_RECORD_ORDINAL_V1,
        family: ZkAmsMkheRnsNativeFamilyV1::RE,
        first_slot: RE_USED_SLOTS_V1 as u32,
        slot_count: RE_PADDING_SLOTS_V1 as u32,
        chunk_count: RE_CHUNKS_V1 as u16,
    },
    PaddingSegmentV1 {
        record_ordinal: RW_RECORD_ORDINAL_V1,
        family: ZkAmsMkheRnsNativeFamilyV1::RW,
        first_slot: RW_USED_SLOTS_V1 as u32,
        slot_count: RW_PADDING_SLOTS_V1 as u32,
        chunk_count: RW_CHUNKS_V1 as u16,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PaddingChunkV1 {
    segment_ordinal: u8,
    record_ordinal: u8,
    family: ZkAmsMkheRnsNativeFamilyV1,
    chunk_in_segment: u16,
    first_slot: u32,
    slot_count: u16,
}

fn padding_chunk_v1(
    ordinal: usize,
) -> Result<PaddingChunkV1, RnsNativeZeroPaddingCommitmentErrorV1> {
    if ordinal >= CHUNKS_PER_LIMB_V1 {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry);
    }
    let mut remaining = ordinal;
    for (segment_ordinal, segment) in PADDING_SEGMENTS_V1.iter().copied().enumerate() {
        let chunk_count = usize::from(segment.chunk_count);
        if remaining < chunk_count {
            let consumed = remaining
                .checked_mul(CHUNK_COLUMNS_V1)
                .ok_or(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?;
            let total = usize::try_from(segment.slot_count)
                .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?;
            let slot_count = total
                .checked_sub(consumed)
                .ok_or(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?
                .min(CHUNK_COLUMNS_V1);
            let first_slot = usize::try_from(segment.first_slot)
                .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?
                .checked_add(consumed)
                .ok_or(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?;
            return Ok(PaddingChunkV1 {
                segment_ordinal: u8::try_from(segment_ordinal)
                    .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?,
                record_ordinal: segment.record_ordinal,
                family: segment.family,
                chunk_in_segment: u16::try_from(remaining)
                    .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?,
                first_slot: u32::try_from(first_slot)
                    .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?,
                slot_count: u16::try_from(slot_count)
                    .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?,
            });
        }
        remaining = remaining
            .checked_sub(chunk_count)
            .ok_or(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?;
    }
    Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)
}

struct DecodedProofV1<'a> {
    binding_digest: [u8; 32],
    point_set_digest: [u8; 32],
    expected_root: [u8; 32],
    schnorr_proof_digest: [u8; 32],
    commitment_bytes: &'a [u8],
    mask_point_bytes: &'a [u8],
    response_bytes: &'a [u8],
    codec_digest: [u8; 32],
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], RnsNativeZeroPaddingCommitmentErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RnsNativeZeroPaddingCommitmentErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeZeroPaddingCommitmentErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeZeroPaddingCommitmentErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeZeroPaddingCommitmentErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn finish(self) -> Result<(), RnsNativeZeroPaddingCommitmentErrorV1> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding)
        }
    }
}

struct DigestRegistryV1 {
    digests: [[u8; 32]; MAX_BOUND_DIGESTS_V1],
    len: usize,
}

impl DigestRegistryV1 {
    const fn new() -> Self {
        Self {
            digests: [[0; 32]; MAX_BOUND_DIGESTS_V1],
            len: 0,
        }
    }

    fn insert(&mut self, digest: [u8; 32]) -> Result<(), RnsNativeZeroPaddingCommitmentErrorV1> {
        if digest == [0; 32] || self.digests[..self.len].contains(&digest) {
            return Err(RnsNativeZeroPaddingCommitmentErrorV1::AliasedDigest);
        }
        let destination = self
            .digests
            .get_mut(self.len)
            .ok_or(RnsNativeZeroPaddingCommitmentErrorV1::AliasedDigest)?;
        *destination = digest;
        self.len += 1;
        Ok(())
    }
}

/// Authenticate the exact forty-limb detached zero-padding prerequisite.
///
/// The section cap and exact width are checked before allocating point vectors.
/// Success proves only the hiding-only aggregate equations for the committed
/// inventory. It is redundant compatibility evidence and intentionally conveys
/// no source, padding, or composite authority.
pub(super) fn authenticate_rns_native_zero_padding_commitments_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    limb_padding_digests: &[[u8; 32]],
    bytes: &[u8],
) -> Result<RnsNativeZeroPaddingCommitmentPrerequisiteV1, RnsNativeZeroPaddingCommitmentErrorV1> {
    if limb_padding_digests.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry);
    }
    let decoded = decode_exact_v1(bytes)?;
    if decoded.expected_root != transcript.zero_padding_root() {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::ContextMismatch);
    }
    if decoded.point_set_digest != point_set_digest_v1(decoded.commitment_bytes)?
        || decoded.schnorr_proof_digest
            != schnorr_proof_digest_v1(decoded.mask_point_bytes, decoded.response_bytes)?
        || decoded.codec_digest != codec_digest_v1(&bytes[..CODEC_DIGEST_OFFSET_V1])
    {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::Integrity);
    }
    let computed_limb_digests = limb_digests_v1(decoded.commitment_bytes)?;
    if computed_limb_digests.as_slice() != limb_padding_digests
        || padding_root_v1(transcript, &computed_limb_digests)? != decoded.expected_root
    {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::ContextMismatch);
    }
    let expected_binding =
        context_binding_digest_v1(transcript, decoded.point_set_digest, &computed_limb_digests)?;
    if decoded.binding_digest != expected_binding {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::ContextMismatch);
    }
    validate_global_digest_aliases_v1(transcript, &decoded, &computed_limb_digests)?;
    let commitments = decode_points_v1(decoded.commitment_bytes, COMMITMENT_COUNT_V1)?;
    let masks = decode_points_v1(decoded.mask_point_bytes, SCHNORR_PROOF_COUNT_V1)?;
    let responses = decode_scalars_v1(decoded.response_bytes)?;
    verify_schnorr_equations_v1(
        transcript,
        decoded.binding_digest,
        decoded.point_set_digest,
        &commitments,
        &masks,
        &responses,
    )?;
    Ok(RnsNativeZeroPaddingCommitmentPrerequisiteV1 {
        binding_digest: decoded.binding_digest,
        point_set_digest: decoded.point_set_digest,
        root: decoded.expected_root,
        proof_digest: decoded.schnorr_proof_digest,
        limb_padding_digests: computed_limb_digests,
    })
}

fn decode_exact_v1(
    bytes: &[u8],
) -> Result<DecodedProofV1<'_>, RnsNativeZeroPaddingCommitmentErrorV1> {
    if bytes.len()
        > usize::try_from(ZK_AMS_MKHE_RNS_NATIVE_ZERO_PADDING_SECTION_MAX_BYTES_V1)
            .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::CapExceeded)?
    {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::CapExceeded);
    }
    if bytes.len() != EXACT_CODEC_BYTES_V1 {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding);
    }
    let mut decoder = DecoderV1::new(bytes);
    if decoder.array::<4>()? != CODEC_TAG_V1
        || decoder.u8()? != CODEC_VERSION_V1
        || decoder.u8()? != CODEC_FLAGS_V1
        || usize::from(decoder.u8()?) != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || usize::from(decoder.u8()?) != SEGMENT_COUNT_V1
        || usize::from(decoder.u16()?) != CHUNK_COLUMNS_V1
        || usize::from(decoder.u16()?) != CHUNKS_PER_LIMB_V1
        || usize::try_from(decoder.u32()?).ok() != Some(COMMITMENT_COUNT_V1)
        || usize::from(decoder.u8()?) != SCHNORR_PROOF_COUNT_V1
        || usize::from(decoder.u8()?) != POINT_BYTES_V1
        || usize::from(decoder.u8()?) != SCALAR_BYTES_V1
        || decoder.u8()? != 0
    {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding);
    }
    let binding_digest = decoder.array()?;
    let point_set_digest = decoder.array()?;
    let expected_root = decoder.array()?;
    let schnorr_proof_digest = decoder.array()?;
    let commitment_bytes = decoder.take(COMMITMENTS_BYTES_V1)?;
    let mask_point_bytes = decoder.take(MASK_POINTS_BYTES_V1)?;
    let response_bytes = decoder.take(RESPONSES_BYTES_V1)?;
    let codec_digest = decoder.array()?;
    decoder.finish()?;
    Ok(DecodedProofV1 {
        binding_digest,
        point_set_digest,
        expected_root,
        schnorr_proof_digest,
        commitment_bytes,
        mask_point_bytes,
        response_bytes,
        codec_digest,
    })
}

fn decode_points_v1(
    bytes: &[u8],
    count: usize,
) -> Result<Vec<Point>, RnsNativeZeroPaddingCommitmentErrorV1> {
    let expected = count
        .checked_mul(POINT_BYTES_V1)
        .ok_or(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding)?;
    if bytes.len() != expected {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding);
    }
    let mut points = Vec::new();
    points
        .try_reserve_exact(count)
        .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::Allocation)?;
    for encoded in bytes.chunks_exact(POINT_BYTES_V1) {
        points.push(
            Point::from_non_identity_wire_bytes_exact(encoded)
                .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidPoint)?,
        );
    }
    if points.len() != count {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding);
    }
    Ok(points)
}

fn decode_scalars_v1(
    bytes: &[u8],
) -> Result<[Scalar; SCHNORR_PROOF_COUNT_V1], RnsNativeZeroPaddingCommitmentErrorV1> {
    if bytes.len() != RESPONSES_BYTES_V1 {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding);
    }
    let mut responses = [Scalar::zero(); SCHNORR_PROOF_COUNT_V1];
    for (destination, encoded) in responses
        .iter_mut()
        .zip(bytes.chunks_exact(SCALAR_BYTES_V1))
    {
        let encoded: [u8; SCALAR_BYTES_V1] = encoded
            .try_into()
            .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidScalar)?;
        *destination = Scalar::from_le_bytes_exact(encoded)
            .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidScalar)?;
    }
    Ok(responses)
}

fn limb_digests_v1(
    commitment_bytes: &[u8],
) -> Result<[[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1], RnsNativeZeroPaddingCommitmentErrorV1> {
    if commitment_bytes.len() != COMMITMENTS_BYTES_V1 {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding);
    }
    let mut digests = [[0; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1];
    for (limb, destination) in digests.iter_mut().enumerate() {
        let start = limb
            .checked_mul(CHUNKS_PER_LIMB_V1)
            .and_then(|value| value.checked_mul(POINT_BYTES_V1))
            .ok_or(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?;
        let end = start
            .checked_add(CHUNKS_PER_LIMB_V1 * POINT_BYTES_V1)
            .ok_or(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?;
        *destination = limb_digest_v1(
            limb,
            commitment_bytes
                .get(start..end)
                .ok_or(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding)?,
        )?;
    }
    Ok(digests)
}

fn limb_digest_v1(
    limb: usize,
    point_bytes: &[u8],
) -> Result<[u8; 32], RnsNativeZeroPaddingCommitmentErrorV1> {
    if limb >= ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || point_bytes.len() != CHUNKS_PER_LIMB_V1 * POINT_BYTES_V1
    {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(LIMB_COMMITMENT_DOMAIN_V1);
    hash.update(&[CODEC_VERSION_V1]);
    hash.update(
        &u8::try_from(limb)
            .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?
            .to_be_bytes(),
    );
    hash.update(&ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb].to_be_bytes());
    hash.update(&(CHUNK_COLUMNS_V1 as u16).to_be_bytes());
    hash.update(&(CHUNKS_PER_LIMB_V1 as u16).to_be_bytes());
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    for (chunk_ordinal, point) in point_bytes.chunks_exact(POINT_BYTES_V1).enumerate() {
        let descriptor = padding_chunk_v1(chunk_ordinal)?;
        hash.update(
            &u16::try_from(chunk_ordinal)
                .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?
                .to_be_bytes(),
        );
        hash.update(&[
            descriptor.segment_ordinal,
            descriptor.record_ordinal,
            descriptor.family as u8,
        ]);
        hash.update(&descriptor.chunk_in_segment.to_be_bytes());
        hash.update(&descriptor.first_slot.to_be_bytes());
        hash.update(&descriptor.slot_count.to_be_bytes());
        hash.update(point);
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::Integrity);
    }
    Ok(digest)
}

fn padding_root_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    limb_digests: &[[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
) -> Result<[u8; 32], RnsNativeZeroPaddingCommitmentErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(PADDING_ROOT_DOMAIN_V1);
    hash.update(&[CODEC_VERSION_V1]);
    hash.update(&(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u16).to_be_bytes());
    hash.update(&(CHUNKS_PER_LIMB_V1 as u16).to_be_bytes());
    hash.update(&(CHUNK_COLUMNS_V1 as u16).to_be_bytes());
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    for digest in context_identities_v1(transcript) {
        hash.update(&digest);
    }
    hash.update(
        &u16::try_from(transcript.opening_commitments().len())
            .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?
            .to_be_bytes(),
    );
    for (ordinal, opening) in transcript.opening_commitments().iter().copied().enumerate() {
        hash.update(
            &u16::try_from(ordinal)
                .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?
                .to_be_bytes(),
        );
        hash.update(&[opening.family() as u8, opening.family_index()]);
        hash.update(&opening.source_commitment_digest());
        hash.update(&opening.hyrax_commitment_digest());
    }
    for digest in [
        transcript.mapping_root(),
        transcript.terminal_hyrax_root(),
        transcript.cross_basis_bridge_root(),
        transcript.qpcs_initial_root(),
        transcript.qpcs_quotient_root(),
    ] {
        hash.update(&digest);
    }
    for root in transcript.qpcs_fri_roots() {
        hash.update(&[root.layer()]);
        hash.update(&root.root());
    }
    hash.update(&transcript.cross_field_root());
    hash.update(&transcript.global_lookup_root());
    for (limb, digest) in limb_digests.iter().enumerate() {
        if *digest == [0; 32] {
            return Err(RnsNativeZeroPaddingCommitmentErrorV1::Integrity);
        }
        hash.update(
            &u8::try_from(limb)
                .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?
                .to_be_bytes(),
        );
        hash.update(&ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb].to_be_bytes());
        hash.update(digest);
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::Integrity);
    }
    Ok(digest)
}

fn point_set_digest_v1(
    commitment_bytes: &[u8],
) -> Result<[u8; 32], RnsNativeZeroPaddingCommitmentErrorV1> {
    if commitment_bytes.len() != COMMITMENTS_BYTES_V1 {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding);
    }
    let mut hash = Keccak256::new();
    hash.update(POINT_SET_DOMAIN_V1);
    hash.update(&[CODEC_VERSION_V1]);
    hash.update(&(COMMITMENT_COUNT_V1 as u32).to_be_bytes());
    for (ordinal, point) in commitment_bytes.chunks_exact(POINT_BYTES_V1).enumerate() {
        hash.update(
            &u32::try_from(ordinal)
                .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry)?
                .to_be_bytes(),
        );
        hash.update(point);
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::Integrity);
    }
    Ok(digest)
}

fn context_binding_digest_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    point_set_digest: [u8; 32],
    limb_digests: &[[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
) -> Result<[u8; 32], RnsNativeZeroPaddingCommitmentErrorV1> {
    if point_set_digest == [0; 32] {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::Integrity);
    }
    let mut hash = Keccak256::new();
    hash.update(CONTEXT_BINDING_DOMAIN_V1);
    hash.update(&[CODEC_VERSION_V1]);
    hash.update(&transcript.zero_padding_root());
    hash.update(&transcript.zero_padding_challenge_seed());
    hash.update(&transcript.composite_binding_challenge_seed());
    hash.update(&transcript.transcript_digest());
    hash.update(&point_set_digest);
    for (limb, digest) in limb_digests.iter().enumerate() {
        hash.update(&[limb as u8]);
        hash.update(digest);
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::Integrity);
    }
    Ok(digest)
}

fn schnorr_proof_digest_v1(
    mask_point_bytes: &[u8],
    response_bytes: &[u8],
) -> Result<[u8; 32], RnsNativeZeroPaddingCommitmentErrorV1> {
    if mask_point_bytes.len() != MASK_POINTS_BYTES_V1 || response_bytes.len() != RESPONSES_BYTES_V1
    {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding);
    }
    let mut hash = Keccak256::new();
    hash.update(SCHNORR_PROOF_DOMAIN_V1);
    hash.update(&[CODEC_VERSION_V1]);
    for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        hash.update(&[limb as u8]);
        let point_start = limb * POINT_BYTES_V1;
        let response_start = limb * SCALAR_BYTES_V1;
        hash.update(&mask_point_bytes[point_start..point_start + POINT_BYTES_V1]);
        hash.update(&response_bytes[response_start..response_start + SCALAR_BYTES_V1]);
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::Integrity);
    }
    Ok(digest)
}

fn codec_digest_v1(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(CODEC_DIGEST_DOMAIN_V1);
    hash.update(&[CODEC_VERSION_V1]);
    hash.update(bytes);
    hash.finalize()
}

fn derive_aggregation_challenge_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    binding_digest: [u8; 32],
    point_set_digest: [u8; 32],
    limb: usize,
) -> Result<Scalar, RnsNativeZeroPaddingCommitmentErrorV1> {
    for attempt in 0..MAX_SCALAR_ATTEMPTS_V1 {
        let mut low = Keccak256::new();
        low.update(AGGREGATION_CHALLENGE_DOMAIN_V1);
        low.update(&[CODEC_VERSION_V1, 0]);
        low.update(&transcript.composite_binding_challenge_seed());
        low.update(&transcript.zero_padding_root());
        low.update(&binding_digest);
        low.update(&point_set_digest);
        low.update(&[limb as u8]);
        low.update(&ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb].to_be_bytes());
        low.update(&[attempt]);
        let mut high = Keccak256::new();
        high.update(AGGREGATION_CHALLENGE_DOMAIN_V1);
        high.update(&[CODEC_VERSION_V1, 1]);
        high.update(&transcript.composite_binding_challenge_seed());
        high.update(&transcript.zero_padding_root());
        high.update(&binding_digest);
        high.update(&point_set_digest);
        high.update(&[limb as u8]);
        high.update(&ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb].to_be_bytes());
        high.update(&[attempt]);
        let mut wide = [0; 64];
        wide[..32].copy_from_slice(&low.finalize());
        wide[32..].copy_from_slice(&high.finalize());
        let challenge = Scalar::from_uniform_le_bytes(wide);
        wide.fill(0);
        if !challenge.is_zero() && challenge != Scalar::one() {
            return Ok(challenge);
        }
    }
    Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidScalar)
}

fn derive_schnorr_challenge_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    binding_digest: [u8; 32],
    point_set_digest: [u8; 32],
    limb: usize,
    aggregation_challenge: Scalar,
    aggregate: Point,
    mask: Point,
) -> Result<Scalar, RnsNativeZeroPaddingCommitmentErrorV1> {
    let aggregate = aggregate
        .to_non_identity_wire_bytes()
        .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidPoint)?;
    let mask = mask
        .to_non_identity_wire_bytes()
        .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidPoint)?;
    for attempt in 0..MAX_SCALAR_ATTEMPTS_V1 {
        let mut low = Keccak256::new();
        low.update(SCHNORR_CHALLENGE_DOMAIN_V1);
        low.update(&[CODEC_VERSION_V1, 0]);
        low.update(&transcript.composite_binding_challenge_seed());
        low.update(&binding_digest);
        low.update(&point_set_digest);
        low.update(&[limb as u8]);
        low.update(&aggregation_challenge.to_le_bytes());
        low.update(&aggregate);
        low.update(&mask);
        low.update(&[attempt]);
        let mut high = Keccak256::new();
        high.update(SCHNORR_CHALLENGE_DOMAIN_V1);
        high.update(&[CODEC_VERSION_V1, 1]);
        high.update(&transcript.composite_binding_challenge_seed());
        high.update(&binding_digest);
        high.update(&point_set_digest);
        high.update(&[limb as u8]);
        high.update(&aggregation_challenge.to_le_bytes());
        high.update(&aggregate);
        high.update(&mask);
        high.update(&[attempt]);
        let mut wide = [0; 64];
        wide[..32].copy_from_slice(&low.finalize());
        wide[32..].copy_from_slice(&high.finalize());
        let challenge = Scalar::from_uniform_le_bytes(wide);
        wide.fill(0);
        if !challenge.is_zero() {
            return Ok(challenge);
        }
    }
    Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidScalar)
}

fn aggregate_limb_v1(
    commitments: &[Point],
    challenge: Scalar,
) -> Result<Point, RnsNativeZeroPaddingCommitmentErrorV1> {
    if commitments.len() != CHUNKS_PER_LIMB_V1 || challenge.is_zero() || challenge == Scalar::one()
    {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry);
    }
    let mut terms = Vec::new();
    terms
        .try_reserve_exact(CHUNKS_PER_LIMB_V1)
        .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::Allocation)?;
    let mut power = Scalar::one();
    for point in commitments {
        terms.push((power, *point));
        power *= challenge;
    }
    let aggregate = multiexp::<ZkAmsT256BulletproofSuiteV1>(&terms);
    if aggregate.is_identity() {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidProof);
    }
    Ok(aggregate)
}

fn verify_schnorr_equations_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    binding_digest: [u8; 32],
    point_set_digest: [u8; 32],
    commitments: &[Point],
    masks: &[Point],
    responses: &[Scalar; SCHNORR_PROOF_COUNT_V1],
) -> Result<(), RnsNativeZeroPaddingCommitmentErrorV1> {
    if commitments.len() != COMMITMENT_COUNT_V1 || masks.len() != SCHNORR_PROOF_COUNT_V1 {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry);
    }
    let hiding_generator = ZkAmsT256BulletproofSuiteV1::generators().h;
    if hiding_generator.is_identity() {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidPoint);
    }
    for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        let start = limb * CHUNKS_PER_LIMB_V1;
        let aggregate_challenge =
            derive_aggregation_challenge_v1(transcript, binding_digest, point_set_digest, limb)?;
        let aggregate = aggregate_limb_v1(
            &commitments[start..start + CHUNKS_PER_LIMB_V1],
            aggregate_challenge,
        )?;
        let schnorr_challenge = derive_schnorr_challenge_v1(
            transcript,
            binding_digest,
            point_set_digest,
            limb,
            aggregate_challenge,
            aggregate,
            masks[limb],
        )?;
        if hiding_generator.mul_scalar(responses[limb])
            != masks[limb] + aggregate.mul_scalar(schnorr_challenge)
        {
            return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidProof);
        }
    }
    Ok(())
}

fn context_identities_v1(transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1) -> [[u8; 32]; 12] {
    [
        transcript.profile_manifest_digest(),
        transcript.profile_digest(),
        transcript.topology_digest(),
        transcript.release_candidate_digest(),
        transcript.statement_digest(),
        transcript.operational_context_digest(),
        transcript.source_binding_digest(),
        transcript.main_snapshot_digest(),
        transcript.nonce_snapshot_digest(),
        transcript.source_receipt_digest(),
        transcript.governed_roster_digest(),
        transcript.public_ciphertext_digest(),
    ]
}

fn validate_global_digest_aliases_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    decoded: &DecodedProofV1<'_>,
    limb_digests: &[[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
) -> Result<(), RnsNativeZeroPaddingCommitmentErrorV1> {
    let mut registry = DigestRegistryV1::new();
    for digest in context_identities_v1(transcript) {
        registry.insert(digest)?;
    }
    for opening in transcript.opening_commitments().iter().copied() {
        registry.insert(opening.source_commitment_digest())?;
        registry.insert(opening.hyrax_commitment_digest())?;
    }
    for digest in [
        transcript.mapping_root(),
        transcript.terminal_hyrax_root(),
        transcript.cross_basis_bridge_root(),
        transcript.qpcs_initial_root(),
        transcript.qpcs_quotient_root(),
        transcript.cross_field_root(),
        transcript.global_lookup_root(),
    ] {
        registry.insert(digest)?;
    }
    for root in transcript.qpcs_fri_roots() {
        registry.insert(root.root())?;
    }
    for seed in transcript.ordered_challenge_seeds() {
        registry.insert(seed)?;
    }
    registry.insert(transcript.zero_padding_root())?;
    registry.insert(transcript.transcript_digest())?;
    for digest in limb_digests {
        registry.insert(*digest)?;
    }
    for digest in [
        decoded.binding_digest,
        decoded.point_set_digest,
        decoded.schnorr_proof_digest,
        decoded.codec_digest,
    ] {
        registry.insert(digest)?;
    }
    Ok(())
}

#[cfg(test)]
struct EncodedTestProofV1 {
    bytes: Vec<u8>,
    limb_digests: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
}

#[cfg(test)]
fn deterministic_test_commitments_v1() -> (Vec<Point>, Vec<Scalar>) {
    let hiding_generator = ZkAmsT256BulletproofSuiteV1::generators().h;
    let mut commitments = Vec::with_capacity(COMMITMENT_COUNT_V1);
    let mut blindings = Vec::with_capacity(COMMITMENT_COUNT_V1);
    for ordinal in 0..COMMITMENT_COUNT_V1 {
        let blinding = Scalar::from_u64((ordinal as u64).wrapping_mul(17).wrapping_add(3));
        commitments.push(hiding_generator.mul_scalar(blinding));
        blindings.push(blinding);
    }
    (commitments, blindings)
}

#[cfg(test)]
fn encode_test_proof_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    commitments: &[Point],
    blindings: &[Scalar],
) -> Result<EncodedTestProofV1, RnsNativeZeroPaddingCommitmentErrorV1> {
    if commitments.len() != COMMITMENT_COUNT_V1 || blindings.len() != COMMITMENT_COUNT_V1 {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidGeometry);
    }
    let mut commitment_bytes = Vec::with_capacity(COMMITMENTS_BYTES_V1);
    for point in commitments {
        commitment_bytes.extend_from_slice(
            &point
                .to_non_identity_wire_bytes()
                .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidPoint)?,
        );
    }
    let limb_digests = limb_digests_v1(&commitment_bytes)?;
    if padding_root_v1(transcript, &limb_digests)? != transcript.zero_padding_root() {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::ContextMismatch);
    }
    let point_set_digest = point_set_digest_v1(&commitment_bytes)?;
    let binding_digest = context_binding_digest_v1(transcript, point_set_digest, &limb_digests)?;
    let hiding_generator = ZkAmsT256BulletproofSuiteV1::generators().h;
    let mut mask_bytes = Vec::with_capacity(MASK_POINTS_BYTES_V1);
    let mut response_bytes = Vec::with_capacity(RESPONSES_BYTES_V1);
    for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        let start = limb * CHUNKS_PER_LIMB_V1;
        let aggregation_challenge =
            derive_aggregation_challenge_v1(transcript, binding_digest, point_set_digest, limb)?;
        let aggregate = aggregate_limb_v1(
            &commitments[start..start + CHUNKS_PER_LIMB_V1],
            aggregation_challenge,
        )?;
        let mut aggregate_blinding = Scalar::zero();
        let mut power = Scalar::one();
        for blinding in &blindings[start..start + CHUNKS_PER_LIMB_V1] {
            aggregate_blinding += power * *blinding;
            power *= aggregation_challenge;
        }
        let mask_scalar = Scalar::from_u64(10_003 + limb as u64 * 101);
        let mask = hiding_generator.mul_scalar(mask_scalar);
        let challenge = derive_schnorr_challenge_v1(
            transcript,
            binding_digest,
            point_set_digest,
            limb,
            aggregation_challenge,
            aggregate,
            mask,
        )?;
        let response = mask_scalar + challenge * aggregate_blinding;
        mask_bytes.extend_from_slice(
            &mask
                .to_non_identity_wire_bytes()
                .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidPoint)?,
        );
        response_bytes.extend_from_slice(&response.to_le_bytes());
    }
    let schnorr_proof_digest = schnorr_proof_digest_v1(&mask_bytes, &response_bytes)?;
    let mut bytes = Vec::with_capacity(EXACT_CODEC_BYTES_V1);
    bytes.extend_from_slice(&CODEC_TAG_V1);
    bytes.extend_from_slice(&[
        CODEC_VERSION_V1,
        CODEC_FLAGS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u8,
        SEGMENT_COUNT_V1 as u8,
    ]);
    bytes.extend_from_slice(&(CHUNK_COLUMNS_V1 as u16).to_be_bytes());
    bytes.extend_from_slice(&(CHUNKS_PER_LIMB_V1 as u16).to_be_bytes());
    bytes.extend_from_slice(&(COMMITMENT_COUNT_V1 as u32).to_be_bytes());
    bytes.extend_from_slice(&[
        SCHNORR_PROOF_COUNT_V1 as u8,
        POINT_BYTES_V1 as u8,
        SCALAR_BYTES_V1 as u8,
        0,
    ]);
    for digest in [
        binding_digest,
        point_set_digest,
        transcript.zero_padding_root(),
        schnorr_proof_digest,
    ] {
        bytes.extend_from_slice(&digest);
    }
    bytes.extend_from_slice(&commitment_bytes);
    bytes.extend_from_slice(&mask_bytes);
    bytes.extend_from_slice(&response_bytes);
    if bytes.len() != CODEC_DIGEST_OFFSET_V1 {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding);
    }
    let codec_digest = codec_digest_v1(&bytes);
    bytes.extend_from_slice(&codec_digest);
    if bytes.len() != EXACT_CODEC_BYTES_V1 {
        return Err(RnsNativeZeroPaddingCommitmentErrorV1::InvalidEncoding);
    }
    authenticate_rns_native_zero_padding_commitments_v1(transcript, &limb_digests, &bytes)?;
    Ok(EncodedTestProofV1 {
        bytes,
        limb_digests,
    })
}

#[cfg(test)]
pub(super) fn deterministic_zero_padding_stage_fixture_v1(
    build_transcript: impl Fn([u8; DIGEST_BYTES_V1]) -> ZkAmsMkheRnsNativeChallengeSeedsV1,
) -> Result<
    (
        [u8; DIGEST_BYTES_V1],
        [[u8; DIGEST_BYTES_V1]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
        Vec<u8>,
    ),
    RnsNativeZeroPaddingCommitmentErrorV1,
> {
    let (commitments, blindings) = deterministic_test_commitments_v1();
    let mut commitment_bytes = Vec::with_capacity(COMMITMENTS_BYTES_V1);
    for point in &commitments {
        commitment_bytes.extend_from_slice(
            &point
                .to_non_identity_wire_bytes()
                .map_err(|_| RnsNativeZeroPaddingCommitmentErrorV1::InvalidPoint)?,
        );
    }
    let limb_digests = limb_digests_v1(&commitment_bytes)?;
    let provisional_transcript = build_transcript([0xa5; DIGEST_BYTES_V1]);
    let root = padding_root_v1(&provisional_transcript, &limb_digests)?;
    let transcript = build_transcript(root);
    let encoded = encode_test_proof_v1(&transcript, &commitments, &blindings)?;
    Ok((root, encoded.limb_digests, encoded.bytes))
}

#[cfg(test)]
#[path = "rns_native_zero_padding_commitment_tests.rs"]
mod tests;
