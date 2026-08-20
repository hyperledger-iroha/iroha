//! Public-CPK-owned persistent-secret authority for split decryption.
//!
//! A decryption proof uses one `secret_response` in both public RNS equations
//!
//! ```text
//! b_i     = -a * s_i + t * e_i
//! share_i = c_1 * s_i + t * z_i.
//! ```
//!
//! The complete CPK verifier separately proves that the exact persistent T256 commitment opens to a
//! short `s_i` in the first equation. This module consumes all eight secret-free, proof-verified
//! CPK contributions at ceremony time, retains their actual ordered commitment points, and makes
//! that opaque authority mandatory at prove, verify, split, reconstruct, and combine. Proving
//! additionally reopens the selected state-owned commitment and rejects a mismatch before
//! randomness is used. No verifier needs another party's private state, and no caller-supplied
//! digest can mint the authority.
//!
//! Equality remains the transitive short-solution claim for the shared CPK
//! equation; it is not presented as a direct Pedersen cross-opening. Release
//! readiness stays closed until that short-solution/SIS assumption has an
//! independently pinned certificate and a replacement release-size KAT.
#[path = "persistent_decryption_direct_equality_v1.rs"]
mod persistent_decryption_direct_equality_v1;
#[path = "persistent_decryption_response_link.rs"]
mod persistent_decryption_response_link;
#[cfg(test)]
use super::decryption::ZkAmsMkheDecryptionStatementV1;
#[cfg(test)]
use super::direct_object_transport::validate_zk_ams_mkhe_direct_object_v1;
use super::{
    ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::{
        PersistentWitnessConsumerV1, VerifiedPersistentWitnessBindingSetV1,
        VerifiedPersistentWitnessBindingV1, mint_collective_secret_binding_from_verified_cpk_v1,
        persistent_commitment_set_digest,
    },
    collective::{
        VerifiedCollectivePublicKeyShareStagedAdmissionV1, ZeroizingRns,
        ZkAmsMkheCollectivePartyStateV1, ZkAmsMkheCollectivePublicKeyShareV1,
        ZkAmsMkheCollectivePublicKeyV1, ZkAmsMkhePreparedCollectivePublicAV1,
        ZkAmsMkheStagedCollectivePublicKeyAdmissionV1, cks_staged_residue_digests_v1,
        consume_collective_public_key_share_for_staging_v1, cpk_party_b_payload_blake3_v1,
        finalize_collective_public_key_from_staged_v1, prepare_zk_ams_mkhe_collective_public_a_v1,
        validate_collective_public_key_share_for_verified_cpk_compact_v1,
    },
    collective_eval_keys::ZkAmsMkheTrustedCksContextV1,
    cpk_relation::{VerifiedZkAmsMkheCpkContributionV1, ZK_AMS_MKHE_CPK_PARTY_B_OBJECT_BYTES_V1},
    decryption::{
        DecryptionCiphertextAxesV1, decryption_key_context_digest_from_bounded_cpk_v1,
        decryption_statement_binding_digest_from_axes_v1,
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
    wire::{ZkAmsMkheGovernedRosterWireV1, ZkAmsMkheRnsPolynomialWireV1},
    zk_ams_mkhe_security_certificate_v1,
};
use crate::{
    generalized_bulletproof::try_exact_capacity_vec_v1,
    vega::{VegaT256PointV1 as Point, sponge::Keccak256},
};
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
const STAGED_CPK_BATCH_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-public-key.staged-batch";
const CKS_RNS_NATIVE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-polynomial-digest";
const CKS_RNS_WIRE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-polynomial";
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
/// The capability is move-only and has no decoder, public constructor, raw pointer constructor, or
/// `Clone` implementation. It can be minted only by the exact eight-party bounded CPK ceremony
/// below and is consumed by the explicit streaming-statement constructor.
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
/// It is crate-private because the verified CPK contribution type is itself a sealed internal
/// capability. At most one public share is borrowed by each transition; no array of eight
/// release-sized shares is accepted or retained. The buffer bound covers this algorithm, not
/// arbitrary storage retained by a caller's CAS implementation. Release deployment must use
/// bounded/external staging and remains blocked on the authenticated whole-worker residency run.
#[allow(dead_code)]
pub(super) struct ZkAmsMkheStreamingDecryptionAuthorityBuilderV1 {
    roster: ZkAmsMkheGovernedActiveRosterV1,
    cpk_transcript_digest: [u8; 32],
    prepared_public_a: ZkAmsMkhePreparedCollectivePublicAV1,
    next_party_index: usize,
    failed: bool,
    admissions: Vec<VerifiedCollectivePublicKeyShareStagedAdmissionV1>,
    bindings: Vec<VerifiedPersistentWitnessBindingV1>,
    party_b_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    verification_read_receipts: Vec<ZkAmsMkheDirectObjectReadReceiptV1>,
    publication_receipts: Vec<ZkAmsMkheDirectObjectPublicationReceiptV1>,
    provider_identity: Option<[u8; 32]>,
    snapshot_identity: Option<[u8; 32]>,
    publication_identity: Option<[u8; 32]>,
}
struct StagedCpkBatchSealV1;
/// Sealed first-stage output retaining no party `b_i` or proof owner.
///
/// Common `a` is the exact allocation unwrapped from the builder-owned
/// prepared context. All party-sized objects are represented only by sealed
/// admissions, immutable pointers, and complete read/publication receipts.
// TODO: Remove the targeted dead-code expectations in this staged corridor
// when the fail-closed CPK release gate wires its production consumer.
pub(super) struct ZkAmsMkheStagedCpkBatchV1 {
    _seal: StagedCpkBatchSealV1,
    roster: ZkAmsMkheGovernedActiveRosterV1,
    cpk_transcript_digest: [u8; 32],
    common_public_a: ZeroizingRns,
    admissions:
        [VerifiedCollectivePublicKeyShareStagedAdmissionV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    bindings: [VerifiedPersistentWitnessBindingV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    party_b_pointers: [ZkAmsMkheDirectObjectPointerV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    verification_read_receipts:
        [ZkAmsMkheDirectObjectReadReceiptV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    publication_receipts:
        [ZkAmsMkheDirectObjectPublicationReceiptV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    publication_identity: [u8; 32],
    batch_digest: [u8; 32],
    failed: bool,
}
/// Exact successful second-stage products. No constructor accepts raw digests.
pub(super) struct ZkAmsMkheFinalizedStagedCpkV1 {
    collective_public_key: ZkAmsMkheCollectivePublicKeyV1,
    collective_public_key_admission: ZkAmsMkheStagedCollectivePublicKeyAdmissionV1,
    trusted_cks_context: ZkAmsMkheTrustedCksContextV1,
    persistent_context: ZkAmsMkhePersistentDecryptionVerificationContextV1,
    streaming_decryption_authority: ZkAmsMkheStreamingDecryptionAuthorityV1,
}
impl ZkAmsMkheFinalizedStagedCpkV1 {
    #[allow(clippy::type_complexity)]
    pub(super) fn into_parts(
        self,
    ) -> (
        ZkAmsMkheCollectivePublicKeyV1,
        ZkAmsMkheStagedCollectivePublicKeyAdmissionV1,
        ZkAmsMkheTrustedCksContextV1,
        ZkAmsMkhePersistentDecryptionVerificationContextV1,
        ZkAmsMkheStreamingDecryptionAuthorityV1,
    ) {
        (
            self.collective_public_key,
            self.collective_public_key_admission,
            self.trusted_cks_context,
            self.persistent_context,
            self.streaming_decryption_authority,
        )
    }
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
/// It is neither `Clone` nor serializable. Each compact statement binding can issue a fresh set for
/// the exact statement; replay rejection therefore rests on the bound ciphertext, record, sample,
/// and admission state rather than on pretending the retained context is a one-shot token. The
/// public prover still consumes each issued use, and there is no omitted-capability or raw-digest
/// proving overload.
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
    #[expect(
        dead_code,
        reason = "axis corruption seam retained for native capability reference tests"
    )]
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
    /// Begin the exact bounded ceremony before generating any party secret.
    ///
    /// This transition allocates the sole prepared common-`a` backing and all bounded metadata
    /// capacity. It deliberately does not allocate the aggregate polynomial; generation and proof
    /// validation can therefore never coexist with that second `P`-sized owner.
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
        let prepared_public_a =
            prepare_zk_ams_mkhe_collective_public_a_v1(roster, cpk_transcript_digest)?;
        let admissions = try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let bindings = try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let party_b_pointers = try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let verification_read_receipts =
            try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let publication_receipts = try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(Self {
            roster: *roster,
            cpk_transcript_digest,
            prepared_public_a,
            next_party_index: 0,
            failed: false,
            admissions,
            bindings,
            party_b_pointers,
            verification_read_receipts,
            publication_receipts,
            provider_identity: None,
            snapshot_identity: None,
            publication_identity: None,
        })
    }
    /// Borrow the builder-owned common `a` for the sole next party generator.
    /// Cloning this prepared context causes `finish_staging_v1` to fail closed:
    /// sealing requires unique ownership of its `Arc<Vec<u64>>` backing.
    pub(super) const fn prepared_public_a_v1(&self) -> &ZkAmsMkhePreparedCollectivePublicAV1 {
        &self.prepared_public_a
    }
    /// Trusted roster retained by the public single-party ceremony facade.
    pub(super) const fn roster_v1(&self) -> &ZkAmsMkheGovernedActiveRosterV1 {
        &self.roster
    }
    /// Exact transcript retained by the public single-party ceremony facade.
    pub(super) const fn cpk_transcript_digest_v1(&self) -> [u8; 32] {
        self.cpk_transcript_digest
    }
    /// Consume and publish the sole next governed CPK contribution.
    ///
    /// The transition is poisoned before any fallible or backend-controlled
    /// operation. An error or caught unwind therefore makes `finish`
    /// permanently unavailable; no partially observed ceremony can resume.
    pub(super) fn absorb_verified_party_v1<P>(
        &mut self,
        contribution: VerifiedZkAmsMkheCpkContributionV1,
        share: ZkAmsMkheCollectivePublicKeyShareV1,
        party_state: &mut ZkAmsMkheCollectivePartyStateV1,
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
        let result =
            self.absorb_verified_party_inner_v1(contribution, share, party_state, publisher);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }
    /// Test-only bounded stand-in for the complete CPK relation verifier.
    ///
    /// It preserves the production ownership topology: one share is published,
    /// independently reread, consumed into a compact admission, and dropped
    /// before the next party is generated. No array of shares is accepted.
    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "synthetic CPK party admission retained for bounded builder reference tests"
    )]
    pub(super) fn absorb_test_party_v1<P>(
        &mut self,
        share: ZkAmsMkheCollectivePublicKeyShareV1,
        party_state: &mut ZkAmsMkheCollectivePartyStateV1,
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
        let result = self.absorb_test_party_inner_v1(share, party_state, publisher);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }
    #[cfg(test)]
    fn absorb_test_party_inner_v1<P>(
        &mut self,
        share: ZkAmsMkheCollectivePublicKeyShareV1,
        party_state: &mut ZkAmsMkheCollectivePartyStateV1,
        publisher: &mut P,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        let party_index = self.next_party_index;
        let prepared_owner = self.prepared_public_a.public_a().shared_residues();
        let share_owner = share.public_a().shared_residues();
        if !std::sync::Arc::ptr_eq(&prepared_owner, &share_owner) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        drop((prepared_owner, share_owner));
        let share_digest = validate_collective_public_key_share_for_verified_cpk_compact_v1(
            &self.roster,
            self.cpk_transcript_digest,
            party_index,
            &share,
        )?;
        let (state_binding, verifier_binding) =
            party_state.test_state_owned_cpk_bindings_v1(&self.roster, &share)?;
        let publication_receipt = publish_canonical_party_b_v1(share.party_public_b(), publisher)?;
        let expected_pointer = publication_receipt.pointer();
        let verification_read_receipt = validate_zk_ams_mkhe_direct_object_v1(
            ZkAmsMkheDirectObjectKindV1::CpkPartyB,
            expected_pointer,
            publisher,
        )?;
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
        let admission = consume_collective_public_key_share_for_staging_v1(
            &self.roster,
            self.cpk_transcript_digest,
            party_index,
            share,
        )?;
        if admission.share_digest() != share_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        party_state.admit_staged_verified_cpk_binding_v1(
            &self.roster,
            &admission,
            state_binding,
        )?;
        self.provider_identity = Some(verification_snapshot.provider_identity());
        self.snapshot_identity = Some(verification_snapshot.snapshot_identity());
        self.publication_identity = Some(publication_receipt.publication_identity());
        self.admissions.push(admission);
        self.bindings.push(verifier_binding);
        self.party_b_pointers.push(expected_pointer);
        self.verification_read_receipts
            .push(verification_read_receipt);
        self.publication_receipts.push(publication_receipt);
        self.next_party_index += 1;
        Ok(())
    }
    fn absorb_verified_party_inner_v1<P>(
        &mut self,
        contribution: VerifiedZkAmsMkheCpkContributionV1,
        share: ZkAmsMkheCollectivePublicKeyShareV1,
        party_state: &mut ZkAmsMkheCollectivePartyStateV1,
        publisher: &mut P,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        let party_index = self.next_party_index;
        let prepared_owner = self.prepared_public_a.public_a().shared_residues();
        let share_owner = share.public_a().shared_residues();
        if !std::sync::Arc::ptr_eq(&prepared_owner, &share_owner) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        drop((prepared_owner, share_owner));
        let share_digest = validate_collective_public_key_share_for_verified_cpk_compact_v1(
            &self.roster,
            self.cpk_transcript_digest,
            party_index,
            &share,
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
        let admission = consume_collective_public_key_share_for_staging_v1(
            &self.roster,
            self.cpk_transcript_digest,
            party_index,
            share,
        )?;
        if admission.share_digest() != share_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let (state_binding, verifier_binding) = binding.fork_for_state_and_verifier_v1();
        self.provider_identity = Some(verification_snapshot.provider_identity());
        self.snapshot_identity = Some(verification_snapshot.snapshot_identity());
        self.publication_identity = Some(publication_receipt.publication_identity());
        self.admissions.push(admission);
        self.bindings.push(verifier_binding);
        self.party_b_pointers.push(expected_pointer);
        self.verification_read_receipts
            .push(verification_read_receipt);
        self.publication_receipts.push(publication_receipt);
        let staged_admission = self
            .admissions
            .last()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        party_state.admit_staged_verified_cpk_binding_v1(
            &self.roster,
            staged_admission,
            state_binding,
        )?;
        self.next_party_index += 1;
        Ok(())
    }
    /// Seal only after all eight ordered shares/proofs were consumed.
    ///
    /// `Arc::try_unwrap` is a structural residency guard: a caller that kept a
    /// prepared/share backing owner cannot advance to aggregation. No
    /// release-sized allocation occurs in this transition.
    pub(super) fn finish_staging_v1(
        mut self,
    ) -> Result<ZkAmsMkheStagedCpkBatchV1, ZkAmsMkheErrorV1> {
        if self.failed
            || self.next_party_index != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || self.admissions.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
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
        let profile = release_profile_v1();
        profile.validate()?;
        let common_public_a = self.prepared_public_a.public_a().shared_residues();
        drop(self.prepared_public_a);
        let common_public_a = std::sync::Arc::try_unwrap(common_public_a)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let common_public_a = ZeroizingRns::from_canonical_flat_v1(&profile, common_public_a)?;
        let admissions: [VerifiedCollectivePublicKeyShareStagedAdmissionV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = self
            .admissions
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
        let bindings: [VerifiedPersistentWitnessBindingV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            self.bindings
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
        let party_b_pointers: [ZkAmsMkheDirectObjectPointerV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            self.party_b_pointers
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
        let verification_read_receipts: [ZkAmsMkheDirectObjectReadReceiptV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = self
            .verification_read_receipts
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
        let publication_receipts: [ZkAmsMkheDirectObjectPublicationReceiptV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = self
            .publication_receipts
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
        let mut batch = ZkAmsMkheStagedCpkBatchV1 {
            _seal: StagedCpkBatchSealV1,
            roster: self.roster,
            cpk_transcript_digest: self.cpk_transcript_digest,
            common_public_a,
            admissions,
            bindings,
            party_b_pointers,
            verification_read_receipts,
            publication_receipts,
            provider_identity,
            snapshot_identity,
            publication_identity,
            batch_digest: [0; 32],
            failed: false,
        };
        batch.batch_digest = staged_cpk_batch_digest_v1(&batch);
        batch.validate_v1()?;
        Ok(batch)
    }
}
fn staged_cpk_batch_digest_v1(batch: &ZkAmsMkheStagedCpkBatchV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(STAGED_CPK_BATCH_DOMAIN_V1);
    hash.update(&batch.roster.profile_digest());
    hash.update(&batch.roster.roster_digest());
    hash.update(&batch.roster.key_material_digest());
    hash.update(&batch.roster.epoch().to_be_bytes());
    hash.update(&batch.cpk_transcript_digest);
    for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        hash.update(&u32::try_from(party_index).unwrap_or(u32::MAX).to_be_bytes());
        hash.update(&batch.admissions[party_index].admission_digest());
        hash.update(&batch.party_b_pointers[party_index].pointer_digest());
        hash.update(&batch.verification_read_receipts[party_index].receipt_digest());
        hash.update(&batch.publication_receipts[party_index].receipt_digest());
    }
    hash.update(&batch.provider_identity);
    hash.update(&batch.snapshot_identity);
    hash.update(&batch.publication_identity);
    hash.finalize()
}
impl ZkAmsMkheStagedCpkBatchV1 {
    fn validate_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        self.roster.validate()?;
        let profile = release_profile_v1();
        profile.validate()?;
        if self.cpk_transcript_digest == [0; 32]
            || self.provider_identity == [0; 32]
            || self.snapshot_identity == [0; 32]
            || self.publication_identity == [0; 32]
            || self.batch_digest == [0; 32]
            || self.batch_digest != staged_cpk_batch_digest_v1(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let common_public_a_digests =
            cks_staged_residue_digests_v1(&profile, self.common_public_a.coefficients())?;
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            let admission = &self.admissions[party_index];
            admission.validate_for_v1(&self.roster, self.cpk_transcript_digest, party_index)?;
            let pointer = self.party_b_pointers[party_index];
            let verification_receipt = &self.verification_read_receipts[party_index];
            let verification_snapshot = verification_receipt.snapshot();
            let publication_receipt = &self.publication_receipts[party_index];
            let publication_read_receipt = publication_receipt.post_publish_read_receipt();
            let publication_snapshot = publication_read_receipt.snapshot();
            if admission.public_a_digests() != common_public_a_digests
                || pointer.kind() != ZkAmsMkheDirectObjectKindV1::CpkPartyB
                || pointer.payload_bytes() != ZK_AMS_MKHE_CPK_PARTY_B_OBJECT_BYTES_V1 as u64
                || self.party_b_pointers[..party_index].contains(&pointer)
                || verification_snapshot.pointer() != pointer
                || verification_snapshot.provider_identity() != self.provider_identity
                || verification_snapshot.snapshot_identity() != self.snapshot_identity
                || verification_receipt.canonical_bytes() != pointer.payload_bytes()
                || verification_receipt.payload_blake3() != pointer.payload_blake3()
                || publication_receipt.pointer() != pointer
                || publication_receipt.publication_identity() != self.publication_identity
                || publication_snapshot.pointer() != pointer
                || publication_snapshot.provider_identity() != self.provider_identity
                || publication_snapshot.snapshot_identity() != self.snapshot_identity
                || publication_read_receipt.canonical_bytes() != pointer.payload_bytes()
                || publication_read_receipt.payload_blake3() != pointer.payload_blake3()
                || verification_receipt.receipt_digest()
                    != publication_read_receipt.receipt_digest()
                || self.verification_read_receipts[..party_index]
                    .iter()
                    .any(|prior| prior.receipt_digest() == verification_receipt.receipt_digest())
                || self.publication_receipts[..party_index]
                    .iter()
                    .any(|prior| prior.receipt_digest() == publication_receipt.receipt_digest())
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
        }
        Ok(())
    }
    /// Consume the sealed stage and build every compact authority.
    ///
    /// Major release payload for the direct aggregation pass is
    /// `2P + 8 KiB = 79_699_968 B` (`P = 39_845_888`): common `a`, one
    /// zeroizing aggregate, and one direct-I/O chunk. The sealed admissions and
    /// witness bindings retain only fixed digests and eight curve commitments
    /// per party; allocator/struct metadata is excluded from this equation. No
    /// current `P`-sized party `b_i` is decoded. The complete finalization
    /// corridor peaks later at `2P + L = 80_740_352 B` (exactly `77 MiB`),
    /// while one `L = 1 MiB` common-`a` limb is re-derived beside both final
    /// key polynomials; that limb does not overlap the 8-KiB direct-I/O chunk.
    pub(super) fn finalize_v1<P>(
        mut self,
        provider: &mut P,
    ) -> Result<ZkAmsMkheFinalizedStagedCpkV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if self.failed {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        // Poison before validation, allocation, or provider-controlled work.
        self.failed = true;
        self.finalize_inner_v1(provider)
    }
    fn finalize_inner_v1<P>(
        self,
        provider: &mut P,
    ) -> Result<ZkAmsMkheFinalizedStagedCpkV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        self.validate_v1()?;
        let profile = release_profile_v1();
        profile.validate()?;
        // The sole fallible ring allocation precedes the first provider call.
        let mut aggregate_b = ZeroizingRns::zero_exact_v1(&profile)?;
        let mut party_public_b_native_digests = [[0_u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
        let mut party_public_b_wire_digests = [[0_u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            let (native_digest, wire_digest) = aggregate_staged_party_b_v1(
                self.party_b_pointers[party_index],
                self.provider_identity,
                self.snapshot_identity,
                &self.verification_read_receipts[party_index],
                &self.publication_receipts[party_index],
                provider,
                &mut aggregate_b,
            )?;
            party_public_b_native_digests[party_index] = native_digest;
            party_public_b_wire_digests[party_index] = wire_digest;
        }
        let share_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            core::array::from_fn(|party_index| self.admissions[party_index].share_digest());
        let (public_key_a_native_digest, public_key_a_wire_digest) =
            self.admissions[0].public_a_digests();
        let (collective_public_key, collective_public_key_admission) =
            finalize_collective_public_key_from_staged_v1(
                &self.roster,
                self.cpk_transcript_digest,
                self.common_public_a,
                aggregate_b,
                self.admissions,
                party_public_b_native_digests,
                party_public_b_wire_digests,
            )?;
        let collective_public_key_digest = collective_public_key.digest();
        let trusted_cks_context = ZkAmsMkheTrustedCksContextV1::from_staged_verified_digests(
            self.roster.to_wire_roster()?,
            self.roster.key_material_digest(),
            self.cpk_transcript_digest,
            collective_public_key_digest,
            share_digests,
            public_key_a_native_digest,
            public_key_a_wire_digest,
            party_public_b_native_digests,
            party_public_b_wire_digests,
        )?;
        let key_context_digest = decryption_key_context_digest_from_bounded_cpk_v1(
            &self.roster,
            self.cpk_transcript_digest,
            collective_public_key_digest,
            share_digests,
            |party_index, hash| {
                stream_canonical_party_b_into_hash_v1(
                    self.party_b_pointers[party_index],
                    self.provider_identity,
                    self.snapshot_identity,
                    provider,
                    hash,
                )
            },
        )?;
        let public_contribution_set_digest = public_contribution_set_digest_from_streamed_cpk_v1(
            &self.roster,
            key_context_digest,
            collective_public_key_digest,
            share_digests,
            self.party_b_pointers,
            self.provider_identity,
            self.snapshot_identity,
            provider,
        )?;
        let binding_refs = core::array::from_fn(|index| &self.bindings[index]);
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
            party_b_pointers: self.party_b_pointers,
            verification_read_receipt_digests,
            publication_receipt_digests,
            provider_identity: self.provider_identity,
            snapshot_identity: self.snapshot_identity,
            publication_identity: self.publication_identity,
            authority_digest: [0; 32],
        };
        streaming_authority.authority_digest =
            streaming_decryption_authority_digest_v1(&streaming_authority)?;
        validate_streaming_decryption_authority_v1(&self.roster, &streaming_authority)?;
        let context_authority_digest = streaming_authority.authority_digest;
        let persistent_context = ZkAmsMkhePersistentDecryptionVerificationContextV1 {
            roster: self.roster,
            authority,
            streaming_authority: Some(streaming_authority),
            public_contribution_set_digest,
            equation_contract_digest: digest_literal(TRANSITIVE_EQUATION_CONTRACT_V1),
            short_solution_assumption_digest: digest_literal(SHORT_SOLUTION_ASSUMPTION_V1),
        };
        persistent_context.validate_streaming_context_v1()?;
        let streaming_decryption_authority = ZkAmsMkheStreamingDecryptionAuthorityV1 {
            _seal: StreamingDecryptionAuthoritySealV1,
            context_authority_digest,
        };
        Ok(ZkAmsMkheFinalizedStagedCpkV1 {
            collective_public_key,
            collective_public_key_admission,
            trusted_cks_context,
            persistent_context,
            streaming_decryption_authority,
        })
    }
}
#[allow(clippy::too_many_arguments)]
fn aggregate_staged_party_b_v1<P>(
    pointer: ZkAmsMkheDirectObjectPointerV1,
    expected_provider_identity: [u8; 32],
    expected_snapshot_identity: [u8; 32],
    staged_verification_receipt: &ZkAmsMkheDirectObjectReadReceiptV1,
    staged_publication_receipt: &ZkAmsMkheDirectObjectPublicationReceiptV1,
    provider: &mut P,
    aggregate_b: &mut ZeroizingRns,
) -> Result<([u8; 32], [u8; 32]), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let profile = release_profile_v1();
    profile.validate()?;
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if pointer.kind() != ZkAmsMkheDirectObjectKindV1::CpkPartyB
        || pointer.payload_bytes() != ZK_AMS_MKHE_CPK_PARTY_B_OBJECT_BYTES_V1 as u64
        || aggregate_b.coefficients().len() != coefficient_count
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let encoded_count = u32::try_from(coefficient_count)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        .to_be_bytes();
    let mut native_hash = Keccak256::new();
    native_hash.update(CKS_RNS_NATIVE_DIGEST_DOMAIN_V1);
    native_hash.update(&encoded_count);
    let mut wire_hash = Keccak256::new();
    wire_hash.update(CKS_RNS_WIRE_DIGEST_DOMAIN_V1);
    wire_hash.update(&encoded_count);
    let mut transaction = ZkAmsMkheDirectObjectReadTransactionV1::begin(
        ZkAmsMkheDirectObjectKindV1::CpkPartyB,
        pointer,
        provider,
    )?;
    let mut observed_count = [0_u8; 4];
    if transaction.read_next(provider, &mut observed_count)? != observed_count.len()
        || observed_count != encoded_count
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut coefficient_index = 0_usize;
    let mut buffer = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    while transaction.remaining_bytes() != 0 {
        let read = transaction.read_next(provider, &mut buffer)?;
        if read == 0 || read % core::mem::size_of::<u64>() != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        native_hash.update(&buffer[..read]);
        wire_hash.update(&buffer[..read]);
        for encoded in buffer[..read].chunks_exact(core::mem::size_of::<u64>()) {
            let residue = u64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            );
            let limb = coefficient_index / profile.ring_degree;
            let modulus = *profile
                .moduli
                .get(limb)
                .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
            if residue >= modulus {
                return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
            }
            let aggregate = aggregate_b
                .coefficients_mut()
                .get_mut(coefficient_index)
                .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
            *aggregate = mod_add(*aggregate, residue, modulus);
            coefficient_index = coefficient_index
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
    }
    if coefficient_index != coefficient_count {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let receipt = transaction.finish(provider)?;
    let snapshot = receipt.snapshot();
    if snapshot.pointer() != pointer
        || snapshot.provider_identity() != expected_provider_identity
        || snapshot.snapshot_identity() != expected_snapshot_identity
        || receipt.canonical_bytes() != pointer.payload_bytes()
        || receipt.payload_blake3() != pointer.payload_blake3()
        || receipt.receipt_digest() != staged_verification_receipt.receipt_digest()
        || receipt.receipt_digest()
            != staged_publication_receipt
                .post_publish_read_receipt()
                .receipt_digest()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let digests = (native_hash.finalize(), wire_hash.finalize());
    if digests.0 == [0; 32] || digests.1 == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(digests)
}
/// Start the sole allocation-bounded compact-authority ceremony.
#[allow(dead_code)]
pub(super) fn begin_zk_ams_mkhe_streaming_decryption_authority_from_verified_cpk_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    cpk_transcript_digest: [u8; 32],
) -> Result<ZkAmsMkheStreamingDecryptionAuthorityBuilderV1, ZkAmsMkheErrorV1> {
    ZkAmsMkheStreamingDecryptionAuthorityBuilderV1::new(roster, cpk_transcript_digest)
}
#[allow(dead_code)]
pub(super) fn publish_canonical_party_b_v1<P>(
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
/// This is the test-only native-reference bridge. The bounded production
/// boundary is [`crate::vega::ZkAmsMkheCpkCeremonyV1`], which admits and drops
/// exactly one full share at a time. This bridge retains the old all-eight
/// native statement shape only for small-profile reference coverage.
#[cfg(test)]
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
#[cfg(test)]
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
    let mut parties = try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
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
#[expect(
    dead_code,
    reason = "state-owned native authority fixture retained for downstream reference tests"
)]
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
#[cfg(test)]
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
    /// Exact collective-key digest admitted by the same complete CPK ceremony.
    pub(super) const fn streaming_collective_key_digest_v1(&self) -> [u8; 32] {
        self.authority.collective_public_key_digest
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
        ciphertext: DecryptionCiphertextAxesV1,
        ciphertext_provider_identity: [u8; 32],
        ciphertext_snapshot_identity: [u8; 32],
        authority: ZkAmsMkheStreamingDecryptionAuthorityV1,
    ) -> Result<ZkAmsMkheStreamingDecryptionAuthorityMaterialV1, ZkAmsMkheErrorV1> {
        self.validate_streaming_context_v1()?;
        let streaming = self
            .streaming_authority
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        validate_exact_streaming_provider_snapshot_axes_v1(
            ciphertext_provider_identity,
            ciphertext_snapshot_identity,
            streaming.provider_identity,
            streaming.snapshot_identity,
        )?;
        if authority.context_authority_digest == [0; 32]
            || authority.context_authority_digest != streaming.authority_digest
            || self.roster.to_wire_roster()? != *roster
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        ciphertext.validate_for_roster_v1(roster)?;
        let ciphertext_digest = ciphertext.ciphertext_digest();
        let statement_digest = decryption_statement_binding_digest_from_axes_v1(
            roster,
            ciphertext,
            streaming.key_context_digest,
        );
        if statement_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut proof_bindings = try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            let party_use = self.mint_party_use_from_compact_axes_v1(
                roster,
                ciphertext,
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
    /// The caller supplies no digest or content address. Every axis is recomputed from the retained
    /// verified CPK authority and the canonical roster/ciphertext pair. Each returned capability is
    /// consumed by one staged prover invocation.
    pub(super) fn bind_streaming_statement_party_uses_v1(
        &self,
        roster: &ZkAmsMkheGovernedRosterWireV1,
        ciphertext: DecryptionCiphertextAxesV1,
        ciphertext_provider_identity: [u8; 32],
        ciphertext_snapshot_identity: [u8; 32],
        key_context_digest: [u8; 32],
    ) -> Result<
        [ZkAmsMkhePersistentDecryptionPartyUseV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        ZkAmsMkheErrorV1,
    > {
        self.validate_streaming_context_v1()?;
        let streaming = self
            .streaming_authority
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        validate_exact_streaming_provider_snapshot_axes_v1(
            ciphertext_provider_identity,
            ciphertext_snapshot_identity,
            streaming.provider_identity,
            streaming.snapshot_identity,
        )?;
        ciphertext.validate_for_roster_v1(roster)?;
        let statement_digest = decryption_statement_binding_digest_from_axes_v1(
            roster,
            ciphertext,
            key_context_digest,
        );
        if statement_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut uses = try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            uses.push(self.mint_party_use_from_compact_axes_v1(
                roster,
                ciphertext,
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
        ciphertext: DecryptionCiphertextAxesV1,
        ciphertext_provider_identity: [u8; 32],
        ciphertext_snapshot_identity: [u8; 32],
        key_context_digest: [u8; 32],
        party_index: usize,
        party_use: ZkAmsMkhePersistentDecryptionPartyUseV1,
        party_state: &ZkAmsMkheCollectivePartyStateV1,
    ) -> Result<PersistentDecryptionProofBindingV1, ZkAmsMkheErrorV1> {
        self.validate_streaming_context_v1()?;
        let streaming = self
            .streaming_authority
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        validate_exact_streaming_provider_snapshot_axes_v1(
            ciphertext_provider_identity,
            ciphertext_snapshot_identity,
            streaming.provider_identity,
            streaming.snapshot_identity,
        )?;
        ciphertext.validate_for_roster_v1(roster)?;
        let statement_digest = decryption_statement_binding_digest_from_axes_v1(
            roster,
            ciphertext,
            key_context_digest,
        );
        let expected = self.mint_party_use_from_compact_axes_v1(
            roster,
            ciphertext,
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
        ciphertext: DecryptionCiphertextAxesV1,
        ciphertext_provider_identity: [u8; 32],
        ciphertext_snapshot_identity: [u8; 32],
        key_context_digest: [u8; 32],
        party_b_pointers: [ZkAmsMkheDirectObjectPointerV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let Some(streaming) = self.streaming_authority.as_ref() else {
            return Ok(());
        };
        self.validate_streaming_context_v1()?;
        validate_exact_streaming_provider_snapshot_axes_v1(
            ciphertext_provider_identity,
            ciphertext_snapshot_identity,
            streaming.provider_identity,
            streaming.snapshot_identity,
        )?;
        if self.roster.to_wire_roster()? != *roster
            || key_context_digest != streaming.key_context_digest
            || party_b_pointers != streaming.party_b_pointers
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        ciphertext.validate_for_roster_v1(roster)?;
        Ok(())
    }
    /// Stable ordered-set identity for evidence inventories.
    #[must_use]
    pub const fn binding_set_root(&self) -> [u8; 32] {
        self.authority.binding_set_root
    }
    /// Bind a fresh exact eight-party use set to a later validated ciphertext.
    #[cfg(test)]
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
    #[cfg(test)]
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
    #[cfg(test)]
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
        ciphertext: DecryptionCiphertextAxesV1,
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
            || statement_digest
                != decryption_statement_binding_digest_from_axes_v1(
                    roster,
                    ciphertext,
                    key_context_digest,
                )
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        ciphertext.validate_for_roster_v1(roster)?;
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
            ciphertext_digest: ciphertext.ciphertext_digest(),
            ciphertext_record_index: ciphertext.ciphertext_record_index(),
            sample_index: ciphertext.sample_index(),
            level: ciphertext.level(),
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
    #[cfg(test)]
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
    #[cfg(test)]
    pub(super) fn proof_binding(
        &self,
        statement: ZkAmsMkheDecryptionStatementV1<'_>,
        party_index: usize,
    ) -> Result<PersistentDecryptionProofBindingV1, ZkAmsMkheErrorV1> {
        let party_use = self.mint_party_use(statement, party_index)?;
        self.proof_binding_from_use(statement, party_use)
    }
    #[cfg(test)]
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
pub(super) fn validate_exact_streaming_provider_snapshot_axes_v1(
    observed_provider_identity: [u8; 32],
    observed_snapshot_identity: [u8; 32],
    expected_provider_identity: [u8; 32],
    expected_snapshot_identity: [u8; 32],
) -> Result<(), ZkAmsMkheErrorV1> {
    if observed_provider_identity == [0; 32]
        || observed_snapshot_identity == [0; 32]
        || expected_provider_identity == [0; 32]
        || expected_snapshot_identity == [0; 32]
        || observed_provider_identity != expected_provider_identity
        || observed_snapshot_identity != expected_snapshot_identity
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}
#[cfg(test)]
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
#[cfg(test)]
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
#[cfg(test)]
#[expect(
    dead_code,
    reason = "explicit native axis validation seam retained for negative reference tests"
)]
pub(super) fn validate_party_state_axes_for_test(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    party_index: usize,
    state: &ZkAmsMkheCollectivePartyStateV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_party_state_axes(roster, statement, party_index, state)
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
#[cfg(test)]
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
