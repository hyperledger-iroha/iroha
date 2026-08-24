//! Allocation-bounded verification of the split decryption transport.
//!
//! This module is deliberately a child of `decryption`: it reuses the exact
//! V1 transcript, response bounds, CRT decoder, and abort taxonomy instead of
//! defining a second relation.  The native implementation remains a
//! `cfg(test)` small-profile reference path. Release resource evidence may only refer to the
//! streaming entry points below, and remains fail-closed until an authenticated peak-residency run
//! is installed for the exact bounded CAS worker.
use super::super::super::MaskedRelaxedRandomErrorV1;
#[cfg(test)]
use super::super::negacyclic_multiply;
#[cfg(test)]
use super::super::wire::ZkAmsMkheRnsPolynomialWireV1;
use super::super::{
    ArtifactAuthentication, BgvProfile, MAX_RANDOM_REJECTION_ATTEMPTS_V1, MKHE_VERSION_V1,
    MaskedRelaxedRandomSourceV1, PartySet, RnsPolynomial, ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active::{ZkAmsMkheActivePartySecretV1, zk_ams_mkhe_active_rkg_linear_proof_security_v1},
    checked_ring_multiplication_work,
    collective::{
        COLLECTIVE_CIPHERTEXT_DOMAIN_V1, ZkAmsMkheCollectivePartyStateV1,
        ZkAmsMkheStreamingCollectiveCiphertextBindingV1, ZkAmsMkheStreamingCollectiveCiphertextV1,
    },
    cpk_relation::{
        ZK_AMS_MKHE_CPK_PARTY_B_OBJECT_BYTES_V1, ZkAmsMkheCpkRelationErrorV1,
        ZkAmsMkhePreparedCollectivePublicAContextV1,
        active_collective_public_a_limb_frame_bytes_v1, prepare_active_collective_public_a_v1,
    },
    direct_object_transport::{
        ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1, ZkAmsMkheDirectObjectCasPublicationV1,
        ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1,
        ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheDirectObjectPublicationTransactionV1,
        ZkAmsMkheDirectObjectReadAtProviderV1, ZkAmsMkheDirectObjectReadReceiptV1,
        ZkAmsMkheDirectObjectReadTransactionV1,
    },
    manifest::{
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1, zk_ams_mkhe_noise_certificate_v1,
        zk_ams_mkhe_release_manifest_v1,
    },
    mod_add, mod_inverse, mod_mul, mod_pow, mod_sub,
    persistent_decryption_equality::{
        ZkAmsMkhePersistentDecryptionPartyUseV1,
        ZkAmsMkhePersistentDecryptionVerificationContextV1,
        ZkAmsMkheStreamingDecryptionAuthorityV1,
    },
    ring_multiplication_work, signed_mod,
    wire::ZkAmsMkheGovernedRosterWireV1,
    zk_ams_mkhe_security_certificate_v1,
};
use super::{
    DECRYPTION_CHALLENGE_DOMAIN_V1, DECRYPTION_MAX_WIDE_LIMBS_V1, DECRYPTION_PROOF_DOMAIN_V1,
    DECRYPTION_PROOF_HEADER_BYTES_V1, DECRYPTION_SET_DOMAIN_V1, DECRYPTION_SIGNED_SMALL_BYTES_V1,
    DECRYPTION_SPLIT_MANIFEST_AUTH_DOMAIN_V1, DecryptionAbortReasonV1, DecryptionBindingV1,
    DecryptionCiphertextAxesV1, SignedWideV1, ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1,
    ZkAmsMkheDecryptionTransportComponentKindV1, ZkAmsMkheDecryptionTransportManifestV1,
    ZkAmsMkheDecryptionTransportPointerV1, ZkAmsMkheFullRosterDecryptionResultV1,
    ZkAmsMkheIdentifiableDecryptionAbortV1, decode_centered_plaintext,
    decryption_binding_from_compact_axes_v1, decryption_split_manifest_digest,
    derive_decryption_resource_evidence, derive_sparse_challenge, identifiable_abort,
    initialize_decryption_challenge_transcript, read_array, read_decryption_transport_pointer,
    read_u8, read_u16, read_u32, read_u64, sample_signed_small, sample_signed_wide,
    small_response_parameters, validate_wide_relation_random_health,
    wide_relation_challenge_weight, wide_response_parameters,
};
use crate::{generalized_bulletproof::try_exact_capacity_vec_v1, vega::sponge::Keccak256};
use core::{fmt, mem::size_of};
const STREAMING_RESOURCE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.decryption-streaming-resource";
const DECRYPTION_PROOF_TAG_V1: [u8; 4] = *b"ZADP";
const DECRYPTION_SPLIT_MANIFEST_TAG_V1: [u8; 4] = *b"ZDSM";
const DECRYPTION_SPLIT_COMPONENT_COUNT_V1: u8 = 2;
const STREAMING_CIPHERTEXT_LIMB_COUNT_BYTES_V1: usize = size_of::<u32>();
// The native per-sample unbiased rejection limit remains 128. Only the outer
// Fiat--Shamir-with-aborts liveness policy is tightened: accepted attempts
// 1..=120 are byte-identical to native, while attempt 121 fails closed.
const STAGED_PROVER_MAXIMUM_FS_ATTEMPTS_V1: usize = 120;
const STAGED_PROVER_MAXIMUM_RING_MULTIPLICATIONS_V1: usize =
    1 + 2 * STAGED_PROVER_MAXIMUM_FS_ATTEMPTS_V1 + 2;
// Forwarded random bytes are charged one-for-one as classified governed bulk
// work. The cap is below the residual allowance after every deterministic
// release pass is enumerated below. It leaves enough room for all 120 outer
// attempts when each native rejection sampler accepts its first candidate,
// while adversarial inner rejection streams fail closed before 1e11 bulk units.
const STAGED_PROVER_RNG_BYTE_BUDGET_V1: u64 = 5_000_000_000;
const STAGED_PROVER_COMMON_A_CANDIDATE_BUDGET_V1: u64 = 1_500_000_000;
const DECRYPTION_BINDING_HASH_BYTES_V1: u64 = 6 * 32 + 8 + 4 + 8 + 1 + 32 + 1;
const ZK_AMS_MKHE_DECRYPTION_STAGED_RELEASE_KAT_DIGEST_V1: [u8; 32] = [0; 32];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SparseChallengeTermV1 {
    shift: usize,
    sign: i8,
}
/// Digest pinned only by an authenticated run of this exact streaming topology.
///
/// This is deliberately distinct from the native-residency certificate. It
/// remains zero until the complete bounded release worker has been measured.
pub const ZK_AMS_MKHE_DECRYPTION_STREAMING_RESIDENCY_CERTIFICATE_DIGEST_V1: [u8; 32] = [0; 32];
/// Missing implementation boundary that keeps streaming decryption fail-closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheDecryptionStreamingBlockerV1 {
    /// Canonical empty blocker slot. Only slots at or above `blocker_count` may contain this value.
    NoBlocker = 0,
    /// Historical compatibility value retained for downstream exhaustive
    /// matches. The bounded prover no longer activates this blocker.
    StagedProverOutputMissing = 1,
    /// Historical compatibility value retained for downstream exhaustive
    /// matches. It is no longer an active implementation blocker.
    CompactAuthorityConstructionStillNative = 2,
    /// Historical work-ceiling blocker retained for exhaustive downstream
    /// matches. The staged-local 120-attempt policy no longer activates it.
    StagedProverWorkCeilingExceeded = 3,
}
/// Phase-specific source accounting for the bounded verifier topology.
///
/// These are exact enumerated large-buffer payloads, not a peak-RSS claim. Allocator metadata,
/// stacks, and the surrounding worker must still be covered by the zero-pinned authenticated
/// residency certificate. Compact-authority figures count algorithm-owned buffers and the currently
/// borrowed share, but deliberately do not count storage, caching, or duplicate immutable-object
/// copies inside a caller-selected CAS provider. An in-process implementation that retains full
/// stages/seals/publication caches can therefore exceed the governed worker ceiling even though
/// this source topology fits it; release requires a separately bounded/external provider and the
/// nonzero runtime residency certificate which is presently absent.
///
/// Work fields classify governed bulk units: ring/coefficient operations and bytes sampled,
/// absorbed, scanned, folded, decoded, or emitted. Fixed control operations are not CPU-cycle
/// estimates. The prepared common-`a` axis records one child constructor after separate
/// statement/authority prevalidation; it is not a count of all profile/roster/PoP validation calls.
/// The absent release KAT and worker certificate remain mandatory for whole-operation authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDecryptionStreamingResidencyEvidenceV1 {
    /// Exact payload bytes in one full native RNS polynomial.
    pub native_rns_polynomial_bytes: u64,
    /// Exact bytes in one release RNS limb.
    pub rns_limb_bytes: u64,
    /// Exact owner plus heap payload of the compact streaming ciphertext
    /// manifest. The legacy field name is retained for evidence codec parity.
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
    /// Complete live passes over the 38 constant-component limb objects.
    pub ciphertext_constant_passes: u8,
    /// Complete live passes over the 38 linear-component limb objects.
    pub ciphertext_linear_passes: u8,
    /// Lower bound of the explicitly excluded native reference combine path.
    pub native_reference_lower_bound_bytes: u64,
    /// Lower bound inherited by the current compact-authority bridge before the returned compact
    /// statement exists. The legacy field name is kept; it now records the exact enumerated
    /// large-buffer peak of the bounded authority constructor.
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
    /// Two borrowed state-owned small witness vectors (`s_i`, `e_i`).
    pub staged_prover_party_state_witness_bytes: u64,
    /// One retained signed-wide smudging witness vector.
    pub staged_prover_smudge_witness_bytes: u64,
    /// Two retained signed-small proof mask vectors.
    pub staged_prover_small_mask_bytes: u64,
    /// One retained signed-wide proof mask vector.
    pub staged_prover_wide_mask_bytes: u64,
    /// One canonical sparse challenge vector.
    pub staged_prover_sparse_challenge_bytes: u64,
    /// Exact compact `(shift, sign)` challenge-term index.
    pub staged_prover_sparse_challenge_terms_bytes: u64,
    /// Four simultaneous zeroizing NTT limb buffers at commitment peak.
    pub staged_prover_limb_scratch_bytes: u64,
    /// One fixed zeroizing direct-object I/O buffer.
    pub staged_prover_direct_io_buffer_bytes: u64,
    /// Exact canonical signed `ZDSM` output bytes.
    pub staged_prover_manifest_bytes: u64,
    /// Once-prepared common-`a` frame-prefix heap payload. The cloned profile's
    /// moduli/roots are static slices; fixed digests, epoch, and container
    /// handles are small stack metadata excluded above and covered at runtime.
    pub staged_prover_common_a_context_bytes: u64,
    /// Per-limb common-`a` frame heap payload allocated beside the derived limb.
    pub staged_prover_common_a_limb_frame_scratch_bytes: u64,
    /// Largest phase while creating and publishing the share polynomial.
    pub staged_prover_share_construction_peak_bytes: u64,
    /// Largest phase while hashing public inputs from immutable CAS.
    pub staged_prover_public_input_hash_peak_bytes: u64,
    /// Largest phase while constructing both masked commitments.
    pub staged_prover_commitment_peak_bytes: u64,
    /// Largest phase while checking and writing canonical proof responses.
    pub staged_prover_proof_write_peak_bytes: u64,
    /// Largest phase while semantically replaying the published proof.
    pub staged_prover_self_verification_peak_bytes: u64,
    /// Largest enumerated bounded-prover phase.
    pub staged_prover_enumerated_peak_bytes: u64,
    /// Whether caller-selected CAS staging/cache residency is included.
    /// This remains false; the authenticated whole-worker certificate covers it.
    pub staged_prover_cas_backend_residency_enumerated: bool,
    /// The enumerated staged-prover topology fits the governed ceiling.
    pub staged_prover_enumerated_ceiling_met: bool,
    /// Canonical source emission passes for each share/proof object.
    pub staged_prover_component_source_passes: u8,
    /// Staged-local outer Fiat--Shamir-with-aborts mask attempts.
    pub staged_prover_maximum_rejection_attempts: u8,
    /// Exact unchanged native inner unbiased rejection-sampling limit.
    pub staged_prover_inner_rejection_attempts: u8,
    /// Exact worst-case number of release-RNS multiplications executed by one
    /// accepted-last-attempt staged share: `1 + 2*120 + 2 = 243`.
    pub staged_prover_maximum_ring_multiplications: u16,
    /// Classified ring-multiplication bulk work for those exact 243 operations.
    pub staged_prover_ring_multiplication_work_units: u64,
    /// Maximum random bytes forwarded by the fail-closed staged budget owner.
    pub staged_prover_rng_byte_budget: u64,
    /// Bytes required for two promptly healthy checks, the smudge witness, and
    /// all 120 mask attempts when every native inner sampler accepts first try.
    pub staged_prover_first_candidate_rng_bytes: u64,
    /// Maximum deterministic SHAKE candidates consumed while deriving every
    /// common-`a` limb across attempts and replay.
    pub staged_prover_common_a_candidate_budget: u64,
    /// Exact candidates required if every common-`a` coefficient accepts its
    /// first deterministic SHAKE word.
    pub staged_prover_first_candidate_common_a_candidates: u64,
    /// Candidate budget converted to byte-consistent classified work (`* 8`).
    pub staged_prover_common_a_xof_byte_budget: u64,
    /// First-candidate common-`a` demand converted to SHAKE bytes (`* 8`).
    pub staged_prover_first_candidate_common_a_xof_bytes: u64,
    /// Fixed accepted common-`a` residue reductions/emissions across all limb
    /// derivations, separately from SHAKE candidate-byte work.
    pub staged_prover_common_a_residue_output_work_units: u64,
    /// Prepared common-`a` child-constructor invocations after the separate
    /// statement/authority prevalidations. This is one before side effects, not
    /// a total count of fixed profile/roster/PoP validation calls.
    pub staged_prover_common_a_prepare_validation_passes: u8,
    /// Exact prepared common-`a` limb derivations across attempts and replay.
    pub staged_prover_common_a_limb_derivations: u64,
    /// Exact 158-byte SHAKE frames constructed and absorbed by those derives.
    pub staged_prover_common_a_frame_work_units: u64,
    /// Bytes read across every immutable party-B/share/proof object scan.
    pub staged_prover_immutable_object_scan_work_units: u64,
    /// Complete C0/C1 object bytes scanned during mandatory live preflight.
    pub staged_prover_ciphertext_preflight_scan_work_units: u64,
    /// Canonical ciphertext-digest bytes absorbed during mandatory preflight.
    pub staged_prover_ciphertext_preflight_hash_work_units: u64,
    /// Sum of the two mandatory ciphertext preflight classifications.
    pub staged_prover_ciphertext_preflight_work_units: u64,
    /// Bytes absorbed from every Fiat--Shamir prefix and seven polynomial
    /// frames, separately from immutable provider read/validation work.
    pub staged_prover_transcript_hash_work_units: u64,
    /// Explicit copied Keccak state bytes for 120 attempts plus replay.
    pub staged_prover_transcript_fork_work_units: u64,
    /// Enumerated challenge generation, sparse folds, non-NTT coefficient
    /// arithmetic, and canonical source-emission work before semantic replay.
    pub staged_prover_response_work_units: u64,
    /// Enumerated proof decoding, sparse equation, and non-NTT coefficient
    /// work in the mandatory published-object semantic replay.
    pub staged_prover_semantic_replay_work_units: u64,
    /// Sum of ring, RNG, prepared common-`a`, immutable-scan, transcript,
    /// response, and semantic-replay work for one accepted-last-attempt share.
    pub staged_prover_total_work_units: u64,
    /// Whether the enumerated classified bulk total fits `profile.max_work_units`.
    pub staged_prover_work_ceiling_met: bool,
    /// Maximum immutable reads of `b_i` when the final permitted attempt is
    /// accepted: one per attempt plus two semantic-replay passes.
    pub staged_prover_party_b_read_passes: u8,
    /// Maximum immutable share reads after source emission: sealed and
    /// published readback, one per attempt, and two semantic-replay passes.
    pub staged_prover_share_immutable_read_passes: u8,
    /// Immutable proof reads after source emission (seal, publication, replay).
    pub staged_prover_proof_immutable_read_passes: u8,
    /// Cached-prefix C0 passes after the separate live preflight.
    pub staged_prover_ciphertext_constant_read_passes: u8,
    /// Share construction, cached prefix, 120 attempts, and replay C1 passes.
    pub staged_prover_ciphertext_linear_read_passes: u8,
    /// The enumerated verifier payload fits the static workspace ceiling.
    pub enumerated_verifier_ceiling_met: bool,
    /// Whether bounded staged prover output exists in this implementation.
    pub staged_prover_output_implemented: bool,
    /// Authenticated end-to-end release KAT for CAS publication, replay,
    /// signed-manifest production, and eight-party streamed consumption.
    /// Zero remains absent; helper parity tests do not satisfy this gate.
    pub staged_prover_release_kat_digest: [u8; 32],
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
    /// Always false until the authenticated peak run exists.
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
            || self.staged_prover_cas_backend_residency_enumerated
            || !self.staged_prover_enumerated_ceiling_met
            || self.staged_prover_first_candidate_rng_bytes > self.staged_prover_rng_byte_budget
            || self.staged_prover_first_candidate_common_a_candidates
                > self.staged_prover_common_a_candidate_budget
            || self.staged_prover_first_candidate_common_a_xof_bytes
                > self.staged_prover_common_a_xof_byte_budget
            || self.staged_prover_common_a_prepare_validation_passes != 1
            || !self.staged_prover_work_ceiling_met
            || !self.staged_prover_output_implemented
            || self.staged_prover_release_kat_digest != [0; 32]
            || !self.bounded_compact_authority_construction_implemented
            || self.implementation_blocker_count != 0
            || self.implementation_blockers
                != [
                    ZkAmsMkheDecryptionStreamingBlockerV1::NoBlocker,
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
    let ciphertext_input_bytes = (size_of::<ZkAmsMkheStreamingCollectiveCiphertextV1>() as u64)
        .checked_add(
            (size_of::<ZkAmsMkheDirectObjectPointerV1>() as u64)
                .checked_mul(limbs)
                .and_then(|value| value.checked_mul(4))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .and_then(|value| {
            value.checked_add(
                (size_of::<ZkAmsMkheDirectObjectReadReceiptV1>() as u64)
                    .checked_mul(limbs)?
                    .checked_mul(4)?,
            )
        })
        .and_then(|value| {
            value.checked_add(
                (size_of::<ZkAmsMkheDirectObjectPublicationReceiptV1>() as u64)
                    .checked_mul(limbs)?
                    .checked_mul(2)?,
            )
        })
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
        .checked_add(rns_limb_bytes)
        .and_then(|value| value.checked_add(direct_read_buffer_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let public_input_hash_peak_bytes = proof_load_peak_bytes
        .checked_add(sparse_challenge_bytes)
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
                .checked_mul(4)
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
    let staged_prover_party_state_witness_bytes = degree
        .checked_mul(size_of::<i64>() as u64)
        .and_then(|value| value.checked_mul(2))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_smudge_witness_bytes = degree
        .checked_mul(size_of::<SignedWideV1>() as u64)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_small_mask_bytes = staged_prover_party_state_witness_bytes;
    let staged_prover_wide_mask_bytes = staged_prover_smudge_witness_bytes;
    let staged_prover_sparse_challenge_bytes = degree;
    let challenge_weight = u64::try_from(wide_relation_challenge_weight(profile.ring_degree)?)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_sparse_challenge_terms_bytes = challenge_weight
        .checked_mul(size_of::<SparseChallengeTermV1>() as u64)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_limb_scratch_bytes = rns_limb_bytes
        .checked_mul(4)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_direct_io_buffer_bytes = direct_read_buffer_bytes;
    let staged_prover_manifest_bytes = ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1 as u64;
    let staged_prover_common_a_context_bytes = u64::try_from(
        active_collective_public_a_limb_frame_bytes_v1()
            .checked_sub(size_of::<u16>())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_common_a_limb_frame_scratch_bytes =
        u64::try_from(active_collective_public_a_limb_frame_bytes_v1())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_witness_base_bytes = ciphertext_input_bytes
        .checked_add(staged_prover_party_state_witness_bytes)
        .and_then(|value| value.checked_add(staged_prover_smudge_witness_bytes))
        .and_then(|value| value.checked_add(staged_prover_common_a_context_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_masked_base_bytes = staged_prover_witness_base_bytes
        .checked_add(staged_prover_small_mask_bytes)
        .and_then(|value| value.checked_add(staged_prover_wide_mask_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_share_construction_peak_bytes = staged_prover_witness_base_bytes
        .checked_add(
            rns_limb_bytes
                .checked_mul(4)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .and_then(|value| value.checked_add(staged_prover_direct_io_buffer_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_public_input_hash_peak_bytes = staged_prover_witness_base_bytes
        .checked_add(rns_limb_bytes)
        .and_then(|value| value.checked_add(staged_prover_direct_io_buffer_bytes))
        .and_then(|value| value.checked_add(staged_prover_common_a_limb_frame_scratch_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_commitment_peak_bytes = staged_prover_masked_base_bytes
        .checked_add(staged_prover_limb_scratch_bytes)
        .and_then(|value| value.checked_add(staged_prover_direct_io_buffer_bytes))
        .and_then(|value| value.checked_add(staged_prover_common_a_limb_frame_scratch_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_proof_write_peak_bytes = staged_prover_masked_base_bytes
        .checked_add(staged_prover_sparse_challenge_bytes)
        .and_then(|value| value.checked_add(staged_prover_sparse_challenge_terms_bytes))
        .and_then(|value| value.checked_add(staged_prover_direct_io_buffer_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_self_verification_peak_bytes = ciphertext_input_bytes
        .checked_add(staged_prover_party_state_witness_bytes)
        .and_then(|value| value.checked_add(staged_prover_common_a_context_bytes))
        .and_then(|value| value.checked_add(proof_view_backing_bytes))
        .and_then(|value| value.checked_add(staged_prover_limb_scratch_bytes))
        .and_then(|value| value.checked_add(staged_prover_direct_io_buffer_bytes))
        .and_then(|value| value.checked_add(staged_prover_sparse_challenge_bytes))
        .and_then(|value| value.checked_add(staged_prover_sparse_challenge_terms_bytes))
        .and_then(|value| value.checked_add(staged_prover_common_a_limb_frame_scratch_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_enumerated_peak_bytes = *[
        staged_prover_share_construction_peak_bytes,
        staged_prover_public_input_hash_peak_bytes,
        staged_prover_commitment_peak_bytes,
        staged_prover_proof_write_peak_bytes,
        staged_prover_self_verification_peak_bytes,
    ]
    .iter()
    .max()
    .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let staged_prover_enumerated_ceiling_met =
        staged_prover_enumerated_peak_bytes <= governed_workspace_ceiling_bytes;
    let maximum_fs_attempts = u64::try_from(STAGED_PROVER_MAXIMUM_FS_ATTEMPTS_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let inner_rejection_attempts = u64::try_from(MAX_RANDOM_REJECTION_ATTEMPTS_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let maximum_ring_multiplications = u64::try_from(STAGED_PROVER_MAXIMUM_RING_MULTIPLICATIONS_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_ring_multiplication_work_units = ring_multiplication_work(&profile)?
        .checked_mul(maximum_ring_multiplications)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let noise = zk_ams_mkhe_noise_certificate_v1()?;
    let smudge_bits = usize::from(noise.decryption_smudge_quotient_bits);
    let smudge_bound = super::WideMagnitudeV1::max_for_bits(smudge_bits)?;
    let (wide_mask_bound, _, wide_response_bytes) = wide_response_parameters(
        smudge_bits,
        usize::try_from(challenge_weight).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )?;
    let smudge_first_candidate_bytes = staged_signed_wide_candidate_bytes_v1(&smudge_bound)?;
    let wide_mask_first_candidate_bytes = staged_signed_wide_candidate_bytes_v1(&wide_mask_bound)?;
    // Two health checks each need a first block and one distinct block. The
    // final 64-byte scalar is the first authentication nonce candidate.
    let staged_prover_first_candidate_rng_bytes = 4_u64
        .checked_mul(32)
        .and_then(|value| value.checked_add(64))
        .and_then(|value| value.checked_add(degree.checked_mul(smudge_first_candidate_bytes)?))
        .and_then(|value| {
            value.checked_add(
                maximum_fs_attempts.checked_mul(
                    degree.checked_mul(
                        2_u64
                            .checked_mul(DECRYPTION_SIGNED_SMALL_BYTES_V1 as u64)?
                            .checked_add(wide_mask_first_candidate_bytes)?,
                    )?,
                )?,
            )
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if staged_prover_first_candidate_rng_bytes > STAGED_PROVER_RNG_BYTE_BUDGET_V1 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let polynomial_object_bytes = legacy.split_polynomial_object_bytes;
    let proof_object_bytes = legacy.split_proof_envelope_bytes;
    let ciphertext_component_object_bytes = rns_limb_bytes
        .checked_add(STREAMING_CIPHERTEXT_LIMB_COUNT_BYTES_V1 as u64)
        .and_then(|value| value.checked_mul(limbs))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_immutable_object_scan_work_units = polynomial_object_bytes
        .checked_mul(6)
        .and_then(|value| value.checked_add(proof_object_bytes.checked_mul(3)?))
        .and_then(|value| value.checked_add(ciphertext_component_object_bytes.checked_mul(124)?))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_ciphertext_preflight_scan_work_units = ciphertext_component_object_bytes
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let transcript_prefix_bytes = u64::try_from(
        DECRYPTION_CHALLENGE_DOMAIN_V1
            .len()
            .checked_add(DECRYPTION_PROOF_DOMAIN_V1.len())
            .and_then(|value| value.checked_add(2 + 2 + 4))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    .checked_add(DECRYPTION_BINDING_HASH_BYTES_V1)
    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let rns_hash_frame_bytes = native_rns_polynomial_bytes
        .checked_add(size_of::<u32>() as u64)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_ciphertext_preflight_hash_work_units = 163_u64
        .checked_add(
            rns_hash_frame_bytes
                .checked_mul(2)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_ciphertext_preflight_work_units =
        staged_prover_ciphertext_preflight_scan_work_units
            .checked_add(staged_prover_ciphertext_preflight_hash_work_units)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_transcript_hash_work_units = transcript_prefix_bytes
        .checked_add(
            rns_hash_frame_bytes
                .checked_mul(5)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .and_then(|value| {
            value.checked_add(
                rns_hash_frame_bytes
                    .checked_mul(2)?
                    .checked_mul(maximum_fs_attempts.checked_add(1)?)?,
            )
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_transcript_fork_work_units = (size_of::<Keccak256>() as u64)
        .checked_mul(
            maximum_fs_attempts
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let sparse_challenge_stream_bytes = challenge_weight
        .checked_mul(inner_rejection_attempts)
        .and_then(|value| value.checked_mul(size_of::<u64>() as u64))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    // Each challenge byte is generated and then inspected; proof attempts also
    // scan the complete sparse vector to materialize the compact term index.
    let challenge_and_term_index_work = sparse_challenge_stream_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_mul(maximum_fs_attempts.checked_add(1)?))
        .and_then(|value| value.checked_add(degree.checked_mul(maximum_fs_attempts)?))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    // Fit checks run on every attempt and accepted responses are folded once
    // more while writing. A wide fold charges every fixed magnitude limb.
    let response_fold_work = degree
        .checked_mul(challenge_weight)
        .and_then(|value| {
            value.checked_mul(2_u64.checked_add(DECRYPTION_MAX_WIDE_LIMBS_V1 as u64)?)
        })
        .and_then(|value| value.checked_mul(maximum_fs_attempts.checked_add(1)?))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    // Outside the NTTs: share creation converts/adds two RNS vectors, while
    // every attempt converts one secret mask for each commitment and adds the
    // error/smudge mask (four complete RNS coefficient passes).
    let non_ntt_prover_coefficient_work = native_rns_polynomial_bytes
        .checked_div(size_of::<u64>() as u64)
        .and_then(|coefficients| {
            coefficients.checked_mul(2_u64.checked_add(maximum_fs_attempts.checked_mul(4)?)?)
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_response_work_units = challenge_and_term_index_work
        .checked_add(response_fold_work)
        .and_then(|value| value.checked_add(non_ntt_prover_coefficient_work))
        .and_then(|value| value.checked_add(polynomial_object_bytes))
        .and_then(|value| value.checked_add(proof_object_bytes))
        .and_then(|value| value.checked_add(staged_prover_manifest_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let rns_coefficients = degree
        .checked_mul(limbs)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    // Two replay equations each validate two limbs, scan the full challenge,
    // and apply every sparse term. The remaining passes derive common-a, add
    // responses, and decode secret twice, error once, and one wide response.
    let replay_sparse_and_coefficient_passes = 2_u64
        .checked_mul(
            3_u64
                .checked_add(challenge_weight)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .and_then(|value| value.checked_add(3))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let replay_response_decode_bytes = 3_u64
        .checked_mul(DECRYPTION_SIGNED_SMALL_BYTES_V1 as u64)
        .and_then(|value| value.checked_add(u64::try_from(wide_response_bytes).ok()?))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_semantic_replay_work_units = rns_coefficients
        .checked_mul(replay_sparse_and_coefficient_passes)
        .and_then(|value| {
            value.checked_add(rns_coefficients.checked_mul(replay_response_decode_bytes)?)
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    // The cached prefix derives common-a once. Each of 120 attempts and the
    // semantic replay derives it once more for U_pk.
    let staged_prover_first_candidate_common_a_candidates = rns_coefficients
        .checked_mul(
            maximum_fs_attempts
                .checked_add(2)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if staged_prover_first_candidate_common_a_candidates
        > STAGED_PROVER_COMMON_A_CANDIDATE_BUDGET_V1
    {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let staged_prover_common_a_xof_byte_budget = STAGED_PROVER_COMMON_A_CANDIDATE_BUDGET_V1
        .checked_mul(size_of::<u64>() as u64)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_first_candidate_common_a_xof_bytes =
        staged_prover_first_candidate_common_a_candidates
            .checked_mul(size_of::<u64>() as u64)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_common_a_residue_output_work_units =
        staged_prover_first_candidate_common_a_candidates;
    let staged_prover_common_a_prepare_validation_passes = 1_u8;
    let staged_prover_common_a_limb_derivations = limbs
        .checked_mul(
            maximum_fs_attempts
                .checked_add(2)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_common_a_frame_work_units =
        u64::try_from(active_collective_public_a_limb_frame_bytes_v1())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .checked_mul(staged_prover_common_a_limb_derivations)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_total_work_units = staged_prover_ring_multiplication_work_units
        .checked_add(STAGED_PROVER_RNG_BYTE_BUDGET_V1)
        .and_then(|value| value.checked_add(staged_prover_common_a_xof_byte_budget))
        .and_then(|value| value.checked_add(staged_prover_common_a_residue_output_work_units))
        .and_then(|value| value.checked_add(staged_prover_common_a_context_bytes))
        .and_then(|value| value.checked_add(staged_prover_common_a_frame_work_units))
        .and_then(|value| value.checked_add(staged_prover_immutable_object_scan_work_units))
        .and_then(|value| value.checked_add(staged_prover_transcript_hash_work_units))
        .and_then(|value| value.checked_add(staged_prover_transcript_fork_work_units))
        .and_then(|value| value.checked_add(staged_prover_response_work_units))
        .and_then(|value| value.checked_add(staged_prover_semantic_replay_work_units))
        .and_then(|value| value.checked_add(staged_prover_ciphertext_preflight_work_units))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let staged_prover_work_ceiling_met = staged_prover_total_work_units <= profile.max_work_units;
    let staged_prover_output_implemented = true;
    let bounded_compact_authority_construction_implemented = true;
    let authenticated_peak_residency_digest =
        ZK_AMS_MKHE_DECRYPTION_STREAMING_RESIDENCY_CERTIFICATE_DIGEST_V1;
    let release_certified = enumerated_verifier_ceiling_met
        && compact_authority_enumerated_ceiling_met
        && staged_prover_enumerated_ceiling_met
        && staged_prover_work_ceiling_met
        && staged_prover_output_implemented
        && ZK_AMS_MKHE_DECRYPTION_STAGED_RELEASE_KAT_DIGEST_V1 != [0; 32]
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
        maximum_full_rns_polynomials: 1,
        maximum_rns_limb_buffers: 4,
        party_b_passes: 2,
        decryption_share_passes: 2,
        ciphertext_constant_passes: 10,
        ciphertext_linear_passes: 17,
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
        staged_prover_party_state_witness_bytes,
        staged_prover_smudge_witness_bytes,
        staged_prover_small_mask_bytes,
        staged_prover_wide_mask_bytes,
        staged_prover_sparse_challenge_bytes,
        staged_prover_sparse_challenge_terms_bytes,
        staged_prover_limb_scratch_bytes,
        staged_prover_direct_io_buffer_bytes,
        staged_prover_manifest_bytes,
        staged_prover_common_a_context_bytes,
        staged_prover_common_a_limb_frame_scratch_bytes,
        staged_prover_share_construction_peak_bytes,
        staged_prover_public_input_hash_peak_bytes,
        staged_prover_commitment_peak_bytes,
        staged_prover_proof_write_peak_bytes,
        staged_prover_self_verification_peak_bytes,
        staged_prover_enumerated_peak_bytes,
        staged_prover_cas_backend_residency_enumerated: false,
        staged_prover_enumerated_ceiling_met,
        staged_prover_component_source_passes: 1,
        staged_prover_maximum_rejection_attempts: u8::try_from(
            STAGED_PROVER_MAXIMUM_FS_ATTEMPTS_V1,
        )
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        staged_prover_inner_rejection_attempts: u8::try_from(MAX_RANDOM_REJECTION_ATTEMPTS_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        staged_prover_maximum_ring_multiplications: u16::try_from(
            STAGED_PROVER_MAXIMUM_RING_MULTIPLICATIONS_V1,
        )
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        staged_prover_ring_multiplication_work_units,
        staged_prover_rng_byte_budget: STAGED_PROVER_RNG_BYTE_BUDGET_V1,
        staged_prover_first_candidate_rng_bytes,
        staged_prover_common_a_candidate_budget: STAGED_PROVER_COMMON_A_CANDIDATE_BUDGET_V1,
        staged_prover_first_candidate_common_a_candidates,
        staged_prover_common_a_xof_byte_budget,
        staged_prover_first_candidate_common_a_xof_bytes,
        staged_prover_common_a_residue_output_work_units,
        staged_prover_common_a_prepare_validation_passes,
        staged_prover_common_a_limb_derivations,
        staged_prover_common_a_frame_work_units,
        staged_prover_immutable_object_scan_work_units,
        staged_prover_ciphertext_preflight_scan_work_units,
        staged_prover_ciphertext_preflight_hash_work_units,
        staged_prover_ciphertext_preflight_work_units,
        staged_prover_transcript_hash_work_units,
        staged_prover_transcript_fork_work_units,
        staged_prover_response_work_units,
        staged_prover_semantic_replay_work_units,
        staged_prover_total_work_units,
        staged_prover_work_ceiling_met,
        staged_prover_party_b_read_passes: 2,
        staged_prover_share_immutable_read_passes: 4,
        staged_prover_proof_immutable_read_passes: 3,
        staged_prover_ciphertext_constant_read_passes: 1,
        staged_prover_ciphertext_linear_read_passes: u8::try_from(
            STAGED_PROVER_MAXIMUM_FS_ATTEMPTS_V1 + 3,
        )
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        enumerated_verifier_ceiling_met,
        staged_prover_output_implemented,
        staged_prover_release_kat_digest: ZK_AMS_MKHE_DECRYPTION_STAGED_RELEASE_KAT_DIGEST_V1,
        bounded_compact_authority_construction_implemented,
        implementation_blocker_count: 0,
        implementation_blockers: [
            ZkAmsMkheDecryptionStreamingBlockerV1::NoBlocker,
            ZkAmsMkheDecryptionStreamingBlockerV1::NoBlocker,
        ],
        authenticated_peak_residency_digest,
        release_certified,
        evidence_digest: [0; 32],
    };
    evidence.evidence_digest = streaming_residency_evidence_digest(evidence);
    Ok(evidence)
}
fn staged_signed_wide_candidate_bytes_v1(
    bound: &super::WideMagnitudeV1,
) -> Result<u64, ZkAmsMkheErrorV1> {
    let twice_bound = bound
        .checked_mul_u64(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    u64::try_from(twice_bound.bit_len().div_ceil(8))
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
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
        evidence.staged_prover_party_state_witness_bytes,
        evidence.staged_prover_smudge_witness_bytes,
        evidence.staged_prover_small_mask_bytes,
        evidence.staged_prover_wide_mask_bytes,
        evidence.staged_prover_sparse_challenge_bytes,
        evidence.staged_prover_sparse_challenge_terms_bytes,
        evidence.staged_prover_limb_scratch_bytes,
        evidence.staged_prover_direct_io_buffer_bytes,
        evidence.staged_prover_manifest_bytes,
        evidence.staged_prover_common_a_context_bytes,
        evidence.staged_prover_common_a_limb_frame_scratch_bytes,
        evidence.staged_prover_share_construction_peak_bytes,
        evidence.staged_prover_public_input_hash_peak_bytes,
        evidence.staged_prover_commitment_peak_bytes,
        evidence.staged_prover_proof_write_peak_bytes,
        evidence.staged_prover_self_verification_peak_bytes,
        evidence.staged_prover_enumerated_peak_bytes,
        evidence.staged_prover_ring_multiplication_work_units,
        evidence.staged_prover_rng_byte_budget,
        evidence.staged_prover_first_candidate_rng_bytes,
        evidence.staged_prover_common_a_candidate_budget,
        evidence.staged_prover_first_candidate_common_a_candidates,
        evidence.staged_prover_common_a_xof_byte_budget,
        evidence.staged_prover_first_candidate_common_a_xof_bytes,
        evidence.staged_prover_common_a_residue_output_work_units,
        evidence.staged_prover_common_a_limb_derivations,
        evidence.staged_prover_common_a_frame_work_units,
        evidence.staged_prover_immutable_object_scan_work_units,
        evidence.staged_prover_ciphertext_preflight_scan_work_units,
        evidence.staged_prover_ciphertext_preflight_hash_work_units,
        evidence.staged_prover_ciphertext_preflight_work_units,
        evidence.staged_prover_transcript_hash_work_units,
        evidence.staged_prover_transcript_fork_work_units,
        evidence.staged_prover_response_work_units,
        evidence.staged_prover_semantic_replay_work_units,
        evidence.staged_prover_total_work_units,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(
        &evidence
            .staged_prover_maximum_ring_multiplications
            .to_be_bytes(),
    );
    hash.update(&[
        evidence.maximum_full_rns_polynomials,
        evidence.maximum_rns_limb_buffers,
        evidence.party_b_passes,
        evidence.decryption_share_passes,
        evidence.ciphertext_constant_passes,
        evidence.ciphertext_linear_passes,
        evidence
            .compact_authority_cas_backend_residency_enumerated
            .into(),
        evidence.compact_authority_enumerated_ceiling_met.into(),
        evidence.compact_authority_party_b_source_passes,
        evidence.compact_authority_publication_readback_passes,
        evidence.compact_authority_context_digest_read_passes,
        evidence
            .staged_prover_cas_backend_residency_enumerated
            .into(),
        evidence.staged_prover_enumerated_ceiling_met.into(),
        evidence.staged_prover_work_ceiling_met.into(),
        evidence.staged_prover_component_source_passes,
        evidence.staged_prover_maximum_rejection_attempts,
        evidence.staged_prover_inner_rejection_attempts,
        evidence.staged_prover_common_a_prepare_validation_passes,
        evidence.staged_prover_party_b_read_passes,
        evidence.staged_prover_share_immutable_read_passes,
        evidence.staged_prover_proof_immutable_read_passes,
        evidence.staged_prover_ciphertext_constant_read_passes,
        evidence.staged_prover_ciphertext_linear_read_passes,
        evidence.enumerated_verifier_ceiling_met.into(),
        evidence.staged_prover_output_implemented.into(),
        evidence
            .bounded_compact_authority_construction_implemented
            .into(),
        evidence.implementation_blocker_count,
        evidence.implementation_blockers[0] as u8,
        evidence.implementation_blockers[1] as u8,
    ]);
    hash.update(&evidence.staged_prover_release_kat_digest);
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
#[allow(
    clippy::too_many_arguments,
    reason = "fixed streaming transcript axes remain explicit to preserve authenticated read order"
)]
fn stream_ciphertext_component_into_hash_v1<P>(
    ciphertext: &ZkAmsMkheStreamingCollectiveCiphertextBindingV1<'_>,
    constant: bool,
    profile: &BgvProfile,
    provider: &mut P,
    hash: &mut Keccak256,
    snapshot: &mut StreamingSnapshotAccumulatorV1,
    limb: &mut ZeroizingStagedU64VectorV1,
    scratch: &mut [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    if limb.as_slice().len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    hash.update(
        &u32::try_from(coefficient_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for limb_index in 0..profile.moduli.len() {
        let receipt = if constant {
            ciphertext.read_constant_limb_into_v1(
                limb_index,
                profile,
                provider,
                limb.as_mut_slice(),
                scratch,
            )?
        } else {
            ciphertext.read_linear_limb_into_v1(
                limb_index,
                profile,
                provider,
                limb.as_mut_slice(),
                scratch,
            )?
        };
        snapshot.observe(&receipt)?;
        update_residue_limb(hash, limb.as_slice());
    }
    Ok(())
}
fn hash_streaming_ciphertext_components_v1<P>(
    ciphertext: &ZkAmsMkheStreamingCollectiveCiphertextBindingV1<'_>,
    profile: &BgvProfile,
    provider: &mut P,
    hash: &mut Keccak256,
    snapshot: &mut StreamingSnapshotAccumulatorV1,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let mut limb = ZeroizingStagedU64VectorV1::new_zeroed(profile.ring_degree)?;
    let mut scratch = ZeroizingStagedBytesV1::<ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1>::zeroed();
    stream_ciphertext_component_into_hash_v1(
        ciphertext,
        true,
        profile,
        provider,
        hash,
        snapshot,
        &mut limb,
        scratch.as_mut_array(),
    )?;
    stream_ciphertext_component_into_hash_v1(
        ciphertext,
        false,
        profile,
        provider,
        hash,
        snapshot,
        &mut limb,
        scratch.as_mut_array(),
    )
}
/// Key-lineage axes checked only at live manifest admission. They deliberately do not enter the
/// legacy statement/ZDSM transcript, whose canonical bytes remain unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DecryptionCiphertextKeyLineageV1 {
    key_material_digest: [u8; 32],
    key_transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
}
impl DecryptionCiphertextKeyLineageV1 {
    fn validate_expected_v1(self, expected: Self) -> Result<(), ZkAmsMkheErrorV1> {
        if self.key_material_digest == [0; 32]
            || self.key_transcript_digest == [0; 32]
            || self.collective_key_digest == [0; 32]
            || expected.key_material_digest == [0; 32]
            || expected.key_transcript_digest == [0; 32]
            || expected.collective_key_digest == [0; 32]
            || self != expected
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(())
    }
}
fn validate_streaming_ciphertext_live_v1<P>(
    roster: &ZkAmsMkheGovernedRosterWireV1,
    ciphertext: &ZkAmsMkheStreamingCollectiveCiphertextV1,
    ciphertext_record_index: u32,
    expected_key_material_digest: [u8; 32],
    expected_key_transcript_digest: [u8; 32],
    expected_collective_key_digest: [u8; 32],
    provider: &mut P,
) -> Result<
    (
        DecryptionCiphertextAxesV1,
        ZkAmsMkheDecryptionStreamingSnapshotV1,
    ),
    ZkAmsMkheErrorV1,
>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let profile = release_profile_v1();
    profile.validate()?;
    let binding = ciphertext.sealed_binding_v1()?;
    let maximum_samples = zk_ams_mkhe_release_manifest_v1()?.max_samples_per_secret_epoch;
    if u64::from(ciphertext_record_index) >= maximum_samples
        || binding.sample_index() >= maximum_samples
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    DecryptionCiphertextKeyLineageV1 {
        key_material_digest: binding.key_material_digest(),
        key_transcript_digest: binding.key_transcript_digest(),
        collective_key_digest: binding.key_digest(),
    }
    .validate_expected_v1(DecryptionCiphertextKeyLineageV1 {
        key_material_digest: expected_key_material_digest,
        key_transcript_digest: expected_key_transcript_digest,
        collective_key_digest: expected_collective_key_digest,
    })?;
    if binding.profile_digest() != profile.digest()?
        || binding.profile_digest() != roster.profile_digest()
        || binding.roster_digest() != roster.roster_digest()
        || binding.epoch() != roster.epoch()
        || binding.security_certificate_digest()
            != zk_ams_mkhe_security_certificate_v1()?.certificate_digest()
        || binding.level() != 0
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let mut digest = Keccak256::new();
    digest.update(COLLECTIVE_CIPHERTEXT_DOMAIN_V1);
    digest.update(&binding.profile_digest());
    digest.update(&binding.roster_digest());
    digest.update(&binding.epoch().to_be_bytes());
    digest.update(&binding.transcript_digest());
    digest.update(&binding.sample_index().to_be_bytes());
    digest.update(&[binding.level()]);
    let mut snapshot = StreamingSnapshotAccumulatorV1::new();
    hash_streaming_ciphertext_components_v1(
        &binding,
        &profile,
        provider,
        &mut digest,
        &mut snapshot,
    )?;
    if digest.finalize() != binding.ciphertext_digest() {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let axes = DecryptionCiphertextAxesV1::new_v1(
        roster,
        binding.profile_digest(),
        binding.roster_digest(),
        binding.epoch(),
        binding.transcript_digest(),
        binding.ciphertext_digest(),
        ciphertext_record_index,
        binding.sample_index(),
        binding.level(),
    )?;
    Ok((axes, snapshot.finish()?))
}
/// Compact, context-minted release statement for the bounded verifier.
///
/// The value retains only roster, ciphertext, compact bindings, and content addresses. In
/// particular, it does not retain the aggregate key or any of the eight full public-key shares. Its
/// production constructor consumes the move-only bounded CPK ceremony authority; no native
/// aggregate-key/share bridge is exposed in production or included in verifier residency evidence.
/// The persistent context remains borrowed for the statement's lifetime; there is no raw-digest,
/// raw-pointer, codec, or decoder constructor.
pub struct ZkAmsMkheStreamingDecryptionStatementV1<'a> {
    roster: &'a ZkAmsMkheGovernedRosterWireV1,
    ciphertext: &'a ZkAmsMkheStreamingCollectiveCiphertextV1,
    ciphertext_axes: DecryptionCiphertextAxesV1,
    ciphertext_snapshot: ZkAmsMkheDecryptionStreamingSnapshotV1,
    persistent_context: &'a ZkAmsMkhePersistentDecryptionVerificationContextV1,
    parties: PartySet,
    party_bindings: [DecryptionBindingV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    party_b_pointers: [ZkAmsMkheDirectObjectPointerV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    key_context_digest: [u8; 32],
}
impl fmt::Debug for ZkAmsMkheStreamingDecryptionStatementV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheStreamingDecryptionStatementV1")
            .field("roster_digest", &hex::encode(self.roster.roster_digest()))
            .field(
                "ciphertext_digest",
                &hex::encode(self.ciphertext_axes.ciphertext_digest()),
            )
            .field(
                "ciphertext_record_index",
                &self.ciphertext_axes.ciphertext_record_index(),
            )
            .field("key_context_digest", &hex::encode(self.key_context_digest))
            .finish_non_exhaustive()
    }
}
impl<'a> ZkAmsMkheStreamingDecryptionStatementV1<'a> {
    /// Mint a compact statement from the exact bounded CPK ceremony authority.
    ///
    /// The one-shot authority is consumed here. Every pointer and key-context axis comes from the
    /// retained persistent context; callers supply only the canonical roster/ciphertext objects to
    /// which fresh party bindings are minted. Release admission requires one unified immutable
    /// provider snapshot serving both the staged CPK party-B objects and every C0/C1 output limb;
    /// an output published to a separate CAS is not admissible until it is represented in that
    /// unified snapshot.
    pub fn from_verified_cpk_authority_v1<P>(
        roster: &'a ZkAmsMkheGovernedRosterWireV1,
        ciphertext: &'a ZkAmsMkheStreamingCollectiveCiphertextV1,
        ciphertext_record_index: u32,
        persistent_context: &'a ZkAmsMkhePersistentDecryptionVerificationContextV1,
        authority: ZkAmsMkheStreamingDecryptionAuthorityV1,
        provider: &mut P,
    ) -> Result<Self, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        let profile = release_profile_v1();
        let mut parties = try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        parties.extend_from_slice(roster.parties());
        let parties = PartySet::new(parties)?;
        let (expected_roster, expected_key_transcript_digest) =
            persistent_context.streaming_public_axes_v1();
        let expected_collective_key_digest =
            persistent_context.streaming_collective_key_digest_v1();
        let (ciphertext_axes, ciphertext_snapshot) = validate_streaming_ciphertext_live_v1(
            roster,
            ciphertext,
            ciphertext_record_index,
            expected_roster.key_material_digest(),
            expected_key_transcript_digest,
            expected_collective_key_digest,
            provider,
        )?;
        let material = persistent_context.consume_streaming_authority_v1(
            roster,
            ciphertext_axes,
            ciphertext_snapshot.provider_identity(),
            ciphertext_snapshot.snapshot_identity(),
            authority,
        )?;
        let (party_b_pointers, proof_bindings, ciphertext_digest, key_context_digest) =
            material.into_parts();
        if ciphertext_digest != ciphertext_axes.ciphertext_digest() {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let mut party_bindings = try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for (party_index, persistent) in proof_bindings.into_iter().enumerate() {
            let binding = decryption_binding_from_compact_axes_v1(
                roster,
                ciphertext_axes,
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
            ciphertext_axes,
            ciphertext_snapshot,
            persistent_context,
            parties,
            party_bindings: party_bindings
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
            party_b_pointers,
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
    /// Exact direct-object ciphertext manifest retained by the compact statement.
    #[must_use]
    pub const fn ciphertext(&self) -> &'a ZkAmsMkheStreamingCollectiveCiphertextV1 {
        self.ciphertext
    }
    /// Native ciphertext digest bound into all eight signed manifests.
    #[must_use]
    pub const fn ciphertext_digest(&self) -> [u8; 32] {
        self.ciphertext_axes.ciphertext_digest()
    }
    /// Explicit release ciphertext-record identifier. It is not a plaintext
    /// packing chunk index and is bound separately from the sample index.
    #[must_use]
    pub const fn ciphertext_record_index(&self) -> u32 {
        self.ciphertext_axes.ciphertext_record_index()
    }
    /// Provider session and immutable snapshot authenticated at statement mint.
    #[must_use]
    pub const fn ciphertext_snapshot(&self) -> ZkAmsMkheDecryptionStreamingSnapshotV1 {
        self.ciphertext_snapshot
    }
    /// Mint the exact ordered move-only party-use set for bounded staged proving.
    ///
    /// No caller-supplied digest or pointer enters this operation. Each use is
    /// derived from the retained complete-CPK authority and is consumed by one
    /// [`prove_zk_ams_mkhe_decryption_share_staged_v1`] invocation. Calling
    /// this method again remints the same statement-bound set; persistent
    /// same-statement replay admission remains an external responsibility.
    pub fn bind_party_uses_v1(
        &self,
    ) -> Result<
        [ZkAmsMkhePersistentDecryptionPartyUseV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        ZkAmsMkheErrorV1,
    > {
        self.validate_compact()?;
        self.persistent_context
            .bind_streaming_statement_party_uses_v1(
                self.roster,
                self.ciphertext_axes,
                self.ciphertext_snapshot.provider_identity(),
                self.ciphertext_snapshot.snapshot_identity(),
                self.key_context_digest,
            )
    }
    fn consume_party_use_v1(
        &self,
        party_index: usize,
        party_use: ZkAmsMkhePersistentDecryptionPartyUseV1,
        party_state: &ZkAmsMkheCollectivePartyStateV1,
    ) -> Result<DecryptionBindingV1, ZkAmsMkheErrorV1> {
        self.validate_compact()?;
        let persistent = self.persistent_context.consume_streaming_party_use_v1(
            self.roster,
            self.ciphertext_axes,
            self.ciphertext_snapshot.provider_identity(),
            self.ciphertext_snapshot.snapshot_identity(),
            self.key_context_digest,
            party_index,
            party_use,
            party_state,
        )?;
        let binding = decryption_binding_from_compact_axes_v1(
            self.roster,
            self.ciphertext_axes,
            self.key_context_digest,
            party_index,
            &persistent,
        )?;
        if self.party_bindings.get(party_index) != Some(&binding) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(binding)
    }
    fn validate_compact(&self) -> Result<(), ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let ciphertext = self.ciphertext.sealed_binding_v1()?;
        self.ciphertext_axes.validate_for_roster_v1(self.roster)?;
        if self.parties.parties.as_slice() != self.roster.parties()
            || self.key_context_digest == [0; 32]
            || self.ciphertext_snapshot.provider_identity == [0; 32]
            || self.ciphertext_snapshot.snapshot_identity == [0; 32]
            || ciphertext.profile_digest() != self.ciphertext_axes.profile_digest()
            || ciphertext.roster_digest() != self.ciphertext_axes.roster_digest()
            || ciphertext.epoch() != self.ciphertext_axes.epoch()
            || ciphertext.transcript_digest() != self.ciphertext_axes.transcript_digest()
            || ciphertext.ciphertext_digest() != self.ciphertext_axes.ciphertext_digest()
            || ciphertext.sample_index() != self.ciphertext_axes.sample_index()
            || ciphertext.level() != self.ciphertext_axes.level()
        {
            return Err(ZkAmsMkheErrorV1::InvalidShareSet);
        }
        let (active_roster, cpk_transcript_digest) =
            self.persistent_context.streaming_public_axes_v1();
        if active_roster.to_wire_roster()? != *self.roster
            || ciphertext.security_certificate_digest()
                != zk_ams_mkhe_security_certificate_v1()?.certificate_digest()
            || ciphertext.key_material_digest() != active_roster.key_material_digest()
            || ciphertext.key_transcript_digest() != cpk_transcript_digest
            || ciphertext.key_digest()
                != self.persistent_context.streaming_collective_key_digest_v1()
        {
            return Err(ZkAmsMkheErrorV1::InvalidShareSet);
        }
        self.persistent_context
            .validate_streaming_statement_axes_if_present_v1(
                self.roster,
                self.ciphertext_axes,
                self.ciphertext_snapshot.provider_identity(),
                self.ciphertext_snapshot.snapshot_identity(),
                self.key_context_digest,
                self.party_b_pointers,
            )?;
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            self.party_bindings[party_index].validate(&profile, &self.parties)?;
            if usize::from(self.party_bindings[party_index].party_index) != party_index
                || self.party_bindings[party_index].party != self.parties.parties[party_index]
                || self.party_bindings[party_index].ciphertext_digest
                    != self.ciphertext_axes.ciphertext_digest()
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
    fn prepare_common_a_context(
        &self,
        profile: &BgvProfile,
    ) -> Result<ZkAmsMkhePreparedCollectivePublicAContextV1, ZkAmsMkheErrorV1> {
        self.validate_compact()?;
        let (roster, cpk_transcript_digest) = self.persistent_context.streaming_public_axes_v1();
        prepare_active_collective_public_a_v1(profile, roster, cpk_transcript_digest)
            .map_err(map_common_a_derivation_error_v1)
    }
}
fn map_common_a_derivation_error_v1(error: ZkAmsMkheCpkRelationErrorV1) -> ZkAmsMkheErrorV1 {
    match error {
        ZkAmsMkheCpkRelationErrorV1::ResourceCeiling => ZkAmsMkheErrorV1::ResourceCeilingExceeded,
        _ => ZkAmsMkheErrorV1::InvalidKeyMaterial,
    }
}
fn derive_prepared_common_a_limb_v1(
    context: &ZkAmsMkhePreparedCollectivePublicAContextV1,
    limb: usize,
    remaining_candidates: &mut u64,
) -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
    context
        .derive_limb_budgeted_v1(limb, remaining_candidates)
        .map_err(map_common_a_derivation_error_v1)
}
const STAGED_DECRYPTION_RELATION_ADMISSION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.staged-decryption-relation-admission";
struct StagedDecryptionRelationAdmissionSealV1;
/// Move-only canonical staged output for one semantically verified party share.
///
/// Both large components have been sealed, content-addressed, atomically published, fully reread,
/// and replayed through the exact streaming response equations before the `ZDSM` manifest is
/// authenticated. The type has no decoder, raw-pointer constructor, `Clone`, or `Copy`
/// implementation. CAS storage/cache residency remains outside the source-level bound and requires
/// the separate authenticated worker certificate.
pub struct ZkAmsMkheStagedDecryptionShareV1 {
    _seal: StagedDecryptionRelationAdmissionSealV1,
    party_index: u8,
    manifest_bytes: [u8; ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1],
    polynomial_publication: ZkAmsMkheDirectObjectPublicationReceiptV1,
    proof_publication: ZkAmsMkheDirectObjectPublicationReceiptV1,
    snapshot: ZkAmsMkheDecryptionStreamingSnapshotV1,
    semantic_verification_digest: [u8; 32],
}
impl fmt::Debug for ZkAmsMkheStagedDecryptionShareV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheStagedDecryptionShareV1")
            .field("party_index", &self.party_index)
            .field("polynomial_pointer", &self.polynomial_publication.pointer())
            .field("proof_pointer", &self.proof_publication.pointer())
            .field(
                "semantic_verification_digest",
                &hex::encode(self.semantic_verification_digest),
            )
            .finish_non_exhaustive()
    }
}
impl ZkAmsMkheStagedDecryptionShareV1 {
    /// Exact governed roster slot represented by the signed manifest.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.party_index
    }
    /// Exact canonical signed `ZDSM` bytes consumed by the streaming verifier.
    #[must_use]
    pub const fn manifest_bytes(&self) -> &[u8; ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1] {
        &self.manifest_bytes
    }
    /// Published, sealed, and independently reread share-polynomial receipt.
    #[must_use]
    pub const fn polynomial_publication(&self) -> &ZkAmsMkheDirectObjectPublicationReceiptV1 {
        &self.polynomial_publication
    }
    /// Published, sealed, and independently reread proof-object receipt.
    #[must_use]
    pub const fn proof_publication(&self) -> &ZkAmsMkheDirectObjectPublicationReceiptV1 {
        &self.proof_publication
    }
    /// Exact immutable provider revision used by every proving and replay read.
    #[must_use]
    pub const fn snapshot(&self) -> ZkAmsMkheDecryptionStreamingSnapshotV1 {
        self.snapshot
    }
    /// Digest minted only after exact semantic response-equation replay.
    #[must_use]
    pub const fn semantic_verification_digest(&self) -> [u8; 32] {
        self.semantic_verification_digest
    }
}
/// Secret-derived limb storage erased on success, error, and unwind.
struct ZeroizingStagedU64VectorV1(Vec<u64>);
impl ZeroizingStagedU64VectorV1 {
    fn new_zeroed(length: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        values.resize(length, 0);
        if values.capacity() != length {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        Ok(Self(values))
    }
    fn with_capacity(capacity: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if values.capacity() != capacity {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        Ok(Self(values))
    }
    fn push(&mut self, value: u64) {
        self.0.push(value);
    }
    fn as_slice(&self) -> &[u64] {
        &self.0
    }
    fn as_mut_slice(&mut self) -> &mut [u64] {
        &mut self.0
    }
}
impl Drop for ZeroizingStagedU64VectorV1 {
    fn drop(&mut self) {
        super::clear_secret_u64_slice_v1(&mut self.0);
        #[cfg(test)]
        super::record_decryption_transient_zeroized_drop_v1(self.0.iter().all(|value| *value == 0));
    }
}
/// Full decrypted aggregate storage erased on every verifier exit path.
///
/// This retains exactly one `P`-byte RNS owner. It deliberately exposes only
/// borrowed coefficient slices and a borrowed decoder view, so an ordinary
/// full-polynomial owner cannot escape before `Drop` clears every residue.
struct ZeroizingAggregateRnsV1(RnsPolynomial);
impl ZeroizingAggregateRnsV1 {
    fn zero_exact_v1(profile: &BgvProfile) -> Result<Self, ZkAmsMkheErrorV1> {
        profile.validate()?;
        let coefficient_count = profile
            .ring_degree
            .checked_mul(profile.moduli.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut coefficients = Vec::new();
        coefficients
            .try_reserve_exact(coefficient_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        coefficients.resize(coefficient_count, 0);
        if coefficients.capacity() != coefficient_count {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        Ok(Self(RnsPolynomial { coefficients }))
    }
    fn coefficients_mut(&mut self) -> &mut [u64] {
        &mut self.0.coefficients
    }
    fn as_rns(&self) -> &RnsPolynomial {
        &self.0
    }
}
impl Drop for ZeroizingAggregateRnsV1 {
    fn drop(&mut self) {
        super::clear_secret_u64_slice_v1(&mut self.0.coefficients);
        #[cfg(test)]
        super::record_decryption_transient_zeroized_drop_v1(
            self.0.coefficients.iter().all(|value| *value == 0),
        );
    }
}
/// Fixed staging/encoding scratch erased on every exit path.
struct ZeroizingStagedBytesV1<const N: usize>([u8; N]);
impl<const N: usize> ZeroizingStagedBytesV1<N> {
    const fn zeroed() -> Self {
        Self([0; N])
    }
    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.0
    }
    fn as_mut_array(&mut self) -> &mut [u8; N] {
        &mut self.0
    }
}
impl<const N: usize> Drop for ZeroizingStagedBytesV1<N> {
    fn drop(&mut self) {
        super::clear_secret_bytes_v1(&mut self.0);
        #[cfg(test)]
        super::record_decryption_transient_zeroized_drop_v1(self.0.iter().all(|value| *value == 0));
    }
}
/// Fallibly allocated exact-length owner for a complete authenticated proof. It is move-only and
/// erases its initialized capacity on success, error, and unwind before the allocation is released.
struct ZeroizingStagedByteVectorV1(Vec<u8>);
impl ZeroizingStagedByteVectorV1 {
    fn new_zeroed_exact(length: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        bytes.resize(length, 0);
        if bytes.capacity() != length {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        Ok(Self(bytes))
    }
    fn as_slice(&self) -> &[u8] {
        &self.0
    }
    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.0
    }
}
impl Drop for ZeroizingStagedByteVectorV1 {
    fn drop(&mut self) {
        super::clear_secret_bytes_v1(self.0.as_mut_slice());
        #[cfg(test)]
        super::record_decryption_transient_zeroized_drop_v1(self.0.iter().all(|value| *value == 0));
    }
}
fn bit_reverse_permute_staged_v1(values: &mut [u64]) {
    let mut target = 0_usize;
    for index in 1..values.len() {
        let mut bit = values.len() >> 1;
        while target & bit != 0 {
            target ^= bit;
            bit >>= 1;
        }
        target ^= bit;
        if index < target {
            values.swap(index, target);
        }
    }
}
fn cyclic_ntt_staged_v1(values: &mut [u64], root: u64, modulus: u64) {
    bit_reverse_permute_staged_v1(values);
    let mut width = 2;
    while width <= values.len() {
        let twiddle_step = mod_pow(root, (values.len() / width) as u64, modulus);
        for block in values.chunks_exact_mut(width) {
            let mut twiddle = 1_u64;
            for offset in 0..width / 2 {
                let even = block[offset];
                let odd = mod_mul(block[offset + width / 2], twiddle, modulus);
                block[offset] = mod_add(even, odd, modulus);
                block[offset + width / 2] = mod_sub(even, odd, modulus);
                twiddle = mod_mul(twiddle, twiddle_step, modulus);
            }
        }
        width <<= 1;
    }
}
fn inverse_cyclic_ntt_staged_v1(
    values: &mut [u64],
    root: u64,
    modulus: u64,
) -> Result<(), ZkAmsMkheErrorV1> {
    let inverse_root = mod_inverse(root, modulus).ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    cyclic_ntt_staged_v1(values, inverse_root, modulus);
    let inverse_degree =
        mod_inverse(values.len() as u64, modulus).ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    for value in values {
        *value = mod_mul(*value, inverse_degree, modulus);
    }
    Ok(())
}
fn negacyclic_multiply_staged_v1(
    left: &[u64],
    right: &[u64],
    modulus: u64,
    psi: u64,
) -> Result<ZeroizingStagedU64VectorV1, ZkAmsMkheErrorV1> {
    if left.len() != right.len()
        || left.is_empty()
        || !left.len().is_power_of_two()
        || left.iter().chain(right).any(|value| *value >= modulus)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut left_twisted = ZeroizingStagedU64VectorV1::with_capacity(left.len())?;
    let mut right_twisted = ZeroizingStagedU64VectorV1::with_capacity(right.len())?;
    let mut twist = 1_u64;
    for (&left, &right) in left.iter().zip(right) {
        left_twisted.push(mod_mul(left, twist, modulus));
        right_twisted.push(mod_mul(right, twist, modulus));
        twist = mod_mul(twist, psi, modulus);
    }
    let root = mod_mul(psi, psi, modulus);
    cyclic_ntt_staged_v1(left_twisted.as_mut_slice(), root, modulus);
    cyclic_ntt_staged_v1(right_twisted.as_mut_slice(), root, modulus);
    for (left, right) in left_twisted
        .as_mut_slice()
        .iter_mut()
        .zip(right_twisted.as_slice().iter().copied())
    {
        *left = mod_mul(*left, right, modulus);
    }
    inverse_cyclic_ntt_staged_v1(left_twisted.as_mut_slice(), root, modulus)?;
    let inverse_psi = mod_inverse(psi, modulus).ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut untwist = 1_u64;
    for value in left_twisted.as_mut_slice() {
        *value = mod_mul(*value, untwist, modulus);
        untwist = mod_mul(untwist, inverse_psi, modulus);
    }
    Ok(left_twisted)
}
fn sparse_challenge_terms_v1(
    challenge: &[i8],
) -> Result<Vec<SparseChallengeTermV1>, ZkAmsMkheErrorV1> {
    let expected = wide_relation_challenge_weight(challenge.len())?;
    let mut terms = try_exact_capacity_vec_v1(expected)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for (shift, sign) in challenge.iter().copied().enumerate() {
        if ![-1, 0, 1].contains(&sign) {
            return Err(ZkAmsMkheErrorV1::InvalidShareProof);
        }
        if sign != 0 {
            terms.push(SparseChallengeTermV1 { shift, sign });
        }
    }
    if terms.len() != expected {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    Ok(terms)
}
fn sparse_fold_small_coefficient_v1(
    terms: &[SparseChallengeTermV1],
    witness: &[i64],
    destination: usize,
) -> Result<i64, ZkAmsMkheErrorV1> {
    if witness.is_empty() || destination >= witness.len() || !witness.len().is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    let mut output = 0_i64;
    for term in terms {
        if term.shift >= witness.len() || ![-1, 1].contains(&term.sign) {
            return Err(ZkAmsMkheErrorV1::InvalidShareProof);
        }
        let (source, wrap_sign) = if destination >= term.shift {
            (destination - term.shift, 1_i64)
        } else {
            (witness.len() + destination - term.shift, -1_i64)
        };
        let value = witness[source]
            .checked_mul(i64::from(term.sign))
            .and_then(|value| value.checked_mul(wrap_sign))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        output = output
            .checked_add(value)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    }
    Ok(output)
}
fn sparse_fold_wide_coefficient_v1(
    terms: &[SparseChallengeTermV1],
    witness: &[SignedWideV1],
    destination: usize,
) -> Result<SignedWideV1, ZkAmsMkheErrorV1> {
    if witness.is_empty() || destination >= witness.len() || !witness.len().is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    let mut output = SignedWideV1::zero();
    for term in terms {
        if term.shift >= witness.len() || ![-1, 1].contains(&term.sign) {
            return Err(ZkAmsMkheErrorV1::InvalidShareProof);
        }
        let (source, wrap_sign) = if destination >= term.shift {
            (destination - term.shift, 1_i8)
        } else {
            (witness.len() + destination - term.shift, -1_i8)
        };
        let coefficient = &witness[source];
        let signed = if term.sign * wrap_sign < 0 {
            coefficient.negated()
        } else {
            coefficient.clone()
        };
        output = output
            .checked_add(&signed)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    }
    Ok(output)
}
fn encode_signed_wide_fixed_into_v1(
    value: &SignedWideV1,
    output: &mut [u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    if output.is_empty()
        || output.len() > DECRYPTION_MAX_WIDE_LIMBS_V1 * size_of::<u64>()
        || value.magnitude.bit_len() > output.len() * 8 - 1
        || (value.negative && value.magnitude.is_zero())
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    output.fill(0);
    for (index, destination) in output.iter_mut().rev().enumerate() {
        let limb = index / 8;
        let shift = (index % 8) * 8;
        *destination = (value.magnitude.limbs[limb] >> shift) as u8;
    }
    if output[0] & 0x80 != 0 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    if value.negative {
        output[0] |= 0x80;
    }
    Ok(())
}
fn write_residue_limb_staged_v1<P>(
    transaction: &mut ZkAmsMkheDirectObjectPublicationTransactionV1<'_, P>,
    residues: &[u64],
    modulus: u64,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    if residues.is_empty() || residues.iter().any(|residue| *residue >= modulus) {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut buffer = ZeroizingStagedBytesV1::<ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1>::zeroed();
    for chunk in residues.chunks(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / size_of::<u64>()) {
        let bytes = chunk
            .len()
            .checked_mul(size_of::<u64>())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for (encoded, residue) in buffer.as_mut_slice()[..bytes]
            .chunks_exact_mut(size_of::<u64>())
            .zip(chunk)
        {
            encoded.copy_from_slice(&residue.to_be_bytes());
        }
        transaction.write_exact(&buffer.as_mut_slice()[..bytes])?;
    }
    Ok(())
}
fn publish_staged_share_polynomial_v1<P>(
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
    profile: &BgvProfile,
    party_state: &ZkAmsMkheCollectivePartyStateV1,
    smudge: &[SignedWideV1],
    publisher: &mut P,
    snapshot: &mut StreamingSnapshotAccumulatorV1,
) -> Result<ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    let secret = &party_state.secret().coefficients;
    if secret.len() != profile.ring_degree
        || smudge.len() != profile.ring_degree
        || secret
            .iter()
            .any(|coefficient| coefficient.unsigned_abs() > 1)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let evidence = derive_decryption_resource_evidence(profile)?;
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut transaction = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
        ZkAmsMkheDirectObjectKindV1::DecryptionSharePolynomial,
        evidence.split_polynomial_object_bytes,
        publisher,
    )?;
    transaction.write_exact(
        &u32::try_from(coefficient_count)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?
            .to_be_bytes(),
    )?;
    let ciphertext = statement.ciphertext.sealed_binding_v1()?;
    let mut linear = ZeroizingStagedU64VectorV1::new_zeroed(profile.ring_degree)?;
    let mut scratch = ZeroizingStagedBytesV1::<ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1>::zeroed();
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let mut secret_limb = ZeroizingStagedU64VectorV1::with_capacity(profile.ring_degree)?;
        for coefficient in secret {
            secret_limb.push(signed_mod(*coefficient, modulus));
        }
        let linear_receipt = transaction.read_immutable_provider(|publisher| {
            ciphertext.read_linear_limb_into_v1(
                limb,
                profile,
                publisher,
                linear.as_mut_slice(),
                scratch.as_mut_array(),
            )
        })?;
        snapshot.observe(&linear_receipt)?;
        let mut share = negacyclic_multiply_staged_v1(
            linear.as_slice(),
            secret_limb.as_slice(),
            modulus,
            profile.negacyclic_roots[limb],
        )?;
        drop(secret_limb);
        let plaintext_modulus = profile.plaintext_modulus.residue(modulus);
        for (coefficient, smudge) in share.as_mut_slice().iter_mut().zip(smudge) {
            *coefficient = mod_add(
                *coefficient,
                mod_mul(plaintext_modulus, smudge.mod_u64(modulus), modulus),
                modulus,
            );
        }
        write_residue_limb_staged_v1(&mut transaction, share.as_slice(), modulus)?;
    }
    if transaction.remaining_bytes() != 0 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    transaction.finish()
}
fn hash_staged_public_key_mask_commitment_v1(
    common_a_context: &ZkAmsMkhePreparedCollectivePublicAContextV1,
    profile: &BgvProfile,
    secret_mask: &[i64],
    error_mask: &[i64],
    transcript: &mut Keccak256,
    remaining_common_a_candidates: &mut u64,
) -> Result<(), ZkAmsMkheErrorV1> {
    if secret_mask.len() != profile.ring_degree || error_mask.len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    update_streamed_rns_header(transcript, profile)?;
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let common_a = derive_prepared_common_a_limb_v1(
            common_a_context,
            limb,
            remaining_common_a_candidates,
        )?;
        let mut secret_mask_limb = ZeroizingStagedU64VectorV1::with_capacity(profile.ring_degree)?;
        for coefficient in secret_mask {
            secret_mask_limb.push(signed_mod(*coefficient, modulus));
        }
        let mut commitment = negacyclic_multiply_staged_v1(
            &common_a,
            secret_mask_limb.as_slice(),
            modulus,
            profile.negacyclic_roots[limb],
        )?;
        drop(common_a);
        drop(secret_mask_limb);
        let plaintext_modulus = profile.plaintext_modulus.residue(modulus);
        for (coefficient, error) in commitment.as_mut_slice().iter_mut().zip(error_mask) {
            *coefficient = mod_add(
                mod_sub(0, *coefficient, modulus),
                mod_mul(plaintext_modulus, signed_mod(*error, modulus), modulus),
                modulus,
            );
        }
        update_residue_limb(transcript, commitment.as_slice());
    }
    Ok(())
}
fn hash_staged_share_mask_commitment_v1<P>(
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
    profile: &BgvProfile,
    secret_mask: &[i64],
    smudge_mask: &[SignedWideV1],
    transcript: &mut Keccak256,
    provider: &mut P,
    snapshot: &mut StreamingSnapshotAccumulatorV1,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    if secret_mask.len() != profile.ring_degree || smudge_mask.len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    update_streamed_rns_header(transcript, profile)?;
    let ciphertext = statement.ciphertext.sealed_binding_v1()?;
    let mut linear = ZeroizingStagedU64VectorV1::new_zeroed(profile.ring_degree)?;
    let mut scratch = ZeroizingStagedBytesV1::<ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1>::zeroed();
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let mut secret_mask_limb = ZeroizingStagedU64VectorV1::with_capacity(profile.ring_degree)?;
        for coefficient in secret_mask {
            secret_mask_limb.push(signed_mod(*coefficient, modulus));
        }
        let receipt = ciphertext.read_linear_limb_into_v1(
            limb,
            profile,
            provider,
            linear.as_mut_slice(),
            scratch.as_mut_array(),
        )?;
        snapshot.observe(&receipt)?;
        let mut commitment = negacyclic_multiply_staged_v1(
            linear.as_slice(),
            secret_mask_limb.as_slice(),
            modulus,
            profile.negacyclic_roots[limb],
        )?;
        drop(secret_mask_limb);
        let plaintext_modulus = profile.plaintext_modulus.residue(modulus);
        for (coefficient, smudge) in commitment.as_mut_slice().iter_mut().zip(smudge_mask) {
            *coefficient = mod_add(
                *coefficient,
                mod_mul(plaintext_modulus, smudge.mod_u64(modulus), modulus),
                modulus,
            );
        }
        update_residue_limb(transcript, commitment.as_slice());
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn build_staged_decryption_transcript_prefix_v1<P>(
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
    common_a_context: &ZkAmsMkhePreparedCollectivePublicAContextV1,
    profile: &BgvProfile,
    smudge_bits: usize,
    party_index: usize,
    binding: &DecryptionBindingV1,
    share_pointer: ZkAmsMkheDirectObjectPointerV1,
    publisher: &mut P,
    snapshot: &mut StreamingSnapshotAccumulatorV1,
    remaining_common_a_candidates: &mut u64,
) -> Result<Keccak256, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    let mut transcript = initialize_decryption_challenge_transcript(profile, smudge_bits, binding)?;
    update_streamed_rns_header(&mut transcript, profile)?;
    for limb in 0..profile.moduli.len() {
        let common_a = derive_prepared_common_a_limb_v1(
            common_a_context,
            limb,
            remaining_common_a_candidates,
        )?;
        update_residue_limb(&mut transcript, &common_a);
    }
    let party_b_receipt = hash_streamed_polynomial_v1(
        ZkAmsMkheDirectObjectKindV1::CpkPartyB,
        statement.party_b_pointers[party_index],
        profile,
        publisher,
        &mut transcript,
    )?;
    snapshot.observe(&party_b_receipt)?;
    let ciphertext = statement.ciphertext.sealed_binding_v1()?;
    hash_streaming_ciphertext_components_v1(
        &ciphertext,
        profile,
        publisher,
        &mut transcript,
        snapshot,
    )?;
    let share_receipt = hash_streamed_polynomial_v1(
        ZkAmsMkheDirectObjectKindV1::DecryptionSharePolynomial,
        share_pointer,
        profile,
        publisher,
        &mut transcript,
    )?;
    snapshot.observe(&share_receipt)?;
    Ok(transcript)
}
#[allow(clippy::too_many_arguments)]
fn derive_staged_decryption_challenge_seed_v1<P>(
    prefix: &Keccak256,
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
    common_a_context: &ZkAmsMkhePreparedCollectivePublicAContextV1,
    profile: &BgvProfile,
    secret_mask: &[i64],
    error_mask: &[i64],
    smudge_mask: &[SignedWideV1],
    publisher: &mut P,
    snapshot: &mut StreamingSnapshotAccumulatorV1,
    remaining_common_a_candidates: &mut u64,
) -> Result<[u8; 32], ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    let mut transcript = prefix.fork_v1();
    hash_staged_public_key_mask_commitment_v1(
        common_a_context,
        profile,
        secret_mask,
        error_mask,
        &mut transcript,
        remaining_common_a_candidates,
    )?;
    hash_staged_share_mask_commitment_v1(
        statement,
        profile,
        secret_mask,
        smudge_mask,
        &mut transcript,
        publisher,
        snapshot,
    )?;
    Ok(transcript.finalize())
}
fn staged_small_response_v1(
    mask: i64,
    terms: &[SparseChallengeTermV1],
    witness: &[i64],
    index: usize,
) -> Result<i64, ZkAmsMkheErrorV1> {
    mask.checked_add(sparse_fold_small_coefficient_v1(terms, witness, index)?)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
fn staged_wide_response_v1(
    mask: &SignedWideV1,
    terms: &[SparseChallengeTermV1],
    witness: &[SignedWideV1],
    index: usize,
) -> Result<SignedWideV1, ZkAmsMkheErrorV1> {
    mask.checked_add(&sparse_fold_wide_coefficient_v1(terms, witness, index)?)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
#[allow(clippy::too_many_arguments)]
fn staged_responses_fit_v1(
    terms: &[SparseChallengeTermV1],
    witness_secret: &[i64],
    witness_error: &[i64],
    witness_smudge: &[SignedWideV1],
    secret_mask: &[i64],
    error_mask: &[i64],
    smudge_mask: &[SignedWideV1],
    secret_limit: i64,
    error_limit: i64,
    wide_limit: &super::WideMagnitudeV1,
) -> Result<bool, ZkAmsMkheErrorV1> {
    let degree = witness_secret.len();
    if degree == 0
        || witness_error.len() != degree
        || witness_smudge.len() != degree
        || secret_mask.len() != degree
        || error_mask.len() != degree
        || smudge_mask.len() != degree
    {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    for index in 0..degree {
        let secret = staged_small_response_v1(secret_mask[index], terms, witness_secret, index)?;
        let error = staged_small_response_v1(error_mask[index], terms, witness_error, index)?;
        let smudge = staged_wide_response_v1(&smudge_mask[index], terms, witness_smudge, index)?;
        if secret.unsigned_abs() > secret_limit as u64
            || error.unsigned_abs() > error_limit as u64
            || smudge.magnitude.cmp(wide_limit).is_gt()
        {
            return Ok(false);
        }
    }
    Ok(true)
}
fn staged_proof_header_v1(
    degree: usize,
    wide_response_bytes: usize,
    challenge_seed: [u8; 32],
) -> Result<[u8; DECRYPTION_PROOF_HEADER_BYTES_V1], ZkAmsMkheErrorV1> {
    if degree == 0 || challenge_seed == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    let mut header = [0_u8; DECRYPTION_PROOF_HEADER_BYTES_V1];
    let mut cursor = 0;
    header[cursor..cursor + 4].copy_from_slice(&DECRYPTION_PROOF_TAG_V1);
    cursor += 4;
    header[cursor] = MKHE_VERSION_V1;
    cursor += 1;
    header[cursor..cursor + 2].copy_from_slice(
        &u16::try_from(wide_response_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    cursor += 2;
    let degree = u32::try_from(degree).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    header[cursor..cursor + 4].copy_from_slice(&degree.to_be_bytes());
    cursor += 4;
    header[cursor..cursor + 32].copy_from_slice(&challenge_seed);
    cursor += 32;
    for _ in 0..3 {
        header[cursor..cursor + 4].copy_from_slice(&degree.to_be_bytes());
        cursor += 4;
    }
    if cursor != header.len() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(header)
}
fn write_staged_small_response_section_v1<P>(
    transaction: &mut ZkAmsMkheDirectObjectPublicationTransactionV1<'_, P>,
    terms: &[SparseChallengeTermV1],
    witness: &[i64],
    mask: &[i64],
    limit: i64,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    if witness.is_empty() || witness.len() != mask.len() {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    let coefficients_per_chunk =
        ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / DECRYPTION_SIGNED_SMALL_BYTES_V1;
    let mut buffer = ZeroizingStagedBytesV1::<ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1>::zeroed();
    for chunk_start in (0..witness.len()).step_by(coefficients_per_chunk) {
        let chunk_end = chunk_start
            .checked_add(coefficients_per_chunk)
            .unwrap_or(witness.len())
            .min(witness.len());
        let bytes = (chunk_end - chunk_start)
            .checked_mul(DECRYPTION_SIGNED_SMALL_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for (offset, index) in (chunk_start..chunk_end).enumerate() {
            let response = staged_small_response_v1(mask[index], terms, witness, index)?;
            if response.unsigned_abs() > limit as u64 {
                return Err(ZkAmsMkheErrorV1::InvalidShareProof);
            }
            let start = offset * DECRYPTION_SIGNED_SMALL_BYTES_V1;
            buffer.as_mut_slice()[start..start + DECRYPTION_SIGNED_SMALL_BYTES_V1]
                .copy_from_slice(&response.to_be_bytes());
        }
        transaction.write_exact(&buffer.as_mut_slice()[..bytes])?;
    }
    Ok(())
}
fn write_staged_wide_response_section_v1<P>(
    transaction: &mut ZkAmsMkheDirectObjectPublicationTransactionV1<'_, P>,
    terms: &[SparseChallengeTermV1],
    witness: &[SignedWideV1],
    mask: &[SignedWideV1],
    limit: &super::WideMagnitudeV1,
    wide_response_bytes: usize,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    if witness.is_empty()
        || witness.len() != mask.len()
        || wide_response_bytes == 0
        || wide_response_bytes > ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    let coefficients_per_chunk = ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / wide_response_bytes;
    if coefficients_per_chunk == 0 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let mut buffer = ZeroizingStagedBytesV1::<ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1>::zeroed();
    for chunk_start in (0..witness.len()).step_by(coefficients_per_chunk) {
        let chunk_end = chunk_start
            .checked_add(coefficients_per_chunk)
            .unwrap_or(witness.len())
            .min(witness.len());
        let bytes = (chunk_end - chunk_start)
            .checked_mul(wide_response_bytes)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for (offset, index) in (chunk_start..chunk_end).enumerate() {
            let response = staged_wide_response_v1(&mask[index], terms, witness, index)?;
            if response.magnitude.cmp(limit).is_gt() {
                return Err(ZkAmsMkheErrorV1::InvalidShareProof);
            }
            let start = offset * wide_response_bytes;
            encode_signed_wide_fixed_into_v1(
                &response,
                &mut buffer.as_mut_slice()[start..start + wide_response_bytes],
            )?;
        }
        transaction.write_exact(&buffer.as_mut_slice()[..bytes])?;
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn publish_staged_decryption_proof_v1<P>(
    profile: &BgvProfile,
    challenge_seed: [u8; 32],
    terms: &[SparseChallengeTermV1],
    witness_secret: &[i64],
    witness_error: &[i64],
    witness_smudge: &[SignedWideV1],
    secret_mask: &[i64],
    error_mask: &[i64],
    smudge_mask: &[SignedWideV1],
    secret_limit: i64,
    error_limit: i64,
    wide_limit: &super::WideMagnitudeV1,
    wide_response_bytes: usize,
    publisher: &mut P,
) -> Result<ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    let evidence = derive_decryption_resource_evidence(profile)?;
    let mut transaction = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        evidence.split_proof_envelope_bytes,
        publisher,
    )?;
    transaction.write_exact(&staged_proof_header_v1(
        profile.ring_degree,
        wide_response_bytes,
        challenge_seed,
    )?)?;
    write_staged_small_response_section_v1(
        &mut transaction,
        terms,
        witness_secret,
        secret_mask,
        secret_limit,
    )?;
    write_staged_small_response_section_v1(
        &mut transaction,
        terms,
        witness_error,
        error_mask,
        error_limit,
    )?;
    write_staged_wide_response_section_v1(
        &mut transaction,
        terms,
        witness_smudge,
        smudge_mask,
        wide_limit,
        wide_response_bytes,
    )?;
    if transaction.remaining_bytes() != 0 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    transaction.finish()
}
fn direct_to_decryption_transport_pointer_v1(
    pointer: ZkAmsMkheDirectObjectPointerV1,
    expected_direct_kind: ZkAmsMkheDirectObjectKindV1,
    transport_kind: ZkAmsMkheDecryptionTransportComponentKindV1,
) -> Result<ZkAmsMkheDecryptionTransportPointerV1, ZkAmsMkheErrorV1> {
    if pointer.kind() != expected_direct_kind {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let value = ZkAmsMkheDecryptionTransportPointerV1 {
        kind: transport_kind,
        payload_bytes: pointer.payload_bytes(),
        payload_blake3: pointer.payload_blake3(),
    };
    value.validate_shape()?;
    Ok(value)
}
/// Replay one published party relation through the same zero-copy readers,
/// transcript frames, and response equations used by the public consumer. The
/// public entry point requires exactly eight authenticated manifests before it
/// can aggregate/decode, so it cannot self-verify a lone staged output here.
#[allow(clippy::too_many_arguments)]
fn verify_published_staged_relation_v1<P>(
    prefix: &Keccak256,
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
    common_a_context: &ZkAmsMkhePreparedCollectivePublicAContextV1,
    profile: &BgvProfile,
    party_index: usize,
    share_pointer: ZkAmsMkheDirectObjectPointerV1,
    proof_pointer: ZkAmsMkheDirectObjectPointerV1,
    publisher: &mut P,
    snapshot: &mut StreamingSnapshotAccumulatorV1,
    remaining_common_a_candidates: &mut u64,
) -> Result<[u8; 32], ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    let (proof_bytes, proof_receipt) = read_complete_object_v1(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        proof_pointer,
        publisher,
    )?;
    snapshot.observe(&proof_receipt)?;
    let proof = ZkAmsMkheDecryptionProofViewV1::decode_release_exact(proof_bytes.as_slice())?;
    checked_ring_multiplication_work(profile, 8)?;
    let challenge = derive_sparse_challenge(profile.ring_degree, proof.challenge_seed())?;
    let mut transcript = prefix.fork_v1();
    let party_b_relation_receipt = reconstruct_public_key_commitment_v1(
        statement,
        profile,
        &proof,
        &challenge,
        party_index,
        publisher,
        &mut transcript,
        common_a_context,
        remaining_common_a_candidates,
    )?;
    snapshot.observe(&party_b_relation_receipt)?;
    let share_relation_receipt = reconstruct_share_commitment_and_aggregate_v1(
        statement,
        profile,
        &proof,
        &challenge,
        share_pointer,
        publisher,
        &mut transcript,
        None,
        snapshot,
    )?;
    snapshot.observe(&share_relation_receipt)?;
    let expected = transcript.finalize();
    if expected == [0; 32] || expected != proof.challenge_seed() {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    Ok(expected)
}
fn staged_semantic_verification_digest_v1(
    binding: &DecryptionBindingV1,
    polynomial_publication: &ZkAmsMkheDirectObjectPublicationReceiptV1,
    proof_publication: &ZkAmsMkheDirectObjectPublicationReceiptV1,
    snapshot: ZkAmsMkheDecryptionStreamingSnapshotV1,
    challenge_seed: [u8; 32],
    manifest_bytes: &[u8; ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    validate_staged_publication_pair_v1(polynomial_publication, proof_publication)?;
    let polynomial_pointer = polynomial_publication.pointer();
    let proof_pointer = proof_publication.pointer();
    if polynomial_pointer.kind() != ZkAmsMkheDirectObjectKindV1::DecryptionSharePolynomial
        || proof_pointer.kind() != ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof
        || snapshot.provider_identity == [0; 32]
        || snapshot.snapshot_identity == [0; 32]
        || challenge_seed == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut hash = Keccak256::new();
    hash.update(STAGED_DECRYPTION_RELATION_ADMISSION_DOMAIN_V1);
    binding.update_hash(&mut hash);
    hash.update(&polynomial_pointer.pointer_digest());
    hash.update(&proof_pointer.pointer_digest());
    hash.update(&polynomial_publication.receipt_digest());
    hash.update(&proof_publication.receipt_digest());
    hash.update(&snapshot.provider_identity);
    hash.update(&snapshot.snapshot_identity);
    hash.update(&challenge_seed);
    hash.update(manifest_bytes);
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(digest)
}
fn validate_staged_publication_pair_v1(
    polynomial_publication: &ZkAmsMkheDirectObjectPublicationReceiptV1,
    proof_publication: &ZkAmsMkheDirectObjectPublicationReceiptV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if polynomial_publication.pointer().kind()
        != ZkAmsMkheDirectObjectKindV1::DecryptionSharePolynomial
        || proof_publication.pointer().kind()
            != ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof
        || polynomial_publication.publication_identity() != proof_publication.publication_identity()
        || polynomial_publication.staging_identity() == proof_publication.staging_identity()
        || polynomial_publication.seal_identity() == proof_publication.seal_identity()
        || polynomial_publication
            .published_binding()
            .published_object_identity()
            == proof_publication
                .published_binding()
                .published_object_identity()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}
struct StagedProverBudgetedRandomSourceV1<'a, R: ?Sized> {
    source: &'a mut R,
    remaining_bytes: u64,
}
impl<'a, R: ?Sized> StagedProverBudgetedRandomSourceV1<'a, R> {
    fn new(source: &'a mut R, maximum_bytes: u64) -> Result<Self, ZkAmsMkheErrorV1> {
        if maximum_bytes == 0 {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        Ok(Self {
            source,
            remaining_bytes: maximum_bytes,
        })
    }
}
impl<R> MaskedRelaxedRandomSourceV1 for StagedProverBudgetedRandomSourceV1<'_, R>
where
    R: MaskedRelaxedRandomSourceV1 + ?Sized,
{
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        let requested = u64::try_from(destination.len())
            .map_err(|_| MaskedRelaxedRandomErrorV1::Unavailable)?;
        self.remaining_bytes = self
            .remaining_bytes
            .checked_sub(requested)
            .ok_or(MaskedRelaxedRandomErrorV1::Unavailable)?;
        self.source.fill_bytes(destination)
    }
}
fn sample_staged_proof_masks_v1<R>(
    degree: usize,
    secret_mask_bound: i64,
    error_mask_bound: i64,
    wide_mask_bound: &super::WideMagnitudeV1,
    random: &mut R,
) -> Result<
    (
        super::ZeroizingI64VectorV1,
        super::ZeroizingI64VectorV1,
        super::ZeroizingSignedWideVectorV1,
    ),
    ZkAmsMkheErrorV1,
>
where
    R: MaskedRelaxedRandomSourceV1,
{
    if degree == 0 || !degree.is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let mut secret_mask = super::ZeroizingI64VectorV1::with_capacity(degree)?;
    let mut error_mask = super::ZeroizingI64VectorV1::with_capacity(degree)?;
    let mut smudge_mask = super::ZeroizingSignedWideVectorV1::with_capacity(degree)?;
    // This three-pass order is the native prover's deterministic RNG contract.
    for _ in 0..degree {
        secret_mask.push(sample_signed_small(secret_mask_bound, random)?);
    }
    for _ in 0..degree {
        error_mask.push(sample_signed_small(error_mask_bound, random)?);
    }
    for _ in 0..degree {
        smudge_mask.push(sample_signed_wide(wide_mask_bound, random)?);
    }
    Ok((secret_mask, error_mask, smudge_mask))
}
/// Create, publish, semantically replay, and authenticate one bounded release share.
///
/// The move-only persistent use is consumed before randomness or staging. The polynomial is written
/// limb-by-limb and the `ZADP` proof section-by-section; neither canonical component is ever
/// retained by the prover. All prover-owned transient smudge, mask, response, NTT, and encoding
/// buffers are erased on success, error, and unwind. Borrowed party-state and active-secret owners
/// deliberately persist and enforce their own `Drop`. The manifest is suitable for
/// [`verify_combine_decode_zk_ams_mkhe_decryption_streaming_v1`], but the separately pinned
/// whole-worker residency certificate remains mandatory for release qualification.
#[allow(clippy::too_many_arguments)]
pub fn prove_zk_ams_mkhe_decryption_share_staged_v1<R, P>(
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
    party_index: usize,
    persistent_use: ZkAmsMkhePersistentDecryptionPartyUseV1,
    party_state: &ZkAmsMkheCollectivePartyStateV1,
    party_secret: &ZkAmsMkheActivePartySecretV1,
    random: &mut R,
    publisher: &mut P,
) -> Result<ZkAmsMkheStagedDecryptionShareV1, ZkAmsMkheErrorV1>
where
    R: MaskedRelaxedRandomSourceV1,
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    let profile = release_profile_v1();
    profile.validate()?;
    // Static topology, memory, and classified bulk-work ceilings are validated
    // before the move-only party use or any RNG/CAS side effect is consumed.
    let streaming_resource = zk_ams_mkhe_decryption_streaming_residency_evidence_v1()?;
    if !streaming_resource.enumerated_verifier_ceiling_met
        || !streaming_resource.compact_authority_enumerated_ceiling_met
        || !streaming_resource.staged_prover_enumerated_ceiling_met
        || !streaming_resource.staged_prover_work_ceiling_met
        || streaming_resource.staged_prover_first_candidate_rng_bytes
            > streaming_resource.staged_prover_rng_byte_budget
    {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    // Freeze the fully validated profile/roster/transcript frame before the
    // move-only party use, injected randomness, or CAS publisher is touched.
    let common_a_context = statement.prepare_common_a_context(&profile)?;
    let binding = statement.consume_party_use_v1(party_index, persistent_use, party_state)?;
    if party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || party_secret.party()? != statement.parties.parties[party_index]
        || usize::from(binding.party_index) != party_index
    {
        return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
    }
    let witness_secret = &party_state.secret().coefficients;
    let witness_error = &party_state.public_error().coefficients;
    if witness_secret.len() != profile.ring_degree
        || witness_error.len() != profile.ring_degree
        || witness_secret
            .iter()
            .any(|coefficient| coefficient.unsigned_abs() > 1)
        || witness_error
            .iter()
            .any(|coefficient| coefficient.unsigned_abs() > u64::from(profile.error_eta))
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let noise = zk_ams_mkhe_noise_certificate_v1()?;
    let smudge_bits = usize::from(noise.decryption_smudge_quotient_bits);
    let challenge_weight = wide_relation_challenge_weight(profile.ring_degree)?;
    let (secret_mask_bound, secret_limit) =
        small_response_parameters(1, challenge_weight, &profile)?;
    let (error_mask_bound, error_limit) =
        small_response_parameters(i64::from(profile.error_eta), challenge_weight, &profile)?;
    let (wide_mask_bound, wide_limit, wide_response_bytes) =
        wide_response_parameters(smudge_bits, challenge_weight)?;
    let resource = derive_decryption_resource_evidence(&profile)?;
    if u64::try_from(
        profile
            .ring_degree
            .checked_mul(wide_response_bytes)
            .and_then(|value| {
                value.checked_add(
                    profile
                        .ring_degree
                        .checked_mul(DECRYPTION_SIGNED_SMALL_BYTES_V1 * 2)?,
                )
            })
            .and_then(|value| value.checked_add(DECRYPTION_PROOF_HEADER_BYTES_V1))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .ok()
        != Some(resource.split_proof_envelope_bytes)
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    checked_ring_multiplication_work(&profile, STAGED_PROVER_MAXIMUM_RING_MULTIPLICATIONS_V1)?;
    let mut bounded_random = StagedProverBudgetedRandomSourceV1::new(
        random,
        streaming_resource.staged_prover_rng_byte_budget,
    )?;
    let mut remaining_common_a_candidates =
        streaming_resource.staged_prover_common_a_candidate_budget;
    // Match the native prover's two independent health checks and random draw
    // order exactly: health, smudge witness, health, then each mask attempt.
    validate_wide_relation_random_health(&mut bounded_random)?;
    let smudge_bound = super::WideMagnitudeV1::max_for_bits(smudge_bits)?;
    let mut smudge = super::ZeroizingSignedWideVectorV1::with_capacity(profile.ring_degree)?;
    for _ in 0..profile.ring_degree {
        smudge.push(sample_signed_wide(&smudge_bound, &mut bounded_random)?);
    }
    let mut snapshot =
        StreamingSnapshotAccumulatorV1::with_expected(statement.ciphertext_snapshot)?;
    let polynomial_publication = publish_staged_share_polynomial_v1(
        statement,
        &profile,
        party_state,
        smudge.as_slice(),
        publisher,
        &mut snapshot,
    )?;
    let share_pointer = polynomial_publication.pointer();
    snapshot.observe(polynomial_publication.post_publish_read_receipt())?;
    validate_wide_relation_random_health(&mut bounded_random)?;
    let transcript_prefix = build_staged_decryption_transcript_prefix_v1(
        statement,
        &common_a_context,
        &profile,
        smudge_bits,
        party_index,
        &binding,
        share_pointer,
        publisher,
        &mut snapshot,
        &mut remaining_common_a_candidates,
    )?;
    let accepted = {
        let mut accepted = None;
        for _ in 0..STAGED_PROVER_MAXIMUM_FS_ATTEMPTS_V1 {
            let (secret_mask, error_mask, smudge_mask) = sample_staged_proof_masks_v1(
                profile.ring_degree,
                secret_mask_bound,
                error_mask_bound,
                &wide_mask_bound,
                &mut bounded_random,
            )?;
            let challenge_seed = derive_staged_decryption_challenge_seed_v1(
                &transcript_prefix,
                statement,
                &common_a_context,
                &profile,
                secret_mask.as_slice(),
                error_mask.as_slice(),
                smudge_mask.as_slice(),
                publisher,
                &mut snapshot,
                &mut remaining_common_a_candidates,
            )?;
            if challenge_seed == [0; 32] {
                continue;
            }
            let challenge = derive_sparse_challenge(profile.ring_degree, challenge_seed)?;
            let terms = sparse_challenge_terms_v1(&challenge)?;
            if !staged_responses_fit_v1(
                &terms,
                witness_secret,
                witness_error,
                smudge.as_slice(),
                secret_mask.as_slice(),
                error_mask.as_slice(),
                smudge_mask.as_slice(),
                secret_limit,
                error_limit,
                &wide_limit,
            )? {
                continue;
            }
            accepted = Some((
                challenge_seed,
                challenge,
                terms,
                secret_mask,
                error_mask,
                smudge_mask,
            ));
            break;
        }
        accepted.ok_or(ZkAmsMkheErrorV1::RandomUnavailable)?
    };
    let (challenge_seed, challenge, challenge_terms, secret_mask, error_mask, smudge_mask) =
        accepted;
    let proof_publication = publish_staged_decryption_proof_v1(
        &profile,
        challenge_seed,
        &challenge_terms,
        witness_secret,
        witness_error,
        smudge.as_slice(),
        secret_mask.as_slice(),
        error_mask.as_slice(),
        smudge_mask.as_slice(),
        secret_limit,
        error_limit,
        &wide_limit,
        wide_response_bytes,
        publisher,
    )?;
    snapshot.observe(proof_publication.post_publish_read_receipt())?;
    validate_staged_publication_pair_v1(&polynomial_publication, &proof_publication)?;
    let proof_pointer = proof_publication.pointer();
    // No private prover vector survives into the public semantic replay.
    drop(secret_mask);
    drop(error_mask);
    drop(smudge_mask);
    drop(smudge);
    drop(challenge_terms);
    drop(challenge);
    let replayed_seed = verify_published_staged_relation_v1(
        &transcript_prefix,
        statement,
        &common_a_context,
        &profile,
        party_index,
        share_pointer,
        proof_pointer,
        publisher,
        &mut snapshot,
        &mut remaining_common_a_candidates,
    )?;
    if replayed_seed != challenge_seed {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    let snapshot = snapshot.finish()?;
    let polynomial = direct_to_decryption_transport_pointer_v1(
        share_pointer,
        ZkAmsMkheDirectObjectKindV1::DecryptionSharePolynomial,
        ZkAmsMkheDecryptionTransportComponentKindV1::SharePolynomial,
    )?;
    let proof = direct_to_decryption_transport_pointer_v1(
        proof_pointer,
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        ZkAmsMkheDecryptionTransportComponentKindV1::ProofEnvelope,
    )?;
    let manifest_digest = decryption_split_manifest_digest(&binding, polynomial, proof)?;
    let authentication = party_secret.authenticate_artifact(
        DECRYPTION_SPLIT_MANIFEST_AUTH_DOMAIN_V1,
        manifest_digest,
        &mut bounded_random,
    )?;
    let manifest = ZkAmsMkheDecryptionTransportManifestV1::new(
        binding.clone(),
        polynomial,
        proof,
        authentication,
    )?;
    let manifest_bytes: [u8; ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1] = manifest
        .encode()?
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let decoded = decode_streaming_manifest_exact(&manifest_bytes)?;
    validate_streaming_manifest_slot_v1(
        &decoded,
        party_index,
        statement.parties.parties[party_index],
        &statement.party_bindings[party_index],
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidShareSet)?;
    let semantic_verification_digest = staged_semantic_verification_digest_v1(
        &binding,
        &polynomial_publication,
        &proof_publication,
        snapshot,
        challenge_seed,
        &manifest_bytes,
    )?;
    Ok(ZkAmsMkheStagedDecryptionShareV1 {
        _seal: StagedDecryptionRelationAdmissionSealV1,
        party_index: binding.party_index,
        manifest_bytes,
        polynomial_publication,
        proof_publication,
        snapshot,
        semantic_verification_digest,
    })
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
    fn secret_limb(&self, modulus: u64) -> Result<ZeroizingStagedU64VectorV1, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let mut output = ZeroizingStagedU64VectorV1::new_zeroed(profile.ring_degree)?;
        for index in 0..profile.ring_degree {
            output.as_mut_slice()[index] =
                signed_mod(read_i64_at(self.bytes, self.secret_offset, index)?, modulus);
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
    fn with_expected(
        expected: ZkAmsMkheDecryptionStreamingSnapshotV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if expected.provider_identity == [0; 32] || expected.snapshot_identity == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self {
            identity: Some(expected),
        })
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
struct StreamingRnsObjectReaderV1 {
    transaction: ZkAmsMkheDirectObjectReadTransactionV1,
    next_limb: usize,
    coefficient_count: usize,
}
impl StreamingRnsObjectReaderV1 {
    fn begin<P>(
        expected_kind: ZkAmsMkheDirectObjectKindV1,
        pointer: ZkAmsMkheDirectObjectPointerV1,
        profile: &BgvProfile,
        provider: &mut P,
    ) -> Result<Self, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
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
            transaction,
            next_limb: 0,
            coefficient_count,
        };
        let mut count = [0_u8; 4];
        if reader.transaction.read_next(provider, &mut count)? != count.len()
            || usize::try_from(u32::from_be_bytes(count)).ok() != Some(coefficient_count)
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(reader)
    }
    fn read_limb<P>(
        &mut self,
        profile: &BgvProfile,
        limb: usize,
        provider: &mut P,
    ) -> Result<ZeroizingStagedU64VectorV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if limb != self.next_limb || profile.moduli.get(limb).is_none() {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let modulus = profile.moduli[limb];
        let mut values = ZeroizingStagedU64VectorV1::new_zeroed(profile.ring_degree)?;
        let mut buffer =
            ZeroizingStagedBytesV1::<ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1>::zeroed();
        let mut coefficient_offset = 0_usize;
        while coefficient_offset != profile.ring_degree {
            let coefficients =
                (profile.ring_degree - coefficient_offset).min(buffer.as_mut_slice().len() / 8);
            let bytes = coefficients
                .checked_mul(8)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            if self
                .transaction
                .read_next(provider, &mut buffer.as_mut_slice()[..bytes])?
                != bytes
            {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            for (index, encoded) in buffer.as_mut_slice()[..bytes].chunks_exact(8).enumerate() {
                let residue = u64::from_be_bytes(
                    encoded
                        .try_into()
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                );
                if residue >= modulus {
                    return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
                }
                values.as_mut_slice()[coefficient_offset + index] = residue;
            }
            coefficient_offset += coefficients;
        }
        self.next_limb += 1;
        Ok(values)
    }
    fn finish<P>(
        self,
        profile: &BgvProfile,
        provider: &mut P,
    ) -> Result<ZkAmsMkheDirectObjectReadReceiptV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
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
        self.transaction.finish(provider)
    }
}
fn read_complete_object_v1<P>(
    kind: ZkAmsMkheDirectObjectKindV1,
    pointer: ZkAmsMkheDirectObjectPointerV1,
    provider: &mut P,
) -> Result<
    (
        ZeroizingStagedByteVectorV1,
        ZkAmsMkheDirectObjectReadReceiptV1,
    ),
    ZkAmsMkheErrorV1,
>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let length = usize::try_from(pointer.payload_bytes())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut bytes = ZeroizingStagedByteVectorV1::new_zeroed_exact(length)?;
    let mut transaction = ZkAmsMkheDirectObjectReadTransactionV1::begin(kind, pointer, provider)?;
    let mut cursor = 0;
    while cursor != bytes.as_slice().len() {
        let end = cursor
            .checked_add(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1)
            .unwrap_or(bytes.as_slice().len())
            .min(bytes.as_slice().len());
        let expected = end - cursor;
        if transaction.read_next(provider, &mut bytes.as_mut_slice()[cursor..end])? != expected {
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
        let residues = reader.read_limb(profile, limb, provider)?;
        update_residue_limb(transcript, residues.as_slice());
    }
    reader.finish(profile, provider)
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
    aggregate: &mut ZeroizingAggregateRnsV1,
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
        .coefficients_mut()
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
#[allow(
    clippy::too_many_arguments,
    reason = "fixed decryption relation axes remain explicit to preserve transcript order"
)]
fn reconstruct_public_key_commitment_v1<P>(
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
    profile: &BgvProfile,
    proof: &ZkAmsMkheDecryptionProofViewV1<'_>,
    challenge: &[i8],
    party_index: usize,
    provider: &mut P,
    transcript: &mut Keccak256,
    prepared_common_a: &ZkAmsMkhePreparedCollectivePublicAContextV1,
    remaining_common_a_candidates: &mut u64,
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
        let common_a = derive_prepared_common_a_limb_v1(
            prepared_common_a,
            limb,
            remaining_common_a_candidates,
        )?;
        let secret_response = proof.secret_limb(modulus)?;
        let mut commitment = negacyclic_multiply_staged_v1(
            &common_a,
            secret_response.as_slice(),
            modulus,
            profile.negacyclic_roots[limb],
        )?;
        drop(common_a);
        drop(secret_response);
        let plaintext_modulus = profile.plaintext_modulus.residue(modulus);
        for (coefficient, value) in commitment.as_mut_slice().iter_mut().enumerate() {
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
        let party_b_limb = party_b.read_limb(profile, limb, provider)?;
        subtract_sparse_negacyclic_product_in_place(
            commitment.as_mut_slice(),
            challenge,
            party_b_limb.as_slice(),
            modulus,
        )?;
        update_residue_limb(transcript, commitment.as_slice());
    }
    party_b.finish(profile, provider)
}
#[allow(
    clippy::too_many_arguments,
    reason = "fixed decryption share axes remain explicit to preserve transcript order"
)]
fn reconstruct_share_commitment_and_aggregate_v1<P>(
    statement: &ZkAmsMkheStreamingDecryptionStatementV1<'_>,
    profile: &BgvProfile,
    proof: &ZkAmsMkheDecryptionProofViewV1<'_>,
    challenge: &[i8],
    share_pointer: ZkAmsMkheDirectObjectPointerV1,
    provider: &mut P,
    transcript: &mut Keccak256,
    mut aggregate: Option<&mut ZeroizingAggregateRnsV1>,
    snapshot: &mut StreamingSnapshotAccumulatorV1,
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
    let ciphertext = statement.ciphertext.sealed_binding_v1()?;
    let mut linear = ZeroizingStagedU64VectorV1::new_zeroed(profile.ring_degree)?;
    let mut scratch = ZeroizingStagedBytesV1::<ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1>::zeroed();
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let linear_receipt = ciphertext.read_linear_limb_into_v1(
            limb,
            profile,
            provider,
            linear.as_mut_slice(),
            scratch.as_mut_array(),
        )?;
        snapshot.observe(&linear_receipt)?;
        let secret_response = proof.secret_limb(modulus)?;
        let mut commitment = negacyclic_multiply_staged_v1(
            linear.as_slice(),
            secret_response.as_slice(),
            modulus,
            profile.negacyclic_roots[limb],
        )?;
        drop(secret_response);
        let plaintext_modulus = profile.plaintext_modulus.residue(modulus);
        for (coefficient, value) in commitment.as_mut_slice().iter_mut().enumerate() {
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
        let share_limb = share_reader.read_limb(profile, limb, provider)?;
        subtract_sparse_negacyclic_product_in_place(
            commitment.as_mut_slice(),
            challenge,
            share_limb.as_slice(),
            modulus,
        )?;
        if let Some(aggregate) = aggregate.as_deref_mut() {
            add_share_limb_in_place(aggregate, profile, limb, share_limb.as_slice())?;
        }
        update_residue_limb(transcript, commitment.as_slice());
    }
    share_reader.finish(profile, provider)
}
/// Verify one native share relation without materializing native `a`/`b_i`
/// polynomials or full RNS response vectors.
///
/// This test reference receives already-decoded shares, while its public key
/// inputs remain canonical wire objects. Reconstructing each commitment
/// limb-by-limb keeps the reference on the same bounded arithmetic
/// corridor as the seekable verifier: ciphertext `(c_0,c_1)` and aggregate
/// are the only complete native payloads retained by the caller.
#[cfg(test)]
#[expect(
    clippy::too_many_arguments,
    reason = "explicit native decryption relation inputs"
)]
pub(super) fn verify_native_relation_limb_streaming(
    profile: &BgvProfile,
    binding: &DecryptionBindingV1,
    common_a: &ZkAmsMkheRnsPolynomialWireV1,
    party_b: &ZkAmsMkheRnsPolynomialWireV1,
    ciphertext: &super::ZkAmsMkheCollectiveCiphertextV1,
    share: &super::RnsPolynomial,
    smudge_bits: usize,
    proof: &super::DecryptionRelationProofV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    common_a.encoded_len()?;
    party_b.encoded_len()?;
    if common_a.residues().iter().all(|value| *value == 0)
        || party_b.residues().iter().all(|value| *value == 0)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    share.validate(profile)?;
    if proof.challenge_seed == [0; 32]
        || proof.secret_response.len() != profile.ring_degree
        || proof.public_key_error_response.len() != profile.ring_degree
        || proof.smudge_response.len() != profile.ring_degree
    {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    checked_ring_multiplication_work(profile, 8)?;
    let challenge = super::derive_sparse_challenge(profile.ring_degree, proof.challenge_seed)?;
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut transcript =
        super::initialize_decryption_challenge_transcript(profile, smudge_bits, binding)?;
    super::update_wire_rns_hash(&mut transcript, common_a)?;
    super::update_wire_rns_hash(&mut transcript, party_b)?;
    super::update_rns_hash(&mut transcript, profile, ciphertext.constant())?;
    super::update_rns_hash(&mut transcript, profile, ciphertext.linear())?;
    super::update_rns_hash(&mut transcript, profile, share)?;
    transcript.update(
        &u32::try_from(coefficient_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let start = limb
            .checked_mul(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let common_a_limb = common_a
            .residues()
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        let party_b_limb = party_b
            .residues()
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        let mut secret_response = ZeroizingStagedU64VectorV1::with_capacity(profile.ring_degree)?;
        for &value in &proof.secret_response {
            secret_response.push(signed_mod(value, modulus));
        }
        let mut commitment = negacyclic_multiply_staged_v1(
            common_a_limb,
            secret_response.as_slice(),
            modulus,
            profile.negacyclic_roots[limb],
        )?;
        drop(secret_response);
        let plaintext_modulus = profile.plaintext_modulus.residue(modulus);
        for (coefficient, value) in commitment.as_mut_slice().iter_mut().enumerate() {
            *value = mod_add(
                mod_sub(0, *value, modulus),
                mod_mul(
                    plaintext_modulus,
                    signed_mod(proof.public_key_error_response[coefficient], modulus),
                    modulus,
                ),
                modulus,
            );
        }
        subtract_sparse_negacyclic_product_in_place(
            commitment.as_mut_slice(),
            &challenge,
            party_b_limb,
            modulus,
        )?;
        for value in commitment.as_slice() {
            transcript.update(&value.to_be_bytes());
        }
    }
    transcript.update(
        &u32::try_from(coefficient_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let start = limb
            .checked_mul(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let linear = ciphertext
            .linear()
            .coefficients
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidCiphertext)?;
        let share_limb = share
            .coefficients
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        let mut secret_response = ZeroizingStagedU64VectorV1::with_capacity(profile.ring_degree)?;
        for &value in &proof.secret_response {
            secret_response.push(signed_mod(value, modulus));
        }
        let mut commitment = negacyclic_multiply_staged_v1(
            linear,
            secret_response.as_slice(),
            modulus,
            profile.negacyclic_roots[limb],
        )?;
        drop(secret_response);
        let plaintext_modulus = profile.plaintext_modulus.residue(modulus);
        for (coefficient, value) in commitment.as_mut_slice().iter_mut().enumerate() {
            *value = mod_add(
                *value,
                mod_mul(
                    plaintext_modulus,
                    proof.smudge_response[coefficient].mod_u64(modulus),
                    modulus,
                ),
                modulus,
            );
        }
        subtract_sparse_negacyclic_product_in_place(
            commitment.as_mut_slice(),
            &challenge,
            share_limb,
            modulus,
        )?;
        for value in commitment.as_slice() {
            transcript.update(&value.to_be_bytes());
        }
    }
    if transcript.finalize() != proof.challenge_seed {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    Ok(())
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
            statement.ciphertext_digest(),
        ));
    }
    let mut manifests =
        try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1).map_err(|_| {
            identifiable_abort(
                &statement.parties,
                0,
                DecryptionAbortReasonV1::BindingMismatch,
                statement.ciphertext_digest(),
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
                    statement.ciphertext_digest(),
                ));
            }
            Err(_) => {
                return Err(identifiable_abort(
                    &statement.parties,
                    party_index,
                    DecryptionAbortReasonV1::BindingMismatch,
                    statement.ciphertext_digest(),
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
                statement.ciphertext_digest(),
            ));
        }
        manifests.push(manifest);
    }
    manifests.try_into().map_err(|_| {
        identifiable_abort(
            &statement.parties,
            0,
            DecryptionAbortReasonV1::BindingMismatch,
            statement.ciphertext_digest(),
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
/// Every manifest is authenticated and bound before the first large read. Each `b_i` and share is
/// then consumed in two complete BLAKE3-authenticated passes from one immutable provider snapshot.
/// The existing proof transcript and CRT correctness bound are unchanged.
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
            statement.ciphertext_digest(),
        )
    };
    statement.validate_compact().map_err(|_| binding_abort(0))?;
    let manifests = preflight_manifests(statement, manifest_bytes)?;
    let profile = release_profile_v1();
    let noise = zk_ams_mkhe_noise_certificate_v1().map_err(|_| binding_abort(0))?;
    let smudge_bits = usize::from(noise.decryption_smudge_quotient_bits);
    let final_residual_bits = usize::from(noise.final_decryption_residual_bits);
    let (expected_roster, expected_key_transcript_digest) =
        statement.persistent_context.streaming_public_axes_v1();
    let (live_axes, live_snapshot) = validate_streaming_ciphertext_live_v1(
        statement.roster,
        statement.ciphertext,
        statement.ciphertext_record_index(),
        expected_roster.key_material_digest(),
        expected_key_transcript_digest,
        statement
            .persistent_context
            .streaming_collective_key_digest_v1(),
        provider,
    )
    .map_err(|_| binding_abort(0))?;
    if live_axes != statement.ciphertext_axes || live_snapshot != statement.ciphertext_snapshot {
        return Err(binding_abort(0));
    }
    let mut aggregate =
        ZeroizingAggregateRnsV1::zero_exact_v1(&profile).map_err(|_| binding_abort(0))?;
    let ciphertext = statement
        .ciphertext
        .sealed_binding_v1()
        .map_err(|_| binding_abort(0))?;
    let mut aggregate_scratch =
        ZeroizingStagedBytesV1::<ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1>::zeroed();
    let mut snapshot = StreamingSnapshotAccumulatorV1::with_expected(statement.ciphertext_snapshot)
        .map_err(|_| binding_abort(0))?;
    for limb in 0..profile.moduli.len() {
        let start = limb
            .checked_mul(profile.ring_degree)
            .ok_or_else(|| binding_abort(0))?;
        let end = start
            .checked_add(profile.ring_degree)
            .ok_or_else(|| binding_abort(0))?;
        let receipt = ciphertext
            .read_constant_limb_into_v1(
                limb,
                &profile,
                provider,
                aggregate
                    .coefficients_mut()
                    .get_mut(start..end)
                    .ok_or_else(|| binding_abort(0))?,
                aggregate_scratch.as_mut_array(),
            )
            .map_err(|_| binding_abort(0))?;
        snapshot.observe(&receipt).map_err(|_| binding_abort(0))?;
    }
    let mut set_hash = Keccak256::new();
    set_hash.update(DECRYPTION_SET_DOMAIN_V1);
    set_hash.update(&statement.roster.roster_digest());
    set_hash.update(&statement.ciphertext_digest());
    let common_a_context = statement
        .prepare_common_a_context(&profile)
        .map_err(|_| binding_abort(0))?;
    let common_a_limb_derivations = profile
        .moduli
        .len()
        .checked_mul(manifests.len())
        .and_then(|count| count.checked_mul(2))
        .ok_or_else(|| binding_abort(0))?;
    let mut remaining_common_a_candidates = common_a_context
        .candidate_budget_for_limbs_v1(common_a_limb_derivations)
        .map_err(|_| binding_abort(0))?;
    for (party_index, manifest) in manifests.iter().enumerate() {
        let proof_pointer = direct_pointer_from_manifest(manifest.proof())
            .map_err(|_| binding_abort(party_index))?;
        let share_pointer = direct_pointer_from_manifest(manifest.polynomial())
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
                statement.ciphertext_digest(),
            )
        })?;
        snapshot
            .observe(&proof_receipt)
            .map_err(|_| binding_abort(party_index))?;
        let proof = ZkAmsMkheDecryptionProofViewV1::decode_release_exact(proof_bytes.as_slice())
            .map_err(|_| {
                identifiable_abort(
                    &statement.parties,
                    party_index,
                    DecryptionAbortReasonV1::ProofFailure,
                    statement.ciphertext_digest(),
                )
            })?;
        checked_ring_multiplication_work(&profile, 8).map_err(|_| binding_abort(party_index))?;
        let challenge = derive_sparse_challenge(profile.ring_degree, proof.challenge_seed())
            .map_err(|_| {
                identifiable_abort(
                    &statement.parties,
                    party_index,
                    DecryptionAbortReasonV1::ProofFailure,
                    statement.ciphertext_digest(),
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
            let common_a = derive_prepared_common_a_limb_v1(
                &common_a_context,
                limb,
                &mut remaining_common_a_candidates,
            )
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
        hash_streaming_ciphertext_components_v1(
            &ciphertext,
            &profile,
            provider,
            &mut transcript,
            &mut snapshot,
        )
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
                statement.ciphertext_digest(),
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
            &common_a_context,
            &mut remaining_common_a_candidates,
        )
        .map_err(|_| {
            identifiable_abort(
                &statement.parties,
                party_index,
                DecryptionAbortReasonV1::ProofFailure,
                statement.ciphertext_digest(),
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
            Some(&mut aggregate),
            &mut snapshot,
        )
        .map_err(|_| {
            identifiable_abort(
                &statement.parties,
                party_index,
                DecryptionAbortReasonV1::ProofFailure,
                statement.ciphertext_digest(),
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
                statement.ciphertext_digest(),
            ));
        }
        set_hash.update(&[u8::try_from(party_index).unwrap_or(u8::MAX)]);
        set_hash.update(&manifests[party_index].manifest_digest());
    }
    let (plaintext, maximum_residual_bits) =
        decode_centered_plaintext(&profile, aggregate.as_rns(), final_residual_bits).map_err(
            |_| {
                identifiable_abort(
                    &statement.parties,
                    0,
                    DecryptionAbortReasonV1::CorrectnessBoundExceeded,
                    statement.ciphertext_digest(),
                )
            },
        )?;
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
