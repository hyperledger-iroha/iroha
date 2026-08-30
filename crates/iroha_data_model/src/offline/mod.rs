//! Canonical Kagemusha offline cash: online top-up, recursive spend, and online redemption.
mod kagemusha_canary_evidence;
mod kagemusha_internal_validation_receipt;
mod kagemusha_post_canary_validator_liveness;
mod kagemusha_promotion_receipt;
mod kagemusha_release_lifecycle;
mod kagemusha_runtime_effective_config_projection;
mod offline_cash_release_v1;
mod offline_cash_v1;
mod receiver_snapshot;
mod status;
pub use self::{
    kagemusha_canary_evidence::*, kagemusha_internal_validation_receipt::*,
    kagemusha_post_canary_validator_liveness::*, kagemusha_promotion_receipt::*,
    kagemusha_release_lifecycle::*, kagemusha_runtime_effective_config_projection::*, model::*,
    offline_cash_release_v1::*, offline_cash_v1::*,
};
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    block::consensus_v2::{
        ConsensusMode, DataAvailabilityLayout, DualQuorum, GlobalPhase, HeightContext,
        HeightContextId, MAX_VALIDATORS_PER_HEIGHT, PROTOCOL_VERSION, QuorumCertificate,
        SnapshotBootstrapAnchor, ValidatorPower, finality::FinalizedNextEpochSnapshot,
    },
    confidential::ConfidentialStatus,
    proof::{ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
#[cfg(test)]
use iroha_crypto::KeyPair;
use iroha_crypto::{
    Algorithm, Hash, PublicKey, SignatureOf, derive_non_signing_ed25519_public_key,
};
use iroha_data_model_derive::model;
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(test)]
use norito::to_bytes;
use p256::ecdsa::{
    Signature as P256Signature, VerifyingKey as P256VerifyingKey, signature::Verifier as _,
};
pub use receiver_snapshot::*;
use sha2::{Digest as _, Sha256};
pub use status::*;
/// Prefix embedded into offline instruction rejection messages.
///
/// Mobile SDKs parse the label after this prefix up to the first `:` to recover
/// stable machine-readable error codes.
pub const OFFLINE_REJECTION_REASON_PREFIX: &str = "offline_reason::";
/// Peer-cash finality capability implemented by every Iroha node.
///
/// `cash_handoff_v1` means the sender irreversibly consumes the selected inputs
/// and signs the exact outgoing payment before the payment is exposed to a
/// receiver-capable transport. Receiver acknowledgement is delivery evidence
/// only: it is never an acceptance, commit, rollback, or clawback gate.
pub const KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1: &str = "cash_handoff_v1";
/// Domain-separation tag for deterministic offline escrow derivation.
pub const OFFLINE_ESCROW_ACCOUNT_DOMAIN: &str = "iroha.offline.escrow.v1";
/// Stable public Norito schema name for the first-release Torii top-up request.
pub const OFFLINE_TOP_UP_REQUEST_SCHEMA_NAME: &str = "iroha.torii.v1.offline.top_up.request";
/// Stable public Norito schema name for the first-release Torii redemption request.
pub const OFFLINE_REDEEM_REQUEST_SCHEMA_NAME: &str = "iroha.torii.v1.offline.redeem.request";
include!("device_attestation_constants.rs");
/// Maximum asset scale accepted by the exact Kagemusha V2 amount contract.
pub const KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2: u32 = 28;
/// Fixed confidential Merkle-tree depth shared by top-up, spend, and redemption.
pub const KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2: usize = 16;
/// Fixed depth-16 confidential tree capacity used by top-up shielding.
pub const KAGEMUSHA_TOPUP_SHIELD_TREE_CAPACITY_V2: u32 = 1 << KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2;
/// Maximum private output leaves appended after recursive-spend initialization.
///
/// Every one of the 64 permitted branch decisions appends one selected output.
/// Up to eight of those decisions may be peer splits, each of which can append
/// one additional sender-change output.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_FUTURE_OUTPUTS_V2: u32 =
    KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2 as u32
        + KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2;
/// Number of tree positions available for top-up insertions.
///
/// After the top-up leaf, the tree reserves the complete recursive-spend
/// output budget plus one distinct final empty frontier leaf. This ensures the
/// last admitted top-up can exercise all 64 branch decisions, including eight
/// two-output peer splits, without stranding its private balance.
pub const KAGEMUSHA_TOPUP_SHIELD_INSERTION_CAPACITY_V2: u32 =
    KAGEMUSHA_TOPUP_SHIELD_TREE_CAPACITY_V2 - KAGEMUSHA_RECURSIVE_SPEND_MAX_FUTURE_OUTPUTS_V2 - 1;
/// Maximum canonical top-up shield proof envelope accepted at typed ingress.
pub const KAGEMUSHA_TOPUP_SHIELD_MAX_PROOF_BYTES_V2: usize = 192 * 1024;
/// Absolute canonical byte ceiling for one ABI-21 unshield-v3 proof.
///
/// The installed verifier record may advertise a lower limit, but no
/// Kagemusha redemption archive may carry a larger proof.
pub const KAGEMUSHA_UNSHIELD_MAX_PROOF_BYTES_V4: usize = 192 * 1024;
/// Maximum number of branch decisions carried by one recursive spend lineage.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2: u8 = 64;
/// Maximum number of device-to-device transfers in one recursive spend lineage.
///
/// This is intentionally independent of branch depth: redemption-change can
/// extend a branch without adding a peer hop.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2: u32 = 8;
/// Bytes retained from each domain-separated transition digest in a branch history.
///
/// A 192-bit chosen-prefix tag gives a 96-bit birthday bound. At depth 64, two claims alone occupy
/// 3,072 bytes, so this layout must not be certified against the 12 KiB peer gate until the
/// complete proof-bearing archive is measured. The complete 256-bit transition digest remains
/// proof-bound in the producing statement.
pub const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2: usize = 24;
/// Current compact top-up finality proof layout.
pub const KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2: u16 = 1;
/// Current trusted validator-roster artifact layout.
pub const KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2: u16 = 1;
/// Consensus maximum matching the block-local bounded Merkle tree.
pub const KAGEMUSHA_TOPUP_FINALITY_MAX_ANCHORS_PER_BLOCK_V2: u32 = 16;
/// Maximum balanced-Merkle siblings for 16 block-local anchors.
pub const KAGEMUSHA_TOPUP_FINALITY_MAX_SIBLINGS_V2: usize = 4;
/// Maximum validator count accepted by an offline roster artifact.
///
/// This is deliberately identical to the live Sumeragi-v2 bound. A smaller offline bound would let
/// consensus finalize a top-up for which no portable proof could subsequently be produced.
pub const KAGEMUSHA_TOPUP_FINALITY_MAX_VALIDATORS_V2: usize = MAX_VALIDATORS_PER_HEIGHT;
/// Maximum roster activation windows in one authenticated finality artifact.
///
/// A release binds exactly one immutable roster window. Rotation publishes a new content-addressed
/// release instead of making every verifier ingest unrelated historical or future validator sets.
pub const KAGEMUSHA_TOPUP_FINALITY_MAX_ROSTER_WINDOWS_V2: usize = 1;
/// Maximum canonical Norito bytes accepted for one compact top-up finality proof.
///
/// The epoch-boundary case can retain the complete next-epoch identity snapshot, including all
/// 4,096 bounded `PoPs` plus current and parent signer lists. Canonical ingress enforces this 2 MiB
/// cap before reconstruction and uses a frame-scaled allocation ceiling for the nested collections.
pub const KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_V2: u64 = 2 * 1024 * 1024;
/// Native-width mirror of [`KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_V2`].
const KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_USIZE_V2: usize = 2 * 1024 * 1024;
/// Maximum canonical Norito bytes accepted for one complete validated top-up anchor.
pub const KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_V2: u64 = 64 * 1024;
/// Native-width mirror of [`KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_V2`].
const KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_USIZE_V2: usize = 64 * 1024;
/// Maximum recursive proof transitions, including top-up and redemption-change splits.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2: u32 = 128;
/// Maximum number of recursive input branches consumed by one peer transition.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2: usize = 2;
/// Maximum independent conflict claims carried by one joined note.
///
/// Keeping this bound equal to the input arity prevents recursively doubling
/// claim metadata through joins.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_CLAIMS_V2: usize = 2;
/// Maximum raw Norito bytes accepted for one complete recursive peer archive.
///
/// The exact-state Eq/Ep proof pair requires a larger release envelope than the retired
/// digest-bound proof. Text transports must independently bound their base64url representation (at
/// most 43,691 unpadded bytes, plus their transport discriminator) before allocation or decoding.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2: usize = 32_768;
/// Maximum canonical receiver-verification request including two complete
/// finality proofs, two full anchors, and one authenticated roster artifact.
///
/// The additional fixed allowance covers the compact peer bundle, recipient
/// request, Norito collection framing, and receiver policy fields.
pub const KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_MAX_BYTES_V2: usize =
    KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2
        + KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_USIZE_V2
        + KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
            * (KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_USIZE_V2
                + KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_USIZE_V2)
        + 64 * 1024;
/// Exact byte length of the sole canonical uncompressed SEC1 P-256 device key.
pub const KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V2: usize = 65;
/// Exact byte length of the canonical raw low-S P-256 signature (`r || s`).
pub const KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2: usize = 64;
/// Maximum lifetime of a signed online top-up or redemption authorization.
pub const KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2: u64 = 5 * 60 * 1_000;
/// Domain separator for nonce-bound recipient payment request digests.
pub const KAGEMUSHA_RECIPIENT_PAYMENT_REQUEST_DIGEST_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:recipient-payment-request";
/// Domain separator for receiver-device signatures on payment requests.
pub const KAGEMUSHA_RECIPIENT_PAYMENT_REQUEST_SIGNING_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:recipient-payment-request-signing";
/// Domain separator for recursive bundle identity digests.
pub const KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_DIGEST_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:recursive-spend-bundle";
/// Domain separator for exact split transition binding digests.
pub const KAGEMUSHA_RECURSIVE_SPEND_SPLIT_BINDING_DIGEST_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:split-binding";
/// SHA-256 domain separator for compact 192-bit transition-choice tags.
pub const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:transition-tag:sha256-192";
/// Domain separator for the V2 recursive public statement digest.
pub const KAGEMUSHA_RECURSIVE_SPEND_PUBLIC_STATEMENT_DIGEST_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:public-statement";
/// Domain separator for the compact V5 field-neutral recursive-state boundary.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_DOMAIN_V5: &[u8] =
    b"iroha:kagemusha:recursive-state-boundary:v5";
/// Shared verifier role id for confidential transfer evidence.
pub const KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2: &str = "confidential_transfer_v2_verifier_record";
/// Verifier role for public-to-confidential Kagemusha top-up shielding.
pub const KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2: &str =
    "kagemusha_topup_shield_v2_verifier_record";
/// Shared verifier role id for unshield evidence.
pub const KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2: &str = "confidential_unshield_v3_verifier_record";
/// Chain verifier role for the ABI-21 Eq/Fp recursive-step half.
pub const KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4: &str =
    "kagemusha_recursive_step_eq_v4_verifier_record";
/// Chain verifier role for the ABI-21 Ep/Fq recursive-step half.
pub const KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4: &str =
    "kagemusha_recursive_step_ep_v4_verifier_record";
/// Shared verifier purpose for top-up and offline split evidence.
pub const KAGEMUSHA_VERIFIER_PURPOSE_TRANSFER_V2: &str = "offline_split";
/// Verifier purpose for the public-to-confidential top-up transition.
pub const KAGEMUSHA_VERIFIER_PURPOSE_TOPUP_SHIELD_V2: &str = "online_to_offline_topup_shield";
/// Shared verifier purpose for offline-to-online redemption.
pub const KAGEMUSHA_VERIFIER_PURPOSE_UNSHIELD_V2: &str = "offline_to_online_redemption";
/// Verifier purpose for either half of the sole ABI-21 recursive step.
pub const KAGEMUSHA_VERIFIER_PURPOSE_STEP_V4: &str = "kagemusha_recursive_spend_step_v4";
/// Domain separator for the self-contained V2 request authorization signature.
pub const KAGEMUSHA_REQUEST_AUTHORIZATION_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:request-authorization";
/// Domain separator for the hardware assertion authorizing one exact online operation.
pub const KAGEMUSHA_ONLINE_HARDWARE_ASSERTION_DOMAIN_V1: &str =
    "iroha:kagemusha:v1:online-hardware-assertion";
/// Domain separator for receiver acknowledgement signing payloads.
pub const KAGEMUSHA_RECEIVER_ACKNOWLEDGEMENT_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:receiver-acknowledgement";
/// Domain separator for a receiver public-key reference.
pub const KAGEMUSHA_RECEIVER_KEY_REFERENCE_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:receiver-key-reference";
/// Domain separator for a canonical receiver acknowledgement identity digest.
pub const KAGEMUSHA_RECEIVER_ACKNOWLEDGEMENT_DIGEST_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:receiver-acknowledgement-digest";
/// Domain separator for unsigned V2 top-up payload digests.
pub const KAGEMUSHA_TOPUP_PAYLOAD_DIGEST_DOMAIN_V2: &str = "iroha:kagemusha:v2:topup-payload";
/// Domain separator for unsigned V2 redemption payload digests.
pub const KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V2: &str = "iroha:kagemusha:v2:redeem-payload";
/// Domain separator for finalized chain top-up anchor receipts.
pub const KAGEMUSHA_TOPUP_ANCHOR_DIGEST_DOMAIN_V2: &str = "iroha:kagemusha:v2:topup-anchor";
/// Domain separator for a V2 redemption transition binding.
pub const KAGEMUSHA_REDEMPTION_TRANSITION_DIGEST_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:redemption-transition";
/// Domain separator for canonical unshield-v3 public-input words.
pub const KAGEMUSHA_UNSHIELD_PUBLIC_INPUTS_DIGEST_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:unshield-public-inputs";
/// Native bridge ABI for the degree-parameterized recursive-spend V4 contract.
///
/// V4 is deliberately not an alias for ABI 19: its public accumulator layout, fold transcripts, key
/// parsing parameters, and artifact framing all depend on an authenticated IPA degree.
pub const KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 23;
/// Exact schema identifier for the degree-parameterized artifact manifest.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4: &str =
    "kagemusha.offline.recursive_spend.artifact_manifest.v4";
/// Exact schema of the independently pinned reviewed clean source closure.
pub const KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_SCHEMA_V1: &str = "iroha.reviewed-source-closure.v1";
/// Maximum untracked regular-file entries in a first-release source closure.
pub const KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_UNTRACKED_FILES_V1: usize = 0;
/// Maximum tracked root `Cargo.lock` bytes admitted by the V1 source closure.
pub const KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_CARGO_LOCK_BYTES_V1: u64 = 16 * 1024 * 1024;
/// Maximum tracked root `Cargo.lock` bytes admitted by the reviewed closure.
pub const KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_CARGO_LOCK_BYTES_V2: u64 = 16 * 1024 * 1024;
/// Degree-parameterized Pasta-cycle backend selected only by ABI 21 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4: &str =
    "halo2/ipa-pasta-cycle-compact-v5";
/// Transcript contract for V4 proofs and degree-sized BGH19 folds.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4: &str =
    "kagemusha-pasta-cycle-poseidon-compact-v5";
/// Eq/Fp recursive-step circuit with authenticated dynamic IPA layout.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4: &str =
    "kagemusha-recursive-spend-step-eq-compact-layout-v5";
/// Ep/Fq recursive-step circuit with authenticated dynamic IPA layout.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4: &str =
    "kagemusha-recursive-spend-step-ep-compact-lineage-v5";
/// Verifying-key curve for the ABI-21 `EqAffine` recursive-step half.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4: &str = "vesta";
/// Verifying-key curve for the ABI-21 `EpAffine` recursive-step half.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V4: &str = "pallas";
/// Number of canonical Pasta field elements exposed by one ABI-21/V4 Step operation.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_OPERATION_FIELD_ELEMENTS_V4: usize = 135;
/// Exact `u32` limbs used by the eight-limb encoding of every operation element.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_OPERATION_LIMBS_V4: usize =
    KAGEMUSHA_RECURSIVE_SPEND_STEP_OPERATION_FIELD_ELEMENTS_V4 * 8;
/// Minimum `u32` length at the authenticated compact degree (`k = 17`).
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_MIN_PUBLIC_INPUT_LIMBS_V4: usize = 66;
/// Maximum `u32` length at the authenticated compact degree (`k = 17`).
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_MAX_PUBLIC_INPUT_LIMBS_V4: usize = 66;
/// Canonical ABI-21/V4 field-neutral public inputs for the EqAffine/Vesta step circuit.
/// Its `operation_protocol_v2` label versions only the subordinate operation-vector layout; it
/// cannot select a V2/V3 executor. Compact V5 state changes V4 circuit identity and invalidates earlier candidates.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PUBLIC_INPUTS_SCHEMA_V4: &[u8] = br#"{"schema":"kagemusha_recursive_spend_step_eq_compact_v5","layout":"single_column_field","elements":66,"ipa_round_count":17,"semantic_authority":"step_eq","semantic_header":{"elements":20,"encoding":"canonical_u128_chunks","fields":["compact_profile_version","parent_count","proof_step_count","public_statement_digest[2]","operation_poseidon_fp[2]","parent_state_poseidon_fp[2][2]","result_state_poseidon_fp[2]","manifest_sha256[2]","step_eq_protocol_sha256[2]","step_ep_protocol_sha256[2]","live_selector"]},"ipa_accumulator":{"wire_version":5,"elements":38,"formula":"2*ipa_round_count+4","encoding":"canonical_u128_chunks"},"reciprocal_audits":{"hash":"sha256","digests":4,"elements_per_digest":2},"private_witness":{"state_layout_version":5,"state_limbs":138,"parent_slots":2,"operation_field_elements":135,"operation_limbs":1080}}"#;
/// Canonical ABI-21/V4 field-neutral public inputs for the EpAffine/Pallas step circuit.
/// As with Eq, `operation_protocol_v2` versions the operation vector, not the execution path.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PUBLIC_INPUTS_SCHEMA_V4: &[u8] = br#"{"schema":"kagemusha_recursive_spend_step_ep_compact_v5","layout":"single_column_field","elements":66,"ipa_round_count":17,"semantic_authority":"step_eq","role":"lineage_and_reciprocal_wrapper","semantic_header":{"elements":20,"encoding":"canonical_u128_chunks","fields":["compact_profile_version","parent_count","proof_step_count","public_statement_digest[2]","operation_poseidon_fp[2]","parent_state_poseidon_fp[2][2]","result_state_poseidon_fp[2]","manifest_sha256[2]","step_eq_protocol_sha256[2]","step_ep_protocol_sha256[2]","live_selector"]},"ipa_accumulator":{"wire_version":5,"elements":38,"formula":"2*ipa_round_count+4","encoding":"canonical_u128_chunks"},"reciprocal_audits":{"hash":"sha256","digests":4,"elements_per_digest":2}}"#;
/// Version of the compact canonical cross-field state boundary.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V5: u16 = 5;
/// Version stored in limb zero of the compact cross-field recursive state.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5: u32 = 5;
/// Exact number of unreduced `u32` limbs carried between both Pasta fields.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5: usize = 138;
/// Proof-envelope version for the authenticated dynamic-layout V4 wire.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4: u16 = 5;
/// Version of the degree-parameterized recursive-spend artifact manifest.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4: u16 = 4;
/// Version carried by every ABI-21 chain-facing request and artifact binding.
pub const KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4: u16 = 4;
/// Schema identifier for the immutable pre-evidence ABI-21 candidate record.
pub const KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4: &str =
    "kagemusha.offline.recursive_spend.candidate.v4";
/// Version of the immutable pre-evidence ABI-21 candidate record.
pub const KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4: u16 = 4;
/// Schema identifier for the canonical actual-recursion qualification receipt.
pub const KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_SCHEMA_V4: &str =
    "kagemusha.offline.recursive_spend.qualification_receipt.v4";
/// First-release version of the canonical actual-recursion qualification receipt.
pub const KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_VERSION_V4: u16 = 1;
/// Canonical candidate/final inventory file carrying actual recursive proof pairs.
pub const KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4: &str =
    "recursive-step-two-qualification-v4.norito";
/// Maximum canonical qualification receipt size, including two bounded proof pairs.
pub const KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4: usize =
    2 * KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize + 16 * 1024;
/// Domain separator for the candidate-plus-receipt release identity.
pub const KAGEMUSHA_RECURSIVE_SPEND_QUALIFIED_CANDIDATE_DOMAIN_V4: &[u8] =
    b"iroha:kagemusha:recursive-spend-qualified-candidate:v4";
/// Absolute first-release memory ceiling for candidate generation and publication.
pub const KAGEMUSHA_RECURSIVE_SPEND_GENERATION_MEMORY_ABSOLUTE_MAX_BYTES_V4: u64 =
    64 * 1024 * 1024 * 1024;
/// Mandatory in-process physical-footprint enforcement profile.
pub const KAGEMUSHA_RECURSIVE_SPEND_GENERATION_MEMORY_ENFORCEMENT_PROFILE_V4: &str =
    "self-physical-footprint-v1";
/// Schema identifier for the configured Kagemusha release-signing policy.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1: &str =
    "kagemusha.offline.recursive_spend.release_policy.v1";
/// Schema identifier for a signed V4 recursive-spend release envelope.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V4: &str =
    "kagemusha.offline.recursive_spend.release_attestation.v4";
/// Schema identifier for an authenticated ABI-21/V4 promotion record.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4: &str =
    "kagemusha.offline.recursive_spend.promoted_release.v4";
/// Version-one domain separator retained by the configured release policy.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_APPROVAL_DOMAIN_V1: &str =
    "iroha:kagemusha:recursive-spend-release-approval:v1";
/// Domain separator for role-specific V4 release approvals.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_APPROVAL_DOMAIN_V4: &str =
    "iroha:kagemusha:recursive-spend-release-approval:v4";
/// Schema identifier for a signed, candidate-bound V4 cryptographic review.
pub const KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_SCHEMA_V4: &str =
    "kagemusha.offline.recursive_spend.cryptographic_review.v4";
/// Domain separator signed by every V4 cryptographic reviewer.
pub const KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_DOMAIN_V4: &str =
    "iroha:kagemusha:recursive-spend-cryptographic-review:v4";
/// Version of the canonical signed V4 cryptographic-review envelope.
pub const KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_VERSION_V4: u16 = 4;
/// Exact number of independently evidenced checks in a production V4 review.
pub const KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_CHECK_COUNT_V4: usize = 6;
/// Current release policy, attestation, and promotion-record version.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1: u16 = 1;
/// Signed-envelope version for explicitly degree-parameterized V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4: u16 = 4;
/// Defensive upper bound for authorized signers or supplied approvals.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1: usize = 64;
/// Maximum signed review or physical-device evidence file accepted by promotion tooling.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1: usize = 16 * 1024 * 1024;
/// Maximum canonical signed V4 cryptographic-review envelope size.
pub const KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4: usize = 1024 * 1024;
/// Maximum canonical ABI-21/V4 promotion record accepted by release consumers.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4: usize = 1024 * 1024;
/// Canonical Norito file containing the signed V4 release envelope.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4: &str =
    "release-attestation-v4.norito";
/// Canonical opaque file containing signed physical-device benchmark evidence.
pub const KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1: &str =
    "physical-device-benchmark.evidence";
/// Canonical Norito file containing the signed, candidate-bound cryptographic review.
pub const KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1: &str =
    "cryptographic-review.evidence";
/// Version of the canonical authenticated V4 circuit configuration.
pub const KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4: u16 = 5;
/// Authenticated fixed degree of the complete compact V5 Step circuit.
pub const KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4: u32 = 17;
/// Highest degree whose serialized Pasta parameters fit the release artifact
/// corridor with a conservative margin.
pub const KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4: u32 = 17;
/// Minimum unusable-row reservation required by the Halo2 base circuit.
pub const KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4: u32 = 9;
/// Exact supported advice challenge-phase vector length.
/// Compact V5 has no challenge-dependent witness work; empty later phases make Halo2 repeat
/// phase-zero synthesis and retain unused polynomials, so parameters authenticate only phase zero.
pub const KAGEMUSHA_STEP_CIRCUIT_MAX_PHASES_V4: usize = 1;
/// Maximum configured columns of any one class in a phase.
pub const KAGEMUSHA_STEP_CIRCUIT_MAX_COLUMNS_V4: u32 = 220;
/// Reviewed first-release advice-column profile for compact degree-17 generation.
pub const KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4: [u32; 1] = [220];
/// Reviewed first-release lookup-column profile for compact degree-17 generation.
/// `BaseCircuitBuilder` reports unused challenge phases as explicit zero-width suffixes.
pub const KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4: [u32; 3] = [25, 0, 0];
/// Domain separator for canonical V4 circuit-parameter identities.
pub const KAGEMUSHA_STEP_CIRCUIT_PARAMS_SHA256_DOMAIN_V4: &[u8] =
    b"iroha:kagemusha:step-circuit-params:compact-v5";
/// Version of the degree-parameterized accumulated-opening wire.
pub const KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4: u16 = 5;
/// Version of the V4 Eq/Ep proof-pair wire.
pub const KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4: u16 = 5;
/// Version of one authenticated selector-zero bootstrap witness payload.
pub const KAGEMUSHA_PASTA_BOOTSTRAP_WITNESS_VERSION_V4: u16 = 5;
/// Domain separator for canonical V4 bootstrap witness identities.
pub const KAGEMUSHA_PASTA_BOOTSTRAP_WITNESS_SHA256_DOMAIN_V4: &[u8] =
    b"iroha:kagemusha:pasta-bootstrap-witness:compact-v5";
/// Public selector used only by the manifest-independent bootstrap circuit.
pub const KAGEMUSHA_PASTA_PUBLIC_BOOTSTRAP_SELECTOR_V4: u32 = 0;
/// Public selector required by every ordinary live V4 Step proof.
pub const KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4: u32 = 1;
/// Absolute defensive ceiling for one measured V4 Step proof transcript.
/// The 192 KiB bound leaves headroom above 93,120 bytes; promotion pins the exact transcript size.
pub const KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4: u32 = 192 * 1024;
/// Exact transcript bytes for one Step proof in the reviewed release profile.
pub const KAGEMUSHA_STEP_PROOF_RELEASE_BYTES_V4: u32 = 93_120;
/// Absolute defensive ceiling for one canonical V4 Eq/Ep proof-pair payload.
/// The 384 KiB bound admits the 191,862-byte recursive shape; promotion pins its exact maximum.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4: u32 = 384 * 1024;
/// Exact initialization-pair bytes for the reviewed release profile.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_INITIALIZATION_BYTES_V4: u32 = 186_852;
/// Exact maximum recursive-pair bytes for the reviewed release profile.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4: u32 = 191_862;
/// Maximum processed proving-key payload admitted by the compact V5 profile.
/// Its five-Table16-SHA processed key is file-backed and bounded apart from verifier memory.
pub const KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5: u64 = 5 * 1024 * 1024 * 1024;
/// Maximum serialized `ParamsIPA` payload admitted by the compact V5 profile.
pub const KAGEMUSHA_COMPACT_PARAMS_IPA_MAX_BYTES_V5: u64 = 9 * 1024 * 1024;
/// Exact cryptographic profile embedded inside the ABI-21/V4 lifecycle.
pub const KAGEMUSHA_COMPACT_PROFILE_VERSION_V5: u32 = 5;
/// Maximum canonical recipient-only ABI-21 peer-payment archive.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4: usize = 32 * 1024 * 1024;
/// Maximum canonical provenance archive carried by one ABI-21 spendable branch.
/// It admits one maximum roster and two bounded anchor/finality pairs plus framing headroom.
pub const KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4: usize =
    KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_USIZE_V2
        + KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
            * (KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_USIZE_V2
                + KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_USIZE_V2)
        + 64 * 1024;
/// Maximum canonical ABI-21 online-to-offline chain request.
/// It covers shield proof, optional device attestation, and bounded metadata with headroom.
pub const KAGEMUSHA_RECURSIVE_SPEND_TOPUP_REQUEST_MAX_BYTES_V4: usize = 512 * 1024;
/// Number of canonical Pallas-field elements in one ABI-21 public operation row.
pub const KAGEMUSHA_RECURSIVE_SPEND_OPERATION_FIELD_ELEMENTS_V4: usize = 135;
/// Exact little-endian `u32` limbs carried for one ABI-21 public operation row.
pub const KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4: usize =
    KAGEMUSHA_RECURSIVE_SPEND_OPERATION_FIELD_ELEMENTS_V4 * 8;
/// Pallas base-field modulus as eight exact little-endian `u32` limbs.
pub const KAGEMUSHA_RECURSIVE_SPEND_OPERATION_FP_MODULUS_U32_LE_V4: [u32; 8] = [
    0x0000_0001,
    0x992d_30ed,
    0x094c_f91b,
    0x2246_98fc,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0x4000_0000,
];
/// Framing magic for a streamed V4 recursive-spend key artifact.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4: &[u8; 8] = b"KRV4KEY\0";
/// Exact public-header layout version used by every `KRV4KEY` file.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4: u16 = 4;
/// Defensive upper bound for the canonical Norito header preceding a V4 payload.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_MAX_BYTES_V4: u32 = 64 * 1024;
/// Maximum size of any one V4 content-addressed artifact file.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4: u64 = 5 * 1024 * 1024 * 1024;
/// Canonical Eq `ParamsIPA` package file name for V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4: &str =
    "step-eq.params-ipa.krv4";
/// Canonical Eq processed proving-key package file name for V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4: &str =
    "step-eq.proving-key.krv4";
/// Canonical Eq processed verifier-key package file name for V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4: &str =
    "step-eq.verifying-key.krv4";
/// Canonical Eq final-key selector-zero bootstrap-witness package for V4 runtime.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4: &str =
    "step-eq.bootstrap-witness.krv4";
/// Canonical Ep `ParamsIPA` package file name for V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4: &str =
    "step-ep.params-ipa.krv4";
/// Canonical Ep processed proving-key package file name for V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4: &str =
    "step-ep.proving-key.krv4";
/// Canonical Ep processed verifier-key package file name for V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4: &str =
    "step-ep.verifying-key.krv4";
/// Canonical Ep final-key selector-zero bootstrap-witness package for V4 runtime.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4: &str =
    "step-ep.bootstrap-witness.krv4";
/// Exact ordered cryptographic artifact roles required by an ABI-21 release.
/// Capability order is Eq then Ep, with the four canonical artifact roles inside each profile.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4: [&str; 8] = [
    "step_eq_params_ipa",
    "step_eq_proving_key",
    "step_eq_verifying_key",
    "step_eq_bootstrap_witness",
    "step_ep_params_ipa",
    "step_ep_proving_key",
    "step_ep_verifying_key",
    "step_ep_bootstrap_witness",
];
/// Circuit/verifier role used by the compact Commit-QC plus anchor-path verifier.
pub const KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2: &str = "kagemusha-topup-finality-qc-merkle-v2";
/// Canonical release-manifest purpose of the trusted validator-roster artifact.
pub const KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2: &str = "topup_finality_roster";
/// Exact Norito type stored in the finality-roster artifact file.
pub const KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2: &str =
    "iroha_data_model::offline::model::KagemushaTopUpFinalityRosterArtifactV2";
/// Canonical release file name for the top-up finality roster.
pub const KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2: &str = "topup-finality-roster.norito";
/// Canonical V4 release name for the unchanged typed finality roster payload.
pub const KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4: &str = "topup-finality-roster-v4.norito";
/// Maximum roster size, pinned above one full 31-validator window by an exact wire-shape test.
pub const KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2: u64 = 2 * 1024 * 1024;
/// Native-width mirror of [`KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2`].
const KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_USIZE_V2: usize = 2 * 1024 * 1024;
/// Production-promotion gate for the ABI-21/V4 paired recursive backend.
/// Only explicitly promoted builds enable it; runtime still requires an authenticated V4 release
/// with the exact verifier/prover inventory, so this gate never substitutes for authentication.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE: bool =
    cfg!(feature = "kagemusha-production-enabled");
/// Canonical verifier-record namespace for Kagemusha proof admission.
pub const KAGEMUSHA_VERIFIER_NAMESPACE: &str = "offline_kagemusha";
/// Transparent backend used by the independent confidential transfer circuits.
pub const KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND: &str = "halo2/ipa";
include!("kagemusha_schema_hashes.rs");
/// Error returned when canonical Kagemusha data fails validation.
#[derive(Debug)]
pub enum KagemushaValidationError {
    /// Canonical Norito encoding failed.
    Encode(norito::Error),
    /// A required collection was empty.
    Empty,
    /// A bounded archive exceeded its protocol limit.
    EncodedSizeExceeded {
        /// Encoded byte length.
        actual: usize,
        /// Maximum accepted byte length.
        max: usize,
    },
    /// A public note descriptor was malformed.
    InvalidRecursiveSpendNote {
        /// Stable field label.
        field: &'static str,
    },
    /// A recursive proof or public binding was malformed.
    InvalidRecursiveSpendProof {
        /// Stable field label.
        field: &'static str,
    },
    /// Recursive inputs disagree on the asset definition.
    RecursiveSpendAssetMismatch,
    /// Recursive inputs disagree on the exact network.
    RecursiveSpendNetworkMismatch,
    /// A transition did not consume its parent nullifier.
    RecursiveSpendMissingPreviousNullifier,
    /// The authenticated paired-proof Pasta backend is not linked.
    RecursiveSpendV2ProofBackendUnavailable,
}
impl core::fmt::Display for KagemushaValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Encode(err) => write!(f, "canonical Kagemusha encoding failed: {err}"),
            Self::Empty => f.write_str("Kagemusha input collection must not be empty"),
            Self::EncodedSizeExceeded { actual, max } => {
                write!(f, "Kagemusha archive size {actual} exceeds limit {max}")
            }
            Self::InvalidRecursiveSpendNote { field } => {
                write!(f, "invalid Kagemusha note field `{field}`")
            }
            Self::InvalidRecursiveSpendProof { field } => {
                write!(f, "invalid Kagemusha proof field `{field}`")
            }
            Self::RecursiveSpendAssetMismatch => {
                f.write_str("Kagemusha recursive inputs use different assets")
            }
            Self::RecursiveSpendNetworkMismatch => {
                f.write_str("Kagemusha recursive inputs use different exact networks")
            }
            Self::RecursiveSpendMissingPreviousNullifier => {
                f.write_str("Kagemusha transition does not consume its parent nullifier")
            }
            Self::RecursiveSpendV2ProofBackendUnavailable => {
                f.write_str("authenticated Kagemusha recursive proof backend is unavailable")
            }
        }
    }
}
impl std::error::Error for KagemushaValidationError {}
impl From<norito::Error> for KagemushaValidationError {
    fn from(err: norito::Error) -> Self {
        Self::Encode(err)
    }
}
fn ensure_kagemusha_encoded_size_at_most(
    actual: usize,
    max: usize,
) -> Result<(), KagemushaValidationError> {
    if actual > max {
        return Err(KagemushaValidationError::EncodedSizeExceeded { actual, max });
    }
    Ok(())
}
/// Stable failure returned while authenticating or promoting a V4 artifact release.
#[allow(variant_size_differences)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KagemushaReleaseVerificationError {
    /// The release manifest fails its versioned structural contract.
    InvalidManifest,
    /// The locally trusted role policy is malformed.
    InvalidPolicy,
    /// The signed release envelope is malformed or does not bind the manifest.
    InvalidAttestation,
    /// The cryptographic review is non-canonical, incomplete, rejected, or mis-bound.
    InvalidCryptographicReview,
    /// The runner-signed internal-validation receipt is absent, non-canonical, invalid, or mis-bound.
    InvalidInternalValidationReceipt,
    /// A supplied evidence file is empty, oversized, or has the wrong digest.
    EvidenceMismatch {
        /// Evidence role with invalid content.
        role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
    },
    /// An approval signer is not authorized for its claimed role.
    UnknownSigner {
        /// Claimed approval role.
        role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
    },
    /// The same role/signer identity appears more than once or out of order.
    DuplicateOrUnorderedSigner,
    /// A role-specific approval signature failed cryptographic verification.
    InvalidSignature {
        /// Claimed approval role.
        role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
    },
    /// A role did not collect the threshold required by the trusted policy.
    InsufficientThreshold {
        /// Approval role below threshold.
        role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
        /// Number of valid distinct approvals collected.
        collected: u16,
        /// Required threshold.
        required: u16,
    },
    /// A promotion record disagrees with the authenticated release or runtime status.
    InvalidPromotionRecord,
}
impl KagemushaReleaseVerificationError {
    /// Stable machine-readable rejection code for deployment automation.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidManifest => "invalid_manifest",
            Self::InvalidPolicy => "invalid_policy",
            Self::InvalidAttestation => "invalid_attestation",
            Self::InvalidCryptographicReview => "invalid_cryptographic_review",
            Self::InvalidInternalValidationReceipt => "invalid_internal_validation_receipt",
            Self::EvidenceMismatch { .. } => "evidence_mismatch",
            Self::UnknownSigner { .. } => "unknown_signer",
            Self::DuplicateOrUnorderedSigner => "duplicate_or_unordered_signer",
            Self::InvalidSignature { .. } => "invalid_signature",
            Self::InsufficientThreshold { .. } => "insufficient_threshold",
            Self::InvalidPromotionRecord => "invalid_promotion_record",
        }
    }
}
impl core::fmt::Display for KagemushaReleaseVerificationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidManifest => f.write_str("invalid Kagemusha artifact manifest"),
            Self::InvalidPolicy => f.write_str("invalid Kagemusha release policy"),
            Self::InvalidAttestation => {
                f.write_str("invalid or mismatched Kagemusha release attestation")
            }
            Self::InvalidCryptographicReview => {
                f.write_str("invalid or mismatched Kagemusha cryptographic review")
            }
            Self::InvalidInternalValidationReceipt => {
                f.write_str("invalid or mismatched Kagemusha internal-validation receipt")
            }
            Self::EvidenceMismatch { role } => {
                write!(f, "Kagemusha release evidence mismatch for {role:?}")
            }
            Self::UnknownSigner { role } => {
                write!(f, "unknown Kagemusha release signer for {role:?}")
            }
            Self::DuplicateOrUnorderedSigner => {
                f.write_str("Kagemusha release signers are duplicated or not canonically ordered")
            }
            Self::InvalidSignature { role } => {
                write!(f, "invalid Kagemusha release signature for {role:?}")
            }
            Self::InsufficientThreshold {
                role,
                collected,
                required,
            } => write!(
                f,
                "Kagemusha release threshold for {role:?} is {collected}, requires {required}"
            ),
            Self::InvalidPromotionRecord => {
                f.write_str("invalid Kagemusha promoted-release record")
            }
        }
    }
}
impl std::error::Error for KagemushaReleaseVerificationError {}
/// Runtime proof that a V4 manifest, evidence set, and role thresholds were authenticated.
/// Private fields keep unsigned material out of the configured ABI-21 catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaAuthenticatedReleaseV4 {
    manifest: KagemushaRecursiveSpendArtifactManifestV4,
    manifest_sha256: [u8; 32],
    release_attestation_sha256: [u8; 32],
    release_policy_sha256: [u8; 32],
    approved_signers: Vec<KagemushaRecursiveSpendApprovedSignerV1>,
}
fn kagemusha_poseidon_preimage<T: Encode>(
    value: &T,
) -> Result<[u8; Hash::LENGTH], KagemushaValidationError> {
    Ok(iroha_zkp_halo2::poseidon::hash_bytes(
        &norito::encode_canonical(value)?,
    ))
}
fn validate_kagemusha_root(
    field: &'static str,
    root: [u8; Hash::LENGTH],
) -> Result<(), KagemushaValidationError> {
    if root == [0; Hash::LENGTH] {
        return Err(KagemushaValidationError::InvalidRecursiveSpendProof { field });
    }
    Ok(())
}
/// Derive the deterministic Kagemusha escrow account for an asset definition.
#[must_use]
pub fn offline_escrow_account_id(
    network_id: &NetworkId,
    definition_id: &AssetDefinitionId,
) -> AccountId {
    let definition_id = definition_id.to_string();
    AccountId::new(derive_non_signing_ed25519_public_key(
        OFFLINE_ESCROW_ACCOUNT_DOMAIN.as_bytes(),
        &[network_id.as_bytes(), definition_id.as_bytes()],
    ))
}
#[cfg(test)]
fn kagemusha_test_network_id(seed: impl AsRef<[u8]>) -> NetworkId {
    NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(Hash::new(seed)),
    )
}
#[cfg(test)]
mod offline_escrow_account_tests {
    use super::*;
    use crate::domain::DomainId;
    fn test_network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(Hash::new([
                seed,
            ])),
        )
    }
    #[test]
    fn derivation_is_stable_without_a_public_signing_seed() {
        let network_id = test_network_id(1);
        let definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "xor".parse().expect("asset name"),
        );
        let custody = offline_escrow_account_id(&network_id, &definition_id);
        assert_eq!(
            custody,
            offline_escrow_account_id(&network_id, &definition_id)
        );
        let legacy_seed_material =
            format!("iroha.offline.escrow|offline-custody-chain|{definition_id}");
        let legacy_seed: [u8; Hash::LENGTH] = Hash::new(legacy_seed_material).into();
        let legacy_keypair = KeyPair::try_from_seed(legacy_seed.to_vec(), Algorithm::Ed25519)
            .expect("legacy public seed derives");
        assert_ne!(
            custody,
            AccountId::new(legacy_keypair.public_key().clone()),
            "offline custody must not expose a signing key through public seed derivation"
        );
        assert_ne!(
            custody,
            offline_escrow_account_id(&test_network_id(2), &definition_id),
            "different genesis hashes must derive different escrow accounts"
        );
    }
}
include!("kagemusha_model.rs");
mod kagemusha_release_verifier;
use kagemusha_release_verifier::verifying_key_commitment_v1;
pub use kagemusha_release_verifier::{
    kagemusha_recursive_spend_verifier_key_id_v4,
    kagemusha_recursive_spend_verifier_owner_manifest_id_v4,
    kagemusha_recursive_spend_verifier_public_inputs_schema_hash_v4,
};
/// On-chain platform-attested registration for a Kagemusha device key.
///
/// This is the device-bound trust anchor used by top-up and redemption authorization. The report
/// and evidence bytes are included so consensus has enough material to perform deterministic
/// platform checks; the hashes provide stable replay keys and compact audit anchors.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineDeviceAttestationRegistration {
    /// Registration format marker.
    pub version: u16,
    /// Platform class, for example `ios-appattest` or `android-keymint`.
    pub platform: String,
    /// Issuer-scoped one-use key identifier.
    pub key_id: String,
    /// Device identifier bound by the platform attestation.
    pub device_id: String,
    /// Account authorized to control the note key.
    pub account_id: AccountId,
    /// Optional asset definition this attestation is intended for.
    pub asset_definition_id: Option<AssetDefinitionId>,
    /// Apple Developer Team ID for iOS App Attest registrations.
    pub ios_team_id: Option<String>,
    /// iOS bundle identifier for App Attest registrations.
    pub ios_bundle_id: Option<String>,
    /// iOS App Attest environment, either `production` or `development`.
    pub ios_environment: Option<String>,
    /// Android package name expected in the `KeyMint` attestation application id.
    pub android_package_name: Option<String>,
    /// Android signing certificate SHA-256 expected in the `KeyMint` attestation application id.
    pub android_signing_certificate_sha256: Option<Vec<u8>>,
    /// Fixed P-256 device authority authenticated by this registration.
    pub public_key: KagemushaDevicePublicKeyV2,
    /// Hardware assertion scheme bound to this note key.
    pub assertion_scheme: String,
    /// Hardware assertion key algorithm, for example `ecdsa-p256-sha256`.
    pub assertion_key_algorithm: String,
    /// Hardware assertion public key bytes, for example SEC1 P-256.
    pub assertion_public_key: Vec<u8>,
    /// Hardware one-use limit when the platform exposes it.
    pub assertion_usage_count_limit: Option<u32>,
    /// True when the submitted evidence claims hardware one-use semantics.
    pub one_use: bool,
    /// Canonical challenge hash signed or embedded by the platform attestation.
    pub challenge_hash: Hash,
    /// Hash of the raw App Attest or `KeyMint` report submitted to the verifier.
    pub attestation_report_hash: Hash,
    /// Raw App Attest or `KeyMint` report bytes submitted for on-chain verification.
    pub attestation_report: Vec<u8>,
    /// Hash of the full platform evidence bundle.
    pub evidence_hash: Hash,
    /// Full platform evidence bundle bytes submitted for on-chain verification.
    pub evidence: Vec<u8>,
    /// Recent committed block height bound into the challenge.
    pub recent_block_height: u64,
    /// Recent committed block hash bound into the challenge.
    pub recent_block_hash: Hash,
    /// Registration validity limit in Unix milliseconds.
    pub expires_at_ms: u64,
}
/// Android `KeyMint` challenge inputs available before the attested key is generated.
///
/// Android derives the final registration `key_id` from the public key created by
/// `KeyMint`. Consequently, this first-phase challenge deliberately has no `key_id`
/// or assertion-public-key field. Consensus later validates both values against the
/// returned certificate chain before accepting the registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineAndroidKeyMintChallenge {
    /// Registration format marker.
    pub version: u16,
    /// Device identifier bound by the platform attestation.
    pub device_id: String,
    /// Account authorized to control the note key.
    pub account_id: AccountId,
    /// Optional asset definition this attestation is intended for.
    pub asset_definition_id: Option<AssetDefinitionId>,
    /// Optional iOS team metadata retained in the registration schema.
    pub ios_team_id: Option<String>,
    /// Optional iOS bundle metadata retained in the registration schema.
    pub ios_bundle_id: Option<String>,
    /// Optional iOS environment metadata retained in the registration schema.
    pub ios_environment: Option<String>,
    /// Android package name expected in the `KeyMint` attestation application id.
    pub android_package_name: Option<String>,
    /// Android signing-certificate SHA-256 expected in the attestation application id.
    pub android_signing_certificate_sha256: Option<Vec<u8>>,
    /// Ed25519 public key bytes for local note/proof signatures.
    pub public_key: KagemushaDevicePublicKeyV2,
    /// Hardware assertion scheme bound to this note key.
    pub assertion_scheme: String,
    /// Hardware assertion key algorithm.
    pub assertion_key_algorithm: String,
    /// Hardware one-use limit exposed by `KeyMint`.
    pub assertion_usage_count_limit: Option<u32>,
    /// True when the submitted evidence claims hardware one-use semantics.
    pub one_use: bool,
    /// Recent committed block height bound into the challenge.
    pub recent_block_height: u64,
    /// Recent committed block hash bound into the challenge.
    pub recent_block_hash: Hash,
    /// Registration validity limit in Unix milliseconds.
    pub expires_at_ms: u64,
}
include!("device_attestation_policy.rs");
#[derive(Debug, Clone, Decode, Encode)]
struct OfflineDeviceAttestationChallengePreimage {
    domain: String,
    version: u16,
    platform: String,
    key_id: String,
    device_id: String,
    account_id: AccountId,
    asset_definition_id: Option<AssetDefinitionId>,
    ios_team_id: Option<String>,
    ios_bundle_id: Option<String>,
    ios_environment: Option<String>,
    android_package_name: Option<String>,
    android_signing_certificate_sha256: Option<Vec<u8>>,
    public_key: KagemushaDevicePublicKeyV2,
    assertion_scheme: String,
    assertion_key_algorithm: String,
    assertion_usage_count_limit: Option<u32>,
    one_use: bool,
    recent_block_height: u64,
    recent_block_hash: Hash,
    expires_at_ms: u64,
}
/// `KeyMint` uses this separate schema because `key_id` is derived from the key
/// that Android creates while processing this challenge.
#[derive(Debug, Clone, Decode, Encode)]
struct OfflineAndroidKeyMintChallengePreimage {
    domain: String,
    version: u16,
    platform: String,
    device_id: String,
    account_id: AccountId,
    asset_definition_id: Option<AssetDefinitionId>,
    ios_team_id: Option<String>,
    ios_bundle_id: Option<String>,
    ios_environment: Option<String>,
    android_package_name: Option<String>,
    android_signing_certificate_sha256: Option<Vec<u8>>,
    public_key: KagemushaDevicePublicKeyV2,
    assertion_scheme: String,
    assertion_key_algorithm: String,
    assertion_usage_count_limit: Option<u32>,
    one_use: bool,
    recent_block_height: u64,
    recent_block_hash: Hash,
    expires_at_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRecipientPaymentRequestDigestPreimageV2 {
    domain: String,
    request: KagemushaRecipientPaymentRequestV2,
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRecipientPaymentRequestSigningPreimageV2 {
    domain: String,
    payload: KagemushaRecipientPaymentRequestSigningPayloadV2,
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRequestAuthorizationSigningPreimageV2 {
    domain: String,
    authority: AccountId,
    device_id: String,
    asset_definition_id: AssetDefinitionId,
    operation_id: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
    nonce: [u8; 32],
    payload_digest: [u8; 32],
    registration_hash: [u8; 32],
    platform: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaReceiverKeyReferencePreimageV2 {
    domain: String,
    receiver_public_key: KagemushaDevicePublicKeyV2,
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaReceiverAcknowledgementSigningPreimageV2 {
    domain: String,
    payload: KagemushaReceiverAcknowledgementPayloadV2,
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaReceiverAcknowledgementDigestPreimageV2 {
    domain: String,
    acknowledgement: KagemushaReceiverAcknowledgementV2,
}
impl norito::NoritoSerialize for KagemushaDevicePublicKeyV2 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.validate()
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        writer.write_all(&self.0)?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V2)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}
impl<'de> norito::NoritoDeserialize<'de> for KagemushaDevicePublicKeyV2 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("Kagemusha device public key must decode from canonical SEC1 bytes")
    }
    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let bytes =
            norito::core::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
        let (value, used) = <Self as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(norito::Error::LengthMismatch);
        }
        Ok(value)
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for KagemushaDevicePublicKeyV2 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        let raw = bytes
            .get(..KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V2)
            .ok_or(norito::Error::LengthMismatch)?;
        let value = Self::from_sec1_bytes(raw)
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        Ok((value, KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V2))
    }
}
impl norito::NoritoSerialize for KagemushaDeviceSignatureV2 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.validate()
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        writer.write_all(&self.0)?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}
impl<'de> norito::NoritoDeserialize<'de> for KagemushaDeviceSignatureV2 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("Kagemusha device signature must decode from canonical raw P-256 bytes")
    }
    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let bytes =
            norito::core::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
        let (value, used) = <Self as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(norito::Error::LengthMismatch);
        }
        Ok(value)
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for KagemushaDeviceSignatureV2 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        let raw = bytes
            .get(..KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2)
            .ok_or(norito::Error::LengthMismatch)?;
        let value =
            Self::from_raw_bytes(raw).map_err(|error| norito::Error::Message(error.to_string()))?;
        Ok((value, KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2))
    }
}
impl KagemushaDevicePublicKeyV2 {
    /// Parse and validate the sole canonical Kagemusha device-key encoding.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn from_sec1_bytes(bytes: &[u8]) -> Result<Self, KagemushaValidationError> {
        let raw: [u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V2] =
            bytes
                .try_into()
                .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "device_public_key",
                })?;
        if raw[0] != 0x04 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "device_public_key",
            });
        }
        let verifying_key = P256VerifyingKey::from_sec1_bytes(&raw).map_err(|_| {
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "device_public_key",
            }
        })?;
        if verifying_key.to_encoded_point(false).as_bytes() != raw {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "device_public_key",
            });
        }
        Ok(Self(raw))
    }
    /// Validate a value obtained through a raw Norito or JSON decoder.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        Self::from_sec1_bytes(&self.0).map(|_| ())
    }
    /// Return the canonical uncompressed SEC1 bytes.
    #[must_use]
    pub const fn as_sec1_bytes(&self) -> &[u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V2] {
        &self.0
    }
    fn verifying_key(&self) -> Result<P256VerifyingKey, KagemushaValidationError> {
        self.validate()?;
        P256VerifyingKey::from_sec1_bytes(&self.0).map_err(|_| {
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "device_public_key",
            }
        })
    }
}
impl TryFrom<&[u8]> for KagemushaDevicePublicKeyV2 {
    type Error = KagemushaValidationError;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Self::from_sec1_bytes(value)
    }
}
impl TryFrom<[u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V2]> for KagemushaDevicePublicKeyV2 {
    type Error = KagemushaValidationError;
    fn try_from(
        value: [u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V2],
    ) -> Result<Self, Self::Error> {
        Self::from_sec1_bytes(&value)
    }
}
impl AsRef<[u8]> for KagemushaDevicePublicKeyV2 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
impl KagemushaDeviceSignatureV2 {
    /// Parse a canonical fixed-width low-S P-256 ECDSA signature.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn from_raw_bytes(bytes: &[u8]) -> Result<Self, KagemushaValidationError> {
        let raw: [u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2] =
            bytes
                .try_into()
                .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "device_signature",
                })?;
        let signature = P256Signature::from_slice(&raw).map_err(|_| {
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "device_signature",
            }
        })?;
        if signature.normalize_s().is_some() {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "device_signature",
            });
        }
        Ok(Self(raw))
    }
    /// Validate a value obtained through a raw Norito or JSON decoder.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        Self::from_raw_bytes(&self.0).map(|_| ())
    }
    /// Return the canonical fixed-width `r || s` bytes.
    #[must_use]
    pub const fn as_raw_bytes(&self) -> &[u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2] {
        &self.0
    }
    /// Verify ECDSA-P256-SHA256 under the fixed Kagemusha authority profile.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when structural, policy, signature, or cryptographic authentication fails.
    pub fn verify(
        &self,
        public_key: &KagemushaDevicePublicKeyV2,
        message: &[u8],
    ) -> Result<(), KagemushaValidationError> {
        self.validate()?;
        let signature = P256Signature::from_slice(&self.0).map_err(|_| {
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "device_signature",
            }
        })?;
        public_key
            .verifying_key()?
            .verify(message, &signature)
            .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "device_signature",
            })
    }
}
impl TryFrom<&[u8]> for KagemushaDeviceSignatureV2 {
    type Error = KagemushaValidationError;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Self::from_raw_bytes(value)
    }
}
impl TryFrom<[u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2]> for KagemushaDeviceSignatureV2 {
    type Error = KagemushaValidationError;
    fn try_from(value: [u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2]) -> Result<Self, Self::Error> {
        Self::from_raw_bytes(&value)
    }
}
impl AsRef<[u8]> for KagemushaDeviceSignatureV2 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaUnshieldPublicInputsDigestPreimageV2 {
    domain: String,
    public_inputs: KagemushaUnshieldPublicInputsBindingV2,
}
impl OfflineDeviceAttestationRegistration {
    fn challenge_preimage(&self) -> OfflineDeviceAttestationChallengePreimage {
        OfflineDeviceAttestationChallengePreimage {
            domain: OFFLINE_DEVICE_ATTESTATION_CHALLENGE_DOMAIN.to_owned(),
            version: self.version,
            platform: self.platform.clone(),
            key_id: self.key_id.clone(),
            device_id: self.device_id.clone(),
            account_id: self.account_id.clone(),
            asset_definition_id: self.asset_definition_id.clone(),
            ios_team_id: self.ios_team_id.clone(),
            ios_bundle_id: self.ios_bundle_id.clone(),
            ios_environment: self.ios_environment.clone(),
            android_package_name: self.android_package_name.clone(),
            android_signing_certificate_sha256: self.android_signing_certificate_sha256.clone(),
            public_key: self.public_key,
            assertion_scheme: self.assertion_scheme.clone(),
            assertion_key_algorithm: self.assertion_key_algorithm.clone(),
            assertion_usage_count_limit: self.assertion_usage_count_limit,
            one_use: self.one_use,
            recent_block_height: self.recent_block_height,
            recent_block_hash: self.recent_block_hash,
            expires_at_ms: self.expires_at_ms,
        }
    }
    fn android_keymint_challenge_preimage(&self) -> OfflineAndroidKeyMintChallengePreimage {
        OfflineAndroidKeyMintChallengePreimage {
            domain: OFFLINE_DEVICE_ATTESTATION_CHALLENGE_DOMAIN.to_owned(),
            version: self.version,
            platform: self.platform.clone(),
            device_id: self.device_id.clone(),
            account_id: self.account_id.clone(),
            asset_definition_id: self.asset_definition_id.clone(),
            ios_team_id: self.ios_team_id.clone(),
            ios_bundle_id: self.ios_bundle_id.clone(),
            ios_environment: self.ios_environment.clone(),
            android_package_name: self.android_package_name.clone(),
            android_signing_certificate_sha256: self.android_signing_certificate_sha256.clone(),
            public_key: self.public_key,
            assertion_scheme: self.assertion_scheme.clone(),
            assertion_key_algorithm: self.assertion_key_algorithm.clone(),
            assertion_usage_count_limit: self.assertion_usage_count_limit,
            one_use: self.one_use,
            recent_block_height: self.recent_block_height,
            recent_block_hash: self.recent_block_hash,
            expires_at_ms: self.expires_at_ms,
        }
    }
    /// Deterministic challenge hash that platform attestation evidence must bind.
    ///
    /// The preimage intentionally excludes the attestation report, evidence hashes, and assertion
    /// public key because those values are learned from the platform response after the challenge
    /// is created. Android `KeyMint` additionally uses a platform-specific preimage without
    /// `key_id`, because its canonical key id is the SHA-256 of that not-yet-generated assertion
    /// public key. Admission binds the reported credential/certificate public key to
    /// `assertion_public_key` and then validates `key_id` before constructing the key certificate.
    ///
    /// # Errors
    ///
    /// Returns an error when the challenge preimage cannot be serialized with Norito.
    pub fn canonical_challenge_hash(&self) -> Result<Hash, norito::Error> {
        if self.platform == OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_PLATFORM {
            return norito::encode_canonical(&self.android_keymint_challenge_preimage())
                .map(Hash::new);
        }
        norito::encode_canonical(&self.challenge_preimage()).map(Hash::new)
    }
}
impl OfflineAndroidKeyMintChallenge {
    fn challenge_preimage(&self) -> OfflineAndroidKeyMintChallengePreimage {
        OfflineAndroidKeyMintChallengePreimage {
            domain: OFFLINE_DEVICE_ATTESTATION_CHALLENGE_DOMAIN.to_owned(),
            version: self.version,
            platform: OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_PLATFORM.to_owned(),
            device_id: self.device_id.clone(),
            account_id: self.account_id.clone(),
            asset_definition_id: self.asset_definition_id.clone(),
            ios_team_id: self.ios_team_id.clone(),
            ios_bundle_id: self.ios_bundle_id.clone(),
            ios_environment: self.ios_environment.clone(),
            android_package_name: self.android_package_name.clone(),
            android_signing_certificate_sha256: self.android_signing_certificate_sha256.clone(),
            public_key: self.public_key,
            assertion_scheme: self.assertion_scheme.clone(),
            assertion_key_algorithm: self.assertion_key_algorithm.clone(),
            assertion_usage_count_limit: self.assertion_usage_count_limit,
            one_use: self.one_use,
            recent_block_height: self.recent_block_height,
            recent_block_hash: self.recent_block_hash,
            expires_at_ms: self.expires_at_ms,
        }
    }
    /// Return the canonical Norito preimage bytes embedded into the `KeyMint` challenge hash.
    ///
    /// # Errors
    ///
    /// Returns an error when the challenge preimage cannot be serialized with Norito.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        norito::encode_canonical(&self.challenge_preimage())
    }
    /// Return the canonical challenge hash Android supplies before generating the key.
    ///
    /// # Errors
    ///
    /// Returns an error when the challenge preimage cannot be serialized with Norito.
    pub fn canonical_challenge_hash(&self) -> Result<Hash, norito::Error> {
        self.canonical_bytes().map(Hash::new)
    }
}
impl KagemushaSpendableNoteDescriptorV2 {
    /// Validate exact amount plus disjoint, non-zero note material.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.amount.validate()?;
        if !is_kagemusha_network_id(&self.network_id) {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "network_id",
            });
        }
        if self.note_commitment == [0; 32] {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "note_commitment",
            });
        }
        if self.spend_nullifier == [0; 32] || self.spend_nullifier == self.note_commitment {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "spend_nullifier",
            });
        }
        Ok(())
    }
}
/// Return the canonical root shared by every branch of one finalized top-up.
///
/// The lineage root is deliberately the complete finalized-anchor digest. This
/// removes a second identity derivation and lets compact peer references bind
/// branch conflict history one-to-one to chain-resolved provenance.
///
/// # Errors
///
/// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
pub fn kagemusha_recursive_spend_lineage_root_v2(
    anchor_digest: [u8; 32],
) -> Result<[u8; 32], KagemushaValidationError> {
    if anchor_digest == [0; 32] {
        return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "topup_anchor_ref.anchor_digest",
        });
    }
    Ok(anchor_digest)
}
/// Derive the compact transition-choice tag retained by descendant claims.
///
/// # Errors
///
/// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
pub fn kagemusha_recursive_spend_transition_tag_v2(
    transition_binding: [u8; 32],
) -> Result<[u8; 24], KagemushaValidationError> {
    if transition_binding == [0; 32] {
        return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "branch_claim.transition_binding",
        });
    }
    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_DOMAIN_V2.as_bytes());
    hasher.update([0]);
    hasher.update(transition_binding);
    let digest = hasher.finalize();
    let mut tag = [0_u8; 24];
    tag.copy_from_slice(&digest[..24]);
    if tag == [0; 24] {
        return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "branch_claim.transition_tag",
        });
    }
    Ok(tag)
}
impl KagemushaRecursiveSpendBranchPathV2 {
    /// Construct the root coordinate for a top-up lineage.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the branch path or claim is invalid or the requested derivation exceeds its canonical bounds.
    pub fn root(lineage_root: [u8; 32]) -> Result<Self, KagemushaValidationError> {
        let path = Self {
            lineage_root,
            depth: 0,
            path_bits: [0; 8],
        };
        path.validate()?;
        Ok(path)
    }
    /// Append the deterministic recipient (`0`) or change (`1`) branch bit.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the branch path or claim is invalid or the requested derivation exceeds its canonical bounds.
    pub fn child(
        self,
        branch: KagemushaRecursiveSpendBranchV2,
    ) -> Result<Self, KagemushaValidationError> {
        self.validate()?;
        if self.depth == KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "branch_path.depth",
            });
        }
        let mut child = self;
        if matches!(branch, KagemushaRecursiveSpendBranchV2::Change) {
            let bit_index = usize::from(child.depth);
            child.path_bits[bit_index / 8] |= 1 << (7 - (bit_index % 8));
        }
        child.depth += 1;
        child.validate()?;
        Ok(child)
    }
    /// Return the canonical parent coordinate, or `None` for a lineage root.
    #[must_use]
    pub fn parent(self) -> Option<Self> {
        self.validate().ok()?;
        if self.depth == 0 {
            return None;
        }
        let mut parent = self;
        parent.depth -= 1;
        let bit_index = usize::from(parent.depth);
        parent.path_bits[bit_index / 8] &= !(1 << (7 - (bit_index % 8)));
        parent.validate().ok()?;
        Some(parent)
    }
    /// Return the canonical prefix at `depth`.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the branch path or claim is invalid or the requested derivation exceeds its canonical bounds.
    pub fn prefix(self, depth: u8) -> Result<Self, KagemushaValidationError> {
        self.validate()?;
        if depth > self.depth {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "branch_path.prefix_depth",
            });
        }
        let mut prefix = self;
        prefix.depth = depth;
        let full_bytes = usize::from(depth / 8);
        let partial_bits = depth % 8;
        if partial_bits == 0 {
            prefix.path_bits[full_bytes..].fill(0);
        } else {
            prefix.path_bits[full_bytes] &= u8::MAX << (8 - partial_bits);
            prefix.path_bits[full_bytes + 1..].fill(0);
        }
        prefix.validate()?;
        Ok(prefix)
    }
    /// Validate the lineage root, depth, and canonical zeroed unused bits.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(self) -> Result<(), KagemushaValidationError> {
        if self.lineage_root == [0; 32] {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "branch_path.lineage_root",
            });
        }
        if self.depth > KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "branch_path.depth",
            });
        }
        let full_bytes = usize::from(self.depth / 8);
        let partial_bits = self.depth % 8;
        if partial_bits == 0 {
            if self.path_bits[full_bytes..].iter().any(|byte| *byte != 0) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "branch_path.path_bits",
                });
            }
        } else {
            let unused_mask = (1_u8 << (8 - partial_bits)) - 1;
            if self.path_bits[full_bytes] & unused_mask != 0
                || self.path_bits[full_bytes + 1..]
                    .iter()
                    .any(|byte| *byte != 0)
            {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "branch_path.path_bits",
                });
            }
        }
        Ok(())
    }
    /// Return whether this coordinate is an ancestor of (or equal to) `other`.
    #[must_use]
    pub fn is_prefix_of(self, other: Self) -> bool {
        if self.validate().is_err()
            || other.validate().is_err()
            || self.lineage_root != other.lineage_root
            || self.depth > other.depth
        {
            return false;
        }
        let full_bytes = usize::from(self.depth / 8);
        if self.path_bits[..full_bytes] != other.path_bits[..full_bytes] {
            return false;
        }
        let partial_bits = self.depth % 8;
        if partial_bits == 0 {
            return true;
        }
        let mask = u8::MAX << (8 - partial_bits);
        self.path_bits[full_bytes] & mask == other.path_bits[full_bytes] & mask
    }
    /// Return whether two redemption coordinates conflict.
    ///
    /// Equal paths and ancestor/descendant pairs conflict; siblings and paths
    /// from different top-up lineages do not.
    #[must_use]
    pub fn conflicts_with(self, other: Self) -> bool {
        self.is_prefix_of(other) || other.is_prefix_of(self)
    }
}
impl KagemushaRecursiveSpendBranchClaimV2 {
    /// Construct a root claim with an empty transition history.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the branch path or claim is invalid or the requested derivation exceeds its canonical bounds.
    pub fn root(lineage_root: [u8; 32]) -> Result<Self, KagemushaValidationError> {
        let claim = Self {
            path: KagemushaRecursiveSpendBranchPathV2::root(lineage_root)?,
            transition_tags: Vec::new(),
        };
        claim.validate()?;
        Ok(claim)
    }
    /// Append one output edge and bind it to the exact producing transition.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the branch path or claim is invalid or the requested derivation exceeds its canonical bounds.
    pub fn child(
        &self,
        branch: KagemushaRecursiveSpendBranchV2,
        transition_binding: [u8; 32],
    ) -> Result<Self, KagemushaValidationError> {
        self.validate()?;
        let transition_tag = kagemusha_recursive_spend_transition_tag_v2(transition_binding)?;
        let mut child = self.clone();
        child.path = self.path.child(branch)?;
        child.transition_tags.extend_from_slice(&transition_tag);
        child.validate()?;
        Ok(child)
    }
    /// Return the canonical ancestor claim at `depth`.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the branch path or claim is invalid or the requested derivation exceeds its canonical bounds.
    pub fn prefix(&self, depth: u8) -> Result<Self, KagemushaValidationError> {
        self.validate()?;
        let path = self.path.prefix(depth)?;
        let history_len = usize::from(depth)
            .checked_mul(KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2)
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "branch_claim.transition_tags.length",
            })?;
        let prefix = Self {
            path,
            transition_tags: self.transition_tags[..history_len].to_vec(),
        };
        prefix.validate()?;
        Ok(prefix)
    }
    /// Validate the path and its exact-depth edge history.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        self.path.validate()?;
        let expected_len = usize::from(self.path.depth)
            .checked_mul(KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2)
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "branch_claim.transition_tags.length",
            })?;
        if self.transition_tags.len() != expected_len {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "branch_claim.transition_tags.length",
            });
        }
        if self
            .transition_tags
            .chunks_exact(KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2)
            .any(|tag| tag.iter().all(|byte| *byte == 0))
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "branch_claim.transition_tags",
            });
        }
        Ok(())
    }
    /// Return the selected transition tag for the edge from `parent_depth`.
    #[must_use]
    pub fn transition_tag_at(&self, parent_depth: u8) -> Option<[u8; 24]> {
        self.validate().ok()?;
        if parent_depth >= self.path.depth {
            return None;
        }
        let start = usize::from(parent_depth)
            .checked_mul(KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2)?;
        let end = start.checked_add(KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2)?;
        self.transition_tags.get(start..end)?.try_into().ok()
    }
    fn transition_history_conflicts_with(
        &self,
        other: &Self,
    ) -> Result<bool, KagemushaValidationError> {
        self.validate()?;
        other.validate()?;
        if self.path.lineage_root != other.path.lineage_root {
            return Ok(false);
        }
        let shared_depth = self.path.depth.min(other.path.depth);
        for parent_depth in 0..shared_depth {
            if self.path.prefix(parent_depth)? == other.path.prefix(parent_depth)?
                && self.transition_tag_at(parent_depth) != other.transition_tag_at(parent_depth)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    /// Return whether two claims select overlapping value or incompatible transition histories.
    ///
    /// In addition to equal and ancestor/descendant coordinates, sibling
    /// outputs from different proof-bound transitions of the same parent
    /// conflict. This prevents mixing outputs from alternative splits.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when either branch claim is malformed or cannot be compared canonically.
    pub fn conflicts_with(&self, other: &Self) -> Result<bool, KagemushaValidationError> {
        self.validate()?;
        other.validate()?;
        Ok(
            self.path.conflicts_with(other.path)
                || self.transition_history_conflicts_with(other)?,
        )
    }
}
fn validate_kagemusha_recursive_spend_branch_claims_v2(
    claims: &[KagemushaRecursiveSpendBranchClaimV2],
) -> Result<(), KagemushaValidationError> {
    if claims.is_empty() || claims.len() > KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_CLAIMS_V2 {
        return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "branch_claims",
        });
    }
    for (index, claim) in claims.iter().enumerate() {
        claim.validate()?;
        if index > 0 && claims[index - 1].path >= claim.path {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "branch_claims.order",
            });
        }
        if claims[..index]
            .iter()
            .any(|previous| previous.path.conflicts_with(claim.path))
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "branch_claims.conflict",
            });
        }
        for previous in &claims[..index] {
            if previous.transition_history_conflicts_with(claim)? {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "branch_claims.transition_choice",
                });
            }
        }
    }
    Ok(())
}
impl KagemushaRecipientOutputDerivationRequestV2 {
    /// Validate the public, secret-free derivation context.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        self.amount.validate()?;
        if !is_kagemusha_network_id(&self.network_id) {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "recipient_output_derivation.network_id",
            });
        }
        if self.request_id == [0; 32] {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "recipient_output_derivation.request_id",
            });
        }
        Ok(())
    }
}
impl KagemushaRecipientOutputDerivationResultV2 {
    /// Validate the native result against the exact public derivation request.
    ///
    /// Native implementations additionally decode the opaque prover material before returning it
    /// and enforce that its schema contains no receiver spend secret or output diversifier.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_request(
        &self,
        request: &KagemushaRecipientOutputDerivationRequestV2,
    ) -> Result<(), KagemushaValidationError> {
        request.validate()?;
        self.recipient_output.validate_public_binding()?;
        if self.recipient_output.network_id != request.network_id {
            return Err(KagemushaValidationError::RecursiveSpendNetworkMismatch);
        }
        if self.recipient_output.asset != request.asset {
            return Err(KagemushaValidationError::RecursiveSpendAssetMismatch);
        }
        if self.recipient_output.amount != request.amount {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "recipient_output_derivation.amount",
            });
        }
        if self.sender_output_prover_material.is_empty()
            || self.sender_output_prover_material.len() > 4 * 1024
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "recipient_output_derivation.prover_material",
            });
        }
        Ok(())
    }
}
impl KagemushaRecipientPaymentRequestSigningPayloadV2 {
    /// Validate unsigned request fields before device signing.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.receiver_public_key.validate()?;
        self.amount.validate()?;
        self.recipient_output.validate_public_binding()?;
        if self.recipient_output.network_id != self.network_id {
            return Err(KagemushaValidationError::RecursiveSpendNetworkMismatch);
        }
        if self.recipient_output.asset != self.asset {
            return Err(KagemushaValidationError::RecursiveSpendAssetMismatch);
        }
        if self.recipient_key_reference
            != kagemusha_receiver_key_reference_v2(&self.receiver_public_key)?
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "recipient_key_reference",
            });
        }
        if self.receiver_device_id.is_empty()
            || self.receiver_device_id.len() > 128
            || self.receiver_device_id.trim() != self.receiver_device_id
            || self.receiver_device_id.chars().any(char::is_control)
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "receiver_device_id",
            });
        }
        if self.request_id == [0; 32] {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "request_id",
            });
        }
        if self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "expires_at_ms",
            });
        }
        if self.recipient_output.amount != self.amount {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "recipient_output.amount",
            });
        }
        if self.sender_output_prover_material.is_empty()
            || self.sender_output_prover_material.len() > 4 * 1024
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "sender_output_prover_material",
            });
        }
        Ok(())
    }
    /// Return the exact domain-separated bytes signed by the receiver device.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the signing subject is invalid or cannot be encoded canonically.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        self.validate_public_binding()?;
        Ok(norito::encode_canonical(
            &KagemushaRecipientPaymentRequestSigningPreimageV2 {
                domain: KAGEMUSHA_RECIPIENT_PAYMENT_REQUEST_SIGNING_DOMAIN_V2.to_owned(),
                payload: self.clone(),
            },
        )?)
    }
}
impl KagemushaRecipientPaymentRequestV2 {
    /// Exact network for the requested offline note.
    #[must_use]
    pub fn network_id(&self) -> &NetworkId {
        &self.network_id
    }
    /// Account that must own the admitted receiver registration.
    #[must_use]
    pub fn recipient(&self) -> &AccountId {
        &self.recipient
    }
    /// Asset definition that must be admitted by the receiver registration.
    #[must_use]
    pub fn asset(&self) -> &AssetDefinitionId {
        &self.asset
    }
    /// Exact requested amount at the authoritative asset scale.
    #[must_use]
    pub const fn amount(&self) -> KagemushaScaledAmountV2 {
        self.amount
    }
    /// Recipient output commitment and nullifier bound by the signed request.
    #[must_use]
    pub fn recipient_output(&self) -> &KagemushaSpendableNoteDescriptorV2 {
        &self.recipient_output
    }
    /// Registered receiver-device identifier bound by this request.
    #[must_use]
    pub fn receiver_device_id(&self) -> &str {
        &self.receiver_device_id
    }
    /// P-256 receiver key that must match the admitted registration.
    #[must_use]
    pub fn receiver_public_key(&self) -> &KagemushaDevicePublicKeyV2 {
        &self.receiver_public_key
    }
    /// Exclusive request expiry in Unix milliseconds.
    #[must_use]
    pub const fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }
    /// Construct the canonical request from prevalidated fields and a device signature.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn from_signed_payload(
        payload: KagemushaRecipientPaymentRequestSigningPayloadV2,
        signature: KagemushaDeviceSignatureV2,
    ) -> Result<Self, KagemushaValidationError> {
        let request = Self {
            network_id: payload.network_id,
            asset: payload.asset,
            amount: payload.amount,
            recipient: payload.recipient,
            recipient_key_reference: payload.recipient_key_reference,
            receiver_device_id: payload.receiver_device_id,
            receiver_public_key: payload.receiver_public_key,
            request_id: payload.request_id,
            issued_at_ms: payload.issued_at_ms,
            expires_at_ms: payload.expires_at_ms,
            recipient_output: payload.recipient_output,
            sender_output_prover_material: payload.sender_output_prover_material,
            signature,
        };
        request.validate_public_binding()?;
        Ok(request)
    }
    /// Reconstruct the canonical unsigned fields covered by the device signature.
    #[must_use]
    pub fn signing_payload(&self) -> KagemushaRecipientPaymentRequestSigningPayloadV2 {
        KagemushaRecipientPaymentRequestSigningPayloadV2 {
            network_id: self.network_id,
            asset: self.asset.clone(),
            amount: self.amount,
            recipient: self.recipient.clone(),
            recipient_key_reference: self.recipient_key_reference,
            receiver_device_id: self.receiver_device_id.clone(),
            receiver_public_key: self.receiver_public_key,
            request_id: self.request_id,
            issued_at_ms: self.issued_at_ms,
            expires_at_ms: self.expires_at_ms,
            recipient_output: self.recipient_output.clone(),
            sender_output_prover_material: self.sender_output_prover_material.clone(),
        }
    }
    /// Validate the exact signed request and opaque sender-prover material.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        let payload = self.signing_payload();
        payload.validate_public_binding()?;
        self.signature
            .verify(&self.receiver_public_key, &payload.signing_bytes()?)
            .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "recipient_request.signature",
            })?;
        let encoded_len = norito::encode_canonical(self)?.len();
        if encoded_len > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2 {
            return Err(KagemushaValidationError::EncodedSizeExceeded {
                max: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2,
                actual: encoded_len,
            });
        }
        Ok(())
    }
    /// Verify request authentication and its `[issued_at_ms, expires_at_ms)` lifetime.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_at(&self, now_ms: u64) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        if now_ms < self.issued_at_ms || now_ms >= self.expires_at_ms {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "recipient_request.expires_at_ms",
            });
        }
        Ok(())
    }
    /// Return the canonical request digest bound by the split proof.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRecipientPaymentRequestDigestPreimageV2 {
            domain: KAGEMUSHA_RECIPIENT_PAYMENT_REQUEST_DIGEST_DOMAIN_V2.to_owned(),
            request: self.clone(),
        })
    }
}
impl KagemushaOnlineHardwareAssertionV1 {
    /// Canonical registration platform selected by this typed assertion.
    #[must_use]
    pub const fn platform(&self) -> &'static str {
        match self {
            Self::AndroidKeyMint(_) => OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_PLATFORM,
            Self::IosAppAttest(_) => OFFLINE_DEVICE_ATTESTATION_IOS_APP_ATTEST_PLATFORM,
        }
    }
}
impl KagemushaRequestAuthorizationV2 {
    #[allow(clippy::too_many_arguments)]
    fn hardware_assertion_preimage_bytes_for_fields(
        authority: &AccountId,
        device_id: &str,
        asset_definition_id: &AssetDefinitionId,
        operation_id: [u8; 32],
        issued_at_ms: u64,
        expires_at_ms: u64,
        nonce: [u8; 32],
        payload_digest: [u8; 32],
        registration_hash: [u8; 32],
        platform: &str,
    ) -> Result<Vec<u8>, KagemushaValidationError> {
        Ok(norito::encode_canonical(
            &KagemushaRequestAuthorizationSigningPreimageV2 {
                domain: KAGEMUSHA_ONLINE_HARDWARE_ASSERTION_DOMAIN_V1.to_owned(),
                authority: authority.clone(),
                device_id: device_id.to_owned(),
                asset_definition_id: asset_definition_id.clone(),
                operation_id,
                issued_at_ms,
                expires_at_ms,
                nonce,
                payload_digest,
                registration_hash,
                platform: platform.to_owned(),
            },
        )?)
    }
    /// Derive platform signing input from unsigned public fields without constructing an
    /// on-wire authorization or fabricating a signature/authenticatorData value.
    #[allow(clippy::too_many_arguments)]
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the signing subject is invalid or cannot be encoded canonically.
    pub fn signing_bytes_for_fields(
        authority: &AccountId,
        device_id: &str,
        asset_definition_id: &AssetDefinitionId,
        operation_id: [u8; 32],
        issued_at_ms: u64,
        expires_at_ms: u64,
        nonce: [u8; 32],
        payload_digest: [u8; 32],
        registration_hash: [u8; 32],
        platform: &str,
    ) -> Result<Vec<u8>, KagemushaValidationError> {
        let preimage = Self::hardware_assertion_preimage_bytes_for_fields(
            authority,
            device_id,
            asset_definition_id,
            operation_id,
            issued_at_ms,
            expires_at_ms,
            nonce,
            payload_digest,
            registration_hash,
            platform,
        )?;
        match platform {
            OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_PLATFORM => Ok(preimage),
            OFFLINE_DEVICE_ATTESTATION_IOS_APP_ATTEST_PLATFORM => {
                Ok(Sha256::digest(preimage).to_vec())
            }
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "authorization.hardware_assertion.platform",
            }),
        }
    }
    /// Return the exact bytes supplied to the platform assertion API.
    ///
    /// Android signs the complete canonical domain-separated preimage with
    /// `SHA256withECDSA`. App Attest accepts a 32-byte client-data hash, so the
    /// iOS form returns SHA-256 of that same canonical preimage.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the signing subject is invalid or cannot be encoded canonically.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        Self::signing_bytes_for_fields(
            &self.authority,
            &self.device_id,
            &self.asset_definition_id,
            self.operation_id,
            self.issued_at_ms,
            self.expires_at_ms,
            self.nonce,
            self.payload_digest,
            self.registration_hash,
            self.hardware_assertion.platform(),
        )
    }
    /// Verify the typed hardware assertion under the exact registered key.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when structural, policy, signature, or cryptographic authentication fails.
    pub fn verify_hardware_signature(
        &self,
        assertion_public_key: &[u8],
    ) -> Result<(), KagemushaValidationError> {
        let public_key = KagemushaDevicePublicKeyV2::from_sec1_bytes(assertion_public_key)
            .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "authorization.assertion_public_key",
            })?;
        let signing_bytes = self.signing_bytes()?;
        match &self.hardware_assertion {
            KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(assertion) => {
                assertion.signature.verify(&public_key, &signing_bytes)
            }
            KagemushaOnlineHardwareAssertionV1::IosAppAttest(assertion) => {
                let mut signed =
                    Vec::with_capacity(assertion.authenticator_data.len() + signing_bytes.len());
                signed.extend_from_slice(&assertion.authenticator_data);
                signed.extend_from_slice(&signing_bytes);
                assertion.signature.verify(&public_key, &signed)
            }
        }
        .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "authorization.hardware_assertion.signature",
        })
    }
    /// Replace only the hardware signature in a prepared authorization.
    pub fn set_hardware_signature(&mut self, signature: KagemushaDeviceSignatureV2) {
        match &mut self.hardware_assertion {
            KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(assertion) => {
                assertion.signature = signature;
            }
            KagemushaOnlineHardwareAssertionV1::IosAppAttest(assertion) => {
                assertion.signature = signature;
            }
        }
    }
    /// Verify structure and exact unsigned-payload binding.
    ///
    /// Consensus verifies the signature only after resolving `registration_hash` to the exact
    /// validated registration and its P-256 assertion public key.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_payload(
        &self,
        expected_payload_digest: [u8; 32],
    ) -> Result<(), KagemushaValidationError> {
        if self.device_id.is_empty()
            || self.device_id.len() > 128
            || self.device_id.trim() != self.device_id
            || self.device_id.chars().any(char::is_control)
            || self.operation_id == [0; 32]
            || self.nonce == [0; 32]
            || self.registration_hash == [0; 32]
            || self.payload_digest != expected_payload_digest
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "authorization",
            });
        }
        match &self.hardware_assertion {
            KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(assertion) => {
                assertion.signature.validate()
            }
            KagemushaOnlineHardwareAssertionV1::IosAppAttest(assertion)
                if (KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_MIN_BYTES_V1
                    ..=KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_MAX_BYTES_V1)
                    .contains(&assertion.authenticator_data.len())
                    && assertion.authenticator_data[32] == 0x80 =>
            {
                assertion.signature.validate()
            }
            KagemushaOnlineHardwareAssertionV1::IosAppAttest(_) => {
                Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "authorization.hardware_assertion.authenticator_data",
                })
            }
        }
        .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "authorization.hardware_assertion.signature",
        })
    }
    /// Verify the signed request is live at the authoritative Torii time.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_payload_at(
        &self,
        expected_payload_digest: [u8; 32],
        now_ms: u64,
    ) -> Result<(), KagemushaValidationError> {
        self.validate_for_payload(expected_payload_digest)?;
        if now_ms < self.issued_at_ms || now_ms >= self.expires_at_ms {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "authorization.expires_at_ms",
            });
        }
        Ok(())
    }
}
impl KagemushaScaledAmountV2 {
    /// Construct an exact positive amount from atomic units and asset scale.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] for zero amounts or unsupported scales.
    pub fn new(atomic_units: u128, scale: u32) -> Result<Self, KagemushaValidationError> {
        let amount = Self {
            atomic_units,
            scale,
        };
        amount.validate()?;
        Ok(amount)
    }
    /// Convert an Iroha public quantity to the asset's atomic units without rounding.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the amount is non-positive, wider than `u128`, has
    /// more precision than the asset, or overflows while being normalized to the asset scale.
    pub fn from_public_quantity(
        amount: &Quantity,
        asset_scale: u32,
    ) -> Result<Self, KagemushaValidationError> {
        if asset_scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2 || amount.scale() > asset_scale {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "amount.scale",
            });
        }
        let Some(mantissa) = amount
            .as_numeric()
            .try_mantissa_u128()
            .filter(|value| *value > 0)
        else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "amount.atomic_units",
            });
        };
        let exponent = asset_scale - amount.scale();
        let factor = 10_u128.checked_pow(exponent).ok_or(
            KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "amount.atomic_units",
            },
        )?;
        let atomic_units = mantissa.checked_mul(factor).ok_or(
            KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "amount.atomic_units",
            },
        )?;
        Self::new(atomic_units, asset_scale)
    }
    /// Return the public quantity at the authoritative asset scale.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a decoded or manually built
    /// amount violates the Kagemusha amount contract.
    pub fn public_quantity(self) -> Result<Quantity, KagemushaValidationError> {
        self.validate()?;
        let numeric = Numeric::try_new(self.atomic_units, self.scale).map_err(|_| {
            KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "amount.scale",
            }
        })?;
        Quantity::from_canonical_numeric(numeric).map_err(|_| {
            KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "amount.atomic_units",
            }
        })
    }
    /// Validate the exact amount contract.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] for zero amounts or unsupported scales.
    pub fn validate(self) -> Result<(), KagemushaValidationError> {
        if self.atomic_units == 0 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "amount.atomic_units",
            });
        }
        if self.scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "amount.scale",
            });
        }
        Ok(())
    }
}
impl KagemushaConfidentialMerklePathV2 {
    /// Validate the fixed-depth, binary-direction path shape.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaValidationError> {
        if self.siblings.len() != KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2
            || self.directions.len() != KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2
            || self.directions.iter().any(|direction| *direction > 1)
            || self.root == [0; 32]
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "membership_witness.path",
            });
        }
        Ok(())
    }
    /// Return the leaf index encoded by the canonical direction bits.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the Merkle path shape or direction bits are invalid.
    pub fn leaf_index(&self) -> Result<u32, KagemushaValidationError> {
        self.validate_structure()?;
        Ok(self
            .directions
            .iter()
            .enumerate()
            .fold(0_u32, |index, (level, direction)| {
                index | (u32::from(*direction) << level)
            }))
    }
    /// Validate that the direction bits encode one exact leaf index.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_leaf_index(&self, leaf_index: u32) -> Result<(), KagemushaValidationError> {
        self.validate_structure()?;
        if leaf_index >= KAGEMUSHA_TOPUP_SHIELD_TREE_CAPACITY_V2 || self.leaf_index()? != leaf_index
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "membership_witness.leaf_index",
            });
        }
        Ok(())
    }
}
impl KagemushaNoteMembershipWitnessV2 {
    /// Validate the public witness shape and shared-root relationship.
    ///
    /// Native verification must additionally recompute both Poseidon paths: the real path from the
    /// proof-bound note commitment and the dummy path from the canonical empty leaf.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaValidationError> {
        self.input_path.validate_for_leaf_index(self.leaf_index)?;
        self.dummy_input_path.validate_structure()?;
        if self.input_path.root != self.dummy_input_path.root
            || self.dummy_input_path.leaf_index()? == self.leaf_index
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "membership_witness",
            });
        }
        Ok(())
    }
    /// Validate that the witness is bound to one proof statement root.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_root(&self, root: [u8; 32]) -> Result<(), KagemushaValidationError> {
        self.validate_structure()?;
        if self.input_path.root != root {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "membership_witness.root",
            });
        }
        Ok(())
    }
    /// Validate both the statement root and its proof-bound append-only frontier index.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_statement_v4(
        &self,
        statement: &KagemushaRecursiveSpendPublicStatementV4,
    ) -> Result<(), KagemushaValidationError> {
        statement.validate_public_binding()?;
        self.validate_for_root(statement.final_root)?;
        if self.dummy_input_path.leaf_index()? != statement.next_zero_leaf_index {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "membership_witness.next_zero_leaf_index",
            });
        }
        Ok(())
    }
}
impl KagemushaRecursiveSpendStateBoundaryV5 {
    /// Construct the field-neutral boundary from the complete exact state.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn new(state_limbs: Vec<u32>) -> Result<Self, KagemushaValidationError> {
        let boundary = Self {
            layout_version: KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V5,
            state_limbs,
        };
        boundary.validate()?;
        Ok(boundary)
    }
    /// Recover the exact canonical limbs without field reduction.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the source value is invalid or the requested canonical result cannot be derived.
    pub fn exact_state(&self) -> Result<&[u32], KagemushaValidationError> {
        self.validate()?;
        Ok(&self.state_limbs)
    }
    /// Validate the canonical cross-field state boundary.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.layout_version != KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V5
            || self.state_limbs.len() != KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5
            || self.state_limbs.first().copied()
                != Some(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5)
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.state_boundary",
            });
        }
        Ok(())
    }
}
impl KagemushaPastaPublicLayoutV4 {
    /// Derive every dynamic offset from the authenticated IPA round count.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn for_ipa_round_count(ipa_round_count: u32) -> Result<Self, KagemushaValidationError> {
        if ipa_round_count != KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.public_layout.ipa_round_count",
            });
        }
        // Version + round count, followed by two canonical u128 chunks for
        // each round challenge and the folded point encoding.
        let accumulator_limbs = ipa_round_count
            .checked_mul(2)
            .and_then(|value| value.checked_add(4))
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.public_layout.accumulator_limbs",
            })?;
        // Nineteen common-header cells precede the parity-local accumulator;
        // the common live/bootstrap selector remains the final (66th) cell so
        // recursive parent loading cannot confuse it with lineage material.
        let parent_eq_accumulator_offset = 19_u32;
        let parent_ep_accumulator_offset = parent_eq_accumulator_offset;
        let parent_eq_deferred_offset = parent_eq_accumulator_offset
            .checked_add(accumulator_limbs)
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.public_layout.eq_deferred_offset",
            })?;
        let parent_ep_deferred_offset = parent_eq_deferred_offset.checked_add(4).ok_or(
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.public_layout.ep_deferred_offset",
            },
        )?;
        let live_selector_offset = parent_ep_deferred_offset.checked_add(4).ok_or(
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.public_layout.live_selector_offset",
            },
        )?;
        let instance_column_limbs = live_selector_offset.checked_add(1).ok_or(
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.public_layout.instance_column_limbs",
            },
        )?;
        Ok(Self {
            ipa_round_count,
            accumulator_limbs,
            parent_eq_accumulator_offset,
            parent_ep_accumulator_offset,
            parent_eq_deferred_offset,
            parent_ep_deferred_offset,
            live_selector_offset,
            instance_column_limbs,
        })
    }
}
impl KagemushaStepCircuitParamsV4 {
    /// Construct and validate the single reviewed first-release generation profile.
    ///
    /// Eq and Ep deliberately share this parameter carrier: parity-specific circuit identities and
    /// keys remain separate, while their authenticated Halo2 geometry is identical.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] if the reviewed constants no longer
    /// form the admitted compact profile.
    pub fn reviewed_first_release_generation_profile() -> Result<Self, KagemushaValidationError> {
        let k = KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
        let layout = KagemushaPastaPublicLayoutV4::for_ipa_round_count(k)?;
        let params = Self {
            version: KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
            k,
            num_advice_per_phase: KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4.to_vec(),
            num_lookup_advice_per_phase: KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4.to_vec(),
            num_fixed: 1,
            lookup_bits: k - 1,
            num_instance_columns: 1,
            public_input_limbs: layout.instance_column_limbs,
            minimum_unusable_rows: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
            max_parent_proof_bytes: KAGEMUSHA_STEP_PROOF_RELEASE_BYTES_V4,
        };
        params.validate_release_generation_profile()?;
        Ok(params)
    }
    /// Validate the complete authenticated layout and return its public ABI.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<KagemushaPastaPublicLayoutV4, KagemushaValidationError> {
        let layout = KagemushaPastaPublicLayoutV4::for_ipa_round_count(self.k)?;
        let domain_rows = 1_u64.checked_shl(self.k).ok_or(
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.circuit_params.domain_rows",
            },
        )?;
        let phase_count = self.num_advice_per_phase.len();
        if self.version != KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4
            || !(KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4..=KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4)
                .contains(&self.k)
            || phase_count != 1
            || phase_count > KAGEMUSHA_STEP_CIRCUIT_MAX_PHASES_V4
            || self.num_advice_per_phase.as_slice()
                != KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4
            || self.num_lookup_advice_per_phase.as_slice()
                != KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4
            || self.num_fixed != 1
            || self.lookup_bits != self.k - 1
            || self.num_instance_columns != 1
            || self.public_input_limbs != layout.instance_column_limbs
            || self.minimum_unusable_rows < KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4
            || self.max_parent_proof_bytes == 0
            || self.max_parent_proof_bytes > KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4
            || u64::from(self.public_input_limbs)
                .checked_add(u64::from(self.minimum_unusable_rows))
                .is_none_or(|minimum_rows| minimum_rows >= domain_rows)
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.circuit_params",
            });
        }
        Ok(layout)
    }
    /// Validate the reviewed first-release profile used for full key generation.
    ///
    /// Artifact decoding and generation both admit only the compact V5 shape.
    /// This dedicated boundary makes the reviewed generation profile explicit
    /// before any expensive key-generation allocation begins.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_release_generation_profile(
        &self,
    ) -> Result<KagemushaPastaPublicLayoutV4, KagemushaValidationError> {
        let layout = self.validate()?;
        if self.k != KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4
            || self.num_advice_per_phase != KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4
            || self.num_lookup_advice_per_phase != KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4
            || self.num_fixed != 1
            || self.lookup_bits != self.k - 1
            || self.minimum_unusable_rows != KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4
            || self.max_parent_proof_bytes != KAGEMUSHA_STEP_PROOF_RELEASE_BYTES_V4
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.circuit_params.release_generation_profile",
            });
        }
        Ok(layout)
    }
    /// Domain-separated identity of the canonical authenticated parameters.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn sha256(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        let encoded = norito::encode_canonical(self).map_err(|_| {
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.circuit_params.encoding",
            }
        })?;
        let mut hasher = Sha256::new();
        hasher.update(KAGEMUSHA_STEP_CIRCUIT_PARAMS_SHA256_DOMAIN_V4);
        hasher.update([0]);
        hasher.update(encoded);
        Ok(hasher.finalize().into())
    }
}
impl KagemushaPastaCycleArtifactV4 {
    /// Validate one immutable V4 artifact descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if !is_kagemusha_portable_file_name(&self.file_name)
            || self.size_bytes == 0
            || self.size_bytes > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
            || self.sha256 == [0; 32]
            || self.payload_size_bytes == 0
            || self.payload_size_bytes >= self.size_bytes
            || self.payload_size_bytes > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
            || self.payload_sha256 == [0; 32]
            || self.payload_sha256 == self.sha256
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.artifact",
            });
        }
        Ok(())
    }
}
impl KagemushaPastaCycleFramedArtifactHeaderV4 {
    /// Validate the bounded public KRV4 header without allocating its payload.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        let expected_circuit_id = match self.parity {
            KagemushaPastaCycleParityV1::StepEq => KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
            KagemushaPastaCycleParityV1::StepEp => KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        };
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4
            || self.manifest_schema != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4
            || self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4
            || !is_kagemusha_portable_identifier(&self.generation)
            || self.circuit_id != expected_circuit_id
            || !is_kagemusha_portable_identifier(&self.parameter_generation)
            || self.ipa_k < KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4
            || self.ipa_k > KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4
            || self.circuit_params_sha256 == [0; 32]
            || self.compiled_protocol_structure_sha256 == [0; 32]
            || self.step_proof_size_bytes == 0
            || self.step_proof_size_bytes > KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4
            || self.payload_size_bytes == 0
            || self.payload_size_bytes > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
            || self.payload_sha256 == [0; 32]
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.artifact_header",
            });
        }
        Ok(())
    }
    /// Bind this header to one exact descriptor in a validated V4 manifest.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_against_manifest(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        descriptor: &KagemushaPastaCycleArtifactV4,
    ) -> Result<(), KagemushaValidationError> {
        self.validate_against_manifest_state(manifest, descriptor, true)
    }
    /// Bind this header to one exact clean pre-promotion candidate manifest.
    /// This authenticates structure and bytes only; it does not promote or
    /// relabel the candidate as a production release.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_against_candidate_manifest(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        descriptor: &KagemushaPastaCycleArtifactV4,
    ) -> Result<(), KagemushaValidationError> {
        self.validate_against_manifest_state(manifest, descriptor, false)
    }
    fn validate_against_manifest_state(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        descriptor: &KagemushaPastaCycleArtifactV4,
        finalized_release: bool,
    ) -> Result<(), KagemushaValidationError> {
        if finalized_release {
            manifest.validate()?;
        } else {
            manifest.validate_unsigned_candidate()?;
        }
        descriptor.validate()?;
        self.validate()?;
        let profile = manifest
            .profiles
            .iter()
            .find(|profile| profile.parity == self.parity)
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.artifact_header.profile",
            })?;
        if !profile
            .artifacts
            .iter()
            .any(|artifact| artifact == descriptor)
            || self.generation != manifest.generation
            || self.circuit_id != profile.circuit_id
            || self.parameter_generation != profile.parameter_generation
            || self.ipa_k != profile.ipa_k
            || self.circuit_params_sha256 != profile.circuit_params_sha256()?
            || self.compiled_protocol_structure_sha256 != profile.compiled_protocol_structure_sha256
            || self.step_proof_size_bytes != profile.step_proof_size_bytes
            || self.kind != descriptor.kind
            || self.payload_size_bytes != descriptor.payload_size_bytes
            || self.payload_sha256 != descriptor.payload_sha256
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.artifact_header.manifest_binding",
            });
        }
        Ok(())
    }
}
impl KagemushaTopUpFinalityRosterArtifactReferenceV4 {
    /// Validate the V4 role-bound reference to a canonical roster archive.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if !is_kagemusha_portable_file_name(&self.file_name)
            || self.file_name != KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4
            || self.size_bytes == 0
            || self.size_bytes > KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2
            || self.sha256 == [0; 32]
            || !is_kagemusha_portable_identifier(&self.artifact_generation)
            || self.circuit_id != KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2
            || self.purpose != KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2
            || self.artifact_type != KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2
            || self.required_bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.v4.roster_artifact_reference",
            });
        }
        Ok(())
    }
}
impl KagemushaPastaCycleProofProfileV4 {
    /// Validate one V4 parity profile and its exact four-file inventory.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        let (expected_circuit, expected_file_names) = match self.parity {
            KagemushaPastaCycleParityV1::StepEq => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
                ],
            ),
            KagemushaPastaCycleParityV1::StepEp => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
                ],
            ),
        };
        self.circuit_params.validate()?;
        if self.circuit_id != expected_circuit
            || !is_kagemusha_portable_identifier(&self.parameter_generation)
            || self.ipa_k != self.circuit_params.k
            || self.compiled_protocol_structure_sha256 == [0; 32]
            || self.step_proof_size_bytes == 0
            || self.step_proof_size_bytes != self.circuit_params.max_parent_proof_bytes
            || self.artifacts.len() != 4
            || self.artifacts[0].kind != KagemushaPastaCycleArtifactKindV4::ParamsIpa
            || self.artifacts[1].kind != KagemushaPastaCycleArtifactKindV4::ProvingKey
            || self.artifacts[2].kind != KagemushaPastaCycleArtifactKindV4::VerifyingKey
            || self.artifacts[3].kind != KagemushaPastaCycleArtifactKindV4::BootstrapWitness
            || self
                .artifacts
                .iter()
                .zip(expected_file_names)
                .any(|(artifact, expected)| artifact.file_name != expected)
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.profile",
            });
        }
        let mut names = std::collections::BTreeSet::new();
        for artifact in &self.artifacts {
            artifact.validate()?;
            if !names.insert(artifact.file_name.as_str()) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.profile.artifact_name",
                });
            }
        }
        Ok(())
    }
    /// Return the exact descriptor for the canonical bootstrap payload.
    #[must_use]
    pub fn bootstrap_artifact(&self) -> Option<&KagemushaPastaCycleArtifactV4> {
        self.artifacts
            .get(3)
            .filter(|artifact| artifact.kind == KagemushaPastaCycleArtifactKindV4::BootstrapWitness)
    }
    /// Return the exact authenticated circuit-parameter identity.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the source value is invalid or the requested canonical result cannot be derived.
    pub fn circuit_params_sha256(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        self.circuit_params.sha256()
    }
}
const KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_DESCRIPTOR_BYTES_V1: usize = 16 * 1024 * 1024;
const KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_PATH_BYTES_V1: usize = 4 * 1024;
const KAGEMUSHA_REVIEWED_SOURCE_DIFF_DOMAIN_V1: &[u8] = b"iroha-source-diff-v1\0";
const KAGEMUSHA_REVIEWED_SOURCE_TRACKED_DIFF_DOMAIN_V1: &[u8] = b"tracked-binary-diff-sha256\0";
const KAGEMUSHA_REVIEWED_SOURCE_UNTRACKED_MANIFEST_DOMAIN_V1: &[u8] =
    b"untracked-path-blob-manifest-sha256\0";
fn append_python_ascii_json_string(out: &mut String, value: &str) {
    use core::fmt::Write as _;
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0020}'..='\u{007e}' => out.push(character),
            _ => {
                let code = u32::from(character);
                if code <= 0xffff {
                    let _ = write!(out, "\\u{code:04x}");
                } else {
                    let scalar = code - 0x1_0000;
                    let high = 0xd800 + (scalar >> 10);
                    let low = 0xdc00 + (scalar & 0x3ff);
                    let _ = write!(out, "\\u{high:04x}\\u{low:04x}");
                }
            }
        }
    }
    out.push('"');
}
fn kagemusha_reviewed_source_manifest_entry_json(
    entry: &KagemushaReviewedSourceClosureManifestEntryV1,
) -> String {
    let mut out = String::new();
    out.push_str("{\"blob_sha256\":\"");
    out.push_str(&hex::encode(entry.blob_sha256));
    out.push_str("\",\"git_blob_oid\":");
    append_python_ascii_json_string(&mut out, &entry.git_blob_oid);
    out.push_str(",\"git_mode\":");
    append_python_ascii_json_string(&mut out, &entry.git_mode);
    out.push_str(",\"path\":");
    append_python_ascii_json_string(&mut out, &entry.path);
    out.push_str(",\"path_bytes_base64\":");
    append_python_ascii_json_string(&mut out, &entry.path_bytes_base64);
    out.push('}');
    out
}
fn kagemusha_reviewed_source_path_is_safe(path: &[u8]) -> bool {
    !path.is_empty()
        && !path.starts_with(b"/")
        && !path.ends_with(b"/")
        && path.len() <= KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_PATH_BYTES_V1
        && !path.contains(&0)
        && !path
            .split(|byte| *byte == b'/')
            .any(|component| component.is_empty() || component == b"." || component == b"..")
        && path.split(|byte| *byte == b'/').next() != Some(b".git".as_slice())
        && path != b"Cargo.lock"
}
impl KagemushaReviewedSourceClosureV1 {
    /// Validate exact descriptor structure, raw-byte path order, and derived digests.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        let untracked_count = usize::try_from(self.untracked_file_count).ok();
        let nonzero_digests = [
            self.source_tree_sha256,
            self.tracked_binary_diff_sha256,
            self.untracked_path_mode_blob_oid_manifest_sha256,
            self.tracked_cargo_lock_sha256,
            self.combined_source_fingerprint_sha256,
        ]
        .into_iter()
        .all(|digest| digest != [0; 32]);
        if self.schema != KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_SCHEMA_V1
            || !is_kagemusha_source_commit(&self.base_commit)
            || self.base_commit != self.source_commit
            || self.source_repo_dirty
            || !nonzero_digests
            || untracked_count != Some(self.untracked_path_mode_blob_oid_manifest.len())
            || untracked_count.is_none_or(|count| {
                count > KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_UNTRACKED_FILES_V1
            })
            || self.tracked_cargo_lock_size_bytes == 0
            || self.tracked_cargo_lock_size_bytes
                > KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_CARGO_LOCK_BYTES_V1
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.reviewed_source_closure",
            });
        }
        let mut previous_path: Option<Vec<u8>> = None;
        let mut descriptor_size = 512_usize;
        let mut manifest_hasher = Sha256::new();
        for entry in &self.untracked_path_mode_blob_oid_manifest {
            let path = BASE64_STANDARD
                .decode(entry.path_bytes_base64.as_bytes())
                .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.reviewed_source_closure.path",
                })?;
            let display_matches =
                core::str::from_utf8(&path).is_ok_and(|display| display == entry.path);
            let path_is_strictly_ordered = previous_path
                .as_ref()
                .is_none_or(|previous| previous.as_slice() < path.as_slice());
            if entry.blob_sha256 == [0; 32]
                || !is_kagemusha_source_commit(&entry.git_blob_oid)
                || !matches!(entry.git_mode.as_str(), "100644" | "100755")
                || BASE64_STANDARD.encode(&path) != entry.path_bytes_base64
                || !display_matches
                || !kagemusha_reviewed_source_path_is_safe(&path)
                || !path_is_strictly_ordered
            {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.reviewed_source_closure.entry",
                });
            }
            previous_path = Some(path);
            let entry_json = kagemusha_reviewed_source_manifest_entry_json(entry);
            descriptor_size = descriptor_size.checked_add(entry_json.len()).ok_or(
                KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.reviewed_source_closure.size",
                },
            )?;
            if descriptor_size > KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_DESCRIPTOR_BYTES_V1 {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.reviewed_source_closure.size",
                });
            }
            manifest_hasher.update(entry_json.as_bytes());
            manifest_hasher.update(b"\n");
        }
        let manifest_sha256: [u8; 32] = manifest_hasher.finalize().into();
        if manifest_sha256 != self.untracked_path_mode_blob_oid_manifest_sha256 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.reviewed_source_closure.manifest_sha256",
            });
        }
        let mut combined = Sha256::new();
        combined.update(KAGEMUSHA_REVIEWED_SOURCE_DIFF_DOMAIN_V1);
        combined.update(KAGEMUSHA_REVIEWED_SOURCE_TRACKED_DIFF_DOMAIN_V1);
        combined.update(self.tracked_binary_diff_sha256);
        combined.update(KAGEMUSHA_REVIEWED_SOURCE_UNTRACKED_MANIFEST_DOMAIN_V1);
        combined.update(self.untracked_path_mode_blob_oid_manifest_sha256);
        let combined_sha256: [u8; 32] = combined.finalize().into();
        let empty_sha256: [u8; 32] = Sha256::digest([]).into();
        let derived_dirty =
            self.tracked_binary_diff_sha256 != empty_sha256 || self.untracked_file_count != 0;
        if combined_sha256 != self.combined_source_fingerprint_sha256 || derived_dirty {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.reviewed_source_closure.fingerprint",
            });
        }
        Ok(())
    }
    /// Return the exact canonical compact sorted-key ASCII JSON descriptor plus LF.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the closure is invalid or the generated descriptor
    /// exceeds its protocol size bound.
    pub fn canonical_descriptor_bytes(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        self.validate()?;
        let mut out = String::new();
        out.push_str("{\"base_commit\":");
        append_python_ascii_json_string(&mut out, &self.base_commit);
        out.push_str(",\"combined_source_fingerprint_sha256\":\"");
        out.push_str(&hex::encode(self.combined_source_fingerprint_sha256));
        out.push_str("\",\"tracked_cargo_lock_sha256\":\"");
        out.push_str(&hex::encode(self.tracked_cargo_lock_sha256));
        out.push_str("\",\"tracked_cargo_lock_size_bytes\":");
        out.push_str(&self.tracked_cargo_lock_size_bytes.to_string());
        out.push_str(",\"schema\":");
        append_python_ascii_json_string(&mut out, &self.schema);
        out.push_str(",\"source_commit\":");
        append_python_ascii_json_string(&mut out, &self.source_commit);
        out.push_str(",\"source_repo_dirty\":false,\"source_tree_sha256\":\"");
        out.push_str(&hex::encode(self.source_tree_sha256));
        out.push_str("\",\"tracked_binary_diff_sha256\":\"");
        out.push_str(&hex::encode(self.tracked_binary_diff_sha256));
        out.push_str("\",\"untracked_file_count\":");
        out.push_str(&self.untracked_file_count.to_string());
        out.push_str(",\"untracked_path_mode_blob_oid_manifest\":[");
        for (index, entry) in self
            .untracked_path_mode_blob_oid_manifest
            .iter()
            .enumerate()
        {
            if index != 0 {
                out.push(',');
            }
            out.push_str(&kagemusha_reviewed_source_manifest_entry_json(entry));
        }
        out.push_str("],\"untracked_path_mode_blob_oid_manifest_sha256\":\"");
        out.push_str(&hex::encode(
            self.untracked_path_mode_blob_oid_manifest_sha256,
        ));
        out.push_str("\"}\n");
        if out.len() > KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_DESCRIPTOR_BYTES_V1 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.reviewed_source_closure.size",
            });
        }
        Ok(out.into_bytes())
    }
    /// SHA-256 of the exact canonical compact sorted-key ASCII JSON plus LF.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn canonical_descriptor_sha256(&self) -> Result<[u8; 32], KagemushaValidationError> {
        Ok(Sha256::digest(self.canonical_descriptor_bytes()?).into())
    }
}
include!("kagemusha_release_v4.rs");
impl KagemushaRecursiveSpendArtifactBindingV4 {
    /// Validate a complete authenticated V4 manifest identity.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4
            || !is_kagemusha_portable_identifier(&self.generation)
            || self.manifest_sha256 == [0; 32]
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "artifact_binding.v4",
            });
        }
        Ok(())
    }
    /// Require this binding to identify supplied canonical V4 manifest bytes.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_manifest(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        canonical_manifest_bytes: &[u8],
    ) -> Result<(), KagemushaValidationError> {
        self.validate()?;
        manifest.validate()?;
        let expected_manifest_bytes = norito::encode_canonical(manifest)?;
        let digest: [u8; 32] = Sha256::digest(&expected_manifest_bytes).into();
        if canonical_manifest_bytes != expected_manifest_bytes.as_slice()
            || self.generation != manifest.generation
            || self.manifest_sha256 != digest
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "artifact_binding.v4.manifest",
            });
        }
        Ok(())
    }
}
impl KagemushaRecursiveSpendNativeCapabilitiesV4 {
    /// Validate an installed ABI-21 backend capability record.
    ///
    /// `max_proof_bytes` is deliberately release-specific: it must come from the authenticated V4
    /// manifest selected by the installed artifact handle, rather than from a compile-time default.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        let expected_roles = KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.map(str::to_owned);
        let missing_gates_are_canonical = !self.missing_gates.is_empty()
            && self.missing_gates.len() <= KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
            && self
                .missing_gates
                .iter()
                .all(|gate| is_kagemusha_portable_identifier(gate))
            && self.missing_gates.windows(2).all(|pair| pair[0] < pair[1]);
        if self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
            || self.artifact_manifest_schema
                != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4
            || self.proof_envelope_version
                != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4
            || self.step_eq_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4
            || self.step_ep_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4
            || self.artifact_roles != expected_roles
            || self.max_proof_bytes == 0
            || self.max_proof_bytes > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
            || (self.proof_backend_available && !self.missing_gates.is_empty())
            || (!self.proof_backend_available && !missing_gates_are_canonical)
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.capabilities",
            });
        }
        Ok(())
    }
}
impl KagemushaPastaCycleProofEnvelopeV4 {
    /// Validate the fixed ABI-21 envelope shape before release lookup.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4
            || self.step_eq_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4
            || self.step_ep_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4
            || self.step_eq_circuit_id == self.step_ep_circuit_id
            || !is_kagemusha_portable_identifier(&self.artifact_generation)
            || !is_kagemusha_portable_identifier(&self.step_eq_parameter_generation)
            || !is_kagemusha_portable_identifier(&self.step_ep_parameter_generation)
            || self.manifest_sha256 == [0; 32]
            || self.step_eq_circuit_params_sha256 == [0; 32]
            || self.step_ep_circuit_params_sha256 == [0; 32]
            || self.step_eq_verifier_key_sha256 == [0; 32]
            || self.step_ep_verifier_key_sha256 == [0; 32]
            || self.step_eq_verifier_key_sha256 == self.step_ep_verifier_key_sha256
            || self.proof.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
            || self.proof.bytes.is_empty()
            || self.proof.bytes.len()
                > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.proof_envelope",
            });
        }
        self.state_boundary.validate()
    }
    /// Bind every release-selected envelope identity to a validated V4 manifest.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_against_manifest(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    ) -> Result<(), KagemushaValidationError> {
        self.validate_against_manifest_state(manifest, true)
    }
    /// Bind an envelope to one exact clean pre-promotion candidate manifest.
    /// This is a structural evidence check and confers no release authority.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_against_candidate_manifest(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    ) -> Result<(), KagemushaValidationError> {
        self.validate_against_manifest_state(manifest, false)
    }
    fn validate_against_manifest_state(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        finalized_release: bool,
    ) -> Result<(), KagemushaValidationError> {
        if finalized_release {
            manifest.validate()?;
        } else {
            manifest.validate_unsigned_candidate()?;
        }
        self.validate()?;
        let [vesta_profile, pallas_profile] = manifest.profiles.as_slice() else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.proof_envelope.profile_pair",
            });
        };
        let manifest_sha256: [u8; 32] =
            Sha256::digest(norito::encode_canonical(manifest).map_err(|_| {
                KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.proof_envelope.manifest_sha256",
                }
            })?)
            .into();
        if self.artifact_generation != manifest.generation
            || self.manifest_sha256 != manifest_sha256
            || self.step_eq_circuit_id != vesta_profile.circuit_id
            || self.step_ep_circuit_id != pallas_profile.circuit_id
            || self.step_eq_parameter_generation != vesta_profile.parameter_generation
            || self.step_ep_parameter_generation != pallas_profile.parameter_generation
            || self.step_eq_circuit_params_sha256 != vesta_profile.circuit_params_sha256()?
            || self.step_ep_circuit_params_sha256 != pallas_profile.circuit_params_sha256()?
            || self.step_eq_verifier_key_sha256 != vesta_profile.artifacts[2].payload_sha256
            || self.step_ep_verifier_key_sha256 != pallas_profile.artifacts[2].payload_sha256
            || self.proof.bytes.len() > manifest.max_proof_bytes as usize
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.proof_envelope.manifest_binding",
            });
        }
        Ok(())
    }
    /// Validate the envelope in an exact network, asset, scale, and height context.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_against_manifest_for_context(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        expected_network_id: &NetworkId,
        expected_asset: &AssetDefinitionId,
        expected_asset_scale: u32,
        block_height: u64,
    ) -> Result<(), KagemushaValidationError> {
        self.validate_against_manifest(manifest)?;
        if &manifest.network_id != expected_network_id
            || &manifest.asset != expected_asset
            || manifest.asset_scale != expected_asset_scale
            || block_height < manifest.activation_height
            || block_height >= manifest.withdrawal_height
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.proof_envelope.release_context",
            });
        }
        Ok(())
    }
}
/// Return whether `value` is a canonical cross-platform artifact identifier.
///
/// Identifiers use the same single-component restrictions as artifact file names so release caches
/// cannot alias punctuation-only or Windows device names across build hosts.
#[must_use]
pub fn is_kagemusha_portable_identifier(value: &str) -> bool {
    is_kagemusha_portable_file_name(value)
}
fn is_kagemusha_portable_file_name(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || !value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
    {
        return false;
    }
    // These basenames are device aliases on Windows even when an extension is
    // present. Rejecting them keeps one manifest path unambiguous on every SDK
    // and artifact-build host instead of making validation platform-dependent.
    let basename = value.split('.').next().unwrap_or_default();
    if ["con", "prn", "aux", "nul"]
        .iter()
        .any(|reserved| basename.eq_ignore_ascii_case(reserved))
    {
        return false;
    }
    let basename_bytes = basename.as_bytes();
    !(basename_bytes.len() == 4
        && (basename_bytes[..3].eq_ignore_ascii_case(b"com")
            || basename_bytes[..3].eq_ignore_ascii_case(b"lpt"))
        && matches!(basename_bytes[3], b'1'..=b'9'))
}
fn is_kagemusha_source_commit(value: &str) -> bool {
    value.len() == 40
        && value.bytes().any(|byte| byte != b'0')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn is_kagemusha_network_id(value: &NetworkId) -> bool {
    value.as_bytes() != Hash::prehashed([0; Hash::LENGTH]).as_ref()
}
impl KagemushaTopUpShieldEvidenceV2 {
    /// Validate the typed proof envelope before authoritative ledger checks.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        let commitment = self.proof.vk_commitment.ok_or(
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "shield_evidence.proof.vk_commitment",
            },
        )?;
        if self.initial_root == [0; 32]
            || self.finalized_root == [0; 32]
            || self.initial_root == self.finalized_root
            || self.leaf_index >= KAGEMUSHA_TOPUP_SHIELD_INSERTION_CAPACITY_V2
            || self.proof.structural_error().is_some()
            || self.proof.backend.as_str() != KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND
            || commitment == [0; 32]
            || self.proof.proof.bytes.len() > KAGEMUSHA_TOPUP_SHIELD_MAX_PROOF_BYTES_V2
            || self.proof.lane_privacy.is_some()
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "shield_evidence",
            });
        }
        Ok(())
    }
}
impl KagemushaRecursiveSpendTopUpAnchorRefV2 {
    /// Validate a non-zero network-resolvable identity pair.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(self) -> Result<(), KagemushaValidationError> {
        if self.topup_operation_id == [0; 32] || self.anchor_digest == [0; 32] {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_anchor_ref",
            });
        }
        Ok(())
    }
}
impl KagemushaTopUpFinalityHeightContextV2 {
    /// Validate the bounded context projection independently of a trust artifact.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaValidationError> {
        if self.execution_policy_hash == Hash::prehashed([0; Hash::LENGTH]) {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.height_context.execution_policy_hash",
            });
        }
        let next_roster_too_large = self.next_epoch_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.roster.len() > KAGEMUSHA_TOPUP_FINALITY_MAX_VALIDATORS_V2
                || snapshot.validator_set_pops.len() > KAGEMUSHA_TOPUP_FINALITY_MAX_VALIDATORS_V2
        });
        let parent_signers_too_large = self.parent_commit_qc.as_ref().is_some_and(|parent| {
            parent.signers.len() > KAGEMUSHA_TOPUP_FINALITY_MAX_VALIDATORS_V2
        });
        if !is_kagemusha_network_id(&self.network_id)
            || self.protocol_version != PROTOCOL_VERSION
            || self.height == 0
            || self.epoch_end_height < self.height
            || next_roster_too_large
            || parent_signers_too_large
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.height_context",
            });
        }
        Ok(())
    }
    /// Reconstruct and validate the exact complete height context using one
    /// manifest-authenticated roster window.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn reconstruct_for_roster_window(
        &self,
        window: &KagemushaTopUpFinalityRosterWindowV2,
    ) -> Result<HeightContext, KagemushaValidationError> {
        self.validate_structure()?;
        window.validate_structure()?;
        if self.height < window.activates_at_height
            || self.height >= window.withdraws_at_height
            || self.mode != window.consensus_mode
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.height_context.roster_window",
            });
        }
        let context = HeightContext {
            network_id: self.network_id,
            protocol_version: self.protocol_version,
            height: self.height,
            epoch: self.epoch,
            epoch_end_height: self.epoch_end_height,
            next_epoch_snapshot: self.next_epoch_snapshot.clone(),
            mode: self.mode,
            parent_commit_qc: self.parent_commit_qc.clone(),
            snapshot_bootstrap: self.snapshot_bootstrap,
            roster: window.validator_set.clone(),
            quorum: DualQuorum::from_roster(&window.validator_set).map_err(|_| {
                KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "topup_finality.height_context.quorum",
                }
            })?,
            nexus_amx_context_hash: self.nexus_amx_context_hash,
            execution_policy_hash: self.execution_policy_hash,
            da_layout: self.da_layout,
            leader_seed: self.leader_seed,
        };
        if context.validate().is_err() || context.id() != self.context_id {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.height_context.context_id",
            });
        }
        Ok(context)
    }
}
impl KagemushaTopUpFinalityCompactQcV2 {
    /// Validate canonical bounds before consulting a trusted roster.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaValidationError> {
        let context = &self.height_context;
        let certificate = &self.certificate;
        context.validate_structure()?;
        if certificate.round.context_id != context.context_id
            || certificate.round.height != context.height
            || certificate.proposal_round.context_id != context.context_id
            || certificate.proposal_round.height != context.height
            || certificate.proposal_round != certificate.round
            || certificate.phase != GlobalPhase::Commit
            || certificate.aggregate_signature.len() != 96
            || certificate.execution_commitment.validate().is_err()
            || certificate.execution_commitment.topup_anchor_root.is_none()
            || certificate.execution_commitment.topup_anchor_count == 0
            || certificate.execution_commitment.topup_anchor_count
                > KAGEMUSHA_TOPUP_FINALITY_MAX_ANCHORS_PER_BLOCK_V2
            || certificate.signers.is_empty()
            || certificate.signers.len() > KAGEMUSHA_TOPUP_FINALITY_MAX_VALIDATORS_V2
            || certificate
                .signers
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.commit_qc",
            });
        }
        Ok(())
    }
    /// Bind the compact certificate to one separately trusted roster window.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_roster_window(
        &self,
        window: &KagemushaTopUpFinalityRosterWindowV2,
    ) -> Result<(), KagemushaValidationError> {
        self.validate_structure()?;
        window.validate_structure()?;
        let context = &self.height_context;
        let complete = context.reconstruct_for_roster_window(window)?;
        self.certificate.validate(&complete).map_err(|_| {
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.commit_qc.roster_window",
            }
        })?;
        Ok(())
    }
}
impl KagemushaTopUpAnchorMerkleProofV2 {
    /// Validate the unique balanced-tree shape implied by `leaf_count`.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.leaf_count == 0
            || self.leaf_count > KAGEMUSHA_TOPUP_FINALITY_MAX_ANCHORS_PER_BLOCK_V2
            || self.leaf_index >= self.leaf_count
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.anchor_path",
            });
        }
        let width = self.leaf_count.next_power_of_two();
        let expected_depth = width.trailing_zeros() as usize;
        if self.siblings.len() != expected_depth
            || self.siblings.len() > KAGEMUSHA_TOPUP_FINALITY_MAX_SIBLINGS_V2
            || self.siblings.contains(&[0; 32])
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.anchor_path.siblings",
            });
        }
        Ok(())
    }
}
impl KagemushaTopUpFinalityProofV2 {
    /// Validate the canonical self-contained proof shape. Cryptographic QC and
    /// Merkle verification are performed by the native verifier.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaValidationError> {
        if self.version != KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality",
            });
        }
        self.anchor.validate()?;
        self.commit_qc.validate_structure()?;
        self.anchor_path.validate()?;
        if self.anchor_path.leaf_count
            != self
                .commit_qc
                .certificate
                .execution_commitment
                .topup_anchor_count
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.anchor_path.leaf_count",
            });
        }
        Ok(())
    }
}
impl KagemushaTopUpFinalityRosterWindowV2 {
    /// Validate the ordered unit-power roster and activation window without proof-of-possession pairings.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaValidationError> {
        if self.activates_at_height == 0
            || self.withdraws_at_height <= self.activates_at_height
            || self.validator_set.is_empty()
            || self.validator_set.len() > KAGEMUSHA_TOPUP_FINALITY_MAX_VALIDATORS_V2
            || self.validator_set.len() != self.validator_set_pops.len()
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.roster_window",
            });
        }
        let unique = self
            .validator_set
            .iter()
            .map(|entry| &entry.validator)
            .collect::<std::collections::BTreeSet<_>>();
        if unique.len() != self.validator_set.len()
            || self.validator_set.iter().any(|entry| {
                entry.power != 1
                    || !matches!(
                        entry.validator.public_key().try_algorithm(),
                        Ok(Algorithm::BlsNormal)
                    )
            })
            || DualQuorum::from_roster(&self.validator_set).is_err()
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.roster_window.validator_set",
            });
        }
        Ok(())
    }
    /// Validate the complete roster and BLS proofs; cache by authenticated roster-archive digest.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        self.validate_structure()?;
        if self
            .validator_set
            .iter()
            .zip(&self.validator_set_pops)
            .any(|(entry, pop)| {
                iroha_crypto::bls_normal_pop_verify(entry.validator.public_key(), pop).is_err()
            })
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.roster_window.validator_set_pops",
            });
        }
        Ok(())
    }
}
impl KagemushaTopUpFinalityRosterArtifactV2 {
    /// Validate network-scoped, strictly ordered, non-overlapping trust windows
    /// without performing BLS proof-of-possession pairings.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaValidationError> {
        if self.version != KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2
            || !is_kagemusha_network_id(&self.network_id)
            || !is_kagemusha_portable_identifier(&self.artifact_generation)
            || self.windows.is_empty()
            || self.windows.len() > KAGEMUSHA_TOPUP_FINALITY_MAX_ROSTER_WINDOWS_V2
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.roster_artifact",
            });
        }
        let mut previous_withdrawal = None;
        for window in &self.windows {
            window.validate_structure()?;
            if previous_withdrawal.is_some_and(|height| height > window.activates_at_height) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "topup_finality.roster_artifact.windows.order",
                });
            }
            previous_withdrawal = Some(window.withdraws_at_height);
        }
        Ok(())
    }
    /// Validate every structural field and every BLS proof of possession.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        self.validate_structure()?;
        for window in &self.windows {
            window.validate()?;
        }
        Ok(())
    }
    /// Select exactly one trusted roster for `height`.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when \`height\` is not covered by exactly one canonical roster window.
    pub fn window_at(
        &self,
        height: u64,
    ) -> Result<&KagemushaTopUpFinalityRosterWindowV2, KagemushaValidationError> {
        self.validate_structure()?;
        let mut matching = self.windows.iter().filter(|window| {
            height >= window.activates_at_height && height < window.withdraws_at_height
        });
        let Some(window) = matching.next() else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.roster_artifact.window",
            });
        };
        if matching.next().is_some() {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.roster_artifact.window.ambiguous",
            });
        }
        Ok(window)
    }
}
fn validate_kagemusha_recursive_spend_topup_anchor_refs_v2(
    refs: &[KagemushaRecursiveSpendTopUpAnchorRefV2],
) -> Result<Vec<[u8; 32]>, KagemushaValidationError> {
    if refs.is_empty() || refs.len() > KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2 {
        return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "topup_anchor_refs",
        });
    }
    let mut lineage_roots = Vec::with_capacity(refs.len());
    let mut previous = None;
    let mut seen_operations = std::collections::BTreeSet::new();
    for anchor_ref in refs {
        anchor_ref.validate()?;
        if !seen_operations.insert(anchor_ref.topup_operation_id)
            || previous.is_some_and(|value| value >= *anchor_ref)
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_anchor_refs.order",
            });
        }
        previous = Some(*anchor_ref);
        lineage_roots.push(kagemusha_recursive_spend_lineage_root_v2(
            anchor_ref.anchor_digest,
        )?);
    }
    lineage_roots.sort_unstable();
    Ok(lineage_roots)
}
/// Encode an exact atomic amount in the confidential circuit's 32-byte field layout.
pub fn kagemusha_confidential_amount_encoding_v2(atomic_units: u128) -> [u8; 32] {
    let mut encoded = [0u8; 32];
    encoded[..16].copy_from_slice(&atomic_units.to_le_bytes());
    encoded
}
impl KagemushaUnshieldPublicInputsBindingV2 {
    /// Validate the standalone Kagemusha unshield public-input shape.
    ///
    /// The exact note, amount, asset, network, and optional change binding is
    /// checked by the enclosing redemption intent. This boundary rejects the
    /// structurally impossible zero identities and second-input coordinates
    /// before they can acquire a canonical digest.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the public-input shape cannot
    /// represent a Kagemusha redemption.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        let zero = [0_u8; 32];
        if self.input_commitment_0 == zero
            || self.input_commitment_1 != zero
            || self.nullifier_0 == zero
            || self.nullifier_1 != zero
            || self.root == zero
            || self.public_amount == zero
            || self.public_amount[16..].iter().any(|byte| *byte != 0)
            || self.asset_tag == zero
            || self.network_tag == zero
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redemption.v4.unshield_public_inputs",
            });
        }
        Ok(())
    }
    /// Return the domain-separated digest exposed by the redemption-change circuit.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaUnshieldPublicInputsDigestPreimageV2 {
            domain: KAGEMUSHA_UNSHIELD_PUBLIC_INPUTS_DIGEST_DOMAIN_V2.to_owned(),
            public_inputs: *self,
        })
    }
}
#[cfg(test)]
mod kagemusha_v4_artifact_contract_tests {
    use super::*;
    use crate::{
        domain::DomainId,
        isi::{InstructionBox, offline::ActivateKagemushaRecursiveReleaseV4},
    };
    use norito::core::{DecodeFromSlice as _, NoritoDeserialize as _};

    fn digest(label: &[u8]) -> [u8; 32] {
        Sha256::digest(label).into()
    }
    fn encode_with_alternate_norito_layout<T: norito::NoritoSerialize>(value: &T) -> Vec<u8> {
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        to_bytes(value).expect("encode alternate-layout V4 artifact value")
    }
    fn reviewed_source_closure() -> KagemushaReviewedSourceClosureV1 {
        let source_commit = "1234567890abcdef1234567890abcdef12345678".to_owned();
        let manifest_sha256 = Sha256::digest([]).into();
        let tracked_binary_diff_sha256 = Sha256::digest([]).into();
        let mut combined = Sha256::new();
        combined.update(KAGEMUSHA_REVIEWED_SOURCE_DIFF_DOMAIN_V1);
        combined.update(KAGEMUSHA_REVIEWED_SOURCE_TRACKED_DIFF_DOMAIN_V1);
        combined.update(tracked_binary_diff_sha256);
        combined.update(KAGEMUSHA_REVIEWED_SOURCE_UNTRACKED_MANIFEST_DOMAIN_V1);
        combined.update(manifest_sha256);
        KagemushaReviewedSourceClosureV1 {
            schema: KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_SCHEMA_V1.to_owned(),
            base_commit: source_commit.clone(),
            source_commit,
            source_repo_dirty: false,
            source_tree_sha256: digest(b"v4 artifact test source tree"),
            tracked_binary_diff_sha256,
            untracked_file_count: 0,
            untracked_path_mode_blob_oid_manifest: Vec::new(),
            untracked_path_mode_blob_oid_manifest_sha256: manifest_sha256,
            tracked_cargo_lock_size_bytes: 123,
            tracked_cargo_lock_sha256: digest(b"reviewed tracked Cargo.lock"),
            combined_source_fingerprint_sha256: combined.finalize().into(),
        }
    }
    #[test]
    fn reviewed_source_closure_json_matches_canonical_hex_descriptor() {
        let closure = reviewed_source_closure();
        let json = norito::json::to_json(&closure).expect("serialize reviewed source closure JSON");
        assert!(json.contains(&format!(
            "\"source_tree_sha256\":\"{}\"",
            hex::encode(closure.source_tree_sha256)
        )));
        assert!(json.contains("\"source_repo_dirty\":false"));
        assert!(json.contains("\"untracked_file_count\":0"));
        let decoded: KagemushaReviewedSourceClosureV1 =
            norito::json::from_str(&json).expect("decode canonical hex descriptor JSON");
        assert_eq!(decoded, closure);
    }
    #[test]
    fn first_release_reviewed_source_closure_rejects_every_dirty_shape() {
        assert_eq!(KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_UNTRACKED_FILES_V1, 0);
        let recompute_combined = |closure: &mut KagemushaReviewedSourceClosureV1| {
            let mut combined = Sha256::new();
            combined.update(KAGEMUSHA_REVIEWED_SOURCE_DIFF_DOMAIN_V1);
            combined.update(KAGEMUSHA_REVIEWED_SOURCE_TRACKED_DIFF_DOMAIN_V1);
            combined.update(closure.tracked_binary_diff_sha256);
            combined.update(KAGEMUSHA_REVIEWED_SOURCE_UNTRACKED_MANIFEST_DOMAIN_V1);
            combined.update(closure.untracked_path_mode_blob_oid_manifest_sha256);
            closure.combined_source_fingerprint_sha256 = combined.finalize().into();
        };
        let mut tracked = reviewed_source_closure();
        tracked.source_repo_dirty = true;
        tracked.tracked_binary_diff_sha256 = digest(b"forbidden tracked diff");
        recompute_combined(&mut tracked);
        assert!(tracked.validate().is_err());
        let mut untracked = reviewed_source_closure();
        let entry = KagemushaReviewedSourceClosureManifestEntryV1 {
            blob_sha256: digest(b"forbidden untracked bytes"),
            git_blob_oid: "abcdef1234567890abcdef1234567890abcdef12".to_owned(),
            git_mode: "100644".to_owned(),
            path: "forbidden-untracked.rs".to_owned(),
            path_bytes_base64: BASE64_STANDARD.encode(b"forbidden-untracked.rs"),
        };
        let entry_json = kagemusha_reviewed_source_manifest_entry_json(&entry);
        untracked.source_repo_dirty = true;
        untracked.untracked_file_count = 1;
        untracked.untracked_path_mode_blob_oid_manifest = vec![entry];
        untracked.untracked_path_mode_blob_oid_manifest_sha256 =
            Sha256::digest(format!("{entry_json}\n")).into();
        recompute_combined(&mut untracked);
        assert!(untracked.validate().is_err());
    }
    fn circuit_params() -> KagemushaStepCircuitParamsV4 {
        KagemushaStepCircuitParamsV4::reviewed_first_release_generation_profile()
            .expect("reviewed first-release circuit profile")
    }
    fn artifact(
        kind: KagemushaPastaCycleArtifactKindV4,
        file_name: &str,
        seed: u8,
        payload_size_bytes: u64,
        payload_sha256: [u8; 32],
    ) -> KagemushaPastaCycleArtifactV4 {
        KagemushaPastaCycleArtifactV4 {
            kind,
            file_name: file_name.to_owned(),
            size_bytes: payload_size_bytes + 256,
            sha256: digest(&[b'f', seed]),
            payload_size_bytes,
            payload_sha256,
        }
    }
    fn profile(
        parity: KagemushaPastaCycleParityV1,
        params: &KagemushaStepCircuitParamsV4,
        seed: u8,
    ) -> KagemushaPastaCycleProofProfileV4 {
        let (circuit_id, names) = match parity {
            KagemushaPastaCycleParityV1::StepEq => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
                ],
            ),
            KagemushaPastaCycleParityV1::StepEp => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
                ],
            ),
        };
        let kinds = [
            KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            KagemushaPastaCycleArtifactKindV4::ProvingKey,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        ];
        let artifacts = kinds
            .into_iter()
            .zip(names)
            .enumerate()
            .map(|(index, (kind, name))| {
                let index = u8::try_from(index).expect("small role index");
                artifact(kind, name, seed + index, 32, digest(&[b'p', seed + index]))
            })
            .collect();
        KagemushaPastaCycleProofProfileV4 {
            parity,
            circuit_id: circuit_id.to_owned(),
            parameter_generation: "v4-artifact-test-params".to_owned(),
            ipa_k: params.k,
            circuit_params: params.clone(),
            compiled_protocol_structure_sha256: digest(&[b's', seed]),
            step_proof_size_bytes: params.max_parent_proof_bytes,
            artifacts,
        }
    }
    fn manifest() -> KagemushaRecursiveSpendArtifactManifestV4 {
        let params = circuit_params();
        let reviewed_source_closure = reviewed_source_closure();
        let reviewed_source_closure_descriptor_sha256 = reviewed_source_closure
            .canonical_descriptor_sha256()
            .expect("reviewed source closure descriptor");
        let mut manifest = KagemushaRecursiveSpendArtifactManifestV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
            generation: "v4-artifact-test-release".to_owned(),
            source_commit: reviewed_source_closure.source_commit.clone(),
            source_tree_sha256: reviewed_source_closure.source_tree_sha256,
            source_repo_dirty: false,
            reviewed_source_closure,
            reviewed_source_closure_descriptor_sha256,
            authenticated_source_seal_projection_sha256: digest(b"v4 source projection"),
            reviewed_cargo_binary_sha256: digest(b"v4 reviewed cargo"),
            reviewed_rustc_binary_sha256: digest(b"v4 reviewed rustc"),
            generator_binary_sha256: digest(b"v4 sealed generator"),
            sealed_candidate_build_report_sha256: digest(b"v4 sealed build report"),
            network_id: kagemusha_test_network_id("v4-artifact-test-network"),
            asset: AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").expect("test domain"),
                "rose".parse().expect("test asset name"),
            ),
            asset_scale: 9,
            activation_height: 1,
            withdrawal_height: 100,
            max_proof_bytes: KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
            generation_memory_limit_bytes:
                KAGEMUSHA_RECURSIVE_SPEND_GENERATION_MEMORY_ABSOLUTE_MAX_BYTES_V4,
            generation_memory_enforcement_profile:
                KAGEMUSHA_RECURSIVE_SPEND_GENERATION_MEMORY_ENFORCEMENT_PROFILE_V4.to_owned(),
            qualification_receipt_sha256: [0; 32],
            qualified_candidate_sha256: [0; 32],
            internal_validation_receipt_sha256: [0; 32],
            profiles: vec![
                profile(KagemushaPastaCycleParityV1::StepEq, &params, 1),
                profile(KagemushaPastaCycleParityV1::StepEp, &params, 11),
            ],
            topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV4 {
                file_name: KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4.to_owned(),
                size_bytes: 128,
                sha256: digest(b"v4 artifact test roster"),
                artifact_generation: "v4-artifact-test-release".to_owned(),
                circuit_id: KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2.to_owned(),
                purpose: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2.to_owned(),
                artifact_type: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2.to_owned(),
                required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            },
            benchmark_evidence_sha256: [0; 32],
            cryptographic_review_sha256: [0; 32],
            release_attestation_sha256: [0; 32],
        };
        let candidate = KagemushaRecursiveSpendCandidateV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4,
            manifest: manifest.clone(),
        };
        let candidate_sha256 = candidate.sha256().expect("test candidate identity");
        manifest.qualification_receipt_sha256 = qualification_receipt_sha256();
        manifest.qualified_candidate_sha256 =
            kagemusha_recursive_spend_qualified_candidate_sha256_v4(
                candidate_sha256,
                manifest.qualification_receipt_sha256,
            );
        manifest.internal_validation_receipt_sha256 =
            digest(&internal_validation_receipt_bytes(&candidate, &manifest));
        manifest.benchmark_evidence_sha256 = digest(b"v4 artifact test benchmark");
        manifest.cryptographic_review_sha256 = digest(b"v4 artifact test review");
        manifest.release_attestation_sha256 = digest(b"v4 artifact test attestation");
        manifest
    }
    fn qualification_receipt_sha256() -> [u8; 32] {
        digest(b"v4 artifact test qualification receipt")
    }
    fn internal_validation_receipt_bytes(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        finalized_manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    ) -> Vec<u8> {
        norito::encode_canonical(
            &kagemusha_internal_validation_receipt::internal_validation_receipt_tests::signed_receipt_for_v4_candidate(
                candidate,
                finalized_manifest,
            ),
        )
        .expect("canonical candidate-bound internal-validation receipt")
    }
    fn unsigned_candidate(
        template: &KagemushaRecursiveSpendArtifactManifestV4,
    ) -> KagemushaRecursiveSpendCandidateV4 {
        let mut manifest = template.clone();
        manifest.qualification_receipt_sha256 = [0; 32];
        manifest.qualified_candidate_sha256 = [0; 32];
        manifest.internal_validation_receipt_sha256 = [0; 32];
        manifest.benchmark_evidence_sha256 = [0; 32];
        manifest.cryptographic_review_sha256 = [0; 32];
        manifest.release_attestation_sha256 = [0; 32];
        let candidate = KagemushaRecursiveSpendCandidateV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4,
            manifest,
        };
        candidate.validate().expect("valid test V4 candidate");
        candidate
    }
    fn signed_review_bytes(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        reviewers: &[&KeyPair],
    ) -> Vec<u8> {
        let receipt_sha256 = qualification_receipt_sha256();
        let qualified_candidate_sha256 = kagemusha_recursive_spend_qualified_candidate_sha256_v4(
            candidate.sha256().expect("test candidate identity"),
            receipt_sha256,
        );
        let payload = KagemushaRecursiveSpendCryptographicReviewPayloadV4::approved(
            candidate,
            receipt_sha256,
            qualified_candidate_sha256,
            digest(b"complete independent cryptographic review report"),
            [
                digest(b"constraint coverage evidence"),
                digest(b"cycle and transcript evidence"),
                digest(b"public input and transition evidence"),
                digest(b"artifact parameter and verifying key evidence"),
                digest(b"nullifier replay and finality evidence"),
                digest(b"parser canonicalization and resource bound evidence"),
            ],
        )
        .expect("canonical approved review payload");
        let mut approvals = reviewers
            .iter()
            .map(
                |key_pair| KagemushaRecursiveSpendCryptographicReviewApprovalV4 {
                    public_key: key_pair.public_key().clone(),
                    signature: SignatureOf::try_new(key_pair.private_key(), &payload)
                        .expect("test cryptographic review signature"),
                },
            )
            .collect::<Vec<_>>();
        approvals.sort_by(|left, right| left.public_key.cmp(&right.public_key));
        norito::encode_canonical(&KagemushaRecursiveSpendCryptographicReviewEvidenceV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_VERSION_V4,
            payload,
            approvals,
        })
        .expect("canonical signed review evidence")
    }
    #[test]
    fn v4_artifact_identities_ignore_ambient_norito_layout() {
        let manifest = manifest();
        let candidate = unsigned_candidate(&manifest);
        let params = &manifest.profiles[0].circuit_params;
        let expected_manifest_sha256 = manifest
            .canonical_sha256()
            .expect("canonical manifest identity");
        let expected_candidate_sha256 = candidate.sha256().expect("canonical candidate identity");
        let expected_params_sha256 = params.sha256().expect("canonical parameter identity");
        let expected_attestation_subject = manifest
            .release_attestation_subject()
            .expect("canonical release-attestation subject");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_eq!(
            manifest
                .canonical_sha256()
                .expect("manifest identity under alternate ambient layout"),
            expected_manifest_sha256
        );
        assert_eq!(
            candidate
                .sha256()
                .expect("candidate identity under alternate ambient layout"),
            expected_candidate_sha256
        );
        assert_eq!(
            params
                .sha256()
                .expect("parameter identity under alternate ambient layout"),
            expected_params_sha256
        );
        assert_eq!(
            manifest
                .release_attestation_subject()
                .expect("release-attestation subject under alternate ambient layout"),
            expected_attestation_subject
        );
    }
    #[test]
    fn v4_internal_validation_receipt_is_non_circular_and_required() {
        let finalized_manifest = manifest();
        let candidate = unsigned_candidate(&finalized_manifest);
        assert_eq!(
            candidate.manifest.internal_validation_receipt_sha256,
            [0; 32]
        );
        assert_eq!(
            finalized_manifest
                .immutable_candidate()
                .expect("recover pre-receipt candidate"),
            candidate
        );

        let receipt_bytes = internal_validation_receipt_bytes(&candidate, &finalized_manifest);
        assert_eq!(
            digest(&receipt_bytes),
            finalized_manifest.internal_validation_receipt_sha256
        );
        validate_internal_validation_receipt_v4(
            &finalized_manifest,
            &candidate,
            &receipt_bytes,
            None,
        )
        .expect("exact signed receipt binds candidate and finalized manifest");

        let mut missing_receipt = finalized_manifest.clone();
        missing_receipt.internal_validation_receipt_sha256 = [0; 32];
        assert!(missing_receipt.validate().is_err());

        let mut polluted_candidate = candidate;
        polluted_candidate
            .manifest
            .internal_validation_receipt_sha256 = digest(b"premature receipt");
        assert!(polluted_candidate.validate().is_err());
    }
    #[test]
    fn qualified_candidate_identity_has_a_fixed_domain_separated_preimage() {
        let candidate_sha256 = [0x11; 32];
        let receipt_sha256 = [0x22; 32];
        let expected = [
            0xe6, 0xde, 0xb4, 0xe8, 0xf6, 0xeb, 0x72, 0xac, 0x38, 0x79, 0x70, 0x33, 0x4f, 0xf1,
            0xae, 0xc0, 0xb6, 0xe9, 0x18, 0xa4, 0xd7, 0x7a, 0x0b, 0xc7, 0x19, 0xb2, 0x5a, 0x89,
            0x02, 0xb2, 0x33, 0xb3,
        ];
        let mut independent = Sha256::new();
        independent.update(KAGEMUSHA_RECURSIVE_SPEND_QUALIFIED_CANDIDATE_DOMAIN_V4);
        independent.update([0]);
        independent.update(candidate_sha256);
        independent.update(receipt_sha256);
        assert_eq!(<[u8; 32]>::from(independent.finalize()), expected);
        let qualified = kagemusha_recursive_spend_qualified_candidate_sha256_v4(
            candidate_sha256,
            receipt_sha256,
        );
        assert_eq!(qualified, expected);
    }
    #[test]
    #[expect(clippy::too_many_lines, reason = "closed qualification receipt matrix")]
    fn qualification_receipt_binds_canonical_role_order_and_candidate() {
        let candidate = unsigned_candidate(&manifest());
        let canonical_roles = [
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            ),
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ProvingKey,
            ),
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ),
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::ProvingKey,
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ),
        ];
        let role_digests = candidate
            .artifact_role_digests()
            .expect("canonical candidate role identities");
        for (index, (parity, kind)) in canonical_roles.into_iter().enumerate() {
            let descriptor = candidate
                .manifest
                .profiles
                .iter()
                .find(|profile| profile.parity == parity)
                .and_then(|profile| {
                    profile
                        .artifacts
                        .iter()
                        .find(|artifact| artifact.kind == kind)
                })
                .expect("canonical candidate role");
            assert_eq!(role_digests[2 * index], descriptor.sha256);
            assert_eq!(role_digests[2 * index + 1], descriptor.payload_sha256);
        }
        let receipt =
            KagemushaRecursiveSpendQualificationReceiptV4::new(&candidate, vec![0x41], vec![0x42])
                .expect("structurally valid qualification receipt");
        let encoded = norito::encode_canonical(&receipt).expect("canonical receipt bytes");
        assert_eq!(
            KagemushaRecursiveSpendQualificationReceiptV4::decode_canonical_against_candidate(
                &encoded, &candidate,
            )
            .expect("decode exact receipt"),
            receipt
        );
        #[cfg(feature = "json")]
        {
            let json = norito::json::to_json(&receipt).expect("qualification receipt JSON");
            let decoded: KagemushaRecursiveSpendQualificationReceiptV4 =
                norito::json::from_str(&json).expect("decode qualification receipt JSON");
            assert_eq!(decoded, receipt);
            decoded
                .validate_against_candidate(&candidate)
                .expect("JSON receipt remains candidate-bound");
        }
        let mut reordered = receipt.clone();
        reordered.artifact_role_digests.swap(0, 2);
        assert!(reordered.validate_against_candidate(&candidate).is_err());
        let mut substituted_digest = receipt.clone();
        substituted_digest.artifact_role_digests[0][0] ^= 1;
        assert!(
            substituted_digest
                .validate_against_candidate(&candidate)
                .is_err()
        );
        let mut substituted_memory_limit = receipt.clone();
        substituted_memory_limit.generation_memory_limit_bytes -= 1;
        assert!(
            substituted_memory_limit
                .validate_against_candidate(&candidate)
                .is_err()
        );
        let mut substituted_memory_profile = receipt.clone();
        substituted_memory_profile.generation_memory_enforcement_profile =
            "substituted-profile".to_owned();
        assert!(
            substituted_memory_profile
                .validate_against_candidate(&candidate)
                .is_err()
        );
        let mut substituted_projection = receipt.clone();
        substituted_projection.authenticated_source_seal_projection_sha256[0] ^= 1;
        assert!(
            substituted_projection
                .validate_against_candidate(&candidate)
                .is_err()
        );
        let mut substituted_cargo = receipt.clone();
        substituted_cargo.reviewed_cargo_binary_sha256[0] ^= 1;
        assert!(
            substituted_cargo
                .validate_against_candidate(&candidate)
                .is_err()
        );
        let mut substituted_rustc = receipt.clone();
        substituted_rustc.reviewed_rustc_binary_sha256[0] ^= 1;
        assert!(
            substituted_rustc
                .validate_against_candidate(&candidate)
                .is_err()
        );
        let mut other_candidate = candidate.clone();
        other_candidate.manifest.network_id =
            kagemusha_test_network_id("other-v4-artifact-test-network");
        other_candidate
            .validate()
            .expect("independently valid substituted candidate");
        assert!(
            receipt
                .validate_against_candidate(&other_candidate)
                .is_err()
        );
        let mut noncanonical = encoded;
        noncanonical.push(0);
        assert!(
            KagemushaRecursiveSpendQualificationReceiptV4::decode_canonical_against_candidate(
                &noncanonical,
                &candidate,
            )
            .is_err()
        );
        let oversized =
            vec![0_u8; KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4 + 1];
        assert!(
            KagemushaRecursiveSpendQualificationReceiptV4::decode_canonical_against_candidate(
                &oversized, &candidate,
            )
            .is_err()
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn qualification_receipt_json_rejects_wrong_digest_cardinality_and_encoding() {
        let candidate = unsigned_candidate(&manifest());
        let receipt =
            KagemushaRecursiveSpendQualificationReceiptV4::new(&candidate, vec![0x41], vec![0x42])
                .expect("structurally valid qualification receipt");
        for malformed_len in [15_usize, 17] {
            let mut value =
                norito::json::to_value(&receipt).expect("qualification receipt JSON value");
            let digests = value
                .as_object_mut()
                .and_then(|object| object.get_mut("artifact_role_digests"))
                .and_then(norito::json::Value::as_array_mut)
                .expect("qualification receipt digest array");
            if malformed_len < digests.len() {
                digests.truncate(malformed_len);
            } else {
                let digest = digests[0].clone();
                digests.push(digest);
            }
            let error =
                norito::json::from_value::<KagemushaRecursiveSpendQualificationReceiptV4>(value)
                    .expect_err("qualification receipt digest cardinality must be exact");
            assert!(
                error
                    .to_string()
                    .contains("expected exactly 16 array elements"),
                "unexpected qualification receipt cardinality error: {error}",
            );
        }
        for malformed_digest in ["00".repeat(31), "gg".repeat(32)] {
            let mut value =
                norito::json::to_value(&receipt).expect("qualification receipt JSON value");
            let digest = value
                .as_object_mut()
                .and_then(|object| object.get_mut("artifact_role_digests"))
                .and_then(norito::json::Value::as_array_mut)
                .and_then(|digests| digests.first_mut())
                .expect("qualification receipt digest");
            *digest = norito::json::Value::String(malformed_digest);
            norito::json::from_value::<KagemushaRecursiveSpendQualificationReceiptV4>(value)
                .expect_err("qualification receipt digest must be exactly 32 bytes of hex");
        }
    }
    fn promoted_release() -> KagemushaRecursiveSpendPromotedReleaseV4 {
        let finalized_manifest = manifest();
        let candidate = unsigned_candidate(&finalized_manifest);
        let candidate_sha256 = candidate.sha256().expect("test candidate identity");
        let approved_signers = [
            KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
        ]
        .into_iter()
        .zip([31_u8, 32, 33])
        .map(|(role, seed)| KagemushaRecursiveSpendApprovedSignerV1 {
            role,
            public_key: KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        })
        .collect();
        KagemushaRecursiveSpendPromotedReleaseV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
            generation: "v4-artifact-test-release".to_owned(),
            authenticated_source_seal_projection_sha256: finalized_manifest
                .authenticated_source_seal_projection_sha256,
            reviewed_cargo_binary_sha256: finalized_manifest.reviewed_cargo_binary_sha256,
            reviewed_rustc_binary_sha256: finalized_manifest.reviewed_rustc_binary_sha256,
            generator_binary_sha256: finalized_manifest.generator_binary_sha256,
            sealed_candidate_build_report_sha256: finalized_manifest
                .sealed_candidate_build_report_sha256,
            candidate_sha256,
            qualification_receipt_sha256: finalized_manifest.qualification_receipt_sha256,
            qualified_candidate_sha256: finalized_manifest.qualified_candidate_sha256,
            internal_validation_receipt_sha256: finalized_manifest
                .internal_validation_receipt_sha256,
            manifest_sha256: digest(b"v4 promotion manifest"),
            release_attestation_sha256: digest(b"v4 promotion attestation"),
            release_policy_sha256: digest(b"v4 promotion policy"),
            approved_signers,
            artifact_inventory_verified: true,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            artifact_roles: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
                .map(str::to_owned)
                .to_vec(),
            max_proof_bytes: KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
        }
    }
    fn release_activation_wire_fixture() -> KagemushaRecursiveSpendReleaseActivationV4 {
        let manifest = manifest();
        let candidate = unsigned_candidate(&manifest);
        let internal_validation_receipt = internal_validation_receipt_bytes(&candidate, &manifest);
        let release_attestation = KagemushaRecursiveSpendReleaseAttestationV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
            subject: manifest
                .release_attestation_subject()
                .expect("wire fixture manifest has a canonical attestation subject"),
            approvals: Vec::new(),
        };
        KagemushaRecursiveSpendReleaseActivationV4 {
            release_record: KagemushaRecursiveSpendReleaseRecordV4 {
                manifest,
                release_attestation,
                internal_validation_receipt,
                physical_device_benchmark_summary: b"wire-bound benchmark evidence".to_vec(),
                cryptographic_review_summary: b"wire-bound review evidence".to_vec(),
                promotion_record: promoted_release(),
            },
            configured_policy_sha256: digest(b"wire-bound release policy"),
            step_eq_verifier_key_id: VerifyingKeyId::new("halo2/ipa", "wire-bound-step-eq"),
            step_eq_verifier_record: VerifyingKeyRecord::new(
                7,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
                BackendTag::Halo2IpaPasta,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
                digest(b"wire-bound Eq public inputs"),
                digest(b"wire-bound Eq verifier"),
            ),
            step_ep_verifier_key_id: VerifyingKeyId::new("halo2/ipa", "wire-bound-step-ep"),
            step_ep_verifier_record: VerifyingKeyRecord::new(
                7,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
                BackendTag::Halo2IpaPasta,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V4,
                digest(b"wire-bound Ep public inputs"),
                digest(b"wire-bound Ep verifier"),
            ),
        }
    }
    fn device_attestation_policy_wire_fixture() -> OfflineDeviceAttestationPolicy {
        OfflineDeviceAttestationPolicy {
            version: 1,
            trusted_roots: vec![OfflineDeviceAttestationTrustedRoot {
                platform: OFFLINE_DEVICE_ATTESTATION_IOS_APP_ATTEST_PLATFORM.to_owned(),
                der: vec![0x30, 0x01, 0x42],
                not_before_ms: Some(1_700_000_000_000),
                not_after_ms: Some(1_900_000_000_000),
            }],
            revoked_certificate_tbs_sha256: vec![vec![0x51; 32]],
            ios_apps: vec![OfflineIosAppAttestationPolicy {
                team_id: "WIRETEAM1".to_owned(),
                bundle_id: "com.example.wire".to_owned(),
                environment: "production".to_owned(),
                allowed_validation_categories: vec![1, 10],
                allowed_bundle_versions: vec!["1.0".to_owned()],
            }],
            android_apps: vec![OfflineAndroidAppAttestationPolicy {
                package_name: "com.example.wire".to_owned(),
                signing_certificate_sha256: vec![vec![0x61; 32]],
            }],
            android_status_snapshot: Some(OfflineAndroidAttestationStatusSnapshotV1 {
                version: OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1,
                payload_sha256: digest(b"wire-bound Android attestation status"),
                response_date_ms: 1_800_000_000_000,
                last_modified_ms: Some(1_799_999_000_000),
                cache_max_age_seconds: 3_600,
                non_valid_serials: vec!["1ab".to_owned(), "fe10".to_owned()],
            }),
            require_ios_app_policy: true,
            require_android_app_policy: true,
        }
    }
    #[derive(Clone, Encode)]
    struct RetiredArtifactBindingFixture {
        generation: String,
        manifest_sha256: [u8; 32],
    }
    #[derive(Clone, Encode)]
    struct RetiredTopUpUnsignedFixture {
        asset: AssetId,
        amount: KagemushaScaledAmountV2,
        current_note: KagemushaSpendableNoteDescriptorV2,
        shield_evidence: KagemushaTopUpShieldEvidenceV2,
        artifact_binding: RetiredArtifactBindingFixture,
        operation_id: [u8; 32],
    }
    fn retired_top_up_fixture() -> RetiredTopUpUnsignedFixture {
        let network_id = kagemusha_test_network_id("v4-wire-test-network");
        let definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("test domain"),
            "rose".parse().expect("test asset name"),
        );
        let account = AccountId::new(
            KeyPair::from_seed(vec![77; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let amount = KagemushaScaledAmountV2 {
            atomic_units: 625,
            scale: 2,
        };
        let current_note = KagemushaSpendableNoteDescriptorV2 {
            network_id,
            asset: definition.clone(),
            note_commitment: digest(b"v4 wire note"),
            spend_nullifier: digest(b"v4 wire nullifier"),
            amount,
        };
        let backend: iroha_schema::Ident = KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.into();
        RetiredTopUpUnsignedFixture {
            asset: AssetId::new(definition, account),
            amount,
            current_note,
            shield_evidence: KagemushaTopUpShieldEvidenceV2 {
                initial_root: digest(b"v4 wire initial root"),
                finalized_root: digest(b"v4 wire finalized root"),
                leaf_index: 7,
                proof: ProofAttachment::new_ref(
                    backend.clone(),
                    ProofBox::new(backend.clone(), vec![1, 2, 3]),
                    VerifyingKeyId::new(backend, "v4-wire-shield"),
                ),
            },
            artifact_binding: RetiredArtifactBindingFixture {
                generation: "retired-wire-layout".to_owned(),
                manifest_sha256: digest(b"retired wire manifest"),
            },
            operation_id: digest(b"v4 wire operation"),
        }
    }
    #[test]
    fn topup_shield_reserves_the_complete_recursive_lifecycle() {
        assert_eq!(
            KAGEMUSHA_RECURSIVE_SPEND_MAX_FUTURE_OUTPUTS_V2,
            u32::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2)
                + KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
        );
        assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_MAX_FUTURE_OUTPUTS_V2, 72);
        assert_eq!(KAGEMUSHA_TOPUP_SHIELD_INSERTION_CAPACITY_V2, 65_463);
        let last_insertable_leaf = KAGEMUSHA_TOPUP_SHIELD_INSERTION_CAPACITY_V2 - 1;
        assert_eq!(
            last_insertable_leaf + 1 + KAGEMUSHA_RECURSIVE_SPEND_MAX_FUTURE_OUTPUTS_V2,
            KAGEMUSHA_TOPUP_SHIELD_TREE_CAPACITY_V2 - 1,
            "the complete future-output budget must leave one in-range empty frontier leaf"
        );

        let backend: iroha_schema::Ident = KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.into();
        let mut proof = ProofAttachment::new_ref(
            backend.clone(),
            ProofBox::new(backend.clone(), vec![1, 2, 3]),
            VerifyingKeyId::new(backend, "topup-shield-v2"),
        );
        proof.vk_commitment = Some(digest(b"top-up shield verifier"));
        let mut evidence = KagemushaTopUpShieldEvidenceV2 {
            initial_root: digest(b"top-up initial root"),
            finalized_root: digest(b"top-up finalized root"),
            leaf_index: last_insertable_leaf,
            proof,
        };
        evidence
            .validate_public_binding()
            .expect("the last insertable leaf retains the full lifecycle reserve");
        evidence.leaf_index = KAGEMUSHA_TOPUP_SHIELD_INSERTION_CAPACITY_V2;
        assert!(matches!(
            evidence.validate_public_binding(),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "shield_evidence"
            })
        ));
    }
    #[test]
    fn offline_note_inputs_reject_zero_network_identity() {
        let mut note = retired_top_up_fixture().current_note;
        note.network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            crate::block::BlockHeader,
        >::from_untyped_unchecked(
            Hash::prehashed([0; Hash::LENGTH])
        ));
        assert!(note.validate_public_binding().is_err());
        let request = KagemushaRecipientOutputDerivationRequestV2 {
            network_id: note.network_id,
            asset: note.asset,
            amount: note.amount,
            request_id: digest(b"zero-network recipient output request"),
        };
        assert!(request.validate().is_err());
    }
    include!("kagemusha_release_generation_profile_inline_test.rs");
    #[test]
    fn v4_public_input_schema_tracks_the_authenticated_dynamic_layout() {
        assert_eq!(
            KAGEMUSHA_RECURSIVE_SPEND_STEP_OPERATION_FIELD_ELEMENTS_V4,
            135
        );
        assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_STEP_OPERATION_LIMBS_V4, 1_080);
        let minimum_layout =
            KagemushaPastaPublicLayoutV4::for_ipa_round_count(KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4)
                .expect("minimum V4 public layout");
        let maximum_layout =
            KagemushaPastaPublicLayoutV4::for_ipa_round_count(KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4)
                .expect("maximum V4 public layout");
        assert_eq!(minimum_layout.accumulator_limbs, 38);
        assert_eq!(minimum_layout.live_selector_offset, 65);
        assert_eq!(minimum_layout.instance_column_limbs, 66);
        assert_eq!(maximum_layout, minimum_layout);
        assert_eq!(
            usize::try_from(minimum_layout.instance_column_limbs)
                .expect("minimum V4 layout fits usize"),
            KAGEMUSHA_RECURSIVE_SPEND_STEP_MIN_PUBLIC_INPUT_LIMBS_V4,
        );
        assert_eq!(
            usize::try_from(maximum_layout.instance_column_limbs)
                .expect("maximum V4 layout fits usize"),
            KAGEMUSHA_RECURSIVE_SPEND_STEP_MAX_PUBLIC_INPUT_LIMBS_V4,
        );
        assert_eq!(KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4, 5);
        let mut maximum_params = circuit_params();
        maximum_params.k = KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4;
        maximum_params.lookup_bits = KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4 - 1;
        maximum_params.public_input_limbs = maximum_layout.instance_column_limbs;
        assert_eq!(
            maximum_params.validate().expect("maximum V4 parameters"),
            maximum_layout,
        );
        let mut below_minimum = circuit_params();
        below_minimum.k = KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4 - 1;
        below_minimum.lookup_bits = below_minimum.k - 1;
        below_minimum.public_input_limbs = 66;
        assert!(below_minimum.validate().is_err());
        let mut above_maximum = circuit_params();
        above_maximum.k = KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4 + 1;
        above_maximum.lookup_bits = above_maximum.k - 1;
        above_maximum.public_input_limbs = 66;
        assert!(above_maximum.validate().is_err());
        for schema in [
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PUBLIC_INPUTS_SCHEMA_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PUBLIC_INPUTS_SCHEMA_V4,
        ] {
            let schema = core::str::from_utf8(schema).expect("static schema is UTF-8");
            assert!(schema.contains("\"elements\":66"));
            assert!(schema.contains("\"ipa_round_count\":17"));
            assert!(schema.contains("\"semantic_authority\":\"step_eq\""));
            assert!(!schema.contains("\"state_layout_version\":2"));
            assert!(!schema.contains("\"state_limbs\":890"));
            assert!(schema.contains("\"ipa_accumulator\":{\"wire_version\":5,\"elements\":38"));
            assert!(schema.contains("\"live_selector\""));
            assert!(!schema.contains("4156"));
            assert!(!schema.contains("4172"));
            assert!(!schema.contains("[106]"));
            assert!(!schema.contains("krv2"));
            assert!(!schema.contains("krv3"));
        }
        let step_eq_schema =
            core::str::from_utf8(KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PUBLIC_INPUTS_SCHEMA_V4)
                .expect("static Eq schema is UTF-8");
        assert!(
            step_eq_schema
                .contains("\"private_witness\":{\"state_layout_version\":5,\"state_limbs\":138")
        );
        assert!(step_eq_schema.contains("\"operation_field_elements\":135"));
        assert!(step_eq_schema.contains("\"operation_limbs\":1080"));
        assert_eq!(
            kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v4(),
            <[u8; 32]>::from(Hash::new(
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PUBLIC_INPUTS_SCHEMA_V4,
            ))
        );
        assert_eq!(
            kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v4(),
            <[u8; 32]>::from(Hash::new(
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PUBLIC_INPUTS_SCHEMA_V4,
            ))
        );
        let manifest = manifest();
        assert_eq!(
            manifest.profiles[0].circuit_id,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4
        );
        assert_eq!(
            manifest.profiles[1].circuit_id,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4
        );
        assert_eq!(
            manifest.bridge_abi_version,
            KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
        );
    }
    include!("kagemusha_v4_release_tail_inline_tests.rs");
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the cohesive attestation digest-cycle tamper matrix shares one signed fixture"
    )]
    fn v4_attestation_subject_breaks_only_the_attestation_digest_cycle() {
        let benchmark = b"signed V4 physical-device benchmark evidence".to_vec();
        let roles = [
            KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
        ];
        let key_pairs = [
            KeyPair::from_seed(vec![21; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![22; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![23; 32], Algorithm::Ed25519),
        ];
        let mut manifest = manifest();
        let candidate = unsigned_candidate(&manifest);
        let review = signed_review_bytes(&candidate, &[&key_pairs[1]]);
        let internal_validation_receipt = internal_validation_receipt_bytes(&candidate, &manifest);
        assert_eq!(
            digest(&internal_validation_receipt),
            manifest.internal_validation_receipt_sha256
        );
        manifest.benchmark_evidence_sha256 = digest(&benchmark);
        manifest.cryptographic_review_sha256 = digest(&review);
        manifest.release_attestation_sha256 = digest(b"first nonzero staging digest");
        let first_subject = manifest
            .release_attestation_subject()
            .expect("first V4 attestation subject");
        manifest.release_attestation_sha256 = digest(b"second nonzero staging digest");
        let second_subject = manifest
            .release_attestation_subject()
            .expect("second V4 attestation subject");
        assert_eq!(first_subject, second_subject);
        assert_eq!(
            second_subject.authenticated_source_seal_projection_sha256,
            manifest.authenticated_source_seal_projection_sha256
        );
        assert_eq!(
            second_subject.reviewed_cargo_binary_sha256,
            manifest.reviewed_cargo_binary_sha256
        );
        assert_eq!(
            second_subject.reviewed_rustc_binary_sha256,
            manifest.reviewed_rustc_binary_sha256
        );
        assert_eq!(
            second_subject.internal_validation_receipt_sha256,
            manifest.internal_validation_receipt_sha256
        );
        let assert_subject_changes = |tampered: &mut _| {
            let candidate = unsigned_candidate(tampered);
            tampered.qualified_candidate_sha256 =
                kagemusha_recursive_spend_qualified_candidate_sha256_v4(
                    candidate.sha256().expect("modified V4 candidate identity"),
                    tampered.qualification_receipt_sha256,
                );
            let subject = tampered
                .release_attestation_subject()
                .expect("valid modified V4 subject");
            assert_ne!(second_subject, subject);
        };
        let mut params_tamper = manifest.clone();
        let params = &mut params_tamper.profiles[0].circuit_params;
        params.minimum_unusable_rows += 1;
        assert_subject_changes(&mut params_tamper);
        let mut bootstrap_tamper = manifest.clone();
        bootstrap_tamper.profiles[0].artifacts[3].payload_sha256[0] ^= 1;
        assert_subject_changes(&mut bootstrap_tamper);
        let mut projection_tamper = manifest.clone();
        projection_tamper.authenticated_source_seal_projection_sha256[0] ^= 1;
        assert_subject_changes(&mut projection_tamper);
        let mut cargo_tamper = manifest.clone();
        cargo_tamper.reviewed_cargo_binary_sha256[0] ^= 1;
        assert_subject_changes(&mut cargo_tamper);
        let mut rustc_tamper = manifest.clone();
        rustc_tamper.reviewed_rustc_binary_sha256[0] ^= 1;
        assert_subject_changes(&mut rustc_tamper);
        let mut internal_receipt_tamper = manifest.clone();
        internal_receipt_tamper.internal_validation_receipt_sha256[0] ^= 1;
        assert_subject_changes(&mut internal_receipt_tamper);
        let policy = KagemushaRecursiveSpendReleasePolicyV1 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            policy_id: "v4-artifact-test-policy".to_owned(),
            internal_validation_runner_identity_sha256:
                KagemushaRecursiveSpendInternalValidationReceiptV1::decode_canonical(
                    &internal_validation_receipt,
                )
                .expect("canonical internal-validation receipt")
                .body
                .validation_runner_identity_sha256,
            roles: roles
                .iter()
                .zip(&key_pairs)
                .map(
                    |(&role, key_pair)| KagemushaRecursiveSpendReleaseRolePolicyV1 {
                        role,
                        threshold: 1,
                        authorized_signers: vec![key_pair.public_key().clone()],
                    },
                )
                .collect(),
        };
        let subject = manifest
            .release_attestation_subject()
            .expect("signed V4 subject");
        let attestation = KagemushaRecursiveSpendReleaseAttestationV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
            subject: subject.clone(),
            approvals: roles
                .iter()
                .zip(&key_pairs)
                .map(
                    |(&role, key_pair)| KagemushaRecursiveSpendReleaseApprovalV4 {
                        role,
                        public_key: key_pair.public_key().clone(),
                        signature: SignatureOf::try_new(
                            key_pair.private_key(),
                            &subject.approval_payload(role),
                        )
                        .expect("test V4 release signature"),
                    },
                )
                .collect(),
        };
        manifest.release_attestation_sha256 =
            digest(&norito::encode_canonical(&attestation).expect("canonical V4 attestation"));
        let authenticated = KagemushaAuthenticatedReleaseV4::verify(
            &manifest,
            &policy,
            &attestation,
            &internal_validation_receipt,
            &benchmark,
            &review,
        )
        .expect("fully authenticated V4 release");
        assert_eq!(authenticated.manifest(), &manifest);
        assert_eq!(authenticated.approved_signers().len(), roles.len());
        let mut wrong_runner_policy = policy.clone();
        wrong_runner_policy.internal_validation_runner_identity_sha256[0] ^= 1;
        assert_eq!(
            KagemushaAuthenticatedReleaseV4::verify(
                &manifest,
                &wrong_runner_policy,
                &attestation,
                &internal_validation_receipt,
                &benchmark,
                &review,
            ),
            Err(KagemushaReleaseVerificationError::InvalidInternalValidationReceipt),
            "a valid self-declared runner signature is not an authorization root",
        );
        let mut unpinned_runner_policy = policy.clone();
        unpinned_runner_policy.internal_validation_runner_identity_sha256 = [0; 32];
        assert_eq!(
            KagemushaAuthenticatedReleaseV4::verify(
                &manifest,
                &unpinned_runner_policy,
                &attestation,
                &internal_validation_receipt,
                &benchmark,
                &review,
            ),
            Err(KagemushaReleaseVerificationError::InvalidPolicy),
            "an authenticated V4 release policy must name a runner trust root",
        );
        let mut tampered_internal_validation_receipt = internal_validation_receipt.clone();
        tampered_internal_validation_receipt[0] ^= 1;
        assert_eq!(
            KagemushaAuthenticatedReleaseV4::verify(
                &manifest,
                &policy,
                &attestation,
                &tampered_internal_validation_receipt,
                &benchmark,
                &review,
            ),
            Err(KagemushaReleaseVerificationError::InvalidInternalValidationReceipt)
        );
        let mismatched_lock_receipt = norito::encode_canonical(
            &kagemusha_internal_validation_receipt::internal_validation_receipt_tests::
                signed_receipt_for_v4_candidate_with_tracked_cargo_lock(
                    &candidate,
                    &manifest,
                    [0xD4; 32],
                    manifest
                        .reviewed_source_closure
                        .tracked_cargo_lock_size_bytes
                        + 1,
                ),
        )
        .expect("canonical receipt with a mismatched tracked Cargo.lock");
        let mut mismatched_lock_manifest = manifest.clone();
        mismatched_lock_manifest.internal_validation_receipt_sha256 =
            digest(&mismatched_lock_receipt);
        assert_eq!(
            validate_internal_validation_receipt_v4(
                &mismatched_lock_manifest,
                &candidate,
                &mismatched_lock_receipt,
                None,
            ),
            Err(KagemushaReleaseVerificationError::InvalidInternalValidationReceipt),
            "a correctly signed receipt cannot substitute a different tracked Cargo.lock"
        );
        let alternate_reviewer = KeyPair::from_seed(vec![24; 32], Algorithm::Ed25519);
        let mut mismatched_policy = policy.clone();
        mismatched_policy.roles[1]
            .authorized_signers
            .push(alternate_reviewer.public_key().clone());
        mismatched_policy.roles[1].authorized_signers.sort();
        let mut mismatched_attestation = attestation.clone();
        let mismatched_review_approval = mismatched_attestation
            .approvals
            .iter_mut()
            .find(|approval| {
                approval.role == KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview
            })
            .expect("cryptographic-review attestation approval");
        mismatched_review_approval.public_key = alternate_reviewer.public_key().clone();
        mismatched_review_approval.signature = SignatureOf::try_new(
            alternate_reviewer.private_key(),
            &subject.approval_payload(
                KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            ),
        )
        .expect("alternate release-review signature");
        let mut mismatched_manifest = manifest.clone();
        mismatched_manifest.release_attestation_sha256 = digest(
            &norito::encode_canonical(&mismatched_attestation)
                .expect("mismatched canonical attestation"),
        );
        assert_eq!(
            KagemushaAuthenticatedReleaseV4::verify(
                &mismatched_manifest,
                &mismatched_policy,
                &mismatched_attestation,
                &internal_validation_receipt,
                &benchmark,
                &review,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );
        let mut signed_params_tamper = manifest.clone();
        let params = &mut signed_params_tamper.profiles[0].circuit_params;
        params.minimum_unusable_rows += 1;
        assert_subject_changes(&mut signed_params_tamper);
        assert_eq!(
            KagemushaAuthenticatedReleaseV4::verify(
                &signed_params_tamper,
                &policy,
                &attestation,
                &internal_validation_receipt,
                &benchmark,
                &review,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );
        let mut signed_bootstrap_tamper = manifest;
        signed_bootstrap_tamper.profiles[0].artifacts[3].payload_sha256[0] ^= 1;
        assert_subject_changes(&mut signed_bootstrap_tamper);
        assert_eq!(
            KagemushaAuthenticatedReleaseV4::verify(
                &signed_bootstrap_tamper,
                &policy,
                &attestation,
                &internal_validation_receipt,
                &benchmark,
                &review,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the cohesive canonicalization and signature tamper matrix shares one candidate"
    )]
    fn v4_cryptographic_review_is_canonical_signed_and_candidate_bound() {
        let candidate = unsigned_candidate(&manifest());
        let qualification_receipt_sha256 = qualification_receipt_sha256();
        let qualified_candidate_sha256 = kagemusha_recursive_spend_qualified_candidate_sha256_v4(
            candidate.sha256().expect("test candidate identity"),
            qualification_receipt_sha256,
        );
        let review_subject = candidate
            .cryptographic_review_subject(qualification_receipt_sha256, qualified_candidate_sha256)
            .expect("candidate-bound review subject");
        assert_eq!(
            review_subject.authenticated_source_seal_projection_sha256,
            candidate
                .manifest
                .authenticated_source_seal_projection_sha256
        );
        assert_eq!(
            review_subject.reviewed_cargo_binary_sha256,
            candidate.manifest.reviewed_cargo_binary_sha256
        );
        assert_eq!(
            review_subject.reviewed_rustc_binary_sha256,
            candidate.manifest.reviewed_rustc_binary_sha256
        );
        let reviewer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let review_bytes = signed_review_bytes(&candidate, &[&reviewer]);
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                &review_bytes,
                &candidate,
                qualification_receipt_sha256,
                qualified_candidate_sha256,
            )
            .expect("canonical signed review"),
            vec![reviewer.public_key().clone()]
        );
        assert!(
            KagemushaRecursiveSpendCryptographicReviewPayloadV4::approved(
                &candidate,
                qualification_receipt_sha256,
                qualified_candidate_sha256,
                [0; 32],
                [
                    [0x91; 32], [0x92; 32], [0x93; 32], [0x94; 32], [0x95; 32], [0x96; 32]
                ],
            )
            .is_err(),
            "the producer constructor must reject an absent report digest"
        );
        assert!(
            KagemushaRecursiveSpendCryptographicReviewPayloadV4::approved(
                &candidate,
                qualification_receipt_sha256,
                qualified_candidate_sha256,
                [0x90; 32],
                [[0x91; 32]; KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_CHECK_COUNT_V4],
            )
            .is_err(),
            "the producer constructor must reject duplicate check evidence"
        );
        let oversized_review =
            vec![0; KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4 + 1];
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                &oversized_review,
                &candidate,
                qualification_receipt_sha256,
                qualified_candidate_sha256,
            ),
            Err(KagemushaReleaseVerificationError::EvidenceMismatch {
                role: KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            })
        );
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                b"approved by independent review",
                &candidate,
                qualification_receipt_sha256,
                qualified_candidate_sha256,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );
        let mut wrong_candidate = candidate.clone();
        wrong_candidate.manifest.activation_height += 1;
        wrong_candidate
            .validate()
            .expect("valid distinct review candidate");
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                &review_bytes,
                &wrong_candidate,
                qualification_receipt_sha256,
                qualified_candidate_sha256,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );
        let review: KagemushaRecursiveSpendCryptographicReviewEvidenceV4 =
            norito::decode_canonical(&review_bytes).expect("decode canonical test review");
        let alternate_review_bytes = encode_with_alternate_norito_layout(&review);
        assert_ne!(alternate_review_bytes, review_bytes);
        assert_eq!(
            norito::decode_from_bytes::<KagemushaRecursiveSpendCryptographicReviewEvidenceV4>(
                &alternate_review_bytes,
            )
            .expect("alternate-layout review remains structurally decodable"),
            review
        );
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                &alternate_review_bytes,
                &candidate,
                qualification_receipt_sha256,
                qualified_candidate_sha256,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );
        let mut rejected = review.clone();
        rejected.payload.decision = KagemushaRecursiveSpendCryptographicReviewDecisionV4::Rejected;
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                &norito::encode_canonical(&rejected).expect("rejected review bytes"),
                &candidate,
                qualification_receipt_sha256,
                qualified_candidate_sha256,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );
        let mut failed_check = review.clone();
        failed_check.payload.checks[0].status =
            KagemushaRecursiveSpendCryptographicReviewCheckStatusV4::Failed;
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                &norito::encode_canonical(&failed_check).expect("failed-check review bytes"),
                &candidate,
                qualification_receipt_sha256,
                qualified_candidate_sha256,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );
        let mut duplicate_digest = review.clone();
        duplicate_digest.payload.checks[1].evidence_sha256 =
            duplicate_digest.payload.checks[0].evidence_sha256;
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                &norito::encode_canonical(&duplicate_digest).expect("duplicate-digest review bytes"),
                &candidate,
                qualification_receipt_sha256,
                qualified_candidate_sha256,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );
        let impostor = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let mut invalid_signature = review.clone();
        invalid_signature.approvals[0].signature =
            SignatureOf::try_new(impostor.private_key(), &invalid_signature.payload)
                .expect("impostor review signature");
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                &norito::encode_canonical(&invalid_signature).expect("invalid-signature review bytes"),
                &candidate,
                qualification_receipt_sha256,
                qualified_candidate_sha256,
            ),
            Err(KagemushaReleaseVerificationError::InvalidSignature {
                role: KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            })
        );
    }
    #[test]
    fn v4_candidate_precedes_and_excludes_external_evidence() {
        let finalized = manifest();
        finalized.validate().expect("valid finalized V4 manifest");
        assert!(
            finalized.validate_unsigned_candidate().is_err(),
            "a finalized manifest must not be accepted as an unsigned candidate"
        );
        let candidate = unsigned_candidate(&finalized);
        assert!(
            candidate.manifest.validate().is_err(),
            "production manifest validation must reject a pre-evidence candidate"
        );
        candidate
            .manifest
            .validate_unsigned_candidate()
            .expect("valid unsigned V4 candidate");
        candidate.validate().expect("valid candidate record");
        assert_ne!(candidate.sha256().expect("candidate digest"), [0; 32]);
        let mut missing_projection = candidate.clone();
        missing_projection
            .manifest
            .authenticated_source_seal_projection_sha256 = [0; 32];
        assert!(missing_projection.validate().is_err());
        let mut missing_cargo = candidate.clone();
        missing_cargo.manifest.reviewed_cargo_binary_sha256 = [0; 32];
        assert!(missing_cargo.validate().is_err());
        let mut missing_rustc = candidate.clone();
        missing_rustc.manifest.reviewed_rustc_binary_sha256 = [0; 32];
        assert!(missing_rustc.validate().is_err());
        let mut unreviewed = candidate;
        unreviewed
            .manifest
            .reviewed_source_closure
            .source_tree_sha256 = digest(b"unreviewed source tree");
        assert!(unreviewed.validate().is_err());
    }
    #[test]
    fn v4_capabilities_require_exact_eight_roles_and_release_cap() {
        let mut capabilities = KagemushaRecursiveSpendNativeCapabilitiesV4 {
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            artifact_manifest_schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4
                .to_owned(),
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
            proof_envelope_version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
            step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
            step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4.to_owned(),
            artifact_roles: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
                .map(str::to_owned)
                .to_vec(),
            max_proof_bytes: manifest().max_proof_bytes,
            proof_backend_available: true,
            missing_gates: Vec::new(),
        };
        capabilities.validate().expect("valid V4 capabilities");
        capabilities.artifact_roles.swap(0, 1);
        assert!(capabilities.validate().is_err());
        capabilities.artifact_roles = KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
            .map(str::to_owned)
            .to_vec();
        capabilities.proof_backend_available = false;
        capabilities.missing_gates = vec![
            "artifact_install".to_owned(),
            "physical_benchmark".to_owned(),
        ];
        capabilities
            .validate()
            .expect("canonical unavailable V4 capabilities");
        capabilities.missing_gates.reverse();
        assert!(capabilities.validate().is_err());
    }
    #[test]
    fn all_v4_lifecycle_schemas_reach_the_v4_artifact_binding() {
        for schema in [
            KagemushaRecursiveSpendTopUpRequestV4::schema(),
            KagemushaRecursiveSpendBundleV4::schema(),
            KagemushaRecursiveSpendInitRequestV4::schema(),
            KagemushaRecursiveSpendAppendRequestV4::schema(),
            KagemushaRecursiveSpendVerifyRequestV4::schema(),
            KagemushaRecursiveSpendRedeemRequestV4::schema(),
        ] {
            assert!(schema.contains_key::<KagemushaRecursiveSpendArtifactBindingV4>());
        }
        assert!(
            KagemushaRecursiveSpendBundleV4::schema()
                .contains_key::<KagemushaRecursiveSpendOperationVectorV4>()
        );
    }
    #[test]
    fn v4_top_up_wire_roundtrips_and_rejects_the_retired_layout() {
        let legacy = retired_top_up_fixture();
        let v4 = KagemushaRecursiveSpendTopUpUnsignedV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            asset: legacy.asset.clone(),
            amount: legacy.amount,
            current_note: legacy.current_note.clone(),
            shield_evidence: legacy.shield_evidence.clone(),
            artifact_binding: KagemushaRecursiveSpendArtifactBindingV4 {
                version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
                generation: "v4-wire-layout".to_owned(),
                manifest_sha256: digest(b"v4 wire manifest"),
            },
            operation_id: legacy.operation_id,
        };
        let encoded = v4.encode();
        let decoded =
            <KagemushaRecursiveSpendTopUpUnsignedV4 as norito::codec::DecodeAll>::decode_all(
                &mut encoded.as_slice(),
            )
            .expect("V4 top-up wire must roundtrip");
        assert_eq!(decoded, v4);
        let legacy_encoded = legacy.encode();
        assert!(
            <KagemushaRecursiveSpendTopUpUnsignedV4 as norito::codec::DecodeAll>::decode_all(
                &mut legacy_encoded.as_slice(),
            )
            .is_err(),
            "the legacy field layout must not decode as ABI-21"
        );
    }
    include!("kagemusha_activation_instruction_inline_tests.rs");
    #[test]
    fn v4_verifier_ids_are_manifest_and_parity_qualified() {
        let manifest_sha256 = [0xab; 32];
        let eq = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            manifest_sha256,
        );
        let ep = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEp,
            manifest_sha256,
        );
        let suffix = "ab".repeat(32);
        assert_eq!(
            eq.name,
            format!("{KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4}-{suffix}")
        );
        assert_eq!(
            ep.name,
            format!("{KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4}-{suffix}")
        );
        assert_eq!(
            eq.backend.as_str(),
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
        );
        assert_ne!(eq, ep);
        assert_ne!(
            eq,
            kagemusha_recursive_spend_verifier_key_id_v4(
                KagemushaPastaCycleParityV1::StepEq,
                [0xac; 32],
            )
        );
    }
}
#[cfg(all(test, feature = "transparent_api"))]
pub(crate) fn lifecycle_enable_witness_wire_fixture() -> KagemushaV4IssuanceEnableWitnessV1 {
    kagemusha_v4_artifact_contract_tests::lifecycle_enable_witness_wire_fixture()
}
include!("device_authority_p256_tests.rs");
/// Derive the canonical public reference carried by a receiver payment request.
///
/// # Errors
///
/// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
pub fn kagemusha_receiver_key_reference_v2(
    receiver_public_key: &KagemushaDevicePublicKeyV2,
) -> Result<[u8; 32], KagemushaValidationError> {
    receiver_public_key.validate()?;
    kagemusha_poseidon_preimage(&KagemushaReceiverKeyReferencePreimageV2 {
        domain: KAGEMUSHA_RECEIVER_KEY_REFERENCE_DOMAIN_V2.to_owned(),
        receiver_public_key: *receiver_public_key,
    })
}
impl KagemushaReceiverAcknowledgementPayloadV2 {
    /// Validate structural fields and the domain-separated public-key reference.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.receiver_public_key.validate()?;
        if self.operation_id == [0; 32]
            || self.recipient_request_digest == [0; 32]
            || self.payment_bundle_digest == [0; 32]
            || self.recipient_commitment == [0; 32]
            || self.accepted_at_ms == 0
            || self.receiver_device_id.is_empty()
            || self.receiver_device_id.len() > 128
            || self.receiver_device_id.trim() != self.receiver_device_id
            || self.receiver_device_id.chars().any(char::is_control)
            || self.receiver_key_reference
                != kagemusha_receiver_key_reference_v2(&self.receiver_public_key)?
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "receiver_acknowledgement.payload",
            });
        }
        Ok(())
    }
    /// Return the exact domain-separated bytes signed by the receiver device key.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the signing subject is invalid or cannot be encoded canonically.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        self.validate_public_binding()?;
        Ok(norito::encode_canonical(
            &KagemushaReceiverAcknowledgementSigningPreimageV2 {
                domain: KAGEMUSHA_RECEIVER_ACKNOWLEDGEMENT_DOMAIN_V2.to_owned(),
                payload: self.clone(),
            },
        )?)
    }
    /// Stable persistence/replay key for byte-identical duplicate acknowledgements.
    #[must_use]
    pub fn idempotency_key(&self) -> ([u8; 32], [u8; 32]) {
        (self.operation_id, self.recipient_request_digest)
    }
}
impl KagemushaReceiverAcknowledgementV2 {
    /// Return the canonical identity digest of the signed acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.payload.validate_public_binding()?;
        self.signature.verify(
            &self.payload.receiver_public_key,
            &self.payload.signing_bytes()?,
        )?;
        kagemusha_poseidon_preimage(&KagemushaReceiverAcknowledgementDigestPreimageV2 {
            domain: KAGEMUSHA_RECEIVER_ACKNOWLEDGEMENT_DIGEST_DOMAIN_V2.to_owned(),
            acknowledgement: self.clone(),
        })
    }
    /// Verify the unchanged ACK leaf against an authoritative ABI-21 recipient bundle.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_payment_v4(
        &self,
        recipient_request: &KagemushaRecipientPaymentRequestV2,
        recipient_bundle: &KagemushaRecursiveSpendBundleV4,
    ) -> Result<(), KagemushaValidationError> {
        self.payload.validate_public_binding()?;
        recipient_request.validate_public_binding()?;
        recipient_bundle.validate_public_binding()?;
        let Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(transition)) =
            recipient_bundle.statement.transition.as_ref()
        else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "receiver_acknowledgement.v4.split",
            });
        };
        if transition.branch != KagemushaRecursiveSpendBranchV2::Recipient
            || recipient_bundle.statement.current_note != recipient_request.recipient_output
            || transition.recipient_request_digest != recipient_request.digest()?
            || self.payload.operation_id != transition.operation_id
            || self.payload.recipient_request_digest != recipient_request.digest()?
            || self.payload.payment_bundle_digest != recipient_bundle.digest()?
            || self.payload.recipient_commitment
                != recipient_bundle.statement.current_note.note_commitment
            || self.payload.recipient_commitment
                != recipient_request.recipient_output.note_commitment
            || self.payload.receiver_key_reference != recipient_request.recipient_key_reference
            || self.payload.receiver_device_id != recipient_request.receiver_device_id
            || self.payload.receiver_public_key != recipient_request.receiver_public_key
            || self.payload.accepted_at_ms >= recipient_request.expires_at_ms
            || self.payload.accepted_at_ms < recipient_request.issued_at_ms
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "receiver_acknowledgement.v4.binding",
            });
        }
        self.signature
            .verify(
                &self.payload.receiver_public_key,
                &self.payload.signing_bytes()?,
            )
            .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "receiver_acknowledgement.v4.signature",
            })?;
        let encoded_len = norito::encode_canonical(self)?.len();
        if encoded_len > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4 {
            return Err(KagemushaValidationError::EncodedSizeExceeded {
                max: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4,
                actual: encoded_len,
            });
        }
        Ok(())
    }
    /// Return canonical ACK bytes after ABI-21 payment validation.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the source value is invalid or the requested canonical result cannot be derived.
    pub fn canonical_archive_for_payment_v4(
        &self,
        recipient_request: &KagemushaRecipientPaymentRequestV2,
        recipient_bundle: &KagemushaRecursiveSpendBundleV4,
    ) -> Result<Vec<u8>, KagemushaValidationError> {
        self.validate_for_payment_v4(recipient_request, recipient_bundle)?;
        Ok(norito::encode_canonical(self)?)
    }
    /// Build the unchanged typed ACK result after ABI-21 payment validation.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the source value is invalid or the requested canonical result cannot be derived.
    pub fn verified_result_v4(
        &self,
        recipient_request: &KagemushaRecipientPaymentRequestV2,
        recipient_bundle: &KagemushaRecursiveSpendBundleV4,
    ) -> Result<KagemushaReceiverAcknowledgementVerifyResultV2, KagemushaValidationError> {
        self.validate_for_payment_v4(recipient_request, recipient_bundle)?;
        Ok(KagemushaReceiverAcknowledgementVerifyResultV2 {
            valid: true,
            operation_id: self.payload.operation_id,
            recipient_request_digest: self.payload.recipient_request_digest,
            payment_bundle_digest: self.payload.payment_bundle_digest,
            acknowledgement_digest: self.digest()?,
        })
    }
}
impl KagemushaReceiverAcknowledgementVerifyResultV2 {
    /// Enforce fail-closed result consistency before a sender consumes inputs.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        if !self.valid
            || self.operation_id == [0; 32]
            || self.recipient_request_digest == [0; 32]
            || self.payment_bundle_digest == [0; 32]
            || self.acknowledgement_digest == [0; 32]
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "receiver_acknowledgement_verify_result",
            });
        }
        Ok(())
    }
}
fn validate_kagemusha_redeem_proof_attachment_v2(
    proof: &ProofAttachment,
) -> Result<(), KagemushaValidationError> {
    if proof.structural_error().is_some()
        || proof.backend.as_str() != KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND
        || proof.proof.backend.as_str() != KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND
        || proof.vk_ref.backend.as_str() != KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND
        || proof.vk_ref.name.is_empty()
        || proof.proof.bytes.is_empty()
        || proof.proof.bytes.len() > KAGEMUSHA_UNSHIELD_MAX_PROOF_BYTES_V4
        || proof
            .vk_commitment
            .is_none_or(|commitment| commitment == [0; 32])
        || proof.lane_privacy.is_some()
    {
        return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "redeem_proof",
        });
    }
    Ok(())
}
/// Domain separator for ABI-21 unsigned top-up authorization payloads.
pub const KAGEMUSHA_TOPUP_PAYLOAD_DIGEST_DOMAIN_V4: &str = "iroha:kagemusha:v4:topup-payload";
/// Domain separator for ABI-21 finalized top-up anchor receipts.
pub const KAGEMUSHA_TOPUP_ANCHOR_DIGEST_DOMAIN_V4: &str = "iroha:kagemusha:v4:topup-anchor";
/// Domain separator for an ABI-21 split transition binding.
pub const KAGEMUSHA_RECURSIVE_SPEND_SPLIT_BINDING_DIGEST_DOMAIN_V4: &str =
    "iroha:kagemusha:v4:split-binding";
/// Domain separator for an ABI-21 redemption transition binding.
pub const KAGEMUSHA_REDEMPTION_TRANSITION_DIGEST_DOMAIN_V4: &str =
    "iroha:kagemusha:v4:redemption-transition";
/// Domain separator for an ABI-21 recursive public statement.
pub const KAGEMUSHA_RECURSIVE_SPEND_PUBLIC_STATEMENT_DIGEST_DOMAIN_V4: &str =
    "iroha:kagemusha:v4:public-statement";
/// Exact finalized-anchor schema carried by an ABI-21 init request.
pub const KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4: u16 = 4;
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaTopUpAnchorDigestPreimageV4 {
    domain: String,
    version: u16,
    network_id: NetworkId,
    payer: AccountId,
    asset: AssetId,
    asset_scale: u32,
    amount: KagemushaScaledAmountV2,
    initial_root: [u8; 32],
    finalized_root: [u8; 32],
    shield_leaf_index: u32,
    current_note: KagemushaSpendableNoteDescriptorV2,
    topup_operation_id: [u8; 32],
    shield_verifier_id: VerifyingKeyId,
    shield_verifier_commitment: [u8; 32],
    artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
    finalized_height: u64,
    finalized_tx_hash: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaTopUpUnsignedPayloadDigestPreimageV4 {
    domain: String,
    version: u16,
    asset: AssetId,
    amount: KagemushaScaledAmountV2,
    current_note: KagemushaSpendableNoteDescriptorV2,
    shield_evidence: KagemushaTopUpShieldEvidenceV2,
    artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
    operation_id: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRecursiveSpendSplitBindingDigestPreimageV4 {
    domain: String,
    split: KagemushaRecursiveSpendSplitIntentV4,
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRedemptionTransitionDigestPreimageV4 {
    domain: String,
    intent: KagemushaRecursiveSpendRedemptionIntentV4,
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRecursiveSpendPublicStatementDigestPreimageV4 {
    domain: String,
    statement: KagemushaRecursiveSpendPublicStatementV4,
}
impl KagemushaRecursiveSpendTopUpAnchorV4 {
    /// Populate and validate the canonical ABI-21 receipt digest.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the source value is invalid or the requested canonical result cannot be derived.
    pub fn finalize_digest(mut self) -> Result<Self, KagemushaValidationError> {
        self.anchor_digest = self.compute_anchor_digest()?;
        self.validate_public_binding()?;
        Ok(self)
    }
    /// Compute the V4-domain digest of every immutable receipt field.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn compute_anchor_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        kagemusha_poseidon_preimage(&KagemushaTopUpAnchorDigestPreimageV4 {
            domain: KAGEMUSHA_TOPUP_ANCHOR_DIGEST_DOMAIN_V4.to_owned(),
            version: self.version,
            network_id: self.network_id,
            payer: self.payer.clone(),
            asset: self.asset.clone(),
            asset_scale: self.asset_scale,
            amount: self.amount,
            initial_root: self.initial_root,
            finalized_root: self.finalized_root,
            shield_leaf_index: self.shield_leaf_index,
            current_note: self.current_note.clone(),
            topup_operation_id: self.topup_operation_id,
            shield_verifier_id: self.shield_verifier_id.clone(),
            shield_verifier_commitment: self.shield_verifier_commitment,
            artifact_binding: self.artifact_binding.clone(),
            finalized_height: self.finalized_height,
            finalized_tx_hash: self.finalized_tx_hash,
        })
    }
    /// Validate the complete finalized receipt and authenticated V4 release.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.amount.validate()?;
        self.current_note.validate_public_binding()?;
        self.artifact_binding.validate()?;
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4
            || self.asset_scale != self.amount.scale
            || self.current_note.amount != self.amount
            || self.asset.account() != &self.payer
            || self.current_note.network_id != self.network_id
            || self.current_note.asset != *self.asset.definition()
            || self.initial_root == [0; 32]
            || self.finalized_root == [0; 32]
            || self.initial_root == self.finalized_root
            || self.shield_leaf_index >= KAGEMUSHA_TOPUP_SHIELD_INSERTION_CAPACITY_V2
            || self.topup_operation_id == [0; 32]
            || self.shield_verifier_id.backend.as_str() != KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND
            || !self.shield_verifier_id.is_portable_registry_id()
            || self.shield_verifier_commitment == [0; 32]
            || self.finalized_height == 0
            || self.finalized_tx_hash == [0; 32]
            || self.anchor_digest != self.compute_anchor_digest()?
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_anchor.v4",
            });
        }
        Ok(())
    }
    /// Return the stable compact identity retained by ABI-21 descendants.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the source value is invalid or the requested canonical result cannot be derived.
    pub fn compact_ref(
        &self,
    ) -> Result<KagemushaRecursiveSpendTopUpAnchorRefV2, KagemushaValidationError> {
        self.validate_public_binding()?;
        Ok(KagemushaRecursiveSpendTopUpAnchorRefV2 {
            topup_operation_id: self.topup_operation_id,
            anchor_digest: self.anchor_digest,
        })
    }
}
impl KagemushaRecursiveSpendTopUpUnsignedV4 {
    /// Validate every ABI-21 top-up field before payer authorization is attached.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.amount.validate()?;
        self.current_note.validate_public_binding()?;
        self.artifact_binding.validate()?;
        self.shield_evidence.validate_public_binding()?;
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4
            || self.current_note.asset != *self.asset.definition()
            || self.current_note.amount != self.amount
            || self.operation_id == [0; 32]
            || self.current_note.note_commitment == self.current_note.spend_nullifier
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_request.v4",
            });
        }
        Ok(())
    }
    /// Return the V4-domain digest placed into payer authorization.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaTopUpUnsignedPayloadDigestPreimageV4 {
            domain: KAGEMUSHA_TOPUP_PAYLOAD_DIGEST_DOMAIN_V4.to_owned(),
            version: self.version,
            asset: self.asset.clone(),
            amount: self.amount,
            current_note: self.current_note.clone(),
            shield_evidence: self.shield_evidence.clone(),
            artifact_binding: self.artifact_binding.clone(),
            operation_id: self.operation_id,
        })
    }
    /// Attach matching payer authorization and produce the authoritative request.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn into_request(
        self,
        authorization: KagemushaRequestAuthorizationV2,
    ) -> Result<KagemushaRecursiveSpendTopUpRequestV4, KagemushaValidationError> {
        let request = KagemushaRecursiveSpendTopUpRequestV4 {
            version: self.version,
            asset: self.asset,
            amount: self.amount,
            current_note: self.current_note,
            shield_evidence: self.shield_evidence,
            artifact_binding: self.artifact_binding,
            operation_id: self.operation_id,
            authorization,
        };
        request.validate_public_binding()?;
        Ok(request)
    }
}
impl KagemushaRecursiveSpendTopUpRequestV4 {
    /// Construct and validate an ABI-21 online-to-offline request.
    #[allow(clippy::too_many_arguments)]
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn new(
        asset: AssetId,
        amount: KagemushaScaledAmountV2,
        current_note: KagemushaSpendableNoteDescriptorV2,
        shield_evidence: KagemushaTopUpShieldEvidenceV2,
        artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        operation_id: [u8; 32],
        authorization: KagemushaRequestAuthorizationV2,
    ) -> Result<Self, KagemushaValidationError> {
        KagemushaRecursiveSpendTopUpUnsignedV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            asset,
            amount,
            current_note,
            shield_evidence,
            artifact_binding,
            operation_id,
        }
        .into_request(authorization)
    }
    /// Reconstruct the exact V4 fields covered by payer authorization.
    #[must_use]
    pub fn unsigned_payload(&self) -> KagemushaRecursiveSpendTopUpUnsignedV4 {
        KagemushaRecursiveSpendTopUpUnsignedV4 {
            version: self.version,
            asset: self.asset.clone(),
            amount: self.amount,
            current_note: self.current_note.clone(),
            shield_evidence: self.shield_evidence.clone(),
            artifact_binding: self.artifact_binding.clone(),
            operation_id: self.operation_id,
        }
    }
    /// Validate the debit, note, release, and self-contained payer authorization.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        let encoded_len = norito::encode_canonical(self)?.len();
        ensure_kagemusha_encoded_size_at_most(
            encoded_len,
            KAGEMUSHA_RECURSIVE_SPEND_TOPUP_REQUEST_MAX_BYTES_V4,
        )?;
        let unsigned = self.unsigned_payload();
        unsigned.validate_public_binding()?;
        if self.asset.account() != &self.authorization.authority
            || self.asset.definition() != &self.authorization.asset_definition_id
            || self.authorization.operation_id != self.operation_id
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "authorization.v4",
            });
        }
        self.authorization.validate_for_payload(unsigned.digest()?)
    }
    /// Return the digest of every unsigned ABI-21 top-up field.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn unsigned_payload_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.unsigned_payload().digest()
    }
    /// Verify payer authorization at authoritative Torii time.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_authorization_at(&self, now_ms: u64) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        self.authorization
            .validate_for_payload_at(self.unsigned_payload_digest()?, now_ms)
    }
}
impl KagemushaRecursiveSpendInitRequestV4 {
    /// Validate finalized provenance and its exact authenticated ABI-21 release.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.topup_anchor.validate_public_binding()?;
        self.topup_finality_proof.validate_structure()?;
        self.topup_finality_roster_artifact.validate_structure()?;
        self.artifact_binding.validate()?;
        if self.artifact_binding != self.topup_anchor.artifact_binding
            || self.topup_finality_proof.anchor != self.topup_anchor.compact_ref()?
            || self.topup_finality_proof.commit_qc.height_context.height
                != self.topup_anchor.finalized_height
            || self.topup_finality_roster_artifact.network_id != self.topup_anchor.network_id
            || self.topup_finality_roster_artifact.network_id
                != self
                    .topup_finality_proof
                    .commit_qc
                    .height_context
                    .network_id
            || self.topup_finality_roster_artifact.artifact_generation
                != self.artifact_binding.generation
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "init_request.v4",
            });
        }
        let height = self.topup_anchor.finalized_height;
        let window = self.topup_finality_roster_artifact.window_at(height)?;
        self.topup_finality_proof
            .commit_qc
            .validate_for_roster_window(window)?;
        Ok(())
    }
}
impl KagemushaRecursiveSpendSplitIntentV4 {
    fn binding_digest_unchecked(&self) -> Result<[u8; 32], KagemushaValidationError> {
        kagemusha_poseidon_preimage(&KagemushaRecursiveSpendSplitBindingDigestPreimageV4 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_SPLIT_BINDING_DIGEST_DOMAIN_V4.to_owned(),
            split: self.clone(),
        })
    }
    fn output_branch_claims_for_binding(
        &self,
        branch: KagemushaRecursiveSpendBranchV2,
        binding: [u8; 32],
    ) -> Result<Vec<KagemushaRecursiveSpendBranchClaimV2>, KagemushaValidationError> {
        let mut claims = self
            .inputs
            .iter()
            .flat_map(|input| &input.branch_claims)
            .map(|claim| claim.child(branch, binding))
            .collect::<Result<Vec<_>, _>>()?;
        claims.sort_unstable_by_key(|claim| claim.path);
        validate_kagemusha_recursive_spend_branch_claims_v2(&claims)?;
        Ok(claims)
    }
    /// Validate exact conservation, canonical parents, and disjoint V4 outputs.
    #[expect(
        clippy::too_many_lines,
        reason = "the ordered validation pass preserves deterministic first-error semantics"
    )]
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.output_artifact_binding.validate()?;
        self.transfer_amount.validate()?;
        self.recipient_output.validate_public_binding()?;
        let lineage_roots =
            validate_kagemusha_recursive_spend_topup_anchor_refs_v2(&self.topup_anchor_refs)?;
        let common_input_root = self
            .inputs
            .first()
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof { field: "split.v4" })?
            .input_root;
        if self.inputs.len() > KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
            || self.asset_scale != self.transfer_amount.scale
            || self.recipient_request_digest == [0; 32]
            || self.operation_id == [0; 32]
            || self.recipient_output.network_id != self.network_id
            || self.recipient_output.asset != self.asset
            || self.recipient_output.amount != self.transfer_amount
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof { field: "split.v4" });
        }
        let mut input_total = 0_u128;
        let mut previous_digest = None;
        let mut actual_roots = Vec::new();
        let mut consumed_material = std::collections::BTreeSet::new();
        for input in &self.inputs {
            input.input_note.validate_public_binding()?;
            validate_kagemusha_recursive_spend_branch_claims_v2(&input.branch_claims)?;
            if input.bundle_digest == [0; 32]
                || previous_digest.is_some_and(|previous| previous >= input.bundle_digest)
                || input.input_note.network_id != self.network_id
                || input.input_note.asset != self.asset
                || input.input_note.amount.scale != self.asset_scale
                || input.input_root == [0; 32]
                || input.input_root != common_input_root
                || input.proof_step_count == 0
                || input.proof_step_count >= KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2
                || input.peer_hop_count >= KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
                || !consumed_material.insert(input.input_note.note_commitment)
                || !consumed_material.insert(input.input_note.spend_nullifier)
            {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "split.v4.inputs",
                });
            }
            previous_digest = Some(input.bundle_digest);
            input_total = input_total
                .checked_add(input.input_note.amount.atomic_units)
                .ok_or(KagemushaValidationError::InvalidRecursiveSpendNote {
                    field: "split.v4.input_amount",
                })?;
            actual_roots.extend(
                input
                    .branch_claims
                    .iter()
                    .map(|claim| claim.path.lineage_root),
            );
        }
        actual_roots.sort_unstable();
        actual_roots.dedup();
        if actual_roots != lineage_roots {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "split.v4.topup_anchor_refs",
            });
        }
        let output_total = self
            .change_output
            .as_ref()
            .map_or(Some(self.transfer_amount.atomic_units), |change| {
                change.validate_public_binding().ok()?;
                if change.network_id != self.network_id
                    || change.asset != self.asset
                    || change.amount.scale != self.asset_scale
                {
                    return None;
                }
                self.transfer_amount
                    .atomic_units
                    .checked_add(change.amount.atomic_units)
            })
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split.v4.change_output",
            })?;
        if output_total != input_total {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split.v4.conservation",
            });
        }
        let mut output_material = vec![
            self.recipient_output.note_commitment,
            self.recipient_output.spend_nullifier,
        ];
        if let Some(change) = &self.change_output {
            output_material.extend([change.note_commitment, change.spend_nullifier]);
        }
        if output_material
            .iter()
            .any(|value| *value == [0; 32] || consumed_material.contains(value))
            || output_material
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != output_material.len()
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split.v4.output_material",
            });
        }
        let binding = self.binding_digest_unchecked()?;
        self.output_branch_claims_for_binding(KagemushaRecursiveSpendBranchV2::Recipient, binding)?;
        if self.change_output.is_some() {
            self.output_branch_claims_for_binding(
                KagemushaRecursiveSpendBranchV2::Change,
                binding,
            )?;
        }
        Ok(())
    }
    /// Return the exact validated input total.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when split validation fails or the exact input sum cannot be represented.
    pub fn input_amount(&self) -> Result<KagemushaScaledAmountV2, KagemushaValidationError> {
        self.validate_public_binding()?;
        let atomic_units = self.inputs.iter().try_fold(0_u128, |sum, input| {
            sum.checked_add(input.input_note.amount.atomic_units)
        });
        KagemushaScaledAmountV2::new(
            atomic_units.ok_or(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split.v4.input_amount",
            })?,
            self.asset_scale,
        )
    }
    /// Return the V4-domain transition binding consumed by ABI-21 Step.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn binding_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        self.binding_digest_unchecked()
    }
    /// Derive the deterministic conflict claims for one ABI-21 child.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the branch path or claim is invalid or the requested derivation exceeds its canonical bounds.
    pub fn output_branch_claims(
        &self,
        branch: KagemushaRecursiveSpendBranchV2,
    ) -> Result<Vec<KagemushaRecursiveSpendBranchClaimV2>, KagemushaValidationError> {
        self.validate_public_binding()?;
        if matches!(branch, KagemushaRecursiveSpendBranchV2::Change) && self.change_output.is_none()
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split.v4.change_output",
            });
        }
        let binding = self.binding_digest_unchecked()?;
        self.output_branch_claims_for_binding(branch, binding)
    }
}
impl KagemushaRecursiveSpendRedemptionIntentV4 {
    /// Validate exact full/partial conservation and canonical unshield words.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.input_note.validate_public_binding()?;
        validate_kagemusha_recursive_spend_branch_claims_v2(&self.parent_branch_claims)?;
        self.public_amount.validate()?;
        if self.input_note.network_id != self.network_id {
            return Err(KagemushaValidationError::RecursiveSpendNetworkMismatch);
        }
        if self.input_note.asset != self.asset {
            return Err(KagemushaValidationError::RecursiveSpendAssetMismatch);
        }
        let expected_lineage_roots = validate_kagemusha_recursive_spend_topup_anchor_refs_v2(
            &self.parent_topup_anchor_refs,
        )?;
        let mut actual_lineage_roots = self
            .parent_branch_claims
            .iter()
            .map(|claim| claim.path.lineage_root)
            .collect::<Vec<_>>();
        actual_lineage_roots.sort_unstable();
        actual_lineage_roots.dedup();
        if self.parent_bundle_digest == [0; 32]
            || self.input_root == [0; 32]
            || self.operation_id == [0; 32]
            || actual_lineage_roots != expected_lineage_roots
            || self.parent_proof_step_count == 0
            || self.parent_proof_step_count > KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2
            || (self.change_output.is_some()
                && self.parent_proof_step_count >= KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2)
            || (self.change_output.is_some()
                && self
                    .parent_branch_claims
                    .iter()
                    .any(|claim| claim.path.depth >= KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2))
            || self.parent_peer_hop_count > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
            || self.public_amount.scale != self.input_note.amount.scale
            || self.unshield_public_inputs_digest == [0; 32]
            || self.unshield_public_inputs_digest != self.unshield_public_inputs.digest()?
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redemption.v4",
            });
        }
        let zero = [0_u8; 32];
        if self.unshield_public_inputs.input_commitment_0 != self.input_note.note_commitment
            || self.unshield_public_inputs.input_commitment_1 != zero
            || self.unshield_public_inputs.nullifier_0 != self.input_note.spend_nullifier
            || self.unshield_public_inputs.nullifier_1 != zero
            || self.unshield_public_inputs.root != self.input_root
            || self.unshield_public_inputs.public_amount
                != kagemusha_confidential_amount_encoding_v2(self.public_amount.atomic_units)
            || self.unshield_public_inputs.asset_tag == zero
            || self.unshield_public_inputs.network_tag == zero
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redemption.v4.unshield_public_inputs",
            });
        }
        let input_amount = self.input_note.amount.atomic_units;
        match (&self.change_output, &self.change_artifact_binding) {
            (None, None)
                if self.public_amount.atomic_units == input_amount
                    && self.unshield_public_inputs.change_output_commitment == zero => {}
            (Some(change), Some(binding)) if self.public_amount.atomic_units < input_amount => {
                change.validate_public_binding()?;
                binding.validate()?;
                if change.network_id != self.network_id {
                    return Err(KagemushaValidationError::RecursiveSpendNetworkMismatch);
                }
                if change.asset != self.asset {
                    return Err(KagemushaValidationError::RecursiveSpendAssetMismatch);
                }
                if change.amount.scale != self.public_amount.scale
                    || self
                        .public_amount
                        .atomic_units
                        .checked_add(change.amount.atomic_units)
                        != Some(input_amount)
                    || self.unshield_public_inputs.change_output_commitment
                        != change.note_commitment
                    || [change.note_commitment, change.spend_nullifier]
                        .iter()
                        .any(|value| {
                            *value == self.input_note.note_commitment
                                || *value == self.input_note.spend_nullifier
                        })
                {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                        field: "redemption.v4.change_output",
                    });
                }
            }
            _ => {
                return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                    field: "redemption.v4.change_output",
                });
            }
        }
        Ok(())
    }
    /// Return the V4-domain circuit binding for this redemption.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn binding_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRedemptionTransitionDigestPreimageV4 {
            domain: KAGEMUSHA_REDEMPTION_TRANSITION_DIGEST_DOMAIN_V4.to_owned(),
            intent: self.clone(),
        })
    }
    /// Derive the exact continuing change claims for a partial redemption.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the branch path or claim is invalid or the requested derivation exceeds its canonical bounds.
    pub fn change_branch_claims(
        &self,
    ) -> Result<Vec<KagemushaRecursiveSpendBranchClaimV2>, KagemushaValidationError> {
        self.validate_public_binding()?;
        if self.change_output.is_none() || self.change_artifact_binding.is_none() {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "redemption.v4.change_output",
            });
        }
        let binding = self.binding_digest()?;
        let mut claims = self
            .parent_branch_claims
            .iter()
            .map(|claim| claim.child(KagemushaRecursiveSpendBranchV2::Change, binding))
            .collect::<Result<Vec<_>, _>>()?;
        claims.sort_unstable_by_key(|claim| claim.path);
        validate_kagemusha_recursive_spend_branch_claims_v2(&claims)?;
        Ok(claims)
    }
}
impl KagemushaRecursiveSpendPublicStatementV4 {
    /// Validate the canonical ABI-21 recursive-state statement.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.current_note.validate_public_binding()?;
        self.artifact_binding.validate()?;
        let expected_verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            self.artifact_binding.manifest_sha256,
        );
        validate_kagemusha_root("final_root.v4", self.final_root)?;
        let lineage_roots =
            validate_kagemusha_recursive_spend_topup_anchor_refs_v2(&self.topup_anchor_refs)?;
        validate_kagemusha_recursive_spend_branch_claims_v2(&self.branch_claims)?;
        let mut claim_roots = self
            .branch_claims
            .iter()
            .map(|claim| claim.path.lineage_root)
            .collect::<Vec<_>>();
        claim_roots.sort_unstable();
        claim_roots.dedup();
        let canonical_init_claim = if self.topup_anchor_refs.len() == 1 {
            Some(KagemushaRecursiveSpendBranchClaimV2::root(
                self.topup_anchor_refs[0].anchor_digest,
            )?)
        } else {
            None
        };
        let has_canonical_init_claim =
            canonical_init_claim.is_some_and(|claim| self.branch_claims.as_slice() == [claim]);
        if self.current_note.network_id != self.network_id
            || self.current_note.asset != self.asset
            || self.current_note.amount.scale != self.asset_scale
            || self.next_zero_leaf_index >= KAGEMUSHA_TOPUP_SHIELD_TREE_CAPACITY_V2
            || self.proof_step_count == 0
            || self.proof_step_count > KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2
            || self.peer_hop_count > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
            || claim_roots != lineage_roots
            || self.verifier_key_id != expected_verifier_key_id
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "public_statement.v4",
            });
        }
        match &self.transition {
            None if self.proof_step_count == 1
                && self.peer_hop_count == 0
                && has_canonical_init_claim => {}
            Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(transition))
                if transition.binding_digest != [0; 32]
                    && transition.recipient_request_digest != [0; 32]
                    && transition.operation_id != [0; 32]
                    && transition.parent_max_proof_step_count > 0
                    && transition.parent_max_proof_step_count.checked_add(1)
                        == Some(self.proof_step_count)
                    && transition.parent_max_peer_hop_count.checked_add(1)
                        == Some(self.peer_hop_count) => {}
            Some(KagemushaRecursiveSpendTransitionV4::RedemptionChange(transition))
                if transition.binding_digest != [0; 32]
                    && transition.parent_bundle_digest != [0; 32]
                    && transition.operation_id != [0; 32]
                    && transition.parent_proof_step_count > 0
                    && transition.parent_proof_step_count.checked_add(1)
                        == Some(self.proof_step_count)
                    && self.peer_hop_count == transition.parent_peer_hop_count => {}
            _ => {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "public_statement.v4.transition",
                });
            }
        }
        Ok(())
    }
    /// Return the V4-domain digest exposed by the ABI-21 Step instance.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRecursiveSpendPublicStatementDigestPreimageV4 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_PUBLIC_STATEMENT_DIGEST_DOMAIN_V4.to_owned(),
            statement: self.clone(),
        })
    }
}
#[cfg(test)]
mod kagemusha_v4_lifecycle_domain_tests {
    use super::*;
    include!("kagemusha_v4_lifecycle_digest_domain_test.rs");
    #[test]
    fn abi21_operation_vector_rejects_noncanonical_pallas_elements() {
        let mut limbs = [0_u32; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4];
        limbs[0] = 1;
        KagemushaRecursiveSpendOperationVectorV4 { limbs }
            .validate()
            .expect("one followed by canonical zero fields is valid");
        let mut equal = limbs;
        equal[..8].copy_from_slice(&KAGEMUSHA_RECURSIVE_SPEND_OPERATION_FP_MODULUS_U32_LE_V4);
        assert!(
            KagemushaRecursiveSpendOperationVectorV4 { limbs: equal }
                .validate()
                .is_err()
        );
        let mut greater = equal;
        greater[0] = greater[0].saturating_add(1);
        assert!(
            KagemushaRecursiveSpendOperationVectorV4 { limbs: greater }
                .validate()
                .is_err()
        );
        let mut high_limb = limbs;
        high_limb[7] = 0x4000_0001;
        assert!(
            KagemushaRecursiveSpendOperationVectorV4 { limbs: high_limb }
                .validate()
                .is_err()
        );
        let mut below_by_high_limb = limbs;
        below_by_high_limb[..7].fill(u32::MAX);
        below_by_high_limb[7] = 0x3fff_ffff;
        KagemushaRecursiveSpendOperationVectorV4 {
            limbs: below_by_high_limb,
        }
        .validate()
        .expect("the first differing high limb is below the modulus");
    }
}
/// Domain separator for ABI-21 recursive bundle identity digests.
pub const KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_DIGEST_DOMAIN_V4: &str =
    "iroha:kagemusha:v4:recursive-spend-bundle";
/// Domain separator for ABI-21 unsigned redemption authorization payloads.
pub const KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V4: &str = "iroha:kagemusha:v4:redeem-payload";
/// Domain separator binding an accepted receiver request to its exact output bundle.
pub const KAGEMUSHA_REQUEST_OUTPUT_BINDING_DIGEST_DOMAIN_V4: &str =
    "iroha:kagemusha:v4:request-output-binding";
/// Maximum canonical ABI-21 receiver-verification request size.
pub const KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_MAX_BYTES_V4: usize = 64 * 1024 * 1024;
/// Maximum canonical ABI-21 redemption request archive size.
pub const KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4: usize = 48 * 1024 * 1024;
/// Frame-derived base multiplier for bounded ABI-21 request reconstruction.
pub const KAGEMUSHA_CANONICAL_DECODE_BASE_ALLOCATION_MULTIPLIER_V4: usize = 4;
/// Extra frame-derived multiplier for an untrusted redemption proof field.
///
/// Canonical wire preflight rejects an oversized unshield proof before its
/// `Vec<u8>` is materialized, so no frame-scaled allowance remains necessary.
pub const KAGEMUSHA_REDEEM_CANONICAL_DECODE_EXTRA_ALLOCATION_MULTIPLIER_V4: usize = 0;
/// Extra frame-derived multiplier for a native redemption-build result.
///
/// Canonical wire preflight rejects its nested oversized unshield proof before reconstruction, so
/// the exact fixed allowance accounts for the remaining charged copies.
pub const KAGEMUSHA_REDEEM_BUILD_RESULT_CANONICAL_DECODE_EXTRA_ALLOCATION_MULTIPLIER_V4: usize = 0;
/// Fixed allocation allowance for a maximum-shaped ABI-21 top-up request.
pub const KAGEMUSHA_TOPUP_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4: usize =
    6 * KAGEMUSHA_TOPUP_SHIELD_MAX_PROOF_BYTES_V2 + 64 * 1024;
/// Fixed allocation allowance for one root ABI-21 recursive bundle.
pub const KAGEMUSHA_BUNDLE_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4: usize =
    8 * KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize + 1024 * 1024;
/// Fixed allocation allowance for one nested ABI-21 recursive bundle.
///
/// This profile covers peer payments, initialization results, and local
/// wrappers containing one bundle.
pub const KAGEMUSHA_SINGLE_RECURSIVE_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4: usize =
    11 * KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize + 1024 * 1024;
/// Fixed allocation allowance for a maximum-shaped ABI-21 split result.
pub const KAGEMUSHA_SPLIT_RESULT_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4: usize =
    24 * KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize + 1024 * 1024;
/// Fixed allocation allowance for a two-parent ABI-21 change-preparation request.
pub const KAGEMUSHA_PEER_SPLIT_PREPARE_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4: usize =
    28 * KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize + 1024 * 1024;
/// Fixed allocation allowance for a two-parent ABI-21 local append request.
pub const KAGEMUSHA_APPEND_LOCAL_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4: usize =
    34 * KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize + 1024 * 1024;
/// Fixed allocation allowance for a nested ABI-21 terminal-verification request.
pub const KAGEMUSHA_VERIFY_LOCAL_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4: usize =
    14 * KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize + 1024 * 1024;
/// Fixed allocation allowance for a maximum-shaped ABI-21 redemption request.
pub const KAGEMUSHA_REDEEM_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4: usize = 27
    * KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize
    + 3 * KAGEMUSHA_UNSHIELD_MAX_PROOF_BYTES_V4
    + 1024 * 1024;
/// Fixed allocation allowance for a maximum-shaped native redemption result.
pub const KAGEMUSHA_REDEEM_BUILD_RESULT_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4: usize = 46
    * KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize
    + 6 * KAGEMUSHA_UNSHIELD_MAX_PROOF_BYTES_V4
    + 1024 * 1024;
const KAGEMUSHA_RECURSIVE_SPEND_REDEEM_DECODE_MAX_NESTING_DEPTH_V4: usize = 64;
fn canonical_kagemusha_archive_payload_v4<T: norito::NoritoSerialize>(
    frame: &[u8],
) -> Result<&[u8], norito::Error> {
    let header = norito::core::Header::read(std::io::Cursor::new(frame))?;
    if header.compression != norito::Compression::None
        || header.flags != norito::core::default_encode_flags()
    {
        return Err(norito::Error::NonCanonicalEncoding);
    }
    if header.schema != <T as norito::NoritoSerialize>::schema_hash() {
        return Err(norito::Error::SchemaMismatch);
    }
    let archive_limit = norito::core::max_archive_len().min(usize::MAX as u64);
    if header.length > archive_limit {
        return Err(norito::Error::ArchiveLengthExceeded {
            length: header.length,
            limit: archive_limit,
        });
    }
    let payload_len =
        usize::try_from(header.length).map_err(|_| norito::Error::ArchiveLengthExceeded {
            length: header.length,
            limit: archive_limit,
        })?;
    let align = norito::core::archived_payload_align::<T>();
    let remainder = norito::core::Header::SIZE % align;
    let padding = if remainder == 0 { 0 } else { align - remainder };
    let payload_start = norito::core::Header::SIZE
        .checked_add(padding)
        .ok_or(norito::Error::LengthMismatch)?;
    let frame_end = payload_start
        .checked_add(payload_len)
        .ok_or(norito::Error::LengthMismatch)?;
    if frame_end != frame.len() {
        return Err(norito::Error::LengthMismatch);
    }
    if frame[norito::core::Header::SIZE..payload_start]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(norito::Error::LengthMismatch);
    }
    let payload = &frame[payload_start..frame_end];
    if norito::hardware_crc64(payload) != header.checksum {
        return Err(norito::Error::ChecksumMismatch);
    }
    Ok(payload)
}
fn canonical_kagemusha_compact_field_v4(
    bytes: &[u8],
    index: usize,
) -> Result<&[u8], norito::Error> {
    let mut offset = 0usize;
    for field_index in 0..=index {
        let tail = bytes.get(offset..).ok_or(norito::Error::LengthMismatch)?;
        let (field_len, header_len) = norito::core::read_len_from_slice_with_flags(
            tail,
            norito::core::default_encode_flags(),
        )?;
        let start = offset
            .checked_add(header_len)
            .ok_or(norito::Error::LengthMismatch)?;
        let end = start
            .checked_add(field_len)
            .ok_or(norito::Error::LengthMismatch)?;
        let field = bytes.get(start..end).ok_or(norito::Error::LengthMismatch)?;
        if field_index == index {
            return Ok(field);
        }
        offset = end;
    }
    Err(norito::Error::LengthMismatch)
}
fn preflight_kagemusha_unshield_proof_archive_v4<T: norito::NoritoSerialize>(
    frame: &[u8],
    field_path: &[usize],
) -> Result<(), norito::Error> {
    let mut field = canonical_kagemusha_archive_payload_v4::<T>(frame)?;
    for &index in field_path {
        field = canonical_kagemusha_compact_field_v4(field, index)?;
    }
    let count_bytes = field.get(..8).ok_or(norito::Error::LengthMismatch)?;
    let proof_len = u64::from_le_bytes(
        count_bytes
            .try_into()
            .map_err(|_| norito::Error::LengthMismatch)?,
    );
    let maximum = KAGEMUSHA_UNSHIELD_MAX_PROOF_BYTES_V4 as u64;
    if proof_len > maximum {
        return Err(norito::Error::FieldLengthExceeded {
            length: proof_len,
            limit: maximum,
        });
    }
    let proof_len = usize::try_from(proof_len).map_err(|_| norito::Error::LengthMismatch)?;
    if field.len() != 8usize.saturating_add(proof_len) {
        return Err(norito::Error::LengthMismatch);
    }
    Ok(())
}
/// Reject an oversized unshield proof in a canonical redemption request before
/// materializing its `ProofAttachment` byte vector.
///
/// # Errors
///
/// Returns a Norito framing, layout, schema, checksum, length, or field-limit error when the
/// archive cannot be safely classified or the proof exceeds the ABI-21 limit.
pub fn preflight_kagemusha_redeem_request_archive_v4(frame: &[u8]) -> Result<(), norito::Error> {
    preflight_kagemusha_unshield_proof_archive_v4::<KagemushaRecursiveSpendRedeemRequestV4>(
        frame,
        &[4, 1, 1],
    )
}
/// Reject an oversized unshield proof in canonical unsigned redemption fields
/// before materializing its `ProofAttachment` byte vector.
///
/// # Errors
///
/// Returns a Norito framing, layout, schema, checksum, length, or field-limit error when the
/// archive cannot be safely classified or the proof exceeds the ABI-21 limit.
pub fn preflight_kagemusha_redeem_unsigned_archive_v4(frame: &[u8]) -> Result<(), norito::Error> {
    preflight_kagemusha_unshield_proof_archive_v4::<KagemushaRecursiveSpendRedeemUnsignedV4>(
        frame,
        &[4, 1, 1],
    )
}
/// Reject an oversized nested unshield proof in a canonical redemption-build
/// result before materializing its `ProofAttachment` byte vector.
///
/// # Errors
///
/// Returns a Norito framing, layout, schema, checksum, length, or field-limit error when the
/// archive cannot be safely classified or the proof exceeds the ABI-21 limit.
pub fn preflight_kagemusha_redeem_build_result_archive_v4(
    frame: &[u8],
) -> Result<(), norito::Error> {
    preflight_kagemusha_unshield_proof_archive_v4::<KagemushaRecursiveSpendRedeemBuildResultV4>(
        frame,
        &[0, 4, 1, 1],
    )
}
fn kagemusha_recursive_spend_redeem_decode_limits_v4(encoded_len: usize) -> norito::DecodeLimits {
    // The canonical wire preflight enforces the unshield cap before Vec
    // reconstruction. The fourfold base covers decoded structures and ordinary
    // collection storage; the fixed allowance covers the bounded unshield,
    // main, and optional-change proofs plus one MiB of structural headroom.
    norito::DecodeLimits::new(
        encoded_len,
        encoded_len,
        encoded_len.saturating_mul(2),
        encoded_len
            .saturating_mul(
                KAGEMUSHA_CANONICAL_DECODE_BASE_ALLOCATION_MULTIPLIER_V4
                    + KAGEMUSHA_REDEEM_CANONICAL_DECODE_EXTRA_ALLOCATION_MULTIPLIER_V4,
            )
            .saturating_add(KAGEMUSHA_REDEEM_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4),
        KAGEMUSHA_RECURSIVE_SPEND_REDEEM_DECODE_MAX_NESTING_DEPTH_V4,
    )
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRecursiveSpendBundleDigestPreimageV4 {
    domain: String,
    bundle: KagemushaRecursiveSpendBundleV4,
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRedeemUnsignedPayloadDigestPreimageV4 {
    domain: String,
    version: u16,
    bundle: KagemushaRecursiveSpendBundleV4,
    recipient: AccountId,
    amount: KagemushaScaledAmountV2,
    redeem_proof: ProofAttachment,
    redemption: KagemushaRecursiveSpendRedemptionIntentV4,
    offline_change: Option<KagemushaRecursiveSpendRedeemChangeBranchV4>,
    block_height: u64,
    operation_id: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRequestOutputBindingDigestPreimageV4 {
    domain: String,
    recipient_request_digest: [u8; 32],
    recipient_output: KagemushaSpendableNoteDescriptorV2,
    bundle_digest: [u8; 32],
}
impl KagemushaRecursiveSpendOperationVectorV4 {
    /// Require all 135 encoded Pallas elements to be canonical and the row to be non-empty.
    /// Comparison is exact on little-endian limbs and performs no modular reduction.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.limbs.iter().all(|limb| *limb == 0) {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "operation_vector.v4",
            });
        }
        for field in self.limbs.chunks_exact(8) {
            let is_less = (0..8).rev().find_map(|index| {
                let actual = field[index];
                let modulus = KAGEMUSHA_RECURSIVE_SPEND_OPERATION_FP_MODULUS_U32_LE_V4[index];
                (actual != modulus).then_some(actual < modulus)
            });
            if is_less != Some(true) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "operation_vector.v4.canonical_field",
                });
            }
        }
        Ok(())
    }
}
impl KagemushaRecursiveSpendBundleV4 {
    /// Validate the exact V4 statement/proof/release identity while keeping proof bytes opaque.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.statement.validate_public_binding()?;
        self.operation.validate()?;
        self.recursive_proof.proof_envelope.validate()?;
        let envelope = &self.recursive_proof.proof_envelope;
        if self.recursive_proof.verifier_key_id != self.statement.verifier_key_id
            || self.recursive_proof.public_statement_digest == [0; 32]
            || self.recursive_proof.public_statement_digest != self.statement.digest()?
            || envelope.artifact_generation != self.statement.artifact_binding.generation
            || envelope.manifest_sha256 != self.statement.artifact_binding.manifest_sha256
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "recursive_proof.v4",
            });
        }
        let encoded_len = norito::encode_canonical(self)?.len();
        if encoded_len > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4 {
            return Err(KagemushaValidationError::EncodedSizeExceeded {
                actual: encoded_len,
                max: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4,
            });
        }
        Ok(())
    }
    /// Return the V4-domain identity of the complete opaque bundle.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRecursiveSpendBundleDigestPreimageV4 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_DIGEST_DOMAIN_V4.to_owned(),
            bundle: self.clone(),
        })
    }
    /// Decode only wallet-visible V4 state while preserving the opaque proof payload.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the bundle is invalid or its wallet-visible state cannot be decoded.
    pub fn summary(
        &self,
    ) -> Result<KagemushaRecursiveSpendBundleSummaryV4, KagemushaValidationError> {
        self.validate_public_binding()?;
        Ok(KagemushaRecursiveSpendBundleSummaryV4 {
            asset: self.statement.asset.clone(),
            amount: self.statement.current_note.amount,
            note_commitment: self.statement.current_note.note_commitment,
            spend_nullifier: self.statement.current_note.spend_nullifier,
            hop_count: self.statement.peer_hop_count,
            proof_step_count: self.statement.proof_step_count,
            branch_claims: self.statement.branch_claims.clone(),
            artifact_binding: self.statement.artifact_binding.clone(),
            verifier_key_id: self.statement.verifier_key_id.clone(),
            bundle_digest: self.digest()?,
        })
    }
}
impl KagemushaRecursiveSpendInitResultV4 {
    /// Validate that initialization created exactly the finalized top-up state.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_request(
        &self,
        request: &KagemushaRecursiveSpendInitRequestV4,
    ) -> Result<(), KagemushaValidationError> {
        request.validate_public_binding()?;
        self.bundle.validate_public_binding()?;
        let anchor = &request.topup_anchor;
        self.membership_witness
            .validate_for_statement_v4(&self.bundle.statement)?;
        self.topup_provenance.validate_for_bundle(&self.bundle)?;
        let expected_provenance = KagemushaRecursiveSpendTopUpProvenanceV4 {
            topup_finality_roster_artifact: request.topup_finality_roster_artifact.clone(),
            topup_finality_evidence: vec![KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
                topup_anchor: anchor.clone(),
                topup_finality_proof: request.topup_finality_proof.clone(),
            }],
        };
        let expected_claim = KagemushaRecursiveSpendBranchClaimV2::root(anchor.anchor_digest)?;
        if self.public_statement_digest == [0; 32]
            || self.public_statement_digest != self.bundle.statement.digest()?
            || self.public_statement_digest != self.bundle.recursive_proof.public_statement_digest
            || self.bundle.statement.network_id != anchor.network_id
            || self.bundle.statement.asset != anchor.asset.definition().clone()
            || self.bundle.statement.asset_scale != anchor.asset_scale
            || self.bundle.statement.final_root != anchor.finalized_root
            || self.bundle.statement.next_zero_leaf_index
                != anchor.shield_leaf_index.checked_add(1).ok_or(
                    KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "init_result.v4.next_zero_leaf_index",
                    },
                )?
            || self.bundle.statement.topup_anchor_refs != vec![anchor.compact_ref()?]
            || self.bundle.statement.proof_step_count != 1
            || self.bundle.statement.peer_hop_count != 0
            || self.bundle.statement.current_note != anchor.current_note
            || self.membership_witness.leaf_index != anchor.shield_leaf_index
            || self.topup_provenance != expected_provenance
            || self.bundle.statement.branch_claims != vec![expected_claim]
            || self.bundle.statement.transition.is_some()
            || self.bundle.statement.artifact_binding != request.artifact_binding
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "init_result.v4",
            });
        }
        Ok(())
    }
}
impl KagemushaRecursiveSpendAppendRequestV4 {
    /// Validate canonical parents, the confidential proof envelope, and the exact split inputs.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.split.validate_public_binding()?;
        if self.block_height == 0
            || self.previous_inputs.is_empty()
            || self.previous_inputs.len() > KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
            || self.previous_inputs.len() != self.split.inputs.len()
            || self
                .confidential_transfer_proof
                .structural_error()
                .is_some()
            || self.confidential_transfer_proof.backend.as_str()
                != KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND
            || self.confidential_transfer_proof.proof.backend.as_str()
                != KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND
            || self.confidential_transfer_proof.vk_ref.backend.as_str()
                != KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND
            || self.confidential_transfer_proof.vk_ref.name.is_empty()
            || self.confidential_transfer_proof.proof.bytes.is_empty()
            || self
                .confidential_transfer_proof
                .vk_commitment
                .is_none_or(|commitment| commitment == [0; 32])
            || self.confidential_transfer_proof.lane_privacy.is_some()
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "append_request.v4",
            });
        }
        let expected_next_zero_leaf_index = self
            .previous_inputs
            .first()
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "append_request.v4.previous_inputs",
            })?
            .previous_bundle
            .statement
            .next_zero_leaf_index;
        for (typed, expected) in self.previous_inputs.iter().zip(&self.split.inputs) {
            typed.previous_bundle.validate_public_binding()?;
            let statement = &typed.previous_bundle.statement;
            if typed.previous_bundle.digest()? != expected.bundle_digest
                || statement.current_note != expected.input_note
                || statement.branch_claims != expected.branch_claims
                || statement.final_root != expected.input_root
                || statement.proof_step_count != expected.proof_step_count
                || statement.peer_hop_count != expected.peer_hop_count
                || statement.network_id != self.split.network_id
                || statement.asset != self.split.asset
                || statement.asset_scale != self.split.asset_scale
                || statement.next_zero_leaf_index != expected_next_zero_leaf_index
                || statement.artifact_binding != self.split.output_artifact_binding
            {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "append_request.v4.previous_inputs",
                });
            }
        }
        let merged = KagemushaRecursiveSpendTopUpProvenanceV4::merge_for_append_inputs(
            &self.previous_inputs,
            self.block_height,
        )?;
        if merged.anchor_refs()? != self.split.topup_anchor_refs {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "append_request.v4.topup_provenance",
            });
        }
        Ok(())
    }
}
impl KagemushaRecursiveSpendSplitIntentV4 {
    /// Validate one independently spendable V4 child against this exact split.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_output_bundle(
        &self,
        bundle: &KagemushaRecursiveSpendBundleV4,
        branch: KagemushaRecursiveSpendBranchV2,
    ) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        bundle.validate_public_binding()?;
        let expected_note = match branch {
            KagemushaRecursiveSpendBranchV2::Recipient => &self.recipient_output,
            KagemushaRecursiveSpendBranchV2::Change => self.change_output.as_ref().ok_or(
                KagemushaValidationError::InvalidRecursiveSpendNote {
                    field: "split.v4.change_output",
                },
            )?,
        };
        let parent_max_proof_step_count = self
            .inputs
            .iter()
            .map(|input| input.proof_step_count)
            .max()
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "split.v4.inputs",
            })?;
        let parent_max_peer_hop_count = self
            .inputs
            .iter()
            .map(|input| input.peer_hop_count)
            .max()
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "split.v4.inputs",
            })?;
        let expected_transition = KagemushaRecursiveSpendTransitionV4::PeerSplit(
            KagemushaRecursiveSpendPeerSplitTransitionV4 {
                binding_digest: self.binding_digest()?,
                branch,
                recipient_request_digest: self.recipient_request_digest,
                operation_id: self.operation_id,
                parent_max_proof_step_count,
                parent_max_peer_hop_count,
            },
        );
        let statement = &bundle.statement;
        if statement.network_id != self.network_id
            || statement.asset != self.asset
            || statement.asset_scale != self.asset_scale
            || statement.current_note != *expected_note
            || statement.branch_claims != self.output_branch_claims(branch)?
            || statement.topup_anchor_refs != self.topup_anchor_refs
            || statement.artifact_binding != self.output_artifact_binding
            || statement.transition.as_ref() != Some(&expected_transition)
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "split.v4.output_bundle",
            });
        }
        Ok(())
    }
}
impl KagemushaRecursiveSpendSplitResultV4 {
    /// Validate conservation and both independently spendable V4 branches.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.split.validate_public_binding()?;
        if self.split_binding_digest != self.split.binding_digest()? {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "split_binding_digest.v4",
            });
        }
        self.split.validate_output_bundle(
            &self.recipient_bundle,
            KagemushaRecursiveSpendBranchV2::Recipient,
        )?;
        self.recipient_membership_witness
            .validate_for_statement_v4(&self.recipient_bundle.statement)?;
        self.recipient_topup_provenance
            .validate_for_bundle(&self.recipient_bundle)?;
        match (
            &self.split.change_output,
            &self.change_bundle,
            &self.change_membership_witness,
            &self.change_topup_provenance,
        ) {
            (None, None, None, None) => Ok(()),
            (Some(_), Some(change_bundle), Some(change_witness), Some(change_provenance)) => {
                self.split.validate_output_bundle(
                    change_bundle,
                    KagemushaRecursiveSpendBranchV2::Change,
                )?;
                change_witness.validate_for_statement_v4(&change_bundle.statement)?;
                change_provenance.validate_for_bundle(change_bundle)?;
                let recipient = &self.recipient_bundle.statement;
                let change = &change_bundle.statement;
                if recipient.final_root != change.final_root
                    || recipient.next_zero_leaf_index != change.next_zero_leaf_index
                    || recipient.topup_anchor_refs != change.topup_anchor_refs
                    || recipient.proof_step_count != change.proof_step_count
                    || recipient.peer_hop_count != change.peer_hop_count
                    || recipient.artifact_binding != change.artifact_binding
                    || recipient.verifier_key_id != change.verifier_key_id
                    || self.recipient_membership_witness.leaf_index == change_witness.leaf_index
                    || self.recipient_membership_witness.dummy_input_path
                        != change_witness.dummy_input_path
                    || self.recipient_topup_provenance != *change_provenance
                    || self.recipient_bundle == *change_bundle
                    || recipient.branch_claims.iter().any(|recipient_claim| {
                        change.branch_claims.iter().any(|change_claim| {
                            recipient_claim.path.conflicts_with(change_claim.path)
                        })
                    })
                {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "split_result.v4.branch_lineage",
                    });
                }
                Ok(())
            }
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split_result.v4.change_bundle",
            }),
        }
    }
}
impl KagemushaRecursiveSpendPeerPaymentV4 {
    /// Project the recipient-only ABI-21 transport from a validated split result.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn from_split_result(
        result: &KagemushaRecursiveSpendSplitResultV4,
    ) -> Result<Self, KagemushaValidationError> {
        result.validate_public_binding()?;
        let payment = Self {
            recipient_bundle: result.recipient_bundle.clone(),
            recipient_membership_witness: result.recipient_membership_witness.clone(),
            topup_provenance: result.recipient_topup_provenance.clone(),
        };
        payment.validate_public_binding()?;
        Ok(payment)
    }
    /// Return the recipient peer-split transition embedded by the ABI-21 statement.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the source value is invalid or the requested canonical result cannot be derived.
    pub fn recipient_split_transition(
        &self,
    ) -> Result<&KagemushaRecursiveSpendPeerSplitTransitionV4, KagemushaValidationError> {
        self.recipient_bundle.validate_public_binding()?;
        let Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(transition)) =
            self.recipient_bundle.statement.transition.as_ref()
        else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "peer_payment.v4.transition",
            });
        };
        if transition.branch != KagemushaRecursiveSpendBranchV2::Recipient {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "peer_payment.v4.binding",
            });
        }
        Ok(transition)
    }
    /// Return the canonical split operation identifier.
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the source value is invalid or the requested canonical result cannot be derived.
    pub fn operation_id(&self) -> Result<[u8; 32], KagemushaValidationError> {
        Ok(self.recipient_split_transition()?.operation_id)
    }
    /// Return the receiver-request digest bound by the recipient transition.
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the source value is invalid or the requested canonical result cannot be derived.
    pub fn recipient_request_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        Ok(self.recipient_split_transition()?.recipient_request_digest)
    }
    /// Validate the recipient branch, membership state, and ABI-21 peer-size ceiling.
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        let transition = self.recipient_split_transition()?;
        self.recipient_membership_witness
            .validate_for_statement_v4(&self.recipient_bundle.statement)?;
        self.topup_provenance
            .validate_for_bundle(&self.recipient_bundle)?;
        if transition.operation_id == [0; 32] || transition.recipient_request_digest == [0; 32] {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "peer_payment.v4.binding",
            });
        }
        let encoded_len = norito::encode_canonical(self)?.len();
        if encoded_len > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4 {
            return Err(KagemushaValidationError::EncodedSizeExceeded {
                max: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4,
                actual: encoded_len,
            });
        }
        Ok(())
    }
}
impl KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
    /// Validate one bounded finalized V4 origin and its exact compact reference.
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.topup_anchor.validate_public_binding()?;
        self.topup_finality_proof.validate_structure()?;
        let anchor_ref = self.topup_anchor.compact_ref()?;
        let anchor_len = norito::encode_canonical(&self.topup_anchor)?.len();
        let proof_len = norito::encode_canonical(&self.topup_finality_proof)?.len();
        if anchor_len == 0
            || anchor_len > KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_USIZE_V2
            || proof_len == 0
            || proof_len > KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_USIZE_V2
            || self.topup_finality_proof.anchor != anchor_ref
            || self.topup_finality_proof.commit_qc.height_context.height
                != self.topup_anchor.finalized_height
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_provenance.v4.evidence",
            });
        }
        Ok(())
    }
    /// Validate the exact canonical origin inventory carried by a V4 state.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_ordered_set(
        evidence: &[Self],
        expected_refs: &[KagemushaRecursiveSpendTopUpAnchorRefV2],
    ) -> Result<(), KagemushaValidationError> {
        if evidence.is_empty()
            || evidence.len() != expected_refs.len()
            || evidence.len() > KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_provenance.v4.evidence_count",
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for (evidence, expected_ref) in evidence.iter().zip(expected_refs) {
            evidence.validate_public_binding()?;
            let actual_ref = evidence.topup_anchor.compact_ref()?;
            if actual_ref != *expected_ref || !seen.insert(actual_ref) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "topup_provenance.v4.evidence_order",
                });
            }
        }
        Ok(())
    }
}
impl KagemushaRecursiveSpendTopUpProvenanceV4 {
    fn anchor_refs(
        &self,
    ) -> Result<Vec<KagemushaRecursiveSpendTopUpAnchorRefV2>, KagemushaValidationError> {
        self.topup_finality_evidence
            .iter()
            .map(|evidence| evidence.topup_anchor.compact_ref())
            .collect()
    }
    fn validate_for_statement_at_height(
        &self,
        statement: &KagemushaRecursiveSpendPublicStatementV4,
        block_height: Option<u64>,
    ) -> Result<(), KagemushaValidationError> {
        self.topup_finality_roster_artifact.validate_structure()?;
        let roster_len = norito::encode_canonical(&self.topup_finality_roster_artifact)?.len();
        let provenance_len = norito::encode_canonical(self)?.len();
        if block_height == Some(0)
            || roster_len == 0
            || roster_len > KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_USIZE_V2
            || provenance_len == 0
            || provenance_len > KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4
            || self.topup_finality_roster_artifact.network_id != statement.network_id
            || self.topup_finality_roster_artifact.artifact_generation
                != statement.artifact_binding.generation
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_provenance.v4",
            });
        }
        KagemushaRecursiveSpendTopUpFinalityEvidenceV4::validate_ordered_set(
            &self.topup_finality_evidence,
            &statement.topup_anchor_refs,
        )?;
        let mut anchored_total = 0_u128;
        for evidence in &self.topup_finality_evidence {
            let anchor = &evidence.topup_anchor;
            let commit_qc = &evidence.topup_finality_proof.commit_qc;
            let height_context = &commit_qc.height_context;
            let finality_height_matches_anchor = height_context.height == anchor.finalized_height;
            let window = self
                .topup_finality_roster_artifact
                .window_at(anchor.finalized_height)?;
            commit_qc.validate_for_roster_window(window)?;
            if anchor.network_id != statement.network_id
                || anchor.asset.definition() != &statement.asset
                || anchor.asset_scale != statement.asset_scale
                || anchor.artifact_binding != statement.artifact_binding
                || block_height.is_some_and(|height| anchor.finalized_height > height)
                || height_context.network_id != self.topup_finality_roster_artifact.network_id
                || !finality_height_matches_anchor
                || height_context.mode != window.consensus_mode
            {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "topup_provenance.v4.evidence_binding",
                });
            }
            anchored_total = anchored_total
                .checked_add(anchor.amount.atomic_units)
                .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "topup_provenance.v4.amount",
                })?;
        }
        if anchored_total < statement.current_note.amount.atomic_units {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_provenance.v4.amount",
            });
        }
        Ok(())
    }
    /// Validate exact ordered provenance against one spendable branch.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_bundle(
        &self,
        bundle: &KagemushaRecursiveSpendBundleV4,
    ) -> Result<(), KagemushaValidationError> {
        bundle.validate_public_binding()?;
        self.validate_for_statement_at_height(&bundle.statement, None)
    }
    /// Validate provenance against a branch at the append evaluation height.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_bundle_at(
        &self,
        bundle: &KagemushaRecursiveSpendBundleV4,
        block_height: u64,
    ) -> Result<(), KagemushaValidationError> {
        bundle.validate_public_binding()?;
        self.validate_for_statement_at_height(&bundle.statement, Some(block_height))
    }
    fn merge_for_statements_at(
        inputs: &[(&KagemushaRecursiveSpendPublicStatementV4, &Self)],
        block_height: u64,
    ) -> Result<Self, KagemushaValidationError> {
        if block_height == 0
            || inputs.is_empty()
            || inputs.len() > KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "append_request.v4.topup_provenance_count",
            });
        }
        let roster = inputs[0].1.topup_finality_roster_artifact.clone();
        let mut merged = std::collections::BTreeMap::new();
        for (statement, provenance) in inputs {
            provenance.validate_for_statement_at_height(statement, Some(block_height))?;
            if provenance.topup_finality_roster_artifact != roster {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "append_request.v4.topup_provenance_roster",
                });
            }
            for evidence in &provenance.topup_finality_evidence {
                let anchor_ref = evidence.topup_anchor.compact_ref()?;
                if let Some(previous) = merged.insert(anchor_ref, evidence.clone())
                    && previous != *evidence
                {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "append_request.v4.topup_provenance_conflict",
                    });
                }
            }
        }
        if merged.is_empty() || merged.len() > KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "append_request.v4.topup_provenance_inventory",
            });
        }
        let provenance = Self {
            topup_finality_roster_artifact: roster,
            topup_finality_evidence: merged.into_values().collect(),
        };
        if provenance.anchor_refs()?
            != inputs
                .iter()
                .flat_map(|(statement, _)| statement.topup_anchor_refs.iter().copied())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "append_request.v4.topup_provenance_inventory",
            });
        }
        Ok(provenance)
    }
    /// Canonically merge one or two parent inventories under one exact roster.
    ///
    /// An origin shared by two parents is coalesced only when its complete
    /// evidence is byte-for-byte equal. Conflicting evidence for one compact
    /// reference is rejected rather than selected by input order.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the source value is invalid or the requested canonical result cannot be derived.
    pub fn merge_for_append_inputs(
        inputs: &[KagemushaRecursiveSpendAppendInputV4],
        block_height: u64,
    ) -> Result<Self, KagemushaValidationError> {
        for input in inputs {
            input.previous_bundle.validate_public_binding()?;
        }
        let contexts = inputs
            .iter()
            .map(|input| (&input.previous_bundle.statement, &input.topup_provenance))
            .collect::<Vec<_>>();
        Self::merge_for_statements_at(&contexts, block_height)
    }
}
include!("kagemusha_v4_topup_provenance_inline_tests.rs");
#[cfg(test)]
include!("kagemusha_v4_lifecycle_additional_domain_tests.rs");
