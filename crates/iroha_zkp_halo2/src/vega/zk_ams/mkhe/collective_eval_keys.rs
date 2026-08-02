//! Proof-carrying generation and roster-independent evaluation of compact collective keys.
//!
//! Each online key contains exactly two polynomials per balanced gadget digit.
//! Relinearization digits encrypt `g^d S^2`; Galois digits encrypt
//! `g^d sigma_k(S)`, where `S` is the exact eight-party sum secret.  Generation
//! retains the full authenticated source topology and compacts every digit with
//! the native full-roster CKS protocol.  Online evaluation therefore performs
//! exactly two ring multiplications per digit, independent of roster size.

use super::{
    BgvProfile, MAX_RANDOM_REJECTION_ATTEMPTS_V1, MKHE_VERSION_V1, MaskedRelaxedRandomSourceV1,
    PartySet, RnsPolynomial, SecretPolynomial, ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active::{
        ZkAmsMkheActiveCollectivePublicKeyStatementV1, ZkAmsMkheActiveGaloisSourceStatementV1,
        ZkAmsMkheActiveGaloisSourceWitnessV1, ZkAmsMkheActivePartySecretV1,
        ZkAmsMkheActiveRkgProofV1, ZkAmsMkheActiveRkgRoundOneStatementV1,
        ZkAmsMkheActiveRkgRoundOneWitnessV1, ZkAmsMkheActiveRkgRoundTwoStatementV1,
        ZkAmsMkheActiveRkgRoundTwoWitnessV1, ZkAmsMkheGovernedActiveRosterV1,
        prove_zk_ams_mkhe_active_galois_source_v1, prove_zk_ams_mkhe_active_rkg_round_one_v1,
        prove_zk_ams_mkhe_active_rkg_round_two_v1, verify_zk_ams_mkhe_active_galois_source_v1,
        verify_zk_ams_mkhe_active_rkg_round_one_v1, verify_zk_ams_mkhe_active_rkg_round_two_v1,
    },
    checked_coefficient_work, checked_ring_multiplication_work,
    cks::{
        ZkAmsMkheAuthenticatedCksContributionV1, ZkAmsMkheCksSourceCiphertextV1,
        ZkAmsMkheCksStatementV1, combine_zk_ams_mkhe_cks_v1, prove_zk_ams_mkhe_cks_contribution_v1,
    },
    collective::{
        ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheCollectiveLevelOneV1,
        ZkAmsMkheCollectivePartyStateV1, ZkAmsMkheCollectivePublicKeyShareV1,
        ZkAmsMkheCollectivePublicKeyV1, aggregate_zk_ams_mkhe_collective_public_key_v1,
        validate_compact_for_key,
    },
    collective_keys::{
        ZkAmsMkheCollectiveEvaluatedKeyEntryV1, ZkAmsMkheCollectiveEvaluatedKeyManifestV1,
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    },
    derive_rkg_common_a, derive_uniform_rns_from_context, gadget_decompose,
    manifest::{ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1},
    packing::{
        ZK_AMS_T256_GALOIS_KEY_COUNT_V1, validate_zk_ams_t256_galois_key_schedule_v1,
        zk_ams_t256_galois_key_schedule_v1,
    },
    wire::{
        ZkAmsMkheGovernedRosterWireV1, ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheSeededRkgKeyWireV1,
        ZkAmsMkheWireBindingV1,
    },
};
use crate::vega::sponge::{Keccak256, keccak256};

const EVALUATED_KEY_TARGET_A_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-target-a";
const EVALUATED_KEY_EVIDENCE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-evidence";
const EVALUATED_KEY_LINEAGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-lineage";
const EVALUATED_KEY_RUNTIME_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-runtime";
const RELINEARIZATION_SOURCE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-relinearization-source";
const GALOIS_SOURCE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-galois-source";
const SOURCE_EVIDENCE_RECORD_TAG_V1: [u8; 4] = *b"ZASE";
const CKS_EVIDENCE_RECORD_TAG_V1: [u8; 4] = *b"ZACE";
const EVIDENCE_RECORD_DIGEST_BYTES_V1: usize = 32;
const SOURCE_EVIDENCE_COMMON_BODY_BYTES_V1: usize =
    4 + 1 + 1 + 8 + 1 + 4 + 1 + 32 + 32 + 32 + 8 + 32 + 32;
const CKS_EVIDENCE_COMMON_BODY_BYTES_V1: usize = 4 + 1 + 8 + 1 + 1 + 32;

/// Largest callback chunk used by the canonical evidence stream.
///
/// Chunk boundaries are transport metadata, not part of the canonical record,
/// but are deterministic and gap-free so a sink can reject omission, reorder,
/// duplication, or cross-record splicing before committing durable bytes.
pub const ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1: usize = 64 * 1024;

/// One of the two independently hashed evidence sets backing an evaluated key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheCollectiveEvidenceSetKindV1 {
    /// Pairwise RKG or automorphism-linked source proofs.
    Source = 1,
    /// Full-roster CKS compaction proofs.
    Cks = 2,
}

/// Exact canonical record family inside an evaluated-key evidence set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheCollectiveEvidenceRecordKindV1 {
    /// First authenticated RKG round for one pair/digit/party coordinate.
    RkgRoundOne = 1,
    /// Second authenticated RKG round for one pair/digit/party coordinate.
    RkgRoundTwo = 2,
    /// One automorphism-linked source encryption.
    GaloisSource = 3,
    /// One complete eight-party CKS digit.
    CksDigit = 4,
}

impl ZkAmsMkheCollectiveEvidenceRecordKindV1 {
    fn decode(value: u8) -> Result<Self, ZkAmsMkheErrorV1> {
        match value {
            1 => Ok(Self::RkgRoundOne),
            2 => Ok(Self::RkgRoundTwo),
            3 => Ok(Self::GaloisSource),
            4 => Ok(Self::CksDigit),
            _ => Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }
}

/// Context opened once for one independently hashed canonical evidence set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
    kind: ZkAmsMkheCollectiveEvidenceSetKindV1,
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    galois_exponent: u32,
    collective_key_digest: [u8; 32],
}

impl ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
    /// Evidence family.
    #[must_use]
    pub const fn kind(self) -> ZkAmsMkheCollectiveEvidenceSetKindV1 {
        self.kind
    }

    /// Evaluated-key purpose.
    #[must_use]
    pub const fn purpose(self) -> ZkAmsMkheCollectiveEvaluatedKeyPurposeV1 {
        self.purpose
    }

    /// Exact evaluated-key ordinal.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        self.ordinal
    }

    /// Frozen Galois exponent, or zero for relinearization.
    #[must_use]
    pub const fn galois_exponent(self) -> u32 {
        self.galois_exponent
    }

    /// Verified aggregate CPK identity.
    #[must_use]
    pub const fn collective_key_digest(self) -> [u8; 32] {
        self.collective_key_digest
    }
}

/// Exact identity and preflighted length announced before a record stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveEvidenceRecordHeaderV1 {
    set: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
    kind: ZkAmsMkheCollectiveEvidenceRecordKindV1,
    record_index: u32,
    canonical_bytes: u64,
}

impl ZkAmsMkheCollectiveEvidenceRecordHeaderV1 {
    /// Parent set identity.
    #[must_use]
    pub const fn set(self) -> ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
        self.set
    }

    /// Exact record family.
    #[must_use]
    pub const fn kind(self) -> ZkAmsMkheCollectiveEvidenceRecordKindV1 {
        self.kind
    }

    /// Gap-free canonical record position.
    #[must_use]
    pub const fn record_index(self) -> u32 {
        self.record_index
    }

    /// Exact self-delimiting `ZASE` or `ZACE` bytes, including digest footer.
    #[must_use]
    pub const fn canonical_bytes(self) -> u64 {
        self.canonical_bytes
    }
}

/// Record commitment announced only after every bounded chunk was accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveEvidenceRecordFooterV1 {
    header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
    chunk_count: u32,
    canonical_digest: [u8; 32],
}

impl ZkAmsMkheCollectiveEvidenceRecordFooterV1 {
    /// Exact opening header.
    #[must_use]
    pub const fn header(self) -> ZkAmsMkheCollectiveEvidenceRecordHeaderV1 {
        self.header
    }

    /// Exact gap-free callback chunk count.
    #[must_use]
    pub const fn chunk_count(self) -> u32 {
        self.chunk_count
    }

    /// Keccak-256 of every record byte preceding the final digest footer.
    #[must_use]
    pub const fn canonical_digest(self) -> [u8; 32] {
        self.canonical_digest
    }
}

/// Final set commitment after its exact gap-free record count was hashed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveEvidenceSetFooterV1 {
    header: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
    record_count: u32,
    canonical_digest: [u8; 32],
}

impl ZkAmsMkheCollectiveEvidenceSetFooterV1 {
    /// Exact opening header.
    #[must_use]
    pub const fn header(self) -> ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
        self.header
    }

    /// Exact gap-free record count.
    #[must_use]
    pub const fn record_count(self) -> u32 {
        self.record_count
    }

    /// Canonical set digest committed by the generated key and manifest entry.
    #[must_use]
    pub const fn canonical_digest(self) -> [u8; 32] {
        self.canonical_digest
    }
}

/// One generated, canonical seeded compact key and its exact evidence identities.
#[derive(PartialEq, Eq)]
pub struct ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1 {
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    galois_exponent: u32,
    collective_key_digest: [u8; 32],
    source_proof_set_digest: [u8; 32],
    cks_proof_set_digest: [u8; 32],
    payload_blake3: [u8; 32],
    payload_bytes: u64,
    wire: ZkAmsMkheSeededRkgKeyWireV1,
}

impl core::fmt::Debug for ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1")
            .field("purpose", &self.purpose)
            .field("ordinal", &self.ordinal)
            .field("galois_exponent", &self.galois_exponent)
            .field(
                "collective_key_digest",
                &hex::encode(self.collective_key_digest),
            )
            .field(
                "source_proof_set_digest",
                &hex::encode(self.source_proof_set_digest),
            )
            .field(
                "cks_proof_set_digest",
                &hex::encode(self.cks_proof_set_digest),
            )
            .field("payload_blake3", &hex::encode(self.payload_blake3))
            .field("payload_bytes", &self.payload_bytes)
            .field("stored_digits", &self.wire.stored_b_digits().len())
            .finish()
    }
}

impl ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1 {
    /// Evaluated-key purpose.
    #[must_use]
    pub const fn purpose(&self) -> ZkAmsMkheCollectiveEvaluatedKeyPurposeV1 {
        self.purpose
    }

    /// Exact release ordinal: relinearization first, then frozen Galois schedule order.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }

    /// Frozen odd Galois exponent, or zero for relinearization.
    #[must_use]
    pub const fn galois_exponent(&self) -> u32 {
        self.galois_exponent
    }

    /// Collective public-key identity used by every source proof and CKS statement.
    #[must_use]
    pub const fn collective_key_digest(&self) -> [u8; 32] {
        self.collective_key_digest
    }

    /// Digest of the exact authenticated pairwise-RKG or Galois-source proof set.
    #[must_use]
    pub const fn source_proof_set_digest(&self) -> [u8; 32] {
        self.source_proof_set_digest
    }

    /// Digest of all exact ordered full-roster CKS contribution proofs.
    #[must_use]
    pub const fn cks_proof_set_digest(&self) -> [u8; 32] {
        self.cks_proof_set_digest
    }

    /// BLAKE3 identity of the exact canonical `ZARK` payload.
    #[must_use]
    pub const fn payload_blake3(&self) -> [u8; 32] {
        self.payload_blake3
    }

    /// Exact canonical payload bytes.
    #[must_use]
    pub const fn payload_bytes(&self) -> u64 {
        self.payload_bytes
    }

    /// Canonical seeded two-polynomial key wire record.
    #[must_use]
    pub const fn wire(&self) -> &ZkAmsMkheSeededRkgKeyWireV1 {
        &self.wire
    }

    /// Build the exact manifest entry at its canonical offset.
    pub fn manifest_entry(
        &self,
        payload_offset: u64,
    ) -> Result<ZkAmsMkheCollectiveEvaluatedKeyEntryV1, ZkAmsMkheErrorV1> {
        ZkAmsMkheCollectiveEvaluatedKeyEntryV1::new(
            self.ordinal,
            self.purpose,
            self.galois_exponent,
            payload_offset,
            self.payload_bytes,
            self.payload_blake3,
            self.source_proof_set_digest,
            self.cks_proof_set_digest,
        )
    }
}

/// Complete public statement carried beside one authenticated source proof.
///
/// The polynomial references remain valid only for the duration of the sink
/// callback.  A durable sink serializes or otherwise persists their residues;
/// retaining only the proof is deliberately insufficient.
#[derive(Clone, Copy)]
pub enum ZkAmsMkheCollectiveSourceStatementEvidenceV1<'a> {
    /// One party's first-round contribution to one canonical unordered RKG pair.
    RkgRoundOne {
        /// Common collective-public-key `a`.
        public_a: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's verified collective-public-key `b_i`.
        party_public_b: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Deterministic pair/digit RKG polynomial.
        common_a: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's first round constant contribution.
        h0: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's first round linear contribution.
        h1: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Canonical left pair endpoint.
        left: ZkAmsMkhePartyIdV1,
        /// Canonical right pair endpoint.
        right: ZkAmsMkhePartyIdV1,
        /// Balanced gadget digit.
        digit_index: u32,
    },
    /// One party's second-round contribution, equality-linked to round one.
    RkgRoundTwo {
        /// Common collective-public-key `a`.
        public_a: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's verified collective-public-key `b_i`.
        party_public_b: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Deterministic pair/digit RKG polynomial.
        common_a: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's first round constant contribution.
        h0: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's first round linear contribution.
        h1: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Ordered aggregate of every first-round constant contribution.
        aggregate_h0: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Ordered aggregate of every first-round linear contribution.
        aggregate_h1: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's second-round constant contribution.
        k0: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Canonical left pair endpoint.
        left: ZkAmsMkhePartyIdV1,
        /// Canonical right pair endpoint.
        right: ZkAmsMkhePartyIdV1,
        /// Balanced gadget digit.
        digit_index: u32,
    },
    /// One party's encryption of `g^d sigma_k(s_i)`.
    Galois {
        /// Common collective-public-key `a`.
        public_a: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's verified collective-public-key `b_i`.
        party_public_b: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Source-encryption constant polynomial.
        source_constant: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Source-encryption linear polynomial.
        source_linear: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Exact frozen schedule position.
        schedule_index: u8,
        /// Exact odd automorphism exponent at that position.
        exponent: u32,
        /// Balanced gadget digit.
        digit_index: u32,
    },
}

impl core::fmt::Debug for ZkAmsMkheCollectiveSourceStatementEvidenceV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RkgRoundOne {
                left,
                right,
                digit_index,
                ..
            } => formatter
                .debug_struct("RkgRoundOne")
                .field("left", left)
                .field("right", right)
                .field("digit_index", digit_index)
                .finish_non_exhaustive(),
            Self::RkgRoundTwo {
                left,
                right,
                digit_index,
                ..
            } => formatter
                .debug_struct("RkgRoundTwo")
                .field("left", left)
                .field("right", right)
                .field("digit_index", digit_index)
                .finish_non_exhaustive(),
            Self::Galois {
                schedule_index,
                exponent,
                digit_index,
                ..
            } => formatter
                .debug_struct("Galois")
                .field("schedule_index", schedule_index)
                .field("exponent", exponent)
                .field("digit_index", digit_index)
                .finish_non_exhaustive(),
        }
    }
}

/// Replayable evidence for one source proof in the exact generation sequence.
pub struct ZkAmsMkheCollectiveSourceProofEvidenceV1<'a> {
    ordinal: u8,
    source_record_index: u32,
    party_index: u8,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    statement: ZkAmsMkheCollectiveSourceStatementEvidenceV1<'a>,
    proof: &'a ZkAmsMkheActiveRkgProofV1,
}

impl core::fmt::Debug for ZkAmsMkheCollectiveSourceProofEvidenceV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCollectiveSourceProofEvidenceV1")
            .field("ordinal", &self.ordinal)
            .field("source_record_index", &self.source_record_index)
            .field("party_index", &self.party_index)
            .field(
                "collective_key_digest",
                &hex::encode(self.collective_key_digest),
            )
            .field("statement", &self.statement)
            .field(
                "statement_digest",
                &hex::encode(self.proof.statement_digest()),
            )
            .finish()
    }
}

impl<'a> ZkAmsMkheCollectiveSourceProofEvidenceV1<'a> {
    /// Evaluated-key ordinal containing this proof.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }

    /// Gap-free canonical record position within the source evidence stream.
    #[must_use]
    pub const fn source_record_index(&self) -> u32 {
        self.source_record_index
    }

    /// Exact governed contributor position.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.party_index
    }

    /// Frozen release-profile identity.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }

    /// Exact ordered roster identity.
    #[must_use]
    pub const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }

    /// Exact active authentication-key-set identity.
    #[must_use]
    pub const fn key_material_digest(&self) -> [u8; 32] {
        self.key_material_digest
    }

    /// Governed key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Exact ceremony transcript.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }

    /// Verified aggregate CPK identity.
    #[must_use]
    pub const fn collective_key_digest(&self) -> [u8; 32] {
        self.collective_key_digest
    }

    /// Complete reconstructible public algebraic statement.
    #[must_use]
    pub const fn statement(&self) -> ZkAmsMkheCollectiveSourceStatementEvidenceV1<'a> {
        self.statement
    }

    /// Authenticated native active proof.
    #[must_use]
    pub const fn proof(&self) -> &'a ZkAmsMkheActiveRkgProofV1 {
        self.proof
    }

    /// Digest of the exact canonical evidence byte stream used by key generation.
    pub fn canonical_digest(&self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let canonical_bytes = self.canonical_encoded_len()?;
        let body_bytes = canonical_bytes
            .checked_sub(EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let mut writer = CanonicalDigestWriter::new(body_bytes);
        self.write_canonical_body(&mut writer, canonical_bytes)?;
        writer.finish()
    }

    /// Exact self-delimiting `ZASE` bytes, including its digest footer.
    pub fn canonical_encoded_len(&self) -> Result<usize, ZkAmsMkheErrorV1> {
        let polynomial_bytes = canonical_wire_polynomial_bytes()?;
        let (metadata_bytes, polynomial_count) = match self.statement {
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundOne { .. } => (32 + 32 + 4, 5),
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundTwo { .. } => (32 + 32 + 4, 8),
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::Galois { .. } => (1 + 4 + 4, 4),
        };
        SOURCE_EVIDENCE_COMMON_BODY_BYTES_V1
            .checked_add(metadata_bytes)
            .and_then(|value| {
                polynomial_bytes
                    .checked_mul(polynomial_count)
                    .and_then(|bytes| value.checked_add(bytes))
            })
            .and_then(|value| value.checked_add(8))
            .and_then(|value| value.checked_add(self.proof.evidence_encoded_len().ok()?))
            .and_then(|value| value.checked_add(EVIDENCE_RECORD_DIGEST_BYTES_V1))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    }

    /// Independently replay the statement, topology, active proof, and CPK linkage.
    pub fn verify(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        collective_key: &ZkAmsMkheCollectivePublicKeyV1,
        shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        validate_evidence_collective_context(
            roster,
            self.profile_digest,
            self.roster_digest,
            self.key_material_digest,
            self.epoch,
            self.transcript_digest,
            self.collective_key_digest,
            collective_key,
            shares,
        )?;
        let party_index = usize::from(self.party_index);
        let share = shares
            .get(party_index)
            .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
        let public_key = ZkAmsMkheActiveCollectivePublicKeyStatementV1::new(
            share.public_a(),
            share.party_public_b(),
        )?;
        let expected_record_index = match self.statement {
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundOne {
                public_a,
                party_public_b,
                common_a,
                h0,
                h1,
                left,
                right,
                digit_index,
            } => {
                if self.ordinal != 0
                    || public_a != share.public_a()
                    || party_public_b != share.party_public_b()
                {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                let statement = ZkAmsMkheActiveRkgRoundOneStatementV1::new(
                    public_key,
                    common_a,
                    h0,
                    h1,
                    left,
                    right,
                    digit_index,
                )?;
                verify_zk_ams_mkhe_active_rkg_round_one_v1(
                    roster,
                    self.transcript_digest,
                    party_index,
                    statement,
                    self.proof,
                )?;
                expected_rkg_source_record_index(
                    roster,
                    left,
                    right,
                    digit_index,
                    party_index,
                    false,
                )?
            }
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundTwo {
                public_a,
                party_public_b,
                common_a,
                h0,
                h1,
                aggregate_h0,
                aggregate_h1,
                k0,
                left,
                right,
                digit_index,
            } => {
                if self.ordinal != 0
                    || public_a != share.public_a()
                    || party_public_b != share.party_public_b()
                {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                let round_one = ZkAmsMkheActiveRkgRoundOneStatementV1::new(
                    public_key,
                    common_a,
                    h0,
                    h1,
                    left,
                    right,
                    digit_index,
                )?;
                let statement = ZkAmsMkheActiveRkgRoundTwoStatementV1::new(
                    round_one,
                    aggregate_h0,
                    aggregate_h1,
                    k0,
                )?;
                verify_zk_ams_mkhe_active_rkg_round_two_v1(
                    roster,
                    self.transcript_digest,
                    party_index,
                    statement,
                    self.proof,
                )?;
                expected_rkg_source_record_index(
                    roster,
                    left,
                    right,
                    digit_index,
                    party_index,
                    true,
                )?
            }
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::Galois {
                public_a,
                party_public_b,
                source_constant,
                source_linear,
                schedule_index,
                exponent,
                digit_index,
            } => {
                if self.ordinal != schedule_index.saturating_add(1)
                    || public_a != share.public_a()
                    || party_public_b != share.party_public_b()
                {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                let statement = ZkAmsMkheActiveGaloisSourceStatementV1::new(
                    public_key,
                    source_constant,
                    source_linear,
                    usize::from(schedule_index),
                    exponent,
                    usize::try_from(digit_index)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                )?;
                verify_zk_ams_mkhe_active_galois_source_v1(
                    roster,
                    self.transcript_digest,
                    party_index,
                    statement,
                    self.proof,
                )?;
                digit_index
                    .checked_mul(
                        u32::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                    )
                    .and_then(|base| base.checked_add(u32::from(self.party_index)))
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            }
        };
        if expected_record_index != self.source_record_index {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    fn record_kind(&self) -> ZkAmsMkheCollectiveEvidenceRecordKindV1 {
        match self.statement {
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundOne { .. } => {
                ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundOne
            }
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundTwo { .. } => {
                ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundTwo
            }
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::Galois { .. } => {
                ZkAmsMkheCollectiveEvidenceRecordKindV1::GaloisSource
            }
        }
    }

    fn write_canonical_body(
        &self,
        writer: &mut impl CanonicalBodyWriter,
        canonical_bytes: usize,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        write_canonical_bytes(writer, &SOURCE_EVIDENCE_RECORD_TAG_V1)?;
        write_canonical_u8(writer, MKHE_VERSION_V1)?;
        write_canonical_u8(writer, self.record_kind() as u8)?;
        write_canonical_u64(
            writer,
            u64::try_from(canonical_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        write_canonical_u8(writer, self.ordinal)?;
        write_canonical_u32(writer, self.source_record_index)?;
        write_canonical_u8(writer, self.party_index)?;
        write_canonical_bytes(writer, &self.profile_digest)?;
        write_canonical_bytes(writer, &self.roster_digest)?;
        write_canonical_bytes(writer, &self.key_material_digest)?;
        write_canonical_u64(writer, self.epoch)?;
        write_canonical_bytes(writer, &self.transcript_digest)?;
        write_canonical_bytes(writer, &self.collective_key_digest)?;
        match self.statement {
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundOne {
                public_a,
                party_public_b,
                common_a,
                h0,
                h1,
                left,
                right,
                digit_index,
            } => {
                write_canonical_bytes(writer, &left.to_bytes())?;
                write_canonical_bytes(writer, &right.to_bytes())?;
                write_canonical_u32(writer, digit_index)?;
                write_canonical_wire_polynomial(writer, public_a)?;
                write_canonical_wire_polynomial(writer, party_public_b)?;
                write_canonical_wire_polynomial(writer, common_a)?;
                write_canonical_wire_polynomial(writer, h0)?;
                write_canonical_wire_polynomial(writer, h1)?;
            }
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundTwo {
                public_a,
                party_public_b,
                common_a,
                h0,
                h1,
                aggregate_h0,
                aggregate_h1,
                k0,
                left,
                right,
                digit_index,
            } => {
                write_canonical_bytes(writer, &left.to_bytes())?;
                write_canonical_bytes(writer, &right.to_bytes())?;
                write_canonical_u32(writer, digit_index)?;
                write_canonical_wire_polynomial(writer, public_a)?;
                write_canonical_wire_polynomial(writer, party_public_b)?;
                write_canonical_wire_polynomial(writer, common_a)?;
                write_canonical_wire_polynomial(writer, h0)?;
                write_canonical_wire_polynomial(writer, h1)?;
                write_canonical_wire_polynomial(writer, aggregate_h0)?;
                write_canonical_wire_polynomial(writer, aggregate_h1)?;
                write_canonical_wire_polynomial(writer, k0)?;
            }
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::Galois {
                public_a,
                party_public_b,
                source_constant,
                source_linear,
                schedule_index,
                exponent,
                digit_index,
            } => {
                write_canonical_u8(writer, schedule_index)?;
                write_canonical_u32(writer, exponent)?;
                write_canonical_u32(writer, digit_index)?;
                write_canonical_wire_polynomial(writer, public_a)?;
                write_canonical_wire_polynomial(writer, party_public_b)?;
                write_canonical_wire_polynomial(writer, source_constant)?;
                write_canonical_wire_polynomial(writer, source_linear)?;
            }
        }
        let proof_bytes = self.proof.evidence_encoded_len()?;
        write_canonical_u64(
            writer,
            u64::try_from(proof_bytes).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        self.proof
            .write_evidence_chunks(|chunk| write_canonical_bytes(writer, chunk))
    }
}

/// Owned statement reconstructed from one exact canonical `ZASE` record.
#[derive(Debug, PartialEq, Eq)]
pub enum ZkAmsMkheOwnedCollectiveSourceStatementEvidenceV1 {
    /// Complete first-round RKG statement.
    RkgRoundOne {
        public_a: ZkAmsMkheRnsPolynomialWireV1,
        party_public_b: ZkAmsMkheRnsPolynomialWireV1,
        common_a: ZkAmsMkheRnsPolynomialWireV1,
        h0: ZkAmsMkheRnsPolynomialWireV1,
        h1: ZkAmsMkheRnsPolynomialWireV1,
        left: ZkAmsMkhePartyIdV1,
        right: ZkAmsMkhePartyIdV1,
        digit_index: u32,
    },
    /// Complete second-round RKG statement.
    RkgRoundTwo {
        public_a: ZkAmsMkheRnsPolynomialWireV1,
        party_public_b: ZkAmsMkheRnsPolynomialWireV1,
        common_a: ZkAmsMkheRnsPolynomialWireV1,
        h0: ZkAmsMkheRnsPolynomialWireV1,
        h1: ZkAmsMkheRnsPolynomialWireV1,
        aggregate_h0: ZkAmsMkheRnsPolynomialWireV1,
        aggregate_h1: ZkAmsMkheRnsPolynomialWireV1,
        k0: ZkAmsMkheRnsPolynomialWireV1,
        left: ZkAmsMkhePartyIdV1,
        right: ZkAmsMkhePartyIdV1,
        digit_index: u32,
    },
    /// Complete automorphism-linked source statement.
    Galois {
        public_a: ZkAmsMkheRnsPolynomialWireV1,
        party_public_b: ZkAmsMkheRnsPolynomialWireV1,
        source_constant: ZkAmsMkheRnsPolynomialWireV1,
        source_linear: ZkAmsMkheRnsPolynomialWireV1,
        schedule_index: u8,
        exponent: u32,
        digit_index: u32,
    },
}

impl ZkAmsMkheOwnedCollectiveSourceStatementEvidenceV1 {
    fn borrowed(&self) -> ZkAmsMkheCollectiveSourceStatementEvidenceV1<'_> {
        match self {
            Self::RkgRoundOne {
                public_a,
                party_public_b,
                common_a,
                h0,
                h1,
                left,
                right,
                digit_index,
            } => ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundOne {
                public_a,
                party_public_b,
                common_a,
                h0,
                h1,
                left: *left,
                right: *right,
                digit_index: *digit_index,
            },
            Self::RkgRoundTwo {
                public_a,
                party_public_b,
                common_a,
                h0,
                h1,
                aggregate_h0,
                aggregate_h1,
                k0,
                left,
                right,
                digit_index,
            } => ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundTwo {
                public_a,
                party_public_b,
                common_a,
                h0,
                h1,
                aggregate_h0,
                aggregate_h1,
                k0,
                left: *left,
                right: *right,
                digit_index: *digit_index,
            },
            Self::Galois {
                public_a,
                party_public_b,
                source_constant,
                source_linear,
                schedule_index,
                exponent,
                digit_index,
            } => ZkAmsMkheCollectiveSourceStatementEvidenceV1::Galois {
                public_a,
                party_public_b,
                source_constant,
                source_linear,
                schedule_index: *schedule_index,
                exponent: *exponent,
                digit_index: *digit_index,
            },
        }
    }
}

/// One owned, exactly decoded and independently replayable `ZASE` record.
pub struct ZkAmsMkheOwnedCollectiveSourceProofEvidenceV1 {
    ordinal: u8,
    source_record_index: u32,
    party_index: u8,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    statement: ZkAmsMkheOwnedCollectiveSourceStatementEvidenceV1,
    proof: ZkAmsMkheActiveRkgProofV1,
    canonical_bytes: u64,
    canonical_digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkheOwnedCollectiveSourceProofEvidenceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheOwnedCollectiveSourceProofEvidenceV1")
            .field("ordinal", &self.ordinal)
            .field("source_record_index", &self.source_record_index)
            .field("party_index", &self.party_index)
            .field("canonical_bytes", &self.canonical_bytes)
            .field("canonical_digest", &hex::encode(self.canonical_digest))
            .field("statement", &self.statement)
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheOwnedCollectiveSourceProofEvidenceV1 {
    fn borrowed(&self) -> ZkAmsMkheCollectiveSourceProofEvidenceV1<'_> {
        ZkAmsMkheCollectiveSourceProofEvidenceV1 {
            ordinal: self.ordinal,
            source_record_index: self.source_record_index,
            party_index: self.party_index,
            profile_digest: self.profile_digest,
            roster_digest: self.roster_digest,
            key_material_digest: self.key_material_digest,
            epoch: self.epoch,
            transcript_digest: self.transcript_digest,
            collective_key_digest: self.collective_key_digest,
            statement: self.statement.borrowed(),
            proof: &self.proof,
        }
    }

    /// Exact canonical record length accepted from durable storage.
    #[must_use]
    pub const fn canonical_bytes(&self) -> u64 {
        self.canonical_bytes
    }

    /// Verified digest footer of every preceding canonical record byte.
    #[must_use]
    pub const fn canonical_digest(&self) -> [u8; 32] {
        self.canonical_digest
    }

    /// Re-run the complete topology, CPK linkage, proof, and authentication checks.
    pub fn verify(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        collective_key: &ZkAmsMkheCollectivePublicKeyV1,
        shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.borrowed().verify(roster, collective_key, shares)
    }

    /// Decode exactly one `ZASE` record, require immediate EOF, and replay it
    /// under independently trusted roster, aggregate CPK, and ordered shares.
    pub fn decode_and_verify_canonical_exact<R: std::io::Read>(
        reader: &mut R,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        collective_key: &ZkAmsMkheCollectivePublicKeyV1,
        shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let value = decode_source_evidence_record(reader)?;
        require_canonical_reader_eof(reader)?;
        value.verify(roster, collective_key, shares)?;
        Ok(value)
    }
}

/// Complete source, target-key context, proofs, and recomputed compact output
/// for one full-roster CKS digit.
pub struct ZkAmsMkheCollectiveCksDigitEvidenceV1<'a> {
    ordinal: u8,
    digit_index: u8,
    collective_key_digest: [u8; 32],
    roster: &'a ZkAmsMkheGovernedRosterWireV1,
    source: &'a ZkAmsMkheCksSourceCiphertextV1,
    target_a: &'a ZkAmsMkheRnsPolynomialWireV1,
    public_key_a: &'a ZkAmsMkheRnsPolynomialWireV1,
    party_public_b: [&'a ZkAmsMkheRnsPolynomialWireV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    contributions: &'a [ZkAmsMkheAuthenticatedCksContributionV1],
    compact_constant: &'a ZkAmsMkheRnsPolynomialWireV1,
}

impl core::fmt::Debug for ZkAmsMkheCollectiveCksDigitEvidenceV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCollectiveCksDigitEvidenceV1")
            .field("ordinal", &self.ordinal)
            .field("digit_index", &self.digit_index)
            .field(
                "collective_key_digest",
                &hex::encode(self.collective_key_digest),
            )
            .field("source_digest", &hex::encode(self.source.source_digest()))
            .field("contributions", &self.contributions.len())
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheCollectiveCksDigitEvidenceV1<'_> {
    /// Evaluated-key ordinal containing this digit.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }

    /// Exact balanced gadget digit.
    #[must_use]
    pub const fn digit_index(&self) -> u8 {
        self.digit_index
    }

    /// Verified aggregate CPK identity.
    #[must_use]
    pub const fn collective_key_digest(&self) -> [u8; 32] {
        self.collective_key_digest
    }

    /// Complete exact governed wire roster used by the CKS statement.
    #[must_use]
    pub const fn roster(&self) -> &ZkAmsMkheGovernedRosterWireV1 {
        self.roster
    }

    /// Full independently keyed source ciphertext.
    #[must_use]
    pub const fn source(&self) -> &ZkAmsMkheCksSourceCiphertextV1 {
        self.source
    }

    /// Compact target `a` polynomial.
    #[must_use]
    pub const fn target_a(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        self.target_a
    }

    /// Common verified collective-public-key `a` relation polynomial.
    #[must_use]
    pub const fn public_key_a(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        self.public_key_a
    }

    /// Exact ordered verified collective-public-key `b_i` relation polynomials.
    #[must_use]
    pub const fn party_public_b(
        &self,
    ) -> &[&ZkAmsMkheRnsPolynomialWireV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] {
        &self.party_public_b
    }

    /// Exact ordered authenticated CKS contribution set.
    #[must_use]
    pub const fn contributions(&self) -> &[ZkAmsMkheAuthenticatedCksContributionV1] {
        self.contributions
    }

    /// Recomputed compact constant polynomial stored in the generated key.
    #[must_use]
    pub const fn compact_constant(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        self.compact_constant
    }

    /// Digest of the exact canonical evidence byte stream used by key generation.
    pub fn canonical_digest(&self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let canonical_bytes = self.canonical_encoded_len()?;
        let body_bytes = canonical_bytes
            .checked_sub(EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let mut writer = CanonicalDigestWriter::new(body_bytes);
        self.write_canonical_body(&mut writer, canonical_bytes)?;
        writer.finish()
    }

    /// Exact self-delimiting `ZACE` bytes, including its digest footer.
    pub fn canonical_encoded_len(&self) -> Result<usize, ZkAmsMkheErrorV1> {
        let polynomial_bytes = canonical_wire_polynomial_bytes()?;
        let roster_bytes = self.roster.encode()?.len();
        let present_components = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .filter(|party_index| self.source.component(*party_index).is_some())
            .count();
        let mut bytes = CKS_EVIDENCE_COMMON_BODY_BYTES_V1
            .checked_add(4)
            .and_then(|value| value.checked_add(roster_bytes))
            .and_then(|value| value.checked_add(32 + 4 + 8 + 1 + 32))
            .and_then(|value| value.checked_add(polynomial_bytes))
            .and_then(|value| value.checked_add(1))
            .and_then(|value| {
                value.checked_add(
                    ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
                        .checked_mul(32 + 1)
                        .and_then(|metadata| {
                            present_components
                                .checked_mul(polynomial_bytes)
                                .and_then(|polynomials| metadata.checked_add(polynomials))
                        })?,
                )
            })
            .and_then(|value| value.checked_add(polynomial_bytes.checked_mul(2)?))
            .and_then(|value| value.checked_add(1))
            .and_then(|value| {
                value.checked_add(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1.checked_mul(polynomial_bytes)?)
            })
            .and_then(|value| value.checked_add(polynomial_bytes))
            .and_then(|value| value.checked_add(1))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let statement = self.statement()?;
        for contribution in self.contributions {
            let contribution_bytes = contribution.to_release_wire(statement)?.encode()?.len();
            bytes = bytes
                .checked_add(8)
                .and_then(|value| value.checked_add(contribution_bytes))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
        bytes
            .checked_add(EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    }

    /// Independently replay all eight CKS proofs and the compact output.
    pub fn verify(
        &self,
        active_roster: &ZkAmsMkheGovernedActiveRosterV1,
        collective_key: &ZkAmsMkheCollectivePublicKeyV1,
        shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        validate_evidence_collective_context(
            active_roster,
            self.roster.profile_digest(),
            self.roster.roster_digest(),
            active_roster.key_material_digest(),
            self.roster.epoch(),
            self.source.transcript_digest(),
            self.collective_key_digest,
            collective_key,
            shares,
        )?;
        if self.roster != &active_roster.to_wire_roster()?
            || self.public_key_a != shares[0].public_a()
            || self
                .party_public_b
                .iter()
                .zip(shares)
                .any(|(observed, share)| *observed != share.party_public_b())
            || self.contributions.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let statement = self.statement()?;
        let compact = combine_zk_ams_mkhe_cks_v1(statement, self.contributions)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCksSet)?;
        if compact.constant() != self.compact_constant || compact.linear() != self.target_a {
            return Err(ZkAmsMkheErrorV1::InvalidCksSet);
        }
        Ok(())
    }

    fn statement(&self) -> Result<ZkAmsMkheCksStatementV1<'_>, ZkAmsMkheErrorV1> {
        ZkAmsMkheCksStatementV1::new(
            self.roster,
            self.source,
            self.target_a,
            self.public_key_a,
            &self.party_public_b,
        )
    }

    fn write_canonical_body(
        &self,
        writer: &mut impl CanonicalBodyWriter,
        canonical_bytes: usize,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let statement = self.statement()?;
        write_canonical_bytes(writer, &CKS_EVIDENCE_RECORD_TAG_V1)?;
        write_canonical_u8(writer, MKHE_VERSION_V1)?;
        write_canonical_u64(
            writer,
            u64::try_from(canonical_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        write_canonical_u8(writer, self.ordinal)?;
        write_canonical_u8(writer, self.digit_index)?;
        write_canonical_bytes(writer, &self.collective_key_digest)?;
        let roster_bytes = self.roster.encode()?;
        write_canonical_u32(
            writer,
            u32::try_from(roster_bytes.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        write_canonical_bytes(writer, &roster_bytes)?;
        write_canonical_bytes(writer, &self.source.transcript_digest())?;
        write_canonical_u32(writer, self.source.record_index())?;
        write_canonical_u64(writer, self.source.sample_index())?;
        write_canonical_u8(writer, self.source.level())?;
        write_canonical_bytes(writer, &self.source.source_digest())?;
        write_canonical_wire_polynomial(writer, self.source.constant())?;
        write_canonical_u8(
            writer,
            u8::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        )?;
        for (party, component) in self.roster.parties().iter().zip(
            (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                .map(|party_index| self.source.component(party_index)),
        ) {
            write_canonical_bytes(writer, &party.to_bytes())?;
            write_canonical_u8(writer, u8::from(component.is_some()))?;
            if let Some(component) = component {
                write_canonical_wire_polynomial(writer, component)?;
            }
        }
        write_canonical_wire_polynomial(writer, self.target_a)?;
        write_canonical_wire_polynomial(writer, self.public_key_a)?;
        write_canonical_u8(
            writer,
            u8::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        )?;
        for party_public_b in self.party_public_b {
            write_canonical_wire_polynomial(writer, party_public_b)?;
        }
        write_canonical_wire_polynomial(writer, self.compact_constant)?;
        write_canonical_u8(
            writer,
            u8::try_from(self.contributions.len()).map_err(|_| ZkAmsMkheErrorV1::InvalidCksSet)?,
        )?;
        for contribution in self.contributions {
            let wire = contribution.to_release_wire(statement)?;
            let bytes = wire.encode()?;
            write_canonical_u64(
                writer,
                u64::try_from(bytes.len())
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )?;
            write_canonical_bytes(writer, &bytes)?;
        }
        Ok(())
    }
}

/// One owned, exactly decoded and independently replayable `ZACE` record.
pub struct ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1 {
    ordinal: u8,
    digit_index: u8,
    collective_key_digest: [u8; 32],
    roster: ZkAmsMkheGovernedRosterWireV1,
    source: ZkAmsMkheCksSourceCiphertextV1,
    target_a: ZkAmsMkheRnsPolynomialWireV1,
    public_key_a: ZkAmsMkheRnsPolynomialWireV1,
    party_public_b: [ZkAmsMkheRnsPolynomialWireV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    contributions: Vec<ZkAmsMkheAuthenticatedCksContributionV1>,
    compact_constant: ZkAmsMkheRnsPolynomialWireV1,
    canonical_bytes: u64,
    canonical_digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1")
            .field("ordinal", &self.ordinal)
            .field("digit_index", &self.digit_index)
            .field("canonical_bytes", &self.canonical_bytes)
            .field("canonical_digest", &hex::encode(self.canonical_digest))
            .field("source_digest", &hex::encode(self.source.source_digest()))
            .field("contributions", &self.contributions.len())
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1 {
    fn borrowed(&self) -> ZkAmsMkheCollectiveCksDigitEvidenceV1<'_> {
        ZkAmsMkheCollectiveCksDigitEvidenceV1 {
            ordinal: self.ordinal,
            digit_index: self.digit_index,
            collective_key_digest: self.collective_key_digest,
            roster: &self.roster,
            source: &self.source,
            target_a: &self.target_a,
            public_key_a: &self.public_key_a,
            party_public_b: std::array::from_fn(|index| &self.party_public_b[index]),
            contributions: &self.contributions,
            compact_constant: &self.compact_constant,
        }
    }

    /// Exact canonical record length accepted from durable storage.
    #[must_use]
    pub const fn canonical_bytes(&self) -> u64 {
        self.canonical_bytes
    }

    /// Verified digest footer of every preceding canonical record byte.
    #[must_use]
    pub const fn canonical_digest(&self) -> [u8; 32] {
        self.canonical_digest
    }

    /// Re-run all eight CKS proofs, ordered CPK linkage, and compact output checks.
    pub fn verify(
        &self,
        active_roster: &ZkAmsMkheGovernedActiveRosterV1,
        collective_key: &ZkAmsMkheCollectivePublicKeyV1,
        shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.borrowed()
            .verify(active_roster, collective_key, shares)
    }

    /// Decode exactly one `ZACE` record, require immediate EOF, and replay it
    /// under independently trusted roster, aggregate CPK, and ordered shares.
    pub fn decode_and_verify_canonical_exact<R: std::io::Read>(
        reader: &mut R,
        active_roster: &ZkAmsMkheGovernedActiveRosterV1,
        collective_key: &ZkAmsMkheCollectivePublicKeyV1,
        shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let value = decode_cks_evidence_record(reader, active_roster)?;
        require_canonical_reader_eof(reader)?;
        value.verify(active_roster, collective_key, shares)?;
        Ok(value)
    }
}

/// Generation-driven durable sink for the exact canonical evidence byte stream.
///
/// Generation first replays each complete statement and proof. It then feeds
/// the same deterministic chunks, in the same order, to both the evidence-set
/// hash and this sink. A sink error fails key generation; there is no advisory
/// callback path that could silently persist a different representation.
pub trait ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1 {
    /// Open one source or CKS evidence set before its first record.
    fn begin_evidence_set(
        &mut self,
        header: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;

    /// Open one preflighted, self-delimiting canonical record.
    fn begin_evidence_record(
        &mut self,
        header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;

    /// Persist the next exact bounded chunk at a gap-free index.
    fn write_evidence_record_chunk(
        &mut self,
        header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
        chunk_index: u32,
        bytes: &[u8],
    ) -> Result<(), ZkAmsMkheErrorV1>;

    /// Atomically close one record after its exact length and digest are known.
    fn finish_evidence_record(
        &mut self,
        footer: ZkAmsMkheCollectiveEvidenceRecordFooterV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;

    /// Close one exact set after its gap-free count is committed to the digest.
    fn finish_evidence_set(
        &mut self,
        footer: ZkAmsMkheCollectiveEvidenceSetFooterV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;
}

struct CeremonyContext<'a> {
    profile: BgvProfile,
    roster: &'a ZkAmsMkheGovernedActiveRosterV1,
    wire_roster: ZkAmsMkheGovernedRosterWireV1,
    transcript_digest: [u8; 32],
    shares: [&'a ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    states: [&'a ZkAmsMkheCollectivePartyStateV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    authentication_secrets: [&'a ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    collective_key: ZkAmsMkheCollectivePublicKeyV1,
}

impl<'a> CeremonyContext<'a> {
    fn new(
        roster: &'a ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        shares: [&'a ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        states: [&'a ZkAmsMkheCollectivePartyStateV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        authentication_secrets: [&'a ZkAmsMkheActivePartySecretV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        roster.validate()?;
        if transcript_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let profile = release_profile_v1();
        profile.validate()?;
        let collective_key =
            aggregate_zk_ams_mkhe_collective_public_key_v1(roster, transcript_digest, shares)?;
        for index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            let expected = roster.participants()[index].party();
            if states[index].party() != expected
                || usize::from(states[index].party_index()) != index
                || states[index].profile_digest_internal() != roster.profile_digest()
                || states[index].roster_digest_internal() != roster.roster_digest()
                || states[index].key_material_digest_internal() != roster.key_material_digest()
                || states[index].epoch() != roster.epoch()
                || states[index].transcript_digest() != transcript_digest
                || states[index].public_share_digest() != shares[index].digest()
                || authentication_secrets[index].party()? != expected
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
        }
        Ok(Self {
            profile,
            roster,
            wire_roster: roster.to_wire_roster()?,
            transcript_digest,
            shares,
            states,
            authentication_secrets,
            collective_key,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_evidence_collective_context(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    collective_key: &ZkAmsMkheCollectivePublicKeyV1,
    shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<(), ZkAmsMkheErrorV1> {
    roster.validate()?;
    let profile = release_profile_v1();
    collective_key.validate(&profile)?;
    if profile_digest != roster.profile_digest()
        || roster_digest != roster.roster_digest()
        || key_material_digest != roster.key_material_digest()
        || epoch != roster.epoch()
        || transcript_digest == [0; 32]
        || collective_key_digest == [0; 32]
        || collective_key.profile_digest() != profile_digest
        || collective_key.roster_digest() != roster_digest
        || collective_key.epoch() != epoch
        || collective_key.transcript_digest() != transcript_digest
        || collective_key.digest() != collective_key_digest
        || collective_key.public_a_wire()? != *shares[0].public_a()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    for (party_index, share) in shares.iter().enumerate() {
        if usize::from(share.party_index()) != party_index
            || share.party() != roster.participants()[party_index].party()
            || share.digest() != collective_key.share_digests_internal()[party_index]
            || share.public_a() != shares[0].public_a()
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
    }
    Ok(())
}

fn expected_rkg_source_record_index(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    left: ZkAmsMkhePartyIdV1,
    right: ZkAmsMkhePartyIdV1,
    digit_index: u32,
    party_index: usize,
    round_two: bool,
) -> Result<u32, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let digit_index =
        usize::try_from(digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    if digit_index >= profile.gadget_digits || party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let left_index = roster
        .participants()
        .iter()
        .position(|participant| participant.party() == left)
        .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
    let right_index = roster
        .participants()
        .iter()
        .position(|participant| participant.party() == right)
        .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
    if left_index > right_index {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    let pair_index = (0..left_index)
        .try_fold(0_usize, |sum, index| {
            sum.checked_add(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 - index)
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        .checked_add(right_index - left_index)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let pair_count = ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        .checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 + 1)
        .and_then(|value| value.checked_div(2))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let records_per_pair = ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let index = digit_index
        .checked_mul(pair_count)
        .and_then(|base| base.checked_add(pair_index))
        .and_then(|pair| pair.checked_mul(records_per_pair))
        .and_then(|base| {
            base.checked_add(if round_two {
                ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            } else {
                0
            })
        })
        .and_then(|base| base.checked_add(party_index))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    u32::try_from(index).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

trait CanonicalBodyWriter {
    fn write_body(&mut self, bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1>;
}

struct CanonicalDigestWriter {
    hash: Keccak256,
    expected_bytes: usize,
    written_bytes: usize,
}

impl CanonicalDigestWriter {
    fn new(expected_bytes: usize) -> Self {
        Self {
            hash: Keccak256::new(),
            expected_bytes,
            written_bytes: 0,
        }
    }

    fn finish(self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if self.written_bytes != self.expected_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(self.hash.finalize())
    }
}

impl CanonicalBodyWriter for CanonicalDigestWriter {
    fn write_body(&mut self, bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
        self.written_bytes = self
            .written_bytes
            .checked_add(bytes.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if self.written_bytes > self.expected_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.hash.update(bytes);
        Ok(())
    }
}

struct CanonicalRecordFanout<'a, S> {
    set_hash: &'a mut Keccak256,
    sink: &'a mut S,
    header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
    record_hash: Keccak256,
    buffer: Box<[u8; ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1]>,
    buffered: usize,
    body_bytes: usize,
    expected_body_bytes: usize,
    chunk_index: u32,
}

impl<'a, S> CanonicalRecordFanout<'a, S>
where
    S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
{
    fn new(
        set_hash: &'a mut Keccak256,
        sink: &'a mut S,
        header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let canonical_bytes = usize::try_from(header.canonical_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let expected_body_bytes = canonical_bytes
            .checked_sub(EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        sink.begin_evidence_record(header)?;
        Ok(Self {
            set_hash,
            sink,
            header,
            record_hash: Keccak256::new(),
            buffer: Box::new([0; ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1]),
            buffered: 0,
            body_bytes: 0,
            expected_body_bytes,
            chunk_index: 0,
        })
    }

    fn flush_body_chunk(&mut self) -> Result<(), ZkAmsMkheErrorV1> {
        if self.buffered == 0 {
            return Ok(());
        }
        let chunk = &self.buffer[..self.buffered];
        self.record_hash.update(chunk);
        self.set_hash.update(chunk);
        self.sink
            .write_evidence_record_chunk(self.header, self.chunk_index, chunk)?;
        self.chunk_index = self
            .chunk_index
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.buffered = 0;
        Ok(())
    }

    fn finish(mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if self.body_bytes != self.expected_body_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.flush_body_chunk()?;
        let digest = self.record_hash.finalize();
        self.set_hash.update(&digest);
        self.sink
            .write_evidence_record_chunk(self.header, self.chunk_index, &digest)?;
        self.chunk_index = self
            .chunk_index
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.sink
            .finish_evidence_record(ZkAmsMkheCollectiveEvidenceRecordFooterV1 {
                header: self.header,
                chunk_count: self.chunk_index,
                canonical_digest: digest,
            })?;
        Ok(digest)
    }
}

impl<S> CanonicalBodyWriter for CanonicalRecordFanout<'_, S>
where
    S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
{
    fn write_body(&mut self, mut bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
        self.body_bytes = self
            .body_bytes
            .checked_add(bytes.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if self.body_bytes > self.expected_body_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        while !bytes.is_empty() {
            let available = ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1 - self.buffered;
            let take = available.min(bytes.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&bytes[..take]);
            self.buffered += take;
            bytes = &bytes[take..];
            if self.buffered == ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1 {
                self.flush_body_chunk()?;
            }
        }
        Ok(())
    }
}

fn write_canonical_bytes(
    writer: &mut impl CanonicalBodyWriter,
    bytes: &[u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    writer.write_body(bytes)
}

fn write_canonical_u8(
    writer: &mut impl CanonicalBodyWriter,
    value: u8,
) -> Result<(), ZkAmsMkheErrorV1> {
    writer.write_body(&[value])
}

fn write_canonical_u32(
    writer: &mut impl CanonicalBodyWriter,
    value: u32,
) -> Result<(), ZkAmsMkheErrorV1> {
    writer.write_body(&value.to_be_bytes())
}

fn write_canonical_u64(
    writer: &mut impl CanonicalBodyWriter,
    value: u64,
) -> Result<(), ZkAmsMkheErrorV1> {
    writer.write_body(&value.to_be_bytes())
}

fn canonical_wire_polynomial_bytes() -> Result<usize, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .and_then(|count| count.checked_mul(core::mem::size_of::<u64>()))
        .and_then(|bytes| bytes.checked_add(4))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn write_canonical_wire_polynomial(
    writer: &mut impl CanonicalBodyWriter,
    polynomial: &ZkAmsMkheRnsPolynomialWireV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    polynomial.encoded_len()?;
    write_canonical_u32(
        writer,
        u32::try_from(polynomial.residues().len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )?;
    const RESIDUES_PER_BATCH: usize = 1024;
    let mut bytes = [0_u8; RESIDUES_PER_BATCH * core::mem::size_of::<u64>()];
    for residues in polynomial.residues().chunks(RESIDUES_PER_BATCH) {
        for (destination, residue) in bytes.chunks_exact_mut(8).zip(residues) {
            destination.copy_from_slice(&residue.to_be_bytes());
        }
        writer.write_body(&bytes[..residues.len() * 8])?;
    }
    Ok(())
}

struct CanonicalBodyReader<'a, R> {
    reader: &'a mut R,
    hash: Keccak256,
    remaining: u64,
}

impl<'a, R> CanonicalBodyReader<'a, R>
where
    R: std::io::Read,
{
    fn new(reader: &'a mut R, prefix: &[u8], remaining: u64) -> Self {
        let mut hash = Keccak256::new();
        hash.update(prefix);
        Self {
            reader,
            hash,
            remaining,
        }
    }

    fn remaining(&self) -> u64 {
        self.remaining
    }

    fn finish(self) -> Result<(&'a mut R, [u8; 32]), ZkAmsMkheErrorV1> {
        if self.remaining != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok((self.reader, self.hash.finalize()))
    }
}

impl<R> std::io::Read for CanonicalBodyReader<'_, R>
where
    R: std::io::Read,
{
    fn read(&mut self, destination: &mut [u8]) -> std::io::Result<usize> {
        if destination.is_empty() || self.remaining == 0 {
            return Ok(0);
        }
        let limit = usize::try_from(self.remaining)
            .unwrap_or(usize::MAX)
            .min(destination.len());
        let read = self.reader.read(&mut destination[..limit])?;
        if read != 0 {
            self.hash.update(&destination[..read]);
            self.remaining -= read as u64;
        }
        Ok(read)
    }
}

fn canonical_polynomial_residue_count() -> Result<usize, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn maximum_source_evidence_record_bytes() -> Result<usize, ZkAmsMkheErrorV1> {
    SOURCE_EVIDENCE_COMMON_BODY_BYTES_V1
        .checked_add(32 + 32 + 4)
        .and_then(|value| {
            canonical_wire_polynomial_bytes()
                .ok()?
                .checked_mul(8)?
                .checked_add(value)
        })
        .and_then(|value| value.checked_add(8))
        .and_then(|value| value.checked_add(super::ZK_AMS_MKHE_MAX_PROOF_BYTES_V1))
        .and_then(|value| value.checked_add(4_096))
        .and_then(|value| value.checked_add(EVIDENCE_RECORD_DIGEST_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn maximum_cks_contribution_record_bytes() -> Result<usize, ZkAmsMkheErrorV1> {
    canonical_wire_polynomial_bytes()?
        .checked_add(super::ZK_AMS_MKHE_MAX_PROOF_BYTES_V1)
        .and_then(|value| value.checked_add(4_096))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn maximum_cks_evidence_record_bytes() -> Result<usize, ZkAmsMkheErrorV1> {
    let polynomial_bytes = canonical_wire_polynomial_bytes()?;
    let roster_bytes = 4_096;
    CKS_EVIDENCE_COMMON_BODY_BYTES_V1
        .checked_add(4 + roster_bytes)
        .and_then(|value| value.checked_add(32 + 4 + 8 + 1 + 32))
        .and_then(|value| polynomial_bytes.checked_mul(20)?.checked_add(value))
        .and_then(|value| value.checked_add(1 + ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 * 33 + 1 + 1))
        .and_then(|value| {
            maximum_cks_contribution_record_bytes()
                .ok()?
                .checked_add(8)?
                .checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)?
                .checked_add(value)
        })
        .and_then(|value| value.checked_add(EVIDENCE_RECORD_DIGEST_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn read_canonical_raw_exact(
    reader: &mut impl std::io::Read,
    bytes: &mut [u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    reader
        .read_exact(bytes)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
}

fn read_canonical_array<const N: usize>(
    reader: &mut impl std::io::Read,
) -> Result<[u8; N], ZkAmsMkheErrorV1> {
    let mut bytes = [0_u8; N];
    read_canonical_raw_exact(reader, &mut bytes)?;
    Ok(bytes)
}

fn read_canonical_u8(reader: &mut impl std::io::Read) -> Result<u8, ZkAmsMkheErrorV1> {
    Ok(read_canonical_array::<1>(reader)?[0])
}

fn read_canonical_u32(reader: &mut impl std::io::Read) -> Result<u32, ZkAmsMkheErrorV1> {
    Ok(u32::from_be_bytes(read_canonical_array(reader)?))
}

fn read_canonical_u64(reader: &mut impl std::io::Read) -> Result<u64, ZkAmsMkheErrorV1> {
    Ok(u64::from_be_bytes(read_canonical_array(reader)?))
}

fn read_canonical_party(
    reader: &mut impl std::io::Read,
) -> Result<ZkAmsMkhePartyIdV1, ZkAmsMkheErrorV1> {
    ZkAmsMkhePartyIdV1::new(read_canonical_array(reader)?)
}

fn read_canonical_wire_polynomial(
    reader: &mut impl std::io::Read,
) -> Result<ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheErrorV1> {
    let count = usize::try_from(read_canonical_u32(reader)?)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let expected_count = canonical_polynomial_residue_count()?;
    if count != expected_count {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut residues = Vec::new();
    residues
        .try_reserve_exact(expected_count)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    const RESIDUES_PER_BATCH: usize = 1024;
    let mut bytes = [0_u8; RESIDUES_PER_BATCH * core::mem::size_of::<u64>()];
    let mut remaining = expected_count;
    while remaining != 0 {
        let take = remaining.min(RESIDUES_PER_BATCH);
        read_canonical_raw_exact(reader, &mut bytes[..take * 8])?;
        for encoded in bytes[..take * 8].chunks_exact(8) {
            residues.push(u64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            ));
        }
        remaining -= take;
    }
    ZkAmsMkheRnsPolynomialWireV1::new(residues)
}

fn read_canonical_vec_exact(
    reader: &mut impl std::io::Read,
    length: usize,
    ceiling: usize,
) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
    if length == 0 || length > ceiling {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    bytes.resize(length, 0);
    read_canonical_raw_exact(reader, &mut bytes)?;
    Ok(bytes)
}

fn finish_canonical_body<R: std::io::Read>(
    body: CanonicalBodyReader<'_, R>,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let (reader, observed) = body.finish()?;
    let expected = read_canonical_array(reader)?;
    if observed != expected {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(observed)
}

fn require_canonical_reader_eof(reader: &mut impl std::io::Read) -> Result<(), ZkAmsMkheErrorV1> {
    let mut trailing = [0_u8; 1];
    loop {
        match reader.read(&mut trailing) {
            Ok(0) => return Ok(()),
            Ok(_) => return Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }
}

fn decode_source_evidence_record<R: std::io::Read>(
    reader: &mut R,
) -> Result<ZkAmsMkheOwnedCollectiveSourceProofEvidenceV1, ZkAmsMkheErrorV1> {
    const PREFIX_BYTES: usize = 4 + 1 + 1 + 8;
    let mut prefix = [0_u8; PREFIX_BYTES];
    read_canonical_raw_exact(reader, &mut prefix)?;
    if prefix[..4] != SOURCE_EVIDENCE_RECORD_TAG_V1 || prefix[4] != MKHE_VERSION_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let kind = ZkAmsMkheCollectiveEvidenceRecordKindV1::decode(prefix[5])?;
    if kind == ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let canonical_bytes = u64::from_be_bytes(
        prefix[6..14]
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    );
    let maximum = u64::try_from(maximum_source_evidence_record_bytes()?)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let minimum =
        u64::try_from(SOURCE_EVIDENCE_COMMON_BODY_BYTES_V1 + EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if canonical_bytes < minimum || canonical_bytes > maximum {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    let body_bytes = canonical_bytes
        .checked_sub(EVIDENCE_RECORD_DIGEST_BYTES_V1 as u64)
        .and_then(|value| value.checked_sub(PREFIX_BYTES as u64))
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let mut body = CanonicalBodyReader::new(reader, &prefix, body_bytes);
    let ordinal = read_canonical_u8(&mut body)?;
    let source_record_index = read_canonical_u32(&mut body)?;
    let party_index = read_canonical_u8(&mut body)?;
    let profile_digest = read_canonical_array(&mut body)?;
    let roster_digest = read_canonical_array(&mut body)?;
    let key_material_digest = read_canonical_array(&mut body)?;
    let epoch = read_canonical_u64(&mut body)?;
    let transcript_digest = read_canonical_array(&mut body)?;
    let collective_key_digest = read_canonical_array(&mut body)?;
    let statement = match kind {
        ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundOne => {
            let left = read_canonical_party(&mut body)?;
            let right = read_canonical_party(&mut body)?;
            let digit_index = read_canonical_u32(&mut body)?;
            ZkAmsMkheOwnedCollectiveSourceStatementEvidenceV1::RkgRoundOne {
                public_a: read_canonical_wire_polynomial(&mut body)?,
                party_public_b: read_canonical_wire_polynomial(&mut body)?,
                common_a: read_canonical_wire_polynomial(&mut body)?,
                h0: read_canonical_wire_polynomial(&mut body)?,
                h1: read_canonical_wire_polynomial(&mut body)?,
                left,
                right,
                digit_index,
            }
        }
        ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundTwo => {
            let left = read_canonical_party(&mut body)?;
            let right = read_canonical_party(&mut body)?;
            let digit_index = read_canonical_u32(&mut body)?;
            ZkAmsMkheOwnedCollectiveSourceStatementEvidenceV1::RkgRoundTwo {
                public_a: read_canonical_wire_polynomial(&mut body)?,
                party_public_b: read_canonical_wire_polynomial(&mut body)?,
                common_a: read_canonical_wire_polynomial(&mut body)?,
                h0: read_canonical_wire_polynomial(&mut body)?,
                h1: read_canonical_wire_polynomial(&mut body)?,
                aggregate_h0: read_canonical_wire_polynomial(&mut body)?,
                aggregate_h1: read_canonical_wire_polynomial(&mut body)?,
                k0: read_canonical_wire_polynomial(&mut body)?,
                left,
                right,
                digit_index,
            }
        }
        ZkAmsMkheCollectiveEvidenceRecordKindV1::GaloisSource => {
            let schedule_index = read_canonical_u8(&mut body)?;
            let exponent = read_canonical_u32(&mut body)?;
            let digit_index = read_canonical_u32(&mut body)?;
            ZkAmsMkheOwnedCollectiveSourceStatementEvidenceV1::Galois {
                public_a: read_canonical_wire_polynomial(&mut body)?,
                party_public_b: read_canonical_wire_polynomial(&mut body)?,
                source_constant: read_canonical_wire_polynomial(&mut body)?,
                source_linear: read_canonical_wire_polynomial(&mut body)?,
                schedule_index,
                exponent,
                digit_index,
            }
        }
        ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit => unreachable!(),
    };
    let proof_bytes = read_canonical_u64(&mut body)?;
    if proof_bytes == 0 || proof_bytes > body.remaining() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let proof = ZkAmsMkheActiveRkgProofV1::decode_evidence_from_reader(&mut body, proof_bytes)?;
    let canonical_digest = finish_canonical_body(body)?;
    Ok(ZkAmsMkheOwnedCollectiveSourceProofEvidenceV1 {
        ordinal,
        source_record_index,
        party_index,
        profile_digest,
        roster_digest,
        key_material_digest,
        epoch,
        transcript_digest,
        collective_key_digest,
        statement,
        proof,
        canonical_bytes,
        canonical_digest,
    })
}

fn decode_cks_evidence_record<R: std::io::Read>(
    reader: &mut R,
    active_roster: &ZkAmsMkheGovernedActiveRosterV1,
) -> Result<ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1, ZkAmsMkheErrorV1> {
    const PREFIX_BYTES: usize = 4 + 1 + 8;
    let mut prefix = [0_u8; PREFIX_BYTES];
    read_canonical_raw_exact(reader, &mut prefix)?;
    if prefix[..4] != CKS_EVIDENCE_RECORD_TAG_V1 || prefix[4] != MKHE_VERSION_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let canonical_bytes = u64::from_be_bytes(
        prefix[5..13]
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    );
    let maximum = u64::try_from(maximum_cks_evidence_record_bytes()?)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let minimum =
        u64::try_from(CKS_EVIDENCE_COMMON_BODY_BYTES_V1 + EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if canonical_bytes < minimum || canonical_bytes > maximum {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    let body_bytes = canonical_bytes
        .checked_sub(EVIDENCE_RECORD_DIGEST_BYTES_V1 as u64)
        .and_then(|value| value.checked_sub(PREFIX_BYTES as u64))
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let mut body = CanonicalBodyReader::new(reader, &prefix, body_bytes);
    let ordinal = read_canonical_u8(&mut body)?;
    let digit_index = read_canonical_u8(&mut body)?;
    let collective_key_digest = read_canonical_array(&mut body)?;
    let trusted_roster = active_roster.to_wire_roster()?;
    let trusted_roster_bytes = trusted_roster.encode()?;
    let roster_bytes = usize::try_from(read_canonical_u32(&mut body)?)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if roster_bytes != trusted_roster_bytes.len() || roster_bytes > 4_096 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let encoded_roster = read_canonical_vec_exact(&mut body, roster_bytes, 4_096)?;
    let roster = ZkAmsMkheGovernedRosterWireV1::decode_exact(
        &encoded_roster,
        trusted_roster.profile_digest(),
        trusted_roster.epoch(),
    )?;
    if roster != trusted_roster {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    let transcript_digest = read_canonical_array(&mut body)?;
    let source_record_index = read_canonical_u32(&mut body)?;
    let sample_index = read_canonical_u64(&mut body)?;
    let level = read_canonical_u8(&mut body)?;
    let encoded_source_digest = read_canonical_array(&mut body)?;
    let source_constant = read_canonical_wire_polynomial(&mut body)?;
    let component_count = usize::from(read_canonical_u8(&mut body)?);
    if component_count != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    let mut components = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
    for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let party = read_canonical_party(&mut body)?;
        if party != roster.parties()[party_index] {
            return Err(ZkAmsMkheErrorV1::InvalidPartySet);
        }
        match read_canonical_u8(&mut body)? {
            0 => {}
            1 => components.push((party, read_canonical_wire_polynomial(&mut body)?)),
            _ => return Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }
    let source = ZkAmsMkheCksSourceCiphertextV1::new(
        &roster,
        transcript_digest,
        source_record_index,
        sample_index,
        level,
        source_constant,
        components,
    )?;
    if source.source_digest() != encoded_source_digest {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let target_a = read_canonical_wire_polynomial(&mut body)?;
    let public_key_a = read_canonical_wire_polynomial(&mut body)?;
    if usize::from(read_canonical_u8(&mut body)?) != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    let mut party_public_b = Vec::new();
    party_public_b
        .try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for _ in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        party_public_b.push(read_canonical_wire_polynomial(&mut body)?);
    }
    let party_public_b: [ZkAmsMkheRnsPolynomialWireV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
        party_public_b
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
    let compact_constant = read_canonical_wire_polynomial(&mut body)?;
    if usize::from(read_canonical_u8(&mut body)?) != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidCksSet);
    }
    let party_public_b_refs = std::array::from_fn(|index| &party_public_b[index]);
    let statement = ZkAmsMkheCksStatementV1::new(
        &roster,
        &source,
        &target_a,
        &public_key_a,
        &party_public_b_refs,
    )?;
    let contribution_ceiling = maximum_cks_contribution_record_bytes()?;
    let mut contributions = Vec::new();
    contributions
        .try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let bytes = usize::try_from(read_canonical_u64(&mut body)?)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        if u64::try_from(bytes)
            .ok()
            .is_none_or(|bytes| bytes > body.remaining())
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let encoded = read_canonical_vec_exact(&mut body, bytes, contribution_ceiling)?;
        contributions.push(
            ZkAmsMkheAuthenticatedCksContributionV1::decode_release_wire_exact(
                statement,
                u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
                &encoded,
            )?,
        );
    }
    let canonical_digest = finish_canonical_body(body)?;
    Ok(ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1 {
        ordinal,
        digit_index,
        collective_key_digest,
        roster,
        source,
        target_a,
        public_key_a,
        party_public_b,
        contributions,
        compact_constant,
        canonical_bytes,
        canonical_digest,
    })
}

struct EvidenceHasher {
    hash: Keccak256,
    records: u32,
    header: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
}

impl EvidenceHasher {
    fn new<S>(
        purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
        ordinal: u8,
        exponent: u32,
        collective_key_digest: [u8; 32],
        kind: ZkAmsMkheCollectiveEvidenceSetKindV1,
        sink: &mut S,
    ) -> Result<Self, ZkAmsMkheErrorV1>
    where
        S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
    {
        if collective_key_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let header = ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
            kind,
            purpose,
            ordinal,
            galois_exponent: exponent,
            collective_key_digest,
        };
        sink.begin_evidence_set(header)?;
        let mut hash = Keccak256::new();
        hash.update(EVALUATED_KEY_EVIDENCE_DOMAIN_V1);
        let evidence_kind: &[u8] = match kind {
            ZkAmsMkheCollectiveEvidenceSetKindV1::Source => b"source",
            ZkAmsMkheCollectiveEvidenceSetKindV1::Cks => b"cks",
        };
        hash.update(
            &u8::try_from(evidence_kind.len())
                .expect("fixed evidence kind length fits in one byte")
                .to_be_bytes(),
        );
        hash.update(evidence_kind);
        hash.update(&[MKHE_VERSION_V1, purpose as u8, ordinal]);
        hash.update(&exponent.to_be_bytes());
        hash.update(&collective_key_digest);
        Ok(Self {
            hash,
            records: 0,
            header,
        })
    }

    fn source<S>(
        &mut self,
        evidence: &ZkAmsMkheCollectiveSourceProofEvidenceV1<'_>,
        sink: &mut S,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
    {
        if self.header.kind != ZkAmsMkheCollectiveEvidenceSetKindV1::Source
            || evidence.ordinal() != self.header.ordinal
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.expect_next(evidence.source_record_index())?;
        let canonical_bytes = evidence.canonical_encoded_len()?;
        let header = ZkAmsMkheCollectiveEvidenceRecordHeaderV1 {
            set: self.header,
            kind: evidence.record_kind(),
            record_index: evidence.source_record_index(),
            canonical_bytes: u64::try_from(canonical_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        };
        let mut writer = CanonicalRecordFanout::new(&mut self.hash, sink, header)?;
        evidence.write_canonical_body(&mut writer, canonical_bytes)?;
        writer.finish()?;
        self.advance()
    }

    fn cks<S>(
        &mut self,
        evidence: &ZkAmsMkheCollectiveCksDigitEvidenceV1<'_>,
        sink: &mut S,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
    {
        if self.header.kind != ZkAmsMkheCollectiveEvidenceSetKindV1::Cks
            || evidence.ordinal() != self.header.ordinal
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.expect_next(u32::from(evidence.digit_index()))?;
        let canonical_bytes = evidence.canonical_encoded_len()?;
        let header = ZkAmsMkheCollectiveEvidenceRecordHeaderV1 {
            set: self.header,
            kind: ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit,
            record_index: u32::from(evidence.digit_index()),
            canonical_bytes: u64::try_from(canonical_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        };
        let mut writer = CanonicalRecordFanout::new(&mut self.hash, sink, header)?;
        evidence.write_canonical_body(&mut writer, canonical_bytes)?;
        writer.finish()?;
        self.advance()
    }

    fn expect_next(&self, record_index: u32) -> Result<(), ZkAmsMkheErrorV1> {
        if record_index != self.records {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    fn advance(&mut self) -> Result<(), ZkAmsMkheErrorV1> {
        self.records = self
            .records
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(())
    }

    #[cfg(test)]
    fn test_record(
        &mut self,
        record_index: u32,
        canonical_bytes: &[u8],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.expect_next(record_index)?;
        self.hash.update(b"test-canonical-record");
        self.hash.update(
            &u64::try_from(canonical_bytes.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        self.hash.update(canonical_bytes);
        self.advance()
    }

    fn finish<S>(
        mut self,
        expected_records: u32,
        sink: &mut S,
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1>
    where
        S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
    {
        if expected_records == 0 || self.records != expected_records {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.hash.update(&self.records.to_be_bytes());
        let digest = self.hash.finalize();
        sink.finish_evidence_set(ZkAmsMkheCollectiveEvidenceSetFooterV1 {
            header: self.header,
            record_count: self.records,
            canonical_digest: digest,
        })?;
        Ok(digest)
    }
}

fn validated_source_evidence<'a>(
    context: &CeremonyContext<'_>,
    ordinal: u8,
    source_record_index: u32,
    party_index: usize,
    statement: ZkAmsMkheCollectiveSourceStatementEvidenceV1<'a>,
    proof: &'a ZkAmsMkheActiveRkgProofV1,
) -> Result<ZkAmsMkheCollectiveSourceProofEvidenceV1<'a>, ZkAmsMkheErrorV1> {
    let evidence = ZkAmsMkheCollectiveSourceProofEvidenceV1 {
        ordinal,
        source_record_index,
        party_index: u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
        profile_digest: context.roster.profile_digest(),
        roster_digest: context.roster.roster_digest(),
        key_material_digest: context.roster.key_material_digest(),
        epoch: context.roster.epoch(),
        transcript_digest: context.transcript_digest,
        collective_key_digest: context.collective_key.digest(),
        statement,
        proof,
    };
    evidence.verify(context.roster, &context.collective_key, context.shares)?;
    Ok(evidence)
}

fn evaluated_key_evidence_digest(
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    exponent: u32,
    collective_key_digest: [u8; 32],
    source_proof_set_digest: [u8; 32],
    cks_proof_set_digest: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if collective_key_digest == [0; 32]
        || source_proof_set_digest == [0; 32]
        || cks_proof_set_digest == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut frame = Vec::with_capacity(160);
    frame.extend_from_slice(EVALUATED_KEY_EVIDENCE_DOMAIN_V1);
    frame.extend_from_slice(&[MKHE_VERSION_V1, purpose as u8, ordinal]);
    frame.extend_from_slice(&exponent.to_be_bytes());
    frame.extend_from_slice(&collective_key_digest);
    frame.extend_from_slice(&source_proof_set_digest);
    frame.extend_from_slice(&cks_proof_set_digest);
    Ok(keccak256(&frame))
}

fn derive_target_a(
    profile: &BgvProfile,
    roster: &ZkAmsMkheGovernedRosterWireV1,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    exponent: u32,
    master_seed: [u8; 32],
    digit_index: usize,
) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
    if transcript_digest == [0; 32]
        || collective_key_digest == [0; 32]
        || master_seed == [0; 32]
        || digit_index >= profile.gadget_digits
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut context = Vec::with_capacity(192);
    context.extend_from_slice(&roster.profile_digest());
    context.extend_from_slice(&roster.roster_digest());
    context.extend_from_slice(&roster.epoch().to_be_bytes());
    context.extend_from_slice(&transcript_digest);
    context.extend_from_slice(&collective_key_digest);
    context.extend_from_slice(&[purpose as u8, ordinal]);
    context.extend_from_slice(&exponent.to_be_bytes());
    context.extend_from_slice(&master_seed);
    context.extend_from_slice(
        &u16::try_from(digit_index)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .to_be_bytes(),
    );
    derive_uniform_rns_from_context(profile, EVALUATED_KEY_TARGET_A_DOMAIN_V1, &context)
}

fn with_cks_statement<T>(
    context: &CeremonyContext<'_>,
    source: &ZkAmsMkheCksSourceCiphertextV1,
    target_a: &ZkAmsMkheRnsPolynomialWireV1,
    operation: impl FnOnce(ZkAmsMkheCksStatementV1<'_>) -> Result<T, ZkAmsMkheErrorV1>,
) -> Result<T, ZkAmsMkheErrorV1> {
    let public_a = context.shares[0].public_a();
    if context
        .shares
        .iter()
        .any(|share| share.public_a() != public_a)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let party_public_b = std::array::from_fn(|index| context.shares[index].party_public_b());
    let statement = ZkAmsMkheCksStatementV1::new(
        &context.wire_roster,
        source,
        target_a,
        public_a,
        &party_public_b,
    )?;
    operation(statement)
}

#[allow(clippy::too_many_arguments)]
fn compact_source_digit<R, S>(
    context: &CeremonyContext<'_>,
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    exponent: u32,
    master_seed: [u8; 32],
    digit_index: usize,
    source_constant: RnsPolynomial,
    source_components: [RnsPolynomial; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    cks_evidence: &mut EvidenceHasher,
    random: &mut R,
    sink: &mut S,
) -> Result<ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheErrorV1>
where
    R: MaskedRelaxedRandomSourceV1,
    S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
{
    source_constant.validate(&context.profile)?;
    for component in &source_components {
        component.validate(&context.profile)?;
    }
    let record_index = usize::from(ordinal)
        .checked_mul(context.profile.gadget_digits)
        .and_then(|base| base.checked_add(digit_index))
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let source = ZkAmsMkheCksSourceCiphertextV1::new(
        &context.wire_roster,
        context.transcript_digest,
        record_index,
        u64::from(record_index),
        0,
        ZkAmsMkheRnsPolynomialWireV1::new(source_constant.coefficients)?,
        context
            .wire_roster
            .parties()
            .iter()
            .copied()
            .zip(source_components)
            .map(|(party, polynomial)| {
                Ok((
                    party,
                    ZkAmsMkheRnsPolynomialWireV1::new(polynomial.coefficients)?,
                ))
            })
            .collect::<Result<Vec<_>, ZkAmsMkheErrorV1>>()?,
    )?;
    let target_a = derive_target_a(
        &context.profile,
        &context.wire_roster,
        context.transcript_digest,
        context.collective_key.digest(),
        purpose,
        ordinal,
        exponent,
        master_seed,
        digit_index,
    )?;
    let target_a_wire = ZkAmsMkheRnsPolynomialWireV1::new(target_a.coefficients)?;
    with_cks_statement(context, &source, &target_a_wire, |statement| {
        let mut contributions = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            let contribution = prove_zk_ams_mkhe_cks_contribution_v1(
                statement,
                party_index,
                context.states[party_index],
                context.authentication_secrets[party_index],
                random,
            )?;
            contributions.push(contribution);
        }
        let compact = combine_zk_ams_mkhe_cks_v1(statement, &contributions)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCksSet)?;
        if compact.linear() != &target_a_wire {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let evidence = ZkAmsMkheCollectiveCksDigitEvidenceV1 {
            ordinal,
            digit_index: u8::try_from(digit_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
            collective_key_digest: context.collective_key.digest(),
            roster: &context.wire_roster,
            source: &source,
            target_a: &target_a_wire,
            public_key_a: statement.public_key_a(),
            party_public_b: *statement.party_public_b(),
            contributions: &contributions,
            compact_constant: compact.constant(),
        };
        evidence.verify(context.roster, &context.collective_key, context.shares)?;
        cks_evidence.cks(&evidence, sink)?;
        Ok(compact.constant().clone())
    })
}

fn finish_generated_key(
    context: &CeremonyContext<'_>,
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    exponent: u32,
    master_seed: [u8; 32],
    source_proof_set_digest: [u8; 32],
    cks_proof_set_digest: [u8; 32],
    stored_b_digits: Vec<ZkAmsMkheRnsPolynomialWireV1>,
) -> Result<ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1, ZkAmsMkheErrorV1> {
    if stored_b_digits.len() != context.profile.gadget_digits {
        return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
    }
    let binding = ZkAmsMkheWireBindingV1::new(
        &context.wire_roster,
        context.transcript_digest,
        u32::from(ordinal),
        0,
    )?;
    let contribution_proof_digest = evaluated_key_evidence_digest(
        purpose,
        ordinal,
        exponent,
        context.collective_key.digest(),
        source_proof_set_digest,
        cks_proof_set_digest,
    )?;
    let wire = ZkAmsMkheSeededRkgKeyWireV1::new(
        binding,
        master_seed,
        contribution_proof_digest,
        stored_b_digits,
    )?;
    let payload = wire.encode()?;
    let payload_bytes =
        u64::try_from(payload.len()).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    Ok(ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1 {
        purpose,
        ordinal,
        galois_exponent: exponent,
        collective_key_digest: context.collective_key.digest(),
        source_proof_set_digest,
        cks_proof_set_digest,
        payload_blake3: blake3_hash(&payload),
        payload_bytes,
        wire,
    })
}

fn sample_nonzero_ternary<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    random: &mut R,
) -> Result<SecretPolynomial, ZkAmsMkheErrorV1> {
    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let candidate = SecretPolynomial::sample_ternary(profile, random)?;
        if candidate
            .coefficients
            .iter()
            .any(|coefficient| *coefficient != 0)
        {
            return Ok(candidate);
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

fn scaled_error(
    profile: &BgvProfile,
    error: &SecretPolynomial,
) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
    error.as_rns(profile)?.scale_plaintext_modulus(profile)
}

fn add_weighted_pair_source(
    profile: &BgvProfile,
    diagonal: bool,
    source_constant: &mut RnsPolynomial,
    source_linear: &mut RnsPolynomial,
    pair_constant: &RnsPolynomial,
    pair_linear: &RnsPolynomial,
) -> Result<(), ZkAmsMkheErrorV1> {
    let weighted_constant = if diagonal {
        pair_constant.clone()
    } else {
        pair_constant.add(pair_constant, profile)?
    };
    let weighted_linear = if diagonal {
        pair_linear.clone()
    } else {
        pair_linear.add(pair_linear, profile)?
    };
    *source_constant = source_constant.add(&weighted_constant, profile)?;
    *source_linear = source_linear.add(&weighted_linear, profile)?;
    Ok(())
}

/// Generate the exact 38-digit compact collective relinearization key.
///
/// Every digit first aggregates all 36 canonical unordered pair products.
/// Diagonal terms have weight one and all 28 off-diagonal terms have weight
/// two, so the source decrypts to exactly `g^d (sum_i s_i)^2`.  The complete
/// source is then compacted by eight real proof-carrying CKS contributions.
#[allow(clippy::too_many_arguments)]
pub fn generate_zk_ams_mkhe_collective_relinearization_key_v1<R, S>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    states: [&ZkAmsMkheCollectivePartyStateV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    authentication_secrets: [&ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    master_seed: [u8; 32],
    random: &mut R,
    sink: &mut S,
) -> Result<ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1, ZkAmsMkheErrorV1>
where
    R: MaskedRelaxedRandomSourceV1,
    S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
{
    let context = CeremonyContext::new(
        roster,
        transcript_digest,
        shares,
        states,
        authentication_secrets,
    )?;
    if master_seed == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let purpose = ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization;
    let ordinal = 0_u8;
    let exponent = 0_u32;
    let mut source_evidence = EvidenceHasher::new(
        purpose,
        ordinal,
        exponent,
        context.collective_key.digest(),
        ZkAmsMkheCollectiveEvidenceSetKindV1::Source,
        sink,
    )?;
    let mut cks_evidence = EvidenceHasher::new(
        purpose,
        ordinal,
        exponent,
        context.collective_key.digest(),
        ZkAmsMkheCollectiveEvidenceSetKindV1::Cks,
        sink,
    )?;
    let parties = PartySet::new(context.wire_roster.parties().to_vec())?;
    let pair_count = ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        .checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 + 1)
        .and_then(|value| value.checked_div(2))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut stored_b_digits = Vec::with_capacity(context.profile.gadget_digits);
    for digit_index in 0..context.profile.gadget_digits {
        let mut source_constant = RnsPolynomial::zero(&context.profile);
        let mut source_linear = RnsPolynomial::zero(&context.profile);
        let mut pair_index = 0_usize;
        for left_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            for right_index in left_index..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
                let left = context.wire_roster.parties()[left_index];
                let right = context.wire_roster.parties()[right_index];
                let common_a = derive_rkg_common_a(
                    &context.profile,
                    &parties,
                    transcript_digest,
                    left,
                    right,
                    digit_index,
                )?;
                let common_a_wire =
                    ZkAmsMkheRnsPolynomialWireV1::new(common_a.coefficients.clone())?;
                let mut ephemerals = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
                let mut error_zeros = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
                let mut error_ones = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
                let mut h0_values = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
                let mut h1_values = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
                for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
                    let ephemeral = sample_nonzero_ternary(&context.profile, random)?;
                    let error_zero = SecretPolynomial::sample_error(&context.profile, random)?;
                    let error_one = SecretPolynomial::sample_error(&context.profile, random)?;
                    let secret_rns = context.states[party_index]
                        .secret()
                        .as_rns(&context.profile)?;
                    let ephemeral_rns = ephemeral.as_rns(&context.profile)?;
                    let mut h0 = common_a
                        .mul(&ephemeral_rns, &context.profile)?
                        .negate(&context.profile)?;
                    if party_index == left_index {
                        h0 = h0.add(
                            &secret_rns.scale_gadget(digit_index, &context.profile)?,
                            &context.profile,
                        )?;
                    }
                    h0 = h0.add(
                        &scaled_error(&context.profile, &error_zero)?,
                        &context.profile,
                    )?;
                    let mut h1 = scaled_error(&context.profile, &error_one)?;
                    if party_index == right_index {
                        h1 = h1.add(
                            &common_a.mul(&secret_rns, &context.profile)?,
                            &context.profile,
                        )?;
                    }
                    let h0_wire = ZkAmsMkheRnsPolynomialWireV1::new(h0.coefficients.clone())?;
                    let h1_wire = ZkAmsMkheRnsPolynomialWireV1::new(h1.coefficients.clone())?;
                    let public_key = ZkAmsMkheActiveCollectivePublicKeyStatementV1::new(
                        context.shares[party_index].public_a(),
                        context.shares[party_index].party_public_b(),
                    )?;
                    let statement = ZkAmsMkheActiveRkgRoundOneStatementV1::new(
                        public_key,
                        &common_a_wire,
                        &h0_wire,
                        &h1_wire,
                        left,
                        right,
                        u32::try_from(digit_index)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                    )?;
                    let witness = ZkAmsMkheActiveRkgRoundOneWitnessV1::new(
                        &context.states[party_index].secret().coefficients,
                        &context.states[party_index].public_error().coefficients,
                        &ephemeral.coefficients,
                        &error_zero.coefficients,
                        &error_one.coefficients,
                    )?;
                    let proof = prove_zk_ams_mkhe_active_rkg_round_one_v1(
                        roster,
                        transcript_digest,
                        party_index,
                        statement,
                        witness,
                        context.authentication_secrets[party_index],
                        random,
                    )?;
                    let record_index = expected_rkg_source_record_index(
                        roster,
                        left,
                        right,
                        u32::try_from(digit_index)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                        party_index,
                        false,
                    )?;
                    let evidence = validated_source_evidence(
                        &context,
                        ordinal,
                        record_index,
                        party_index,
                        ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundOne {
                            public_a: context.shares[party_index].public_a(),
                            party_public_b: context.shares[party_index].party_public_b(),
                            common_a: &common_a_wire,
                            h0: &h0_wire,
                            h1: &h1_wire,
                            left,
                            right,
                            digit_index: u32::try_from(digit_index)
                                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                        },
                        &proof,
                    )?;
                    source_evidence.source(&evidence, sink)?;
                    ephemerals.push(ephemeral);
                    error_zeros.push(error_zero);
                    error_ones.push(error_one);
                    h0_values.push(h0);
                    h1_values.push(h1);
                }
                checked_coefficient_work(&context.profile, 2 * ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)?;
                let mut aggregate_h0 = RnsPolynomial::zero(&context.profile);
                let mut aggregate_h1 = RnsPolynomial::zero(&context.profile);
                for (h0, h1) in h0_values.iter().zip(&h1_values) {
                    aggregate_h0 = aggregate_h0.add(h0, &context.profile)?;
                    aggregate_h1 = aggregate_h1.add(h1, &context.profile)?;
                }
                let aggregate_h0_wire =
                    ZkAmsMkheRnsPolynomialWireV1::new(aggregate_h0.coefficients.clone())?;
                let aggregate_h1_wire =
                    ZkAmsMkheRnsPolynomialWireV1::new(aggregate_h1.coefficients.clone())?;
                let mut pair_constant = RnsPolynomial::zero(&context.profile);
                for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
                    let error_two = SecretPolynomial::sample_error(&context.profile, random)?;
                    let secret_rns = context.states[party_index]
                        .secret()
                        .as_rns(&context.profile)?;
                    let right_secret = if party_index == right_index {
                        secret_rns
                    } else {
                        RnsPolynomial::zero(&context.profile)
                    };
                    let difference = ephemerals[party_index]
                        .sub(context.states[party_index].secret())?
                        .as_rns(&context.profile)?;
                    let k0 = aggregate_h0
                        .mul(&right_secret, &context.profile)?
                        .add(
                            &aggregate_h1.mul(&difference, &context.profile)?,
                            &context.profile,
                        )?
                        .add(
                            &scaled_error(&context.profile, &error_two)?,
                            &context.profile,
                        )?;
                    let k0_wire = ZkAmsMkheRnsPolynomialWireV1::new(k0.coefficients.clone())?;
                    let public_key = ZkAmsMkheActiveCollectivePublicKeyStatementV1::new(
                        context.shares[party_index].public_a(),
                        context.shares[party_index].party_public_b(),
                    )?;
                    let party_h0_wire = ZkAmsMkheRnsPolynomialWireV1::new(
                        h0_values[party_index].coefficients.clone(),
                    )?;
                    let party_h1_wire = ZkAmsMkheRnsPolynomialWireV1::new(
                        h1_values[party_index].coefficients.clone(),
                    )?;
                    let round_one_statement = ZkAmsMkheActiveRkgRoundOneStatementV1::new(
                        public_key,
                        &common_a_wire,
                        &party_h0_wire,
                        &party_h1_wire,
                        left,
                        right,
                        u32::try_from(digit_index)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                    )?;
                    let round_one_witness = ZkAmsMkheActiveRkgRoundOneWitnessV1::new(
                        &context.states[party_index].secret().coefficients,
                        &context.states[party_index].public_error().coefficients,
                        &ephemerals[party_index].coefficients,
                        &error_zeros[party_index].coefficients,
                        &error_ones[party_index].coefficients,
                    )?;
                    let statement = ZkAmsMkheActiveRkgRoundTwoStatementV1::new(
                        round_one_statement,
                        &aggregate_h0_wire,
                        &aggregate_h1_wire,
                        &k0_wire,
                    )?;
                    let witness = ZkAmsMkheActiveRkgRoundTwoWitnessV1::new(
                        round_one_witness,
                        &error_two.coefficients,
                    )?;
                    let proof = prove_zk_ams_mkhe_active_rkg_round_two_v1(
                        roster,
                        transcript_digest,
                        party_index,
                        statement,
                        witness,
                        context.authentication_secrets[party_index],
                        random,
                    )?;
                    let record_index = expected_rkg_source_record_index(
                        roster,
                        left,
                        right,
                        u32::try_from(digit_index)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                        party_index,
                        true,
                    )?;
                    let evidence = validated_source_evidence(
                        &context,
                        ordinal,
                        record_index,
                        party_index,
                        ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundTwo {
                            public_a: context.shares[party_index].public_a(),
                            party_public_b: context.shares[party_index].party_public_b(),
                            common_a: &common_a_wire,
                            h0: &party_h0_wire,
                            h1: &party_h1_wire,
                            aggregate_h0: &aggregate_h0_wire,
                            aggregate_h1: &aggregate_h1_wire,
                            k0: &k0_wire,
                            left,
                            right,
                            digit_index: u32::try_from(digit_index)
                                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                        },
                        &proof,
                    )?;
                    source_evidence.source(&evidence, sink)?;
                    pair_constant = pair_constant.add(&k0, &context.profile)?;
                }
                add_weighted_pair_source(
                    &context.profile,
                    left_index == right_index,
                    &mut source_constant,
                    &mut source_linear,
                    &pair_constant,
                    &aggregate_h1,
                )?;
                pair_index = pair_index
                    .checked_add(1)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            }
        }
        if pair_index != pair_count {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let source_components = std::array::from_fn(|_| source_linear.clone());
        stored_b_digits.push(compact_source_digit(
            &context,
            purpose,
            ordinal,
            exponent,
            master_seed,
            digit_index,
            source_constant,
            source_components,
            &mut cks_evidence,
            random,
            sink,
        )?);
    }
    let expected_source_records = pair_count
        .checked_mul(context.profile.gadget_digits)
        .and_then(|records| records.checked_mul(2))
        .and_then(|records| records.checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1))
        .and_then(|records| u32::try_from(records).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let expected_cks_records = u32::try_from(context.profile.gadget_digits)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let source_proof_set_digest = source_evidence.finish(expected_source_records, sink)?;
    let cks_proof_set_digest = cks_evidence.finish(expected_cks_records, sink)?;
    finish_generated_key(
        &context,
        purpose,
        ordinal,
        exponent,
        master_seed,
        source_proof_set_digest,
        cks_proof_set_digest,
        stored_b_digits,
    )
}

/// Generate one exact compact collective Galois key in frozen schedule order.
///
/// For each digit all eight parties prove an encryption of
/// `g^d sigma_k(s_i)` under their already verified collective-public-key
/// share.  The ordered aggregate is then compacted through the same real CKS
/// path as relinearization.  The caller supplies a schedule index, not a free
/// exponent, so missing, reordered, or substituted keys cannot be repaired.
#[allow(clippy::too_many_arguments)]
pub fn generate_zk_ams_mkhe_collective_galois_key_v1<R, S>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    states: [&ZkAmsMkheCollectivePartyStateV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    authentication_secrets: [&ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    schedule_index: usize,
    master_seed: [u8; 32],
    random: &mut R,
    sink: &mut S,
) -> Result<ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1, ZkAmsMkheErrorV1>
where
    R: MaskedRelaxedRandomSourceV1,
    S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
{
    let context = CeremonyContext::new(
        roster,
        transcript_digest,
        shares,
        states,
        authentication_secrets,
    )?;
    let schedule = zk_ams_t256_galois_key_schedule_v1()?;
    validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
    let schedule_entry = schedule
        .entries
        .get(schedule_index)
        .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)?;
    if schedule_index >= ZK_AMS_T256_GALOIS_KEY_COUNT_V1 || master_seed == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let purpose = ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois;
    let ordinal = u8::try_from(
        schedule_index
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let exponent = schedule_entry.exponent;
    let exponent_usize =
        usize::try_from(exponent).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let mut source_evidence = EvidenceHasher::new(
        purpose,
        ordinal,
        exponent,
        context.collective_key.digest(),
        ZkAmsMkheCollectiveEvidenceSetKindV1::Source,
        sink,
    )?;
    let mut cks_evidence = EvidenceHasher::new(
        purpose,
        ordinal,
        exponent,
        context.collective_key.digest(),
        ZkAmsMkheCollectiveEvidenceSetKindV1::Cks,
        sink,
    )?;
    let mut stored_b_digits = Vec::with_capacity(context.profile.gadget_digits);
    for digit_index in 0..context.profile.gadget_digits {
        let mut source_constant = RnsPolynomial::zero(&context.profile);
        let mut source_components: [RnsPolynomial; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            std::array::from_fn(|_| RnsPolynomial::zero(&context.profile));
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            let ephemeral = sample_nonzero_ternary(&context.profile, random)?;
            let error_zero = SecretPolynomial::sample_error(&context.profile, random)?;
            let error_one = SecretPolynomial::sample_error(&context.profile, random)?;
            let public_a = RnsPolynomial::from_flat(
                &context.profile,
                context.shares[party_index].public_a().residues().to_vec(),
            )?;
            let public_b = RnsPolynomial::from_flat(
                &context.profile,
                context.shares[party_index]
                    .party_public_b()
                    .residues()
                    .to_vec(),
            )?;
            let ephemeral_rns = ephemeral.as_rns(&context.profile)?;
            let transformed_secret = context.states[party_index]
                .secret()
                .automorphism(exponent_usize, &context.profile)?
                .as_rns(&context.profile)?
                .scale_gadget(digit_index, &context.profile)?;
            let constant = public_b
                .mul(&ephemeral_rns, &context.profile)?
                .add(
                    &scaled_error(&context.profile, &error_zero)?,
                    &context.profile,
                )?
                .add(&transformed_secret, &context.profile)?;
            let linear = public_a.mul(&ephemeral_rns, &context.profile)?.add(
                &scaled_error(&context.profile, &error_one)?,
                &context.profile,
            )?;
            let constant_wire = ZkAmsMkheRnsPolynomialWireV1::new(constant.coefficients.clone())?;
            let linear_wire = ZkAmsMkheRnsPolynomialWireV1::new(linear.coefficients.clone())?;
            let public_key = ZkAmsMkheActiveCollectivePublicKeyStatementV1::new(
                context.shares[party_index].public_a(),
                context.shares[party_index].party_public_b(),
            )?;
            let statement = ZkAmsMkheActiveGaloisSourceStatementV1::new(
                public_key,
                &constant_wire,
                &linear_wire,
                schedule_index,
                exponent,
                digit_index,
            )?;
            let witness = ZkAmsMkheActiveGaloisSourceWitnessV1::new(
                &context.states[party_index].secret().coefficients,
                &context.states[party_index].public_error().coefficients,
                &ephemeral.coefficients,
                &error_zero.coefficients,
                &error_one.coefficients,
            )?;
            let proof = prove_zk_ams_mkhe_active_galois_source_v1(
                roster,
                transcript_digest,
                party_index,
                statement,
                witness,
                context.authentication_secrets[party_index],
                random,
            )?;
            verify_zk_ams_mkhe_active_galois_source_v1(
                roster,
                transcript_digest,
                party_index,
                statement,
                &proof,
            )?;
            let source_record_index = digit_index
                .checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                .and_then(|base| base.checked_add(party_index))
                .and_then(|value| u32::try_from(value).ok())
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let evidence = validated_source_evidence(
                &context,
                ordinal,
                source_record_index,
                party_index,
                ZkAmsMkheCollectiveSourceStatementEvidenceV1::Galois {
                    public_a: context.shares[party_index].public_a(),
                    party_public_b: context.shares[party_index].party_public_b(),
                    source_constant: &constant_wire,
                    source_linear: &linear_wire,
                    schedule_index: u8::try_from(schedule_index)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                    exponent,
                    digit_index: u32::try_from(digit_index)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                },
                &proof,
            )?;
            source_evidence.source(&evidence, sink)?;
            source_constant = source_constant.add(&constant, &context.profile)?;
            source_components[party_index] = linear;
        }
        stored_b_digits.push(compact_source_digit(
            &context,
            purpose,
            ordinal,
            exponent,
            master_seed,
            digit_index,
            source_constant,
            source_components,
            &mut cks_evidence,
            random,
            sink,
        )?);
    }
    let expected_source_records = context
        .profile
        .gadget_digits
        .checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .and_then(|records| u32::try_from(records).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let expected_cks_records = u32::try_from(context.profile.gadget_digits)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let source_proof_set_digest = source_evidence.finish(expected_source_records, sink)?;
    let cks_proof_set_digest = cks_evidence.finish(expected_cks_records, sink)?;
    finish_generated_key(
        &context,
        purpose,
        ordinal,
        exponent,
        master_seed,
        source_proof_set_digest,
        cks_proof_set_digest,
        stored_b_digits,
    )
}

/// Reusable, non-secret runtime context for the exact 32-key evaluated-key set.
///
/// Construction validates the governed roster, all eight ordered proof-carrying
/// collective-public-key shares, and the complete manifest exactly once.  It
/// retains only the small manifest table and verified aggregate CPK; individual
/// ~1.5 GiB evaluated-key payloads remain provider-streamed one at a time.
#[derive(Debug)]
pub struct ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1 {
    profile: BgvProfile,
    wire_roster: ZkAmsMkheGovernedRosterWireV1,
    collective_key: ZkAmsMkheCollectivePublicKeyV1,
    transcript_digest: [u8; 32],
    manifest_digest: [u8; 32],
    entries: Vec<ZkAmsMkheCollectiveEvaluatedKeyEntryV1>,
    runtime_context_digest: [u8; 32],
}

/// One canonical evaluated-key payload validated for one reusable runtime.
///
/// The wrapper owns exactly one streamed payload and cannot be constructed
/// without checking its manifest entry, canonical wire binding, BLAKE3 digest,
/// and complete evidence-set identity.
#[derive(PartialEq, Eq)]
pub struct ZkAmsMkheValidatedCollectiveEvaluatedKeyV1 {
    runtime_context_digest: [u8; 32],
    entry: ZkAmsMkheCollectiveEvaluatedKeyEntryV1,
    wire: ZkAmsMkheSeededRkgKeyWireV1,
}

impl core::fmt::Debug for ZkAmsMkheValidatedCollectiveEvaluatedKeyV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheValidatedCollectiveEvaluatedKeyV1")
            .field(
                "runtime_context_digest",
                &hex::encode(self.runtime_context_digest),
            )
            .field("entry", &self.entry)
            .field("stored_digits", &self.wire.stored_b_digits().len())
            .finish()
    }
}

impl ZkAmsMkheValidatedCollectiveEvaluatedKeyV1 {
    /// Exact canonical manifest entry for this payload.
    #[must_use]
    pub const fn entry(&self) -> ZkAmsMkheCollectiveEvaluatedKeyEntryV1 {
        self.entry
    }

    /// Validated canonical two-polynomial-per-digit key wire.
    #[must_use]
    pub const fn wire(&self) -> &ZkAmsMkheSeededRkgKeyWireV1 {
        &self.wire
    }
}

impl ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1 {
    /// Validate the aggregate CPK and exact consensus-bound key-set manifest once.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        manifest: &ZkAmsMkheCollectiveEvaluatedKeyManifestV1,
        expected_manifest_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if expected_manifest_digest == [0; 32]
            || manifest.manifest_digest() != expected_manifest_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let profile = release_profile_v1();
        profile.validate()?;
        let wire_roster = roster.to_wire_roster()?;
        let collective_key =
            aggregate_zk_ams_mkhe_collective_public_key_v1(roster, transcript_digest, shares)?;
        let manifest_bytes = manifest.encode(&wire_roster)?;
        let decoded = ZkAmsMkheCollectiveEvaluatedKeyManifestV1::decode_exact(
            &manifest_bytes,
            &wire_roster,
            transcript_digest,
        )?;
        if decoded.manifest_digest() != expected_manifest_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        if decoded != *manifest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut runtime_frame = Vec::with_capacity(256);
        runtime_frame.extend_from_slice(EVALUATED_KEY_RUNTIME_DOMAIN_V1);
        runtime_frame.push(MKHE_VERSION_V1);
        runtime_frame.extend_from_slice(&wire_roster.profile_digest());
        runtime_frame.extend_from_slice(&wire_roster.roster_digest());
        runtime_frame.extend_from_slice(&wire_roster.epoch().to_be_bytes());
        runtime_frame.extend_from_slice(&transcript_digest);
        runtime_frame.extend_from_slice(&collective_key.digest());
        runtime_frame.extend_from_slice(&expected_manifest_digest);
        let runtime_context_digest = keccak256(&runtime_frame);
        if runtime_context_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self {
            profile,
            wire_roster,
            collective_key,
            transcript_digest,
            manifest_digest: expected_manifest_digest,
            entries: manifest.entries().to_vec(),
            runtime_context_digest,
        })
    }

    /// Verified aggregate collective-public-key digest.
    #[must_use]
    pub const fn collective_key_digest(&self) -> [u8; 32] {
        self.collective_key.digest()
    }

    /// Exact consensus-bound evaluated-key manifest digest.
    #[must_use]
    pub const fn manifest_digest(&self) -> [u8; 32] {
        self.manifest_digest
    }

    /// Validate one already decoded provider payload without retaining any other key.
    pub fn validate_streamed_key(
        &self,
        ordinal: usize,
        wire: ZkAmsMkheSeededRkgKeyWireV1,
    ) -> Result<ZkAmsMkheValidatedCollectiveEvaluatedKeyV1, ZkAmsMkheErrorV1> {
        let payload = wire.encode()?;
        let payload_bytes =
            u64::try_from(payload.len()).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let payload_blake3 = blake3_hash(&payload);
        self.validate_streamed_key_prehashed(ordinal, wire, payload_bytes, payload_blake3)
    }

    /// Decode and validate one exact provider payload in canonical manifest order.
    pub fn decode_and_validate_streamed_key(
        &self,
        ordinal: usize,
        payload: &[u8],
    ) -> Result<ZkAmsMkheValidatedCollectiveEvaluatedKeyV1, ZkAmsMkheErrorV1> {
        let entry = self.entry(ordinal)?;
        let payload_bytes =
            u64::try_from(payload.len()).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if payload_bytes != entry.payload_bytes() {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let payload_blake3 = blake3_hash(payload);
        if payload_blake3 != entry.payload_blake3() {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let expected_binding = ZkAmsMkheWireBindingV1::new(
            &self.wire_roster,
            self.transcript_digest,
            u32::try_from(ordinal).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
            0,
        )?;
        let wire = ZkAmsMkheSeededRkgKeyWireV1::decode_exact(payload, expected_binding)?;
        self.validate_streamed_key_prehashed(ordinal, wire, payload_bytes, payload_blake3)
    }

    fn entry(
        &self,
        ordinal: usize,
    ) -> Result<ZkAmsMkheCollectiveEvaluatedKeyEntryV1, ZkAmsMkheErrorV1> {
        let entry = *self
            .entries
            .get(ordinal)
            .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)?;
        if usize::from(entry.ordinal()) != ordinal {
            return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
        }
        let expected_exponent = match entry.purpose() {
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization => {
                if ordinal != 0 || entry.galois_exponent() != 0 {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                0
            }
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois => {
                let schedule = zk_ams_t256_galois_key_schedule_v1()?;
                validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
                schedule
                    .entries
                    .get(
                        ordinal
                            .checked_sub(1)
                            .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)?,
                    )
                    .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)?
                    .exponent
            }
        };
        if entry.galois_exponent() != expected_exponent {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(entry)
    }

    fn validate_streamed_key_prehashed(
        &self,
        ordinal: usize,
        wire: ZkAmsMkheSeededRkgKeyWireV1,
        payload_bytes: u64,
        payload_blake3: [u8; 32],
    ) -> Result<ZkAmsMkheValidatedCollectiveEvaluatedKeyV1, ZkAmsMkheErrorV1> {
        let entry = self.entry(ordinal)?;
        let binding = wire.binding();
        if binding.profile_digest() != self.wire_roster.profile_digest()
            || binding.roster_digest() != self.wire_roster.roster_digest()
            || binding.epoch() != self.wire_roster.epoch()
            || binding.transcript_digest() != self.transcript_digest
            || usize::try_from(binding.record_index()).ok() != Some(ordinal)
            || binding.level() != 0
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        if payload_bytes != entry.payload_bytes()
            || payload_blake3 != entry.payload_blake3()
            || wire.contribution_proof_digest()
                != evaluated_key_evidence_digest(
                    entry.purpose(),
                    entry.ordinal(),
                    entry.galois_exponent(),
                    self.collective_key.digest(),
                    entry.source_proof_set_digest(),
                    entry.cks_proof_set_digest(),
                )?
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(ZkAmsMkheValidatedCollectiveEvaluatedKeyV1 {
            runtime_context_digest: self.runtime_context_digest,
            entry,
            wire,
        })
    }

    fn validate_key_context(
        &self,
        key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if key.runtime_context_digest != self.runtime_context_digest
            || self.entry(usize::from(key.entry.ordinal()))? != key.entry
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    fn native_digit(
        &self,
        key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
        digit_index: usize,
    ) -> Result<(RnsPolynomial, RnsPolynomial), ZkAmsMkheErrorV1> {
        self.validate_key_context(key)?;
        if key.wire.stored_b_digits().len() != self.profile.gadget_digits
            || digit_index >= self.profile.gadget_digits
        {
            return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
        }
        let stored_b = key
            .wire
            .stored_b_digits()
            .get(digit_index)
            .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)?;
        Ok((
            RnsPolynomial::from_flat(&self.profile, stored_b.residues().to_vec())?,
            derive_target_a(
                &self.profile,
                &self.wire_roster,
                self.collective_key.transcript_digest(),
                self.collective_key.digest(),
                key.entry.purpose(),
                key.entry.ordinal(),
                key.entry.galois_exponent(),
                key.wire.a_master_seed(),
                digit_index,
            )?,
        ))
    }

    fn output_lineage(
        &self,
        key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
        input_digest: [u8; 32],
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        self.validate_key_context(key)?;
        let mut frame = Vec::with_capacity(192);
        frame.extend_from_slice(EVALUATED_KEY_LINEAGE_DOMAIN_V1);
        frame.extend_from_slice(&[MKHE_VERSION_V1, key.entry.purpose() as u8]);
        frame.extend_from_slice(&key.entry.galois_exponent().to_be_bytes());
        frame.extend_from_slice(&self.collective_key.digest());
        frame.extend_from_slice(&self.manifest_digest);
        frame.extend_from_slice(&key.entry.payload_blake3());
        frame.extend_from_slice(&input_digest);
        Ok(keccak256(&frame))
    }
}

fn apply_compact_switch_with_provider(
    profile: &BgvProfile,
    base_constant: &RnsPolynomial,
    base_linear: &RnsPolynomial,
    switched: &RnsPolynomial,
    digit_count: usize,
    mut digit_provider: impl FnMut(usize) -> Result<(RnsPolynomial, RnsPolynomial), ZkAmsMkheErrorV1>,
) -> Result<(RnsPolynomial, RnsPolynomial), ZkAmsMkheErrorV1> {
    base_constant.validate(profile)?;
    base_linear.validate(profile)?;
    switched.validate(profile)?;
    if digit_count != profile.gadget_digits {
        return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
    }
    checked_ring_multiplication_work(
        profile,
        digit_count
            .checked_mul(2)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )?;
    let decomposition = gadget_decompose(profile, switched)?;
    let mut constant = base_constant.clone();
    let mut linear = base_linear.clone();
    for (digit_index, plaintext_digit) in decomposition.iter().enumerate() {
        let (stored_b, seeded_a) = digit_provider(digit_index)?;
        stored_b.validate(profile)?;
        seeded_a.validate(profile)?;
        constant = constant.add(&plaintext_digit.mul(&stored_b, profile)?, profile)?;
        linear = linear.add(&plaintext_digit.mul(&seeded_a, profile)?, profile)?;
    }
    Ok((constant, linear))
}

#[cfg(test)]
fn apply_compact_switch(
    profile: &BgvProfile,
    base_constant: &RnsPolynomial,
    base_linear: &RnsPolynomial,
    switched: &RnsPolynomial,
    digits: &[(RnsPolynomial, RnsPolynomial)],
) -> Result<(RnsPolynomial, RnsPolynomial), ZkAmsMkheErrorV1> {
    apply_compact_switch_with_provider(
        profile,
        base_constant,
        base_linear,
        switched,
        digits.len(),
        |digit_index| {
            digits
                .get(digit_index)
                .cloned()
                .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)
        },
    )
}

/// Relinearize one exact collective level-one ciphertext with a compact key.
///
/// The reusable runtime has already validated the roster, CPK proofs, and
/// manifest; `key` owns exactly one provider-streamed canonical payload.
pub fn relinearize_zk_ams_mkhe_collective_v1(
    runtime: &ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1,
    key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
    ciphertext: &ZkAmsMkheCollectiveLevelOneV1,
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1> {
    runtime.validate_key_context(key)?;
    if key.entry.purpose() != ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization
        || key.entry.ordinal() != 0
    {
        return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
    }
    ciphertext.validate_for_key(&runtime.collective_key, &runtime.profile)?;
    let (constant, linear) = apply_compact_switch_with_provider(
        &runtime.profile,
        ciphertext.constant(),
        ciphertext.linear(),
        ciphertext.quadratic(),
        runtime.profile.gadget_digits,
        |digit_index| runtime.native_digit(key, digit_index),
    )?;
    ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        &runtime.profile,
        runtime.collective_key.parties(),
        ciphertext.epoch(),
        runtime.output_lineage(key, ciphertext.digest())?,
        ciphertext.sample_index(),
        1,
        constant,
        linear,
        Some(runtime.collective_key.digest()),
    )
}

/// Apply a frozen Galois automorphism and compactly switch back to `S`.
pub fn automorphism_switch_zk_ams_mkhe_collective_v1(
    runtime: &ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1,
    schedule_index: usize,
    key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1> {
    let ordinal = schedule_index
        .checked_add(1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    runtime.validate_key_context(key)?;
    if key.entry.purpose() != ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois
        || usize::from(key.entry.ordinal()) != ordinal
    {
        return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
    }
    validate_compact_for_key(ciphertext, &runtime.collective_key, &runtime.profile)?;
    let exponent = usize::try_from(key.entry.galois_exponent())
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let transformed_constant = ciphertext
        .constant()
        .automorphism(exponent, &runtime.profile)?;
    let transformed_linear = ciphertext
        .linear()
        .automorphism(exponent, &runtime.profile)?;
    let (constant, linear) = apply_compact_switch_with_provider(
        &runtime.profile,
        &transformed_constant,
        &RnsPolynomial::zero(&runtime.profile),
        &transformed_linear,
        runtime.profile.gadget_digits,
        |digit_index| runtime.native_digit(key, digit_index),
    )?;
    ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        &runtime.profile,
        runtime.collective_key.parties(),
        ciphertext.epoch(),
        runtime.output_lineage(key, ciphertext.digest())?,
        ciphertext.sample_index(),
        ciphertext.level(),
        constant,
        linear,
        Some(runtime.collective_key.digest()),
    )
}

/// Exact release ring-multiplication count of one compact key switch.
pub fn zk_ams_mkhe_compact_key_switch_ring_multiplications_v1() -> Result<u64, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile.validate()?;
    u64::try_from(
        profile
            .gadget_digits
            .checked_mul(2)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn blake3_hash(input: &[u8]) -> [u8; 32] {
    norito::streaming::blake3_hash(input)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    const TEST_MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
    const TEST_ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];

    fn test_profile() -> BgvProfile {
        BgvProfile {
            profile_id: [0x79; 32],
            ring_degree: 8,
            moduli: &TEST_MODULI,
            negacyclic_roots: &TEST_ROOTS,
            plaintext_modulus: super::super::PlaintextModulus::Tiny(17),
            error_eta: 2,
            hybrid_rns_decomposition: false,
            gadget_base_log: 8,
            gadget_digits: 8,
            max_ciphertext_bytes: 1 << 20,
            max_evaluated_key_bytes: 16 << 20,
            max_round_bytes: 16 << 20,
            max_share_bytes: 4 << 20,
            max_workspace_bytes: 16 << 20,
            max_work_units: 1 << 20,
        }
    }

    fn signed(profile: &BgvProfile, values: &[i64; 8]) -> RnsPolynomial {
        RnsPolynomial::from_signed(profile, values).unwrap()
    }

    fn deterministic_a(profile: &BgvProfile, digit_index: usize) -> RnsPolynomial {
        let values: [u64; 8] = std::array::from_fn(|coefficient| {
            u64::try_from((digit_index + 3) * (coefficient + 5) + coefficient * coefficient + 1)
                .unwrap()
        });
        RnsPolynomial::from_unsigned(profile, &values).unwrap()
    }

    fn exact_compact_digits(
        profile: &BgvProfile,
        secret: &RnsPolynomial,
        encrypted_target: &RnsPolynomial,
    ) -> Vec<(RnsPolynomial, RnsPolynomial)> {
        (0..profile.gadget_digits)
            .map(|digit_index| {
                let a = deterministic_a(profile, digit_index);
                let b = encrypted_target
                    .scale_gadget(digit_index, profile)
                    .unwrap()
                    .sub(&a.mul(secret, profile).unwrap(), profile)
                    .unwrap();
                (b, a)
            })
            .collect()
    }

    #[test]
    fn compact_relinearization_matches_direct_tiny_decryption_with_balanced_digits() {
        let profile = test_profile();
        profile.validate().unwrap();
        let secret = signed(&profile, &[-1, 0, 1, 1, 0, -1, 1, 0]);
        let constant = signed(&profile, &[4, -7, 9, 0, -3, 12, 1, -5]);
        let linear = signed(&profile, &[-2, 5, 0, 8, -11, 3, 7, 1]);
        let quadratic = signed(&profile, &[127, -129, 255, -257, 513, -769, 31, -63]);
        let secret_squared = secret.mul(&secret, &profile).unwrap();
        let digits = exact_compact_digits(&profile, &secret, &secret_squared);
        let (switched_constant, switched_linear) =
            apply_compact_switch(&profile, &constant, &linear, &quadratic, &digits).unwrap();
        let observed = switched_constant
            .add(&switched_linear.mul(&secret, &profile).unwrap(), &profile)
            .unwrap();
        let expected = constant
            .add(&linear.mul(&secret, &profile).unwrap(), &profile)
            .unwrap()
            .add(&quadratic.mul(&secret_squared, &profile).unwrap(), &profile)
            .unwrap();
        assert_eq!(observed, expected);

        assert_eq!(
            apply_compact_switch(
                &profile,
                &constant,
                &linear,
                &quadratic,
                &digits[..digits.len() - 1],
            ),
            Err(ZkAmsMkheErrorV1::MissingEvaluatedKey)
        );
    }

    #[test]
    fn compact_galois_switch_matches_direct_decryption_for_every_tiny_automorphism() {
        let profile = test_profile();
        let secret = signed(&profile, &[-1, 1, 0, 1, -1, 0, 0, 1]);
        let constant = signed(&profile, &[11, -13, 17, 19, -23, 29, -31, 37]);
        let linear = signed(&profile, &[-41, 43, 47, -53, 59, 61, -67, 71]);
        let decrypted = constant
            .add(&linear.mul(&secret, &profile).unwrap(), &profile)
            .unwrap();
        for exponent in (1..(2 * profile.ring_degree)).step_by(2) {
            let transformed_secret = secret.automorphism(exponent, &profile).unwrap();
            let transformed_constant = constant.automorphism(exponent, &profile).unwrap();
            let transformed_linear = linear.automorphism(exponent, &profile).unwrap();
            let digits = exact_compact_digits(&profile, &secret, &transformed_secret);
            let (switched_constant, switched_linear) = apply_compact_switch(
                &profile,
                &transformed_constant,
                &RnsPolynomial::zero(&profile),
                &transformed_linear,
                &digits,
            )
            .unwrap();
            let observed = switched_constant
                .add(&switched_linear.mul(&secret, &profile).unwrap(), &profile)
                .unwrap();
            assert_eq!(
                observed,
                decrypted.automorphism(exponent, &profile).unwrap(),
                "odd exponent {exponent}"
            );
        }
        assert!(secret.automorphism(2, &profile).is_err());
    }

    #[test]
    fn unordered_pair_aggregation_has_eight_diagonal_and_28_doubled_off_diagonal_terms() {
        let profile = test_profile();
        let secrets = [
            [-1, 0, 1, 0, 0, 0, 0, 0],
            [0, 1, 0, -1, 0, 0, 0, 0],
            [1, 0, 0, 0, 1, 0, 0, 0],
            [0, -1, 0, 0, 0, 1, 0, 0],
            [0, 0, 1, 0, 0, 0, -1, 0],
            [0, 0, 0, 1, 0, 0, 0, -1],
            [1, -1, 0, 0, 0, 0, 0, 0],
            [0, 0, 1, -1, 0, 0, 0, 0],
        ]
        .map(|values| signed(&profile, &values));
        let collective = secrets
            .iter()
            .try_fold(RnsPolynomial::zero(&profile), |sum, secret| {
                sum.add(secret, &profile)
            })
            .unwrap();
        let mut source_constant = RnsPolynomial::zero(&profile);
        let mut source_linear = RnsPolynomial::zero(&profile);
        let mut diagonal = 0;
        let mut off_diagonal = 0;
        for left in 0..secrets.len() {
            for right in left..secrets.len() {
                let pair = secrets[left].mul(&secrets[right], &profile).unwrap();
                add_weighted_pair_source(
                    &profile,
                    left == right,
                    &mut source_constant,
                    &mut source_linear,
                    &pair,
                    &pair,
                )
                .unwrap();
                if left == right {
                    diagonal += 1;
                } else {
                    off_diagonal += 1;
                }
            }
        }
        let collective_squared = collective.mul(&collective, &profile).unwrap();
        assert_eq!(diagonal, 8);
        assert_eq!(off_diagonal, 28);
        assert_eq!(source_constant, collective_squared);
        assert_eq!(source_linear, collective_squared);
    }

    fn test_evidence_digest(
        records: &[(u32, &[u8])],
        expected_records: u32,
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let mut sink = NoopEvidenceSink;
        let mut hasher = EvidenceHasher::new(
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
            0,
            0,
            [0x55; 32],
            ZkAmsMkheCollectiveEvidenceSetKindV1::Source,
            &mut sink,
        )?;
        for (index, bytes) in records {
            hasher.test_record(*index, bytes)?;
        }
        hasher.finish(expected_records, &mut sink)
    }

    struct NoopEvidenceSink;

    impl ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1 for NoopEvidenceSink {
        fn begin_evidence_set(
            &mut self,
            _header: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
        ) -> Result<(), ZkAmsMkheErrorV1> {
            Ok(())
        }

        fn begin_evidence_record(
            &mut self,
            _header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
        ) -> Result<(), ZkAmsMkheErrorV1> {
            Ok(())
        }

        fn write_evidence_record_chunk(
            &mut self,
            _header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
            _chunk_index: u32,
            _bytes: &[u8],
        ) -> Result<(), ZkAmsMkheErrorV1> {
            Ok(())
        }

        fn finish_evidence_record(
            &mut self,
            _footer: ZkAmsMkheCollectiveEvidenceRecordFooterV1,
        ) -> Result<(), ZkAmsMkheErrorV1> {
            Ok(())
        }

        fn finish_evidence_set(
            &mut self,
            _footer: ZkAmsMkheCollectiveEvidenceSetFooterV1,
        ) -> Result<(), ZkAmsMkheErrorV1> {
            Ok(())
        }
    }

    #[test]
    fn canonical_evidence_stream_rejects_omission_reorder_duplicate_and_splice() {
        let baseline = [
            (0, b"statement-proof-0".as_slice()),
            (1, b"statement-proof-1"),
        ];
        let digest = test_evidence_digest(&baseline, 2).unwrap();
        assert_ne!(digest, [0; 32]);
        assert!(test_evidence_digest(&baseline[..1], 2).is_err());
        assert!(
            test_evidence_digest(&[(1, b"statement-proof-1"), (0, b"statement-proof-0")], 2,)
                .is_err()
        );
        assert!(
            test_evidence_digest(&[(0, b"statement-proof-0"), (0, b"statement-proof-0")], 2,)
                .is_err()
        );
        let mutated =
            test_evidence_digest(&[(0, b"statement-proof-X"), (1, b"statement-proof-1")], 2)
                .unwrap();
        let spliced =
            test_evidence_digest(&[(0, b"statement-proof-0"), (1, b"other-roster-proof")], 2)
                .unwrap();
        assert_ne!(mutated, digest);
        assert_ne!(spliced, digest);
    }

    #[test]
    fn release_schedule_and_online_work_are_exact_and_roster_independent() {
        let schedule = zk_ams_t256_galois_key_schedule_v1().unwrap();
        validate_zk_ams_t256_galois_key_schedule_v1(&schedule).unwrap();
        assert_eq!(schedule.entries.len(), 31);
        let exponents = schedule
            .entries
            .iter()
            .map(|entry| entry.exponent)
            .collect::<BTreeSet<_>>();
        assert_eq!(exponents.len(), 31);
        assert!(exponents.iter().all(|exponent| exponent % 2 == 1));
        assert_eq!(
            zk_ams_mkhe_compact_key_switch_ring_multiplications_v1().unwrap(),
            76
        );
        assert_eq!(test_profile().gadget_digits * 2, 16);
    }
}
