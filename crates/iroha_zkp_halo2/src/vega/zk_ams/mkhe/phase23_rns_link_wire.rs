//! Canonical release-shape transport for one unverified RNS-Link envelope.
//!
//! This module freezes only the bounded byte-level envelope. It does not prove or verify any
//! Phase-II/III algebra and cannot mint a receipt or open a readiness gate. The eventual algebraic
//! verifier must independently check every committed packing, radix/CRT carry, negacyclic-quotient,
//! and Hyrax-to-BGV equality relation before treating a decoded value as evidence.
//!
//! The sole v1 order is `X, U, E, RE, W, RW`, followed by one 38-record
//! packing section, one 38-record radix/CRT-carry section, one 38-record
//! negacyclic-quotient section, and one 38-record Hyrax-to-BGV equality
//! section. Native geometry fixes `X` at 89 values in one chunk and keeps `U`
//! replicated over all 1,048,576 relation rows in 16 chunks; the six families
//! therefore contain 43 records. Every point is an exact non-identity T256
//! encoding, every scalar is canonical, and every public blinding response is
//! nonzero. Preflight checks all counts, offsets, lengths, padding metadata,
//! and canonical field/group encodings without allocating attacker-sized
//! storage.
//! The bound decoder gives the header's statement digest one meaning only: the
//! verifier-derived digest of the complete release RNS challenge set.

use super::{
    ZkAmsMkheErrorV1,
    phase23_rns_link::{
        ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1, ZkAmsPhase23RnsLinkChunkCommitmentV1,
        ZkAmsPhase23RnsLinkContextV1, ZkAmsPhase23RnsLinkFamilyV1,
        ZkAmsPhase23RnsLinkWholeProofBindingV1, expected_logical_values_v1,
    },
};
use crate::vega::{
    MaskedRelaxedRandomSourceV1, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
    bulletproof_t256::{ZeroizingT256ScalarCopyV1, ZeroizingT256ScalarVecV1},
};
use core::fmt;
const WHOLE_PROOF_MAGIC_V1: [u8; 8] = *b"ZKRNLNK1";
const WHOLE_PROOF_VERSION_V1: u8 = 1;
const WHOLE_PROOF_FLAGS_V1: u8 = 0;
const WHOLE_PROOF_SECTION_FLAGS_V1: u8 = 0;
const WHOLE_PROOF_RECORD_FLAGS_V1: u8 = 0;
const WHOLE_PROOF_FAMILY_COUNT_V1: usize = 6;
const WHOLE_PROOF_SECTION_COUNT_V1: usize = 10;
const WHOLE_PROOF_RNS_LIMB_COUNT_V1: usize = ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1;
const WHOLE_PROOF_SLOT_COUNT_V1: usize = 65_536;
const WHOLE_PROOF_MAX_FAMILY_CHUNKS_V1: usize = 16;
const WHOLE_PROOF_HYRAX_EQUALITY_RECORDS_V1: usize = WHOLE_PROOF_RNS_LIMB_COUNT_V1;
const WHOLE_PROOF_MAX_SECTION_RECORDS_V1: usize = WHOLE_PROOF_RNS_LIMB_COUNT_V1;
const WHOLE_PROOF_INTERNAL_BLINDING_COUNT_V1: usize = 196;
const WHOLE_PROOF_BLINDING_REJECTION_ATTEMPTS_V1: usize = 128;
const WHOLE_PROOF_HEADER_BYTES_V1: usize = 213;
const WHOLE_PROOF_SECTION_HEADER_BYTES_V1: usize = 8;
const WHOLE_PROOF_FAMILY_RECORD_BYTES_V1: usize = 111;
const WHOLE_PROOF_RELATION_RECORD_BYTES_V1: usize = 101;
const WHOLE_PROOF_FAMILY_RECORD_COUNT_V1: usize = 43;
const WHOLE_PROOF_RELATION_RECORD_COUNT_V1: usize = 152;
const WHOLE_PROOF_EXACT_BYTES_V1: usize = WHOLE_PROOF_HEADER_BYTES_V1
    + WHOLE_PROOF_SECTION_COUNT_V1 * WHOLE_PROOF_SECTION_HEADER_BYTES_V1
    + WHOLE_PROOF_FAMILY_RECORD_COUNT_V1 * WHOLE_PROOF_FAMILY_RECORD_BYTES_V1
    + WHOLE_PROOF_RELATION_RECORD_COUNT_V1 * WHOLE_PROOF_RELATION_RECORD_BYTES_V1;
/// Exact and maximum canonical byte length of the frozen release-shape structural envelope.
///
/// This is not a measured algebraic-proof size and is never resource evidence.
pub(super) const ZK_AMS_PHASE23_RNS_LINK_WHOLE_PROOF_MAX_BYTES_V1: usize =
    WHOLE_PROOF_EXACT_BYTES_V1;
const HEADER_FLAGS_OFFSET_V1: usize = 9;
const HEADER_LENGTH_OFFSET_V1: usize = 10;
const HEADER_TOTAL_LENGTH_OFFSET_V1: usize = 12;
const HEADER_SECTION_COUNT_OFFSET_V1: usize = 16;
const HEADER_FAMILY_COUNT_OFFSET_V1: usize = 17;
const HEADER_LIMB_COUNT_OFFSET_V1: usize = 18;
const HEADER_RESERVED_OFFSET_V1: usize = 19;
const HEADER_PROOF_BLINDING_COMMITMENT_OFFSET_V1: usize = 180;
const SECTION_FLAGS_OFFSET_V1: usize = 1;
const SECTION_RECORD_COUNT_OFFSET_V1: usize = 2;
const SECTION_PAYLOAD_LENGTH_OFFSET_V1: usize = 4;
const FAMILY_RECORD_FLAGS_OFFSET_V1: usize = 1;
const FAMILY_RECORD_LOGICAL_OFFSET_OFFSET_V1: usize = 2;
const FAMILY_RECORD_USED_VALUES_OFFSET_V1: usize = 6;
const FAMILY_RECORD_ZERO_PADDING_OFFSET_V1: usize = 10;
const FAMILY_RECORD_COMMITMENT_OFFSET_V1: usize = 14;
const FAMILY_RECORD_EVALUATION_OFFSET_V1: usize = 47;
const FAMILY_RECORD_BLINDING_RESPONSE_OFFSET_V1: usize = 79;
const RELATION_RECORD_FLAGS_OFFSET_V1: usize = 1;
const RELATION_RECORD_ARITY_OFFSET_V1: usize = 2;
const RELATION_RECORD_COMMITMENT_OFFSET_V1: usize = 4;
const RELATION_RECORD_EVALUATION_OFFSET_V1: usize = 37;
const RELATION_RECORD_BLINDING_RESPONSE_OFFSET_V1: usize = 69;
const _: () = {
    assert!(WHOLE_PROOF_FAMILY_COUNT_V1 == 6);
    assert!(WHOLE_PROOF_SECTION_COUNT_V1 == 10);
    assert!(WHOLE_PROOF_RNS_LIMB_COUNT_V1 == 38);
    assert!(WHOLE_PROOF_MAX_FAMILY_CHUNKS_V1 == 16);
    assert!(WHOLE_PROOF_MAX_SECTION_RECORDS_V1 == 38);
    assert!(WHOLE_PROOF_FAMILY_RECORD_COUNT_V1 == 1 + 16 + 16 + 1 + 8 + 1);
    assert!(WHOLE_PROOF_RELATION_RECORD_COUNT_V1 == 4 * 38);
    assert!(WHOLE_PROOF_INTERNAL_BLINDING_COUNT_V1 == 1 + 43 + 152);
    assert!(WHOLE_PROOF_EXACT_BYTES_V1 == 20_418);
    assert!(WHOLE_PROOF_EXACT_BYTES_V1 < 32 * 1024 * 1024);
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum WholeProofFamilyV1 {
    X = 1,
    U = 2,
    E = 3,
    RE = 4,
    W = 5,
    RW = 6,
}
impl WholeProofFamilyV1 {
    const fn logical_values(self) -> usize {
        expected_logical_values_v1(match self {
            Self::X => ZkAmsPhase23RnsLinkFamilyV1::X,
            Self::U => ZkAmsPhase23RnsLinkFamilyV1::U,
            Self::E => ZkAmsPhase23RnsLinkFamilyV1::E,
            Self::RE => ZkAmsPhase23RnsLinkFamilyV1::RE,
            Self::W => ZkAmsPhase23RnsLinkFamilyV1::W,
            Self::RW => ZkAmsPhase23RnsLinkFamilyV1::RW,
        })
    }
    const fn chunk_count(self) -> usize {
        self.logical_values().div_ceil(WHOLE_PROOF_SLOT_COUNT_V1)
    }
}
const WHOLE_PROOF_FAMILY_ORDER_V1: [WholeProofFamilyV1; WHOLE_PROOF_FAMILY_COUNT_V1] = [
    WholeProofFamilyV1::X,
    WholeProofFamilyV1::U,
    WholeProofFamilyV1::E,
    WholeProofFamilyV1::RE,
    WholeProofFamilyV1::W,
    WholeProofFamilyV1::RW,
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum WholeProofRelationSectionV1 {
    Packing = 7,
    RadixCrtCarry = 8,
    NegacyclicQuotient = 9,
    HyraxBgvEquality = 10,
}
impl WholeProofRelationSectionV1 {
    const fn record_count(self) -> usize {
        match self {
            Self::Packing | Self::RadixCrtCarry | Self::NegacyclicQuotient => {
                WHOLE_PROOF_RNS_LIMB_COUNT_V1
            }
            Self::HyraxBgvEquality => WHOLE_PROOF_HYRAX_EQUALITY_RECORDS_V1,
        }
    }
    const fn relation_arity(self) -> u16 {
        match self {
            // Five transcript-derived evaluation points are frozen per RNS limb.
            Self::Packing => 5,
            // Input digit, output digit, and exact carry form one radix equation.
            Self::RadixCrtCarry => 3,
            // A, R/E/M-C, quotient form the batched negacyclic equation.
            Self::NegacyclicQuotient => 4,
            // Reserve arity metadata for six Hyrax and six BGV inputs per RNS
            // limb in fixed X/U/E/RE/W/RW order. This structural record does
            // not yet carry or verify those opening equations.
            Self::HyraxBgvEquality => 12,
        }
    }
}
const WHOLE_PROOF_RELATION_ORDER_V1: [WholeProofRelationSectionV1; 4] = [
    WholeProofRelationSectionV1::Packing,
    WholeProofRelationSectionV1::RadixCrtCarry,
    WholeProofRelationSectionV1::NegacyclicQuotient,
    WholeProofRelationSectionV1::HyraxBgvEquality,
];
const _: () = {
    assert!(WholeProofFamilyV1::X.logical_values() == 89);
    assert!(WholeProofFamilyV1::X.chunk_count() == 1);
    assert!(WholeProofFamilyV1::U.logical_values() == 1_048_576);
    assert!(WholeProofFamilyV1::U.chunk_count() == 16);
    assert!(WholeProofFamilyV1::E.logical_values() == 1_048_576);
    assert!(WholeProofFamilyV1::E.chunk_count() == 16);
    assert!(WholeProofFamilyV1::RE.logical_values() == 1_024);
    assert!(WholeProofFamilyV1::RE.chunk_count() == 1);
    assert!(WholeProofFamilyV1::W.logical_values() == 524_288);
    assert!(WholeProofFamilyV1::W.chunk_count() == 8);
    assert!(WholeProofFamilyV1::RW.logical_values() == 512);
    assert!(WholeProofFamilyV1::RW.chunk_count() == 1);
};
#[derive(Clone, Debug, PartialEq, Eq)]
struct WholeProofFamilyRecordV1 {
    ordinal: u8,
    logical_offset: u32,
    used_values: u32,
    zero_padding_values: u32,
    commitment: Point,
    evaluation: Scalar,
    blinding_response: Scalar,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct WholeProofFamilySectionV1 {
    family: WholeProofFamilyV1,
    records: Vec<WholeProofFamilyRecordV1>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct WholeProofRelationRecordV1 {
    ordinal: u8,
    relation_arity: u16,
    commitment: Point,
    evaluation: Scalar,
    blinding_response: Scalar,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct WholeProofRelationRecordsV1 {
    section: WholeProofRelationSectionV1,
    records: Vec<WholeProofRelationRecordV1>,
}
/// Structurally decoded but cryptographically unverified whole-proof envelope.
///
/// No constructor is exposed: decoding proves canonical transport shape only. It is not evidence of
/// any RNS-Link equation and cannot be converted into a verified receipt by this module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1 {
    profile_digest: [u8; 32],
    algorithm_manifest_digest: [u8; 32],
    context_digest: [u8; 32],
    statement_digest: [u8; 32],
    commitment_root: [u8; 32],
    proof_blinding_commitment: Point,
    family_sections: [WholeProofFamilySectionV1; WHOLE_PROOF_FAMILY_COUNT_V1],
    relation_sections: [WholeProofRelationRecordsV1; 4],
}
impl ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1 {
    /// Decode exactly after an allocation-free structural and canonical
    /// preflight. The returned value remains explicitly unverified.
    pub(super) fn decode_exact(bytes: &[u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        preflight_zk_ams_phase23_rns_link_whole_proof_v1(bytes)?;
        let mut decoder = WholeProofDecoderV1::new(bytes);
        decoder.expect_bytes(&WHOLE_PROOF_MAGIC_V1)?;
        decoder.expect_u8(WHOLE_PROOF_VERSION_V1)?;
        decoder.expect_u8(WHOLE_PROOF_FLAGS_V1)?;
        decoder.expect_u16(as_u16(WHOLE_PROOF_HEADER_BYTES_V1)?)?;
        decoder.expect_u32(as_u32(WHOLE_PROOF_EXACT_BYTES_V1)?)?;
        decoder.expect_u8(as_u8(WHOLE_PROOF_SECTION_COUNT_V1)?)?;
        decoder.expect_u8(as_u8(WHOLE_PROOF_FAMILY_COUNT_V1)?)?;
        decoder.expect_u8(as_u8(WHOLE_PROOF_RNS_LIMB_COUNT_V1)?)?;
        decoder.expect_u8(0)?;
        let profile_digest = decoder.array()?;
        let algorithm_manifest_digest = decoder.array()?;
        let context_digest = decoder.array()?;
        let statement_digest = decoder.array()?;
        let commitment_root = decoder.array()?;
        let proof_blinding_commitment = decoder.point()?;
        let mut family_sections = Vec::new();
        reserve_decode_records(&mut family_sections, WHOLE_PROOF_FAMILY_COUNT_V1)?;
        for family in WHOLE_PROOF_FAMILY_ORDER_V1 {
            decoder.expect_u8(family as u8)?;
            decoder.expect_u8(WHOLE_PROOF_SECTION_FLAGS_V1)?;
            decoder.expect_u16(as_u16(family.chunk_count())?)?;
            decoder.expect_u32(as_u32(
                family
                    .chunk_count()
                    .checked_mul(WHOLE_PROOF_FAMILY_RECORD_BYTES_V1)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )?)?;
            let mut records = Vec::new();
            reserve_decode_records(&mut records, family.chunk_count())?;
            for ordinal in 0..family.chunk_count() {
                let (logical_offset, used_values, zero_padding_values) =
                    expected_family_record_shape(family, ordinal)?;
                records.push(WholeProofFamilyRecordV1 {
                    ordinal: decoder.u8()?,
                    // Preflight already established the sole accepted value.
                    // Consume it again so the owned decode stays byte-aligned.
                    logical_offset: {
                        decoder.expect_u8(WHOLE_PROOF_RECORD_FLAGS_V1)?;
                        decoder.u32()?
                    },
                    used_values: decoder.u32()?,
                    zero_padding_values: decoder.u32()?,
                    commitment: decoder.point()?,
                    evaluation: decoder.scalar()?,
                    blinding_response: decoder.scalar()?,
                });
                let decoded = records
                    .last()
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                if decoded.ordinal != as_u8(ordinal)?
                    || decoded.logical_offset != logical_offset
                    || decoded.used_values != used_values
                    || decoded.zero_padding_values != zero_padding_values
                {
                    return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
                }
            }
            family_sections.push(WholeProofFamilySectionV1 { family, records });
        }
        let mut relation_sections = Vec::new();
        reserve_decode_records(&mut relation_sections, WHOLE_PROOF_RELATION_ORDER_V1.len())?;
        for section in WHOLE_PROOF_RELATION_ORDER_V1 {
            decoder.expect_u8(section as u8)?;
            decoder.expect_u8(WHOLE_PROOF_SECTION_FLAGS_V1)?;
            decoder.expect_u16(as_u16(section.record_count())?)?;
            decoder.expect_u32(as_u32(
                section
                    .record_count()
                    .checked_mul(WHOLE_PROOF_RELATION_RECORD_BYTES_V1)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )?)?;
            let mut records = Vec::new();
            reserve_decode_records(&mut records, section.record_count())?;
            for ordinal in 0..section.record_count() {
                records.push(WholeProofRelationRecordV1 {
                    ordinal: decoder.u8()?,
                    relation_arity: {
                        decoder.expect_u8(WHOLE_PROOF_RECORD_FLAGS_V1)?;
                        decoder.u16()?
                    },
                    commitment: decoder.point()?,
                    evaluation: decoder.scalar()?,
                    blinding_response: decoder.scalar()?,
                });
                let decoded = records
                    .last()
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                if decoded.ordinal != as_u8(ordinal)?
                    || decoded.relation_arity != section.relation_arity()
                {
                    return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
                }
            }
            relation_sections.push(WholeProofRelationRecordsV1 { section, records });
        }
        decoder.finish()?;
        let value = Self {
            profile_digest,
            algorithm_manifest_digest,
            context_digest,
            statement_digest,
            commitment_root,
            proof_blinding_commitment,
            family_sections: family_sections
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            relation_sections: relation_sections
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        };
        value.validate_structure()?;
        let canonical = value.encode_canonical()?;
        if canonical.as_slice() != bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(value)
    }
    /// Decode canonically and bind the envelope to verifier-owned release inputs.
    ///
    /// This recomputes the profile, immutable algorithm manifest, context, ordered commitment root,
    /// complete 38-limb challenge-set digest, and all 43 Hyrax chunk commitments through the native
    /// relation types. Passing this check only establishes that the envelope is the transport for
    /// those inputs. The returned type remains explicitly unverified because the packing, carry,
    /// quotient, and Hyrax-to-BGV response equations are not yet represented by this envelope.
    pub(super) fn decode_exact_bound_unverified(
        bytes: &[u8],
        context: &ZkAmsPhase23RnsLinkContextV1,
        commitments: &[ZkAmsPhase23RnsLinkChunkCommitmentV1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        // Preserve the cheap malformed-input boundary before deriving the
        // release challenge set or allocating owned proof records.
        preflight_zk_ams_phase23_rns_link_whole_proof_v1(bytes)?;
        let value = Self::decode_exact(bytes)?;
        let binding = ZkAmsPhase23RnsLinkWholeProofBindingV1::derive(context, commitments)?;
        value.validate_release_binding(&binding)?;
        Ok(value)
    }
    /// Encode the sole canonical structural representation. This operation
    /// does not claim that any encoded algebraic relation is true.
    pub(super) fn encode_canonical(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        self.validate_structure()?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(WHOLE_PROOF_EXACT_BYTES_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        bytes.extend_from_slice(&WHOLE_PROOF_MAGIC_V1);
        bytes.push(WHOLE_PROOF_VERSION_V1);
        bytes.push(WHOLE_PROOF_FLAGS_V1);
        push_u16(&mut bytes, as_u16(WHOLE_PROOF_HEADER_BYTES_V1)?);
        push_u32(&mut bytes, as_u32(WHOLE_PROOF_EXACT_BYTES_V1)?);
        bytes.push(as_u8(WHOLE_PROOF_SECTION_COUNT_V1)?);
        bytes.push(as_u8(WHOLE_PROOF_FAMILY_COUNT_V1)?);
        bytes.push(as_u8(WHOLE_PROOF_RNS_LIMB_COUNT_V1)?);
        bytes.push(0);
        bytes.extend_from_slice(&self.profile_digest);
        bytes.extend_from_slice(&self.algorithm_manifest_digest);
        bytes.extend_from_slice(&self.context_digest);
        bytes.extend_from_slice(&self.statement_digest);
        bytes.extend_from_slice(&self.commitment_root);
        push_point(&mut bytes, self.proof_blinding_commitment)?;
        for section in &self.family_sections {
            bytes.push(section.family as u8);
            bytes.push(WHOLE_PROOF_SECTION_FLAGS_V1);
            push_u16(&mut bytes, as_u16(section.records.len())?);
            push_u32(
                &mut bytes,
                as_u32(
                    section
                        .records
                        .len()
                        .checked_mul(WHOLE_PROOF_FAMILY_RECORD_BYTES_V1)
                        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                )?,
            );
            for record in &section.records {
                bytes.push(record.ordinal);
                bytes.push(WHOLE_PROOF_RECORD_FLAGS_V1);
                push_u32(&mut bytes, record.logical_offset);
                push_u32(&mut bytes, record.used_values);
                push_u32(&mut bytes, record.zero_padding_values);
                push_point(&mut bytes, record.commitment)?;
                bytes.extend_from_slice(&record.evaluation.to_be_bytes());
                bytes.extend_from_slice(&record.blinding_response.to_be_bytes());
            }
        }
        for section in &self.relation_sections {
            bytes.push(section.section as u8);
            bytes.push(WHOLE_PROOF_SECTION_FLAGS_V1);
            push_u16(&mut bytes, as_u16(section.records.len())?);
            push_u32(
                &mut bytes,
                as_u32(
                    section
                        .records
                        .len()
                        .checked_mul(WHOLE_PROOF_RELATION_RECORD_BYTES_V1)
                        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                )?,
            );
            for record in &section.records {
                bytes.push(record.ordinal);
                bytes.push(WHOLE_PROOF_RECORD_FLAGS_V1);
                push_u16(&mut bytes, record.relation_arity);
                push_point(&mut bytes, record.commitment)?;
                bytes.extend_from_slice(&record.evaluation.to_be_bytes());
                bytes.extend_from_slice(&record.blinding_response.to_be_bytes());
            }
        }
        if bytes.len() != WHOLE_PROOF_EXACT_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        preflight_zk_ams_phase23_rns_link_whole_proof_v1(&bytes)?;
        Ok(bytes)
    }
    fn validate_structure(&self) -> Result<(), ZkAmsMkheErrorV1> {
        if [
            self.profile_digest,
            self.algorithm_manifest_digest,
            self.context_digest,
            self.statement_digest,
            self.commitment_root,
        ]
        .contains(&[0; 32])
            || self.proof_blinding_commitment.is_identity()
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        for (expected_family, section) in WHOLE_PROOF_FAMILY_ORDER_V1
            .into_iter()
            .zip(&self.family_sections)
        {
            if section.family != expected_family
                || section.records.len() != expected_family.chunk_count()
            {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            for (ordinal, record) in section.records.iter().enumerate() {
                let (logical_offset, used_values, zero_padding_values) =
                    expected_family_record_shape(expected_family, ordinal)?;
                if record.ordinal != as_u8(ordinal)?
                    || record.logical_offset != logical_offset
                    || record.used_values != used_values
                    || record.zero_padding_values != zero_padding_values
                    || record.commitment.is_identity()
                    || record.blinding_response.is_zero()
                {
                    return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
                }
            }
        }
        for (expected_section, section) in WHOLE_PROOF_RELATION_ORDER_V1
            .into_iter()
            .zip(&self.relation_sections)
        {
            if section.section != expected_section
                || section.records.len() != expected_section.record_count()
            {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            for (ordinal, record) in section.records.iter().enumerate() {
                if record.ordinal != as_u8(ordinal)?
                    || record.relation_arity != expected_section.relation_arity()
                    || record.commitment.is_identity()
                    || record.blinding_response.is_zero()
                {
                    return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
                }
            }
        }
        Ok(())
    }
    fn validate_release_binding(
        &self,
        binding: &ZkAmsPhase23RnsLinkWholeProofBindingV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.profile_digest != binding.profile_digest
            || self.algorithm_manifest_digest != binding.algorithm_manifest_digest
            || self.context_digest != binding.context_digest
            || self.statement_digest != binding.statement_digest
            || self.commitment_root != binding.ordered_commitment_root
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut expected = binding.hyrax_commitments.iter();
        for record in self
            .family_sections
            .iter()
            .flat_map(|section| section.records.iter())
        {
            if expected.next() != Some(&record.commitment) {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
        }
        if expected.next().is_some() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}
/// Allocation-free structural preflight for the exact release-shape envelope.
///
/// This parses no variable-size count into owned storage. It rejects the full
/// wire unless every governed count, byte length, section tag, chunk offset,
/// padding count, point, scalar, and nonzero blinding response is canonical.
pub(super) fn preflight_zk_ams_phase23_rns_link_whole_proof_v1(
    bytes: &[u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    if bytes.len() > ZK_AMS_PHASE23_RNS_LINK_WHOLE_PROOF_MAX_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    if bytes.len() != WHOLE_PROOF_EXACT_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut decoder = WholeProofDecoderV1::new(bytes);
    decoder.expect_bytes(&WHOLE_PROOF_MAGIC_V1)?;
    decoder.expect_u8(WHOLE_PROOF_VERSION_V1)?;
    decoder.expect_u8(WHOLE_PROOF_FLAGS_V1)?;
    decoder.expect_u16(as_u16(WHOLE_PROOF_HEADER_BYTES_V1)?)?;
    decoder.expect_u32(as_u32(WHOLE_PROOF_EXACT_BYTES_V1)?)?;
    decoder.expect_u8(as_u8(WHOLE_PROOF_SECTION_COUNT_V1)?)?;
    decoder.expect_u8(as_u8(WHOLE_PROOF_FAMILY_COUNT_V1)?)?;
    decoder.expect_u8(as_u8(WHOLE_PROOF_RNS_LIMB_COUNT_V1)?)?;
    decoder.expect_u8(0)?;
    for _ in 0..5 {
        if decoder.array::<32>()? == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
    }
    decoder.point()?;
    for family in WHOLE_PROOF_FAMILY_ORDER_V1 {
        preflight_family_section(&mut decoder, family)?;
    }
    for section in WHOLE_PROOF_RELATION_ORDER_V1 {
        preflight_relation_section(&mut decoder, section)?;
    }
    decoder.finish()
}
fn preflight_family_section(
    decoder: &mut WholeProofDecoderV1<'_>,
    family: WholeProofFamilyV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    decoder.expect_u8(family as u8)?;
    decoder.expect_u8(WHOLE_PROOF_SECTION_FLAGS_V1)?;
    decoder.expect_u16(as_u16(family.chunk_count())?)?;
    let payload_bytes = family
        .chunk_count()
        .checked_mul(WHOLE_PROOF_FAMILY_RECORD_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    decoder.expect_u32(as_u32(payload_bytes)?)?;
    let payload_start = decoder.position();
    for ordinal in 0..family.chunk_count() {
        let (logical_offset, used_values, zero_padding_values) =
            expected_family_record_shape(family, ordinal)?;
        decoder.expect_u8(as_u8(ordinal)?)?;
        decoder.expect_u8(WHOLE_PROOF_RECORD_FLAGS_V1)?;
        decoder.expect_u32(logical_offset)?;
        decoder.expect_u32(used_values)?;
        decoder.expect_u32(zero_padding_values)?;
        decoder.point()?;
        decoder.scalar()?;
        decoder.nonzero_scalar()?;
    }
    if decoder.position().checked_sub(payload_start) != Some(payload_bytes) {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}
fn preflight_relation_section(
    decoder: &mut WholeProofDecoderV1<'_>,
    section: WholeProofRelationSectionV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    decoder.expect_u8(section as u8)?;
    decoder.expect_u8(WHOLE_PROOF_SECTION_FLAGS_V1)?;
    decoder.expect_u16(as_u16(section.record_count())?)?;
    let payload_bytes = section
        .record_count()
        .checked_mul(WHOLE_PROOF_RELATION_RECORD_BYTES_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    decoder.expect_u32(as_u32(payload_bytes)?)?;
    let payload_start = decoder.position();
    for ordinal in 0..section.record_count() {
        decoder.expect_u8(as_u8(ordinal)?)?;
        decoder.expect_u8(WHOLE_PROOF_RECORD_FLAGS_V1)?;
        decoder.expect_u16(section.relation_arity())?;
        decoder.point()?;
        decoder.scalar()?;
        decoder.nonzero_scalar()?;
    }
    if decoder.position().checked_sub(payload_start) != Some(payload_bytes) {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}
fn expected_family_record_shape(
    family: WholeProofFamilyV1,
    ordinal: usize,
) -> Result<(u32, u32, u32), ZkAmsMkheErrorV1> {
    if ordinal >= family.chunk_count() || family.chunk_count() > WHOLE_PROOF_MAX_FAMILY_CHUNKS_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let logical_offset = ordinal
        .checked_mul(WHOLE_PROOF_SLOT_COUNT_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let remaining = family
        .logical_values()
        .checked_sub(logical_offset)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let used_values = remaining.min(WHOLE_PROOF_SLOT_COUNT_V1);
    let zero_padding_values = WHOLE_PROOF_SLOT_COUNT_V1
        .checked_sub(used_values)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    Ok((
        as_u32(logical_offset)?,
        as_u32(used_values)?,
        as_u32(zero_padding_values)?,
    ))
}
struct WholeProofDecoderV1<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> WholeProofDecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    const fn position(&self) -> usize {
        self.offset
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], ZkAmsMkheErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], ZkAmsMkheErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
    }
    fn u8(&mut self) -> Result<u8, ZkAmsMkheErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
    }
    fn u16(&mut self) -> Result<u16, ZkAmsMkheErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, ZkAmsMkheErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }
    fn point(&mut self) -> Result<Point, ZkAmsMkheErrorV1> {
        Point::from_non_identity_wire_bytes_exact(self.take(33)?)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
    }
    fn scalar(&mut self) -> Result<Scalar, ZkAmsMkheErrorV1> {
        Scalar::from_be_bytes_exact(self.array()?)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
    }
    fn nonzero_scalar(&mut self) -> Result<Scalar, ZkAmsMkheErrorV1> {
        let scalar = self.scalar()?;
        if scalar.is_zero() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(scalar)
    }
    fn expect_bytes(&mut self, expected: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
        if self.take(expected.len())? != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }
    fn expect_u8(&mut self, expected: u8) -> Result<(), ZkAmsMkheErrorV1> {
        if self.u8()? != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }
    fn expect_u16(&mut self, expected: u16) -> Result<(), ZkAmsMkheErrorV1> {
        if self.u16()? != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }
    fn expect_u32(&mut self, expected: u32) -> Result<(), ZkAmsMkheErrorV1> {
        if self.u32()? != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }
    fn finish(self) -> Result<(), ZkAmsMkheErrorV1> {
        if self.offset != self.bytes.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }
}
fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_be_bytes());
}
fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}
fn push_point(bytes: &mut Vec<u8>, point: Point) -> Result<(), ZkAmsMkheErrorV1> {
    bytes.extend_from_slice(
        &point
            .to_non_identity_wire_bytes()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    );
    Ok(())
}
fn as_u8(value: usize) -> Result<u8, ZkAmsMkheErrorV1> {
    u8::try_from(value).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
fn as_u16(value: usize) -> Result<u16, ZkAmsMkheErrorV1> {
    u16::try_from(value).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
fn as_u32(value: usize) -> Result<u32, ZkAmsMkheErrorV1> {
    u32::try_from(value).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
#[cfg(test)]
std::thread_local! {
    static WHOLE_PROOF_DECODE_ALLOCATIONS_V1: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    static WHOLE_PROOF_UNIFORM_BUFFER_ZEROIZED_DROPS_V1: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}
fn reserve_decode_records<T>(values: &mut Vec<T>, length: usize) -> Result<(), ZkAmsMkheErrorV1> {
    if length > WHOLE_PROOF_MAX_SECTION_RECORDS_V1 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    values
        .try_reserve_exact(length)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    #[cfg(test)]
    WHOLE_PROOF_DECODE_ALLOCATIONS_V1.with(|count| count.set(count.get().saturating_add(1)));
    Ok(())
}
struct ZeroizingWholeProofUniformBytesV1([u8; 64]);
impl ZeroizingWholeProofUniformBytesV1 {
    const fn new() -> Self {
        Self([0; 64])
    }
    fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
    fn bytes(&self) -> &[u8; 64] {
        &self.0
    }
}
impl Drop for ZeroizingWholeProofUniformBytesV1 {
    fn drop(&mut self) {
        self.0.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.0);
        #[cfg(test)]
        if self.0 == [0; 64] {
            WHOLE_PROOF_UNIFORM_BUFFER_ZEROIZED_DROPS_V1
                .with(|count| count.set(count.get().saturating_add(1)));
        }
    }
}
/// Move-only owner for the exact internally sampled whole-proof blindings.
/// These scalars never appear directly in the public envelope.
struct ZkAmsPhase23RnsLinkWholeProofBlindingsV1(ZeroizingT256ScalarVecV1);
impl fmt::Debug for ZkAmsPhase23RnsLinkWholeProofBlindingsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ZkAmsPhase23RnsLinkWholeProofBlindingsV1([REDACTED])")
    }
}
impl ZkAmsPhase23RnsLinkWholeProofBlindingsV1 {
    fn validate(&self) -> Result<(), ZkAmsMkheErrorV1> {
        if self.0.len() != WHOLE_PROOF_INTERNAL_BLINDING_COUNT_V1
            || self.0.as_slice().iter().any(|scalar| scalar.is_zero())
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
    fn as_slice(&self) -> &[Scalar] {
        self.0.as_slice()
    }
}
fn sample_internal_whole_proof_blindings_v1<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
) -> Result<ZkAmsPhase23RnsLinkWholeProofBlindingsV1, ZkAmsMkheErrorV1> {
    let mut blindings =
        ZeroizingT256ScalarVecV1::with_capacity(WHOLE_PROOF_INTERNAL_BLINDING_COUNT_V1);
    for _ in 0..WHOLE_PROOF_INTERNAL_BLINDING_COUNT_V1 {
        let mut uniform = ZeroizingWholeProofUniformBytesV1::new();
        let mut accepted = false;
        for _ in 0..WHOLE_PROOF_BLINDING_REJECTION_ATTEMPTS_V1 {
            random
                .fill_bytes(uniform.bytes_mut())
                .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
            let candidate =
                ZeroizingT256ScalarCopyV1::new(Scalar::from_uniform_le_bytes_ref(uniform.bytes()));
            if !candidate.as_ref().is_zero() {
                blindings.push(candidate.get());
                accepted = true;
                break;
            }
        }
        if !accepted {
            return Err(ZkAmsMkheErrorV1::RandomUnavailable);
        }
    }
    let blindings = ZkAmsPhase23RnsLinkWholeProofBlindingsV1(blindings);
    blindings.validate()?;
    Ok(blindings)
}
fn with_internal_whole_proof_blindings_v1<R, T, F>(
    random: &mut R,
    use_blindings: F,
) -> Result<T, ZkAmsMkheErrorV1>
where
    R: MaskedRelaxedRandomSourceV1,
    F: FnOnce(&[Scalar]) -> Result<T, ZkAmsMkheErrorV1>,
{
    let blindings = sample_internal_whole_proof_blindings_v1(random)?;
    blindings.validate()?;
    use_blindings(blindings.as_slice())
}
#[cfg(test)]
mod tests {
    use super::super::phase23_encrypted::zk_ams_phase23_release_map_set_digest_v1;
    use super::super::phase23_rns_link::{
        ZkAmsPhase23RnsLinkCommitmentDigestsV1, ZkAmsPhase23RnsLinkFamilyV1,
    };
    use super::*;
    use crate::vega::{
        MaskedRelaxedRandomErrorV1, VEGA_T256_BASE_MODULUS_BE_V1, VEGA_T256_SCALAR_MODULUS_BE_V1,
        bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1, derive_t256_generators_v1,
        sponge::keccak256,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};
    fn next_scalar(counter: &mut u64) -> Scalar {
        let value = *counter;
        *counter = counter.checked_add(1).expect("fixture scalar counter fits");
        Scalar::from_u64(value)
    }
    fn whole_proof_fixture() -> ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1 {
        let mut points = derive_t256_generators_v1(
            b"iroha.zk-ams.v1.phase23.rns-link.whole-proof-wire-test",
            WHOLE_PROOF_INTERNAL_BLINDING_COUNT_V1,
        )
        .expect("test point basis")
        .into_iter();
        let proof_blinding_commitment = points.next().expect("proof blinding point");
        let mut scalar_counter = 1_u64;
        let mut family_sections = Vec::new();
        family_sections
            .try_reserve_exact(WHOLE_PROOF_FAMILY_COUNT_V1)
            .expect("bounded family sections");
        for family in WHOLE_PROOF_FAMILY_ORDER_V1 {
            let mut records = Vec::new();
            records
                .try_reserve_exact(family.chunk_count())
                .expect("bounded family records");
            for ordinal in 0..family.chunk_count() {
                let (logical_offset, used_values, zero_padding_values) =
                    expected_family_record_shape(family, ordinal).expect("family record shape");
                records.push(WholeProofFamilyRecordV1 {
                    ordinal: as_u8(ordinal).expect("ordinal fits"),
                    logical_offset,
                    used_values,
                    zero_padding_values,
                    commitment: points.next().expect("family point"),
                    evaluation: next_scalar(&mut scalar_counter),
                    blinding_response: next_scalar(&mut scalar_counter),
                });
            }
            family_sections.push(WholeProofFamilySectionV1 { family, records });
        }
        let mut relation_sections = Vec::new();
        relation_sections
            .try_reserve_exact(WHOLE_PROOF_RELATION_ORDER_V1.len())
            .expect("bounded relation sections");
        for section in WHOLE_PROOF_RELATION_ORDER_V1 {
            let mut records = Vec::new();
            records
                .try_reserve_exact(section.record_count())
                .expect("bounded relation records");
            for ordinal in 0..section.record_count() {
                records.push(WholeProofRelationRecordV1 {
                    ordinal: as_u8(ordinal).expect("ordinal fits"),
                    relation_arity: section.relation_arity(),
                    commitment: points.next().expect("relation point"),
                    evaluation: next_scalar(&mut scalar_counter),
                    blinding_response: next_scalar(&mut scalar_counter),
                });
            }
            relation_sections.push(WholeProofRelationRecordsV1 { section, records });
        }
        assert!(points.next().is_none());
        ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1 {
            profile_digest: [0x11; 32],
            algorithm_manifest_digest: [0x22; 32],
            context_digest: [0x33; 32],
            statement_digest: [0x44; 32],
            commitment_root: [0x55; 32],
            proof_blinding_commitment,
            family_sections: family_sections.try_into().expect("six family sections"),
            relation_sections: relation_sections
                .try_into()
                .expect("four relation sections"),
        }
    }
    fn binding_digest(label: &[u8], family: u8, chunk: u8, field: u8) -> [u8; 32] {
        let mut frame = Vec::with_capacity(label.len() + 3);
        frame.extend_from_slice(label);
        frame.extend_from_slice(&[family, chunk, field]);
        keccak256(&frame)
    }
    fn release_context(label: &[u8]) -> ZkAmsPhase23RnsLinkContextV1 {
        let axis = |field: u8| binding_digest(label, 0, 0, field);
        ZkAmsPhase23RnsLinkContextV1::new(
            axis(1),
            axis(2),
            axis(3),
            axis(4),
            axis(5),
            axis(6),
            zk_ams_phase23_release_map_set_digest_v1().expect("release map-set digest"),
        )
        .expect("release context")
    }
    const fn relation_family(family: WholeProofFamilyV1) -> ZkAmsPhase23RnsLinkFamilyV1 {
        match family {
            WholeProofFamilyV1::X => ZkAmsPhase23RnsLinkFamilyV1::X,
            WholeProofFamilyV1::U => ZkAmsPhase23RnsLinkFamilyV1::U,
            WholeProofFamilyV1::E => ZkAmsPhase23RnsLinkFamilyV1::E,
            WholeProofFamilyV1::RE => ZkAmsPhase23RnsLinkFamilyV1::RE,
            WholeProofFamilyV1::W => ZkAmsPhase23RnsLinkFamilyV1::W,
            WholeProofFamilyV1::RW => ZkAmsPhase23RnsLinkFamilyV1::RW,
        }
    }
    fn relation_commitments(
        proof: &ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1,
        label: &[u8],
    ) -> Vec<ZkAmsPhase23RnsLinkChunkCommitmentV1> {
        let mut commitments = Vec::new();
        commitments
            .try_reserve_exact(WHOLE_PROOF_FAMILY_RECORD_COUNT_V1)
            .expect("bounded commitment fixture");
        for section in &proof.family_sections {
            let family = relation_family(section.family);
            let chunk_count = section.records.len();
            let present_bitmap = if chunk_count == u16::BITS as usize {
                u16::MAX
            } else {
                (1_u16 << chunk_count) - 1
            };
            for record in &section.records {
                let chunk = record.ordinal;
                let digest = |field: u8| binding_digest(label, family as u8, chunk, field);
                let digests = ZkAmsPhase23RnsLinkCommitmentDigestsV1::new(
                    binding_digest(label, family as u8, 0, 1),
                    digest(2),
                    digest(3),
                    digest(4),
                    digest(5),
                    digest(6),
                    digest(7),
                    digest(8),
                )
                .expect("nonzero commitment digests");
                commitments.push(
                    ZkAmsPhase23RnsLinkChunkCommitmentV1::new(
                        family,
                        u16::from(record.ordinal),
                        u16::try_from(chunk_count).expect("chunk count fits"),
                        u32::try_from(section.family.logical_values())
                            .expect("logical value count fits"),
                        record.used_values,
                        !present_bitmap,
                        record
                            .commitment
                            .to_non_identity_wire_bytes()
                            .expect("fixture commitment is non-identity"),
                        digests,
                    )
                    .expect("native chunk commitment"),
                );
            }
        }
        commitments
    }
    fn release_bound_fixture() -> (
        ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1,
        ZkAmsPhase23RnsLinkContextV1,
        Vec<ZkAmsPhase23RnsLinkChunkCommitmentV1>,
    ) {
        let mut proof = whole_proof_fixture();
        let context = release_context(b"release-bound-wire-fixture");
        let commitments = relation_commitments(&proof, b"release-bound-commitments");
        let binding = ZkAmsPhase23RnsLinkWholeProofBindingV1::derive(&context, &commitments)
            .expect("verifier-owned release binding");
        proof.profile_digest = binding.profile_digest;
        proof.algorithm_manifest_digest = binding.algorithm_manifest_digest;
        proof.context_digest = binding.context_digest;
        proof.statement_digest = binding.statement_digest;
        proof.commitment_root = binding.ordered_commitment_root;
        (proof, context, commitments)
    }
    fn section_ranges_v1() -> [(usize, usize); WHOLE_PROOF_SECTION_COUNT_V1] {
        let mut offset = WHOLE_PROOF_HEADER_BYTES_V1;
        let mut ranges = Vec::new();
        ranges
            .try_reserve_exact(WHOLE_PROOF_SECTION_COUNT_V1)
            .expect("bounded ranges");
        for family in WHOLE_PROOF_FAMILY_ORDER_V1 {
            let length = WHOLE_PROOF_SECTION_HEADER_BYTES_V1
                + family.chunk_count() * WHOLE_PROOF_FAMILY_RECORD_BYTES_V1;
            ranges.push((offset, offset + length));
            offset += length;
        }
        for section in WHOLE_PROOF_RELATION_ORDER_V1 {
            let length = WHOLE_PROOF_SECTION_HEADER_BYTES_V1
                + section.record_count() * WHOLE_PROOF_RELATION_RECORD_BYTES_V1;
            ranges.push((offset, offset + length));
            offset += length;
        }
        assert_eq!(offset, WHOLE_PROOF_EXACT_BYTES_V1);
        ranges.try_into().expect("ten section ranges")
    }
    fn decode_allocation_count_v1() -> usize {
        WHOLE_PROOF_DECODE_ALLOCATIONS_V1.with(core::cell::Cell::get)
    }
    fn uniform_zeroized_drop_count_v1() -> usize {
        WHOLE_PROOF_UNIFORM_BUFFER_ZEROIZED_DROPS_V1.with(core::cell::Cell::get)
    }
    fn assert_preflight_rejects_without_decode_allocation(bytes: &[u8]) {
        let before = decode_allocation_count_v1();
        assert!(preflight_zk_ams_phase23_rns_link_whole_proof_v1(bytes).is_err());
        assert_eq!(decode_allocation_count_v1(), before);
    }
    #[test]
    fn release_shape_roundtrip_is_exact_bounded_and_explicitly_unverified() {
        let proof = whole_proof_fixture();
        let wire = proof
            .encode_canonical()
            .expect("canonical whole-proof wire");
        assert_eq!(wire.len(), 20_418, "unverified structural-envelope bytes");
        assert_eq!(wire.len(), WHOLE_PROOF_EXACT_BYTES_V1);
        assert_eq!(wire.len(), ZK_AMS_PHASE23_RNS_LINK_WHOLE_PROOF_MAX_BYTES_V1);
        assert_eq!(WHOLE_PROOF_FAMILY_ORDER_V1.len(), 6);
        assert_eq!(WHOLE_PROOF_RNS_LIMB_COUNT_V1, 38);
        assert_eq!(WHOLE_PROOF_FAMILY_RECORD_COUNT_V1, 43);
        assert_eq!(WHOLE_PROOF_RELATION_RECORD_COUNT_V1, 152);
        let before_preflight = decode_allocation_count_v1();
        preflight_zk_ams_phase23_rns_link_whole_proof_v1(&wire).expect("preflight");
        assert_eq!(decode_allocation_count_v1(), before_preflight);
        let decoded = ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1::decode_exact(&wire)
            .expect("strict decode");
        assert!(decode_allocation_count_v1() > before_preflight);
        assert!(decoded == proof);
        assert_eq!(decoded.encode_canonical().expect("re-encode"), wire);
    }
    #[test]
    fn bound_decode_recomputes_release_context_challenges_root_and_hyrax_commitments() {
        let (proof, context, commitments) = release_bound_fixture();
        let wire = proof.encode_canonical().expect("release-bound wire");
        let decoded =
            ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1::decode_exact_bound_unverified(
                &wire,
                &context,
                &commitments,
            )
            .expect("canonical transport is bound to native relation inputs");
        assert_eq!(decoded, proof);
        // A structurally canonical digest shell is not sufficient.
        let shell = whole_proof_fixture()
            .encode_canonical()
            .expect("structural digest shell");
        assert!(ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1::decode_exact(&shell).is_ok());
        assert_eq!(
            ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1::decode_exact_bound_unverified(
                &shell,
                &context,
                &commitments,
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let mut digest_substitutions = Vec::new();
        for axis in 0..5 {
            let mut changed = proof.clone();
            let digest = match axis {
                0 => &mut changed.profile_digest,
                1 => &mut changed.algorithm_manifest_digest,
                2 => &mut changed.context_digest,
                3 => &mut changed.statement_digest,
                4 => &mut changed.commitment_root,
                _ => unreachable!("five bound digest axes"),
            };
            digest[0] ^= 1;
            digest_substitutions.push(changed);
        }
        for changed in digest_substitutions {
            let changed = changed
                .encode_canonical()
                .expect("structurally canonical digest substitution");
            assert!(
                ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1::decode_exact(&changed).is_ok()
            );
            assert_eq!(
                ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1::decode_exact_bound_unverified(
                    &changed,
                    &context,
                    &commitments,
                ),
                Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
            );
        }
        let wrong_context = release_context(b"different-release-context");
        assert_eq!(
            ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1::decode_exact_bound_unverified(
                &wire,
                &wrong_context,
                &commitments,
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let different_commitments = relation_commitments(&proof, b"different-commitments");
        assert_eq!(
            ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1::decode_exact_bound_unverified(
                &wire,
                &context,
                &different_commitments,
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let mut reordered_commitments = commitments.clone();
        reordered_commitments.swap(0, 1);
        assert_eq!(
            ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1::decode_exact_bound_unverified(
                &wire,
                &context,
                &reordered_commitments,
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        // Even with every digest header intact, substituting one canonical
        // non-identity Hyrax point is rejected against the native commitment.
        let mut changed_hyrax = proof;
        changed_hyrax.family_sections[0].records[0].commitment =
            changed_hyrax.relation_sections[0].records[0].commitment;
        let changed_hyrax = changed_hyrax
            .encode_canonical()
            .expect("structurally canonical Hyrax substitution");
        assert_eq!(
            ZkAmsPhase23RnsLinkUnverifiedWholeProofEnvelopeV1::decode_exact_bound_unverified(
                &changed_hyrax,
                &context,
                &commitments,
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
    }
    #[test]
    fn every_truncation_and_every_single_byte_suffix_fail_before_allocation() {
        let wire = whole_proof_fixture().encode_canonical().expect("wire");
        for end in 0..wire.len() {
            assert_preflight_rejects_without_decode_allocation(&wire[..end]);
        }
        for suffix in u8::MIN..=u8::MAX {
            let mut extended = wire.clone();
            extended.push(suffix);
            assert_preflight_rejects_without_decode_allocation(&extended);
        }
    }
    #[test]
    fn duplicate_reordered_unknown_and_wrong_count_sections_fail_closed() {
        let wire = whole_proof_fixture().encode_canonical().expect("wire");
        let ranges = section_ranges_v1();
        let mut duplicate = wire.clone();
        duplicate[ranges[1].0] = duplicate[ranges[0].0];
        assert_preflight_rejects_without_decode_allocation(&duplicate);
        let mut unknown = wire.clone();
        unknown[ranges[0].0] = 0xff;
        assert_preflight_rejects_without_decode_allocation(&unknown);
        let mut reordered = Vec::new();
        reordered
            .try_reserve_exact(wire.len())
            .expect("bounded reordered wire");
        reordered.extend_from_slice(&wire[..WHOLE_PROOF_HEADER_BYTES_V1]);
        reordered.extend_from_slice(&wire[ranges[1].0..ranges[1].1]);
        reordered.extend_from_slice(&wire[ranges[0].0..ranges[0].1]);
        for &(start, end) in &ranges[2..] {
            reordered.extend_from_slice(&wire[start..end]);
        }
        assert_eq!(reordered.len(), wire.len());
        assert_preflight_rejects_without_decode_allocation(&reordered);
        let mut wrong_count = wire.clone();
        wrong_count[ranges[0].0 + SECTION_RECORD_COUNT_OFFSET_V1
            ..ranges[0].0 + SECTION_RECORD_COUNT_OFFSET_V1 + 2]
            .copy_from_slice(&u16::MAX.to_be_bytes());
        assert_preflight_rejects_without_decode_allocation(&wrong_count);
        let mut missing_section = wire;
        missing_section[HEADER_SECTION_COUNT_OFFSET_V1] = 9;
        assert_preflight_rejects_without_decode_allocation(&missing_section);
    }
    #[test]
    fn noncanonical_numbers_lengths_flags_and_overflow_claims_fail_before_allocation() {
        let wire = whole_proof_fixture().encode_canonical().expect("wire");
        let ranges = section_ranges_v1();
        let first_family_record = ranges[0].0 + WHOLE_PROOF_SECTION_HEADER_BYTES_V1;
        let first_relation_record = ranges[6].0 + WHOLE_PROOF_SECTION_HEADER_BYTES_V1;
        for (offset, value) in [
            (HEADER_FLAGS_OFFSET_V1, 1),
            (HEADER_FAMILY_COUNT_OFFSET_V1, 5),
            (HEADER_LIMB_COUNT_OFFSET_V1, 37),
            (HEADER_RESERVED_OFFSET_V1, 1),
            (ranges[0].0 + SECTION_FLAGS_OFFSET_V1, 1),
            (first_family_record + FAMILY_RECORD_FLAGS_OFFSET_V1, 1),
            (first_relation_record + RELATION_RECORD_FLAGS_OFFSET_V1, 1),
        ] {
            let mut malformed = wire.clone();
            malformed[offset] = value;
            assert_preflight_rejects_without_decode_allocation(&malformed);
        }
        let mut header_length = wire.clone();
        header_length[HEADER_LENGTH_OFFSET_V1..HEADER_LENGTH_OFFSET_V1 + 2]
            .copy_from_slice(&0_u16.to_be_bytes());
        assert_preflight_rejects_without_decode_allocation(&header_length);
        let mut total_length = wire.clone();
        total_length[HEADER_TOTAL_LENGTH_OFFSET_V1..HEADER_TOTAL_LENGTH_OFFSET_V1 + 4]
            .copy_from_slice(&u32::MAX.to_be_bytes());
        assert_preflight_rejects_without_decode_allocation(&total_length);
        let mut payload_length = wire.clone();
        payload_length[ranges[0].0 + SECTION_PAYLOAD_LENGTH_OFFSET_V1
            ..ranges[0].0 + SECTION_PAYLOAD_LENGTH_OFFSET_V1 + 4]
            .copy_from_slice(&u32::MAX.to_be_bytes());
        assert_preflight_rejects_without_decode_allocation(&payload_length);
        let mut logical_offset = wire.clone();
        logical_offset[first_family_record + FAMILY_RECORD_LOGICAL_OFFSET_OFFSET_V1
            ..first_family_record + FAMILY_RECORD_LOGICAL_OFFSET_OFFSET_V1 + 4]
            .copy_from_slice(&u32::MAX.to_be_bytes());
        assert_preflight_rejects_without_decode_allocation(&logical_offset);
        let mut used_values = wire.clone();
        used_values[first_family_record + FAMILY_RECORD_USED_VALUES_OFFSET_V1
            ..first_family_record + FAMILY_RECORD_USED_VALUES_OFFSET_V1 + 4]
            .copy_from_slice(&0_u32.to_be_bytes());
        assert_preflight_rejects_without_decode_allocation(&used_values);
        let mut padding = wire.clone();
        padding[first_family_record + FAMILY_RECORD_ZERO_PADDING_OFFSET_V1
            ..first_family_record + FAMILY_RECORD_ZERO_PADDING_OFFSET_V1 + 4]
            .copy_from_slice(&1_u32.to_be_bytes());
        assert_preflight_rejects_without_decode_allocation(&padding);
        let mut arity = wire.clone();
        arity[first_relation_record + RELATION_RECORD_ARITY_OFFSET_V1
            ..first_relation_record + RELATION_RECORD_ARITY_OFFSET_V1 + 2]
            .copy_from_slice(&u16::MAX.to_be_bytes());
        assert_preflight_rejects_without_decode_allocation(&arity);
    }
    #[test]
    fn noncanonical_points_scalars_identity_and_zero_blindings_fail_closed() {
        let wire = whole_proof_fixture().encode_canonical().expect("wire");
        let ranges = section_ranges_v1();
        let first_family_record = ranges[0].0 + WHOLE_PROOF_SECTION_HEADER_BYTES_V1;
        let first_relation_record = ranges[6].0 + WHOLE_PROOF_SECTION_HEADER_BYTES_V1;
        let mut identity = wire.clone();
        identity[HEADER_PROOF_BLINDING_COMMITMENT_OFFSET_V1
            ..HEADER_PROOF_BLINDING_COMMITMENT_OFFSET_V1 + 33]
            .fill(0);
        identity[HEADER_PROOF_BLINDING_COMMITMENT_OFFSET_V1] = 0x40;
        assert_preflight_rejects_without_decode_allocation(&identity);
        let family_point = first_family_record + FAMILY_RECORD_COMMITMENT_OFFSET_V1;
        let mut undefined_point_flag = wire.clone();
        undefined_point_flag[family_point] = 0x20;
        assert_preflight_rejects_without_decode_allocation(&undefined_point_flag);
        let mut noncanonical_x = wire.clone();
        noncanonical_x[family_point] = 0;
        noncanonical_x[family_point + 1..family_point + 33]
            .copy_from_slice(&VEGA_T256_BASE_MODULUS_BE_V1);
        assert_preflight_rejects_without_decode_allocation(&noncanonical_x);
        let mut noncanonical_scalar = wire.clone();
        let evaluation = first_family_record + FAMILY_RECORD_EVALUATION_OFFSET_V1;
        noncanonical_scalar[evaluation..evaluation + 32]
            .copy_from_slice(&VEGA_T256_SCALAR_MODULUS_BE_V1);
        assert_preflight_rejects_without_decode_allocation(&noncanonical_scalar);
        let mut zero_blinding = wire.clone();
        let blinding = first_family_record + FAMILY_RECORD_BLINDING_RESPONSE_OFFSET_V1;
        zero_blinding[blinding..blinding + 32].fill(0);
        assert_preflight_rejects_without_decode_allocation(&zero_blinding);
        let mut relation_identity = wire;
        let relation_point = first_relation_record + RELATION_RECORD_COMMITMENT_OFFSET_V1;
        relation_identity[relation_point..relation_point + 33].fill(0);
        relation_identity[relation_point] = 0x40;
        assert_preflight_rejects_without_decode_allocation(&relation_identity);
    }
    struct CounterRandom(u64);
    impl MaskedRelaxedRandomSourceV1 for CounterRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            for (index, byte) in destination.iter_mut().enumerate() {
                let index =
                    u64::try_from(index).map_err(|_| MaskedRelaxedRandomErrorV1::Unavailable)?;
                *byte = self.0.wrapping_add(index).to_le_bytes()[0];
            }
            self.0 = self.0.wrapping_add(1);
            Ok(())
        }
    }
    struct FailingRandom;
    impl MaskedRelaxedRandomSourceV1 for FailingRandom {
        fn fill_bytes(
            &mut self,
            _destination: &mut [u8],
        ) -> Result<(), MaskedRelaxedRandomErrorV1> {
            Err(MaskedRelaxedRandomErrorV1::Unavailable)
        }
    }
    struct ZeroRandom;
    impl MaskedRelaxedRandomSourceV1 for ZeroRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            destination.fill(0);
            Ok(())
        }
    }
    struct PanickingRandom;
    impl MaskedRelaxedRandomSourceV1 for PanickingRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            destination.fill(0xa5);
            panic!("injected entropy-source panic")
        }
    }
    #[test]
    fn internal_blindings_are_nonzero_redacted_and_zeroized_on_every_exit() {
        let scalar_before = zeroizing_t256_scalar_vec_drop_count_v1();
        let uniform_before = uniform_zeroized_drop_count_v1();
        let mut success_random = CounterRandom(1);
        with_internal_whole_proof_blindings_v1(&mut success_random, |blindings| {
            assert_eq!(blindings.len(), WHOLE_PROOF_INTERNAL_BLINDING_COUNT_V1);
            assert!(blindings.iter().all(|scalar| !scalar.is_zero()));
            Ok(())
        })
        .expect("success path");
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), scalar_before + 1);
        assert_eq!(
            uniform_zeroized_drop_count_v1(),
            uniform_before + WHOLE_PROOF_INTERNAL_BLINDING_COUNT_V1
        );
        let mut debug_random = CounterRandom(2);
        let debug_owner = sample_internal_whole_proof_blindings_v1(&mut debug_random)
            .expect("debug owner sample");
        let debug = format!("{debug_owner:?}");
        assert!(debug.contains("[REDACTED]"));
        drop(debug_owner);
        let before_error = zeroizing_t256_scalar_vec_drop_count_v1();
        let mut error_random = CounterRandom(3);
        assert_eq!(
            with_internal_whole_proof_blindings_v1(&mut error_random, |_blindings| {
                Err::<(), _>(ZkAmsMkheErrorV1::InvalidPhase23Fold)
            }),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before_error + 1);
        let before_unwind = zeroizing_t256_scalar_vec_drop_count_v1();
        let mut unwind_random = CounterRandom(4);
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ = with_internal_whole_proof_blindings_v1(
                &mut unwind_random,
                |_blindings| -> Result<(), ZkAmsMkheErrorV1> { panic!("injected callback panic") },
            );
        }));
        assert!(unwind.is_err());
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before_unwind + 1);
        let failing_scalar_before = zeroizing_t256_scalar_vec_drop_count_v1();
        let failing_uniform_before = uniform_zeroized_drop_count_v1();
        assert!(matches!(
            sample_internal_whole_proof_blindings_v1(&mut FailingRandom),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        ));
        assert_eq!(
            zeroizing_t256_scalar_vec_drop_count_v1(),
            failing_scalar_before + 1
        );
        assert_eq!(uniform_zeroized_drop_count_v1(), failing_uniform_before + 1);
        let zero_scalar_before = zeroizing_t256_scalar_vec_drop_count_v1();
        assert!(matches!(
            sample_internal_whole_proof_blindings_v1(&mut ZeroRandom),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        ));
        assert_eq!(
            zeroizing_t256_scalar_vec_drop_count_v1(),
            zero_scalar_before + 1
        );
        let panic_scalar_before = zeroizing_t256_scalar_vec_drop_count_v1();
        let panic_uniform_before = uniform_zeroized_drop_count_v1();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = sample_internal_whole_proof_blindings_v1(&mut PanickingRandom);
        }));
        assert!(panic.is_err());
        assert_eq!(
            zeroizing_t256_scalar_vec_drop_count_v1(),
            panic_scalar_before + 1
        );
        assert_eq!(uniform_zeroized_drop_count_v1(), panic_uniform_before + 1);
        let mut raw = vec![Scalar::one(); WHOLE_PROOF_INTERNAL_BLINDING_COUNT_V1];
        raw[0] = Scalar::zero();
        let zero_owner =
            ZkAmsPhase23RnsLinkWholeProofBlindingsV1(ZeroizingT256ScalarVecV1::new(raw));
        assert_eq!(
            zero_owner.validate(),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
    }
}
