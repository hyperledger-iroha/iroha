//! Move-only, allocation-bounded collective-public-key ceremony.
//!
//! The legacy aggregate API borrows all eight release-sized shares at once.
//! This facade instead verifies and consumes exactly the next governed party.
//! Only compact admissions, commitment bindings, immutable object pointers,
//! and receipts survive a transition.  The final aggregate is allocated only
//! after every full share and relation proof has been released.

use core::mem::size_of;

use super::{
    Scalar, ZkAmsMkheErrorV1,
    active::{ZkAmsMkheGovernedActiveRosterV1, zk_ams_mkhe_active_rkg_linear_proof_security_v1},
    collective::{
        ZkAmsMkheCollectivePartyStateV1, ZkAmsMkheCollectivePublicKeyShareV1,
        ZkAmsMkheCollectivePublicKeyV1, ZkAmsMkhePreparedCollectivePublicAV1,
        ZkAmsMkheStreamingCollectiveCiphertextV1,
        ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
        ZkAmsMkheStreamingCollectiveEvalKeyBindingV1,
        bind_zk_ams_mkhe_streaming_collective_eval_key_v1,
        fork_zk_ams_mkhe_staged_collective_key_admission_v1,
        mint_zk_ams_mkhe_streaming_collective_encryption_key_authority_v1,
    },
    collective_eval_keys::{
        ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1, ZkAmsMkheTrustedCksContextV1,
        ZkAmsMkheTrustedSourceContextV1,
    },
    collective_keys::ZkAmsMkheCollectiveEvaluatedKeyManifestV1,
    cpk_relation::{
        VerifiedZkAmsMkheCpkContributionV1, ZK_AMS_MKHE_CPK_CHUNK_COEFFICIENTS_V1,
        ZK_AMS_MKHE_CPK_CHUNKS_V1, ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_BYTES_V1,
        ZK_AMS_MKHE_CPK_RELATION_BODY_BYTES_V1, ZK_AMS_MKHE_CPK_RELATION_PROOF_BYTES_V1,
        ZK_AMS_MKHE_CPK_RING_DEGREE_V1, ZK_AMS_MKHE_CPK_RNS_LIMBS_V1,
        ZK_AMS_MKHE_CPK_SECRET_MEMBERSHIP_BYTES_V1, ZkAmsMkheCpkPartyBPointerV1,
        ZkAmsMkheCpkRelationErrorV1, ZkAmsMkheCpkRelationProofPointerV1,
        ZkAmsMkheCpkShareStatementV1, verify_zk_ams_mkhe_cpk_relation_v1,
    },
    decryption::ZkAmsMkheStreamingDecryptionStatementV1,
    direct_object_transport::{
        ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1, ZkAmsMkheDirectObjectCasPublicationV1,
        ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1,
        ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheDirectObjectReadAtProviderV1,
    },
    manifest::{ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1},
    persistent_decryption_equality::{
        ZkAmsMkheFinalizedStagedCpkV1, ZkAmsMkhePersistentDecryptionVerificationContextV1,
        ZkAmsMkheStreamingDecryptionAuthorityBuilderV1, ZkAmsMkheStreamingDecryptionAuthorityV1,
    },
    wire::{ZkAmsMkheAuthenticationWireV1, ZkAmsMkheGovernedRosterWireV1},
};
use crate::vega::VegaT256PointV1;

/// Exact canonical bytes in one persistent-secret membership frame.
pub const ZK_AMS_MKHE_CPK_SECRET_MEMBERSHIP_WIRE_BYTES_V1: usize =
    ZK_AMS_MKHE_CPK_SECRET_MEMBERSHIP_BYTES_V1;

/// Exact canonical bytes in one public-error membership frame.
pub const ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_WIRE_BYTES_V1: usize =
    ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_BYTES_V1;

/// Source-derived large-payload residency accounting for one CPK transition.
///
/// This enumerates every simultaneous ring/proof/witness allocation owned by
/// the ceremony, complete native relation verifier, and caller-retained prior
/// admitted state successors. Fixed struct and
/// allocator metadata are deliberately excluded, as in the neighboring
/// decryption residency certificate. Caller-selected CAS storage, page cache,
/// and filesystem buffering are explicitly not covered and keep release
/// certification closed until an authenticated whole-worker peak exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCpkCeremonyResidencyEvidenceV1 {
    /// Sole builder-owned common-`a` residue payload.
    pub builder_common_a_bytes: u64,
    /// One live party-`b_i` residue payload; the share's `a` is only an alias.
    pub live_party_b_bytes: u64,
    /// Two retained signed `i64` witnesses plus eight T256 blindings.
    pub live_party_state_witness_bytes: u64,
    /// Up to seven already returned admitted state successors retained by the caller.
    pub maximum_prior_admitted_state_bytes: u64,
    /// Exact two-witness active-proof payload retained by the live share.
    pub live_active_share_proof_bytes: u64,
    /// Both fixed canonical membership frames retained by the owned input.
    pub membership_wire_bytes: u64,
    /// Proof-wire plus decoded-body overlap during exact proof parsing.
    pub relation_proof_decode_scratch_bytes: u64,
    /// Decoded proof plus the largest commitment-MSM term arena.
    pub relation_commitment_scratch_bytes: u64,
    /// Decoded proof plus five simultaneous one-limb RNS buffers.
    pub relation_rns_scratch_bytes: u64,
    /// Largest enumerated complete-relation verifier phase.
    pub complete_relation_verifier_scratch_bytes: u64,
    /// Exact two-polynomial native key retained while its limbs are published.
    pub final_native_collective_key_bytes: u64,
    /// Sole fixed direct-I/O buffer used for key-limb publication/readback.
    pub streaming_key_publication_scratch_bytes: u64,
    /// Exact heap payload of 76 key pointers and 76 publication receipts.
    pub streaming_key_authority_heap_bytes: u64,
    /// Native key plus bounded scratch and compact authority heap during minting.
    pub streaming_key_publication_peak_bytes: u64,
    /// Largest enumerated ceremony-owned large-payload live set.
    pub enumerated_ceremony_peak_bytes: u64,
    /// Governed 160 MiB workspace ceiling.
    pub governed_workspace_ceiling_bytes: u64,
    /// False because arbitrary CAS/page-cache residency is not source-bounded.
    pub cas_backend_residency_enumerated: bool,
    /// Whether the source-owned large-payload topology fits the ceiling.
    pub source_owned_ceiling_met: bool,
    /// Zero until an authenticated whole-worker peak run is pinned.
    pub authenticated_peak_residency_digest: [u8; 32],
    /// False while CAS/page-cache residency and the peak run remain absent.
    pub release_certified: bool,
}

impl ZkAmsMkheCpkCeremonyResidencyEvidenceV1 {
    /// Recompute every source-owned payload axis without opening a release gate.
    pub fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        let expected = derive_cpk_ceremony_residency_evidence_v1()?;
        if self != expected
            || !self.source_owned_ceiling_met
            || self.enumerated_ceremony_peak_bytes > self.governed_workspace_ceiling_bytes
            || self.cas_backend_residency_enumerated
            || self.authenticated_peak_residency_digest != [0; 32]
            || self.release_certified
        {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        Ok(())
    }
}

/// Return exact source-owned CPK residency accounting without certifying an
/// arbitrary caller-selected CAS implementation or its page cache.
pub fn zk_ams_mkhe_cpk_ceremony_residency_evidence_v1()
-> Result<ZkAmsMkheCpkCeremonyResidencyEvidenceV1, ZkAmsMkheErrorV1> {
    let evidence = derive_cpk_ceremony_residency_evidence_v1()?;
    evidence.validate()?;
    Ok(evidence)
}

fn derive_cpk_ceremony_residency_evidence_v1()
-> Result<ZkAmsMkheCpkCeremonyResidencyEvidenceV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile.validate()?;
    if profile.ring_degree != ZK_AMS_MKHE_CPK_RING_DEGREE_V1
        || profile.moduli.len() != ZK_AMS_MKHE_CPK_RNS_LIMBS_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let degree = u64::try_from(profile.ring_degree)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let limbs = u64::try_from(profile.moduli.len())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let native_rns_polynomial_bytes = degree
        .checked_mul(limbs)
        .and_then(|value| value.checked_mul(size_of::<u64>() as u64))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let builder_common_a_bytes = native_rns_polynomial_bytes;
    let live_party_b_bytes = native_rns_polynomial_bytes;
    let live_party_state_witness_bytes = degree
        .checked_mul(2)
        .and_then(|value| value.checked_mul(size_of::<i64>() as u64))
        .and_then(|value| {
            value.checked_add((ZK_AMS_MKHE_CPK_CHUNKS_V1 * size_of::<Scalar>()) as u64)
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let maximum_prior_admitted_state_bytes = live_party_state_witness_bytes
        .checked_mul((ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 - 1) as u64)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;

    let active_security = zk_ams_mkhe_active_rkg_linear_proof_security_v1()?;
    if u64::from(active_security.ring_degree) != degree
        || active_security.max_witness_polynomials < 2
        || active_security.signed_coefficient_bytes == 0
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let maximum_active_responses = degree
        .checked_mul(u64::from(active_security.max_witness_polynomials))
        .and_then(|value| value.checked_mul(u64::from(active_security.signed_coefficient_bytes)))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let active_header_bytes = u64::from(active_security.max_proof_bytes)
        .checked_sub(maximum_active_responses)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let live_active_share_proof_bytes = degree
        .checked_mul(2)
        .and_then(|value| value.checked_mul(u64::from(active_security.signed_coefficient_bytes)))
        .and_then(|value| value.checked_add(active_header_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let membership_wire_bytes = u64::try_from(
        ZK_AMS_MKHE_CPK_SECRET_MEMBERSHIP_WIRE_BYTES_V1
            + ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_WIRE_BYTES_V1,
    )
    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let decoded_relation_body_bytes = ZK_AMS_MKHE_CPK_RELATION_BODY_BYTES_V1 as u64;
    let direct_read_buffer_bytes = ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 as u64;
    let relation_proof_decode_scratch_bytes = (ZK_AMS_MKHE_CPK_RELATION_PROOF_BYTES_V1 as u64)
        .checked_add(decoded_relation_body_bytes)
        .and_then(|value| value.checked_add(direct_read_buffer_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let relation_commitment_scratch_bytes = decoded_relation_body_bytes
        .checked_add(
            u64::try_from(ZK_AMS_MKHE_CPK_CHUNK_COEFFICIENTS_V1 + 2)
                .ok()
                .and_then(|terms| terms.checked_mul(size_of::<(Scalar, VegaT256PointV1)>() as u64))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .and_then(|value| {
            value.checked_add((2 * ZK_AMS_MKHE_CPK_CHUNKS_V1 * size_of::<VegaT256PointV1>()) as u64)
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let relation_rns_scratch_bytes = decoded_relation_body_bytes
        .checked_add(
            degree
                .checked_mul(5)
                .and_then(|value| value.checked_mul(size_of::<u64>() as u64))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .and_then(|value| value.checked_add(direct_read_buffer_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let complete_relation_verifier_scratch_bytes = relation_proof_decode_scratch_bytes
        .max(relation_commitment_scratch_bytes)
        .max(relation_rns_scratch_bytes);
    let party_transition_peak_bytes = builder_common_a_bytes
        .checked_add(live_party_b_bytes)
        .and_then(|value| value.checked_add(live_party_state_witness_bytes))
        .and_then(|value| value.checked_add(maximum_prior_admitted_state_bytes))
        .and_then(|value| value.checked_add(live_active_share_proof_bytes))
        .and_then(|value| value.checked_add(membership_wire_bytes))
        .and_then(|value| value.checked_add(complete_relation_verifier_scratch_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let final_native_collective_key_bytes = native_rns_polynomial_bytes
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let streaming_key_publication_scratch_bytes = direct_read_buffer_bytes;
    let streaming_key_authority_heap_bytes = limbs
        .checked_mul(2)
        .and_then(|value| {
            value.checked_mul(
                u64::try_from(
                    size_of::<ZkAmsMkheDirectObjectPointerV1>()
                        + size_of::<ZkAmsMkheDirectObjectPublicationReceiptV1>(),
                )
                .ok()?,
            )
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let streaming_key_publication_peak_bytes = final_native_collective_key_bytes
        .checked_add(streaming_key_publication_scratch_bytes)
        .and_then(|value| value.checked_add(streaming_key_authority_heap_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let enumerated_ceremony_peak_bytes =
        party_transition_peak_bytes.max(streaming_key_publication_peak_bytes);
    let governed_workspace_ceiling_bytes = profile.max_workspace_bytes as u64;
    Ok(ZkAmsMkheCpkCeremonyResidencyEvidenceV1 {
        builder_common_a_bytes,
        live_party_b_bytes,
        live_party_state_witness_bytes,
        maximum_prior_admitted_state_bytes,
        live_active_share_proof_bytes,
        membership_wire_bytes,
        relation_proof_decode_scratch_bytes,
        relation_commitment_scratch_bytes,
        relation_rns_scratch_bytes,
        complete_relation_verifier_scratch_bytes,
        final_native_collective_key_bytes,
        streaming_key_publication_scratch_bytes,
        streaming_key_authority_heap_bytes,
        streaming_key_publication_peak_bytes,
        enumerated_ceremony_peak_bytes,
        governed_workspace_ceiling_bytes,
        cas_backend_residency_enumerated: false,
        source_owned_ceiling_met: enumerated_ceremony_peak_bytes
            <= governed_workspace_ceiling_bytes,
        authenticated_peak_residency_digest: [0; 32],
        release_certified: false,
    })
}

/// One owned party transition into the bounded CPK ceremony.
///
/// Both membership frames have fixed-size boxed backing.  A caller therefore
/// cannot smuggle an oversized-capacity `Vec` into the ceremony.  The relation
/// proof and party `b_i` remain immutable direct objects and are named only by
/// their exact typed content addresses.
pub struct ZkAmsMkheCpkPartyInputV1 {
    state: ZkAmsMkheCollectivePartyStateV1,
    share: ZkAmsMkheCollectivePublicKeyShareV1,
    party_b_pointer: ZkAmsMkheDirectObjectPointerV1,
    secret_membership_wire: Box<[u8; ZK_AMS_MKHE_CPK_SECRET_MEMBERSHIP_WIRE_BYTES_V1]>,
    error_membership_wire: Box<[u8; ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_WIRE_BYTES_V1]>,
    relation_proof_pointer: ZkAmsMkheDirectObjectPointerV1,
    authentication: ZkAmsMkheAuthenticationWireV1,
}

impl core::fmt::Debug for ZkAmsMkheCpkPartyInputV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCpkPartyInputV1")
            .field("party_index", &self.state.party_index())
            .field("party", &self.state.party())
            .field("party_b_pointer", &self.party_b_pointer)
            .field("relation_proof_pointer", &self.relation_proof_pointer)
            .field("secret_membership_wire", &"[REDACTED CANONICAL FRAME]")
            .field("error_membership_wire", &"[REDACTED CANONICAL FRAME]")
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheCpkPartyInputV1 {
    /// Bind one generated state/share pair to its exact immutable CPK evidence.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: ZkAmsMkheCollectivePartyStateV1,
        share: ZkAmsMkheCollectivePublicKeyShareV1,
        party_b_pointer: ZkAmsMkheDirectObjectPointerV1,
        secret_membership_wire: Box<[u8; ZK_AMS_MKHE_CPK_SECRET_MEMBERSHIP_WIRE_BYTES_V1]>,
        error_membership_wire: Box<[u8; ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_WIRE_BYTES_V1]>,
        relation_proof_pointer: ZkAmsMkheDirectObjectPointerV1,
        authentication: ZkAmsMkheAuthenticationWireV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if state.party_index() != share.party_index()
            || state.party() != share.party()
            || state.epoch() != share.epoch()
            || state.transcript_digest() != share.transcript_digest()
            || state.public_share_digest() != share.digest()
            || authentication.party() != state.party()
            || party_b_pointer.kind() != ZkAmsMkheDirectObjectKindV1::CpkPartyB
            || relation_proof_pointer.kind() != ZkAmsMkheDirectObjectKindV1::CpkRelationProof
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        // The sealed wrappers repeat exact kind, length, digest, and pointer
        // validation here, before this value can retain any evidence frame.
        ZkAmsMkheCpkPartyBPointerV1::from_wire_bytes_exact(&party_b_pointer.encode())
            .map_err(map_cpk_relation_error_v1)?;
        ZkAmsMkheCpkRelationProofPointerV1::from_wire_bytes_exact(&relation_proof_pointer.encode())
            .map_err(map_cpk_relation_error_v1)?;
        Ok(Self {
            state,
            share,
            party_b_pointer,
            secret_membership_wire,
            error_membership_wire,
            relation_proof_pointer,
            authentication,
        })
    }

    /// Exact governed roster position consumed by this transition.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.state.party_index()
    }
}

/// Admitted small state returned after its full public share and proof die.
///
/// The wrapper is move-only.  Its inner state carries the private verified CPK
/// binding needed by the existing decryption prover, but no party-sized public
/// polynomial or proof owner.
pub struct ZkAmsMkheAdmittedCpkPartyV1 {
    state: ZkAmsMkheCollectivePartyStateV1,
}

impl core::fmt::Debug for ZkAmsMkheAdmittedCpkPartyV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheAdmittedCpkPartyV1")
            .field("party_index", &self.state.party_index())
            .field("party", &self.state.party())
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheAdmittedCpkPartyV1 {
    /// Borrow the purpose-bound state for decryption-share proving.
    #[must_use]
    pub const fn state(&self) -> &ZkAmsMkheCollectivePartyStateV1 {
        &self.state
    }

    /// Consume the wrapper into the already admitted move-only party state.
    #[must_use]
    pub fn into_state(self) -> ZkAmsMkheCollectivePartyStateV1 {
        self.state
    }
}

/// Public move-only state machine for the exact ordered eight-party CPK.
///
/// The only transition accepts one [`ZkAmsMkheCpkPartyInputV1`].  There is no
/// batch, iterator, slice, callback, or `Clone` surface capable of retaining
/// eight full shares.  Any operational error poisons the ceremony permanently;
/// a caught backend unwind leaves `failed = true` as well.
pub struct ZkAmsMkheCpkCeremonyV1 {
    inner: Option<ZkAmsMkheStreamingDecryptionAuthorityBuilderV1>,
    failed: bool,
}

impl core::fmt::Debug for ZkAmsMkheCpkCeremonyV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCpkCeremonyV1")
            .field("failed", &self.failed)
            .field("active", &self.inner.is_some())
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheCpkCeremonyV1 {
    /// Allocate the sole common-`a` backing and compact metadata before the
    /// first party secret, share, or evidence object is generated.
    pub fn new(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        cpk_transcript_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Ok(Self {
            inner: Some(ZkAmsMkheStreamingDecryptionAuthorityBuilderV1::new(
                roster,
                cpk_transcript_digest,
            )?),
            failed: false,
        })
    }

    /// Borrow the sole prepared common-`a` owner for exactly the next public
    /// party-state generator.  Keeping a cloned backing makes `finish_v1` fail
    /// closed through the builder's unique-ownership check.
    pub fn prepared_public_a_v1(
        &self,
    ) -> Result<&ZkAmsMkhePreparedCollectivePublicAV1, ZkAmsMkheErrorV1> {
        if self.failed {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.inner
            .as_ref()
            .map(ZkAmsMkheStreamingDecryptionAuthorityBuilderV1::prepared_public_a_v1)
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }

    /// Verify and consume exactly the next party, then return only its small
    /// admitted state successor.
    pub fn verify_and_absorb_next_party_v1<P>(
        &mut self,
        input: ZkAmsMkheCpkPartyInputV1,
        backend: &mut P,
    ) -> Result<ZkAmsMkheAdmittedCpkPartyV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        if self.failed {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        // Poison before destructuring the owned input or calling the backend.
        self.failed = true;
        let result = self.verify_and_absorb_next_party_inner_v1(input, backend);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }

    fn verify_and_absorb_next_party_inner_v1<P>(
        &mut self,
        input: ZkAmsMkheCpkPartyInputV1,
        backend: &mut P,
    ) -> Result<ZkAmsMkheAdmittedCpkPartyV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        let ZkAmsMkheCpkPartyInputV1 {
            mut state,
            share,
            party_b_pointer,
            secret_membership_wire,
            error_membership_wire,
            relation_proof_pointer,
            authentication,
        } = input;
        let builder = self
            .inner
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let party_index = usize::from(state.party_index());
        let party_b_pointer =
            ZkAmsMkheCpkPartyBPointerV1::from_wire_bytes_exact(&party_b_pointer.encode())
                .map_err(map_cpk_relation_error_v1)?;
        let relation_proof_pointer = ZkAmsMkheCpkRelationProofPointerV1::from_wire_bytes_exact(
            &relation_proof_pointer.encode(),
        )
        .map_err(map_cpk_relation_error_v1)?;
        let statement = ZkAmsMkheCpkShareStatementV1::from_governed_roster(
            builder.roster_v1(),
            builder.cpk_transcript_digest_v1(),
            party_index,
            party_b_pointer,
        )
        .map_err(map_cpk_relation_error_v1)?;
        let receipt = verify_zk_ams_mkhe_cpk_relation_v1(
            builder.roster_v1(),
            builder.cpk_transcript_digest_v1(),
            statement,
            secret_membership_wire.as_slice(),
            error_membership_wire.as_slice(),
            relation_proof_pointer,
            authentication,
            backend,
        )
        .map_err(map_cpk_relation_error_v1)?;
        let contribution = VerifiedZkAmsMkheCpkContributionV1::from_verified_relation(receipt);
        // These fixed evidence owners are no longer needed once the sealed
        // verifier receipt exists. Release them before republishing/consuming
        // the P-sized share.
        drop((secret_membership_wire, error_membership_wire));
        builder.absorb_verified_party_v1(contribution, share, &mut state, backend)?;
        Ok(ZkAmsMkheAdmittedCpkPartyV1 { state })
    }

    /// Consume the exact complete ceremony and directly aggregate all staged
    /// party-`b` objects with one `P`-sized accumulator and one 8-KiB buffer.
    pub fn finish_v1<P>(
        mut self,
        backend: &mut P,
    ) -> Result<ZkAmsMkheFinalizedCpkCeremonyV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        if self.failed {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.failed = true;
        let builder = self
            .inner
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let active_roster = *builder.roster_v1();
        let staged = builder.finish_staging_v1()?;
        ZkAmsMkheFinalizedCpkCeremonyV1::from_staged_v1(
            staged.finalize_v1(backend)?,
            active_roster,
            backend,
        )
    }
}

/// Sealed successful CPK products before evaluated-key runtime selection.
///
/// The native `2P` key and both one-shot admissions have already been consumed.
/// Only compact purpose-bound successors remain reachable from this value.
pub struct ZkAmsMkheFinalizedCpkCeremonyV1 {
    evaluated_key_binding: ZkAmsMkheStreamingCollectiveEvalKeyBindingV1,
    trusted_cks_context: ZkAmsMkheTrustedCksContextV1,
    trusted_source_context: ZkAmsMkheTrustedSourceContextV1,
    persistent_context: ZkAmsMkhePersistentDecryptionVerificationContextV1,
    streaming_collective_encryption_key_authority:
        ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    streaming_decryption_authority: Option<ZkAmsMkheStreamingDecryptionAuthorityV1>,
}

impl core::fmt::Debug for ZkAmsMkheFinalizedCpkCeremonyV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheFinalizedCpkCeremonyV1")
            .field(
                "collective_public_key_digest",
                &hex::encode(self.evaluated_key_binding.key_digest()),
            )
            .field(
                "streaming_collective_encryption_key_authority_available",
                &true,
            )
            .field(
                "streaming_decryption_authority_available",
                &self.streaming_decryption_authority.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheFinalizedCpkCeremonyV1 {
    fn from_staged_v1<P>(
        value: ZkAmsMkheFinalizedStagedCpkV1,
        active_roster: ZkAmsMkheGovernedActiveRosterV1,
        backend: &mut P,
    ) -> Result<Self, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        let (
            collective_public_key,
            collective_public_key_admission,
            trusted_cks_context,
            persistent_context,
            streaming_decryption_authority,
        ) = value.into_parts();
        let trusted_source_context = ZkAmsMkheTrustedSourceContextV1::from_staged_verified_digests(
            active_roster,
            &trusted_cks_context,
        )?;
        let key_transcript_digest = collective_public_key.transcript_digest();
        let (streaming_key_admission, evaluated_key_admission) =
            fork_zk_ams_mkhe_staged_collective_key_admission_v1(
                &active_roster,
                key_transcript_digest,
                &collective_public_key,
                collective_public_key_admission,
            )?;
        let streaming_collective_encryption_key_authority =
            mint_zk_ams_mkhe_streaming_collective_encryption_key_authority_v1(
                &active_roster,
                key_transcript_digest,
                &collective_public_key,
                streaming_key_admission,
                backend,
            )?;
        let evaluated_key_binding = bind_zk_ams_mkhe_streaming_collective_eval_key_v1(
            &active_roster,
            key_transcript_digest,
            &collective_public_key,
            evaluated_key_admission,
            &streaming_collective_encryption_key_authority,
        )?;
        drop(collective_public_key);
        Ok(Self {
            evaluated_key_binding,
            trusted_cks_context,
            trusted_source_context,
            persistent_context,
            streaming_collective_encryption_key_authority,
            streaming_decryption_authority: Some(streaming_decryption_authority),
        })
    }

    /// Compact authority for allocation-bounded collective-key switching.
    #[must_use]
    pub const fn trusted_cks_context(&self) -> &ZkAmsMkheTrustedCksContextV1 {
        &self.trusted_cks_context
    }

    /// Compact sealed authority for bounded evaluated-key source verification.
    #[must_use]
    pub const fn trusted_source_context(&self) -> &ZkAmsMkheTrustedSourceContextV1 {
        &self.trusted_source_context
    }

    /// Compact secret-free authority for decryption-share verification.
    #[must_use]
    pub const fn persistent_context(&self) -> &ZkAmsMkhePersistentDecryptionVerificationContextV1 {
        &self.persistent_context
    }

    /// Consume the finalized CPK, discard its compact evaluated-key successor,
    /// and return only the move-only streaming encryption authority.
    #[must_use]
    pub fn into_streaming_collective_encryption_key_authority_v1(
        self,
    ) -> ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1 {
        self.streaming_collective_encryption_key_authority
    }

    /// Consume the one-shot compact authority and bind a streaming decryption
    /// statement without recreating any public-key share. Release admission
    /// requires the provider to expose the CPK and ciphertext objects through
    /// the same immutable snapshot. The authority is poisoned before any
    /// provider-controlled preflight. A failed bind is terminal. It cannot be
    /// retried, and no party-use capability escapes a failed admission.
    pub fn bind_streaming_decryption_statement_v1<'a, P>(
        &'a mut self,
        roster: &'a ZkAmsMkheGovernedRosterWireV1,
        ciphertext: &'a ZkAmsMkheStreamingCollectiveCiphertextV1,
        ciphertext_record_index: u32,
        provider: &mut P,
    ) -> Result<ZkAmsMkheStreamingDecryptionStatementV1<'a>, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        let authority = self
            .streaming_decryption_authority
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        ZkAmsMkheStreamingDecryptionStatementV1::from_verified_cpk_authority_v1(
            roster,
            ciphertext,
            ciphertext_record_index,
            &self.persistent_context,
            authority,
            provider,
        )
    }

    /// Consume the compact evaluated-key binding into the reusable runtime
    /// while preserving only compact CKS/decryption successors.
    pub fn into_evaluated_key_runtime_v1(
        self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        cpk_transcript_digest: [u8; 32],
        manifest: &ZkAmsMkheCollectiveEvaluatedKeyManifestV1,
        expected_manifest_digest: [u8; 32],
    ) -> Result<ZkAmsMkheCpkRuntimeV1, ZkAmsMkheErrorV1> {
        let Self {
            evaluated_key_binding,
            trusted_cks_context,
            trusted_source_context,
            persistent_context,
            streaming_collective_encryption_key_authority,
            streaming_decryption_authority,
        } = self;
        let runtime = ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1::new_from_compact_cpk_v1(
            roster,
            cpk_transcript_digest,
            evaluated_key_binding,
            manifest,
            expected_manifest_digest,
        )?;
        Ok(ZkAmsMkheCpkRuntimeV1 {
            evaluated_key_runtime: runtime,
            trusted_cks_context,
            trusted_source_context,
            persistent_context,
            streaming_collective_encryption_key_authority: Some(
                streaming_collective_encryption_key_authority,
            ),
            streaming_decryption_authority,
        })
    }
}

/// Purpose-bound runtime successors of one consumed staged CPK admission.
pub struct ZkAmsMkheCpkRuntimeV1 {
    evaluated_key_runtime: ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1,
    trusted_cks_context: ZkAmsMkheTrustedCksContextV1,
    trusted_source_context: ZkAmsMkheTrustedSourceContextV1,
    persistent_context: ZkAmsMkhePersistentDecryptionVerificationContextV1,
    streaming_collective_encryption_key_authority:
        Option<ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1>,
    streaming_decryption_authority: Option<ZkAmsMkheStreamingDecryptionAuthorityV1>,
}

impl core::fmt::Debug for ZkAmsMkheCpkRuntimeV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCpkRuntimeV1")
            .field(
                "collective_key_digest",
                &hex::encode(self.evaluated_key_runtime.collective_key_digest()),
            )
            .field(
                "streaming_collective_encryption_key_authority_available",
                &self.streaming_collective_encryption_key_authority.is_some(),
            )
            .field(
                "streaming_decryption_authority_available",
                &self.streaming_decryption_authority.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheCpkRuntimeV1 {
    /// Reusable evaluated-key runtime admitted by the consumed staged key.
    #[must_use]
    pub const fn evaluated_key_runtime(&self) -> &ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1 {
        &self.evaluated_key_runtime
    }

    /// Compact authority for allocation-bounded collective-key switching.
    #[must_use]
    pub const fn trusted_cks_context(&self) -> &ZkAmsMkheTrustedCksContextV1 {
        &self.trusted_cks_context
    }

    /// Compact sealed authority for bounded evaluated-key source verification.
    #[must_use]
    pub const fn trusted_source_context(&self) -> &ZkAmsMkheTrustedSourceContextV1 {
        &self.trusted_source_context
    }

    /// Compact secret-free authority for decryption-share verification.
    #[must_use]
    pub const fn persistent_context(&self) -> &ZkAmsMkhePersistentDecryptionVerificationContextV1 {
        &self.persistent_context
    }

    /// Take the sole move-only streaming encryption authority. The evaluated
    /// runtime remains usable and retains no second encryption capability.
    pub fn take_streaming_collective_encryption_key_authority_v1(
        &mut self,
    ) -> Result<ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1, ZkAmsMkheErrorV1> {
        self.streaming_collective_encryption_key_authority
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }

    /// Consume the remaining one-shot authority into a compact streaming
    /// decryption statement. Release admission requires the provider to expose
    /// the CPK and ciphertext objects through the same immutable snapshot. The
    /// authority is poisoned before any provider-controlled preflight. A failed
    /// bind is terminal. It cannot be retried, and no party-use capability
    /// escapes a failed admission.
    pub fn bind_streaming_decryption_statement_v1<'a, P>(
        &'a mut self,
        roster: &'a ZkAmsMkheGovernedRosterWireV1,
        ciphertext: &'a ZkAmsMkheStreamingCollectiveCiphertextV1,
        ciphertext_record_index: u32,
        provider: &mut P,
    ) -> Result<ZkAmsMkheStreamingDecryptionStatementV1<'a>, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        let authority = self
            .streaming_decryption_authority
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        ZkAmsMkheStreamingDecryptionStatementV1::from_verified_cpk_authority_v1(
            roster,
            ciphertext,
            ciphertext_record_index,
            &self.persistent_context,
            authority,
            provider,
        )
    }
}

fn map_cpk_relation_error_v1(error: ZkAmsMkheCpkRelationErrorV1) -> ZkAmsMkheErrorV1 {
    match error {
        ZkAmsMkheCpkRelationErrorV1::ResourceCeiling => ZkAmsMkheErrorV1::ResourceCeilingExceeded,
        ZkAmsMkheCpkRelationErrorV1::RandomUnavailable => ZkAmsMkheErrorV1::RandomUnavailable,
        ZkAmsMkheCpkRelationErrorV1::Authentication => ZkAmsMkheErrorV1::InvalidAuthentication,
        ZkAmsMkheCpkRelationErrorV1::ObjectPointer
        | ZkAmsMkheCpkRelationErrorV1::ShareStatement
        | ZkAmsMkheCpkRelationErrorV1::RelationHeader
        | ZkAmsMkheCpkRelationErrorV1::RelationBody
        | ZkAmsMkheCpkRelationErrorV1::RnsFirstMessage => ZkAmsMkheErrorV1::InvalidWireEncoding,
        _ => ZkAmsMkheErrorV1::InvalidKeyMaterial,
    }
}

#[cfg(test)]
mod tests {
    use super::ZkAmsMkheCollectivePublicKeyShareV1;

    fn production_source_v1() -> &'static str {
        include_str!("cpk_ceremony.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("production CPK ceremony source")
    }

    #[test]
    fn move_only_ceremony_is_reachable_through_the_public_vega_facade() {
        type Begin =
            fn(
                &crate::vega::ZkAmsMkheGovernedActiveRosterV1,
                [u8; 32],
            )
                -> Result<crate::vega::ZkAmsMkheCpkCeremonyV1, crate::vega::ZkAmsMkheErrorV1>;
        let _: Begin = crate::vega::ZkAmsMkheCpkCeremonyV1::new;

        type PartyInput =
            fn(
                crate::vega::ZkAmsMkheCollectivePartyStateV1,
                crate::vega::ZkAmsMkheCollectivePublicKeyShareV1,
                crate::vega::ZkAmsMkheDirectObjectPointerV1,
                Box<[u8; crate::vega::ZK_AMS_MKHE_CPK_SECRET_MEMBERSHIP_WIRE_BYTES_V1]>,
                Box<[u8; crate::vega::ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_WIRE_BYTES_V1]>,
                crate::vega::ZkAmsMkheDirectObjectPointerV1,
                crate::vega::ZkAmsMkheAuthenticationWireV1,
            )
                -> Result<crate::vega::ZkAmsMkheCpkPartyInputV1, crate::vega::ZkAmsMkheErrorV1>;
        let _: PartyInput = crate::vega::ZkAmsMkheCpkPartyInputV1::new;

        let source = production_source_v1();
        let transition = source
            .split("pub fn verify_and_absorb_next_party_v1")
            .nth(1)
            .expect("public one-party transition")
            .split("fn verify_and_absorb_next_party_inner_v1")
            .next()
            .expect("public transition boundary");
        assert!(transition.contains("input: ZkAmsMkheCpkPartyInputV1"));
        assert!(transition.contains("self.failed = true"));
        assert!(transition.contains("verify_and_absorb_next_party_inner_v1(input, backend)"));
        for retaining_shape in ["Vec<", "IntoIterator", "impl Iterator", "FnMut", "FnOnce"] {
            assert!(
                !transition.contains(retaining_shape),
                "the public transition must not accept {retaining_shape}",
            );
        }

        let finish = source
            .split("pub fn finish_v1")
            .nth(1)
            .expect("consuming finish")
            .split("/// Sealed successful CPK products")
            .next()
            .expect("finish boundary");
        assert!(finish.contains("mut self,"));
        assert!(!finish.contains("&mut self"));
    }

    #[test]
    fn party_input_binds_the_share_epoch_and_transcript() {
        let _: fn(&ZkAmsMkheCollectivePublicKeyShareV1) -> u64 =
            ZkAmsMkheCollectivePublicKeyShareV1::epoch;
        let _: fn(&ZkAmsMkheCollectivePublicKeyShareV1) -> [u8; 32] =
            ZkAmsMkheCollectivePublicKeyShareV1::transcript_digest;

        let collective_source = include_str!("collective.rs");
        let share_impl = collective_source
            .split("impl ZkAmsMkheCollectivePublicKeyShareV1")
            .nth(1)
            .expect("collective-public-key share implementation")
            .split("/// Verified aggregate")
            .next()
            .expect("collective-public-key share accessors");
        let epoch_getter = share_impl
            .split("pub const fn epoch")
            .nth(1)
            .expect("share epoch getter")
            .split('}')
            .next()
            .expect("share epoch getter body");
        let transcript_getter = share_impl
            .split("pub const fn transcript_digest")
            .nth(1)
            .expect("share transcript getter")
            .split('}')
            .next()
            .expect("share transcript getter body");
        assert!(epoch_getter.contains("self.epoch"));
        assert!(transcript_getter.contains("self.transcript_digest"));

        let ceremony_source = production_source_v1();
        let constructor = ceremony_source
            .split("pub fn new(")
            .nth(1)
            .expect("CPK party-input constructor")
            .split("/// Exact governed roster position")
            .next()
            .expect("CPK party-input constructor body");
        assert!(constructor.contains("state.epoch() != share.epoch()"));
        assert!(constructor.contains("state.transcript_digest() != share.transcript_digest()"));
    }

    #[test]
    fn streaming_decryption_bind_failure_is_terminal_without_partial_capability_escape() {
        let source = production_source_v1();
        assert_eq!(
            source.matches("bind is terminal.").count(),
            2,
            "both public CPK owners document terminal admission failure"
        );

        let mut remaining = source;
        for _ in 0..2 {
            let start = remaining
                .find("pub fn bind_streaming_decryption_statement_v1")
                .expect("streaming decryption binder");
            let binder = remaining[start..]
                .split("\n    }\n")
                .next()
                .expect("streaming decryption binder body");
            let poison = binder
                .find(".streaming_decryption_authority\n            .take()")
                .expect("one-shot authority poison");
            let preflight = binder
                .find("ZkAmsMkheStreamingDecryptionStatementV1::from_verified_cpk_authority_v1(")
                .expect("fallible provider preflight");
            assert!(poison < preflight);
            assert!(!binder.contains("streaming_decryption_authority = Some"));
            assert!(binder.contains("authority,\n            provider,"));
            remaining = &remaining[start + binder.len()..];
        }
    }
}
