//! Proof-bound collective ingress for the governed ZK-AMS MKHE roster.
//!
//! The native relation proved by each ordered party is
//! `b_i = -a_pk*s_i + t*e_i` together with
//! `d_i = (c_i-a_target)*s_i + t*z_i`. Every public entry point validates the
//! complete profile, roster, epoch, transcript, source, key context, party,
//! and native relation before accepting or combining a contribution.

use core::mem::size_of;

use super::{
    ArtifactAuthentication, BgvProfile, MAX_RANDOM_REJECTION_ATTEMPTS_V1, MKHE_VERSION_V1,
    MaskedRelaxedRandomSourceV1, RnsPolynomial, SecretPolynomial, ZkAmsMkheActivePartySecretV1,
    ZkAmsMkheAuthenticationWireV1, ZkAmsMkheCksContributionWireV1,
    ZkAmsMkheCollectiveCiphertextWireV1, ZkAmsMkheErrorV1, ZkAmsMkheGovernedRosterWireV1,
    ZkAmsMkhePartyIdV1, ZkAmsMkheProofEnvelopeWireV1, ZkAmsMkheProofKindV1,
    ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheWireBindingV1, checked_ring_multiplication_work,
    collective::ZkAmsMkheCollectivePartyStateV1,
    decryption::{
        SignedWideV1, WIDE_RELATION_MASK_SLACK_LOG2_V1, WideMagnitudeV1, sample_signed_small,
        sample_signed_wide, small_response_parameters, sparse_negacyclic_mul_small,
        sparse_negacyclic_mul_wide, validate_wide_relation_random_health,
        wide_relation_challenge_weight, wide_response_parameters, wide_vector_as_rns,
    },
    manifest::{
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1, zk_ams_mkhe_noise_certificate_v1,
        zk_ams_mkhe_release_manifest_v1,
    },
    wire::{
        ZK_AMS_MKHE_MAX_PROOF_BYTES_V1, derive_wire_length_certificate_v1,
        zk_ams_mkhe_cks_statement_digest_v1,
    },
};
use crate::vega::sponge::{Keccak256, keccak256, shake256};

const CKS_PROOF_TAG_V1: [u8; 4] = *b"ZACP";
const CKS_PROOF_HEADER_BYTES_V1: usize = 4 + 1 + 2 + 4 + 32 + 4 + 4 + 4;
const CKS_SIGNED_SMALL_BYTES_V1: usize = size_of::<i64>();
const CKS_RESOURCE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.cks-resource-evidence";
const CKS_SOURCE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.cks-source-ciphertext";
const CKS_KEY_CONTEXT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.cks-key-context";
const CKS_BINDING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.cks-binding";
const CKS_AUTH_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.authenticated-cks-contribution";
const CKS_PROOF_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.cks-wide-relation-proof";
const CKS_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.cks-proof-fiat-shamir";
const CKS_SPARSE_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.cks-proof-sparse-challenge";

/// Canonical full-roster extension of an independently keyed source ciphertext.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCksSourceCiphertextV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    parties: [ZkAmsMkhePartyIdV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    transcript_digest: [u8; 32],
    record_index: u32,
    sample_index: u64,
    level: u8,
    constant: ZkAmsMkheRnsPolynomialWireV1,
    components: [Option<ZkAmsMkheRnsPolynomialWireV1>; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    source_digest: [u8; 32],
}

impl ZkAmsMkheCksSourceCiphertextV1 {
    /// Construct and canonically extend an ordered source component list to the full roster.
    ///
    /// A roster member omitted from `components` is exactly the zero polynomial.
    /// Explicit zero components are rejected so the absent representation is unique.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        roster: &ZkAmsMkheGovernedRosterWireV1,
        transcript_digest: [u8; 32],
        record_index: u32,
        sample_index: u64,
        level: u8,
        constant: ZkAmsMkheRnsPolynomialWireV1,
        components: Vec<(ZkAmsMkhePartyIdV1, ZkAmsMkheRnsPolynomialWireV1)>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let manifest = zk_ams_mkhe_release_manifest_v1()?;
        ZkAmsMkheWireBindingV1::new(roster, transcript_digest, record_index, level)?;
        if roster.profile_digest() != profile.digest()?
            || sample_index >= manifest.max_samples_per_secret_epoch
            || components.len() > ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        constant.encoded_len()?;
        let mut canonical = std::array::from_fn(|_| None);
        let mut previous = None;
        for (party, polynomial) in components {
            if previous.is_some_and(|value| value >= party)
                || polynomial.residues().iter().all(|value| *value == 0)
            {
                return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
            }
            polynomial.encoded_len()?;
            let index = roster
                .parties()
                .iter()
                .position(|candidate| *candidate == party)
                .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
            if canonical[index].replace(polynomial).is_some() {
                return Err(ZkAmsMkheErrorV1::InvalidPartySet);
            }
            previous = Some(party);
        }
        let mut value = Self {
            profile_digest: roster.profile_digest(),
            roster_digest: roster.roster_digest(),
            epoch: roster.epoch(),
            parties: *roster.parties(),
            transcript_digest,
            record_index,
            sample_index,
            level,
            constant,
            components: canonical,
            source_digest: [0; 32],
        };
        value.source_digest = value.recompute_digest()?;
        Ok(value)
    }

    /// Digest of the exact source metadata, constant, and zero-extended component vector.
    #[must_use]
    pub const fn source_digest(&self) -> [u8; 32] {
        self.source_digest
    }

    /// Source transcript digest inherited by every contribution and the compact output.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }

    /// Source record index used for the compact output binding.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.record_index
    }

    /// RLWE sample index retained by the compact output.
    #[must_use]
    pub const fn sample_index(&self) -> u64 {
        self.sample_index
    }

    /// BGV level retained by the compact output.
    #[must_use]
    pub const fn level(&self) -> u8 {
        self.level
    }

    /// Constant source polynomial `c_0`.
    #[must_use]
    pub const fn constant(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        &self.constant
    }

    /// Present source component for one roster slot; absence means canonical zero.
    #[must_use]
    pub fn component(&self, party_index: usize) -> Option<&ZkAmsMkheRnsPolynomialWireV1> {
        self.components.get(party_index).and_then(Option::as_ref)
    }

    fn validate_for_roster(
        &self,
        roster: &ZkAmsMkheGovernedRosterWireV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let manifest = zk_ams_mkhe_release_manifest_v1()?;
        if self.profile_digest != roster.profile_digest()
            || self.roster_digest != roster.roster_digest()
            || self.epoch != roster.epoch()
            || self.parties != *roster.parties()
            || self.transcript_digest == [0; 32]
            || self.sample_index >= manifest.max_samples_per_secret_epoch
            || self.level > 1
            || self.source_digest == [0; 32]
            || self.source_digest != self.recompute_digest()?
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        self.constant.encoded_len()?;
        for component in self.components.iter().flatten() {
            component.encoded_len()?;
            if component.residues().iter().all(|value| *value == 0) {
                return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
            }
        }
        Ok(())
    }

    fn recompute_digest(&self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let mut hash = Keccak256::new();
        hash.update(CKS_SOURCE_DOMAIN_V1);
        hash.update(&self.profile_digest);
        hash.update(&self.roster_digest);
        hash.update(&self.epoch.to_be_bytes());
        hash.update(&self.transcript_digest);
        hash.update(&self.record_index.to_be_bytes());
        hash.update(&self.sample_index.to_be_bytes());
        hash.update(&[self.level]);
        hash.update(&wire_polynomial_digest(&self.constant)?);
        let zero_digest = zero_polynomial_digest(&profile)?;
        for (party, component) in self.parties.iter().zip(&self.components) {
            hash.update(&party.to_bytes());
            hash.update(
                &component
                    .as_ref()
                    .map_or(Ok(zero_digest), wire_polynomial_digest)?,
            );
        }
        Ok(hash.finalize())
    }
}

/// Borrowed public CKS statement for one governed source ciphertext.
#[derive(Clone, Copy, Debug)]
pub struct ZkAmsMkheCksStatementV1<'a> {
    roster: &'a ZkAmsMkheGovernedRosterWireV1,
    source: &'a ZkAmsMkheCksSourceCiphertextV1,
    target_a: &'a ZkAmsMkheRnsPolynomialWireV1,
    public_key_a: &'a ZkAmsMkheRnsPolynomialWireV1,
    party_public_b: &'a [&'a ZkAmsMkheRnsPolynomialWireV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    key_context_digest: [u8; 32],
}

impl<'a> ZkAmsMkheCksStatementV1<'a> {
    /// Construct the complete release statement for all ordered CKS contributions.
    pub fn new(
        roster: &'a ZkAmsMkheGovernedRosterWireV1,
        source: &'a ZkAmsMkheCksSourceCiphertextV1,
        target_a: &'a ZkAmsMkheRnsPolynomialWireV1,
        public_key_a: &'a ZkAmsMkheRnsPolynomialWireV1,
        party_public_b: &'a [&'a ZkAmsMkheRnsPolynomialWireV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        source.validate_for_roster(roster)?;
        target_a.encoded_len()?;
        public_key_a.encoded_len()?;
        if target_a.residues().iter().all(|value| *value == 0)
            || public_key_a.residues().iter().all(|value| *value == 0)
            || party_public_b
                .iter()
                .any(|value| value.residues().iter().all(|residue| *residue == 0))
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        for value in party_public_b {
            value.encoded_len()?;
        }
        let key_context_digest = key_context_digest(target_a, public_key_a, party_public_b)?;
        Ok(Self {
            roster,
            source,
            target_a,
            public_key_a,
            party_public_b,
            key_context_digest,
        })
    }

    /// Exact governed roster.
    #[must_use]
    pub const fn roster(&self) -> &'a ZkAmsMkheGovernedRosterWireV1 {
        self.roster
    }

    /// Canonically zero-extended source ciphertext.
    #[must_use]
    pub const fn source(&self) -> &'a ZkAmsMkheCksSourceCiphertextV1 {
        self.source
    }

    /// Compact-output linear polynomial `a_target`.
    #[must_use]
    pub const fn target_a(&self) -> &'a ZkAmsMkheRnsPolynomialWireV1 {
        self.target_a
    }

    /// Common public-key relation polynomial `a_pk`.
    #[must_use]
    pub const fn public_key_a(&self) -> &'a ZkAmsMkheRnsPolynomialWireV1 {
        self.public_key_a
    }

    /// Ordered public-key relation polynomials `b_i`.
    #[must_use]
    pub const fn party_public_b(
        &self,
    ) -> &'a [&'a ZkAmsMkheRnsPolynomialWireV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] {
        self.party_public_b
    }

    /// Digest binding the target and complete governed public-key relation set.
    #[must_use]
    pub const fn key_context_digest(&self) -> [u8; 32] {
        self.key_context_digest
    }

    fn validate(&self) -> Result<(), ZkAmsMkheErrorV1> {
        self.source.validate_for_roster(self.roster)?;
        if self.key_context_digest
            != key_context_digest(self.target_a, self.public_key_a, self.party_public_b)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    fn internal_for_party(
        &self,
        party_index: usize,
    ) -> Result<(BgvProfile, super::PartySet, CksRelationV1), ZkAmsMkheErrorV1> {
        self.validate()?;
        if party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPartySet);
        }
        let profile = release_profile_v1();
        let parties = super::PartySet::new(self.roster.parties().to_vec())?;
        let source_component = self.source.components[party_index].as_ref().map_or_else(
            || Ok(RnsPolynomial::zero(&profile)),
            |value| RnsPolynomial::from_flat(&profile, value.residues().to_vec()),
        )?;
        let binding = CksBindingV1 {
            profile_digest: self.roster.profile_digest(),
            roster_digest: self.roster.roster_digest(),
            epoch: self.roster.epoch(),
            transcript_digest: self.source.transcript_digest,
            source_digest: self.source.source_digest,
            key_context_digest: self.key_context_digest,
            source_record_index: self.source.record_index,
            sample_index: self.source.sample_index,
            party_index: u8::try_from(party_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
            party: self.roster.parties()[party_index],
            level: self.source.level,
        };
        let relation = CksRelationV1 {
            binding,
            public_key_a: RnsPolynomial::from_flat(
                &profile,
                self.public_key_a.residues().to_vec(),
            )?,
            party_public_b: RnsPolynomial::from_flat(
                &profile,
                self.party_public_b[party_index].residues().to_vec(),
            )?,
            source_component,
            target_a: RnsPolynomial::from_flat(&profile, self.target_a.residues().to_vec())?,
        };
        relation.validate(&profile, &parties)?;
        Ok((profile, parties, relation))
    }
}

/// Canonical fixed-width wide-coefficient CKS relation proof.
#[derive(Clone, PartialEq, Eq)]
pub struct ZkAmsMkheCksProofV1 {
    wide_response_bytes: u16,
    challenge_seed: [u8; 32],
    secret_response: Vec<i64>,
    public_key_error_response: Vec<i64>,
    smudge_response: Vec<SignedWideV1>,
}

impl core::fmt::Debug for ZkAmsMkheCksProofV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCksProofV1")
            .field("wide_response_bytes", &self.wide_response_bytes)
            .field("challenge_seed", &hex::encode(self.challenge_seed))
            .field("coefficient_count", &self.secret_response.len())
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheCksProofV1 {
    /// Exact length of the canonical fixed-width native proof.
    pub fn encoded_len(&self) -> Result<usize, ZkAmsMkheErrorV1> {
        if self.challenge_seed == [0; 32]
            || self.secret_response.is_empty()
            || self.secret_response.len() != self.public_key_error_response.len()
            || self.secret_response.len() != self.smudge_response.len()
        {
            return Err(ZkAmsMkheErrorV1::InvalidCksProof);
        }
        cks_proof_bytes(
            self.secret_response.len(),
            usize::from(self.wide_response_bytes),
        )
    }

    /// Encode the sole canonical `ZACP` proof layout.
    pub fn encode(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        let length = self.encoded_len()?;
        let degree = self.secret_response.len();
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        bytes.extend_from_slice(&CKS_PROOF_TAG_V1);
        bytes.push(MKHE_VERSION_V1);
        bytes.extend_from_slice(&self.wide_response_bytes.to_be_bytes());
        bytes.extend_from_slice(
            &u32::try_from(degree)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?
                .to_be_bytes(),
        );
        bytes.extend_from_slice(&self.challenge_seed);
        for count in [
            self.secret_response.len(),
            self.public_key_error_response.len(),
            self.smudge_response.len(),
        ] {
            bytes.extend_from_slice(
                &u32::try_from(count)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?
                    .to_be_bytes(),
            );
        }
        for value in &self.secret_response {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        for value in &self.public_key_error_response {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        for value in &self.smudge_response {
            value.encode_fixed_into(&mut bytes, usize::from(self.wide_response_bytes))?;
        }
        if bytes.len() != length {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(bytes)
    }

    fn decode_exact(
        bytes: &[u8],
        expected_degree: usize,
        expected_wide_response_bytes: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.len() != cks_proof_bytes(expected_degree, expected_wide_response_bytes)? {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut cursor = 0;
        expect(&mut cursor, bytes, &CKS_PROOF_TAG_V1)?;
        if read_array::<1>(&mut cursor, bytes)?[0] != MKHE_VERSION_V1
            || usize::from(u16::from_be_bytes(read_array(&mut cursor, bytes)?))
                != expected_wide_response_bytes
            || usize::try_from(u32::from_be_bytes(read_array(&mut cursor, bytes)?)).ok()
                != Some(expected_degree)
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let challenge_seed = read_array::<32>(&mut cursor, bytes)?;
        if challenge_seed == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        for _ in 0..3 {
            if usize::try_from(u32::from_be_bytes(read_array(&mut cursor, bytes)?)).ok()
                != Some(expected_degree)
            {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
        }
        let mut secret_response = Vec::new();
        let mut public_key_error_response = Vec::new();
        let mut smudge_response = Vec::new();
        for target in [&mut secret_response, &mut public_key_error_response] {
            target
                .try_reserve_exact(expected_degree)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            for _ in 0..expected_degree {
                target.push(i64::from_be_bytes(read_array(&mut cursor, bytes)?));
            }
        }
        smudge_response
            .try_reserve_exact(expected_degree)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for _ in 0..expected_degree {
            let end = cursor
                .checked_add(expected_wide_response_bytes)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            smudge_response.push(SignedWideV1::decode_fixed(
                bytes
                    .get(cursor..end)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            )?);
            cursor = end;
        }
        if cursor != bytes.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(Self {
            wide_response_bytes: u16::try_from(expected_wide_response_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            challenge_seed,
            secret_response,
            public_key_error_response,
            smudge_response,
        })
    }

    /// Decode the exact release-profile proof shape after complete length preflight.
    pub fn decode_release_exact(bytes: &[u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        let evidence = zk_ams_mkhe_cks_resource_evidence_v1()?;
        Self::decode_exact(
            bytes,
            usize::try_from(evidence.ring_degree).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
            usize::from(evidence.wide_response_coefficient_bytes),
        )
    }
}

/// One proof-bound and authentication-key-bound CKS contribution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheAuthenticatedCksContributionV1 {
    binding: CksBindingV1,
    contribution: RnsPolynomial,
    proof: ZkAmsMkheCksProofV1,
    authentication: ArtifactAuthentication,
}

/// Exact release proof and contribution-record size evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCksResourceEvidenceV1 {
    /// Frozen release ring degree.
    pub ring_degree: u32,
    /// Exact per-party CKS smudge quotient width.
    pub smudge_quotient_bits: u16,
    /// Sparse proof challenge weight.
    pub challenge_weight: u8,
    /// Fixed bytes for one signed wide response coefficient.
    pub wide_response_coefficient_bytes: u16,
    /// Exact proof payload bytes.
    pub proof_payload_bytes: u64,
    /// Independent governed proof payload ceiling.
    pub governed_proof_payload_ceiling_bytes: u64,
    /// True only when the proof payload fits its independent ceiling.
    pub proof_payload_ceiling_met: bool,
    /// Exact complete canonical `ZACK` record bytes.
    pub total_contribution_record_bytes: u64,
    /// Governed per-round record ceiling.
    pub governed_contribution_ceiling_bytes: u64,
    /// Exact headroom under the governed round ceiling.
    pub contribution_headroom_bytes: u64,
    /// Whether the exact release record fits every governed CKS ceiling.
    pub contribution_ceiling_met: bool,
    /// Digest of every accounting field and proof domain.
    pub evidence_digest: [u8; 32],
}

/// Deterministic reason for rejecting an ordered CKS contribution set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheCksAbortReasonV1 {
    /// A required governed contribution was absent.
    MissingContribution = 1,
    /// More than the exact governed roster size was supplied.
    ExcessContribution = 2,
    /// The first offending contribution was duplicated or reordered.
    ReorderedOrDuplicateContribution = 3,
    /// A profile, roster, epoch, transcript, source, key context, index, or level differed.
    BindingMismatch = 4,
    /// The party authentication failed.
    AuthenticationFailure = 5,
    /// The native public-key/CKS relation proof failed.
    ProofFailure = 6,
}

/// First governed roster slot rejected during CKS combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheIdentifiableCksAbortV1 {
    /// Zero-based first rejected input slot. The first excess slot is exactly the roster length.
    pub party_index: u8,
    /// Governed party expected at the rejected slot, or `None` for an excess slot.
    pub expected_party: Option<ZkAmsMkhePartyIdV1>,
    /// Party observed in the supplied record, when one was present.
    pub observed_party: Option<ZkAmsMkhePartyIdV1>,
    /// Deterministic rejection reason.
    pub reason: ZkAmsMkheCksAbortReasonV1,
    /// Digest binding the rejected evidence and statement.
    pub evidence_digest: [u8; 32],
}

/// Return exact governed release accounting for one CKS contribution.
pub fn zk_ams_mkhe_cks_resource_evidence_v1()
-> Result<ZkAmsMkheCksResourceEvidenceV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let noise = zk_ams_mkhe_noise_certificate_v1()?;
    let smudge_bits = usize::from(noise.cks_smudge_quotient_bits);
    let challenge_weight = wide_relation_challenge_weight(profile.ring_degree)?;
    let (_, _, wide_response_bytes) = wide_response_parameters(smudge_bits, challenge_weight)?;
    let proof_payload_bytes = cks_proof_bytes(profile.ring_degree, wide_response_bytes)?;
    let lengths = derive_wire_length_certificate_v1(&profile)?;
    let total = lengths
        .streamed_contribution_base_wire_bytes
        .checked_add(lengths.proof_envelope_header_wire_bytes)
        .and_then(|value| value.checked_add(proof_payload_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut evidence = ZkAmsMkheCksResourceEvidenceV1 {
        ring_degree: u32::try_from(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        smudge_quotient_bits: noise.cks_smudge_quotient_bits,
        challenge_weight: u8::try_from(challenge_weight)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        wide_response_coefficient_bytes: u16::try_from(wide_response_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        proof_payload_bytes: u64::try_from(proof_payload_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        governed_proof_payload_ceiling_bytes: ZK_AMS_MKHE_MAX_PROOF_BYTES_V1 as u64,
        proof_payload_ceiling_met: proof_payload_bytes <= ZK_AMS_MKHE_MAX_PROOF_BYTES_V1,
        total_contribution_record_bytes: u64::try_from(total)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        governed_contribution_ceiling_bytes: profile.max_round_bytes as u64,
        contribution_headroom_bytes: (profile.max_round_bytes as u64).saturating_sub(total as u64),
        contribution_ceiling_met: total <= profile.max_round_bytes,
        evidence_digest: [0; 32],
    };
    evidence.evidence_digest = cks_resource_digest(evidence);
    Ok(evidence)
}

fn cks_proof_bytes(
    ring_degree: usize,
    wide_response_bytes: usize,
) -> Result<usize, ZkAmsMkheErrorV1> {
    ring_degree
        .checked_mul(CKS_SIGNED_SMALL_BYTES_V1 * 2 + wide_response_bytes)
        .and_then(|value| value.checked_add(CKS_PROOF_HEADER_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn cks_resource_digest(evidence: ZkAmsMkheCksResourceEvidenceV1) -> [u8; 32] {
    let mut frame = Vec::with_capacity(256);
    frame.extend_from_slice(CKS_RESOURCE_DOMAIN_V1);
    frame.extend_from_slice(&CKS_PROOF_TAG_V1);
    frame.extend_from_slice(CKS_SOURCE_DOMAIN_V1);
    frame.extend_from_slice(CKS_KEY_CONTEXT_DOMAIN_V1);
    frame.extend_from_slice(CKS_BINDING_DOMAIN_V1);
    frame.extend_from_slice(CKS_AUTH_DOMAIN_V1);
    frame.extend_from_slice(&WIDE_RELATION_MASK_SLACK_LOG2_V1.to_be_bytes());
    frame.extend_from_slice(&evidence.ring_degree.to_be_bytes());
    frame.extend_from_slice(&evidence.smudge_quotient_bits.to_be_bytes());
    frame.push(evidence.challenge_weight);
    frame.extend_from_slice(&evidence.wide_response_coefficient_bytes.to_be_bytes());
    for value in [
        evidence.proof_payload_bytes,
        evidence.governed_proof_payload_ceiling_bytes,
        evidence.total_contribution_record_bytes,
        evidence.governed_contribution_ceiling_bytes,
        evidence.contribution_headroom_bytes,
    ] {
        frame.extend_from_slice(&value.to_be_bytes());
    }
    frame.push(evidence.proof_payload_ceiling_met.into());
    frame.push(evidence.contribution_ceiling_met.into());
    keccak256(&frame)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CksBindingV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    source_digest: [u8; 32],
    key_context_digest: [u8; 32],
    source_record_index: u32,
    sample_index: u64,
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    level: u8,
}

impl CksBindingV1 {
    fn update_hash(&self, hash: &mut Keccak256) {
        hash.update(CKS_BINDING_DOMAIN_V1);
        hash.update(&self.profile_digest);
        hash.update(&self.roster_digest);
        hash.update(&self.epoch.to_be_bytes());
        hash.update(&self.transcript_digest);
        hash.update(&self.source_digest);
        hash.update(&self.key_context_digest);
        hash.update(&self.source_record_index.to_be_bytes());
        hash.update(&self.sample_index.to_be_bytes());
        hash.update(&[self.party_index]);
        hash.update(&self.party.to_bytes());
        hash.update(&[self.level]);
    }
}

fn wire_polynomial_digest(
    polynomial: &ZkAmsMkheRnsPolynomialWireV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    polynomial.encoded_len()?;
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-polynomial-digest");
    hash.update(
        &u32::try_from(polynomial.residues().len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?
            .to_be_bytes(),
    );
    for residue in polynomial.residues() {
        hash.update(&residue.to_be_bytes());
    }
    Ok(hash.finalize())
}

fn zero_polynomial_digest(profile: &BgvProfile) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-polynomial-digest");
    hash.update(
        &u32::try_from(count)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?
            .to_be_bytes(),
    );
    let zeroes = [0_u8; 4096];
    let mut remaining = count
        .checked_mul(8)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    while remaining != 0 {
        let take = remaining.min(zeroes.len());
        hash.update(&zeroes[..take]);
        remaining -= take;
    }
    Ok(hash.finalize())
}

fn key_context_digest(
    target_a: &ZkAmsMkheRnsPolynomialWireV1,
    public_key_a: &ZkAmsMkheRnsPolynomialWireV1,
    party_public_b: &[&ZkAmsMkheRnsPolynomialWireV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(CKS_KEY_CONTEXT_DOMAIN_V1);
    hash.update(&wire_polynomial_digest(target_a)?);
    hash.update(&wire_polynomial_digest(public_key_a)?);
    for polynomial in party_public_b {
        hash.update(&wire_polynomial_digest(polynomial)?);
    }
    Ok(hash.finalize())
}

fn expect(cursor: &mut usize, bytes: &[u8], expected: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
    let end = cursor
        .checked_add(expected.len())
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if bytes.get(*cursor..end) != Some(expected) {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    *cursor = end;
    Ok(())
}

fn read_array<const N: usize>(
    cursor: &mut usize,
    bytes: &[u8],
) -> Result<[u8; N], ZkAmsMkheErrorV1> {
    let end = cursor
        .checked_add(N)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    *cursor = end;
    Ok(value)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CksRelationV1 {
    binding: CksBindingV1,
    public_key_a: RnsPolynomial,
    party_public_b: RnsPolynomial,
    source_component: RnsPolynomial,
    target_a: RnsPolynomial,
}

impl CksRelationV1 {
    fn validate(
        &self,
        profile: &BgvProfile,
        parties: &super::PartySet,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let index = usize::from(self.binding.party_index);
        if self.binding.profile_digest != profile.digest()?
            || self.binding.roster_digest
                != super::wire::governed_roster_digest(
                    self.binding.profile_digest,
                    self.binding.epoch,
                    &parties.parties,
                )
            || self.binding.epoch == 0
            || self.binding.transcript_digest == [0; 32]
            || self.binding.source_digest == [0; 32]
            || self.binding.key_context_digest == [0; 32]
            || self.binding.level > 1
            || parties.parties.get(index) != Some(&self.binding.party)
        {
            return Err(ZkAmsMkheErrorV1::InvalidCksSet);
        }
        for value in [
            &self.public_key_a,
            &self.party_public_b,
            &self.source_component,
            &self.target_a,
        ] {
            value.validate(profile)?;
        }
        if self.public_key_a == RnsPolynomial::zero(profile)
            || self.party_public_b == RnsPolynomial::zero(profile)
            || self.target_a == RnsPolynomial::zero(profile)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    fn multiplier(&self, profile: &BgvProfile) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
        self.source_component.sub(&self.target_a, profile)
    }
}

struct CksPartyWitnessV1<'a> {
    secret: &'a SecretPolynomial,
    public_key_error: &'a SecretPolynomial,
}

impl core::fmt::Debug for CksPartyWitnessV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("CksPartyWitnessV1([REDACTED])")
    }
}

fn validate_witness(
    profile: &BgvProfile,
    parties: &super::PartySet,
    relation: &CksRelationV1,
    witness: &CksPartyWitnessV1<'_>,
) -> Result<(), ZkAmsMkheErrorV1> {
    relation.validate(profile, parties)?;
    if witness.secret.coefficients.len() != profile.ring_degree
        || witness.public_key_error.coefficients.len() != profile.ring_degree
        || witness
            .secret
            .coefficients
            .iter()
            .any(|value| value.unsigned_abs() > 1)
        || witness
            .public_key_error
            .coefficients
            .iter()
            .any(|value| value.unsigned_abs() > u64::from(profile.error_eta))
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let expected_b = relation
        .public_key_a
        .mul(&witness.secret.as_rns(profile)?, profile)?
        .negate(profile)?
        .add(
            &witness
                .public_key_error
                .as_rns(profile)?
                .scale_plaintext_modulus(profile)?,
            profile,
        )?;
    if expected_b != relation.party_public_b {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn native_polynomial_digest(
    profile: &BgvProfile,
    polynomial: &RnsPolynomial,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    polynomial.validate(profile)?;
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-polynomial-digest");
    hash.update(
        &u32::try_from(polynomial.coefficients.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?
            .to_be_bytes(),
    );
    for residue in &polynomial.coefficients {
        hash.update(&residue.to_be_bytes());
    }
    Ok(hash.finalize())
}

fn cks_challenge_seed(
    profile: &BgvProfile,
    relation: &CksRelationV1,
    contribution: &RnsPolynomial,
    public_key_commitment: &RnsPolynomial,
    contribution_commitment: &RnsPolynomial,
    smudge_bits: usize,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(CKS_CHALLENGE_DOMAIN_V1);
    hash.update(CKS_PROOF_DOMAIN_V1);
    hash.update(
        &u16::try_from(smudge_bits)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    hash.update(
        &u16::try_from(wide_relation_challenge_weight(profile.ring_degree)?)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    hash.update(&WIDE_RELATION_MASK_SLACK_LOG2_V1.to_be_bytes());
    relation.binding.update_hash(&mut hash);
    for polynomial in [
        &relation.public_key_a,
        &relation.party_public_b,
        &relation.source_component,
        &relation.target_a,
        contribution,
        public_key_commitment,
        contribution_commitment,
    ] {
        hash.update(&native_polynomial_digest(profile, polynomial)?);
    }
    Ok(hash.finalize())
}

fn derive_cks_sparse_challenge(
    ring_degree: usize,
    challenge_seed: [u8; 32],
) -> Result<Vec<i8>, ZkAmsMkheErrorV1> {
    if challenge_seed == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidCksProof);
    }
    let weight = wide_relation_challenge_weight(ring_degree)?;
    let mut frame = Vec::with_capacity(96);
    frame.extend_from_slice(CKS_SPARSE_CHALLENGE_DOMAIN_V1);
    frame.extend_from_slice(&challenge_seed);
    frame.extend_from_slice(
        &u32::try_from(ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    frame.extend_from_slice(
        &u32::try_from(weight)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    let stream = shake256(
        &frame,
        weight
            .checked_mul(MAX_RANDOM_REJECTION_ATTEMPTS_V1)
            .and_then(|value| value.checked_mul(8))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    );
    let mask = u64::try_from(ring_degree - 1).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut challenge = vec![0_i8; ring_degree];
    let mut selected = 0;
    for chunk in stream.chunks_exact(8) {
        let word = u64::from_le_bytes(
            chunk
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidCksProof)?,
        );
        let position =
            usize::try_from(word & mask).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
        if challenge[position] != 0 {
            continue;
        }
        challenge[position] = if word >> 63 == 0 { -1 } else { 1 };
        selected += 1;
        if selected == weight {
            return Ok(challenge);
        }
    }
    Err(ZkAmsMkheErrorV1::InvalidCksProof)
}

#[allow(clippy::too_many_arguments)]
fn prove_cks_relation<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    parties: &super::PartySet,
    relation: &CksRelationV1,
    witness: &CksPartyWitnessV1<'_>,
    contribution: &RnsPolynomial,
    smudge: &[SignedWideV1],
    smudge_bits: usize,
    random: &mut R,
) -> Result<ZkAmsMkheCksProofV1, ZkAmsMkheErrorV1> {
    validate_witness(profile, parties, relation, witness)?;
    contribution.validate(profile)?;
    if smudge.len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidCksProof);
    }
    let smudge_bound = WideMagnitudeV1::max_for_bits(smudge_bits)?;
    if smudge.iter().any(|value| value.magnitude > smudge_bound) {
        return Err(ZkAmsMkheErrorV1::InvalidCksProof);
    }
    let expected = relation
        .multiplier(profile)?
        .mul(&witness.secret.as_rns(profile)?, profile)?
        .add(
            &wide_vector_as_rns(profile, smudge)?.scale_plaintext_modulus(profile)?,
            profile,
        )?;
    if expected != *contribution {
        return Err(ZkAmsMkheErrorV1::InvalidCksProof);
    }

    let weight = wide_relation_challenge_weight(profile.ring_degree)?;
    let (secret_mask_bound, secret_response_limit) = small_response_parameters(1, weight, profile)?;
    let (error_mask_bound, error_response_limit) =
        small_response_parameters(i64::from(profile.error_eta), weight, profile)?;
    let (wide_mask_bound, wide_response_limit, wide_response_bytes) =
        wide_response_parameters(smudge_bits, weight)?;
    if cks_proof_bytes(profile.ring_degree, wide_response_bytes)? > ZK_AMS_MKHE_MAX_PROOF_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    checked_ring_multiplication_work(profile, 8)?;
    validate_wide_relation_random_health(random)?;

    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let mut secret_mask = (0..profile.ring_degree)
            .map(|_| sample_signed_small(secret_mask_bound, random))
            .collect::<Result<Vec<_>, _>>()?;
        let mut error_mask = (0..profile.ring_degree)
            .map(|_| sample_signed_small(error_mask_bound, random))
            .collect::<Result<Vec<_>, _>>()?;
        let mut smudge_mask = (0..profile.ring_degree)
            .map(|_| sample_signed_wide(&wide_mask_bound, random))
            .collect::<Result<Vec<_>, _>>()?;
        let secret_mask_rns = RnsPolynomial::from_signed(profile, &secret_mask)?;
        let error_mask_rns = RnsPolynomial::from_signed(profile, &error_mask)?;
        let smudge_mask_rns = wide_vector_as_rns(profile, &smudge_mask)?;
        let public_key_commitment = relation
            .public_key_a
            .mul(&secret_mask_rns, profile)?
            .negate(profile)?
            .add(&error_mask_rns.scale_plaintext_modulus(profile)?, profile)?;
        let contribution_commitment = relation
            .multiplier(profile)?
            .mul(&secret_mask_rns, profile)?
            .add(&smudge_mask_rns.scale_plaintext_modulus(profile)?, profile)?;
        let challenge_seed = cks_challenge_seed(
            profile,
            relation,
            contribution,
            &public_key_commitment,
            &contribution_commitment,
            smudge_bits,
        )?;
        if challenge_seed == [0; 32] {
            secret_mask.fill(0);
            error_mask.fill(0);
            smudge_mask.clear();
            continue;
        }
        let challenge = derive_cks_sparse_challenge(profile.ring_degree, challenge_seed)?;
        let folded_secret = sparse_negacyclic_mul_small(&challenge, &witness.secret.coefficients)?;
        let folded_error =
            sparse_negacyclic_mul_small(&challenge, &witness.public_key_error.coefficients)?;
        let folded_smudge = sparse_negacyclic_mul_wide(&challenge, smudge)?;
        let mut accepted = true;
        let mut secret_response = Vec::with_capacity(profile.ring_degree);
        let mut public_key_error_response = Vec::with_capacity(profile.ring_degree);
        let mut smudge_response = Vec::with_capacity(profile.ring_degree);
        for (mask, folded) in secret_mask.iter().copied().zip(folded_secret) {
            let response = mask
                .checked_add(folded)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            accepted &= response.unsigned_abs() <= secret_response_limit as u64;
            secret_response.push(response);
        }
        for (mask, folded) in error_mask.iter().copied().zip(folded_error) {
            let response = mask
                .checked_add(folded)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            accepted &= response.unsigned_abs() <= error_response_limit as u64;
            public_key_error_response.push(response);
        }
        for (mask, folded) in smudge_mask.iter().zip(folded_smudge) {
            let response = mask
                .checked_add(&folded)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            accepted &= response.magnitude <= wide_response_limit;
            smudge_response.push(response);
        }
        secret_mask.fill(0);
        error_mask.fill(0);
        smudge_mask.clear();
        if !accepted {
            continue;
        }
        let proof = ZkAmsMkheCksProofV1 {
            wide_response_bytes: u16::try_from(wide_response_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            challenge_seed,
            secret_response,
            public_key_error_response,
            smudge_response,
        };
        verify_cks_relation(
            profile,
            parties,
            relation,
            contribution,
            smudge_bits,
            &proof,
        )?;
        return Ok(proof);
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

fn verify_cks_relation(
    profile: &BgvProfile,
    parties: &super::PartySet,
    relation: &CksRelationV1,
    contribution: &RnsPolynomial,
    smudge_bits: usize,
    proof: &ZkAmsMkheCksProofV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    relation.validate(profile, parties)?;
    contribution.validate(profile)?;
    if proof.challenge_seed == [0; 32]
        || proof.secret_response.len() != profile.ring_degree
        || proof.public_key_error_response.len() != profile.ring_degree
        || proof.smudge_response.len() != profile.ring_degree
    {
        return Err(ZkAmsMkheErrorV1::InvalidCksProof);
    }
    let weight = wide_relation_challenge_weight(profile.ring_degree)?;
    let (_, secret_response_limit) = small_response_parameters(1, weight, profile)?;
    let (_, error_response_limit) =
        small_response_parameters(i64::from(profile.error_eta), weight, profile)?;
    let (_, wide_response_limit, wide_response_bytes) =
        wide_response_parameters(smudge_bits, weight)?;
    if usize::from(proof.wide_response_bytes) != wide_response_bytes
        || proof
            .secret_response
            .iter()
            .any(|value| value.unsigned_abs() > secret_response_limit as u64)
        || proof
            .public_key_error_response
            .iter()
            .any(|value| value.unsigned_abs() > error_response_limit as u64)
        || proof
            .smudge_response
            .iter()
            .any(|value| value.magnitude > wide_response_limit)
    {
        return Err(ZkAmsMkheErrorV1::InvalidCksProof);
    }
    checked_ring_multiplication_work(profile, 8)?;
    let secret_response = RnsPolynomial::from_signed(profile, &proof.secret_response)?;
    let error_response = RnsPolynomial::from_signed(profile, &proof.public_key_error_response)?;
    let smudge_response = wide_vector_as_rns(profile, &proof.smudge_response)?;
    let challenge = derive_cks_sparse_challenge(profile.ring_degree, proof.challenge_seed)?;
    let challenge_rns = RnsPolynomial::from_signed(
        profile,
        &challenge
            .iter()
            .map(|value| i64::from(*value))
            .collect::<Vec<_>>(),
    )?;
    let public_key_commitment = relation
        .public_key_a
        .mul(&secret_response, profile)?
        .negate(profile)?
        .add(&error_response.scale_plaintext_modulus(profile)?, profile)?
        .sub(
            &relation.party_public_b.mul(&challenge_rns, profile)?,
            profile,
        )?;
    let contribution_commitment = relation
        .multiplier(profile)?
        .mul(&secret_response, profile)?
        .add(&smudge_response.scale_plaintext_modulus(profile)?, profile)?
        .sub(&contribution.mul(&challenge_rns, profile)?, profile)?;
    let expected = cks_challenge_seed(
        profile,
        relation,
        contribution,
        &public_key_commitment,
        &contribution_commitment,
        smudge_bits,
    )?;
    if expected != proof.challenge_seed {
        return Err(ZkAmsMkheErrorV1::InvalidCksProof);
    }
    Ok(())
}

fn create_cks_contribution<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    parties: &super::PartySet,
    relation: &CksRelationV1,
    witness: &CksPartyWitnessV1<'_>,
    smudge_bits: usize,
    random: &mut R,
) -> Result<ZkAmsMkheAuthenticatedCksContributionV1, ZkAmsMkheErrorV1> {
    validate_witness(profile, parties, relation, witness)?;
    validate_wide_relation_random_health(random)?;
    let bound = WideMagnitudeV1::max_for_bits(smudge_bits)?;
    let mut smudge = (0..profile.ring_degree)
        .map(|_| sample_signed_wide(&bound, random))
        .collect::<Result<Vec<_>, _>>()?;
    let contribution = relation
        .multiplier(profile)?
        .mul(&witness.secret.as_rns(profile)?, profile)?
        .add(
            &wide_vector_as_rns(profile, &smudge)?.scale_plaintext_modulus(profile)?,
            profile,
        )?;
    let proof = prove_cks_relation(
        profile,
        parties,
        relation,
        witness,
        &contribution,
        &smudge,
        smudge_bits,
        random,
    )?;
    smudge.clear();
    Ok(ZkAmsMkheAuthenticatedCksContributionV1 {
        binding: relation.binding.clone(),
        contribution,
        proof,
        authentication: ArtifactAuthentication {
            version: MKHE_VERSION_V1,
            party: relation.binding.party,
            public_key: [0; 33],
            signature: [0; 65],
        },
    })
}

impl ZkAmsMkheAuthenticatedCksContributionV1 {
    /// Governed roster index authenticated by this contribution.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.binding.party_index
    }

    /// Authentication-key-derived governed party.
    #[must_use]
    pub const fn party(&self) -> ZkAmsMkhePartyIdV1 {
        self.binding.party
    }

    /// Canonical contribution residues `d_i`.
    #[must_use]
    pub fn contribution_residues(&self) -> &[u64] {
        &self.contribution.coefficients
    }

    /// Native fixed-width proof.
    #[must_use]
    pub const fn proof(&self) -> &ZkAmsMkheCksProofV1 {
        &self.proof
    }

    /// Canonical native proof bytes.
    pub fn canonical_proof_bytes(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        self.proof.encode()
    }

    fn record_digest(&self, profile: &BgvProfile) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let proof = self.proof.encode()?;
        let mut hash = Keccak256::new();
        hash.update(CKS_AUTH_DOMAIN_V1);
        self.binding.update_hash(&mut hash);
        hash.update(&native_polynomial_digest(profile, &self.contribution)?);
        hash.update(
            &u32::try_from(proof.len())
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?
                .to_be_bytes(),
        );
        hash.update(&proof);
        Ok(hash.finalize())
    }

    /// Convert the verified native contribution to the canonical public `ZACK` record.
    pub fn to_release_wire(
        &self,
        statement: ZkAmsMkheCksStatementV1<'_>,
    ) -> Result<ZkAmsMkheCksContributionWireV1, ZkAmsMkheErrorV1> {
        verify_zk_ams_mkhe_cks_contribution_v1(statement, self)?;
        let evidence = zk_ams_mkhe_cks_resource_evidence_v1()?;
        if !evidence.proof_payload_ceiling_met || !evidence.contribution_ceiling_met {
            return Err(ZkAmsMkheErrorV1::WireTooLarge);
        }
        let binding = ZkAmsMkheWireBindingV1::new(
            statement.roster,
            self.binding.transcript_digest,
            u32::from(self.binding.party_index),
            self.binding.level,
        )?;
        let authentication = ZkAmsMkheAuthenticationWireV1::new(
            self.authentication.party,
            self.authentication.public_key,
            self.authentication.signature,
        )?;
        let contribution =
            ZkAmsMkheRnsPolynomialWireV1::new(self.contribution.coefficients.clone())?;
        let statement_digest = zk_ams_mkhe_cks_statement_digest_v1(
            binding,
            self.binding.source_digest,
            self.binding.party,
            &contribution,
        )?;
        let proof_bytes = self.proof.encode()?;
        if u64::try_from(proof_bytes.len()).ok() != Some(evidence.proof_payload_bytes) {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let proof = ZkAmsMkheProofEnvelopeWireV1::new(
            binding,
            ZkAmsMkheProofKindV1::CksContribution,
            statement_digest,
            proof_bytes,
        )?;
        ZkAmsMkheCksContributionWireV1::new(
            statement.roster,
            binding,
            self.binding.source_digest,
            authentication,
            contribution,
            proof,
        )
    }

    /// Convert, authenticate, and verify a decoded canonical public `ZACK` record.
    pub fn from_release_wire(
        statement: ZkAmsMkheCksStatementV1<'_>,
        wire: &ZkAmsMkheCksContributionWireV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        statement.validate()?;
        let index = usize::try_from(wire.binding().record_index())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
        if index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPartySet);
        }
        let expected_binding = ZkAmsMkheWireBindingV1::new(
            statement.roster,
            statement.source.transcript_digest,
            u32::try_from(index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
            statement.source.level,
        )?;
        if wire.binding() != expected_binding
            || wire.source_ciphertext_digest() != statement.source.source_digest
            || wire.authentication().party() != statement.roster.parties()[index]
        {
            return Err(ZkAmsMkheErrorV1::InvalidCksSet);
        }
        let proof = ZkAmsMkheCksProofV1::decode_release_exact(wire.proof().proof_bytes())?;
        let contribution = RnsPolynomial::from_flat(
            &release_profile_v1(),
            wire.contribution().residues().to_vec(),
        )?;
        let value = Self {
            binding: CksBindingV1 {
                profile_digest: statement.roster.profile_digest(),
                roster_digest: statement.roster.roster_digest(),
                epoch: statement.roster.epoch(),
                transcript_digest: statement.source.transcript_digest,
                source_digest: statement.source.source_digest,
                key_context_digest: statement.key_context_digest,
                source_record_index: statement.source.record_index,
                sample_index: statement.source.sample_index,
                party_index: u8::try_from(index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
                party: statement.roster.parties()[index],
                level: statement.source.level,
            },
            contribution,
            proof,
            authentication: ArtifactAuthentication {
                version: MKHE_VERSION_V1,
                party: wire.authentication().party(),
                public_key: wire.authentication().public_key(),
                signature: wire.authentication().signature(),
            },
        };
        verify_zk_ams_mkhe_cks_contribution_v1(statement, &value)?;
        Ok(value)
    }

    /// Decode, bind, authenticate, and verify one exact canonical `ZACK` byte string.
    pub fn decode_release_wire_exact(
        statement: ZkAmsMkheCksStatementV1<'_>,
        party_index: u8,
        bytes: &[u8],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let binding = ZkAmsMkheWireBindingV1::new(
            statement.roster,
            statement.source.transcript_digest,
            u32::from(party_index),
            statement.source.level,
        )?;
        let wire = ZkAmsMkheCksContributionWireV1::decode_exact(
            bytes,
            statement.roster,
            binding,
            statement.source.source_digest,
        )?;
        Self::from_release_wire(statement, &wire)
    }
}

/// Prove and authenticate one governed CKS contribution.
pub fn prove_zk_ams_mkhe_cks_contribution_v1<R: MaskedRelaxedRandomSourceV1>(
    statement: ZkAmsMkheCksStatementV1<'_>,
    party_index: usize,
    party_state: &ZkAmsMkheCollectivePartyStateV1,
    party_secret: &ZkAmsMkheActivePartySecretV1,
    random: &mut R,
) -> Result<ZkAmsMkheAuthenticatedCksContributionV1, ZkAmsMkheErrorV1> {
    statement.validate()?;
    let evidence = zk_ams_mkhe_cks_resource_evidence_v1()?;
    if !evidence.proof_payload_ceiling_met || !evidence.contribution_ceiling_met {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    if party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || party_secret.party()? != statement.roster.parties()[party_index]
        || usize::from(party_state.party_index()) != party_index
        || party_state.party() != statement.roster.parties()[party_index]
        || party_state.profile_digest() != statement.roster.profile_digest()
        || party_state.roster_digest() != statement.roster.roster_digest()
        || party_state.epoch() != statement.roster.epoch()
        || party_state.transcript_digest() != statement.source.transcript_digest
        || party_state.public_share_digest() == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let (profile, parties, relation) = statement.internal_for_party(party_index)?;
    let native_witness = CksPartyWitnessV1 {
        secret: party_state.secret(),
        public_key_error: party_state.public_error(),
    };
    let mut contribution = create_cks_contribution(
        &profile,
        &parties,
        &relation,
        &native_witness,
        usize::from(evidence.smudge_quotient_bits),
        random,
    )?;
    contribution.authentication = party_secret.authenticate_artifact(
        CKS_AUTH_DOMAIN_V1,
        contribution.record_digest(&profile)?,
        random,
    )?;
    verify_zk_ams_mkhe_cks_contribution_v1(statement, &contribution)?;
    Ok(contribution)
}

/// Verify one governed CKS contribution against the complete public statement.
pub fn verify_zk_ams_mkhe_cks_contribution_v1(
    statement: ZkAmsMkheCksStatementV1<'_>,
    contribution: &ZkAmsMkheAuthenticatedCksContributionV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    statement.validate()?;
    let index = usize::from(contribution.binding.party_index);
    if index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || contribution.binding.profile_digest != statement.roster.profile_digest()
        || contribution.binding.roster_digest != statement.roster.roster_digest()
        || contribution.binding.epoch != statement.roster.epoch()
        || contribution.binding.transcript_digest != statement.source.transcript_digest
        || contribution.binding.source_digest != statement.source.source_digest
        || contribution.binding.key_context_digest != statement.key_context_digest
        || contribution.binding.source_record_index != statement.source.record_index
        || contribution.binding.sample_index != statement.source.sample_index
        || contribution.binding.party != statement.roster.parties()[index]
        || contribution.binding.level != statement.source.level
    {
        return Err(ZkAmsMkheErrorV1::InvalidCksSet);
    }
    let (profile, parties, relation) = statement.internal_for_party(index)?;
    contribution
        .authentication
        .verify(CKS_AUTH_DOMAIN_V1, contribution.record_digest(&profile)?)?;
    if contribution.authentication.party != contribution.binding.party {
        return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
    }
    verify_cks_relation(
        &profile,
        &parties,
        &relation,
        &contribution.contribution,
        usize::from(zk_ams_mkhe_cks_resource_evidence_v1()?.smudge_quotient_bits),
        &contribution.proof,
    )
}

/// Verify and combine the exact ordered full-roster CKS contribution set.
pub fn combine_zk_ams_mkhe_cks_v1(
    statement: ZkAmsMkheCksStatementV1<'_>,
    contributions: &[ZkAmsMkheAuthenticatedCksContributionV1],
) -> Result<ZkAmsMkheCollectiveCiphertextWireV1, ZkAmsMkheIdentifiableCksAbortV1> {
    let abort =
        |index: usize, observed: Option<ZkAmsMkhePartyIdV1>, reason: ZkAmsMkheCksAbortReasonV1| {
            identifiable_cks_abort(statement, index, observed, reason)
        };
    statement
        .validate()
        .map_err(|_| abort(0, None, ZkAmsMkheCksAbortReasonV1::BindingMismatch))?;
    if let Some((index, observed, reason)) =
        first_cks_set_shape_error(statement.roster.parties(), contributions)
    {
        return Err(abort(index, observed, reason));
    }
    for (index, contribution) in contributions.iter().enumerate() {
        if let Err(error) = verify_zk_ams_mkhe_cks_contribution_v1(statement, contribution) {
            let reason = match error {
                ZkAmsMkheErrorV1::InvalidAuthentication => {
                    ZkAmsMkheCksAbortReasonV1::AuthenticationFailure
                }
                ZkAmsMkheErrorV1::InvalidCksProof => ZkAmsMkheCksAbortReasonV1::ProofFailure,
                _ => ZkAmsMkheCksAbortReasonV1::BindingMismatch,
            };
            return Err(abort(index, Some(contribution.party()), reason));
        }
    }
    let profile = release_profile_v1();
    let mut constant =
        RnsPolynomial::from_flat(&profile, statement.source.constant.residues().to_vec())
            .map_err(|_| abort(0, None, ZkAmsMkheCksAbortReasonV1::BindingMismatch))?;
    for contribution in contributions {
        constant = constant
            .add(&contribution.contribution, &profile)
            .map_err(|_| abort(0, None, ZkAmsMkheCksAbortReasonV1::BindingMismatch))?;
    }
    let binding = ZkAmsMkheWireBindingV1::new(
        statement.roster,
        statement.source.transcript_digest,
        statement.source.record_index,
        statement.source.level,
    )
    .map_err(|_| abort(0, None, ZkAmsMkheCksAbortReasonV1::BindingMismatch))?;
    ZkAmsMkheCollectiveCiphertextWireV1::new(
        binding,
        statement.source.sample_index,
        ZkAmsMkheRnsPolynomialWireV1::new(constant.coefficients)
            .map_err(|_| abort(0, None, ZkAmsMkheCksAbortReasonV1::BindingMismatch))?,
        statement.target_a.clone(),
    )
    .map_err(|_| abort(0, None, ZkAmsMkheCksAbortReasonV1::BindingMismatch))
}

fn first_cks_set_shape_error(
    expected_parties: &[ZkAmsMkhePartyIdV1],
    contributions: &[ZkAmsMkheAuthenticatedCksContributionV1],
) -> Option<(usize, Option<ZkAmsMkhePartyIdV1>, ZkAmsMkheCksAbortReasonV1)> {
    for (index, contribution) in contributions
        .iter()
        .take(expected_parties.len())
        .enumerate()
    {
        if usize::from(contribution.party_index()) != index
            || contribution.party() != expected_parties[index]
        {
            return Some((
                index,
                Some(contribution.party()),
                ZkAmsMkheCksAbortReasonV1::ReorderedOrDuplicateContribution,
            ));
        }
    }
    if contributions.len() < expected_parties.len() {
        return Some((
            contributions.len(),
            None,
            ZkAmsMkheCksAbortReasonV1::MissingContribution,
        ));
    }
    if contributions.len() > expected_parties.len() {
        return Some((
            expected_parties.len(),
            contributions
                .get(expected_parties.len())
                .map(ZkAmsMkheAuthenticatedCksContributionV1::party),
            ZkAmsMkheCksAbortReasonV1::ExcessContribution,
        ));
    }
    None
}

fn identifiable_cks_abort(
    statement: ZkAmsMkheCksStatementV1<'_>,
    index: usize,
    observed_party: Option<ZkAmsMkhePartyIdV1>,
    reason: ZkAmsMkheCksAbortReasonV1,
) -> ZkAmsMkheIdentifiableCksAbortV1 {
    let party_index = u8::try_from(index).unwrap_or(u8::MAX);
    let expected_party = statement.roster.parties().get(index).copied();
    let mut frame = Vec::with_capacity(256);
    frame.extend_from_slice(b"iroha.zk-ams.v1.mkhe.cks-identifiable-abort");
    frame.extend_from_slice(&statement.roster.profile_digest());
    frame.extend_from_slice(&statement.roster.roster_digest());
    frame.extend_from_slice(&statement.roster.epoch().to_be_bytes());
    frame.extend_from_slice(&statement.source.transcript_digest);
    frame.extend_from_slice(&statement.source.source_digest);
    frame.extend_from_slice(&statement.key_context_digest);
    frame.push(party_index);
    match expected_party {
        Some(party) => {
            frame.push(1);
            frame.extend_from_slice(&party.to_bytes());
        }
        None => frame.push(0),
    }
    match observed_party {
        Some(party) => {
            frame.push(1);
            frame.extend_from_slice(&party.to_bytes());
        }
        None => frame.push(0),
    }
    frame.push(reason as u8);
    ZkAmsMkheIdentifiableCksAbortV1 {
        party_index,
        expected_party,
        observed_party,
        reason,
        evidence_digest: keccak256(&frame),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AuthenticationSecret, PlaintextModulus, reduce_test_polynomial};
    use super::*;
    use crate::vega::MaskedRelaxedRandomErrorV1;

    const TEST_MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
    const TEST_ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];
    const TEST_SMUDGE_BITS: usize = 8;

    fn test_profile() -> BgvProfile {
        BgvProfile {
            profile_id: [0xc5; 32],
            ring_degree: 8,
            moduli: &TEST_MODULI,
            negacyclic_roots: &TEST_ROOTS,
            plaintext_modulus: PlaintextModulus::Tiny(17),
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

    struct KatRandom {
        state: [u8; 32],
        counter: u64,
    }

    impl KatRandom {
        fn new(label: &[u8]) -> Self {
            Self {
                state: keccak256(label),
                counter: 0,
            }
        }
    }

    impl MaskedRelaxedRandomSourceV1 for KatRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            let mut cursor = 0;
            while cursor < destination.len() {
                let mut frame = Vec::with_capacity(40);
                frame.extend_from_slice(&self.state);
                frame.extend_from_slice(&self.counter.to_be_bytes());
                let block = shake256(&frame, 64);
                let take = (destination.len() - cursor).min(block.len());
                destination[cursor..cursor + take].copy_from_slice(&block[..take]);
                cursor += take;
                self.counter = self.counter.wrapping_add(1);
            }
            Ok(())
        }
    }

    struct Fixture {
        profile: BgvProfile,
        parties: super::super::PartySet,
        authentication: Vec<AuthenticationSecret>,
        secrets: Vec<SecretPolynomial>,
        errors: Vec<SecretPolynomial>,
        relations: Vec<CksRelationV1>,
        constant: RnsPolynomial,
    }

    fn fixture() -> Fixture {
        let profile = test_profile();
        let mut random = KatRandom::new(b"zk-ams.cks.tiny.fixture.auth");
        let mut authentication = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map(|_| AuthenticationSecret::generate(&mut random).unwrap())
            .collect::<Vec<_>>();
        authentication.sort_by_key(|value| value.party_id().unwrap());
        let parties = super::super::PartySet::new(
            authentication
                .iter()
                .map(|value| value.party_id().unwrap())
                .collect(),
        )
        .unwrap();
        let public_key_a =
            RnsPolynomial::from_signed(&profile, &[3, -1, 2, 1, 0, -2, 1, 2]).unwrap();
        let target_a = RnsPolynomial::from_signed(&profile, &[2, 1, -1, 3, 1, 0, -2, 1]).unwrap();
        let constant = RnsPolynomial::from_signed(&profile, &[5, 1, 4, 0, -3, 2, 1, 6]).unwrap();
        let source_components = [
            RnsPolynomial::from_signed(&profile, &[4, -2, 0, 1, 3, 1, -1, 2]).unwrap(),
            RnsPolynomial::zero(&profile),
            RnsPolynomial::from_signed(&profile, &[-1, 3, 2, 0, 1, -2, 4, 1]).unwrap(),
            RnsPolynomial::from_signed(&profile, &[2, 0, -3, 1, -1, 2, 0, 1]).unwrap(),
            RnsPolynomial::zero(&profile),
            RnsPolynomial::from_signed(&profile, &[1, 2, 0, -2, 3, -1, 1, 0]).unwrap(),
            RnsPolynomial::zero(&profile),
            RnsPolynomial::from_signed(&profile, &[-2, 1, 3, 0, 2, 1, -1, 4]).unwrap(),
        ];
        let secret_values = [
            [1, 0, -1, 1, 0, 1, -1, 0],
            [0, 1, 0, -1, 1, 0, 1, -1],
            [-1, 1, 1, 0, -1, 0, 0, 1],
            [1, -1, 0, 0, 1, -1, 1, 0],
            [0, 0, 1, 1, -1, 0, -1, 1],
            [-1, 0, 1, -1, 0, 1, 1, 0],
            [1, 1, -1, 0, 0, -1, 0, 1],
            [0, -1, 0, 1, 1, 0, -1, 1],
        ];
        let error_values = [
            [1, -1, 0, 2, -2, 1, 0, -1],
            [0, 1, -2, 1, 0, -1, 2, 0],
            [-1, 0, 1, -1, 2, 0, -2, 1],
            [2, 0, -1, 1, -2, 1, 0, -1],
            [0, -2, 1, 0, 1, -1, 2, 0],
            [-2, 1, 0, -1, 1, 2, 0, -1],
            [1, 0, 2, -2, 0, -1, 1, 0],
            [0, 2, -1, 1, -1, 0, -2, 1],
        ];
        let secrets = secret_values
            .into_iter()
            .map(|coefficients| SecretPolynomial {
                coefficients: coefficients.to_vec(),
            })
            .collect::<Vec<_>>();
        let errors = error_values
            .into_iter()
            .map(|coefficients| SecretPolynomial {
                coefficients: coefficients.to_vec(),
            })
            .collect::<Vec<_>>();
        let source_digest = keccak256(b"zk-ams.cks.tiny.source");
        let key_context_digest = keccak256(b"zk-ams.cks.tiny.key-context");
        let relations = (0..parties.parties.len())
            .map(|index| {
                let party_public_b = public_key_a
                    .mul(&secrets[index].as_rns(&profile).unwrap(), &profile)
                    .unwrap()
                    .negate(&profile)
                    .unwrap()
                    .add(
                        &errors[index]
                            .as_rns(&profile)
                            .unwrap()
                            .scale_plaintext_modulus(&profile)
                            .unwrap(),
                        &profile,
                    )
                    .unwrap();
                CksRelationV1 {
                    binding: CksBindingV1 {
                        profile_digest: profile.digest().unwrap(),
                        roster_digest: super::super::wire::governed_roster_digest(
                            profile.digest().unwrap(),
                            17,
                            &parties.parties,
                        ),
                        epoch: 17,
                        transcript_digest: keccak256(b"zk-ams.cks.tiny.transcript"),
                        source_digest,
                        key_context_digest,
                        source_record_index: 5,
                        sample_index: 9,
                        party_index: u8::try_from(index).unwrap(),
                        party: parties.parties[index],
                        level: 0,
                    },
                    public_key_a: public_key_a.clone(),
                    party_public_b,
                    source_component: source_components[index].clone(),
                    target_a: target_a.clone(),
                }
            })
            .collect();
        Fixture {
            profile,
            parties,
            authentication,
            secrets,
            errors,
            relations,
            constant,
        }
    }

    fn contributions(
        fixture: &Fixture,
        label: &[u8],
    ) -> Vec<ZkAmsMkheAuthenticatedCksContributionV1> {
        let mut random = KatRandom::new(label);
        fixture
            .relations
            .iter()
            .enumerate()
            .map(|(index, relation)| {
                let witness = CksPartyWitnessV1 {
                    secret: &fixture.secrets[index],
                    public_key_error: &fixture.errors[index],
                };
                let mut contribution = create_cks_contribution(
                    &fixture.profile,
                    &fixture.parties,
                    relation,
                    &witness,
                    TEST_SMUDGE_BITS,
                    &mut random,
                )
                .unwrap();
                contribution.authentication = ArtifactAuthentication::sign(
                    CKS_AUTH_DOMAIN_V1,
                    contribution.record_digest(&fixture.profile).unwrap(),
                    &fixture.authentication[index],
                    &mut random,
                )
                .unwrap();
                contribution
            })
            .collect()
    }

    fn verify_native(
        fixture: &Fixture,
        index: usize,
        contribution: &ZkAmsMkheAuthenticatedCksContributionV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        contribution.authentication.verify(
            CKS_AUTH_DOMAIN_V1,
            contribution.record_digest(&fixture.profile)?,
        )?;
        if contribution.authentication.party != fixture.parties.parties[index] {
            return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
        }
        verify_cks_relation(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations[index],
            &contribution.contribution,
            TEST_SMUDGE_BITS,
            &contribution.proof,
        )
    }

    #[test]
    fn tiny_full_roster_cks_relation_oracle_and_kat() {
        let fixture = fixture();
        let contributions = contributions(&fixture, b"zk-ams.cks.tiny.kat");
        for (index, contribution) in contributions.iter().enumerate() {
            verify_native(&fixture, index, contribution).unwrap();
        }
        let mut original = fixture.constant.clone();
        let mut compact = fixture.constant.clone();
        let mut collective_secret = RnsPolynomial::zero(&fixture.profile);
        for (index, relation) in fixture.relations.iter().enumerate() {
            let secret = fixture.secrets[index].as_rns(&fixture.profile).unwrap();
            original = original
                .add(
                    &relation
                        .source_component
                        .mul(&secret, &fixture.profile)
                        .unwrap(),
                    &fixture.profile,
                )
                .unwrap();
            compact = compact
                .add(&contributions[index].contribution, &fixture.profile)
                .unwrap();
            collective_secret = collective_secret.add(&secret, &fixture.profile).unwrap();
        }
        compact = compact
            .add(
                &fixture.relations[0]
                    .target_a
                    .mul(&collective_secret, &fixture.profile)
                    .unwrap(),
                &fixture.profile,
            )
            .unwrap();
        assert_eq!(
            reduce_test_polynomial(&fixture.profile, &original).unwrap(),
            reduce_test_polynomial(&fixture.profile, &compact).unwrap()
        );
        let mut transcript = Vec::new();
        for contribution in &contributions {
            transcript.extend_from_slice(&contribution.proof.encode().unwrap());
            transcript.extend_from_slice(
                &native_polynomial_digest(&fixture.profile, &contribution.contribution).unwrap(),
            );
        }
        transcript
            .extend_from_slice(&native_polynomial_digest(&fixture.profile, &compact).unwrap());
        // Freeze the exact eight-party proof/contribution transcript only after the
        // independently computed source and compact plaintext oracles agree above.
        assert_eq!(
            hex::encode(keccak256(&transcript)),
            "ba0dad2a52a03a43fa2262081f9d1577cdd3eff4689cd64fb5dd5cb6834d9f96"
        );
    }

    #[test]
    fn proof_codec_is_canonical_and_preflights_every_count() {
        let fixture = fixture();
        let contribution = contributions(&fixture, b"zk-ams.cks.codec").remove(0);
        let encoded = contribution.proof.encode().unwrap();
        let decoded = ZkAmsMkheCksProofV1::decode_exact(
            &encoded,
            fixture.profile.ring_degree,
            usize::from(contribution.proof.wide_response_bytes),
        )
        .unwrap();
        assert_eq!(decoded, contribution.proof);
        let mut mutations = Vec::new();
        mutations.push(encoded[..encoded.len() - 1].to_vec());
        let mut trailing = encoded.clone();
        trailing.push(0);
        mutations.push(trailing);
        let mut tag = encoded.clone();
        tag[0] ^= 1;
        mutations.push(tag);
        let mut count = encoded.clone();
        count[43..47].copy_from_slice(&u32::MAX.to_be_bytes());
        mutations.push(count);
        for mutation in mutations {
            assert!(
                ZkAmsMkheCksProofV1::decode_exact(
                    &mutation,
                    fixture.profile.ring_degree,
                    usize::from(contribution.proof.wide_response_bytes),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn wrong_secret_error_and_party_relation_fail_before_proving() {
        let fixture = fixture();
        let mut random = KatRandom::new(b"zk-ams.cks.wrong-witness");
        for (secret_index, error_index) in [(1, 0), (0, 1), (2, 2)] {
            let witness = CksPartyWitnessV1 {
                secret: &fixture.secrets[secret_index],
                public_key_error: &fixture.errors[error_index],
            };
            assert!(
                create_cks_contribution(
                    &fixture.profile,
                    &fixture.parties,
                    &fixture.relations[0],
                    &witness,
                    TEST_SMUDGE_BITS,
                    &mut random,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn every_binding_axis_and_polynomial_splice_invalidates_proof() {
        let fixture = fixture();
        let contribution = contributions(&fixture, b"zk-ams.cks.binding").remove(0);
        let mut relations = Vec::new();
        for axis in 0..13 {
            let mut changed = fixture.relations[0].clone();
            match axis {
                0 => changed.binding.profile_digest[0] ^= 1,
                1 => changed.binding.roster_digest[0] ^= 1,
                2 => changed.binding.epoch += 1,
                3 => changed.binding.transcript_digest[0] ^= 1,
                4 => changed.binding.source_digest[0] ^= 1,
                5 => changed.binding.key_context_digest[0] ^= 1,
                6 => changed.binding.source_record_index += 1,
                7 => changed.binding.sample_index += 1,
                8 => changed.binding.party_index = 1,
                9 => changed.binding.party = fixture.parties.parties[1],
                10 => changed.binding.level = 1,
                11 => changed.source_component = fixture.relations[2].source_component.clone(),
                12 => changed.target_a = fixture.relations[2].source_component.clone(),
                _ => unreachable!(),
            }
            relations.push(changed);
        }
        let mut pk = fixture.relations[0].clone();
        pk.public_key_a = fixture.relations[0].target_a.clone();
        relations.push(pk);
        let mut b = fixture.relations[0].clone();
        b.party_public_b = fixture.relations[1].party_public_b.clone();
        relations.push(b);
        for changed in relations {
            assert!(
                verify_cks_relation(
                    &fixture.profile,
                    &fixture.parties,
                    &changed,
                    &contribution.contribution,
                    TEST_SMUDGE_BITS,
                    &contribution.proof,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn contribution_proof_and_authentication_mutations_fail_closed() {
        let fixture = fixture();
        let contribution = contributions(&fixture, b"zk-ams.cks.mutations").remove(0);
        let mut changed = contribution.clone();
        changed.contribution.coefficients[0] =
            (changed.contribution.coefficients[0] + 1) % fixture.profile.moduli[0];
        assert!(verify_native(&fixture, 0, &changed).is_err());

        let mut challenge = contribution.clone();
        challenge.proof.challenge_seed[0] ^= 1;
        assert!(verify_native(&fixture, 0, &challenge).is_err());
        let mut secret = contribution.clone();
        secret.proof.secret_response[0] += 1;
        assert!(verify_native(&fixture, 0, &secret).is_err());
        let mut error = contribution.clone();
        error.proof.public_key_error_response[0] += 1;
        assert!(verify_native(&fixture, 0, &error).is_err());
        let mut wide = contribution.clone();
        wide.proof.smudge_response[0] = wide.proof.smudge_response[0]
            .checked_add(&SignedWideV1::from_i64(1))
            .unwrap();
        assert!(verify_native(&fixture, 0, &wide).is_err());
        let mut auth = contribution.clone();
        auth.authentication.signature[64] ^= 1;
        assert!(verify_native(&fixture, 0, &auth).is_err());
        let mut auth_key = contribution.clone();
        auth_key.authentication.public_key = fixture.authentication[1].public_key().unwrap();
        assert!(verify_native(&fixture, 0, &auth_key).is_err());
    }

    #[test]
    fn missing_duplicate_reordered_and_excess_sets_are_identified_first() {
        let fixture = fixture();
        let contributions = contributions(&fixture, b"zk-ams.cks.set-order");
        let inspect = |values: &[ZkAmsMkheAuthenticatedCksContributionV1]| {
            first_cks_set_shape_error(&fixture.parties.parties, values)
                .map(|(index, _, reason)| (index, reason))
                .unwrap_or((usize::MAX, ZkAmsMkheCksAbortReasonV1::ProofFailure))
        };
        assert_eq!(
            inspect(&contributions[..2]),
            (2, ZkAmsMkheCksAbortReasonV1::MissingContribution)
        );
        let mut duplicate = contributions.clone();
        duplicate[1] = duplicate[0].clone();
        assert_eq!(
            inspect(&duplicate),
            (
                1,
                ZkAmsMkheCksAbortReasonV1::ReorderedOrDuplicateContribution
            )
        );
        let mut reordered = contributions.clone();
        reordered.swap(0, 1);
        assert_eq!(
            inspect(&reordered),
            (
                0,
                ZkAmsMkheCksAbortReasonV1::ReorderedOrDuplicateContribution
            )
        );
        let mut reordered_and_missing = contributions[..2].to_vec();
        reordered_and_missing.swap(0, 1);
        assert_eq!(
            inspect(&reordered_and_missing),
            (
                0,
                ZkAmsMkheCksAbortReasonV1::ReorderedOrDuplicateContribution
            )
        );
        let mut excess = contributions.clone();
        excess.push(contributions[0].clone());
        assert_eq!(
            inspect(&excess),
            (
                ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
                ZkAmsMkheCksAbortReasonV1::ExcessContribution
            )
        );
    }

    #[test]
    fn release_resource_evidence_is_exact_and_below_both_ceilings() {
        let evidence = zk_ams_mkhe_cks_resource_evidence_v1().unwrap();
        assert_eq!(evidence.smudge_quotient_bits, 151);
        assert_eq!(evidence.challenge_weight, 20);
        assert_eq!(evidence.wide_response_coefficient_bytes, 23);
        assert_eq!(evidence.proof_payload_bytes, 5_111_863);
        assert_eq!(evidence.total_contribution_record_bytes, 44_958_187);
        assert!(evidence.proof_payload_ceiling_met);
        assert!(evidence.contribution_ceiling_met);
        assert_eq!(evidence.contribution_headroom_bytes, 22_150_677);
        assert_ne!(evidence.evidence_digest, [0; 32]);
        assert_eq!(evidence.evidence_digest, cks_resource_digest(evidence));
    }

    #[test]
    fn wide_and_small_response_boundaries_reject_malformed_proofs() {
        let fixture = fixture();
        let contribution = contributions(&fixture, b"zk-ams.cks.bounds").remove(0);
        let weight = wide_relation_challenge_weight(fixture.profile.ring_degree).unwrap();
        let (_, secret_limit) = small_response_parameters(1, weight, &fixture.profile).unwrap();
        let (_, error_limit) = small_response_parameters(
            i64::from(fixture.profile.error_eta),
            weight,
            &fixture.profile,
        )
        .unwrap();
        let (_, wide_limit, _) = wide_response_parameters(TEST_SMUDGE_BITS, weight).unwrap();
        let mut secret = contribution.proof.clone();
        secret.secret_response[0] = secret_limit + 1;
        let mut error = contribution.proof.clone();
        error.public_key_error_response[0] = error_limit + 1;
        let mut wide = contribution.proof.clone();
        wide.smudge_response[0] = SignedWideV1::new(
            false,
            wide_limit
                .checked_add(&WideMagnitudeV1 {
                    limbs: {
                        let mut limbs = [0; 32];
                        limbs[0] = 1;
                        limbs
                    },
                })
                .unwrap(),
        )
        .unwrap();
        for proof in [secret, error, wide] {
            assert!(
                verify_cks_relation(
                    &fixture.profile,
                    &fixture.parties,
                    &fixture.relations[0],
                    &contribution.contribution,
                    TEST_SMUDGE_BITS,
                    &proof,
                )
                .is_err()
            );
        }
    }
}
