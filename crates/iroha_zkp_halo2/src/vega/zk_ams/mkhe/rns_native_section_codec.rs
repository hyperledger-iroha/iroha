//! Strict canonical codecs for the four RNS-native composite-proof sections.
//!
//! These codecs frame the cryptographic subproof bytes with the exact profile,
//! topology, transcript, challenge, root, and fixed-cardinality metadata needed
//! by the replacement proof. Decoding borrows the already-owned envelope bytes,
//! performs a section-cap preflight before parsing, and never allocates. Encoding
//! computes and checks the complete size before its sole exact allocation.
//!
//! Successful decoding proves transport structure and contextual integrity only.
//! It is not cryptographic proof verification and grants neither readiness nor
//! release authority.

#![allow(
    clippy::large_types_passed_by_value,
    reason = "fixed-cardinality transport records intentionally retain complete canonical metadata"
)]

use super::{
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_POINT_COUNT_V1, ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1,
        ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1, ZK_AMS_MKHE_RNS_NATIVE_RLWE_EQUATION_COUNT_V1,
        ZK_AMS_MKHE_RNS_NATIVE_SUMCHECK_ROUNDS_V1, ZkAmsMkheRnsNativeFamilyV1,
        zk_ams_mkhe_rns_native_profile_manifest_v1,
        zk_ams_mkhe_rns_native_release_candidate_digest_v1, zk_ams_mkhe_rns_native_topology_v1,
    },
    rns_native_transcript::{
        ZkAmsMkheRnsNativeChallengeSeedsV1, ZkAmsMkheRnsNativeOpeningCommitmentV1,
    },
    rns_native_wire::{
        ZkAmsMkheRnsNativeProofEnvelopeV1, ZkAmsMkheRnsNativeProofSectionKindV1,
    },
};
use crate::vega::sponge::Keccak256;

#[cfg(test)]
use std::cell::Cell;

const TERMINAL_TAG_V1: [u8; 4] = *b"ZATB";
const RNS_QPCS_TAG_V1: [u8; 4] = *b"ZARQ";
const CROSS_LOOKUP_TAG_V1: [u8; 4] = *b"ZACG";
const ZERO_PADDING_TAG_V1: [u8; 4] = *b"ZAZP";
const PROOF_BODY_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-section-proof-body";
const CODEC_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-section-codec";
const COMMON_DIGEST_COUNT_V1: usize = 5;
const COMMON_PREFIX_BYTES_V1: usize = 4 + 1 + 1 + 4 + COMMON_DIGEST_COUNT_V1 * 32;
const PROOF_FRAME_BYTES_V1: usize = 4 + 32;
const CODEC_DIGEST_BYTES_V1: usize = 32;
const MAX_SECTION_DIGESTS_V1: usize = 512;

const OPENING_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 as usize;
const EQUATION_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_RLWE_EQUATION_COUNT_V1 as usize;
const POINT_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_POINT_COUNT_V1 as usize;
const QUERY_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1 as usize;
const FRI_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1 as usize;
const SUMCHECK_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_SUMCHECK_ROUNDS_V1 as usize;

const TERMINAL_FIXED_BYTES_V1: usize = COMMON_PREFIX_BYTES_V1
    + 1
    + 2 * 32
    + 3 * 32
    + OPENING_COUNT_V1 * (3 + 2 * 32)
    + PROOF_FRAME_BYTES_V1
    + CODEC_DIGEST_BYTES_V1;
pub(super) const RNS_QPCS_FIXED_BYTES_V1: usize = COMMON_PREFIX_BYTES_V1
    + 5
    + 4 * 32
    + FRI_COUNT_V1 * (1 + 32)
    + 2 * 32
    + FRI_COUNT_V1 * (1 + 32)
    + EQUATION_COUNT_V1 * (1 + 32)
    + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * (1 + 32)
    + QUERY_COUNT_V1 * (2 + 32)
    + PROOF_FRAME_BYTES_V1
    + CODEC_DIGEST_BYTES_V1;
pub(super) const CROSS_LOOKUP_FIXED_BYTES_V1: usize = COMMON_PREFIX_BYTES_V1
    + 3
    + 2 * 32
    + 2 * 32
    + POINT_COUNT_V1 * (1 + 32)
    + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * (1 + 32)
    + SUMCHECK_COUNT_V1 * (1 + 32)
    + PROOF_FRAME_BYTES_V1
    + CODEC_DIGEST_BYTES_V1;
const CROSS_LOOKUP_PROOF_LENGTH_OFFSET_V1: usize =
    CROSS_LOOKUP_FIXED_BYTES_V1 - PROOF_FRAME_BYTES_V1 - CODEC_DIGEST_BYTES_V1;
const CROSS_LOOKUP_PROOF_OFFSET_V1: usize =
    CROSS_LOOKUP_PROOF_LENGTH_OFFSET_V1 + PROOF_FRAME_BYTES_V1;
const ZERO_PADDING_FIXED_BYTES_V1: usize = COMMON_PREFIX_BYTES_V1
    + 1
    + 2 * 32
    + 32
    + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * (1 + 32)
    + PROOF_FRAME_BYTES_V1
    + CODEC_DIGEST_BYTES_V1;

/// Canonical schema version shared by all four typed section codecs.
pub const ZK_AMS_MKHE_RNS_NATIVE_SECTION_CODEC_VERSION_V1: u8 = 1;

const _: () = {
    assert!(OPENING_COUNT_V1 == 43);
    assert!(EQUATION_COUNT_V1 == 2);
    assert!(POINT_COUNT_V1 == 5);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(QUERY_COUNT_V1 == 160);
    assert!(FRI_COUNT_V1 == 18);
    assert!(SUMCHECK_COUNT_V1 == 29);
    assert!(MAX_SECTION_DIGESTS_V1 >= 482);
    assert!(CROSS_LOOKUP_FIXED_BYTES_V1 == 2_811);
    assert!(CROSS_LOOKUP_PROOF_LENGTH_OFFSET_V1 == 2_743);
    assert!(CROSS_LOOKUP_PROOF_OFFSET_V1 == 2_779);
};

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CrossLookupUnboundAuditCountersV1 {
    parse_passes: usize,
    proof_hash_passes: usize,
    codec_hash_passes: usize,
    final_context_binds: usize,
}

#[cfg(test)]
std::thread_local! {
    static CROSS_LOOKUP_UNBOUND_AUDIT_COUNTERS_V1: Cell<CrossLookupUnboundAuditCountersV1> =
        const { Cell::new(CrossLookupUnboundAuditCountersV1 {
            parse_passes: 0,
            proof_hash_passes: 0,
            codec_hash_passes: 0,
            final_context_binds: 0,
        }) };
}

#[cfg(test)]
fn update_cross_lookup_unbound_audit_counters_v1(
    update: impl FnOnce(&mut CrossLookupUnboundAuditCountersV1),
) {
    CROSS_LOOKUP_UNBOUND_AUDIT_COUNTERS_V1.with(|cell| {
        let mut counters = cell.get();
        update(&mut counters);
        cell.set(counters);
    });
}

#[cfg(test)]
fn cross_lookup_unbound_audit_counters_v1() -> CrossLookupUnboundAuditCountersV1 {
    CROSS_LOOKUP_UNBOUND_AUDIT_COUNTERS_V1.with(Cell::get)
}

/// Failure while constructing, encoding, or decoding a typed proof section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZkAmsMkheRnsNativeSectionCodecErrorV1 {
    /// The outer section exceeded its governed cap before parsing or allocation.
    ResourceCeilingExceeded,
    /// A tag, version, scalar, length, or trailing byte was not canonical.
    InvalidEncoding,
    /// Profile, topology, candidate, transcript, challenge, or root context differed.
    ContextMismatch,
    /// A fixed internal cardinality was not exact.
    InvalidCount,
    /// An explicitly encoded ordinal, family, limb, query, FRI, or round was reordered.
    InvalidOrder,
    /// A zero or semantically aliased digest was present.
    AliasedDigest,
    /// A proof-body or whole-codec digest did not match the exact bytes.
    Integrity,
}

impl core::fmt::Display for ZkAmsMkheRnsNativeSectionCodecErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::ResourceCeilingExceeded => "RNS-native section resource ceiling exceeded",
            Self::InvalidEncoding => "invalid RNS-native typed section encoding",
            Self::ContextMismatch => "mismatched RNS-native typed section context",
            Self::InvalidCount => "invalid RNS-native typed section count",
            Self::InvalidOrder => "invalid RNS-native typed section order",
            Self::AliasedDigest => "zero or aliased RNS-native typed section digest",
            Self::Integrity => "invalid RNS-native typed section integrity digest",
        })
    }
}

impl std::error::Error for ZkAmsMkheRnsNativeSectionCodecErrorV1 {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SectionHeaderV1 {
    profile_manifest_digest: [u8; 32],
    profile_digest: [u8; 32],
    topology_digest: [u8; 32],
    release_candidate_digest: [u8; 32],
    transcript_digest: [u8; 32],
}

fn canonical_header_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
) -> Result<SectionHeaderV1, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()
        .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch)?;
    manifest
        .validate()
        .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch)?;
    let topology = zk_ams_mkhe_rns_native_topology_v1()
        .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch)?;
    topology
        .validate()
        .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch)?;
    let header = SectionHeaderV1 {
        profile_manifest_digest: manifest.manifest_digest,
        profile_digest: manifest.profile_digest,
        topology_digest: topology.topology_digest,
        release_candidate_digest: zk_ams_mkhe_rns_native_release_candidate_digest_v1()
            .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch)?,
        transcript_digest: transcript.transcript_digest(),
    };
    let mut registry = DigestRegistryV1::new();
    insert_header_digests_v1(&mut registry, header)?;
    Ok(header)
}

fn insert_header_digests_v1(
    registry: &mut DigestRegistryV1,
    header: SectionHeaderV1,
) -> Result<(), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    for digest in [
        header.profile_manifest_digest,
        header.profile_digest,
        header.topology_digest,
        header.release_candidate_digest,
        header.transcript_digest,
    ] {
        registry.insert(digest)?;
    }
    Ok(())
}

fn family_from_ordinal_v1(ordinal: usize) -> Option<(ZkAmsMkheRnsNativeFamilyV1, u8)> {
    match ordinal {
        0 => Some((ZkAmsMkheRnsNativeFamilyV1::X, 0)),
        1..=16 => Some((
            ZkAmsMkheRnsNativeFamilyV1::U,
            u8::try_from(ordinal - 1).ok()?,
        )),
        17..=32 => Some((
            ZkAmsMkheRnsNativeFamilyV1::E,
            u8::try_from(ordinal - 17).ok()?,
        )),
        33 => Some((ZkAmsMkheRnsNativeFamilyV1::RE, 0)),
        34..=41 => Some((
            ZkAmsMkheRnsNativeFamilyV1::W,
            u8::try_from(ordinal - 34).ok()?,
        )),
        42 => Some((ZkAmsMkheRnsNativeFamilyV1::RW, 0)),
        _ => None,
    }
}

fn exact_digest_array_v1<const N: usize>(
    values: &[[u8; 32]],
) -> Result<[[u8; 32]; N], ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    let exact: &[[u8; 32]; N] = values
        .try_into()
        .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount)?;
    Ok(*exact)
}

fn tag_v1(kind: ZkAmsMkheRnsNativeProofSectionKindV1) -> [u8; 4] {
    match kind {
        ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge => TERMINAL_TAG_V1,
        ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs => RNS_QPCS_TAG_V1,
        ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup => CROSS_LOOKUP_TAG_V1,
        ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding => ZERO_PADDING_TAG_V1,
    }
}

fn encoded_len_v1(
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    fixed_bytes: usize,
    proof_bytes: usize,
) -> Result<usize, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    if proof_bytes == 0 {
        return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidEncoding);
    }
    let total = fixed_bytes
        .checked_add(proof_bytes)
        .ok_or(ZkAmsMkheRnsNativeSectionCodecErrorV1::ResourceCeilingExceeded)?;
    let cap = usize::try_from(kind.max_bytes())
        .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::ResourceCeilingExceeded)?;
    if total > cap || u32::try_from(total).is_err() {
        return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ResourceCeilingExceeded);
    }
    Ok(total)
}

fn preflight_v1(
    bytes: &[u8],
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    minimum: usize,
) -> Result<(), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    let cap = usize::try_from(kind.max_bytes())
        .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::ResourceCeilingExceeded)?;
    if bytes.len() > cap {
        return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ResourceCeilingExceeded);
    }
    if bytes.len() < minimum {
        return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidEncoding);
    }
    Ok(())
}

fn proof_body_digest_v1(
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    proof: &[u8],
) -> Result<[u8; 32], ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    let length = u64::try_from(proof.len())
        .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::ResourceCeilingExceeded)?;
    let mut hash = Keccak256::new();
    hash.update(PROOF_BODY_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_SECTION_CODEC_VERSION_V1, kind as u8]);
    hash.update(&length.to_be_bytes());
    hash.update(proof);
    Ok(hash.finalize())
}

fn codec_digest_v1(kind: ZkAmsMkheRnsNativeProofSectionKindV1, prefix: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(CODEC_DIGEST_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_SECTION_CODEC_VERSION_V1, kind as u8]);
    hash.update(prefix);
    hash.finalize()
}

fn write_header_v1(
    bytes: &mut Vec<u8>,
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    total_bytes: usize,
    header: SectionHeaderV1,
) -> Result<(), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    bytes.extend_from_slice(&tag_v1(kind));
    bytes.push(ZK_AMS_MKHE_RNS_NATIVE_SECTION_CODEC_VERSION_V1);
    bytes.push(kind as u8);
    bytes.extend_from_slice(
        &u32::try_from(total_bytes)
            .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for digest in [
        header.profile_manifest_digest,
        header.profile_digest,
        header.topology_digest,
        header.release_candidate_digest,
        header.transcript_digest,
    ] {
        bytes.extend_from_slice(&digest);
    }
    Ok(())
}

fn write_indexed_u8_v1(
    bytes: &mut Vec<u8>,
    values: &[[u8; 32]],
) -> Result<(), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    for (index, digest) in values.iter().enumerate() {
        bytes.push(
            u8::try_from(index).map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount)?,
        );
        bytes.extend_from_slice(digest);
    }
    Ok(())
}

fn write_indexed_u16_v1(
    bytes: &mut Vec<u8>,
    values: &[[u8; 32]],
) -> Result<(), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    for (index, digest) in values.iter().enumerate() {
        bytes.extend_from_slice(
            &u16::try_from(index)
                .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount)?
                .to_be_bytes(),
        );
        bytes.extend_from_slice(digest);
    }
    Ok(())
}

fn finish_encoding_v1(
    mut bytes: Vec<u8>,
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    total_bytes: usize,
) -> Result<Vec<u8>, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    let digest = codec_digest_v1(kind, &bytes);
    if digest == [0; 32] {
        return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::Integrity);
    }
    bytes.extend_from_slice(&digest);
    if bytes.len() != total_bytes || bytes.capacity() < total_bytes {
        return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidEncoding);
    }
    Ok(bytes)
}

fn begin_encoding_v1(
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    fixed_bytes: usize,
    proof: &[u8],
    header: SectionHeaderV1,
) -> Result<(Vec<u8>, usize, [u8; 32]), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    let total = encoded_len_v1(kind, fixed_bytes, proof.len())?;
    let proof_digest = proof_body_digest_v1(kind, proof)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::ResourceCeilingExceeded)?;
    write_header_v1(&mut bytes, kind, total, header)?;
    Ok((bytes, total, proof_digest))
}

fn write_proof_v1(
    bytes: &mut Vec<u8>,
    proof: &[u8],
    digest: [u8; 32],
) -> Result<(), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    bytes.extend_from_slice(
        &u32::try_from(proof.len())
            .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    bytes.extend_from_slice(&digest);
    bytes.extend_from_slice(proof);
    Ok(())
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidEncoding)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidEncoding)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidEncoding)
    }

    fn u8(&mut self) -> Result<u8, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    const fn position(&self) -> usize {
        self.cursor
    }
}

fn read_unbound_header_v1(
    decoder: &mut DecoderV1<'_>,
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    total_bytes: usize,
) -> Result<SectionHeaderV1, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    if decoder.array::<4>()? != tag_v1(kind)
        || decoder.u8()? != ZK_AMS_MKHE_RNS_NATIVE_SECTION_CODEC_VERSION_V1
        || decoder.u8()? != kind as u8
        || usize::try_from(decoder.u32()?).ok() != Some(total_bytes)
    {
        return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidEncoding);
    }
    Ok(SectionHeaderV1 {
        profile_manifest_digest: decoder.array()?,
        profile_digest: decoder.array()?,
        topology_digest: decoder.array()?,
        release_candidate_digest: decoder.array()?,
        transcript_digest: decoder.array()?,
    })
}

fn read_header_v1(
    decoder: &mut DecoderV1<'_>,
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    total_bytes: usize,
    expected: SectionHeaderV1,
) -> Result<SectionHeaderV1, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    let actual = read_unbound_header_v1(decoder, kind, total_bytes)?;
    if actual != expected {
        return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch);
    }
    Ok(actual)
}

fn read_indexed_u8_v1<const N: usize>(
    decoder: &mut DecoderV1<'_>,
    registry: &mut DigestRegistryV1,
) -> Result<[[u8; 32]; N], ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    let mut values = [[0_u8; 32]; N];
    for (index, value) in values.iter_mut().enumerate() {
        if usize::from(decoder.u8()?) != index {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidOrder);
        }
        *value = decoder.array()?;
        registry.insert(*value)?;
    }
    Ok(values)
}

fn read_indexed_u16_v1<const N: usize>(
    decoder: &mut DecoderV1<'_>,
    registry: &mut DigestRegistryV1,
) -> Result<[[u8; 32]; N], ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    let mut values = [[0_u8; 32]; N];
    for (index, value) in values.iter_mut().enumerate() {
        if usize::from(decoder.u16()?) != index {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidOrder);
        }
        *value = decoder.array()?;
        registry.insert(*value)?;
    }
    Ok(values)
}

fn read_proof_v1<'a>(
    decoder: &mut DecoderV1<'a>,
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    registry: &mut DigestRegistryV1,
) -> Result<(&'a [u8], [u8; 32]), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    let length = usize::try_from(decoder.u32()?)
        .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::ResourceCeilingExceeded)?;
    if length == 0 {
        return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidEncoding);
    }
    let expected_digest = decoder.array()?;
    registry.insert(expected_digest)?;
    let proof = decoder.take(length)?;
    if proof_body_digest_v1(kind, proof)? != expected_digest {
        return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::Integrity);
    }
    Ok((proof, expected_digest))
}

fn finish_decoding_v1(
    decoder: &mut DecoderV1<'_>,
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    registry: &mut DigestRegistryV1,
) -> Result<[u8; 32], ZkAmsMkheRnsNativeSectionCodecErrorV1> {
    let prefix_bytes = decoder.position();
    let digest = decoder.array::<32>()?;
    if decoder.position() != decoder.bytes.len() {
        return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidEncoding);
    }
    if digest != codec_digest_v1(kind, &decoder.bytes[..prefix_bytes]) {
        return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::Integrity);
    }
    registry.insert(digest)?;
    Ok(digest)
}

struct DigestRegistryV1 {
    digests: [[u8; 32]; MAX_SECTION_DIGESTS_V1],
    len: usize,
}

impl DigestRegistryV1 {
    const fn new() -> Self {
        Self {
            digests: [[0; 32]; MAX_SECTION_DIGESTS_V1],
            len: 0,
        }
    }

    fn insert(&mut self, digest: [u8; 32]) -> Result<(), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        if digest == [0; 32] || self.digests[..self.len].contains(&digest) {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::AliasedDigest);
        }
        let destination = self
            .digests
            .get_mut(self.len)
            .ok_or(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount)?;
        *destination = digest;
        self.len += 1;
        Ok(())
    }
}

/// Borrowed, typed terminal mapping/Hyrax/cross-basis proof section.
///
/// This is a canonical transport view, not a verified proof or authorization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeTerminalBridgeSectionV1<'a> {
    header: SectionHeaderV1,
    opening_commitments: [ZkAmsMkheRnsNativeOpeningCommitmentV1; OPENING_COUNT_V1],
    mapping_challenge_seed: [u8; 32],
    cross_basis_challenge_seed: [u8; 32],
    mapping_root: [u8; 32],
    terminal_hyrax_root: [u8; 32],
    cross_basis_bridge_root: [u8; 32],
    proof: &'a [u8],
    proof_digest: [u8; 32],
}

impl<'a> ZkAmsMkheRnsNativeTerminalBridgeSectionV1<'a> {
    /// Construct a canonical terminal transport view for one exact transcript.
    ///
    /// # Errors
    ///
    /// Rejects an empty/oversized proof or any zero/aliased contextual digest.
    pub fn new(
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
        proof: &'a [u8],
    ) -> Result<Self, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        let section = Self {
            header: canonical_header_v1(transcript)?,
            opening_commitments: *transcript.opening_commitments(),
            mapping_challenge_seed: transcript.mapping_challenge_seed(),
            cross_basis_challenge_seed: transcript.cross_basis_challenge_seed(),
            mapping_root: transcript.mapping_root(),
            terminal_hyrax_root: transcript.terminal_hyrax_root(),
            cross_basis_bridge_root: transcript.cross_basis_bridge_root(),
            proof,
            proof_digest: proof_body_digest_v1(
                ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge,
                proof,
            )?,
        };
        section.validate_v1()?;
        Ok(section)
    }

    /// Decode exactly one canonical terminal section without allocation.
    ///
    /// The outer cap is checked before any field is read. All transcript-bound
    /// commitments, challenges, and roots must equal the supplied typed result.
    ///
    /// # Errors
    ///
    /// Rejects every cap, truncation, count, order, context, alias, integrity,
    /// or trailing-byte violation.
    pub fn from_canonical_bytes_exact_v1(
        bytes: &'a [u8],
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        let kind = ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge;
        preflight_v1(bytes, kind, TERMINAL_FIXED_BYTES_V1 + 1)?;
        let expected_header = canonical_header_v1(transcript)?;
        let mut decoder = DecoderV1::new(bytes);
        let header = read_header_v1(&mut decoder, kind, bytes.len(), expected_header)?;
        if decoder.u8()? != ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount);
        }
        let mapping_challenge_seed = decoder.array()?;
        let cross_basis_challenge_seed = decoder.array()?;
        let mapping_root = decoder.array()?;
        let terminal_hyrax_root = decoder.array()?;
        let cross_basis_bridge_root = decoder.array()?;
        if mapping_challenge_seed != transcript.mapping_challenge_seed()
            || cross_basis_challenge_seed != transcript.cross_basis_challenge_seed()
            || mapping_root != transcript.mapping_root()
            || terminal_hyrax_root != transcript.terminal_hyrax_root()
            || cross_basis_bridge_root != transcript.cross_basis_bridge_root()
        {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch);
        }
        let opening_commitments = *transcript.opening_commitments();
        let mut registry = DigestRegistryV1::new();
        insert_header_digests_v1(&mut registry, header)?;
        for digest in [
            mapping_challenge_seed,
            cross_basis_challenge_seed,
            mapping_root,
            terminal_hyrax_root,
            cross_basis_bridge_root,
        ] {
            registry.insert(digest)?;
        }
        for (ordinal, expected) in opening_commitments.iter().copied().enumerate() {
            let ordinal_u8 = u8::try_from(ordinal)
                .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount)?;
            let family = decoder.u8()?;
            let family_index = decoder.u8()?;
            let encoded_ordinal = decoder.u8()?;
            let source_digest = decoder.array()?;
            let hyrax_digest = decoder.array()?;
            if encoded_ordinal != ordinal_u8
                || family != expected.family() as u8
                || family_index != expected.family_index()
                || source_digest != expected.source_commitment_digest()
                || hyrax_digest != expected.hyrax_commitment_digest()
            {
                return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidOrder);
            }
            registry.insert(source_digest)?;
            registry.insert(hyrax_digest)?;
        }
        let (proof, proof_digest) = read_proof_v1(&mut decoder, kind, &mut registry)?;
        finish_decoding_v1(&mut decoder, kind, &mut registry)?;
        Ok(Self {
            header,
            opening_commitments,
            mapping_challenge_seed,
            cross_basis_challenge_seed,
            mapping_root,
            terminal_hyrax_root,
            cross_basis_bridge_root,
            proof,
            proof_digest,
        })
    }

    /// Encode the exact canonical terminal section after a pre-allocation cap check.
    ///
    /// # Errors
    ///
    /// Rejects invalid context, proof length, aliases, or failed bounded allocation.
    pub fn to_canonical_bytes_v1(&self) -> Result<Vec<u8>, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        self.validate_v1()?;
        let kind = ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge;
        let (mut bytes, total, proof_digest) =
            begin_encoding_v1(kind, TERMINAL_FIXED_BYTES_V1, self.proof, self.header)?;
        bytes.push(ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1);
        for digest in [
            self.mapping_challenge_seed,
            self.cross_basis_challenge_seed,
            self.mapping_root,
            self.terminal_hyrax_root,
            self.cross_basis_bridge_root,
        ] {
            bytes.extend_from_slice(&digest);
        }
        for (ordinal, opening) in self.opening_commitments.iter().copied().enumerate() {
            bytes.push(opening.family() as u8);
            bytes.push(opening.family_index());
            bytes.push(
                u8::try_from(ordinal)
                    .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount)?,
            );
            bytes.extend_from_slice(&opening.source_commitment_digest());
            bytes.extend_from_slice(&opening.hyrax_commitment_digest());
        }
        write_proof_v1(&mut bytes, self.proof, proof_digest)?;
        finish_encoding_v1(bytes, kind, total)
    }

    /// Borrow the 43 exact opening commitments in canonical family order.
    #[must_use]
    pub const fn opening_commitments(&self) -> &[ZkAmsMkheRnsNativeOpeningCommitmentV1] {
        &self.opening_commitments
    }

    /// Borrow the opaque cryptographic terminal proof bytes.
    #[must_use]
    pub const fn proof(&self) -> &'a [u8] {
        self.proof
    }

    fn validate_v1(&self) -> Result<(), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        encoded_len_v1(
            ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge,
            TERMINAL_FIXED_BYTES_V1,
            self.proof.len(),
        )?;
        let mut registry = DigestRegistryV1::new();
        insert_header_digests_v1(&mut registry, self.header)?;
        for digest in [
            self.mapping_challenge_seed,
            self.cross_basis_challenge_seed,
            self.mapping_root,
            self.terminal_hyrax_root,
            self.cross_basis_bridge_root,
        ] {
            registry.insert(digest)?;
        }
        for (ordinal, opening) in self.opening_commitments.iter().copied().enumerate() {
            if family_from_ordinal_v1(ordinal) != Some((opening.family(), opening.family_index())) {
                return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidOrder);
            }
            registry.insert(opening.source_commitment_digest())?;
            registry.insert(opening.hyrax_commitment_digest())?;
        }
        if self.proof_digest
            != proof_body_digest_v1(
                ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge,
                self.proof,
            )?
        {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::Integrity);
        }
        registry.insert(self.proof_digest)?;
        Ok(())
    }
}

/// Borrowed, typed two-equation RNS-relation and qPCS proof section.
///
/// Fixed arrays make the two equations, forty limbs, 160 queries, and eighteen
/// FRI layers explicit. This view conveys transport validity only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1<'a> {
    header: SectionHeaderV1,
    rns_aggregation_challenge_seed: [u8; 32],
    qpcs_relation_challenge_seed: [u8; 32],
    qpcs_batching_challenge_seed: [u8; 32],
    qpcs_fri_fold_challenge_seeds: [[u8; 32]; FRI_COUNT_V1],
    qpcs_query_challenge_seed: [u8; 32],
    qpcs_initial_root: [u8; 32],
    qpcs_quotient_root: [u8; 32],
    qpcs_fri_roots: [[u8; 32]; FRI_COUNT_V1],
    equation_commitment_digests: [[u8; 32]; EQUATION_COUNT_V1],
    limb_commitment_digests: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    query_opening_digests: [[u8; 32]; QUERY_COUNT_V1],
    proof: &'a [u8],
    proof_digest: [u8; 32],
}

impl<'a> ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1<'a> {
    /// Construct a canonical RNS/qPCS transport view for one exact transcript.
    ///
    /// # Errors
    ///
    /// Rejects an empty/oversized proof or any zero/aliased metadata digest.
    pub fn new(
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
        equation_commitment_digests: &[[u8; 32]],
        limb_commitment_digests: &[[u8; 32]],
        query_opening_digests: &[[u8; 32]],
        proof: &'a [u8],
    ) -> Result<Self, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        let qpcs_fri_roots =
            core::array::from_fn(|layer| transcript.qpcs_fri_roots()[layer].root());
        let section = Self {
            header: canonical_header_v1(transcript)?,
            rns_aggregation_challenge_seed: transcript.rns_aggregation_challenge_seed(),
            qpcs_relation_challenge_seed: transcript.qpcs_relation_challenge_seed(),
            qpcs_batching_challenge_seed: transcript.qpcs_batching_challenge_seed(),
            qpcs_fri_fold_challenge_seeds: *transcript.qpcs_fri_fold_challenge_seeds(),
            qpcs_query_challenge_seed: transcript.qpcs_query_challenge_seed(),
            qpcs_initial_root: transcript.qpcs_initial_root(),
            qpcs_quotient_root: transcript.qpcs_quotient_root(),
            qpcs_fri_roots,
            equation_commitment_digests: exact_digest_array_v1(equation_commitment_digests)?,
            limb_commitment_digests: exact_digest_array_v1(limb_commitment_digests)?,
            query_opening_digests: exact_digest_array_v1(query_opening_digests)?,
            proof,
            proof_digest: proof_body_digest_v1(
                ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs,
                proof,
            )?,
        };
        section.validate_v1()?;
        Ok(section)
    }

    /// Decode exactly one canonical RNS/qPCS section without allocation.
    ///
    /// # Errors
    ///
    /// Rejects every cap, truncation, count, order, context, alias, integrity,
    /// overflow, or trailing-byte violation.
    pub fn from_canonical_bytes_exact_v1(
        bytes: &'a [u8],
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        let kind = ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs;
        preflight_v1(bytes, kind, RNS_QPCS_FIXED_BYTES_V1 + 1)?;
        let expected_header = canonical_header_v1(transcript)?;
        let mut decoder = DecoderV1::new(bytes);
        let header = read_header_v1(&mut decoder, kind, bytes.len(), expected_header)?;
        if decoder.u8()? != ZK_AMS_MKHE_RNS_NATIVE_RLWE_EQUATION_COUNT_V1
            || usize::from(decoder.u8()?) != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || decoder.u16()? != ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1
            || decoder.u8()? != ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1
        {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount);
        }
        let rns_aggregation_challenge_seed = decoder.array()?;
        let qpcs_relation_challenge_seed = decoder.array()?;
        let qpcs_batching_challenge_seed = decoder.array()?;
        let mut qpcs_fri_fold_challenge_seeds = [[0_u8; 32]; FRI_COUNT_V1];
        for (layer, challenge) in qpcs_fri_fold_challenge_seeds.iter_mut().enumerate() {
            if usize::from(decoder.u8()?) != layer {
                return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidOrder);
            }
            *challenge = decoder.array()?;
        }
        let qpcs_query_challenge_seed = decoder.array()?;
        if rns_aggregation_challenge_seed != transcript.rns_aggregation_challenge_seed()
            || qpcs_relation_challenge_seed != transcript.qpcs_relation_challenge_seed()
            || qpcs_batching_challenge_seed != transcript.qpcs_batching_challenge_seed()
            || qpcs_fri_fold_challenge_seeds != *transcript.qpcs_fri_fold_challenge_seeds()
            || qpcs_query_challenge_seed != transcript.qpcs_query_challenge_seed()
        {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch);
        }
        let qpcs_initial_root = decoder.array()?;
        let qpcs_quotient_root = decoder.array()?;
        let mut qpcs_fri_roots = [[0_u8; 32]; FRI_COUNT_V1];
        for (layer, root) in qpcs_fri_roots.iter_mut().enumerate() {
            if usize::from(decoder.u8()?) != layer {
                return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidOrder);
            }
            *root = decoder.array()?;
            let expected = transcript.qpcs_fri_roots()[layer];
            if usize::from(expected.layer()) != layer || *root != expected.root() {
                return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch);
            }
        }
        if qpcs_initial_root != transcript.qpcs_initial_root()
            || qpcs_quotient_root != transcript.qpcs_quotient_root()
        {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch);
        }
        let mut registry = DigestRegistryV1::new();
        insert_header_digests_v1(&mut registry, header)?;
        for digest in [
            rns_aggregation_challenge_seed,
            qpcs_relation_challenge_seed,
            qpcs_batching_challenge_seed,
            qpcs_query_challenge_seed,
            qpcs_initial_root,
            qpcs_quotient_root,
        ] {
            registry.insert(digest)?;
        }
        for digest in qpcs_fri_fold_challenge_seeds {
            registry.insert(digest)?;
        }
        for digest in qpcs_fri_roots {
            registry.insert(digest)?;
        }
        let equation_commitment_digests = read_indexed_u8_v1(&mut decoder, &mut registry)?;
        let limb_commitment_digests = read_indexed_u8_v1(&mut decoder, &mut registry)?;
        let query_opening_digests = read_indexed_u16_v1(&mut decoder, &mut registry)?;
        let (proof, proof_digest) = read_proof_v1(&mut decoder, kind, &mut registry)?;
        finish_decoding_v1(&mut decoder, kind, &mut registry)?;
        Ok(Self {
            header,
            rns_aggregation_challenge_seed,
            qpcs_relation_challenge_seed,
            qpcs_batching_challenge_seed,
            qpcs_fri_fold_challenge_seeds,
            qpcs_query_challenge_seed,
            qpcs_initial_root,
            qpcs_quotient_root,
            qpcs_fri_roots,
            equation_commitment_digests,
            limb_commitment_digests,
            query_opening_digests,
            proof,
            proof_digest,
        })
    }

    /// Encode the exact canonical RNS/qPCS section after a cap preflight.
    ///
    /// # Errors
    ///
    /// Rejects invalid metadata, proof length, aliases, or bounded allocation failure.
    pub fn to_canonical_bytes_v1(&self) -> Result<Vec<u8>, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        self.validate_v1()?;
        let kind = ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs;
        let (mut bytes, total, proof_digest) =
            begin_encoding_v1(kind, RNS_QPCS_FIXED_BYTES_V1, self.proof, self.header)?;
        bytes.push(ZK_AMS_MKHE_RNS_NATIVE_RLWE_EQUATION_COUNT_V1);
        bytes.push(
            u8::try_from(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
                .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount)?,
        );
        bytes.extend_from_slice(&ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1.to_be_bytes());
        bytes.push(ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1);
        for digest in [
            self.rns_aggregation_challenge_seed,
            self.qpcs_relation_challenge_seed,
            self.qpcs_batching_challenge_seed,
        ] {
            bytes.extend_from_slice(&digest);
        }
        for (layer, challenge) in self.qpcs_fri_fold_challenge_seeds.iter().enumerate() {
            bytes.push(
                u8::try_from(layer)
                    .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount)?,
            );
            bytes.extend_from_slice(challenge);
        }
        bytes.extend_from_slice(&self.qpcs_query_challenge_seed);
        bytes.extend_from_slice(&self.qpcs_initial_root);
        bytes.extend_from_slice(&self.qpcs_quotient_root);
        for (layer, root) in self.qpcs_fri_roots.iter().enumerate() {
            bytes.push(
                u8::try_from(layer)
                    .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount)?,
            );
            bytes.extend_from_slice(root);
        }
        write_indexed_u8_v1(&mut bytes, &self.equation_commitment_digests)?;
        write_indexed_u8_v1(&mut bytes, &self.limb_commitment_digests)?;
        write_indexed_u16_v1(&mut bytes, &self.query_opening_digests)?;
        write_proof_v1(&mut bytes, self.proof, proof_digest)?;
        finish_encoding_v1(bytes, kind, total)
    }

    /// Borrow the two ordered RNS-equation commitment digests.
    #[must_use]
    pub const fn equation_commitment_digests(&self) -> &[[u8; 32]] {
        &self.equation_commitment_digests
    }

    /// Borrow the forty ordered limb commitment digests.
    #[must_use]
    pub const fn limb_commitment_digests(&self) -> &[[u8; 32]] {
        &self.limb_commitment_digests
    }

    /// Borrow the 160 ordered common-query opening digests.
    #[must_use]
    pub const fn query_opening_digests(&self) -> &[[u8; 32]] {
        &self.query_opening_digests
    }

    /// Borrow the opaque cryptographic RNS/qPCS proof bytes.
    #[must_use]
    pub const fn proof(&self) -> &'a [u8] {
        self.proof
    }

    fn validate_v1(&self) -> Result<(), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        encoded_len_v1(
            ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs,
            RNS_QPCS_FIXED_BYTES_V1,
            self.proof.len(),
        )?;
        let mut registry = DigestRegistryV1::new();
        insert_header_digests_v1(&mut registry, self.header)?;
        for digest in [
            self.rns_aggregation_challenge_seed,
            self.qpcs_relation_challenge_seed,
            self.qpcs_batching_challenge_seed,
            self.qpcs_query_challenge_seed,
            self.qpcs_initial_root,
            self.qpcs_quotient_root,
        ] {
            registry.insert(digest)?;
        }
        for digest in self.qpcs_fri_fold_challenge_seeds {
            registry.insert(digest)?;
        }
        for digest in self.qpcs_fri_roots {
            registry.insert(digest)?;
        }
        for digest in self
            .equation_commitment_digests
            .into_iter()
            .chain(self.limb_commitment_digests)
            .chain(self.query_opening_digests)
        {
            registry.insert(digest)?;
        }
        if self.proof_digest
            != proof_body_digest_v1(
                ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs,
                self.proof,
            )?
        {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::Integrity);
        }
        registry.insert(self.proof_digest)?;
        Ok(())
    }
}

/// Borrowed, typed cross-field and committed-global-lookup proof section.
///
/// The five evaluation points, forty limbs, and twenty-nine sumcheck rounds
/// are explicit ordered records. This view conveys transport validity only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1<'a> {
    header: SectionHeaderV1,
    cross_field_challenge_seed: [u8; 32],
    global_lookup_challenge_seed: [u8; 32],
    cross_field_root: [u8; 32],
    global_lookup_root: [u8; 32],
    point_evaluation_digests: [[u8; 32]; POINT_COUNT_V1],
    limb_relation_digests: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    sumcheck_round_digests: [[u8; 32]; SUMCHECK_COUNT_V1],
    proof: &'a [u8],
    proof_digest: [u8; 32],
}

impl<'a> ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1<'a> {
    /// Construct a canonical cross-field/lookup view for one exact transcript.
    ///
    /// # Errors
    ///
    /// Rejects an empty/oversized proof or any zero/aliased metadata digest.
    pub fn new(
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
        point_evaluation_digests: &[[u8; 32]],
        limb_relation_digests: &[[u8; 32]],
        sumcheck_round_digests: &[[u8; 32]],
        proof: &'a [u8],
    ) -> Result<Self, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        let section = Self {
            header: canonical_header_v1(transcript)?,
            cross_field_challenge_seed: transcript.cross_field_challenge_seed(),
            global_lookup_challenge_seed: transcript.global_lookup_challenge_seed(),
            cross_field_root: transcript.cross_field_root(),
            global_lookup_root: transcript.global_lookup_root(),
            point_evaluation_digests: exact_digest_array_v1(point_evaluation_digests)?,
            limb_relation_digests: exact_digest_array_v1(limb_relation_digests)?,
            sumcheck_round_digests: exact_digest_array_v1(sumcheck_round_digests)?,
            proof,
            proof_digest: proof_body_digest_v1(
                ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup,
                proof,
            )?,
        };
        section.validate_v1()?;
        Ok(section)
    }

    /// Decode exactly one canonical cross-field/lookup section without allocation.
    ///
    /// # Errors
    ///
    /// Rejects every cap, truncation, count, order, context, alias, integrity,
    /// overflow, or trailing-byte violation.
    pub fn from_canonical_bytes_exact_v1(
        bytes: &'a [u8],
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        let kind = ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup;
        preflight_v1(bytes, kind, CROSS_LOOKUP_FIXED_BYTES_V1 + 1)?;
        let expected_header = canonical_header_v1(transcript)?;
        let mut decoder = DecoderV1::new(bytes);
        let header = read_header_v1(&mut decoder, kind, bytes.len(), expected_header)?;
        if decoder.u8()? != ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_POINT_COUNT_V1
            || usize::from(decoder.u8()?) != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || decoder.u8()? != ZK_AMS_MKHE_RNS_NATIVE_SUMCHECK_ROUNDS_V1
        {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount);
        }
        let cross_field_challenge_seed = decoder.array()?;
        let global_lookup_challenge_seed = decoder.array()?;
        let cross_field_root = decoder.array()?;
        let global_lookup_root = decoder.array()?;
        if cross_field_challenge_seed != transcript.cross_field_challenge_seed()
            || global_lookup_challenge_seed != transcript.global_lookup_challenge_seed()
            || cross_field_root != transcript.cross_field_root()
            || global_lookup_root != transcript.global_lookup_root()
        {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch);
        }
        let mut registry = DigestRegistryV1::new();
        insert_header_digests_v1(&mut registry, header)?;
        for digest in [
            cross_field_challenge_seed,
            global_lookup_challenge_seed,
            cross_field_root,
            global_lookup_root,
        ] {
            registry.insert(digest)?;
        }
        let point_evaluation_digests = read_indexed_u8_v1(&mut decoder, &mut registry)?;
        let limb_relation_digests = read_indexed_u8_v1(&mut decoder, &mut registry)?;
        let sumcheck_round_digests = read_indexed_u8_v1(&mut decoder, &mut registry)?;
        let (proof, proof_digest) = read_proof_v1(&mut decoder, kind, &mut registry)?;
        finish_decoding_v1(&mut decoder, kind, &mut registry)?;
        Ok(Self {
            header,
            cross_field_challenge_seed,
            global_lookup_challenge_seed,
            cross_field_root,
            global_lookup_root,
            point_evaluation_digests,
            limb_relation_digests,
            sumcheck_round_digests,
            proof,
            proof_digest,
        })
    }

    /// Encode the exact canonical cross-field/lookup section after a cap preflight.
    ///
    /// # Errors
    ///
    /// Rejects invalid metadata, proof length, aliases, or bounded allocation failure.
    pub fn to_canonical_bytes_v1(&self) -> Result<Vec<u8>, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        self.validate_v1()?;
        let kind = ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup;
        let (mut bytes, total, proof_digest) =
            begin_encoding_v1(kind, CROSS_LOOKUP_FIXED_BYTES_V1, self.proof, self.header)?;
        bytes.push(ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_POINT_COUNT_V1);
        bytes.push(
            u8::try_from(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
                .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount)?,
        );
        bytes.push(ZK_AMS_MKHE_RNS_NATIVE_SUMCHECK_ROUNDS_V1);
        for digest in [
            self.cross_field_challenge_seed,
            self.global_lookup_challenge_seed,
            self.cross_field_root,
            self.global_lookup_root,
        ] {
            bytes.extend_from_slice(&digest);
        }
        write_indexed_u8_v1(&mut bytes, &self.point_evaluation_digests)?;
        write_indexed_u8_v1(&mut bytes, &self.limb_relation_digests)?;
        write_indexed_u8_v1(&mut bytes, &self.sumcheck_round_digests)?;
        write_proof_v1(&mut bytes, self.proof, proof_digest)?;
        finish_encoding_v1(bytes, kind, total)
    }

    /// Borrow the five ordered cross-field point-evaluation digests.
    #[must_use]
    pub const fn point_evaluation_digests(&self) -> &[[u8; 32]] {
        &self.point_evaluation_digests
    }

    /// Borrow the forty ordered cross-field limb-relation digests.
    #[must_use]
    pub const fn limb_relation_digests(&self) -> &[[u8; 32]] {
        &self.limb_relation_digests
    }

    /// Borrow the twenty-nine ordered global-lookup sumcheck round digests.
    #[must_use]
    pub const fn sumcheck_round_digests(&self) -> &[[u8; 32]] {
        &self.sumcheck_round_digests
    }

    /// Borrow the opaque cryptographic cross-field/lookup proof bytes.
    #[must_use]
    pub const fn proof(&self) -> &'a [u8] {
        self.proof
    }

    fn validate_v1(&self) -> Result<(), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        encoded_len_v1(
            ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup,
            CROSS_LOOKUP_FIXED_BYTES_V1,
            self.proof.len(),
        )?;
        let mut registry = DigestRegistryV1::new();
        insert_header_digests_v1(&mut registry, self.header)?;
        for digest in [
            self.cross_field_challenge_seed,
            self.global_lookup_challenge_seed,
            self.cross_field_root,
            self.global_lookup_root,
        ] {
            registry.insert(digest)?;
        }
        for digest in self
            .point_evaluation_digests
            .into_iter()
            .chain(self.limb_relation_digests)
            .chain(self.sumcheck_round_digests)
        {
            registry.insert(digest)?;
        }
        if self.proof_digest
            != proof_body_digest_v1(
                ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup,
                self.proof,
            )?
        {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::Integrity);
        }
        registry.insert(self.proof_digest)?;
        Ok(())
    }
}

/// Borrowed, typed forty-limb zero-padding proof section.
///
/// This is a canonical transport view and never attests that padding is zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeZeroPaddingSectionV1<'a> {
    header: SectionHeaderV1,
    zero_padding_challenge_seed: [u8; 32],
    composite_binding_challenge_seed: [u8; 32],
    zero_padding_root: [u8; 32],
    limb_padding_digests: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    proof: &'a [u8],
    proof_digest: [u8; 32],
}

impl<'a> ZkAmsMkheRnsNativeZeroPaddingSectionV1<'a> {
    /// Construct a canonical zero-padding transport view for one exact transcript.
    ///
    /// # Errors
    ///
    /// Rejects an empty/oversized proof or any zero/aliased metadata digest.
    pub fn new(
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
        limb_padding_digests: &[[u8; 32]],
        proof: &'a [u8],
    ) -> Result<Self, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        let section = Self {
            header: canonical_header_v1(transcript)?,
            zero_padding_challenge_seed: transcript.zero_padding_challenge_seed(),
            composite_binding_challenge_seed: transcript.composite_binding_challenge_seed(),
            zero_padding_root: transcript.zero_padding_root(),
            limb_padding_digests: exact_digest_array_v1(limb_padding_digests)?,
            proof,
            proof_digest: proof_body_digest_v1(
                ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding,
                proof,
            )?,
        };
        section.validate_v1()?;
        Ok(section)
    }

    /// Decode exactly one canonical zero-padding section without allocation.
    ///
    /// # Errors
    ///
    /// Rejects every cap, truncation, count, order, context, alias, integrity,
    /// overflow, or trailing-byte violation.
    pub fn from_canonical_bytes_exact_v1(
        bytes: &'a [u8],
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        let kind = ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding;
        preflight_v1(bytes, kind, ZERO_PADDING_FIXED_BYTES_V1 + 1)?;
        let expected_header = canonical_header_v1(transcript)?;
        let mut decoder = DecoderV1::new(bytes);
        let header = read_header_v1(&mut decoder, kind, bytes.len(), expected_header)?;
        if usize::from(decoder.u8()?) != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount);
        }
        let zero_padding_challenge_seed = decoder.array()?;
        let composite_binding_challenge_seed = decoder.array()?;
        let zero_padding_root = decoder.array()?;
        if zero_padding_challenge_seed != transcript.zero_padding_challenge_seed()
            || composite_binding_challenge_seed != transcript.composite_binding_challenge_seed()
            || zero_padding_root != transcript.zero_padding_root()
        {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::ContextMismatch);
        }
        let mut registry = DigestRegistryV1::new();
        insert_header_digests_v1(&mut registry, header)?;
        for digest in [
            zero_padding_challenge_seed,
            composite_binding_challenge_seed,
            zero_padding_root,
        ] {
            registry.insert(digest)?;
        }
        let limb_padding_digests = read_indexed_u8_v1(&mut decoder, &mut registry)?;
        let (proof, proof_digest) = read_proof_v1(&mut decoder, kind, &mut registry)?;
        finish_decoding_v1(&mut decoder, kind, &mut registry)?;
        Ok(Self {
            header,
            zero_padding_challenge_seed,
            composite_binding_challenge_seed,
            zero_padding_root,
            limb_padding_digests,
            proof,
            proof_digest,
        })
    }

    /// Encode the exact canonical zero-padding section after a cap preflight.
    ///
    /// # Errors
    ///
    /// Rejects invalid metadata, proof length, aliases, or bounded allocation failure.
    pub fn to_canonical_bytes_v1(&self) -> Result<Vec<u8>, ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        self.validate_v1()?;
        let kind = ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding;
        let (mut bytes, total, proof_digest) =
            begin_encoding_v1(kind, ZERO_PADDING_FIXED_BYTES_V1, self.proof, self.header)?;
        bytes.push(
            u8::try_from(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
                .map_err(|_| ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidCount)?,
        );
        for digest in [
            self.zero_padding_challenge_seed,
            self.composite_binding_challenge_seed,
            self.zero_padding_root,
        ] {
            bytes.extend_from_slice(&digest);
        }
        write_indexed_u8_v1(&mut bytes, &self.limb_padding_digests)?;
        write_proof_v1(&mut bytes, self.proof, proof_digest)?;
        finish_encoding_v1(bytes, kind, total)
    }

    /// Borrow the forty ordered padding-limb commitment digests.
    #[must_use]
    pub const fn limb_padding_digests(&self) -> &[[u8; 32]] {
        &self.limb_padding_digests
    }

    /// Borrow the opaque cryptographic zero-padding proof bytes.
    #[must_use]
    pub const fn proof(&self) -> &'a [u8] {
        self.proof
    }

    fn validate_v1(&self) -> Result<(), ZkAmsMkheRnsNativeSectionCodecErrorV1> {
        encoded_len_v1(
            ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding,
            ZERO_PADDING_FIXED_BYTES_V1,
            self.proof.len(),
        )?;
        let mut registry = DigestRegistryV1::new();
        insert_header_digests_v1(&mut registry, self.header)?;
        for digest in [
            self.zero_padding_challenge_seed,
            self.composite_binding_challenge_seed,
            self.zero_padding_root,
        ] {
            registry.insert(digest)?;
        }
        for digest in self.limb_padding_digests {
            registry.insert(digest)?;
        }
        if self.proof_digest
            != proof_body_digest_v1(
                ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding,
                self.proof,
            )?
        {
            return Err(ZkAmsMkheRnsNativeSectionCodecErrorV1::Integrity);
        }
        registry.insert(self.proof_digest)?;
        Ok(())
    }
}

/// Internal classification for one atomic four-section decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CompositeSectionSetErrorV1 {
    /// One typed section failed before cryptographic verification.
    Section(ZkAmsMkheRnsNativeProofSectionKindV1),
    /// A semantic digest was reused across otherwise-valid sections.
    CrossSectionAlias,
}

/// Decode the complete four-section set and reject cross-section digest reuse.
pub(super) fn validate_composite_section_set_exact_v1(
    terminal_bytes: &[u8],
    rns_qpcs_bytes: &[u8],
    cross_lookup_bytes: &[u8],
    zero_padding_bytes: &[u8],
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    outer_section_digests: &[[u8; 32]; 4],
    outer_proof_digest: [u8; 32],
) -> Result<(), CompositeSectionSetErrorV1> {
    let terminal = ZkAmsMkheRnsNativeTerminalBridgeSectionV1::from_canonical_bytes_exact_v1(
        terminal_bytes,
        transcript,
    )
    .map_err(|_| {
        CompositeSectionSetErrorV1::Section(
            ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge,
        )
    })?;
    let rns = ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::from_canonical_bytes_exact_v1(
        rns_qpcs_bytes,
        transcript,
    )
    .map_err(|_| {
        CompositeSectionSetErrorV1::Section(ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs)
    })?;
    let cross = ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1::from_canonical_bytes_exact_v1(
        cross_lookup_bytes,
        transcript,
    )
    .map_err(|_| {
        CompositeSectionSetErrorV1::Section(
            ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup,
        )
    })?;
    let padding = ZkAmsMkheRnsNativeZeroPaddingSectionV1::from_canonical_bytes_exact_v1(
        zero_padding_bytes,
        transcript,
    )
    .map_err(|_| {
        CompositeSectionSetErrorV1::Section(ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding)
    })?;

    (|| {
        let mut registry = DigestRegistryV1::new();
        insert_header_digests_v1(&mut registry, terminal.header)?;
        for digest in [
            transcript.statement_digest(),
            transcript.operational_context_digest(),
            transcript.source_binding_digest(),
            transcript.main_snapshot_digest(),
            transcript.nonce_snapshot_digest(),
            transcript.source_receipt_digest(),
            transcript.governed_roster_digest(),
            transcript.public_ciphertext_digest(),
        ] {
            registry.insert(digest)?;
        }
        for digest in *outer_section_digests {
            registry.insert(digest)?;
        }
        registry.insert(outer_proof_digest)?;
        for digest in transcript.ordered_challenge_seeds() {
            registry.insert(digest)?;
        }
        for opening in transcript.opening_commitments() {
            registry.insert(opening.source_commitment_digest())?;
            registry.insert(opening.hyrax_commitment_digest())?;
        }
        for digest in [
            transcript.mapping_root(),
            transcript.terminal_hyrax_root(),
            transcript.cross_basis_bridge_root(),
            transcript.qpcs_initial_root(),
            transcript.q_mask_s_root(),
            transcript.qpcs_quotient_root(),
            transcript.cross_field_root(),
            transcript.global_lookup_root(),
            transcript.zero_padding_root(),
        ] {
            registry.insert(digest)?;
        }
        for root in transcript.qpcs_fri_roots() {
            registry.insert(root.root())?;
        }
        for digest in rns
            .equation_commitment_digests
            .into_iter()
            .chain(rns.limb_commitment_digests)
            .chain(rns.query_opening_digests)
            .chain(cross.point_evaluation_digests)
            .chain(cross.limb_relation_digests)
            .chain(cross.sumcheck_round_digests)
            .chain(padding.limb_padding_digests)
        {
            registry.insert(digest)?;
        }
        for digest in [
            terminal.proof_digest,
            rns.proof_digest,
            cross.proof_digest,
            padding.proof_digest,
        ] {
            registry.insert(digest)?;
        }
        for bytes in [
            terminal_bytes,
            rns_qpcs_bytes,
            cross_lookup_bytes,
            zero_padding_bytes,
        ] {
            let digest = bytes
                .get(bytes.len().saturating_sub(CODEC_DIGEST_BYTES_V1)..)
                .and_then(|value| value.try_into().ok())
                .ok_or(ZkAmsMkheRnsNativeSectionCodecErrorV1::InvalidEncoding)?;
            registry.insert(digest)?;
        }
        Ok(())
    })()
    .map_err(|_: ZkAmsMkheRnsNativeSectionCodecErrorV1| {
        CompositeSectionSetErrorV1::CrossSectionAlias
    })
}

#[cfg(test)]
#[path = "rns_native_section_codec_tests.rs"]
mod tests;
