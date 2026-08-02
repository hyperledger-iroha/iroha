//! Proof-carrying generation and roster-independent evaluation of compact collective keys.
//!
//! Each online key contains exactly two polynomials per balanced gadget digit.
//! Relinearization digits encrypt `g^d S^2`; Galois digits encrypt
//! `g^d sigma_k(S)`, where `S` is the exact eight-party sum secret.  Generation
//! retains the full authenticated source topology and compacts every digit with
//! the native full-roster CKS protocol.  Online evaluation therefore performs
//! exactly two ring multiplications per digit, independent of roster size.

use super::{
    BgvProfile, MAX_RANDOM_REJECTION_ATTEMPTS_V1, MKHE_VERSION_V1,
    MaskedRelaxedRandomSourceV1, PartySet, RnsPolynomial, SecretPolynomial,
    ZkAmsMkheErrorV1, checked_coefficient_work, checked_ring_multiplication_work,
    derive_rkg_common_a, derive_uniform_rns_from_context, gadget_decompose,
    active::{
        ZkAmsMkheActiveCollectivePublicKeyStatementV1,
        ZkAmsMkheActiveGaloisSourceStatementV1,
        ZkAmsMkheActiveGaloisSourceWitnessV1, ZkAmsMkheActivePartySecretV1,
        ZkAmsMkheActiveRkgProofV1, ZkAmsMkheActiveRkgRoundOneStatementV1,
        ZkAmsMkheActiveRkgRoundOneWitnessV1, ZkAmsMkheActiveRkgRoundTwoStatementV1,
        ZkAmsMkheActiveRkgRoundTwoWitnessV1, ZkAmsMkheGovernedActiveRosterV1,
        prove_zk_ams_mkhe_active_galois_source_v1,
        prove_zk_ams_mkhe_active_rkg_round_one_v1,
        prove_zk_ams_mkhe_active_rkg_round_two_v1,
        verify_zk_ams_mkhe_active_galois_source_v1,
    },
    cks::{
        ZkAmsMkheAuthenticatedCksContributionV1, ZkAmsMkheCksSourceCiphertextV1,
        ZkAmsMkheCksStatementV1, combine_zk_ams_mkhe_cks_v1,
        prove_zk_ams_mkhe_cks_contribution_v1,
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
    manifest::{
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1,
        zk_ams_mkhe_release_manifest_v1,
    },
    packing::{
        ZK_AMS_T256_GALOIS_KEY_COUNT_V1, validate_zk_ams_t256_galois_key_schedule_v1,
        zk_ams_t256_galois_key_schedule_v1,
    },
    wire::{
        ZkAmsMkheGovernedRosterWireV1, ZkAmsMkheRnsPolynomialWireV1,
        ZkAmsMkheSeededRkgKeyWireV1, ZkAmsMkheWireBindingV1,
    },
};
use crate::vega::sponge::{Keccak256, keccak256};

const EVALUATED_KEY_TARGET_A_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-target-a";
const EVALUATED_KEY_EVIDENCE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-evidence";
const EVALUATED_KEY_LINEAGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-lineage";
const RELINEARIZATION_SOURCE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-relinearization-source";
const GALOIS_SOURCE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-galois-source";

/// One generated, canonical seeded compact key and its exact evidence identities.
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// Streaming audit sink for every proof that backs a generated compact key.
///
/// Generation validates and hashes evidence before invoking the sink.  A
/// deployment persists these records beside the SoraFS payload; the callback
/// avoids retaining tens of thousands of release-sized proofs in memory.
pub trait ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1 {
    /// Persist one canonical active source proof in generation order.
    fn record_active_source_proof(
        &mut self,
        ordinal: u8,
        source_record_index: u32,
        party_index: u8,
        proof: &ZkAmsMkheActiveRkgProofV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;

    /// Persist one canonical native CKS contribution in generation order.
    fn record_cks_contribution(
        &mut self,
        ordinal: u8,
        digit_index: u8,
        party_index: u8,
        contribution: &ZkAmsMkheAuthenticatedCksContributionV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;
}

struct CeremonyContext<'a> {
    profile: BgvProfile,
    roster: &'a ZkAmsMkheGovernedActiveRosterV1,
    wire_roster: ZkAmsMkheGovernedRosterWireV1,
    transcript_digest: [u8; 32],
    shares: [&'a ZkAmsMkheCollectivePublicKeyShareV1;
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    states: [&'a ZkAmsMkheCollectivePartyStateV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    authentication_secrets:
        [&'a ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    collective_key: ZkAmsMkheCollectivePublicKeyV1,
}

impl<'a> CeremonyContext<'a> {
    fn new(
        roster: &'a ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        shares: [&'a ZkAmsMkheCollectivePublicKeyShareV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        states: [&'a ZkAmsMkheCollectivePartyStateV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        authentication_secrets: [&'a ZkAmsMkheActivePartySecretV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        roster.validate()?;
        if transcript_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let profile = release_profile_v1();
        profile.validate()?;
        let collective_key = aggregate_zk_ams_mkhe_collective_public_key_v1(
            roster,
            transcript_digest,
            shares,
        )?;
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

struct EvidenceHasher {
    hash: Keccak256,
    records: u32,
}

impl EvidenceHasher {
    fn new(
        purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
        ordinal: u8,
        exponent: u32,
        collective_key_digest: [u8; 32],
    ) -> Self {
        let mut hash = Keccak256::new();
        hash.update(EVALUATED_KEY_EVIDENCE_DOMAIN_V1);
        hash.update(&[MKHE_VERSION_V1, purpose as u8, ordinal]);
        hash.update(&exponent.to_be_bytes());
        hash.update(&collective_key_digest);
        Self { hash, records: 0 }
    }

    fn active(
        &mut self,
        source_record_index: u32,
        party_index: usize,
        proof: &ZkAmsMkheActiveRkgProofV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.hash.update(b"active");
        self.hash.update(&source_record_index.to_be_bytes());
        self.hash.update(
            &u8::try_from(party_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                .to_be_bytes(),
        );
        self.hash.update(&proof.statement_digest());
        self.hash.update(&[proof.witness_polynomials()]);
        self.hash.update(
            &u32::try_from(proof.proof_bytes().len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        self.hash.update(proof.proof_bytes());
        self.hash.update(&proof.contribution().digest()?);
        self.records = self
            .records
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(())
    }

    fn cks(
        &mut self,
        digit_index: usize,
        party_index: usize,
        statement: ZkAmsMkheCksStatementV1<'_>,
        contribution: &ZkAmsMkheAuthenticatedCksContributionV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.hash.update(b"cks");
        self.hash.update(
            &u8::try_from(digit_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                .to_be_bytes(),
        );
        self.hash.update(
            &u8::try_from(party_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                .to_be_bytes(),
        );
        let wire = contribution.to_release_wire(statement)?;
        let bytes = wire.encode()?;
        self.hash.update(
            &u64::try_from(bytes.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        self.hash.update(&bytes);
        self.records = self
            .records
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(())
    }

    fn finish(mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if self.records == 0 {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.hash.update(&self.records.to_be_bytes());
        Ok(self.hash.finalize())
    }
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
            cks_evidence.cks(digit_index, party_index, statement, &contribution)?;
            sink.record_cks_contribution(
                ordinal,
                u8::try_from(digit_index)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                u8::try_from(party_index)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                &contribution,
            )?;
            contributions.push(contribution);
        }
        let compact = combine_zk_ams_mkhe_cks_v1(statement, &contributions)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCksSet)?;
        if compact.linear() != &target_a_wire {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
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
    let payload_bytes = u64::try_from(payload.len())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
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
