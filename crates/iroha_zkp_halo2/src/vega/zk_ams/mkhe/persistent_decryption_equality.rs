//! Public-CPK-owned persistent-secret authority for split decryption.
//!
//! A decryption proof uses one `secret_response` in both public RNS equations
//!
//! ```text
//! b_i     = -a * s_i + t * e_i
//! share_i = c_1 * s_i + t * z_i.
//! ```
//!
//! The complete CPK verifier separately proves that the exact persistent T256
//! commitment opens to a short `s_i` in the first equation. This module
//! consumes all eight secret-free, proof-verified CPK contributions at ceremony
//! time, retains their actual ordered commitment points, and makes that opaque
//! authority mandatory at prove, verify, split, reconstruct, and combine.
//! Proving additionally reopens the selected state-owned commitment and rejects
//! a mismatch before randomness is used. No verifier needs another party's
//! private state, and no caller-supplied digest can mint the authority.
//!
//! Equality remains the transitive short-solution claim for the shared CPK
//! equation; it is not presented as a direct Pedersen cross-opening. Release
//! readiness stays closed until that short-solution/SIS assumption has an
//! independently pinned certificate and a replacement release-size KAT.

use super::{
    ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::{
        PersistentWitnessConsumerV1, VerifiedPersistentWitnessBindingSetV1,
        VerifiedPersistentWitnessBindingV1, mint_collective_secret_binding_from_verified_cpk_v1,
        persistent_commitment_set_digest,
    },
    collective::{
        ZkAmsMkheCollectivePartyStateV1, ZkAmsMkheCollectivePublicKeyShareV1,
        collective_public_key_digest_from_bounded_cpk_v1, cpk_party_b_payload_blake3_v1,
        validate_collective_public_key_share_for_verified_cpk_compact_v1,
    },
    cpk_relation::{VerifiedZkAmsMkheCpkContributionV1, ZK_AMS_MKHE_CPK_PARTY_B_OBJECT_BYTES_V1},
    decryption::{
        ZkAmsMkheDecryptionStatementV1, decryption_key_context_digest_from_bounded_cpk_v1,
        decryption_statement_binding_digest_from_axes_v1, decryption_wire_ciphertext_digest_v1,
    },
    direct_object_transport::{
        ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1, ZkAmsMkheDirectObjectCasPublicationV1,
        ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1,
        ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheDirectObjectPublicationTransactionV1,
        ZkAmsMkheDirectObjectReadAtProviderV1, ZkAmsMkheDirectObjectReadReceiptV1,
        ZkAmsMkheDirectObjectReadTransactionV1,
    },
    manifest::{ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1},
    mod_add,
    persistent_membership_evidence::ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1,
    wire::{
        ZkAmsMkheCollectiveCiphertextWireV1, ZkAmsMkheGovernedRosterWireV1,
        ZkAmsMkheRnsPolynomialWireV1,
    },
    zk_ams_mkhe_security_certificate_v1,
};
use crate::vega::{VegaT256PointV1 as Point, sponge::Keccak256};

const TRANSITIVE_EQUATION_CONTRACT_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.persistent-decryption-equations:b_i=-a*s_i+t*e_i;share_i=c_1*s_i+t*z_i;same-secret-response";
const SHORT_SOLUTION_ASSUMPTION_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.persistent-decryption-short-solution-assumption:shared-cpk-equation:ternary-s:centered-binomial-e:sis-binding:certificate-required";
const CONTRIBUTION_SET_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.persistent-decryption-public-contribution-set";
const COMMITMENT_CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.persistent-decryption-commitment-context";
const PARTY_USE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.persistent-decryption-party-use";
const PROOF_BINDING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.persistent-decryption-proof-binding";
const STREAMING_AUTHORITY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.persistent-decryption-streaming-authority";
struct PersistentDecryptionPartyAuthorityV1 {
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    secret_identity_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    commitment_set_digest: [u8; 32],
    commitments: [Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
}

struct PersistentDecryptionSetAuthorityV1 {
    binding_set_root: [u8; 32],
    cpk_transcript_digest: [u8; 32],
    collective_public_key_digest: [u8; 32],
    parties: [PersistentDecryptionPartyAuthorityV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
}

struct PersistentDecryptionStreamingAuthorityV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    cpk_transcript_digest: [u8; 32],
    binding_set_root: [u8; 32],
    collective_public_key_digest: [u8; 32],
    share_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    key_context_digest: [u8; 32],
    public_contribution_set_digest: [u8; 32],
    party_b_pointers: [ZkAmsMkheDirectObjectPointerV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    verification_read_receipt_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    publication_receipt_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    publication_identity: [u8; 32],
    authority_digest: [u8; 32],
}

struct StreamingDecryptionAuthoritySealV1;

/// One-shot capability for constructing a bounded decryption statement.
///
/// The capability is move-only and has no decoder, public constructor, raw
/// pointer constructor, or `Clone` implementation. It can be minted only by
/// the exact eight-party bounded CPK ceremony below and is consumed by the
/// explicit streaming-statement constructor.
pub struct ZkAmsMkheStreamingDecryptionAuthorityV1 {
    _seal: StreamingDecryptionAuthoritySealV1,
    context_authority_digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkheStreamingDecryptionAuthorityV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheStreamingDecryptionAuthorityV1")
            .field(
                "context_authority_digest",
                &hex::encode(self.context_authority_digest),
            )
            .finish_non_exhaustive()
    }
}

/// Private material returned only after consuming the one-shot authority.
pub(super) struct ZkAmsMkheStreamingDecryptionAuthorityMaterialV1 {
    party_b_pointers: [ZkAmsMkheDirectObjectPointerV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    proof_bindings: [PersistentDecryptionProofBindingV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ciphertext_digest: [u8; 32],
    key_context_digest: [u8; 32],
}

impl ZkAmsMkheStreamingDecryptionAuthorityMaterialV1 {
    pub(super) fn into_parts(
        self,
    ) -> (
        [ZkAmsMkheDirectObjectPointerV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        [PersistentDecryptionProofBindingV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        [u8; 32],
        [u8; 32],
    ) {
        (
            self.party_b_pointers,
            self.proof_bindings,
            self.ciphertext_digest,
            self.key_context_digest,
        )
    }
}

/// Poisoned monotonic builder for the exact ordered eight-party CPK ceremony.
///
/// It is crate-private because the verified CPK contribution type is itself a
/// sealed internal capability. At most one public share is borrowed by each
/// transition; no array of eight release-sized shares is accepted or retained.
/// The buffer bound covers this algorithm, not arbitrary storage retained by a
/// caller's CAS implementation. Release deployment must use bounded/external
/// staging and remains blocked on the authenticated whole-worker residency run.
pub(super) struct ZkAmsMkheStreamingDecryptionAuthorityBuilderV1 {
    roster: ZkAmsMkheGovernedActiveRosterV1,
    cpk_transcript_digest: [u8; 32],
    next_party_index: usize,
    failed: bool,
    aggregate_b: Vec<u64>,
    share_digests: Vec<[u8; 32]>,
    bindings: Vec<VerifiedPersistentWitnessBindingV1>,
    party_b_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    verification_read_receipts: Vec<ZkAmsMkheDirectObjectReadReceiptV1>,
    publication_receipts: Vec<ZkAmsMkheDirectObjectPublicationReceiptV1>,
    provider_identity: Option<[u8; 32]>,
    snapshot_identity: Option<[u8; 32]>,
    publication_identity: Option<[u8; 32]>,
}

/// Secret-free verified CPK authority retained by an independent decryption verifier.
///
/// This type is move-only and has neither a decoder nor a public constructor.
/// Production construction consumes the exact eight complete native CPK
/// verifier capabilities, not private party states or digest shells.
pub struct ZkAmsMkhePersistentDecryptionVerificationContextV1 {
    roster: ZkAmsMkheGovernedActiveRosterV1,
    authority: PersistentDecryptionSetAuthorityV1,
    streaming_authority: Option<PersistentDecryptionStreamingAuthorityV1>,
    public_contribution_set_digest: [u8; 32],
    equation_contract_digest: [u8; 32],
    short_solution_assumption_digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkhePersistentDecryptionVerificationContextV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkhePersistentDecryptionVerificationContextV1")
            .field(
                "binding_set_root",
                &hex::encode(self.authority.binding_set_root),
            )
            .field(
                "equation_contract_digest",
                &hex::encode(self.equation_contract_digest),
            )
            .field(
                "short_solution_assumption_digest",
                &hex::encode(self.short_solution_assumption_digest),
            )
            .field(
                "streaming_authority_digest",
                &self
                    .streaming_authority
                    .as_ref()
                    .map(|authority| hex::encode(authority.authority_digest)),
            )
            .finish_non_exhaustive()
    }
}

/// Move-only authority for one exact governed party and ciphertext statement.
///
/// It is neither `Clone` nor serializable. Each invocation of
/// [`ZkAmsMkhePersistentDecryptionVerificationContextV1::bind_statement_v1`]
/// can issue a fresh set for the exact statement; replay rejection therefore
/// rests on the bound ciphertext, record, sample, and admission state rather
/// than on pretending the retained context is a one-shot token. The public
/// prover still consumes each issued use, and there is no omitted-capability
/// or raw-digest proving overload.
#[derive(Debug, PartialEq, Eq)]
pub struct ZkAmsMkhePersistentDecryptionPartyUseV1 {
    binding_set_root: [u8; 32],
    collective_public_key_digest: [u8; 32],
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    cpk_transcript_digest: [u8; 32],
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    secret_identity_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    commitment_set_digest: [u8; 32],
    commitments: [Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
    key_context_digest: [u8; 32],
    ciphertext_digest: [u8; 32],
    ciphertext_record_index: u32,
    sample_index: u64,
    level: u8,
    statement_digest: [u8; 32],
    public_contribution_set_digest: [u8; 32],
    commitment_context_digest: [u8; 32],
    equation_contract_digest: [u8; 32],
    short_solution_assumption_digest: [u8; 32],
    use_digest: [u8; 32],
}

impl ZkAmsMkhePersistentDecryptionPartyUseV1 {
    #[cfg(test)]
    pub(super) fn corrupt_axis_for_test(&mut self, axis: usize, other_party: ZkAmsMkhePartyIdV1) {
        match axis {
            0 => self.binding_set_root[0] ^= 1,
            1 => self.collective_public_key_digest[0] ^= 1,
            2 => self.profile_digest[0] ^= 1,
            3 => self.roster_digest[0] ^= 1,
            4 => self.epoch = self.epoch.wrapping_add(1),
            5 => self.cpk_transcript_digest[0] ^= 1,
            6 => self.party_index ^= 1,
            7 => self.party = other_party,
            8 => self.secret_identity_digest[0] ^= 1,
            9 => self.generator_basis_digest[0] ^= 1,
            10 => self.commitment_set_digest[0] ^= 1,
            11 => self.commitments.swap(0, 1),
            12 => self.key_context_digest[0] ^= 1,
            13 => self.ciphertext_digest[0] ^= 1,
            14 => self.ciphertext_record_index = self.ciphertext_record_index.wrapping_add(1),
            15 => self.sample_index = self.sample_index.wrapping_add(1),
            16 => self.level ^= 1,
            17 => self.statement_digest[0] ^= 1,
            18 => self.public_contribution_set_digest[0] ^= 1,
            19 => self.commitment_context_digest[0] ^= 1,
            20 => self.equation_contract_digest[0] ^= 1,
            21 => self.short_solution_assumption_digest[0] ^= 1,
            22 => self.use_digest[0] ^= 1,
            _ => unreachable!(),
        }
    }
}

/// Typed evidence metadata committed into the existing decryption transcript.
///
/// It is produced only after the move-only party use (proving) or retained
/// secret-free CPK authority (verification) has been validated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PersistentDecryptionProofBindingV1 {
    binding_set_root: [u8; 32],
    collective_public_key_digest: [u8; 32],
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    secret_identity_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    commitment_set_digest: [u8; 32],
    commitments: [Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
    use_digest: [u8; 32],
    equation_contract_digest: [u8; 32],
    short_solution_assumption_digest: [u8; 32],
    binding_digest: [u8; 32],
}

impl PersistentDecryptionProofBindingV1 {
    pub(super) const fn binding_digest(&self) -> [u8; 32] {
        self.binding_digest
    }
}

impl ZkAmsMkheStreamingDecryptionAuthorityBuilderV1 {
    /// Begin the exact bounded ceremony before borrowing any public share.
    pub(super) fn new(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        cpk_transcript_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        roster.validate()?;
        let profile = release_profile_v1();
        profile.validate()?;
        if cpk_transcript_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let coefficient_count = profile
            .ring_degree
            .checked_mul(profile.moduli.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut aggregate_b = Vec::new();
        aggregate_b
            .try_reserve_exact(coefficient_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        aggregate_b.resize(coefficient_count, 0);
        let mut share_digests = Vec::new();
        let mut bindings = Vec::new();
        let mut party_b_pointers = Vec::new();
        let mut verification_read_receipts = Vec::new();
        let mut publication_receipts = Vec::new();
        for reserve in [
            share_digests.try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1),
            bindings.try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1),
            party_b_pointers.try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1),
            verification_read_receipts.try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1),
            publication_receipts.try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1),
        ] {
            reserve.map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
        Ok(Self {
            roster: *roster,
            cpk_transcript_digest,
            next_party_index: 0,
            failed: false,
            aggregate_b,
            share_digests,
            bindings,
            party_b_pointers,
            verification_read_receipts,
            publication_receipts,
            provider_identity: None,
            snapshot_identity: None,
            publication_identity: None,
        })
    }

    /// Consume and publish the sole next governed CPK contribution.
    ///
    /// The transition is poisoned before any fallible or backend-controlled
    /// operation. An error or caught unwind therefore makes `finish`
    /// permanently unavailable; no partially observed ceremony can resume.
    pub(super) fn absorb_verified_party_v1<P>(
        &mut self,
        contribution: VerifiedZkAmsMkheCpkContributionV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
        publisher: &mut P,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        if self.failed || self.next_party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            self.failed = true;
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.failed = true;
        let result = self.absorb_verified_party_inner_v1(contribution, share, publisher);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }

    fn absorb_verified_party_inner_v1<P>(
        &mut self,
        contribution: VerifiedZkAmsMkheCpkContributionV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
        publisher: &mut P,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        let party_index = self.next_party_index;
        let share_digest = validate_collective_public_key_share_for_verified_cpk_compact_v1(
            &self.roster,
            self.cpk_transcript_digest,
            party_index,
            share,
        )?;
        let compact_source = contribution
            .into_compact_decryption_source(&self.roster, self.cpk_transcript_digest, party_index)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let expected_pointer = compact_source.party_b_pointer();
        let payload_blake3 = cpk_party_b_payload_blake3_v1(share.party_public_b())?;
        if expected_pointer.kind() != ZkAmsMkheDirectObjectKindV1::CpkPartyB
            || expected_pointer.payload_blake3() != payload_blake3
            || self.party_b_pointers.contains(&expected_pointer)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let (binding_source, source_pointer, verification_read_receipt) =
            compact_source.into_parts();
        if source_pointer != expected_pointer {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let binding = mint_collective_secret_binding_from_verified_cpk_v1(
            &self.roster,
            self.cpk_transcript_digest,
            party_index,
            share_digest,
            binding_source,
        )?;
        let publication_receipt = publish_canonical_party_b_v1(share.party_public_b(), publisher)?;
        validate_compact_publication_provenance_v1(
            expected_pointer,
            &verification_read_receipt,
            &publication_receipt,
            self.provider_identity,
            self.snapshot_identity,
            self.publication_identity,
            &self.publication_receipts,
        )?;
        let verification_snapshot = verification_read_receipt.snapshot();
        self.provider_identity = Some(verification_snapshot.provider_identity());
        self.snapshot_identity = Some(verification_snapshot.snapshot_identity());
        self.publication_identity = Some(publication_receipt.publication_identity());

        let profile = release_profile_v1();
        for (limb, residues) in share
            .party_public_b()
            .residues()
            .chunks_exact(profile.ring_degree)
            .enumerate()
        {
            let start = limb
                .checked_mul(profile.ring_degree)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let modulus = profile.moduli[limb];
            for (offset, residue) in residues.iter().copied().enumerate() {
                let aggregate = self
                    .aggregate_b
                    .get_mut(start + offset)
                    .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
                *aggregate = mod_add(*aggregate, residue, modulus);
            }
        }
        self.share_digests.push(share_digest);
        self.bindings.push(binding);
        self.party_b_pointers.push(expected_pointer);
        self.verification_read_receipts
            .push(verification_read_receipt);
        self.publication_receipts.push(publication_receipt);
        self.next_party_index += 1;
        Ok(())
    }

    /// Finish only after all eight ordered capabilities and publications exist.
    pub(super) fn finish<P>(
        mut self,
        publisher: &mut P,
    ) -> Result<
        (
            ZkAmsMkhePersistentDecryptionVerificationContextV1,
            ZkAmsMkheStreamingDecryptionAuthorityV1,
            [VerifiedPersistentWitnessBindingV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        ),
        ZkAmsMkheErrorV1,
    >
    where
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        if self.failed
            || self.next_party_index != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || self.share_digests.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || self.bindings.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || self.party_b_pointers.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || self.verification_read_receipts.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || self.publication_receipts.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        {
            self.failed = true;
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.failed = true;
        let provider_identity = self
            .provider_identity
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let snapshot_identity = self
            .snapshot_identity
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let publication_identity = self
            .publication_identity
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let share_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = self
            .share_digests
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
        let party_b_pointers: [ZkAmsMkheDirectObjectPointerV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            self.party_b_pointers
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
        let collective_public_key_digest = collective_public_key_digest_from_bounded_cpk_v1(
            &self.roster,
            self.cpk_transcript_digest,
            &self.aggregate_b,
            share_digests,
        )?;
        // The sole full-RNS construction buffer is no longer live during any
        // direct-object reread or common-`a` limb derivation below.
        drop(self.aggregate_b);

        let key_context_digest = decryption_key_context_digest_from_bounded_cpk_v1(
            &self.roster,
            self.cpk_transcript_digest,
            collective_public_key_digest,
            share_digests,
            |party_index, hash| {
                stream_canonical_party_b_into_hash_v1(
                    party_b_pointers[party_index],
                    provider_identity,
                    snapshot_identity,
                    publisher,
                    hash,
                )
            },
        )?;
        let public_contribution_set_digest = public_contribution_set_digest_from_streamed_cpk_v1(
            &self.roster,
            key_context_digest,
            collective_public_key_digest,
            share_digests,
            party_b_pointers,
            provider_identity,
            snapshot_identity,
            publisher,
        )?;

        let bindings: [VerifiedPersistentWitnessBindingV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            self.bindings
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
        let binding_refs = core::array::from_fn(|index| &bindings[index]);
        let binding_set = VerifiedPersistentWitnessBindingSetV1::new(
            &self.roster,
            self.cpk_transcript_digest,
            collective_public_key_digest,
            share_digests,
            binding_refs,
        )?;
        let authority =
            persistent_decryption_authority_from_binding_set_v1(&self.roster, &binding_set)?;
        let verification_read_receipt_digests =
            core::array::from_fn(|index| self.verification_read_receipts[index].receipt_digest());
        let publication_receipt_digests =
            core::array::from_fn(|index| self.publication_receipts[index].receipt_digest());
        let mut streaming_authority = PersistentDecryptionStreamingAuthorityV1 {
            profile_digest: self.roster.profile_digest(),
            roster_digest: self.roster.roster_digest(),
            key_material_digest: self.roster.key_material_digest(),
            epoch: self.roster.epoch(),
            cpk_transcript_digest: self.cpk_transcript_digest,
            binding_set_root: authority.binding_set_root,
            collective_public_key_digest,
            share_digests,
            key_context_digest,
            public_contribution_set_digest,
            party_b_pointers,
            verification_read_receipt_digests,
            publication_receipt_digests,
            provider_identity,
            snapshot_identity,
            publication_identity,
            authority_digest: [0; 32],
        };
        streaming_authority.authority_digest =
            streaming_decryption_authority_digest_v1(&streaming_authority)?;
        validate_streaming_decryption_authority_v1(&self.roster, &streaming_authority)?;
        let context_authority_digest = streaming_authority.authority_digest;
        let context = ZkAmsMkhePersistentDecryptionVerificationContextV1 {
            roster: self.roster,
            authority,
            streaming_authority: Some(streaming_authority),
            public_contribution_set_digest,
            equation_contract_digest: digest_literal(TRANSITIVE_EQUATION_CONTRACT_V1),
            short_solution_assumption_digest: digest_literal(SHORT_SOLUTION_ASSUMPTION_V1),
        };
        context.validate_streaming_context_v1()?;
        let compact_authority = ZkAmsMkheStreamingDecryptionAuthorityV1 {
            _seal: StreamingDecryptionAuthoritySealV1,
            context_authority_digest,
        };
        Ok((context, compact_authority, bindings))
    }
}

/// Start the sole allocation-bounded compact-authority ceremony.
pub(super) fn begin_zk_ams_mkhe_streaming_decryption_authority_from_verified_cpk_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    cpk_transcript_digest: [u8; 32],
) -> Result<ZkAmsMkheStreamingDecryptionAuthorityBuilderV1, ZkAmsMkheErrorV1> {
    ZkAmsMkheStreamingDecryptionAuthorityBuilderV1::new(roster, cpk_transcript_digest)
}

fn publish_canonical_party_b_v1<P>(
    party_b: &ZkAmsMkheRnsPolynomialWireV1,
    publisher: &mut P,
) -> Result<ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    party_b.encoded_len()?;
    let coefficient_count = u32::try_from(party_b.residues().len())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let payload_bytes = party_b
        .residues()
        .len()
        .checked_mul(core::mem::size_of::<u64>())
        .and_then(|bytes| bytes.checked_add(core::mem::size_of::<u32>()))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut transaction = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
        ZkAmsMkheDirectObjectKindV1::CpkPartyB,
        u64::try_from(payload_bytes).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        publisher,
    )?;
    transaction.write_exact(&coefficient_count.to_be_bytes())?;
    {
        let mut buffer = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
        for residues in party_b
            .residues()
            .chunks(buffer.len() / core::mem::size_of::<u64>())
        {
            let bytes = residues
                .len()
                .checked_mul(core::mem::size_of::<u64>())
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            for (encoded, residue) in buffer[..bytes]
                .chunks_exact_mut(core::mem::size_of::<u64>())
                .zip(residues)
            {
                encoded.copy_from_slice(&residue.to_be_bytes());
            }
            transaction.write_exact(&buffer[..bytes])?;
        }
    }
    transaction.finish()
}

#[allow(clippy::too_many_arguments)]
fn validate_compact_publication_provenance_v1(
    expected_pointer: ZkAmsMkheDirectObjectPointerV1,
    verification_read_receipt: &ZkAmsMkheDirectObjectReadReceiptV1,
    publication_receipt: &ZkAmsMkheDirectObjectPublicationReceiptV1,
    expected_provider_identity: Option<[u8; 32]>,
    expected_snapshot_identity: Option<[u8; 32]>,
    expected_publication_identity: Option<[u8; 32]>,
    prior_publications: &[ZkAmsMkheDirectObjectPublicationReceiptV1],
) -> Result<(), ZkAmsMkheErrorV1> {
    let verification_snapshot = verification_read_receipt.snapshot();
    let publication_snapshot = publication_receipt.post_publish_read_receipt().snapshot();
    if expected_pointer.kind() != ZkAmsMkheDirectObjectKindV1::CpkPartyB
        || verification_snapshot.pointer() != expected_pointer
        || publication_receipt.pointer() != expected_pointer
        || publication_receipt.published_binding().pointer() != expected_pointer
        || publication_snapshot.pointer() != expected_pointer
        || verification_read_receipt.canonical_bytes() != expected_pointer.payload_bytes()
        || publication_receipt
            .post_publish_read_receipt()
            .canonical_bytes()
            != expected_pointer.payload_bytes()
        || verification_read_receipt.payload_blake3() != expected_pointer.payload_blake3()
        || publication_receipt
            .post_publish_read_receipt()
            .payload_blake3()
            != expected_pointer.payload_blake3()
        || verification_snapshot.provider_identity() != publication_snapshot.provider_identity()
        || verification_snapshot.snapshot_identity() != publication_snapshot.snapshot_identity()
        || expected_provider_identity
            .is_some_and(|expected| expected != verification_snapshot.provider_identity())
        || expected_snapshot_identity
            .is_some_and(|expected| expected != verification_snapshot.snapshot_identity())
        || expected_publication_identity
            .is_some_and(|expected| expected != publication_receipt.publication_identity())
        || prior_publications.iter().any(|prior| {
            prior.pointer() == expected_pointer
                || prior.staging_identity() == publication_receipt.staging_identity()
                || prior.seal_identity() == publication_receipt.seal_identity()
                || prior.published_binding().published_object_identity()
                    == publication_receipt
                        .published_binding()
                        .published_object_identity()
        })
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn stream_canonical_party_b_into_hash_v1<P>(
    pointer: ZkAmsMkheDirectObjectPointerV1,
    expected_provider_identity: [u8; 32],
    expected_snapshot_identity: [u8; 32],
    provider: &mut P,
    hash: &mut Keccak256,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let profile = release_profile_v1();
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut transaction = ZkAmsMkheDirectObjectReadTransactionV1::begin(
        ZkAmsMkheDirectObjectKindV1::CpkPartyB,
        pointer,
        provider,
    )?;
    let mut count = [0_u8; 4];
    if transaction.read_next(provider, &mut count)? != count.len()
        || usize::try_from(u32::from_be_bytes(count)).ok() != Some(coefficient_count)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    hash.update(&count);
    let mut buffer = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    while transaction.remaining_bytes() != 0 {
        let read = transaction.read_next(provider, &mut buffer)?;
        if read == 0 || read % core::mem::size_of::<u64>() != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        hash.update(&buffer[..read]);
    }
    let receipt = transaction.finish(provider)?;
    let snapshot = receipt.snapshot();
    if snapshot.pointer() != pointer
        || snapshot.provider_identity() != expected_provider_identity
        || snapshot.snapshot_identity() != expected_snapshot_identity
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn public_contribution_set_digest_from_streamed_cpk_v1<P>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    key_context_digest: [u8; 32],
    collective_public_key_digest: [u8; 32],
    share_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    party_b_pointers: [ZkAmsMkheDirectObjectPointerV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    provider: &mut P,
) -> Result<[u8; 32], ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let mut hash = Keccak256::new();
    hash.update(CONTRIBUTION_SET_DOMAIN_V1);
    hash.update(&key_context_digest);
    hash.update(&collective_public_key_digest);
    for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        hash.update(
            &u32::try_from(party_index)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        hash.update(&roster.participants()[party_index].party().to_bytes());
        hash.update(&share_digests[party_index]);
        stream_canonical_party_b_into_hash_v1(
            party_b_pointers[party_index],
            provider_identity,
            snapshot_identity,
            provider,
            &mut hash,
        )?;
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(digest)
}

fn streaming_decryption_authority_digest_v1(
    authority: &PersistentDecryptionStreamingAuthorityV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(STREAMING_AUTHORITY_DOMAIN_V1);
    hash.update(&authority.profile_digest);
    hash.update(&authority.roster_digest);
    hash.update(&authority.key_material_digest);
    hash.update(&authority.epoch.to_be_bytes());
    hash.update(&authority.cpk_transcript_digest);
    hash.update(&authority.binding_set_root);
    hash.update(&authority.collective_public_key_digest);
    for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        hash.update(
            &u32::try_from(party_index)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        hash.update(&authority.share_digests[party_index]);
        hash.update(&authority.party_b_pointers[party_index].pointer_digest());
        hash.update(&authority.verification_read_receipt_digests[party_index]);
        hash.update(&authority.publication_receipt_digests[party_index]);
    }
    hash.update(&authority.key_context_digest);
    hash.update(&authority.public_contribution_set_digest);
    hash.update(&authority.provider_identity);
    hash.update(&authority.snapshot_identity);
    hash.update(&authority.publication_identity);
    Ok(hash.finalize())
}

fn validate_streaming_decryption_authority_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    authority: &PersistentDecryptionStreamingAuthorityV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    roster.validate()?;
    if authority.profile_digest != roster.profile_digest()
        || authority.roster_digest != roster.roster_digest()
        || authority.key_material_digest != roster.key_material_digest()
        || authority.epoch != roster.epoch()
        || authority.cpk_transcript_digest == [0; 32]
        || authority.binding_set_root == [0; 32]
        || authority.collective_public_key_digest == [0; 32]
        || authority.share_digests.contains(&[0; 32])
        || authority.key_context_digest == [0; 32]
        || authority.public_contribution_set_digest == [0; 32]
        || authority
            .verification_read_receipt_digests
            .contains(&[0; 32])
        || authority.publication_receipt_digests.contains(&[0; 32])
        || authority.provider_identity == [0; 32]
        || authority.snapshot_identity == [0; 32]
        || authority.publication_identity == [0; 32]
        || authority.authority_digest == [0; 32]
        || authority.authority_digest != streaming_decryption_authority_digest_v1(authority)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let pointer = authority.party_b_pointers[party_index];
        if pointer.kind() != ZkAmsMkheDirectObjectKindV1::CpkPartyB
            || pointer.payload_bytes() != ZK_AMS_MKHE_CPK_PARTY_B_OBJECT_BYTES_V1 as u64
            || authority.party_b_pointers[..party_index].contains(&pointer)
            || authority.verification_read_receipt_digests[..party_index]
                .contains(&authority.verification_read_receipt_digests[party_index])
            || authority.publication_receipt_digests[..party_index]
                .contains(&authority.publication_receipt_digests[party_index])
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
    }
    Ok(())
}

/// Consume all eight complete, secret-free CPK verifier capabilities.
///
/// This is the production ceremony boundary. Every contribution is rebound to
/// the exact ordered public share and direct-object `b_i` payload before an
/// opaque verifier context or party-use capability is returned. The third
/// return value is the same ordered set of move-only bindings, emitted
/// atomically for admission into the eight party states; callers never need to
/// clone or re-verify a CPK contribution.
#[allow(
    dead_code,
    reason = "retained by the private CPK ceremony until the fail-closed release gate opens"
)]
pub(super) fn prepare_zk_ams_mkhe_persistent_decryption_from_verified_cpk_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    contributions: [VerifiedZkAmsMkheCpkContributionV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<
    (
        ZkAmsMkhePersistentDecryptionVerificationContextV1,
        [ZkAmsMkhePersistentDecryptionPartyUseV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        [VerifiedPersistentWitnessBindingV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ),
    ZkAmsMkheErrorV1,
> {
    validate_roster_statement(roster, statement)?;
    let mut bindings = Vec::<VerifiedPersistentWitnessBindingV1>::new();
    bindings
        .try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for (party_index, contribution) in contributions.into_iter().enumerate() {
        let share = statement.public_key_shares()[party_index];
        let source = contribution
            .into_collective_binding_source(
                roster,
                statement.collective_public_key().transcript_digest(),
                party_index,
                cpk_party_b_payload_blake3_v1(share.party_public_b())?,
            )
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        bindings.push(mint_collective_secret_binding_from_verified_cpk_v1(
            roster,
            statement.collective_public_key().transcript_digest(),
            party_index,
            share.digest(),
            source,
        )?);
    }
    let bindings: [VerifiedPersistentWitnessBindingV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
        bindings
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
    let binding_refs = core::array::from_fn(|index| &bindings[index]);
    let binding_set = VerifiedPersistentWitnessBindingSetV1::new(
        roster,
        statement.collective_public_key().transcript_digest(),
        statement.collective_public_key().digest(),
        (*statement.public_key_shares()).map(|share| share.digest()),
        binding_refs,
    )?;
    let (context, uses) = context_from_verified_binding_set(roster, statement, binding_set)?;
    Ok((context, uses, bindings))
}

fn context_from_verified_binding_set(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    bindings: VerifiedPersistentWitnessBindingSetV1,
) -> Result<
    (
        ZkAmsMkhePersistentDecryptionVerificationContextV1,
        [ZkAmsMkhePersistentDecryptionPartyUseV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ),
    ZkAmsMkheErrorV1,
> {
    bindings.validate_for_consumer(roster, PersistentWitnessConsumerV1::Decryption)?;
    let authority = persistent_decryption_authority_from_binding_set_v1(roster, &bindings)?;
    build_context(roster, statement, authority)
}

fn persistent_decryption_authority_from_binding_set_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    bindings: &VerifiedPersistentWitnessBindingSetV1,
) -> Result<PersistentDecryptionSetAuthorityV1, ZkAmsMkheErrorV1> {
    bindings.validate_for_consumer(roster, PersistentWitnessConsumerV1::Decryption)?;
    let mut parties = Vec::new();
    parties
        .try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let (identity, basis, commitment_digest, commitments) =
            bindings.decryption_party_material(party_index)?;
        parties.push(PersistentDecryptionPartyAuthorityV1 {
            party_index: u8::try_from(party_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
            party: roster.participants()[party_index].party(),
            secret_identity_digest: identity,
            generator_basis_digest: basis,
            commitment_set_digest: commitment_digest,
            commitments,
        });
    }
    Ok(PersistentDecryptionSetAuthorityV1 {
        binding_set_root: bindings.set_root(),
        cpk_transcript_digest: bindings.cpk_transcript_digest(),
        collective_public_key_digest: bindings.collective_public_key_digest(),
        parties: parties
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
    })
}

/// Test-only downstream graph fixture; production code cannot call it.
///
/// Every state must already retain the cfg(test)-only synthetic CPK admission.
#[cfg(test)]
pub(super) fn prepare_zk_ams_mkhe_persistent_decryption_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    party_states: [&ZkAmsMkheCollectivePartyStateV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<
    (
        ZkAmsMkhePersistentDecryptionVerificationContextV1,
        [ZkAmsMkhePersistentDecryptionPartyUseV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ),
    ZkAmsMkheErrorV1,
> {
    validate_roster_statement(roster, statement)?;
    let mut binding_refs = Vec::new();
    binding_refs
        .try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for (party_index, state) in party_states.iter().enumerate() {
        validate_party_state_axes(roster, statement, party_index, state)?;
        binding_refs.push(
            state.persistent_secret_binding_for(roster, PersistentWitnessConsumerV1::Decryption)?,
        );
    }
    let binding_refs: [&VerifiedPersistentWitnessBindingV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
        binding_refs
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
    let binding_set = VerifiedPersistentWitnessBindingSetV1::new(
        roster,
        statement.collective_public_key().transcript_digest(),
        statement.collective_public_key().digest(),
        (*statement.public_key_shares()).map(|share| share.digest()),
        binding_refs,
    )?;
    context_from_verified_binding_set(roster, statement, binding_set)
}

fn build_context(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    authority: PersistentDecryptionSetAuthorityV1,
) -> Result<
    (
        ZkAmsMkhePersistentDecryptionVerificationContextV1,
        [ZkAmsMkhePersistentDecryptionPartyUseV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ),
    ZkAmsMkheErrorV1,
> {
    let context = ZkAmsMkhePersistentDecryptionVerificationContextV1 {
        roster: *roster,
        authority,
        streaming_authority: None,
        public_contribution_set_digest: public_contribution_set_digest(statement)?,
        equation_contract_digest: digest_literal(TRANSITIVE_EQUATION_CONTRACT_V1),
        short_solution_assumption_digest: digest_literal(SHORT_SOLUTION_ASSUMPTION_V1),
    };
    context.validate_statement(statement)?;
    let uses = context.bind_statement_v1(statement)?;
    Ok((context, uses))
}

impl ZkAmsMkhePersistentDecryptionVerificationContextV1 {
    /// Trusted public axes needed to rederive the common CPK `a` polynomial
    /// without retaining any full public-key share.
    pub(super) fn streaming_public_axes_v1(&self) -> (&ZkAmsMkheGovernedActiveRosterV1, [u8; 32]) {
        (&self.roster, self.authority.cpk_transcript_digest)
    }

    fn validate_streaming_context_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        self.roster.validate()?;
        let streaming = self
            .streaming_authority
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        validate_streaming_decryption_authority_v1(&self.roster, streaming)?;
        if self.authority.binding_set_root != streaming.binding_set_root
            || self.authority.cpk_transcript_digest != streaming.cpk_transcript_digest
            || self.authority.collective_public_key_digest != streaming.collective_public_key_digest
            || self.public_contribution_set_digest != streaming.public_contribution_set_digest
            || self.equation_contract_digest != digest_literal(TRANSITIVE_EQUATION_CONTRACT_V1)
            || self.short_solution_assumption_digest != digest_literal(SHORT_SOLUTION_ASSUMPTION_V1)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        for (party_index, authority) in self.authority.parties.iter().enumerate() {
            validate_party_authority(&self.roster, party_index, authority)?;
        }
        Ok(())
    }

    /// Consume the sole bounded ceremony capability and bind it to one exact
    /// roster/ciphertext pair without constructing the native statement.
    pub(super) fn consume_streaming_authority_v1(
        &self,
        roster: &ZkAmsMkheGovernedRosterWireV1,
        ciphertext: &ZkAmsMkheCollectiveCiphertextWireV1,
        authority: ZkAmsMkheStreamingDecryptionAuthorityV1,
    ) -> Result<ZkAmsMkheStreamingDecryptionAuthorityMaterialV1, ZkAmsMkheErrorV1> {
        self.validate_streaming_context_v1()?;
        let streaming = self
            .streaming_authority
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        if authority.context_authority_digest == [0; 32]
            || authority.context_authority_digest != streaming.authority_digest
            || self.roster.to_wire_roster()? != *roster
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let profile = release_profile_v1();
        let ciphertext_digest = decryption_wire_ciphertext_digest_v1(&profile, roster, ciphertext)?;
        let statement_digest = decryption_statement_binding_digest_from_axes_v1(
            roster,
            ciphertext,
            streaming.key_context_digest,
        );
        if statement_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut proof_bindings = Vec::new();
        proof_bindings
            .try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            let party_use = self.mint_party_use_from_compact_axes_v1(
                roster,
                ciphertext,
                ciphertext_digest,
                streaming.key_context_digest,
                statement_digest,
                party_index,
            )?;
            proof_bindings
                .push(self.proof_binding_from_use_digest_v1(statement_digest, party_use)?);
        }
        Ok(ZkAmsMkheStreamingDecryptionAuthorityMaterialV1 {
            party_b_pointers: streaming.party_b_pointers,
            proof_bindings: proof_bindings
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
            ciphertext_digest,
            key_context_digest: streaming.key_context_digest,
        })
    }

    /// Mint the exact move-only party-use set for a compact streaming statement.
    ///
    /// The caller supplies no digest or content address. Every axis is
    /// recomputed from the retained verified CPK authority and the canonical
    /// roster/ciphertext pair. Each returned capability is consumed by one
    /// staged prover invocation.
    pub(super) fn bind_streaming_statement_party_uses_v1(
        &self,
        roster: &ZkAmsMkheGovernedRosterWireV1,
        ciphertext: &ZkAmsMkheCollectiveCiphertextWireV1,
        ciphertext_digest: [u8; 32],
        key_context_digest: [u8; 32],
    ) -> Result<
        [ZkAmsMkhePersistentDecryptionPartyUseV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        ZkAmsMkheErrorV1,
    > {
        self.validate_streaming_context_v1()?;
        let statement_digest = decryption_statement_binding_digest_from_axes_v1(
            roster,
            ciphertext,
            key_context_digest,
        );
        if statement_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut uses = Vec::new();
        uses.try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            uses.push(self.mint_party_use_from_compact_axes_v1(
                roster,
                ciphertext,
                ciphertext_digest,
                key_context_digest,
                statement_digest,
                party_index,
            )?);
        }
        uses.try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)
    }

    /// Consume one compact statement use and reopen its exact state-owned CPK
    /// witness binding without materializing a native decryption statement.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn consume_streaming_party_use_v1(
        &self,
        roster: &ZkAmsMkheGovernedRosterWireV1,
        ciphertext: &ZkAmsMkheCollectiveCiphertextWireV1,
        ciphertext_digest: [u8; 32],
        key_context_digest: [u8; 32],
        party_index: usize,
        party_use: ZkAmsMkhePersistentDecryptionPartyUseV1,
        party_state: &ZkAmsMkheCollectivePartyStateV1,
    ) -> Result<PersistentDecryptionProofBindingV1, ZkAmsMkheErrorV1> {
        self.validate_streaming_context_v1()?;
        let statement_digest = decryption_statement_binding_digest_from_axes_v1(
            roster,
            ciphertext,
            key_context_digest,
        );
        let expected = self.mint_party_use_from_compact_axes_v1(
            roster,
            ciphertext,
            ciphertext_digest,
            key_context_digest,
            statement_digest,
            party_index,
        )?;
        if party_use != expected {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        validate_streaming_party_state_axes_v1(self, party_index, party_state)?;
        let binding = party_state
            .persistent_secret_binding_for(&self.roster, PersistentWitnessConsumerV1::Decryption)?;
        if binding.identity_digest() != party_use.secret_identity_digest
            || binding.commitment_set_digest() != party_use.commitment_set_digest
            || binding.commitments() != &party_use.commitments
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.proof_binding_from_use_digest_v1(statement_digest, party_use)
    }

    pub(super) fn validate_streaming_statement_axes_if_present_v1(
        &self,
        roster: &ZkAmsMkheGovernedRosterWireV1,
        ciphertext: &ZkAmsMkheCollectiveCiphertextWireV1,
        key_context_digest: [u8; 32],
        party_b_pointers: [ZkAmsMkheDirectObjectPointerV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let Some(streaming) = self.streaming_authority.as_ref() else {
            return Ok(());
        };
        self.validate_streaming_context_v1()?;
        if self.roster.to_wire_roster()? != *roster
            || key_context_digest != streaming.key_context_digest
            || party_b_pointers != streaming.party_b_pointers
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        decryption_wire_ciphertext_digest_v1(&release_profile_v1(), roster, ciphertext)?;
        Ok(())
    }

    /// Stable ordered-set identity for evidence inventories.
    #[must_use]
    pub const fn binding_set_root(&self) -> [u8; 32] {
        self.authority.binding_set_root
    }

    /// Bind a fresh exact eight-party use set to a later validated ciphertext.
    pub fn bind_statement_v1(
        &self,
        statement: ZkAmsMkheDecryptionStatementV1<'_>,
    ) -> Result<
        [ZkAmsMkhePersistentDecryptionPartyUseV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        ZkAmsMkheErrorV1,
    > {
        let mut uses = Vec::new();
        uses.try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            uses.push(self.mint_party_use(statement, party_index)?);
        }
        uses.try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)
    }

    fn validate_statement(
        &self,
        statement: ZkAmsMkheDecryptionStatementV1<'_>,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        validate_roster_statement(&self.roster, statement)?;
        if self.authority.binding_set_root == [0; 32]
            || self.authority.cpk_transcript_digest
                != statement.collective_public_key().transcript_digest()
            || self.authority.collective_public_key_digest
                != statement.collective_public_key().digest()
            || self.public_contribution_set_digest != public_contribution_set_digest(statement)?
            || self.equation_contract_digest != digest_literal(TRANSITIVE_EQUATION_CONTRACT_V1)
            || self.short_solution_assumption_digest != digest_literal(SHORT_SOLUTION_ASSUMPTION_V1)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        for (party_index, authority) in self.authority.parties.iter().enumerate() {
            validate_party_authority(&self.roster, party_index, authority)?;
        }
        if let Some(streaming) = self.streaming_authority.as_ref() {
            validate_streaming_decryption_authority_v1(&self.roster, streaming)?;
            if streaming.binding_set_root != self.authority.binding_set_root
                || streaming.collective_public_key_digest
                    != statement.collective_public_key().digest()
                || streaming.share_digests
                    != (*statement.public_key_shares()).map(|share| share.digest())
                || streaming.key_context_digest != statement.key_context_digest()
                || streaming.public_contribution_set_digest != self.public_contribution_set_digest
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
                let party_b = statement
                    .party_public_b(party_index)
                    .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
                if streaming.party_b_pointers[party_index].payload_blake3()
                    != cpk_party_b_payload_blake3_v1(party_b)?
                {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
            }
        }
        Ok(())
    }

    fn mint_party_use(
        &self,
        statement: ZkAmsMkheDecryptionStatementV1<'_>,
        party_index: usize,
    ) -> Result<ZkAmsMkhePersistentDecryptionPartyUseV1, ZkAmsMkheErrorV1> {
        self.validate_statement(statement)?;
        let authority = self
            .authority
            .parties
            .get(party_index)
            .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
        let mut party_use = ZkAmsMkhePersistentDecryptionPartyUseV1 {
            binding_set_root: self.authority.binding_set_root,
            collective_public_key_digest: self.authority.collective_public_key_digest,
            profile_digest: statement.roster().profile_digest(),
            roster_digest: statement.roster().roster_digest(),
            epoch: statement.roster().epoch(),
            cpk_transcript_digest: self.authority.cpk_transcript_digest,
            party_index: authority.party_index,
            party: authority.party,
            secret_identity_digest: authority.secret_identity_digest,
            generator_basis_digest: authority.generator_basis_digest,
            commitment_set_digest: authority.commitment_set_digest,
            commitments: authority.commitments,
            key_context_digest: statement.key_context_digest(),
            ciphertext_digest: statement.ciphertext_digest()?,
            ciphertext_record_index: statement.ciphertext().binding().record_index(),
            sample_index: statement.ciphertext().sample_index(),
            level: statement.ciphertext().binding().level(),
            statement_digest: statement.binding_digest(),
            public_contribution_set_digest: self.public_contribution_set_digest,
            commitment_context_digest: commitment_context_digest(
                self.authority.binding_set_root,
                authority.party_index,
            ),
            equation_contract_digest: self.equation_contract_digest,
            short_solution_assumption_digest: self.short_solution_assumption_digest,
            use_digest: [0; 32],
        };
        party_use.use_digest = party_use_digest(&party_use)?;
        validate_party_use(&party_use)?;
        Ok(party_use)
    }

    #[allow(clippy::too_many_arguments)]
    fn mint_party_use_from_compact_axes_v1(
        &self,
        roster: &ZkAmsMkheGovernedRosterWireV1,
        ciphertext: &ZkAmsMkheCollectiveCiphertextWireV1,
        ciphertext_digest: [u8; 32],
        key_context_digest: [u8; 32],
        statement_digest: [u8; 32],
        party_index: usize,
    ) -> Result<ZkAmsMkhePersistentDecryptionPartyUseV1, ZkAmsMkheErrorV1> {
        self.validate_streaming_context_v1()?;
        let streaming = self
            .streaming_authority
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        if self.roster.to_wire_roster()? != *roster
            || key_context_digest != streaming.key_context_digest
            || ciphertext_digest
                != decryption_wire_ciphertext_digest_v1(&release_profile_v1(), roster, ciphertext)?
            || statement_digest
                != decryption_statement_binding_digest_from_axes_v1(
                    roster,
                    ciphertext,
                    key_context_digest,
                )
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let authority = self
            .authority
            .parties
            .get(party_index)
            .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
        let mut party_use = ZkAmsMkhePersistentDecryptionPartyUseV1 {
            binding_set_root: self.authority.binding_set_root,
            collective_public_key_digest: self.authority.collective_public_key_digest,
            profile_digest: roster.profile_digest(),
            roster_digest: roster.roster_digest(),
            epoch: roster.epoch(),
            cpk_transcript_digest: self.authority.cpk_transcript_digest,
            party_index: authority.party_index,
            party: authority.party,
            secret_identity_digest: authority.secret_identity_digest,
            generator_basis_digest: authority.generator_basis_digest,
            commitment_set_digest: authority.commitment_set_digest,
            commitments: authority.commitments,
            key_context_digest,
            ciphertext_digest,
            ciphertext_record_index: ciphertext.binding().record_index(),
            sample_index: ciphertext.sample_index(),
            level: ciphertext.binding().level(),
            statement_digest,
            public_contribution_set_digest: self.public_contribution_set_digest,
            commitment_context_digest: commitment_context_digest(
                self.authority.binding_set_root,
                authority.party_index,
            ),
            equation_contract_digest: self.equation_contract_digest,
            short_solution_assumption_digest: self.short_solution_assumption_digest,
            use_digest: [0; 32],
        };
        party_use.use_digest = party_use_digest(&party_use)?;
        validate_party_use(&party_use)?;
        Ok(party_use)
    }

    pub(super) fn consume_party_use(
        &self,
        statement: ZkAmsMkheDecryptionStatementV1<'_>,
        party_index: usize,
        party_use: ZkAmsMkhePersistentDecryptionPartyUseV1,
        party_state: &ZkAmsMkheCollectivePartyStateV1,
    ) -> Result<PersistentDecryptionProofBindingV1, ZkAmsMkheErrorV1> {
        let expected = self.mint_party_use(statement, party_index)?;
        if party_use != expected {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        validate_party_state_axes(&self.roster, statement, party_index, party_state)?;
        let binding = party_state
            .persistent_secret_binding_for(&self.roster, PersistentWitnessConsumerV1::Decryption)?;
        if binding.identity_digest() != party_use.secret_identity_digest
            || binding.commitment_set_digest() != party_use.commitment_set_digest
            || binding.commitments() != &party_use.commitments
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.proof_binding_from_use(statement, party_use)
    }

    pub(super) fn proof_binding(
        &self,
        statement: ZkAmsMkheDecryptionStatementV1<'_>,
        party_index: usize,
    ) -> Result<PersistentDecryptionProofBindingV1, ZkAmsMkheErrorV1> {
        let party_use = self.mint_party_use(statement, party_index)?;
        self.proof_binding_from_use(statement, party_use)
    }

    fn proof_binding_from_use(
        &self,
        statement: ZkAmsMkheDecryptionStatementV1<'_>,
        party_use: ZkAmsMkhePersistentDecryptionPartyUseV1,
    ) -> Result<PersistentDecryptionProofBindingV1, ZkAmsMkheErrorV1> {
        self.proof_binding_from_use_digest_v1(statement.binding_digest(), party_use)
    }

    fn proof_binding_from_use_digest_v1(
        &self,
        statement_binding_digest: [u8; 32],
        party_use: ZkAmsMkhePersistentDecryptionPartyUseV1,
    ) -> Result<PersistentDecryptionProofBindingV1, ZkAmsMkheErrorV1> {
        validate_party_use(&party_use)?;
        if statement_binding_digest == [0; 32]
            || party_use.statement_digest != statement_binding_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut value = PersistentDecryptionProofBindingV1 {
            binding_set_root: party_use.binding_set_root,
            collective_public_key_digest: party_use.collective_public_key_digest,
            party_index: party_use.party_index,
            party: party_use.party,
            secret_identity_digest: party_use.secret_identity_digest,
            generator_basis_digest: party_use.generator_basis_digest,
            commitment_set_digest: party_use.commitment_set_digest,
            commitments: party_use.commitments,
            use_digest: party_use.use_digest,
            equation_contract_digest: party_use.equation_contract_digest,
            short_solution_assumption_digest: party_use.short_solution_assumption_digest,
            binding_digest: [0; 32],
        };
        value.binding_digest =
            proof_binding_digest_from_statement_binding_v1(statement_binding_digest, &value)?;
        if value.binding_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(value)
    }
}

fn validate_roster_statement(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
) -> Result<(), ZkAmsMkheErrorV1> {
    roster.validate()?;
    statement.validate()?;
    if roster.to_wire_roster()? != *statement.roster()
        || roster.key_material_digest()
            != statement
                .collective_public_key()
                .key_material_digest_internal()
        || roster.epoch() != statement.collective_public_key().epoch()
        || roster.roster_digest() != statement.collective_public_key().roster_digest()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn validate_party_state_axes(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    party_index: usize,
    state: &ZkAmsMkheCollectivePartyStateV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let share = statement
        .public_key_shares()
        .get(party_index)
        .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
    if usize::from(state.party_index()) != party_index
        || state.party() != roster.participants()[party_index].party()
        || state.profile_digest_internal() != roster.profile_digest()
        || state.roster_digest_internal() != roster.roster_digest()
        || state.key_material_digest_internal() != roster.key_material_digest()
        || state.security_certificate_digest_internal()
            != statement
                .collective_public_key()
                .security_certificate_digest()
        || state.public_share_digest() != share.digest()
        || state.transcript_digest() != statement.collective_public_key().transcript_digest()
        || state.epoch() != roster.epoch()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn validate_streaming_party_state_axes_v1(
    context: &ZkAmsMkhePersistentDecryptionVerificationContextV1,
    party_index: usize,
    state: &ZkAmsMkheCollectivePartyStateV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    context.validate_streaming_context_v1()?;
    let streaming = context
        .streaming_authority
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let participant = context
        .roster
        .participants()
        .get(party_index)
        .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
    if usize::from(state.party_index()) != party_index
        || state.party() != participant.party()
        || state.profile_digest_internal() != context.roster.profile_digest()
        || state.security_certificate_digest_internal()
            != zk_ams_mkhe_security_certificate_v1()?.certificate_digest()
        || state.roster_digest_internal() != context.roster.roster_digest()
        || state.key_material_digest_internal() != context.roster.key_material_digest()
        || state.public_share_digest() != streaming.share_digests[party_index]
        || state.transcript_digest() != streaming.cpk_transcript_digest
        || state.epoch() != context.roster.epoch()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn validate_party_authority(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    party_index: usize,
    authority: &PersistentDecryptionPartyAuthorityV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if usize::from(authority.party_index) != party_index
        || authority.party != roster.participants()[party_index].party()
        || authority.secret_identity_digest == [0; 32]
        || persistent_commitment_set_digest(
            authority.generator_basis_digest,
            &authority.commitments,
        )? != authority.commitment_set_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn validate_party_use(
    party_use: &ZkAmsMkhePersistentDecryptionPartyUseV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if party_use.binding_set_root == [0; 32]
        || party_use.collective_public_key_digest == [0; 32]
        || party_use.profile_digest == [0; 32]
        || party_use.roster_digest == [0; 32]
        || party_use.epoch == 0
        || party_use.cpk_transcript_digest == [0; 32]
        || usize::from(party_use.party_index) >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || party_use.party.to_bytes() == [0; 32]
        || party_use.secret_identity_digest == [0; 32]
        || party_use.key_context_digest == [0; 32]
        || party_use.ciphertext_digest == [0; 32]
        || party_use.statement_digest == [0; 32]
        || party_use.public_contribution_set_digest == [0; 32]
        || party_use.commitment_context_digest == [0; 32]
        || party_use.equation_contract_digest != digest_literal(TRANSITIVE_EQUATION_CONTRACT_V1)
        || party_use.short_solution_assumption_digest
            != digest_literal(SHORT_SOLUTION_ASSUMPTION_V1)
        || persistent_commitment_set_digest(
            party_use.generator_basis_digest,
            &party_use.commitments,
        )? != party_use.commitment_set_digest
        || party_use.use_digest == [0; 32]
        || party_use.use_digest != party_use_digest(party_use)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn public_contribution_set_digest(
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(CONTRIBUTION_SET_DOMAIN_V1);
    hash.update(&statement.key_context_digest());
    hash.update(&statement.collective_public_key().digest());
    for (index, share) in statement.public_key_shares().iter().enumerate() {
        hash.update(
            &u32::try_from(index)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        hash.update(&share.party().to_bytes());
        hash.update(&share.digest());
        hash.update(
            &u32::try_from(share.party_public_b().residues().len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        for residue in share.party_public_b().residues() {
            hash.update(&residue.to_be_bytes());
        }
    }
    Ok(hash.finalize())
}

fn commitment_context_digest(binding_set_root: [u8; 32], party_index: u8) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(COMMITMENT_CONTEXT_DOMAIN_V1);
    hash.update(&binding_set_root);
    hash.update(&u32::from(party_index).to_be_bytes());
    hash.finalize()
}

fn party_use_digest(
    party_use: &ZkAmsMkhePersistentDecryptionPartyUseV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(PARTY_USE_DOMAIN_V1);
    hash.update(&party_use.binding_set_root);
    hash.update(&party_use.collective_public_key_digest);
    hash.update(&party_use.profile_digest);
    hash.update(&party_use.roster_digest);
    hash.update(&party_use.epoch.to_be_bytes());
    hash.update(&party_use.cpk_transcript_digest);
    hash.update(&[party_use.party_index]);
    hash.update(&party_use.party.to_bytes());
    hash.update(&party_use.secret_identity_digest);
    hash.update(&party_use.generator_basis_digest);
    hash.update(&party_use.commitment_set_digest);
    hash.update(&party_use.key_context_digest);
    hash.update(&party_use.ciphertext_digest);
    hash.update(&party_use.ciphertext_record_index.to_be_bytes());
    hash.update(&party_use.sample_index.to_be_bytes());
    hash.update(&[party_use.level]);
    hash.update(&party_use.statement_digest);
    hash.update(&party_use.public_contribution_set_digest);
    hash.update(&party_use.commitment_context_digest);
    hash.update(&party_use.equation_contract_digest);
    hash.update(&party_use.short_solution_assumption_digest);
    hash_points(&mut hash, &party_use.commitments)?;
    Ok(hash.finalize())
}

fn proof_binding_digest_from_statement_binding_v1(
    statement_binding_digest: [u8; 32],
    binding: &PersistentDecryptionProofBindingV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if statement_binding_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut hash = Keccak256::new();
    hash.update(PROOF_BINDING_DOMAIN_V1);
    hash.update(&statement_binding_digest);
    hash.update(&binding.binding_set_root);
    hash.update(&binding.collective_public_key_digest);
    hash.update(&[binding.party_index]);
    hash.update(&binding.party.to_bytes());
    hash.update(&binding.secret_identity_digest);
    hash.update(&binding.generator_basis_digest);
    hash.update(&binding.commitment_set_digest);
    hash.update(&binding.use_digest);
    hash.update(&binding.equation_contract_digest);
    hash.update(&binding.short_solution_assumption_digest);
    hash_points(&mut hash, &binding.commitments)?;
    Ok(hash.finalize())
}

fn hash_points(
    hash: &mut Keccak256,
    points: &[Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
) -> Result<(), ZkAmsMkheErrorV1> {
    for (index, point) in points.iter().enumerate() {
        hash.update(
            &u32::try_from(index)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        hash.update(
            &point
                .to_non_identity_wire_bytes()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        );
    }
    Ok(())
}

fn digest_literal(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(bytes);
    hash.finalize()
}
