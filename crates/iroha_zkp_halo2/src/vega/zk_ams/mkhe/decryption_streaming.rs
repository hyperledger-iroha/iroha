//! Allocation-bounded verification of the split decryption transport.
//!
//! This module is deliberately a child of `decryption`: it reuses the exact
//! V1 transcript, response bounds, CRT decoder, and abort taxonomy instead of
//! defining a second relation.  The native implementation remains the small
//! profile/reference path. Release resource evidence may only refer to the
//! streaming entry point below, and remains fail-closed until a staged prover
//! and an authenticated peak-residency run are installed.

use core::{fmt, mem::size_of};

use super::super::{
    ArtifactAuthentication, BgvProfile, MKHE_VERSION_V1, PartySet, RnsPolynomial, ZkAmsMkheErrorV1,
    ZkAmsMkhePartyIdV1,
    active::zk_ams_mkhe_active_rkg_linear_proof_security_v1,
    checked_ring_multiplication_work,
    collective::cpk_party_b_payload_blake3_v1,
    cpk_relation::{
        ZK_AMS_MKHE_CPK_PARTY_B_OBJECT_BYTES_V1, derive_active_collective_public_a_limb_v1,
    },
    direct_object_transport::{
        ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1, ZkAmsMkheDirectObjectKindV1,
        ZkAmsMkheDirectObjectPointerV1, ZkAmsMkheDirectObjectReadAtProviderV1,
        ZkAmsMkheDirectObjectReadReceiptV1, ZkAmsMkheDirectObjectReadTransactionV1,
    },
    manifest::{
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1, zk_ams_mkhe_noise_certificate_v1,
    },
    mod_add, mod_mul, mod_sub, negacyclic_multiply,
    persistent_decryption_equality::{
        ZkAmsMkhePersistentDecryptionVerificationContextV1, ZkAmsMkheStreamingDecryptionAuthorityV1,
    },
    signed_mod,
    wire::{ZkAmsMkheCollectiveCiphertextWireV1, ZkAmsMkheGovernedRosterWireV1},
};
use super::{
    DECRYPTION_CHALLENGE_DOMAIN_V1, DECRYPTION_PROOF_DOMAIN_V1, DECRYPTION_PROOF_HEADER_BYTES_V1,
    DECRYPTION_SET_DOMAIN_V1, DECRYPTION_SIGNED_SMALL_BYTES_V1, DecryptionAbortReasonV1,
    DecryptionBindingV1, SignedWideV1, ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1,
    ZkAmsMkheDecryptionStatementV1, ZkAmsMkheDecryptionTransportComponentKindV1,
    ZkAmsMkheDecryptionTransportManifestV1, ZkAmsMkheDecryptionTransportPointerV1,
    ZkAmsMkheFullRosterDecryptionResultV1, ZkAmsMkheIdentifiableDecryptionAbortV1,
    decode_centered_plaintext, decryption_binding_from_compact_axes_v1,
    decryption_binding_from_statement, decryption_wire_ciphertext_digest_v1,
    derive_decryption_resource_evidence, derive_sparse_challenge, identifiable_abort, read_array,
    read_decryption_transport_pointer, read_u8, read_u16, read_u32, read_u64, update_wire_rns_hash,
    wide_relation_challenge_weight, wide_response_parameters,
};
use crate::vega::sponge::Keccak256;

const STREAMING_RESOURCE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.decryption-streaming-resource";
const DECRYPTION_PROOF_TAG_V1: [u8; 4] = *b"ZADP";
const DECRYPTION_SPLIT_MANIFEST_TAG_V1: [u8; 4] = *b"ZDSM";
const DECRYPTION_SPLIT_COMPONENT_COUNT_V1: u8 = 2;

/// Digest pinned only by an authenticated run of this exact streaming topology.
///
/// This is deliberately distinct from the native-residency certificate. It
/// remains zero until the bounded authority constructor and staged prover exist
/// and the complete release worker has been measured.
pub const ZK_AMS_MKHE_DECRYPTION_STREAMING_RESIDENCY_CERTIFICATE_DIGEST_V1: [u8; 32] = [0; 32];

// TODO: write share/proof bytes directly into staged objects before installing
// any streaming release certificate. The compact authority path below already
// retains exact snapshot-bound complete-CPK `b_i` pointers without a native
// decryption statement.

/// Missing implementation boundary that keeps streaming decryption fail-closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheDecryptionStreamingBlockerV1 {
    /// Canonical empty blocker slot. Only slots at or above `blocker_count`
    /// may contain this value.
    NoBlocker = 0,
    /// The prover still returns a retained native share instead of writing both
    /// canonical components directly into bounded staged objects.
    StagedProverOutputMissing = 1,
    /// Historical compatibility value retained for downstream exhaustive
    /// matches. It is no longer an active implementation blocker.
    CompactAuthorityConstructionStillNative = 2,
}

/// Phase-specific source accounting for the bounded verifier topology.
///
/// These are exact enumerated large-buffer payloads, not a peak-RSS claim.
/// Allocator metadata, stacks, and the surrounding worker must still be covered
/// by the zero-pinned authenticated residency certificate. Compact-authority
/// figures count algorithm-owned buffers and the currently borrowed share, but
/// deliberately do not count storage, caching, or duplicate immutable-object
/// copies inside a caller-selected CAS provider. An in-process implementation
/// that retains full stages/seals/publication caches can therefore exceed the
/// governed worker ceiling even though this source topology fits it; release
/// requires a separately bounded/external provider and the nonzero runtime
/// residency certificate which is presently absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDecryptionStreamingResidencyEvidenceV1 {
    /// Exact payload bytes in one full native RNS polynomial.
    pub native_rns_polynomial_bytes: u64,
    /// Exact bytes in one release RNS limb.
    pub rns_limb_bytes: u64,
    /// Two retained ciphertext polynomial payloads.
    pub ciphertext_input_bytes: u64,
    /// One in-place full-RNS aggregate.
    pub aggregate_bytes: u64,
    /// One exact canonical `ZADP` proof buffer.
    pub proof_view_backing_bytes: u64,
    /// Eight exact signed `ZDSM` manifests retained by the caller.
    pub manifest_preflight_bytes: u64,
    /// Fixed direct-object read buffer.
    pub direct_read_buffer_bytes: u64,
    /// Sparse challenge vector bytes.
    pub sparse_challenge_bytes: u64,
    /// Enumerated payload peak while authenticating manifests.
    pub manifest_preflight_peak_bytes: u64,
    /// Enumerated payload peak while loading one authenticated proof.
    pub proof_load_peak_bytes: u64,
    /// Enumerated payload peak while hashing one streamed public polynomial.
    pub public_input_hash_peak_bytes: u64,
    /// Enumerated payload peak while reconstructing `U_pk`.
    pub public_key_commitment_peak_bytes: u64,
    /// Enumerated payload peak while reconstructing `U_share`.
    pub share_commitment_peak_bytes: u64,
    /// Enumerated payload peak while CRT decoding the aggregate.
    pub crt_decode_peak_bytes: u64,
    /// Largest of the enumerated verifier phases.
    pub enumerated_verifier_peak_bytes: u64,
    /// Governed 160 MiB native workspace ceiling.
    pub governed_workspace_ceiling_bytes: u64,
    /// Maximum simultaneous complete RNS polynomial payloads.
    pub maximum_full_rns_polynomials: u8,
    /// Maximum simultaneous one-limb arithmetic buffers.
    pub maximum_rns_limb_buffers: u8,
    /// Complete authenticated passes over each party `b_i` object.
    pub party_b_passes: u8,
    /// Complete authenticated passes over each decryption-share object.
    pub decryption_share_passes: u8,
    /// Lower bound of the explicitly excluded native reference combine path.
    pub native_reference_lower_bound_bytes: u64,
    /// Lower bound inherited by the current compact-authority bridge before
    /// the returned compact statement exists. The legacy field name is kept;
    /// it now records the exact enumerated large-buffer peak of the bounded
    /// authority constructor.
    pub compact_authority_construction_lower_bound_bytes: u64,
    /// One retained aggregate `b` construction buffer.
    pub compact_authority_aggregate_bytes: u64,
    /// The borrowed public share's common `a` and party `b` payloads.
    pub compact_authority_absorbed_share_rns_bytes: u64,
    /// Exact two-witness active collective-key proof payload in that share.
    pub compact_authority_share_proof_bytes: u64,
    /// One derived common-`a` limb during compact share validation.
    pub compact_authority_limb_scratch_bytes: u64,
    /// Largest enumerated bounded-construction phase.
    pub compact_authority_enumerated_peak_bytes: u64,
    /// Whether caller-selected CAS backend storage/cache is included in that
    /// number. This remains false; only a runtime certificate can cover it.
    pub compact_authority_cas_backend_residency_enumerated: bool,
    /// The bounded-construction topology fits the governed workspace ceiling.
    pub compact_authority_enumerated_ceiling_met: bool,
    /// Canonical source emissions into immutable staging per party `b_i`.
    pub compact_authority_party_b_source_passes: u8,
    /// Complete sealed and post-publication readbacks per party `b_i`.
    pub compact_authority_publication_readback_passes: u8,
    /// Complete published-object rereads for key-context and set digests.
    pub compact_authority_context_digest_read_passes: u8,
    /// The enumerated verifier payload fits the static workspace ceiling.
    pub enumerated_verifier_ceiling_met: bool,
    /// Whether bounded staged prover output exists in this implementation.
    pub staged_prover_output_implemented: bool,
    /// Whether the source topology can mint compact authority without the native
    /// nine-polynomial statement. This is not an RSS claim for arbitrary CAS
    /// trait implementations; the runtime certificate remains decisive.
    pub bounded_compact_authority_construction_implemented: bool,
    /// Number of active entries in `implementation_blockers`.
    pub implementation_blocker_count: u8,
    /// Fixed canonical slots for independently actionable blockers.
    pub implementation_blockers: [ZkAmsMkheDecryptionStreamingBlockerV1; 2],
    /// Authenticated runtime peak certificate; zero remains absent.
    pub authenticated_peak_residency_digest: [u8; 32],
    /// Always false until the staged prover and authenticated peak run exist.
    pub release_certified: bool,
    /// Digest binding every field above.
    pub evidence_digest: [u8; 32],
}

impl ZkAmsMkheDecryptionStreamingResidencyEvidenceV1 {
    /// Recompute every byte and fail closed on any forged readiness field.
    pub fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        let expected = derive_streaming_residency_evidence_v1()?;
        if self != expected
            || self.evidence_digest == [0; 32]
            || !self.enumerated_verifier_ceiling_met
            || self.compact_authority_cas_backend_residency_enumerated
            || !self.compact_authority_enumerated_ceiling_met
            || self.staged_prover_output_implemented
            || !self.bounded_compact_authority_construction_implemented
            || self.implementation_blocker_count != 1
            || self.implementation_blockers
                != [
                    ZkAmsMkheDecryptionStreamingBlockerV1::StagedProverOutputMissing,
                    ZkAmsMkheDecryptionStreamingBlockerV1::NoBlocker,
                ]
            || self.authenticated_peak_residency_digest != [0; 32]
            || self.release_certified
        {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        Ok(())
    }
}

/// Return source-derived bounded-verifier accounting without opening a release gate.
pub fn zk_ams_mkhe_decryption_streaming_residency_evidence_v1()
-> Result<ZkAmsMkheDecryptionStreamingResidencyEvidenceV1, ZkAmsMkheErrorV1> {
    let evidence = derive_streaming_residency_evidence_v1()?;
    evidence.validate()?;
    Ok(evidence)
}

fn derive_streaming_residency_evidence_v1()
-> Result<ZkAmsMkheDecryptionStreamingResidencyEvidenceV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile.validate()?;
    let legacy = derive_decryption_resource_evidence(&profile)?;
    let degree = u64::try_from(profile.ring_degree)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let limbs = u64::try_from(profile.moduli.len())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let rns_limb_bytes = degree
        .checked_mul(size_of::<u64>() as u64)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let native_rns_polynomial_bytes = rns_limb_bytes
        .checked_mul(limbs)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let ciphertext_input_bytes = native_rns_polynomial_bytes
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let aggregate_bytes = native_rns_polynomial_bytes;
    let proof_view_backing_bytes = legacy.proof_payload_bytes;
    let manifest_preflight_bytes = u64::try_from(
        ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1
            .checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let direct_read_buffer_bytes = ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 as u64;
    let sparse_challenge_bytes = degree;
    let common_retained = ciphertext_input_bytes
        .checked_add(aggregate_bytes)
        .and_then(|value| value.checked_add(proof_view_backing_bytes))
        .and_then(|value| value.checked_add(manifest_preflight_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let manifest_preflight_peak_bytes = ciphertext_input_bytes
        .checked_add(manifest_preflight_bytes)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let proof_load_peak_bytes = common_retained
        .checked_add(direct_read_buffer_bytes)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let public_input_hash_peak_bytes = common_retained
        .checked_add(rns_limb_bytes)
        .and_then(|value| value.checked_add(direct_read_buffer_bytes))
        .and_then(|value| value.checked_add(sparse_challenge_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let public_key_commitment_peak_bytes = common_retained
        .checked_add(
            rns_limb_bytes
                .checked_mul(4)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .and_then(|value| value.checked_add(direct_read_buffer_bytes))
        .and_then(|value| value.checked_add(sparse_challenge_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let share_commitment_peak_bytes = common_retained
        .checked_add(
            rns_limb_bytes
                .checked_mul(3)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .and_then(|value| value.checked_add(direct_read_buffer_bytes))
        .and_then(|value| value.checked_add(sparse_challenge_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let crt_decode_peak_bytes = ciphertext_input_bytes
        .checked_add(aggregate_bytes)
        .and_then(|value| value.checked_add(degree.checked_mul(32)?))
        .and_then(|value| value.checked_add(manifest_preflight_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let enumerated_verifier_peak_bytes = *[
        manifest_preflight_peak_bytes,
        proof_load_peak_bytes,
        public_input_hash_peak_bytes,
        public_key_commitment_peak_bytes,
        share_commitment_peak_bytes,
        crt_decode_peak_bytes,
    ]
    .iter()
    .max()
    .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let governed_workspace_ceiling_bytes = profile.max_workspace_bytes as u64;
    let enumerated_verifier_ceiling_met =
        enumerated_verifier_peak_bytes <= governed_workspace_ceiling_bytes;
    let compact_authority_aggregate_bytes = native_rns_polynomial_bytes;
    let compact_authority_absorbed_share_rns_bytes = native_rns_polynomial_bytes
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let active_proof_security = zk_ams_mkhe_active_rkg_linear_proof_security_v1()?;
    if u64::from(active_proof_security.ring_degree) != degree
        || active_proof_security.max_witness_polynomials < 2
        || active_proof_security.signed_coefficient_bytes == 0
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let maximum_active_response_bytes = degree
        .checked_mul(u64::from(active_proof_security.max_witness_polynomials))
        .and_then(|value| {
            value.checked_mul(u64::from(active_proof_security.signed_coefficient_bytes))
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let active_proof_header_bytes = u64::from(active_proof_security.max_proof_bytes)
        .checked_sub(maximum_active_response_bytes)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    // A collective-key share has exactly the secret and public-error response
    // polynomials. Derive its exact retained proof payload from the governed
    // active-proof certificate rather than charging the unrelated eight-witness
    // generic decoder ceiling.
    let compact_authority_share_proof_bytes = degree
        .checked_mul(2)
        .and_then(|value| {
            value.checked_mul(u64::from(active_proof_security.signed_coefficient_bytes))
        })
        .and_then(|value| value.checked_add(active_proof_header_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let compact_authority_limb_scratch_bytes = rns_limb_bytes;
    let compact_authority_validation_peak_bytes = compact_authority_aggregate_bytes
        .checked_add(compact_authority_absorbed_share_rns_bytes)
        .and_then(|value| value.checked_add(compact_authority_share_proof_bytes))
        .and_then(|value| value.checked_add(compact_authority_limb_scratch_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let compact_authority_publication_peak_bytes = compact_authority_aggregate_bytes
        .checked_add(compact_authority_absorbed_share_rns_bytes)
        .and_then(|value| value.checked_add(compact_authority_share_proof_bytes))
        .and_then(|value| value.checked_add(direct_read_buffer_bytes.checked_mul(2)?))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let compact_authority_enumerated_peak_bytes =
        compact_authority_validation_peak_bytes.max(compact_authority_publication_peak_bytes);
    let compact_authority_enumerated_ceiling_met =
        compact_authority_enumerated_peak_bytes <= governed_workspace_ceiling_bytes;
    let staged_prover_output_implemented = false;
    let bounded_compact_authority_construction_implemented = true;
    let authenticated_peak_residency_digest =
        ZK_AMS_MKHE_DECRYPTION_STREAMING_RESIDENCY_CERTIFICATE_DIGEST_V1;
    let release_certified = enumerated_verifier_ceiling_met
        && staged_prover_output_implemented
        && bounded_compact_authority_construction_implemented
        && authenticated_peak_residency_digest != [0; 32];
    let mut evidence = ZkAmsMkheDecryptionStreamingResidencyEvidenceV1 {
        native_rns_polynomial_bytes,
        rns_limb_bytes,
        ciphertext_input_bytes,
        aggregate_bytes,
        proof_view_backing_bytes,
        manifest_preflight_bytes,
        direct_read_buffer_bytes,
        sparse_challenge_bytes,
        manifest_preflight_peak_bytes,
        proof_load_peak_bytes,
        public_input_hash_peak_bytes,
        public_key_commitment_peak_bytes,
        share_commitment_peak_bytes,
        crt_decode_peak_bytes,
        enumerated_verifier_peak_bytes,
        governed_workspace_ceiling_bytes,
        maximum_full_rns_polynomials: 3,
        maximum_rns_limb_buffers: 4,
        party_b_passes: 2,
        decryption_share_passes: 2,
        native_reference_lower_bound_bytes: legacy
            .native_combine_relation_residency_lower_bound_bytes,
        compact_authority_construction_lower_bound_bytes: compact_authority_enumerated_peak_bytes,
        compact_authority_aggregate_bytes,
        compact_authority_absorbed_share_rns_bytes,
        compact_authority_share_proof_bytes,
        compact_authority_limb_scratch_bytes,
        compact_authority_enumerated_peak_bytes,
        compact_authority_cas_backend_residency_enumerated: false,
        compact_authority_enumerated_ceiling_met,
        compact_authority_party_b_source_passes: 1,
        compact_authority_publication_readback_passes: 2,
        compact_authority_context_digest_read_passes: 2,
        enumerated_verifier_ceiling_met,
        staged_prover_output_implemented,
        bounded_compact_authority_construction_implemented,
        implementation_blocker_count: 1,
        implementation_blockers: [
            ZkAmsMkheDecryptionStreamingBlockerV1::StagedProverOutputMissing,
            ZkAmsMkheDecryptionStreamingBlockerV1::NoBlocker,
        ],
        authenticated_peak_residency_digest,
        release_certified,
        evidence_digest: [0; 32],
    };
    evidence.evidence_digest = streaming_residency_evidence_digest(evidence);
    Ok(evidence)
}

fn streaming_residency_evidence_digest(
    evidence: ZkAmsMkheDecryptionStreamingResidencyEvidenceV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(STREAMING_RESOURCE_DOMAIN_V1);
    for value in [
        evidence.native_rns_polynomial_bytes,
        evidence.rns_limb_bytes,
        evidence.ciphertext_input_bytes,
        evidence.aggregate_bytes,
        evidence.proof_view_backing_bytes,
        evidence.manifest_preflight_bytes,
        evidence.direct_read_buffer_bytes,
        evidence.sparse_challenge_bytes,
        evidence.manifest_preflight_peak_bytes,
        evidence.proof_load_peak_bytes,
        evidence.public_input_hash_peak_bytes,
        evidence.public_key_commitment_peak_bytes,
        evidence.share_commitment_peak_bytes,
        evidence.crt_decode_peak_bytes,
        evidence.enumerated_verifier_peak_bytes,
        evidence.governed_workspace_ceiling_bytes,
        evidence.native_reference_lower_bound_bytes,
        evidence.compact_authority_construction_lower_bound_bytes,
        evidence.compact_authority_aggregate_bytes,
        evidence.compact_authority_absorbed_share_rns_bytes,
        evidence.compact_authority_share_proof_bytes,
        evidence.compact_authority_limb_scratch_bytes,
        evidence.compact_authority_enumerated_peak_bytes,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&[
        evidence.maximum_full_rns_polynomials,
        evidence.maximum_rns_limb_buffers,
        evidence.party_b_passes,
        evidence.decryption_share_passes,
        evidence
            .compact_authority_cas_backend_residency_enumerated
            .into(),
        evidence.compact_authority_enumerated_ceiling_met.into(),
        evidence.compact_authority_party_b_source_passes,
        evidence.compact_authority_publication_readback_passes,
        evidence.compact_authority_context_digest_read_passes,
        evidence.enumerated_verifier_ceiling_met.into(),
        evidence.staged_prover_output_implemented.into(),
        evidence
            .bounded_compact_authority_construction_implemented
            .into(),
        evidence.implementation_blocker_count,
        evidence.implementation_blockers[0] as u8,
        evidence.implementation_blockers[1] as u8,
    ]);
    hash.update(&evidence.authenticated_peak_residency_digest);
    hash.update(&[evidence.release_certified.into()]);
    hash.finalize()
}

/// Exact provider session and immutable snapshot used by one full-roster read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDecryptionStreamingSnapshotV1 {
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
}

impl ZkAmsMkheDecryptionStreamingSnapshotV1 {
    /// Identity of the exact open provider session.
    #[must_use]
    pub const fn provider_identity(self) -> [u8; 32] {
        self.provider_identity
    }

    /// Identity of the immutable object revision used for every pass.
    #[must_use]
    pub const fn snapshot_identity(self) -> [u8; 32] {
        self.snapshot_identity
    }
}

/// Decryption result accompanied by its exact immutable provider snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheStreamingFullRosterDecryptionResultV1 {
    result: ZkAmsMkheFullRosterDecryptionResultV1,
    snapshot: ZkAmsMkheDecryptionStreamingSnapshotV1,
}

impl ZkAmsMkheStreamingFullRosterDecryptionResultV1 {
    /// Verified plaintext, residual bound, and ordered-share-set digest.
    #[must_use]
    pub const fn result(&self) -> &ZkAmsMkheFullRosterDecryptionResultV1 {
        &self.result
    }

    /// Exact immutable provider revision shared by every object pass.
    #[must_use]
    pub const fn snapshot(&self) -> ZkAmsMkheDecryptionStreamingSnapshotV1 {
        self.snapshot
    }

    /// Consume the snapshot wrapper and return the existing result type.
    #[must_use]
    pub fn into_result(self) -> ZkAmsMkheFullRosterDecryptionResultV1 {
        self.result
    }
}

/// Compact, context-minted release statement for the bounded verifier.
///
/// The value retains only roster, ciphertext, compact bindings, and content
/// addresses. In particular, it does not retain the aggregate key or any of the
/// eight full public-key shares. The preferred constructor consumes the
/// move-only bounded CPK ceremony authority. The explicitly named native
/// reference bridge remains available only as a compatibility/reference path
/// and is not included in verifier residency evidence. The persistent context
/// remains borrowed for the statement's lifetime; there is no raw-digest,
/// raw-pointer, codec, or decoder constructor.
pub struct ZkAmsMkheStreamingDecryptionStatementV1<'a> {
    roster: &'a ZkAmsMkheGovernedRosterWireV1,
    ciphertext: &'a ZkAmsMkheCollectiveCiphertextWireV1,
    persistent_context: &'a ZkAmsMkhePersistentDecryptionVerificationContextV1,
    parties: PartySet,
    party_bindings: [DecryptionBindingV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    party_b_pointers: [ZkAmsMkheDirectObjectPointerV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ciphertext_digest: [u8; 32],
    key_context_digest: [u8; 32],
}

impl fmt::Debug for ZkAmsMkheStreamingDecryptionStatementV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheStreamingDecryptionStatementV1")
            .field("roster_digest", &hex::encode(self.roster.roster_digest()))
            .field("ciphertext_digest", &hex::encode(self.ciphertext_digest))
            .field("key_context_digest", &hex::encode(self.key_context_digest))
            .finish_non_exhaustive()
    }
}

impl<'a> ZkAmsMkheStreamingDecryptionStatementV1<'a> {
    /// Mint a compact statement from the exact bounded CPK ceremony authority.
    ///
    /// The one-shot authority is consumed here. Every pointer and key-context
    /// axis comes from the retained persistent context; callers supply only the
    /// canonical roster/ciphertext objects to which fresh party bindings are
    /// minted.
    pub fn from_verified_cpk_authority_v1(
        roster: &'a ZkAmsMkheGovernedRosterWireV1,
        ciphertext: &'a ZkAmsMkheCollectiveCiphertextWireV1,
        persistent_context: &'a ZkAmsMkhePersistentDecryptionVerificationContextV1,
        authority: ZkAmsMkheStreamingDecryptionAuthorityV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let parties = PartySet::new(roster.parties().to_vec())?;
        let material =
            persistent_context.consume_streaming_authority_v1(roster, ciphertext, authority)?;
        let (party_b_pointers, proof_bindings, ciphertext_digest, key_context_digest) =
            material.into_parts();
        let mut party_bindings = Vec::new();
        party_bindings
            .try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for (party_index, persistent) in proof_bindings.into_iter().enumerate() {
            let binding = decryption_binding_from_compact_axes_v1(
                roster,
                ciphertext,
                ciphertext_digest,
                key_context_digest,
                party_index,
                &persistent,
            )?;
            binding.validate(&profile, &parties)?;
            party_bindings.push(binding);
        }
        let value = Self {
            roster,
            ciphertext,
            persistent_context,
            parties,
            party_bindings: party_bindings
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
            party_b_pointers,
            ciphertext_digest,
            key_context_digest,
        };
        value.validate_compact()?;
        Ok(value)
    }

    /// Mint a compact statement through the over-budget native reference authority.
    ///
    /// `verified_source` is used only during this call. Its aggregate key and
    /// eight public-key shares are not tied to `'a` and may be dropped as soon
    /// as construction returns. This compatibility bridge is excluded from
    /// release evidence; bounded routing uses
    /// [`Self::from_verified_cpk_authority_v1`].
    pub fn from_native_reference_v1(
        roster: &'a ZkAmsMkheGovernedRosterWireV1,
        ciphertext: &'a ZkAmsMkheCollectiveCiphertextWireV1,
        persistent_context: &'a ZkAmsMkhePersistentDecryptionVerificationContextV1,
        verified_source: ZkAmsMkheDecryptionStatementV1<'_>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        verified_source.validate()?;
        if verified_source.roster() != roster || verified_source.ciphertext() != ciphertext {
            return Err(ZkAmsMkheErrorV1::InvalidShareSet);
        }
        let profile = release_profile_v1();
        let parties = PartySet::new(roster.parties().to_vec())?;
        let ciphertext_digest = decryption_wire_ciphertext_digest_v1(&profile, roster, ciphertext)?;
        let key_context_digest = verified_source.key_context_digest();
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut party_b_pointers = Vec::new();
        party_b_pointers
            .try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            let persistent = persistent_context.proof_binding(verified_source, party_index)?;
            let binding =
                decryption_binding_from_statement(verified_source, party_index, &persistent)?;
            if binding.ciphertext_digest != ciphertext_digest
                || binding.key_context_digest != key_context_digest
            {
                return Err(ZkAmsMkheErrorV1::InvalidShareSet);
            }
            let party_b = verified_source
                .party_public_b(party_index)
                .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
            party_b_pointers.push(ZkAmsMkheDirectObjectPointerV1::new(
                ZkAmsMkheDirectObjectKindV1::CpkPartyB,
                ZK_AMS_MKHE_CPK_PARTY_B_OBJECT_BYTES_V1 as u64,
                cpk_party_b_payload_blake3_v1(party_b)?,
            )?);
            bindings.push(binding);
        }
        let party_bindings = bindings
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
        let party_b_pointers = party_b_pointers
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?;
        let value = Self {
            roster,
            ciphertext,
            persistent_context,
            parties,
            party_bindings,
            party_b_pointers,
            ciphertext_digest,
            key_context_digest,
        };
        value.validate_compact()?;
        Ok(value)
    }

    /// Exact governed roster retained by the compact statement.
    #[must_use]
    pub const fn roster(&self) -> &'a ZkAmsMkheGovernedRosterWireV1 {
        self.roster
    }

    /// Exact two-polynomial ciphertext retained by the compact statement.
    #[must_use]
    pub const fn ciphertext(&self) -> &'a ZkAmsMkheCollectiveCiphertextWireV1 {
        self.ciphertext
    }

    /// Native ciphertext digest bound into all eight signed manifests.
    #[must_use]
    pub const fn ciphertext_digest(&self) -> [u8; 32] {
        self.ciphertext_digest
    }

    fn validate_compact(&self) -> Result<(), ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        if self.parties.parties.as_slice() != self.roster.parties()
            || self.key_context_digest == [0; 32]
            || self.ciphertext_digest
                != decryption_wire_ciphertext_digest_v1(&profile, self.roster, self.ciphertext)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidShareSet);
        }
        let (active_roster, _) = self.persistent_context.streaming_public_axes_v1();
        if active_roster.to_wire_roster()? != *self.roster {
            return Err(ZkAmsMkheErrorV1::InvalidShareSet);
        }
        self.persistent_context
            .validate_streaming_statement_axes_if_present_v1(
                self.roster,
                self.ciphertext,
                self.key_context_digest,
                self.party_b_pointers,
            )?;
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            self.party_bindings[party_index].validate(&profile, &self.parties)?;
            if usize::from(self.party_bindings[party_index].party_index) != party_index
                || self.party_bindings[party_index].party != self.parties.parties[party_index]
                || self.party_bindings[party_index].ciphertext_digest != self.ciphertext_digest
                || self.party_bindings[party_index].key_context_digest != self.key_context_digest
                || self.party_b_pointers[party_index].kind()
                    != ZkAmsMkheDirectObjectKindV1::CpkPartyB
                || self.party_b_pointers[party_index].payload_bytes()
                    != ZK_AMS_MKHE_CPK_PARTY_B_OBJECT_BYTES_V1 as u64
            {
                return Err(ZkAmsMkheErrorV1::InvalidShareSet);
            }
        }
        Ok(())
    }

    fn derive_common_a_limb(
        &self,
        profile: &BgvProfile,
        limb: usize,
    ) -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
        let (roster, cpk_transcript_digest) = self.persistent_context.streaming_public_axes_v1();
        derive_active_collective_public_a_limb_v1(profile, roster, cpk_transcript_digest, limb)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }
}

/// Zero-copy canonical view of one exact flat release `ZADP` proof.
#[derive(Clone, Copy)]
pub struct ZkAmsMkheDecryptionProofViewV1<'a> {
    bytes: &'a [u8],
    challenge_seed: [u8; 32],
    secret_offset: usize,
    error_offset: usize,
    smudge_offset: usize,
    wide_response_bytes: usize,
}

impl fmt::Debug for ZkAmsMkheDecryptionProofViewV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheDecryptionProofViewV1")
            .field("challenge_seed", &hex::encode(self.challenge_seed))
            .field("encoded_len", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

impl<'a> ZkAmsMkheDecryptionProofViewV1<'a> {
    /// Preflight exact release counts, length, canonical signed values, and bounds.
    pub fn decode_release_exact(bytes: &'a [u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let noise = zk_ams_mkhe_noise_certificate_v1()?;
        let challenge_weight = wide_relation_challenge_weight(profile.ring_degree)?;
        let (_, secret_limit) = super::small_response_parameters(1, challenge_weight, &profile)?;
        let (_, error_limit) = super::small_response_parameters(
            i64::from(profile.error_eta),
            challenge_weight,
            &profile,
        )?;
        let (_, wide_limit, wide_response_bytes) = wide_response_parameters(
            usize::from(noise.decryption_smudge_quotient_bits),
            challenge_weight,
        )?;
        let evidence = derive_decryption_resource_evidence(&profile)?;
        if u64::try_from(bytes.len()).ok() != Some(evidence.proof_payload_bytes) {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut cursor = 0;
        if read_array::<4>(bytes, &mut cursor)? != DECRYPTION_PROOF_TAG_V1
            || read_u8(bytes, &mut cursor)? != MKHE_VERSION_V1
            || usize::from(read_u16(bytes, &mut cursor)?) != wide_response_bytes
            || usize::try_from(read_u32(bytes, &mut cursor)?).ok() != Some(profile.ring_degree)
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let challenge_seed = read_array::<32>(bytes, &mut cursor)?;
        if challenge_seed == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        for _ in 0..3 {
            if usize::try_from(read_u32(bytes, &mut cursor)?).ok() != Some(profile.ring_degree) {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
        }
        if cursor != DECRYPTION_PROOF_HEADER_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let secret_offset = cursor;
        let small_vector_bytes = profile
            .ring_degree
            .checked_mul(DECRYPTION_SIGNED_SMALL_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let error_offset = secret_offset
            .checked_add(small_vector_bytes)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let smudge_offset = error_offset
            .checked_add(small_vector_bytes)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for index in 0..profile.ring_degree {
            if read_i64_at(bytes, secret_offset, index)?.unsigned_abs() > secret_limit as u64
                || read_i64_at(bytes, error_offset, index)?.unsigned_abs() > error_limit as u64
            {
                return Err(ZkAmsMkheErrorV1::InvalidShareProof);
            }
            let response = read_wide_at(bytes, smudge_offset, wide_response_bytes, index)?;
            if response.magnitude > wide_limit {
                return Err(ZkAmsMkheErrorV1::InvalidShareProof);
            }
        }
        let expected_end = smudge_offset
            .checked_add(
                profile
                    .ring_degree
                    .checked_mul(wide_response_bytes)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if expected_end != bytes.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(Self {
            bytes,
            challenge_seed,
            secret_offset,
            error_offset,
            smudge_offset,
            wide_response_bytes,
        })
    }

    /// Fiat--Shamir seed encoded by the proof.
    #[must_use]
    pub const fn challenge_seed(self) -> [u8; 32] {
        self.challenge_seed
    }

    fn secret_limb(&self, modulus: u64) -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let mut output = Vec::new();
        output
            .try_reserve_exact(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for index in 0..profile.ring_degree {
            output.push(signed_mod(
                read_i64_at(self.bytes, self.secret_offset, index)?,
                modulus,
            ));
        }
        Ok(output)
    }

    fn error_mod(&self, index: usize, modulus: u64) -> Result<u64, ZkAmsMkheErrorV1> {
        Ok(signed_mod(
            read_i64_at(self.bytes, self.error_offset, index)?,
            modulus,
        ))
    }

    fn smudge_mod(&self, index: usize, modulus: u64) -> Result<u64, ZkAmsMkheErrorV1> {
        Ok(read_wide_at(
            self.bytes,
            self.smudge_offset,
            self.wide_response_bytes,
            index,
        )?
        .mod_u64(modulus))
    }
}

fn read_i64_at(bytes: &[u8], vector_offset: usize, index: usize) -> Result<i64, ZkAmsMkheErrorV1> {
    let start = vector_offset
        .checked_add(
            index
                .checked_mul(size_of::<i64>())
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let end = start
        .checked_add(size_of::<i64>())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    Ok(i64::from_be_bytes(
        bytes
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    ))
}

fn read_wide_at(
    bytes: &[u8],
    vector_offset: usize,
    coefficient_bytes: usize,
    index: usize,
) -> Result<SignedWideV1, ZkAmsMkheErrorV1> {
    let start = vector_offset
        .checked_add(
            index
                .checked_mul(coefficient_bytes)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let end = start
        .checked_add(coefficient_bytes)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    SignedWideV1::decode_fixed(
        bytes
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    )
}

struct StreamingSnapshotAccumulatorV1 {
    identity: Option<ZkAmsMkheDecryptionStreamingSnapshotV1>,
}

impl StreamingSnapshotAccumulatorV1 {
    const fn new() -> Self {
        Self { identity: None }
    }

    fn observe(
        &mut self,
        receipt: &ZkAmsMkheDirectObjectReadReceiptV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let snapshot = receipt.snapshot();
        let observed = ZkAmsMkheDecryptionStreamingSnapshotV1 {
            provider_identity: snapshot.provider_identity(),
            snapshot_identity: snapshot.snapshot_identity(),
        };
        if observed.provider_identity == [0; 32]
            || observed.snapshot_identity == [0; 32]
            || self.identity.is_some_and(|expected| expected != observed)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.identity = Some(observed);
        Ok(())
    }

    fn finish(self) -> Result<ZkAmsMkheDecryptionStreamingSnapshotV1, ZkAmsMkheErrorV1> {
        self.identity.ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }
}

struct StreamingRnsObjectReaderV1<'a, P: ?Sized> {
    provider: &'a mut P,
    transaction: ZkAmsMkheDirectObjectReadTransactionV1,
    next_limb: usize,
    coefficient_count: usize,
}

impl<'a, P> StreamingRnsObjectReaderV1<'a, P>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    fn begin(
        expected_kind: ZkAmsMkheDirectObjectKindV1,
        pointer: ZkAmsMkheDirectObjectPointerV1,
        profile: &BgvProfile,
        provider: &'a mut P,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        profile.validate()?;
        let coefficient_count = profile
            .ring_degree
            .checked_mul(profile.moduli.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let expected_bytes = coefficient_count
            .checked_mul(size_of::<u64>())
            .and_then(|value| value.checked_add(size_of::<u32>()))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if usize::try_from(pointer.payload_bytes()).ok() != Some(expected_bytes) {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let transaction =
            ZkAmsMkheDirectObjectReadTransactionV1::begin(expected_kind, pointer, provider)?;
        let mut reader = Self {
            provider,
            transaction,
            next_limb: 0,
            coefficient_count,
        };
        let mut count = [0_u8; 4];
        if reader.transaction.read_next(reader.provider, &mut count)? != count.len()
            || usize::try_from(u32::from_be_bytes(count)).ok() != Some(coefficient_count)
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(reader)
    }

    fn read_limb(
        &mut self,
        profile: &BgvProfile,
        limb: usize,
    ) -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
        if limb != self.next_limb || profile.moduli.get(limb).is_none() {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let modulus = profile.moduli[limb];
        let mut values = Vec::new();
        values
            .try_reserve_exact(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut buffer = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
        while values.len() != profile.ring_degree {
            let coefficients = (profile.ring_degree - values.len()).min(buffer.len() / 8);
            let bytes = coefficients
                .checked_mul(8)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            if self
                .transaction
                .read_next(self.provider, &mut buffer[..bytes])?
                != bytes
            {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            for encoded in buffer[..bytes].chunks_exact(8) {
                let residue = u64::from_be_bytes(
                    encoded
                        .try_into()
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                );
                if residue >= modulus {
                    return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
                }
                values.push(residue);
            }
        }
        self.next_limb += 1;
        Ok(values)
    }

    fn finish(
        self,
        profile: &BgvProfile,
    ) -> Result<ZkAmsMkheDirectObjectReadReceiptV1, ZkAmsMkheErrorV1> {
        if self.next_limb != profile.moduli.len()
            || self.coefficient_count
                != profile
                    .ring_degree
                    .checked_mul(profile.moduli.len())
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            || self.transaction.remaining_bytes() != 0
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.transaction.finish(self.provider)
    }
}

fn read_complete_object_v1<P>(
    kind: ZkAmsMkheDirectObjectKindV1,
    pointer: ZkAmsMkheDirectObjectPointerV1,
    provider: &mut P,
) -> Result<(Vec<u8>, ZkAmsMkheDirectObjectReadReceiptV1), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let length = usize::try_from(pointer.payload_bytes())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    bytes.resize(length, 0);
    let mut transaction = ZkAmsMkheDirectObjectReadTransactionV1::begin(kind, pointer, provider)?;
    let mut cursor = 0;
    while cursor != bytes.len() {
        let end = cursor
            .checked_add(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1)
            .unwrap_or(bytes.len())
            .min(bytes.len());
        let expected = end - cursor;
        if transaction.read_next(provider, &mut bytes[cursor..end])? != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        cursor = end;
    }
    let receipt = transaction.finish(provider)?;
    Ok((bytes, receipt))
}

fn direct_pointer_from_manifest(
    pointer: ZkAmsMkheDecryptionTransportPointerV1,
) -> Result<ZkAmsMkheDirectObjectPointerV1, ZkAmsMkheErrorV1> {
    let kind = match pointer.kind() {
        ZkAmsMkheDecryptionTransportComponentKindV1::SharePolynomial => {
            ZkAmsMkheDirectObjectKindV1::DecryptionSharePolynomial
        }
        ZkAmsMkheDecryptionTransportComponentKindV1::ProofEnvelope => {
            ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof
        }
    };
    ZkAmsMkheDirectObjectPointerV1::new(kind, pointer.payload_bytes(), pointer.payload_blake3())
}

fn decode_streaming_manifest_exact(
    bytes: &[u8],
) -> Result<ZkAmsMkheDecryptionTransportManifestV1, ZkAmsMkheErrorV1> {
    if bytes.len() != ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut cursor = 0;
    if read_array::<4>(bytes, &mut cursor)? != DECRYPTION_SPLIT_MANIFEST_TAG_V1
        || read_u8(bytes, &mut cursor)? != MKHE_VERSION_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let binding = DecryptionBindingV1 {
        profile_digest: read_array::<32>(bytes, &mut cursor)?,
        roster_digest: read_array::<32>(bytes, &mut cursor)?,
        epoch: read_u64(bytes, &mut cursor)?,
        transcript_digest: read_array::<32>(bytes, &mut cursor)?,
        ciphertext_digest: read_array::<32>(bytes, &mut cursor)?,
        key_context_digest: read_array::<32>(bytes, &mut cursor)?,
        statement_binding_digest: read_array::<32>(bytes, &mut cursor)?,
        ciphertext_record_index: read_u32(bytes, &mut cursor)?,
        sample_index: read_u64(bytes, &mut cursor)?,
        party_index: read_u8(bytes, &mut cursor)?,
        party: ZkAmsMkhePartyIdV1::new(read_array::<32>(bytes, &mut cursor)?)?,
        level: read_u8(bytes, &mut cursor)?,
    };
    if read_u8(bytes, &mut cursor)? != DECRYPTION_SPLIT_COMPONENT_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let polynomial = read_decryption_transport_pointer(
        bytes,
        &mut cursor,
        0,
        ZkAmsMkheDecryptionTransportComponentKindV1::SharePolynomial,
    )?;
    let proof = read_decryption_transport_pointer(
        bytes,
        &mut cursor,
        1,
        ZkAmsMkheDecryptionTransportComponentKindV1::ProofEnvelope,
    )?;
    let manifest_digest = read_array::<32>(bytes, &mut cursor)?;
    let authentication = ArtifactAuthentication {
        version: MKHE_VERSION_V1,
        party: ZkAmsMkhePartyIdV1::new(read_array::<32>(bytes, &mut cursor)?)?,
        public_key: read_array::<33>(bytes, &mut cursor)?,
        signature: read_array::<65>(bytes, &mut cursor)?,
    };
    if cursor != bytes.len() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let manifest = ZkAmsMkheDecryptionTransportManifestV1 {
        binding,
        polynomial,
        proof,
        manifest_digest,
        authentication,
    };
    manifest.validate_structural()?;
    Ok(manifest)
}

fn update_streamed_rns_header(
    hash: &mut Keccak256,
    profile: &BgvProfile,
) -> Result<(), ZkAmsMkheErrorV1> {
    let coefficients = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    hash.update(
        &u32::try_from(coefficients)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?
            .to_be_bytes(),
    );
    Ok(())
}

fn update_residue_limb(hash: &mut Keccak256, residues: &[u64]) {
    for residue in residues {
        hash.update(&residue.to_be_bytes());
    }
}

fn hash_streamed_polynomial_v1<P>(
    kind: ZkAmsMkheDirectObjectKindV1,
    pointer: ZkAmsMkheDirectObjectPointerV1,
    profile: &BgvProfile,
    provider: &mut P,
    transcript: &mut Keccak256,
) -> Result<ZkAmsMkheDirectObjectReadReceiptV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    update_streamed_rns_header(transcript, profile)?;
    let mut reader = StreamingRnsObjectReaderV1::begin(kind, pointer, profile, provider)?;
    for limb in 0..profile.moduli.len() {
        let residues = reader.read_limb(profile, limb)?;
        update_residue_limb(transcript, &residues);
    }
    reader.finish(profile)
}

fn subtract_sparse_negacyclic_product_in_place(
    accumulator: &mut [u64],
    challenge: &[i8],
    source: &[u64],
    modulus: u64,
) -> Result<(), ZkAmsMkheErrorV1> {
    if accumulator.len() != challenge.len()
        || accumulator.len() != source.len()
        || accumulator.is_empty()
        || !accumulator.len().is_power_of_two()
        || source.iter().any(|residue| *residue >= modulus)
        || accumulator.iter().any(|residue| *residue >= modulus)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let degree = accumulator.len();
    for (shift, challenge_sign) in challenge.iter().copied().enumerate() {
        if challenge_sign == 0 {
            continue;
        }
        if ![-1, 1].contains(&challenge_sign) {
            return Err(ZkAmsMkheErrorV1::InvalidShareProof);
        }
        for (source_index, residue) in source.iter().copied().enumerate() {
            let destination = source_index + shift;
            let (destination, wrap_sign) = if destination >= degree {
                (destination - degree, -1_i8)
            } else {
                (destination, 1_i8)
            };
            if challenge_sign * wrap_sign > 0 {
                accumulator[destination] = mod_sub(accumulator[destination], residue, modulus);
            } else {
                accumulator[destination] = mod_add(accumulator[destination], residue, modulus);
            }
        }
    }
    Ok(())
}

fn add_share_limb_in_place(
    aggregate: &mut RnsPolynomial,
    profile: &BgvProfile,
    limb: usize,
    share: &[u64],
) -> Result<(), ZkAmsMkheErrorV1> {
    if share.len() != profile.ring_degree || limb >= profile.moduli.len() {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let start = limb
        .checked_mul(profile.ring_degree)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let end = start
        .checked_add(profile.ring_degree)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let aggregate_limb = aggregate
        .coefficients
        .get_mut(start..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
    let modulus = profile.moduli[limb];
    for (accumulator, residue) in aggregate_limb.iter_mut().zip(share) {
        if *residue >= modulus {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        *accumulator = mod_add(*accumulator, *residue, modulus);
    }
    Ok(())
}

fn initialize_decryption_challenge_transcript(
    profile: &BgvProfile,
    smudge_bits: usize,
    binding: &DecryptionBindingV1,
) -> Result<Keccak256, ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(DECRYPTION_CHALLENGE_DOMAIN_V1);
    hash.update(DECRYPTION_PROOF_DOMAIN_V1);
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
    hash.update(&super::WIDE_RELATION_MASK_SLACK_LOG2_V1.to_be_bytes());
    binding.update_hash(&mut hash);
    Ok(hash)
}

fn reconstruct_public_key_commitment_v1<P>(
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
    profile: &BgvProfile,
    proof: &ZkAmsMkheDecryptionProofViewV1<'_>,
    challenge: &[i8],
    party_index: usize,
    provider: &mut P,
    transcript: &mut Keccak256,
) -> Result<ZkAmsMkheDirectObjectReadReceiptV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    update_streamed_rns_header(transcript, profile)?;
    let mut party_b = StreamingRnsObjectReaderV1::begin(
        ZkAmsMkheDirectObjectKindV1::CpkPartyB,
        statement.party_b_pointers[party_index],
        profile,
        provider,
    )?;
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let common_a = statement.derive_common_a_limb(profile, limb)?;
        let secret_response = proof.secret_limb(modulus)?;
        let mut commitment = negacyclic_multiply(
            &common_a,
            &secret_response,
            modulus,
            profile.negacyclic_roots[limb],
        )?;
        drop(common_a);
        drop(secret_response);
        let plaintext_modulus = profile.plaintext_modulus.residue(modulus);
        for (coefficient, value) in commitment.iter_mut().enumerate() {
            *value = mod_add(
                mod_sub(0, *value, modulus),
                mod_mul(
                    plaintext_modulus,
                    proof.error_mod(coefficient, modulus)?,
                    modulus,
                ),
                modulus,
            );
        }
        let party_b_limb = party_b.read_limb(profile, limb)?;
        subtract_sparse_negacyclic_product_in_place(
            &mut commitment,
            challenge,
            &party_b_limb,
            modulus,
        )?;
        update_residue_limb(transcript, &commitment);
    }
    party_b.finish(profile)
}

fn reconstruct_share_commitment_and_aggregate_v1<P>(
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
    profile: &BgvProfile,
    proof: &ZkAmsMkheDecryptionProofViewV1<'_>,
    challenge: &[i8],
    share_pointer: ZkAmsMkheDirectObjectPointerV1,
    provider: &mut P,
    transcript: &mut Keccak256,
    aggregate: &mut RnsPolynomial,
) -> Result<ZkAmsMkheDirectObjectReadReceiptV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    update_streamed_rns_header(transcript, profile)?;
    let mut share_reader = StreamingRnsObjectReaderV1::begin(
        ZkAmsMkheDirectObjectKindV1::DecryptionSharePolynomial,
        share_pointer,
        profile,
        provider,
    )?;
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let start = limb
            .checked_mul(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let linear = statement
            .ciphertext
            .linear()
            .residues()
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidCiphertext)?;
        let secret_response = proof.secret_limb(modulus)?;
        let mut commitment = negacyclic_multiply(
            linear,
            &secret_response,
            modulus,
            profile.negacyclic_roots[limb],
        )?;
        drop(secret_response);
        let plaintext_modulus = profile.plaintext_modulus.residue(modulus);
        for (coefficient, value) in commitment.iter_mut().enumerate() {
            *value = mod_add(
                *value,
                mod_mul(
                    plaintext_modulus,
                    proof.smudge_mod(coefficient, modulus)?,
                    modulus,
                ),
                modulus,
            );
        }
        let share_limb = share_reader.read_limb(profile, limb)?;
        subtract_sparse_negacyclic_product_in_place(
            &mut commitment,
            challenge,
            &share_limb,
            modulus,
        )?;
        add_share_limb_in_place(aggregate, profile, limb, &share_limb)?;
        update_residue_limb(transcript, &commitment);
    }
    share_reader.finish(profile)
}

fn preflight_manifests(
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
    manifest_bytes: &[&[u8]],
) -> Result<
    [ZkAmsMkheDecryptionTransportManifestV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ZkAmsMkheIdentifiableDecryptionAbortV1,
> {
    if manifest_bytes.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(identifiable_abort(
            &statement.parties,
            manifest_bytes.len(),
            if manifest_bytes.len() < ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
                DecryptionAbortReasonV1::MissingShare
            } else {
                DecryptionAbortReasonV1::ExcessShare
            },
            statement.ciphertext_digest,
        ));
    }
    let mut manifests = Vec::new();
    manifests
        .try_reserve_exact(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map_err(|_| {
            identifiable_abort(
                &statement.parties,
                0,
                DecryptionAbortReasonV1::BindingMismatch,
                statement.ciphertext_digest,
            )
        })?;
    for (party_index, bytes) in manifest_bytes.iter().enumerate() {
        let manifest = match decode_streaming_manifest_exact(bytes) {
            Ok(manifest) => manifest,
            Err(ZkAmsMkheErrorV1::InvalidAuthentication) => {
                return Err(identifiable_abort(
                    &statement.parties,
                    party_index,
                    DecryptionAbortReasonV1::AuthenticationFailure,
                    statement.ciphertext_digest,
                ));
            }
            Err(_) => {
                return Err(identifiable_abort(
                    &statement.parties,
                    party_index,
                    DecryptionAbortReasonV1::BindingMismatch,
                    statement.ciphertext_digest,
                ));
            }
        };
        if let Err(reason) = validate_streaming_manifest_slot_v1(
            &manifest,
            party_index,
            statement.parties.parties[party_index],
            &statement.party_bindings[party_index],
        ) {
            return Err(identifiable_abort(
                &statement.parties,
                party_index,
                reason,
                statement.ciphertext_digest,
            ));
        }
        manifests.push(manifest);
    }
    manifests.try_into().map_err(|_| {
        identifiable_abort(
            &statement.parties,
            0,
            DecryptionAbortReasonV1::BindingMismatch,
            statement.ciphertext_digest,
        )
    })
}

fn validate_streaming_manifest_slot_v1(
    manifest: &ZkAmsMkheDecryptionTransportManifestV1,
    expected_index: usize,
    expected_party: ZkAmsMkhePartyIdV1,
    expected_binding: &DecryptionBindingV1,
) -> Result<(), DecryptionAbortReasonV1> {
    if usize::from(manifest.party_index()) != expected_index || manifest.party() != expected_party {
        return Err(DecryptionAbortReasonV1::ReorderedOrDuplicateShare);
    }
    if &manifest.binding != expected_binding {
        return Err(DecryptionAbortReasonV1::BindingMismatch);
    }
    Ok(())
}

/// Authenticate, verify, and combine the exact ordered eight split shares with
/// at most three complete RNS polynomial payloads resident at once.
///
/// Every manifest is authenticated and bound before the first large read. Each
/// `b_i` and share is then consumed in two complete BLAKE3-authenticated passes
/// from one immutable provider snapshot. The existing proof transcript and CRT
/// correctness bound are unchanged.
pub fn verify_combine_decode_zk_ams_mkhe_decryption_streaming_v1<P>(
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
    manifest_bytes: &[&[u8]],
    provider: &mut P,
) -> Result<ZkAmsMkheStreamingFullRosterDecryptionResultV1, ZkAmsMkheIdentifiableDecryptionAbortV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let binding_abort = |party_index: usize| {
        identifiable_abort(
            &statement.parties,
            party_index,
            DecryptionAbortReasonV1::BindingMismatch,
            statement.ciphertext_digest,
        )
    };
    statement.validate_compact().map_err(|_| binding_abort(0))?;
    let manifests = preflight_manifests(statement, manifest_bytes)?;
    let profile = release_profile_v1();
    let noise = zk_ams_mkhe_noise_certificate_v1().map_err(|_| binding_abort(0))?;
    let smudge_bits = usize::from(noise.decryption_smudge_quotient_bits);
    let final_residual_bits = usize::from(noise.final_decryption_residual_bits);
    let mut aggregate = RnsPolynomial::from_flat(
        &profile,
        statement.ciphertext.constant().residues().to_vec(),
    )
    .map_err(|_| binding_abort(0))?;
    let mut snapshot = StreamingSnapshotAccumulatorV1::new();
    let mut set_hash = Keccak256::new();
    set_hash.update(DECRYPTION_SET_DOMAIN_V1);
    set_hash.update(&statement.roster.roster_digest());
    set_hash.update(&statement.ciphertext_digest);

    for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let proof_pointer = direct_pointer_from_manifest(manifests[party_index].proof())
            .map_err(|_| binding_abort(party_index))?;
        let share_pointer = direct_pointer_from_manifest(manifests[party_index].polynomial())
            .map_err(|_| binding_abort(party_index))?;
        let (proof_bytes, proof_receipt) = read_complete_object_v1(
            ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
            proof_pointer,
            provider,
        )
        .map_err(|_| {
            identifiable_abort(
                &statement.parties,
                party_index,
                DecryptionAbortReasonV1::ProofFailure,
                statement.ciphertext_digest,
            )
        })?;
        snapshot
            .observe(&proof_receipt)
            .map_err(|_| binding_abort(party_index))?;
        let proof =
            ZkAmsMkheDecryptionProofViewV1::decode_release_exact(&proof_bytes).map_err(|_| {
                identifiable_abort(
                    &statement.parties,
                    party_index,
                    DecryptionAbortReasonV1::ProofFailure,
                    statement.ciphertext_digest,
                )
            })?;
        checked_ring_multiplication_work(&profile, 8).map_err(|_| binding_abort(party_index))?;
        let challenge = derive_sparse_challenge(profile.ring_degree, proof.challenge_seed())
            .map_err(|_| {
                identifiable_abort(
                    &statement.parties,
                    party_index,
                    DecryptionAbortReasonV1::ProofFailure,
                    statement.ciphertext_digest,
                )
            })?;
        let mut transcript = initialize_decryption_challenge_transcript(
            &profile,
            smudge_bits,
            &statement.party_bindings[party_index],
        )
        .map_err(|_| binding_abort(party_index))?;

        update_streamed_rns_header(&mut transcript, &profile)
            .map_err(|_| binding_abort(party_index))?;
        for limb in 0..profile.moduli.len() {
            let common_a = statement
                .derive_common_a_limb(&profile, limb)
                .map_err(|_| binding_abort(party_index))?;
            update_residue_limb(&mut transcript, &common_a);
        }
        let party_b_receipt = hash_streamed_polynomial_v1(
            ZkAmsMkheDirectObjectKindV1::CpkPartyB,
            statement.party_b_pointers[party_index],
            &profile,
            provider,
            &mut transcript,
        )
        .map_err(|_| binding_abort(party_index))?;
        snapshot
            .observe(&party_b_receipt)
            .map_err(|_| binding_abort(party_index))?;
        update_wire_rns_hash(&mut transcript, statement.ciphertext.constant())
            .map_err(|_| binding_abort(party_index))?;
        update_wire_rns_hash(&mut transcript, statement.ciphertext.linear())
            .map_err(|_| binding_abort(party_index))?;
        let share_receipt = hash_streamed_polynomial_v1(
            ZkAmsMkheDirectObjectKindV1::DecryptionSharePolynomial,
            share_pointer,
            &profile,
            provider,
            &mut transcript,
        )
        .map_err(|_| {
            identifiable_abort(
                &statement.parties,
                party_index,
                DecryptionAbortReasonV1::ProofFailure,
                statement.ciphertext_digest,
            )
        })?;
        snapshot
            .observe(&share_receipt)
            .map_err(|_| binding_abort(party_index))?;
        let party_b_relation_receipt = reconstruct_public_key_commitment_v1(
            statement,
            &profile,
            &proof,
            &challenge,
            party_index,
            provider,
            &mut transcript,
        )
        .map_err(|_| {
            identifiable_abort(
                &statement.parties,
                party_index,
                DecryptionAbortReasonV1::ProofFailure,
                statement.ciphertext_digest,
            )
        })?;
        snapshot
            .observe(&party_b_relation_receipt)
            .map_err(|_| binding_abort(party_index))?;
        let share_relation_receipt = reconstruct_share_commitment_and_aggregate_v1(
            statement,
            &profile,
            &proof,
            &challenge,
            share_pointer,
            provider,
            &mut transcript,
            &mut aggregate,
        )
        .map_err(|_| {
            identifiable_abort(
                &statement.parties,
                party_index,
                DecryptionAbortReasonV1::ProofFailure,
                statement.ciphertext_digest,
            )
        })?;
        snapshot
            .observe(&share_relation_receipt)
            .map_err(|_| binding_abort(party_index))?;
        if transcript.finalize() != proof.challenge_seed() {
            return Err(identifiable_abort(
                &statement.parties,
                party_index,
                DecryptionAbortReasonV1::ProofFailure,
                statement.ciphertext_digest,
            ));
        }
        set_hash.update(&[u8::try_from(party_index).unwrap_or(u8::MAX)]);
        set_hash.update(&manifests[party_index].manifest_digest());
    }
    let (plaintext, maximum_residual_bits) =
        decode_centered_plaintext(&profile, &aggregate, final_residual_bits).map_err(|_| {
            identifiable_abort(
                &statement.parties,
                0,
                DecryptionAbortReasonV1::CorrectnessBoundExceeded,
                statement.ciphertext_digest,
            )
        })?;
    Ok(ZkAmsMkheStreamingFullRosterDecryptionResultV1 {
        result: ZkAmsMkheFullRosterDecryptionResultV1 {
            plaintext,
            maximum_residual_bits,
            ordered_share_set_digest: set_hash.finalize(),
        },
        snapshot: snapshot.finish().map_err(|_| binding_abort(0))?,
    })
}

#[cfg(test)]
#[path = "decryption_streaming_tests.rs"]
mod tests;
