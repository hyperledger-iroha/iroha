//! Canonical Kagemusha offline-cash models.
//!
//! The module exposes one lifecycle: exact online top-up, recursive
//! offline split/spend, and exact online redemption.

mod receiver_snapshot;

pub use receiver_snapshot::*;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_crypto::{Algorithm, Hash, KeyPair, PublicKey, SignatureOf};
use iroha_data_model_derive::model;
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    to_bytes,
};
use p256::ecdsa::{
    Signature as P256Signature, VerifyingKey as P256VerifyingKey, signature::Verifier as _,
};
use sha2::{Digest as _, Sha256};

pub use self::model::*;
use crate::{
    ChainId,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    block::consensus_v2::{
        ConsensusMode, DataAvailabilityLayout, DualQuorum, GlobalPhase, HeightContext,
        HeightContextId, MAX_VALIDATORS_PER_HEIGHT, PROTOCOL_VERSION, QuorumCertificate,
        SnapshotBootstrapAnchor, ValidatorPower, finality::FinalizedNextEpochSnapshot,
    },
    confidential::ConfidentialStatus,
    proof::{ProofAttachment, ProofBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};

/// Prefix embedded into offline instruction rejection messages.
///
/// Mobile SDKs parse the label after this prefix up to the first `:` to recover
/// stable machine-readable error codes.
pub const OFFLINE_REJECTION_REASON_PREFIX: &str = "offline_reason::";
/// Asset-definition metadata key that enables Offline escrow tracking.
pub const OFFLINE_ASSET_ENABLED_METADATA_KEY: &str = "offline.enabled";
/// Domain-separation tag for deterministic offline escrow derivation.
pub const OFFLINE_ESCROW_SEED_LABEL: &str = "iroha.offline.escrow";
/// Stable public Norito schema name for the first-release Torii top-up request.
pub const OFFLINE_TOP_UP_REQUEST_SCHEMA_NAME: &str = "iroha.torii.v1.offline.top_up.request";
/// Stable public Norito schema name for the first-release Torii redemption request.
pub const OFFLINE_REDEEM_REQUEST_SCHEMA_NAME: &str = "iroha.torii.v1.offline.redeem.request";
/// Domain-separation tag for on-chain Kagemusha device-attestation challenges.
pub const OFFLINE_DEVICE_ATTESTATION_CHALLENGE_DOMAIN: &str =
    "iroha:kagemusha:device-attestation-challenge:v1";
/// Canonical Android hardware-attestation platform label for Kagemusha.
pub const OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_PLATFORM: &str = "android-keymint";
/// Canonical Android one-use assertion scheme for Kagemusha.
pub const OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_ASSERTION_SCHEME: &str =
    "android-keymint-ecdsa-p256-usage-limit-v1";
/// Canonical Android assertion-key algorithm for Kagemusha.
pub const OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM: &str =
    "ecdsa-p256-sha256";
/// Canonical Apple App Attest platform label for Kagemusha.
pub const OFFLINE_DEVICE_ATTESTATION_IOS_APP_ATTEST_PLATFORM: &str = "ios-appattest";
/// Legacy App Attest assertion authenticator-data size (RP hash, flags, counter).
pub const KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_BYTES_V1: usize = 37;
/// Minimum App Attest assertion authenticator-data size.
pub const KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_MIN_BYTES_V1: usize =
    KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_BYTES_V1;
/// Maximum App Attest assertion authenticator-data size, including iOS 27 extensions.
pub const KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_MAX_BYTES_V1: usize = 4 * 1024;

/// Maximum asset scale accepted by the exact Kagemusha V2 amount contract.
pub const KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2: u32 = 28;
/// Fixed confidential Merkle-tree depth shared by top-up, spend, and redemption.
pub const KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2: usize = 16;
/// Fixed depth-16 confidential tree capacity used by top-up shielding.
pub const KAGEMUSHA_TOPUP_SHIELD_TREE_CAPACITY_V2: u32 = 1 << KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2;
/// Maximum canonical top-up shield proof envelope accepted at typed ingress.
pub const KAGEMUSHA_TOPUP_SHIELD_MAX_PROOF_BYTES_V2: usize = 192 * 1024;
/// Maximum number of branch decisions carried by one recursive spend lineage.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2: u8 = 64;
/// Maximum number of device-to-device transfers in one recursive spend lineage.
///
/// This is intentionally independent of branch depth: redemption-change can
/// extend a branch without adding a peer hop.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2: u32 = 8;
/// Bytes retained from each domain-separated transition digest in a branch history.
///
/// A 192-bit chosen-prefix tag gives a 96-bit birthday bound. At depth 64, two
/// claims alone occupy 3,072 bytes, so this layout must not be certified against
/// the 12 KiB peer gate until the complete proof-bearing archive is measured.
/// The complete 256-bit transition digest remains proof-bound in the producing
/// statement.
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
/// This is deliberately identical to the live Sumeragi-v2 bound. A smaller
/// offline bound would let consensus finalize a top-up for which no portable
/// proof could subsequently be produced.
pub const KAGEMUSHA_TOPUP_FINALITY_MAX_VALIDATORS_V2: usize = MAX_VALIDATORS_PER_HEIGHT;
/// Maximum roster activation windows in one authenticated finality artifact.
///
/// A release binds exactly one immutable roster window. Rotation publishes a
/// new content-addressed release instead of making every verifier ingest
/// unrelated historical or future validator sets.
pub const KAGEMUSHA_TOPUP_FINALITY_MAX_ROSTER_WINDOWS_V2: usize = 1;
/// Maximum canonical Norito bytes accepted for one compact top-up finality proof.
///
/// The epoch-boundary case retains the complete next-epoch identity snapshot,
/// including all 4,096 bounded `PoPs` plus maximum current and parent signer
/// lists. The exact maximum wire-shape test below pins the encoded size below
/// this 2 MiB ingress cap.
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
/// The exact-state Eq/Ep proof pair requires a larger release envelope than
/// the retired digest-bound proof. Text transports must independently bound
/// their base64url representation (at most 43,691 unpadded bytes, plus their
/// transport discriminator) before allocation or decoding.
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
/// Source-compatible domain name for the retained V2 state-boundary carrier.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_DOMAIN_V1: &[u8] =
    KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_DOMAIN_V5;
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
/// V4 is deliberately not an alias for ABI 19: its public accumulator layout,
/// fold transcripts, key parsing parameters, and artifact framing all depend
/// on an authenticated IPA degree.
pub const KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 21;
/// Exact schema identifier for the degree-parameterized artifact manifest.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4: &str =
    "kagemusha.offline.recursive_spend.artifact_manifest.v4";
/// Exact schema of the independently pinned reviewed dirty source closure.
pub const KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_SCHEMA_V1: &str = "iroha.reviewed-source-closure.v1";
/// Maximum untracked regular-file entries in one reviewed source closure.
pub const KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_UNTRACKED_FILES_V1: usize = 100_000;
/// Maximum ignored root `Cargo.lock` bytes admitted by the reviewed closure.
pub const KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_CARGO_LOCK_BYTES_V1: u64 = 16 * 1024 * 1024;
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
/// Minimum number of `u32` values in the ABI-21/V4 single-column Step ABI.
///
/// This is the exact layout at the authenticated compact degree (`k = 16`).
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_MIN_PUBLIC_INPUT_LIMBS_V4: usize = 64;
/// Maximum number of `u32` values in the ABI-21/V4 single-column Step ABI.
///
/// This is the exact layout at the authenticated compact degree (`k = 16`).
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_MAX_PUBLIC_INPUT_LIMBS_V4: usize = 64;
/// Canonical ABI-21/V4 field-neutral public inputs for the EqAffine/Vesta step circuit.
///
/// The embedded `operation_protocol_v2` label versions the subordinate, field-neutral
/// operation-vector layout. It is not a release or chain-wire version and cannot select
/// a V2/V3 executor. The operation row remains V2, while the compact V5 recursive-state
/// layout deliberately changes the V4 circuit identity and invalidates earlier candidates.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PUBLIC_INPUTS_SCHEMA_V4: &[u8] = br#"{"schema":"kagemusha_recursive_spend_step_eq_compact_v5","layout":"single_column_field","elements":64,"ipa_round_count":16,"semantic_authority":"step_eq","semantic_header":{"elements":20,"encoding":"canonical_u128_chunks","fields":["compact_profile_version","parent_count","proof_step_count","public_statement_digest[2]","operation_poseidon_fp[2]","parent_state_poseidon_fp[2][2]","result_state_poseidon_fp[2]","manifest_sha256[2]","step_eq_protocol_sha256[2]","step_ep_protocol_sha256[2]","live_selector"]},"ipa_accumulator":{"wire_version":5,"elements":36,"formula":"2*ipa_round_count+4","encoding":"canonical_u128_chunks"},"reciprocal_audits":{"hash":"sha256","digests":4,"elements_per_digest":2},"private_witness":{"state_layout_version":5,"state_limbs":138,"parent_slots":2,"operation_field_elements":135,"operation_limbs":1080}}"#;
/// Canonical ABI-21/V4 field-neutral public inputs for the EpAffine/Pallas step circuit.
///
/// As with the Eq schema, `operation_protocol_v2` is the subordinate operation-vector
/// ABI version, not permission to enter a historical Kagemusha execution path.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PUBLIC_INPUTS_SCHEMA_V4: &[u8] = br#"{"schema":"kagemusha_recursive_spend_step_ep_compact_v5","layout":"single_column_field","elements":64,"ipa_round_count":16,"semantic_authority":"step_eq","role":"lineage_and_reciprocal_wrapper","semantic_header":{"elements":20,"encoding":"canonical_u128_chunks","fields":["compact_profile_version","parent_count","proof_step_count","public_statement_digest[2]","operation_poseidon_fp[2]","parent_state_poseidon_fp[2][2]","result_state_poseidon_fp[2]","manifest_sha256[2]","step_eq_protocol_sha256[2]","step_ep_protocol_sha256[2]","live_selector"]},"ipa_accumulator":{"wire_version":5,"elements":36,"formula":"2*ipa_round_count+4","encoding":"canonical_u128_chunks"},"reciprocal_audits":{"hash":"sha256","digests":4,"elements_per_digest":2}}"#;
/// Version of the compact canonical cross-field state boundary.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V5: u16 = 5;
/// Version stored in limb zero of the compact cross-field recursive state.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5: u32 = 5;
/// Exact number of unreduced `u32` limbs carried between both Pasta fields.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5: usize = 138;
/// Source-compatible name for the state-boundary version used by the retained V2 carrier.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V2: u16 =
    KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V5;
/// Source-compatible name for the state layout used by the retained V2 carrier.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2: u32 =
    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
/// Source-compatible name for the state size used by the retained V2 carrier.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2: usize =
    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5;
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
/// Schema identifier for the configured Kagemusha release-signing policy.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1: &str =
    "kagemusha.offline.recursive_spend.release_policy.v1";
/// Historical version-one attestation schema identifier retained by policy tooling.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V1: &str =
    "kagemusha.offline.recursive_spend.release_attestation.v1";
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
/// Historical version-one attestation file name retained by policy tooling.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V1: &str =
    "release-attestation.norito";
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
/// Lowest degree admitted for the complete fixed-shape V4 Step circuit.
///
/// This is the authenticated fixed degree of the compact V5 profile.
pub const KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4: u32 = 16;
/// Highest degree whose serialized Pasta parameters fit the release artifact
/// corridor with a conservative margin.
pub const KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4: u32 = 16;
/// Minimum unusable-row reservation required by the Halo2 base circuit.
pub const KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4: u32 = 9;
/// Exact supported challenge-phase vector length.
///
/// The compact V5 Kagemusha profile has no challenge-dependent witness work. Admitting empty
/// second/third advice phases makes Halo2 re-synthesise the phase-zero circuit
/// and retains otherwise unused domain-sized polynomials during proving.
/// Circuit parameters therefore authenticate the one phase that is actually
/// constrained instead of reserving speculative future phases.
pub const KAGEMUSHA_STEP_CIRCUIT_MAX_PHASES_V4: usize = 1;
/// Maximum configured columns of any one class in a phase.
pub const KAGEMUSHA_STEP_CIRCUIT_MAX_COLUMNS_V4: u32 = 256;
/// Reviewed first-release advice-column profile for compact degree-16 generation.
pub const KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4: [u32; 1] = [8];
/// Reviewed first-release lookup-column profile for compact degree-16 generation.
pub const KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4: [u32; 1] = [1];
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
pub const KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4: u32 = 8 * 1024;
/// Absolute defensive ceiling for one canonical V4 Eq/Ep proof-pair payload.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4: u32 = 21_764;
/// Maximum processed proving-key payload admitted by the compact V5 profile.
pub const KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5: u64 = 96 * 1024 * 1024;
/// Maximum serialized `ParamsIPA` payload admitted by the compact V5 profile.
pub const KAGEMUSHA_COMPACT_PARAMS_IPA_MAX_BYTES_V5: u64 = 8 * 1024 * 1024;
/// Exact cryptographic profile embedded inside the ABI-21/V4 lifecycle.
pub const KAGEMUSHA_COMPACT_PROFILE_VERSION_V5: u32 = 5;
/// Maximum canonical recipient-only ABI-21 peer-payment archive.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4: usize = 32 * 1024 * 1024;
/// Maximum canonical provenance archive carried by one ABI-21 spendable branch.
///
/// This admits one maximum roster plus exactly two bounded anchors and finality
/// proofs, with fixed Norito framing headroom. Inventories remain capped at two.
pub const KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4: usize =
    KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_USIZE_V2
        + KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
            * (KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_USIZE_V2
                + KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_USIZE_V2)
        + 64 * 1024;
/// Maximum canonical ABI-21 online-to-offline chain request.
///
/// This covers the 192 KiB shield-proof ceiling, the 16 KiB optional device
/// attestation, and bounded authorization/note/release metadata with headroom.
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
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4: u64 = 256 * 1024 * 1024;
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
///
/// The order is part of the capability contract and follows the canonical
/// Eq-then-Ep profile order and the four-artifact order within each profile.
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
/// Maximum canonical roster artifact size; one full 4,096-validator window is
/// pinned below this bound by an exact maximum wire-shape test.
pub const KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2: u64 = 2 * 1024 * 1024;
/// Native-width mirror of [`KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2`].
const KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_USIZE_V2: usize = 2 * 1024 * 1024;
/// Production-promotion gate for the ABI-21/V4 paired recursive backend.
///
/// This is false in default and candidate builds. It becomes true only when
/// the non-default `kagemusha-production-enabled` Cargo feature is selected by
/// an explicitly promoted bridge build. Runtime readiness additionally
/// requires an installed, authenticated V4 release with the exact verifier and
/// prover inventory; this compile-time gate never substitutes for release
/// authentication.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE: bool =
    cfg!(feature = "kagemusha-production-enabled");
/// Canonical verifier-record namespace for Kagemusha proof admission.
pub const KAGEMUSHA_VERIFIER_NAMESPACE: &str = "offline_kagemusha";
/// Transparent backend used by the independent confidential transfer circuits.
pub const KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND: &str = "halo2/ipa";

/// Canonical public-input schema hash for the ABI-21/V4 `StepEq` verifier record.
#[must_use]
pub fn kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v4() -> [u8; 32] {
    Hash::new(KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PUBLIC_INPUTS_SCHEMA_V4).into()
}

/// Canonical public-input schema hash for the ABI-21/V4 `StepEp` verifier record.
#[must_use]
pub fn kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v4() -> [u8; 32] {
    Hash::new(KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PUBLIC_INPUTS_SCHEMA_V4).into()
}

/// Compute the SHA-256 content identifier used by Kagemusha release files.
#[must_use]
pub fn kagemusha_recursive_spend_release_sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

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
    /// Recursive inputs disagree on the chain.
    RecursiveSpendChainMismatch,
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
            Self::RecursiveSpendChainMismatch => {
                f.write_str("Kagemusha recursive inputs use different chains")
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
///
/// Its private fields prevent unsigned release material from entering the
/// configured ABI-21 catalog without authentication.
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
    Ok(iroha_zkp_halo2::poseidon::hash_bytes(&to_bytes(value)?))
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
    chain_id: &ChainId,
    definition_id: &AssetDefinitionId,
) -> AccountId {
    let seed_material = format!(
        "{OFFLINE_ESCROW_SEED_LABEL}|{}|{definition_id}",
        chain_id.as_str()
    );
    let seed: [u8; Hash::LENGTH] = Hash::new(seed_material).into();
    let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
        .expect("fixed Offline escrow Ed25519 account seed must derive");
    AccountId::new(keypair.public_key().clone())
}

#[model]
mod model {
    use super::*;

    /// Sole first-release Kagemusha device authority key.
    ///
    /// The wire value is exactly one canonical uncompressed SEC1 NIST P-256
    /// point (`0x04 || x || y`). There is deliberately no algorithm tag or
    /// selector in this type or any request carrying it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
        norito(transparent),
        norito(with = "crate::json_helpers::fixed_bytes")
    )]
    pub struct KagemushaDevicePublicKeyV2(
        pub(super) [u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V2],
    );

    /// Sole first-release Kagemusha device signature.
    ///
    /// The wire value is exactly the fixed-width big-endian ECDSA scalar pair
    /// `r || s`. Both scalars must be in `1..n`, and `s` must be low. DER and
    /// recoverable encodings are not part of the protocol.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
        norito(transparent),
        norito(with = "crate::json_helpers::fixed_bytes")
    )]
    pub struct KagemushaDeviceSignatureV2(pub(super) [u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2]);

    /// Exact amount contract for fractional recursive Kagemusha cash.
    ///
    /// `atomic_units` is the positive proof amount. `scale` is copied from the
    /// authoritative asset definition and determines the public quantity
    /// spelling used when charging or crediting the online balance.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaScaledAmountV2 {
        /// Positive proof amount in the asset's smallest unit.
        pub atomic_units: u128,
        /// Authoritative on-chain asset scale.
        pub scale: u32,
    }

    /// Scale-, chain-, and asset-bound spendable note descriptor.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaSpendableNoteDescriptorV2 {
        /// Chain that scopes the commitment and nullifier.
        pub chain_id: ChainId,
        /// Asset committed by the confidential note.
        pub asset: AssetDefinitionId,
        /// Current note commitment.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub note_commitment: [u8; 32],
        /// Nullifier consumed by the next split or redemption.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub spend_nullifier: [u8; 32],
        /// Exact amount at the authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
    }

    /// Secret-free Merkle authentication path retained with an owned note.
    ///
    /// Witness nodes are deliberately absent: native verification recomputes
    /// every Poseidon node from the note commitment and these canonical fields.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaConfidentialMerklePathV2 {
        /// One canonical sibling field element per tree level.
        pub siblings: Vec<[u8; 32]>,
        /// One left (`0`) or right (`1`) direction per tree level.
        pub directions: Vec<u8>,
        /// Root authenticated by the complete path.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root: [u8; 32],
    }

    /// Proof-bound membership state required to spend one recursive output.
    ///
    /// `input_path` authenticates the owned note at `leaf_index`.
    /// `dummy_input_path` authenticates a distinct canonical empty leaf against
    /// the same root for the fixed two-input confidential circuits.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaNoteMembershipWitnessV2 {
        /// Confidential-tree position of the owned note.
        pub leaf_index: u32,
        /// Path authenticating the owned note commitment.
        pub input_path: KagemushaConfidentialMerklePathV2,
        /// Path authenticating a distinct empty leaf for dummy input two.
        pub dummy_input_path: KagemushaConfidentialMerklePathV2,
    }

    /// Canonical branch coordinate inside one top-up lineage.
    ///
    /// The first `depth` most-significant bits of `path_bits` identify the
    /// branch. Unused bits must be zero. A recipient output appends bit `0` and
    /// a sender-change output appends bit `1`. This makes sibling redemptions
    /// disjoint while allowing the ledger to reject an ancestor and any of its
    /// descendants by a deterministic prefix check.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendBranchPathV2 {
        /// Stable top-up lineage root, unique for one online-to-offline operation.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub lineage_root: [u8; 32],
        /// Number of significant path bits, from zero through 64.
        pub depth: u8,
        /// Big-endian branch bits; unused low-order bits are canonical zeroes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub path_bits: [u8; 8],
    }

    /// Replay-safe conflict claim for one independently spendable lineage leaf.
    ///
    /// `transition_tags` is one contiguous byte string containing exactly
    /// `path.depth` consecutive 24-byte entries. Entry `i` is the non-zero,
    /// domain-separated 192-bit tag of the complete proof-bound transition
    /// digest selected at the edge from depth `i` to `i + 1`.
    /// Carrying every ancestor choice prevents recipient/change outputs from
    /// alternative splits of the same parent from being mixed to inflate value.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendBranchClaimV2 {
        /// Canonical leaf coordinate used for ancestor/descendant conflicts.
        pub path: KagemushaRecursiveSpendBranchPathV2,
        /// Contiguous exact-depth transition-selection history with no padding.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub transition_tags: Vec<u8>,
    }

    /// Public inputs used by the native bridge to derive one receiver-owned
    /// confidential output.
    ///
    /// The receiver's local note opening is deliberately not part of this
    /// archive. It is supplied through a separate native-only archive and must
    /// never cross a payment, Torii, or peer protocol boundary.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecipientOutputDerivationRequestV2 {
        /// Chain that scopes the output commitment and nullifier.
        pub chain_id: ChainId,
        /// Asset definition committed by the receiver output.
        pub asset: AssetDefinitionId,
        /// Exact requested amount at the authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
        /// Non-zero receiver-created nonce that domain-separates derivation.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub request_id: [u8; 32],
    }

    /// Public descriptor plus sender-prover material derived for one receiver
    /// output by the native bridge.
    ///
    /// `sender_output_prover_material` may contain only the amount opening,
    /// `rho`, and owner tag required by the sender's proof. It must never
    /// contain the receiver spend key or the output diversifier.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecipientOutputDerivationResultV2 {
        /// Receiver-owned confidential output descriptor.
        pub recipient_output: KagemushaSpendableNoteDescriptorV2,
        /// Opaque, bounded opening archive consumed by the sender prover.
        pub sender_output_prover_material: Vec<u8>,
    }

    /// Canonical unsigned fields of a receiver-created payment request.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecipientPaymentRequestSigningPayloadV2 {
        /// Chain that scopes the note and its nullifier.
        pub chain_id: ChainId,
        /// Asset definition requested by the receiver.
        pub asset: AssetDefinitionId,
        /// Exact requested amount at the authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
        /// Online account used only for recipient/request identity binding.
        pub recipient: AccountId,
        /// Domain-separated receiver-device public-key reference.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recipient_key_reference: [u8; 32],
        /// Registered receiver device identifier.
        pub receiver_device_id: String,
        /// Device-bound key that authenticates this request and its later ACK.
        pub receiver_public_key: KagemushaDevicePublicKeyV2,
        /// Unique request/nonce identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub request_id: [u8; 32],
        /// Request creation time in Unix milliseconds.
        pub issued_at_ms: u64,
        /// Exclusive Unix expiry in milliseconds.
        pub expires_at_ms: u64,
        /// Requested recipient output descriptor.
        pub recipient_output: KagemushaSpendableNoteDescriptorV2,
        /// Peer-carried opaque output-opening archive consumed by the sender prover.
        pub sender_output_prover_material: Vec<u8>,
    }

    /// Receiver-created, nonce-bound and device-signed request for one exact offline payment.
    ///
    /// `sender_output_prover_material` is part of the signed peer request but
    /// remains opaque to wallet code. The native bridge derives it from a
    /// receiver-held local note opening and the public request fields. It
    /// contains only the amount opening, `rho`, and owner tag needed to prove
    /// the requested commitment; it must never contain the receiver's spend
    /// key or diversifier.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecipientPaymentRequestV2 {
        /// Chain that scopes the note and its nullifier.
        pub chain_id: ChainId,
        /// Asset definition requested by the receiver.
        pub asset: AssetDefinitionId,
        /// Exact requested amount at the authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
        /// Online account used only for recipient/request identity binding.
        pub recipient: AccountId,
        /// Stable receiver-side key reference; not secret key bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recipient_key_reference: [u8; 32],
        /// Registered receiver device identifier.
        pub receiver_device_id: String,
        /// Device-bound public key authenticating the request and later ACK.
        pub receiver_public_key: KagemushaDevicePublicKeyV2,
        /// Unique request/nonce identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub request_id: [u8; 32],
        /// Request creation time in Unix milliseconds.
        pub issued_at_ms: u64,
        /// Exclusive Unix expiry in milliseconds.
        pub expires_at_ms: u64,
        /// Requested recipient output descriptor.
        pub recipient_output: KagemushaSpendableNoteDescriptorV2,
        /// Peer-carried opaque output-opening archive consumed by the sender prover.
        pub sender_output_prover_material: Vec<u8>,
        /// Receiver-device signature over the canonical unsigned fields.
        pub signature: KagemushaDeviceSignatureV2,
    }

    /// Platform assertion made by the exact hardware key admitted at registration.
    ///
    /// Both platforms carry the same canonical raw low-S P-256 signature. iOS
    /// additionally carries the App Attest authenticator data that Apple binds
    /// ahead of the client-data hash.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaAndroidKeyMintHardwareAssertionV1 {
        /// Canonical raw low-S P-256 signature (`r || s`).
        pub signature: KagemushaDeviceSignatureV2,
    }

    /// Apple App Attest assertion result for an online operation.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaIosAppAttestHardwareAssertionV1 {
        /// Exact authenticator data returned by `generateAssertion`.
        pub authenticator_data: Vec<u8>,
        /// Canonical raw low-S P-256 signature (`r || s`).
        pub signature: KagemushaDeviceSignatureV2,
    }

    /// Typed platform assertion, without a stringly-typed fallback variant.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "platform", content = "assertion", rename_all = "snake_case")]
    pub enum KagemushaOnlineHardwareAssertionV1 {
        /// Android `KeyMint` `SHA256withECDSA` assertion from a maxUsageCount=1 key.
        AndroidKeyMint(KagemushaAndroidKeyMintHardwareAssertionV1),
        /// Apple App Attest assertion over authenticatorData || clientDataHash.
        IosAppAttest(KagemushaIosAppAttestHardwareAssertionV1),
    }

    /// Self-contained payer/recipient hardware authorization carried inside one V2 archive.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRequestAuthorizationV2 {
        /// Account bound by the registered hardware assertion key.
        pub authority: AccountId,
        /// Registered device identifier used for exact registration lookup.
        pub device_id: String,
        /// Asset definition bound into the hardware-signed operation.
        pub asset_definition_id: AssetDefinitionId,
        /// Globally unique chain idempotency/replay identifier.
        ///
        /// Unlike nonces and payload digests, this identifier is not scoped by
        /// `authority`; every Kagemusha V2 chain operation shares one replay
        /// namespace.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Request creation time in Unix milliseconds.
        pub issued_at_ms: u64,
        /// Inclusive request expiry time in Unix milliseconds.
        pub expires_at_ms: u64,
        /// Unique signed nonce.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub nonce: [u8; 32],
        /// Digest of the canonical unsigned top-up or redemption payload.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub payload_digest: [u8; 32],
        /// Canonical Iroha hash of the exact registration admitted by consensus.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub registration_hash: [u8; 32],
        /// Typed assertion from the registered online hardware key.
        pub hardware_assertion: KagemushaOnlineHardwareAssertionV1,
    }

    /// Typed public-to-confidential shield evidence for one online top-up.
    ///
    /// The proof bytes remain opaque to wallets. The duplicated root and leaf
    /// fields let Torii reject stale requests before execution; the executor
    /// parses the proof public inputs and rechecks them against authoritative
    /// ledger state before mutating balances or the confidential tree.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaTopUpShieldEvidenceV2 {
        /// Authoritative confidential root before inserting the top-up note.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub initial_root: [u8; 32],
        /// Root after inserting exactly the requested top-up note.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub finalized_root: [u8; 32],
        /// Authoritative zero-leaf position consumed by the insertion.
        pub leaf_index: u32,
        /// Canonical shield proof and registered verifier reference.
        pub proof: ProofAttachment,
    }

    /// Compact, ledger-resolvable reference carried by spendable peer bundles.
    ///
    /// The complete finalized anchor remains in chain state and in the init
    /// transition archive. Peer payloads carry only this strict identity pair;
    /// redemption resolves it before crediting any value.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendTopUpAnchorRefV2 {
        /// Stable top-up operation identifier used for the chain-state lookup.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub topup_operation_id: [u8; 32],
        /// Canonical digest of the complete finalized anchor.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub anchor_digest: [u8; 32],
    }

    /// Bounded projection of the live Sumeragi-v2 height context needed to
    /// authenticate one Commit certificate offline.
    ///
    /// `context_id` is copied from the persisted [`HeightContext`] and is part
    /// of the exact live [`crate::block::consensus_v2::Vote::signature_preimage`]
    /// through the certificate round. Every non-roster identity field is
    /// retained so verification can reconstruct and validate the complete
    /// context with the manifest-authenticated roster window, then require its
    /// computed identifier to equal `context_id`. This avoids duplicating the
    /// current roster in every proof without making the context identifier an
    /// opaque, attacker-selected cross-chain binding.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaTopUpFinalityHeightContextV2 {
        /// Typed identifier of the complete persisted height context.
        pub context_id: HeightContextId,
        /// Chain identifier committed by the complete height context.
        pub chain_id: ChainId,
        /// Live Sumeragi-v2 wire protocol revision.
        pub protocol_version: u16,
        /// Height governed by the projected context.
        pub height: u64,
        /// Finalized validator-election epoch.
        pub epoch: u64,
        /// Last height governed by the current frozen epoch snapshot.
        pub epoch_end_height: u64,
        /// Complete next-epoch transition on an epoch-boundary height.
        pub next_epoch_snapshot: Option<FinalizedNextEpochSnapshot>,
        /// Consensus mode governing the frozen roster.
        pub mode: ConsensusMode,
        /// Parent Commit certificate, absent at genesis or an audited snapshot boundary.
        pub parent_commit_qc: Option<QuorumCertificate>,
        /// Audited snapshot anchor when no parent `CommitQC` exists.
        pub snapshot_bootstrap: Option<SnapshotBootstrapAnchor>,
        /// Frozen Nexus/AMX context commitment.
        pub nexus_amx_context_hash: Hash,
        /// Frozen data-availability layout.
        pub da_layout: DataAvailabilityLayout,
        /// Finalized leader-rotation seed.
        pub leader_seed: [u8; 32],
    }

    /// Canonical Sumeragi-v2 height-context projection and Commit certificate.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaTopUpFinalityCompactQcV2 {
        /// Bounded immutable consensus-context projection for the finalized height.
        pub height_context: KagemushaTopUpFinalityHeightContextV2,
        /// Exact Sumeragi-v2 Commit certificate persisted by Kura.
        pub certificate: QuorumCertificate,
    }

    /// Canonical balanced-Merkle inclusion path for one finalized top-up.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaTopUpAnchorMerkleProofV2 {
        /// Position in strict operation-id order.
        pub leaf_index: u32,
        /// Number of real leaves in the block-local tree.
        pub leaf_count: u32,
        /// Siblings from leaf level to root.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub siblings: Vec<[u8; 32]>,
    }

    /// Offline-verifiable proof that a finalized Commit QC authenticated one
    /// exact `(operation_id, anchor_digest)` write.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaTopUpFinalityProofV2 {
        /// Proof layout version.
        pub version: u16,
        /// Exact compact anchor identity bound by the recursive init proof.
        pub anchor: KagemushaRecursiveSpendTopUpAnchorRefV2,
        /// Commit QC with its roster `PoPs` supplied by the trusted artifact.
        pub commit_qc: KagemushaTopUpFinalityCompactQcV2,
        /// Bounded block-local inclusion proof.
        pub anchor_path: KagemushaTopUpAnchorMerkleProofV2,
    }

    /// Ordered validator set trusted for one non-overlapping height window.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaTopUpFinalityRosterWindowV2 {
        /// First accepted block height, inclusive.
        pub activates_at_height: u64,
        /// First rejected block height, exclusive.
        pub withdraws_at_height: u64,
        /// Consensus mode governing this immutable roster window.
        pub consensus_mode: ConsensusMode,
        /// Exact ordered BLS validator identities and voting powers.
        pub validator_set: Vec<ValidatorPower>,
        /// Fixed-size BLS proofs of possession aligned one-to-one with `validator_set`.
        pub validator_set_pops: Vec<[u8; 96]>,
    }

    /// Content-addressed trust artifact prefetched before any peer exchange.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaTopUpFinalityRosterArtifactV2 {
        /// Artifact layout version.
        pub version: u16,
        /// Chain whose vote domain is trusted.
        pub chain_id: ChainId,
        /// Human-readable roster generation selected by the manifest descriptor.
        pub artifact_generation: String,
        /// Strictly ordered, non-overlapping validator windows.
        pub windows: Vec<KagemushaTopUpFinalityRosterWindowV2>,
    }

    /// Canonical descriptor for one previous branch consumed by a V2 split.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendInputBranchV2 {
        /// Canonical digest of the complete previous recursive bundle.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub bundle_digest: [u8; 32],
        /// Exact note consumed by the confidential transfer.
        pub input_note: KagemushaSpendableNoteDescriptorV2,
        /// Canonical conflict claims of the consumed branch. A joined note
        /// carries one transition-bound claim per contributing ancestor.
        pub branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Root at which the input transfer output was created.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub input_root: [u8; 32],
        /// Recursive proof-step count of the consumed bundle.
        pub proof_step_count: u32,
        /// Peer-hop count of the consumed bundle.
        pub peer_hop_count: u32,
    }

    /// Role of one independently spendable output from a V2 split.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "branch", content = "value", rename_all = "snake_case")]
    pub enum KagemushaRecursiveSpendBranchV2 {
        /// Receiver-owned output branch.
        Recipient,
        /// Sender-owned change branch.
        Change,
    }

    /// Canonical unshield-v3 public words cross-checked by the V4 redemption transition.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaUnshieldPublicInputsBindingV2 {
        /// First input note commitment.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub input_commitment_0: [u8; 32],
        /// Optional second input note commitment; zero for Kagemusha redemption.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub input_commitment_1: [u8; 32],
        /// First input nullifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub nullifier_0: [u8; 32],
        /// Optional second input nullifier; zero for Kagemusha redemption.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub nullifier_1: [u8; 32],
        /// Zero for full redemption or the partial-redemption change commitment.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub change_output_commitment: [u8; 32],
        /// Root at which the input note is proved live.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root: [u8; 32],
        /// Confidential-circuit encoding of the exact credited atomic amount.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub public_amount: [u8; 32],
        /// Canonical asset tag.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub asset_tag: [u8; 32],
        /// Canonical chain tag.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub chain_tag: [u8; 32],
    }

    /// Curve role of one proof in the current two-proof Pasta recursion pair.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "parity", content = "value", rename_all = "snake_case")]
    pub enum KagemushaPastaCycleParityV1 {
        /// EqAffine/Vesta recursive step over the Pallas scalar field.
        StepEq,
        /// EpAffine/Pallas recursive step over the Vesta scalar field.
        StepEp,
    }

    /// Canonical exact state vector carried across the Pasta field boundary.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendStateBoundaryV2 {
        /// State-boundary layout version.
        pub layout_version: u16,
        /// All 138 canonical `u32` limbs, including compact branch-history accumulators.
        pub state_limbs: Vec<u32>,
    }

    /// Exact dynamic offsets for one authenticated V4 public instance column.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPastaPublicLayoutV4 {
        /// Authenticated IPA degree and accumulator round count.
        pub ipa_round_count: u32,
        /// Field-neutral limbs occupied by either carried IPA accumulator.
        pub accumulator_limbs: u32,
        /// First Eq carried-accumulator limb.
        pub parent_eq_accumulator_offset: u32,
        /// First Ep carried-accumulator limb.
        pub parent_ep_accumulator_offset: u32,
        /// First Eq deferred-audit word.
        pub parent_eq_deferred_offset: u32,
        /// First Ep deferred-audit word.
        pub parent_ep_deferred_offset: u32,
        /// Final public limb selecting bootstrap (`0`) or a live Step (`1`).
        pub live_selector_offset: u32,
        /// Exact length of the single public instance column.
        pub instance_column_limbs: u32,
    }

    /// Canonical Halo2 base-circuit configuration authenticated by a V4 profile.
    ///
    /// `Default` is intentionally an invalid sentinel. Key readers and runtime
    /// constructors must receive this value from an authenticated V4 manifest;
    /// no FFI or local configuration value may substitute for it.
    #[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaStepCircuitParamsV4 {
        /// Exact parameter-layout version.
        pub version: u16,
        /// Halo2 domain exponent and IPA round count.
        pub k: u32,
        /// Advice-column count in each challenge phase.
        pub num_advice_per_phase: Vec<u32>,
        /// Lookup-advice-column count in each challenge phase.
        pub num_lookup_advice_per_phase: Vec<u32>,
        /// Fixed-column count.
        pub num_fixed: u32,
        /// Range-table lookup width.
        pub lookup_bits: u32,
        /// Exact number of public instance columns.
        pub num_instance_columns: u32,
        /// Exact dynamic length of the single public instance column.
        pub public_input_limbs: u32,
        /// Row reservation used during deterministic layout calibration.
        pub minimum_unusable_rows: u32,
        /// Exact release cap for one ordinary parent proof transcript.
        pub max_parent_proof_bytes: u32,
    }

    /// Kind of content-addressed material bound to one V4 Pasta profile.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "kind", content = "value", rename_all = "snake_case")]
    pub enum KagemushaPastaCycleArtifactKindV4 {
        /// Canonical `ParamsIPA` generator material.
        ParamsIpa,
        /// Halo2 processed proving key.
        ProvingKey,
        /// Halo2 processed verifying key.
        VerifyingKey,
        /// Genuine selector-zero proof and terminally verified folds for absent slots.
        BootstrapWitness,
    }

    /// One immutable file in a V4 recursive-spend artifact manifest.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPastaCycleArtifactV4 {
        /// Material kind within the parity profile.
        pub kind: KagemushaPastaCycleArtifactKindV4,
        /// Safe single-component V4 file name.
        pub file_name: String,
        /// Exact framed byte length.
        pub size_bytes: u64,
        /// SHA-256 of the exact framed file bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub sha256: [u8; 32],
        /// Exact byte length of the unframed cryptographic payload.
        pub payload_size_bytes: u64,
        /// SHA-256 of the unframed cryptographic payload.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub payload_sha256: [u8; 32],
    }

    /// Canonical public header preceding one streamed `KRV4KEY` payload.
    ///
    /// The header intentionally contains no release-sized byte vector. A
    /// streaming loader validates these small role bindings first, then hashes
    /// exactly `payload_size_bytes` trailing bytes before exposing the payload.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPastaCycleFramedArtifactHeaderV4 {
        /// Exact `KRV4KEY` public-header layout version.
        pub version: u16,
        /// Manifest schema which authorizes this file.
        pub manifest_schema: String,
        /// Native bridge ABI required by this file.
        pub bridge_abi_version: u32,
        /// Exact paired-proof backend profile.
        pub proof_backend: String,
        /// Exact transcript profile.
        pub transcript_profile: String,
        /// Release generation selected by the manifest.
        pub generation: String,
        /// Curve/parity selected by this artifact.
        pub parity: KagemushaPastaCycleParityV1,
        /// Exact V4 circuit identifier for `parity`.
        pub circuit_id: String,
        /// `ParamsIPA` generation selected by the profile.
        pub parameter_generation: String,
        /// Authenticated IPA degree.
        pub ipa_k: u32,
        /// Domain-separated identity of the embedded canonical circuit parameters.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub circuit_params_sha256: [u8; 32],
        /// Value-free compiled protocol structure selected by this profile.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub compiled_protocol_structure_sha256: [u8; 32],
        /// Measured ordinary Step proof bytes selected by this profile.
        pub step_proof_size_bytes: u32,
        /// Role of the following payload.
        pub kind: KagemushaPastaCycleArtifactKindV4,
        /// Exact byte length of the following unframed payload.
        pub payload_size_bytes: u64,
        /// Raw SHA-256 of the following unframed payload.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub payload_sha256: [u8; 32],
    }

    /// V4 reference to the unchanged canonical top-up finality roster type.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaTopUpFinalityRosterArtifactReferenceV4 {
        /// Safe single-component V4 file name.
        pub file_name: String,
        /// Exact canonical Norito byte length.
        pub size_bytes: u64,
        /// SHA-256 of the exact canonical roster bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub sha256: [u8; 32],
        /// Exact generation declared by the roster archive.
        pub artifact_generation: String,
        /// Native finality verifier/circuit role.
        pub circuit_id: String,
        /// Stable product purpose.
        pub purpose: String,
        /// Exact Norito type name contained by the file.
        pub artifact_type: String,
        /// Required V4 bridge ABI.
        pub required_bridge_abi_version: u32,
    }

    /// Authenticated fixed configuration and key material for one V4 parity.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPastaCycleProofProfileV4 {
        /// Curve/parity implemented by this profile.
        pub parity: KagemushaPastaCycleParityV1,
        /// Exact V4 circuit identifier.
        pub circuit_id: String,
        /// Canonical `ParamsIPA` generation identifier.
        pub parameter_generation: String,
        /// Redundant, fail-closed IPA degree; must equal `circuit_params.k`.
        pub ipa_k: u32,
        /// Complete authenticated Halo2 base-circuit configuration.
        pub circuit_params: KagemushaStepCircuitParamsV4,
        /// Value-free structure identity shared by bootstrap and final protocol.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub compiled_protocol_structure_sha256: [u8; 32],
        /// Measured augmented proof bytes for this exact key and layout.
        pub step_proof_size_bytes: u32,
        /// Exactly one `ParamsIPA`, processed proving key, processed verifying
        /// key, and final-key selector-zero bootstrap-witness package, in that
        /// order. `circuit_params` is authenticated inline, never as a file.
        pub artifacts: Vec<KagemushaPastaCycleArtifactV4>,
    }

    /// One raw-byte-qualified untracked regular file in a reviewed source closure.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaReviewedSourceClosureManifestEntryV1 {
        /// SHA-256 of the exact regular-file bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub blob_sha256: [u8; 32],
        /// Canonical lowercase SHA-1 Git blob object id of the same bytes.
        pub git_blob_oid: String,
        /// Exact Git regular-file mode, `100644` or `100755`.
        pub git_mode: String,
        /// UTF-8 display form of the exact relative path bytes.
        pub path: String,
        /// Canonical Base64 of the exact relative POSIX path bytes.
        pub path_bytes_base64: String,
    }

    /// Canonical independently reviewed dirty source closure for one candidate.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaReviewedSourceClosureV1 {
        /// Exact reviewed-source-closure schema.
        pub schema: String,
        /// Signed base commit against which the tracked binary diff is defined.
        pub base_commit: String,
        /// Exact checked-out source commit; first release requires `base_commit`.
        pub source_commit: String,
        /// Derived dirty state; first release requires `true`.
        pub source_repo_dirty: bool,
        /// Producer full-tree SHA-256 of tracked, untracked, and `Cargo.lock` bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub source_tree_sha256: [u8; 32],
        /// SHA-256 of the canonical full-index binary Git diff from `source_commit`.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub tracked_binary_diff_sha256: [u8; 32],
        /// Exact number of raw-byte-sorted untracked manifest entries.
        pub untracked_file_count: u64,
        /// Raw-byte-sorted path/mode/blob identities for all untracked source files.
        pub untracked_path_mode_blob_oid_manifest:
            Vec<KagemushaReviewedSourceClosureManifestEntryV1>,
        /// SHA-256 of each entry's canonical compact sorted-key JSON plus LF.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub untracked_path_mode_blob_oid_manifest_sha256: [u8; 32],
        /// Exact ignored root `Cargo.lock` byte length.
        pub ignored_cargo_lock_size_bytes: u64,
        /// SHA-256 of the exact ignored root `Cargo.lock` bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub ignored_cargo_lock_sha256: [u8; 32],
        /// Cross-repository tracked-diff/untracked-manifest fingerprint.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub combined_source_fingerprint_sha256: [u8; 32],
    }

    /// Production release manifest for degree-parameterized paired Pasta proofs.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendArtifactManifestV4 {
        /// Exact V4 manifest schema identifier.
        pub schema: String,
        /// Manifest layout version.
        pub version: u16,
        /// Required native bridge ABI.
        pub bridge_abi_version: u32,
        /// Exact V4 paired-proof backend profile.
        pub proof_backend: String,
        /// Exact V4 transcript profile.
        pub transcript_profile: String,
        /// Human-readable release generation.
        pub generation: String,
        /// Lowercase 40-hex source revision.
        pub source_commit: String,
        /// SHA-256 of the exact tracked and untracked build source tree.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub source_tree_sha256: [u8; 32],
        /// Whether the exact build tree differed from `source_commit`.
        pub source_repo_dirty: bool,
        /// Complete independently pinned reviewed dirty source closure.
        pub reviewed_source_closure: KagemushaReviewedSourceClosureV1,
        /// SHA-256 of the exact canonical descriptor JSON bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_source_closure_descriptor_sha256: [u8; 32],
        /// Chain for which the release was built.
        pub chain_id: ChainId,
        /// Asset definition for which the release was built.
        pub asset: AssetDefinitionId,
        /// Authoritative fixed asset scale.
        pub asset_scale: u32,
        /// First block at which this release may issue notes.
        pub activation_height: u64,
        /// First block at which new issuance must stop.
        pub withdrawal_height: u64,
        /// Exact measured upper bound for one canonical V4 proof-pair payload.
        pub max_proof_bytes: u32,
        /// Eq then Ep V4 recursive-step profiles.
        pub profiles: Vec<KagemushaPastaCycleProofProfileV4>,
        /// Release-bound validator roster reference.
        pub topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV4,
        /// Digest of signed physical-device benchmark evidence.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub benchmark_evidence_sha256: [u8; 32],
        /// Digest of independent cryptographic review evidence.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub cryptographic_review_sha256: [u8; 32],
        /// Digest of the V4 signed release attestation.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub release_attestation_sha256: [u8; 32],
    }

    /// Immutable ABI-21 candidate captured before external review and device evidence exist.
    ///
    /// The embedded manifest commits the independently reviewed dirty source
    /// closure, network parameters, inline circuit configuration, exact eight
    /// recursive artifacts, and finality roster. Its benchmark, review, and
    /// attestation digest slots must all be zero.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendCandidateV4 {
        /// Exact candidate-record schema identifier.
        pub schema: String,
        /// Candidate-record layout version.
        pub version: u16,
        /// Complete pre-evidence manifest with its three promotion digest slots zeroed.
        pub manifest: KagemushaRecursiveSpendArtifactManifestV4,
    }

    /// Immutable release identity reviewed before evidence finalization.
    ///
    /// `candidate_sha256` commits the complete artifact/profile/roster/window
    /// inventory. The repeated human-auditable fields prevent a correctly signed
    /// review from being presented with an ambiguous release description.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendCryptographicReviewSubjectV4 {
        /// SHA-256 of the canonical immutable pre-evidence candidate.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub candidate_sha256: [u8; 32],
        /// Exact release generation copied from the candidate.
        pub generation: String,
        /// Exact source revision copied from the candidate.
        pub source_commit: String,
        /// Exact source-tree identity copied from the candidate.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub source_tree_sha256: [u8; 32],
        /// Exact reviewed dirty-tree state copied from the candidate.
        pub source_repo_dirty: bool,
        /// Exact independently pinned closure descriptor digest copied from the candidate.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_source_closure_descriptor_sha256: [u8; 32],
        /// Chain for which the reviewed candidate was built.
        pub chain_id: ChainId,
        /// Asset definition for which the reviewed candidate was built.
        pub asset: AssetDefinitionId,
        /// Native bridge ABI required by the reviewed candidate.
        pub bridge_abi_version: u32,
    }

    /// Production disposition recorded by an independent cryptographic review.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "decision", content = "value", rename_all = "snake_case")]
    pub enum KagemushaRecursiveSpendCryptographicReviewDecisionV4 {
        /// The exact candidate is approved for release finalization.
        Approved,
        /// The exact candidate is rejected and must not be finalized.
        Rejected,
    }

    /// Closed, canonically ordered set of security properties reviewed for V4.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "check", content = "value", rename_all = "snake_case")]
    pub enum KagemushaRecursiveSpendCryptographicReviewCheckV4 {
        /// Recursive-circuit constraints cover every claimed state transition.
        RecursiveCircuitConstraintCoverage,
        /// Pasta-cycle recursion and transcripts are domain- and lineage-bound.
        RecursiveCycleAndTranscriptBinding,
        /// Public inputs bind the complete state transition and operation.
        PublicInputAndStateTransitionBinding,
        /// Parameters, artifacts, and verifying keys bind the reviewed candidate.
        ArtifactParameterAndVerifyingKeyBinding,
        /// Nullifiers, replay protection, and finality inputs are correctly constrained.
        NullifierReplayAndFinalityBinding,
        /// Parsers are canonical and all attacker-controlled resources are bounded.
        ParserCanonicalizationAndResourceBounds,
    }

    /// Result of one mandatory cryptographic-review check.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "status", content = "value", rename_all = "snake_case")]
    pub enum KagemushaRecursiveSpendCryptographicReviewCheckStatusV4 {
        /// The referenced evidence supports the reviewed property.
        Passed,
        /// The referenced evidence does not support the reviewed property.
        Failed,
    }

    /// One content-addressed mandatory check inside a V4 review.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendCryptographicReviewCheckResultV4 {
        /// Mandatory reviewed property.
        pub check: KagemushaRecursiveSpendCryptographicReviewCheckV4,
        /// Review result; production finalization requires `Passed`.
        pub status: KagemushaRecursiveSpendCryptographicReviewCheckStatusV4,
        /// SHA-256 of property-specific evidence retained by the reviewer.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub evidence_sha256: [u8; 32],
    }

    /// Exact domain-separated payload signed by every V4 reviewer.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendCryptographicReviewPayloadV4 {
        /// Cross-protocol replay separator.
        pub domain: String,
        /// Exact immutable candidate under review.
        pub subject: KagemushaRecursiveSpendCryptographicReviewSubjectV4,
        /// Review disposition; production requires `Approved`.
        pub decision: KagemushaRecursiveSpendCryptographicReviewDecisionV4,
        /// SHA-256 of the complete retained review report.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub report_sha256: [u8; 32],
        /// Exact Eq-then-Ep cryptographic artifact roles reviewed for ABI-21.
        pub artifact_roles: Vec<String>,
        /// Exact ordered set of mandatory, independently evidenced checks.
        pub checks: Vec<KagemushaRecursiveSpendCryptographicReviewCheckResultV4>,
    }

    /// One policy-authorized signature over a complete V4 review payload.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendCryptographicReviewApprovalV4 {
        /// Reviewer identity selected by the trusted release policy.
        pub public_key: PublicKey,
        /// Signature over the exact domain, candidate, report, roles, and checks.
        pub signature: SignatureOf<KagemushaRecursiveSpendCryptographicReviewPayloadV4>,
    }

    /// Canonical signed independent cryptographic-review evidence for ABI-21/V4.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendCryptographicReviewEvidenceV4 {
        /// Exact V4 cryptographic-review schema.
        pub schema: String,
        /// Cryptographic-review envelope version.
        pub version: u16,
        /// Candidate-bound review decision signed by every approval.
        pub payload: KagemushaRecursiveSpendCryptographicReviewPayloadV4,
        /// Strictly ascending, unique reviewer approvals.
        pub approvals: Vec<KagemushaRecursiveSpendCryptographicReviewApprovalV4>,
    }

    /// Independent authority role required to promote an authenticated release.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "role", content = "value", rename_all = "snake_case")]
    pub enum KagemushaRecursiveSpendReleaseApprovalRoleV1 {
        /// Operational release authority approving publication.
        Release,
        /// Independent cryptographic reviewer approving the referenced report.
        CryptographicReview,
        /// Device-lab authority approving the referenced physical-device measurements.
        PhysicalDeviceBenchmark,
    }

    /// Immutable subject shared by every role-specific V4 release approval.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendReleaseAttestationSubjectV4 {
        /// SHA-256 of the canonical V4 manifest with its attestation slot zeroed.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub manifest_subject_sha256: [u8; 32],
        /// Exact release generation copied from the V4 manifest.
        pub generation: String,
        /// Exact source revision copied from the V4 manifest.
        pub source_commit: String,
        /// Exact source-tree identity copied from the V4 manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub source_tree_sha256: [u8; 32],
        /// Exact dirty-tree state copied from the V4 manifest.
        pub source_repo_dirty: bool,
        /// Exact independently pinned closure descriptor digest copied from the manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_source_closure_descriptor_sha256: [u8; 32],
        /// Digest of the signed physical-device evidence file.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub benchmark_evidence_sha256: [u8; 32],
        /// Digest of the independent cryptographic review file.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub cryptographic_review_sha256: [u8; 32],
    }

    /// Domain-separated value signed for one independent V4 approval role.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendReleaseApprovalPayloadV4 {
        /// Cross-protocol replay separator.
        pub domain: String,
        /// Authority role for which this signature is valid.
        pub role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
        /// Complete V4 release subject approved by the signer.
        pub subject: KagemushaRecursiveSpendReleaseAttestationSubjectV4,
    }

    /// One role-bound signature inside a V4 release attestation.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendReleaseApprovalV4 {
        /// Independent authority role represented by this signature.
        pub role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
        /// Exact signer key selected by the trusted release policy.
        pub public_key: PublicKey,
        /// Signature over the V4 domain, role, and complete subject.
        pub signature: SignatureOf<KagemushaRecursiveSpendReleaseApprovalPayloadV4>,
    }

    /// Authenticated release envelope whose digest occupies the V4 manifest slot.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendReleaseAttestationV4 {
        /// Exact V4 attestation schema.
        pub schema: String,
        /// Attestation layout version.
        pub version: u16,
        /// Immutable V4 subject approved by all roles.
        pub subject: KagemushaRecursiveSpendReleaseAttestationSubjectV4,
        /// Strictly ordered, unique role/signer approvals.
        pub approvals: Vec<KagemushaRecursiveSpendReleaseApprovalV4>,
    }

    /// Trusted signer threshold for one independent release-approval role.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendReleaseRolePolicyV1 {
        /// Role governed by this threshold.
        pub role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
        /// Number of distinct authorized signatures required for the role.
        pub threshold: u16,
        /// Strictly ordered authorized signer keys.
        pub authorized_signers: Vec<PublicKey>,
    }

    /// Locally trusted policy for authenticating a release envelope.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendReleasePolicyV1 {
        /// Exact policy schema.
        pub schema: String,
        /// Policy layout version.
        pub version: u16,
        /// Portable identifier selected by deployment policy.
        pub policy_id: String,
        /// Exactly release, cryptographic-review, and device-benchmark policies.
        pub roles: Vec<KagemushaRecursiveSpendReleaseRolePolicyV1>,
    }

    /// Verified signer identity retained in a machine-readable promotion record.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendApprovedSignerV1 {
        /// Approval role satisfied by this signer.
        pub role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
        /// Verified signer key.
        pub public_key: PublicKey,
    }

    /// Deterministic ABI-21 deployment marker written only after V4 release verification.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendPromotedReleaseV4 {
        /// Exact V4 promotion-record schema.
        pub schema: String,
        /// Promotion-record layout version.
        pub version: u16,
        /// Authenticated V4 release generation.
        pub generation: String,
        /// SHA-256 of the immutable pre-evidence candidate record.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub candidate_sha256: [u8; 32],
        /// SHA-256 of the complete canonical V4 manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub manifest_sha256: [u8; 32],
        /// SHA-256 of the canonical signed V4 release attestation.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub release_attestation_sha256: [u8; 32],
        /// SHA-256 of the locally trusted release policy.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub release_policy_sha256: [u8; 32],
        /// Canonically ordered role/signer identities whose signatures were verified.
        pub approved_signers: Vec<KagemushaRecursiveSpendApprovedSignerV1>,
        /// Whether every content-addressed V4 artifact was verified before publication.
        pub artifact_inventory_verified: bool,
        /// Native bridge ABI required to consume this promoted release.
        pub bridge_abi_version: u32,
        /// Exact Eq-then-Ep eight-role artifact inventory selected by ABI-21.
        pub artifact_roles: Vec<String>,
        /// Authenticated release-specific proof-pair byte ceiling.
        pub max_proof_bytes: u32,
    }

    /// Complete signed ABI-21 release material persisted by consensus activation.
    ///
    /// The two evidence fields contain canonical signed summaries, never raw
    /// device logs, parameters, proving keys, or bootstrap witness payloads.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendReleaseRecordV4 {
        /// Canonical evidence-bearing release manifest.
        pub manifest: KagemushaRecursiveSpendArtifactManifestV4,
        /// Role-threshold signatures over the finalized release subject.
        pub release_attestation: KagemushaRecursiveSpendReleaseAttestationV4,
        /// Canonical signed physical-device benchmark summary bytes.
        pub physical_device_benchmark_summary: Vec<u8>,
        /// Canonical signed independent cryptographic-review summary bytes.
        pub cryptographic_review_summary: Vec<u8>,
        /// Promotion marker binding the candidate, policy, release, and inventory.
        pub promotion_record: KagemushaRecursiveSpendPromotedReleaseV4,
    }

    /// Atomic consensus payload for one ABI-21 release and its two terminal verifiers.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendReleaseActivationV4 {
        /// Fully authenticated signed release record.
        pub release_record: KagemushaRecursiveSpendReleaseRecordV4,
        /// SHA-256 of the operator-configured canonical release policy.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub configured_policy_sha256: [u8; 32],
        /// Registry id for the EqAffine/Vesta verifying key.
        pub step_eq_verifier_key_id: VerifyingKeyId,
        /// Inline EqAffine/Vesta verifying-key record.
        pub step_eq_verifier_record: VerifyingKeyRecord,
        /// Registry id for the EpAffine/Pallas verifying key.
        pub step_ep_verifier_key_id: VerifyingKeyId,
        /// Inline EpAffine/Pallas verifying-key record.
        pub step_ep_verifier_record: VerifyingKeyRecord,
    }

    /// Installed authenticated V4 release selected by a degree-parameterized operation.
    ///
    /// The explicit wire version prevents an unversioned historical binding
    /// from being interpreted as an ABI-21 release identity.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendArtifactBindingV4 {
        /// Exact ABI-21 binding version. Only `4` is accepted.
        pub version: u16,
        /// Human-readable authenticated V4 release generation.
        pub generation: String,
        /// SHA-256 of the exact signed V4 manifest bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub manifest_sha256: [u8; 32],
    }

    /// Canonical fields signed by a receiver after durable payment persistence.
    ///
    /// The receiver must persist the final acknowledgement bytes under
    /// `(operation_id, recipient_request_digest)` in the same atomic operation
    /// that persists the accepted bundle. Duplicate delivery returns those
    /// exact bytes instead of signing a new timestamp.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaReceiverAcknowledgementPayloadV2 {
        /// Sender operation whose reserved inputs may be committed after ACK verification.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Canonical digest of the receiver-created payment request.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Canonical digest of the accepted recipient bundle.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub payment_bundle_digest: [u8; 32],
        /// Recipient output commitment persisted by the receiver.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recipient_commitment: [u8; 32],
        /// Receiver wall-clock time captured once at the durable commit boundary.
        pub accepted_at_ms: u64,
        /// Registered receiver device identifier used for device-lineage lookup.
        pub receiver_device_id: String,
        /// Domain-separated reference to `receiver_public_key`.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub receiver_key_reference: [u8; 32],
        /// Device-bound acknowledgement verification key.
        pub receiver_public_key: KagemushaDevicePublicKeyV2,
    }

    /// Signed durable receiver acknowledgement for one offline payment.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaReceiverAcknowledgementV2 {
        /// Canonical signed bindings.
        pub payload: KagemushaReceiverAcknowledgementPayloadV2,
        /// Device-key signature over the domain-separated canonical payload.
        pub signature: KagemushaDeviceSignatureV2,
    }

    /// Typed result returned after native acknowledgement verification.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaReceiverAcknowledgementVerifyResultV2 {
        /// All request, bundle, key-reference, and signature bindings passed.
        pub valid: bool,
        /// Stable sender operation id copied from the verified acknowledgement.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Canonical receiver request digest copied from the verified acknowledgement.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Canonical accepted-bundle digest copied from the verified acknowledgement.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub payment_bundle_digest: [u8; 32],
        /// Canonical identity digest of the complete acknowledgement.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub acknowledgement_digest: [u8; 32],
    }

    /// Native capability record for the explicitly versioned ABI-21 backend.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendNativeCapabilitiesV4 {
        /// Native bridge ABI reported by the loaded library.
        pub bridge_abi_version: u32,
        /// Required V4 artifact manifest schema.
        pub artifact_manifest_schema: String,
        /// Required V4 proof backend.
        pub proof_backend: String,
        /// Required V4 transcript profile.
        pub transcript_profile: String,
        /// Proof-envelope format version.
        pub proof_envelope_version: u16,
        /// Eq recursive-step circuit id.
        pub step_eq_circuit_id: String,
        /// Ep recursive-step circuit id.
        pub step_ep_circuit_id: String,
        /// Exact ordered eight-role cryptographic inventory.
        pub artifact_roles: Vec<String>,
        /// Maximum proof-pair payload accepted by the installed V4 release.
        pub max_proof_bytes: u32,
        /// Whether all proof, audit, release, and performance gates passed.
        pub proof_backend_available: bool,
        /// Stable remaining backend gates.
        pub missing_gates: Vec<String>,
    }

    /// Degree-parameterized Pasta-cycle envelope carried by a V4 proof wrapper.
    ///
    /// The backend-native Eq/Ep pair remains canonical opaque bytes inside
    /// `proof`; wallets and bridge carriers do not reinterpret its internal
    /// accumulators or fold transcripts.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPastaCycleProofEnvelopeV4 {
        /// Exact V4 proof-envelope version.
        pub version: u16,
        /// Exact V4 paired-proof backend.
        pub proof_backend: String,
        /// Exact V4 transcript profile.
        pub transcript_profile: String,
        /// Exact Eq recursive-step circuit id.
        pub step_eq_circuit_id: String,
        /// Exact Ep recursive-step circuit id.
        pub step_ep_circuit_id: String,
        /// Authenticated artifact generation.
        pub artifact_generation: String,
        /// SHA-256 of the exact authenticated V4 manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub manifest_sha256: [u8; 32],
        /// Eq parameter generation identifier.
        pub step_eq_parameter_generation: String,
        /// Ep parameter generation identifier.
        pub step_ep_parameter_generation: String,
        /// Domain-separated identity of the Eq circuit configuration.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub step_eq_circuit_params_sha256: [u8; 32],
        /// Domain-separated identity of the Ep circuit configuration.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub step_ep_circuit_params_sha256: [u8; 32],
        /// SHA-256 of the exact Eq processed verifier-key payload.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub step_eq_verifier_key_sha256: [u8; 32],
        /// SHA-256 of the exact Ep processed verifier-key payload.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub step_ep_verifier_key_sha256: [u8; 32],
        /// Canonical cross-field state boundary exposed by the proof.
        pub state_boundary: KagemushaRecursiveSpendStateBoundaryV2,
        /// Canonical adapter-owned V4 Eq/Ep proof-pair bytes.
        pub proof: ProofBox,
    }

    /// Exact fixed-size ABI-21 public operation row bound by the terminal proof.
    ///
    /// Each consecutive group of eight limbs is one canonical Pallas-field
    /// element in little-endian `u32` order. Core rejects non-canonical field
    /// encodings before proof verification.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendOperationVectorV4 {
        /// All 1,080 exact public limbs; no compact or legacy encoding is accepted.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_u32_limbs")
        )]
        pub limbs: [u32; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4],
    }

    /// Peer-to-peer split transition carried by an ABI-21 recursive output.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendPeerSplitTransitionV4 {
        /// Circuit-exposed digest of the exact local split intent.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub binding_digest: [u8; 32],
        /// Independently spendable output selected by this statement.
        pub branch: KagemushaRecursiveSpendBranchV2,
        /// Receiver request digest bound by the split.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Stable split operation identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Maximum proof-step count among the consumed parent bundles.
        pub parent_max_proof_step_count: u32,
        /// Maximum peer-hop count among the consumed parent bundles.
        pub parent_max_peer_hop_count: u32,
    }

    /// Partial-redemption change transition carried by an ABI-21 child statement.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedemptionChangeTransitionV4 {
        /// Circuit-exposed digest of the exact redemption/change intent.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub binding_digest: [u8; 32],
        /// Parent bundle identity consumed by the unshield transition.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub parent_bundle_digest: [u8; 32],
        /// Stable redemption operation identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Parent proof-step count.
        pub parent_proof_step_count: u32,
        /// Parent peer-hop count.
        pub parent_peer_hop_count: u32,
    }

    /// Mutually exclusive semantic transition that produced an ABI-21 state.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "transition", content = "value", rename_all = "snake_case")]
    pub enum KagemushaRecursiveSpendTransitionV4 {
        /// Ordinary offline peer split.
        PeerSplit(KagemushaRecursiveSpendPeerSplitTransitionV4),
        /// Proof-bound partial-redemption change child.
        RedemptionChange(KagemushaRecursiveSpendRedemptionChangeTransitionV4),
    }

    /// Canonical public statement bound by an ABI-21 recursive proof.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendPublicStatementV4 {
        /// Chain that scopes this cash state.
        pub chain_id: ChainId,
        /// Asset committed by every note in the transition.
        pub asset: AssetDefinitionId,
        /// Authoritative asset scale.
        pub asset_scale: u32,
        /// Root after the current transition.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub final_root: [u8; 32],
        /// First empty commitment-tree leaf after this transition.
        pub next_zero_leaf_index: u32,
        /// One or two canonical finalized top-up references funding this state.
        pub topup_anchor_refs: Vec<KagemushaRecursiveSpendTopUpAnchorRefV2>,
        /// Total recursive proof transitions.
        pub proof_step_count: u32,
        /// Number of peer-to-peer spends after top-up.
        pub peer_hop_count: u32,
        /// Current independently spendable note.
        pub current_note: KagemushaSpendableNoteDescriptorV2,
        /// Transition-bound conflict claims.
        pub branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Binding-only semantic transition under the sole ABI-21 wire layout.
        pub transition: Option<KagemushaRecursiveSpendTransitionV4>,
        /// Authenticated V4 proving-artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Recursive verifier selected by the proof.
        pub verifier_key_id: VerifyingKeyId,
    }

    /// V4 recursive proof whose public instance includes the statement digest.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendProofV4 {
        /// Verifier selected by the statement.
        pub verifier_key_id: VerifyingKeyId,
        /// Circuit-exposed digest of the complete V4 public statement.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub public_statement_digest: [u8; 32],
        /// Explicitly versioned envelope containing the opaque native pair.
        pub proof_envelope: KagemushaPastaCycleProofEnvelopeV4,
    }

    /// Independently spendable ABI-21 recursive state.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendBundleV4 {
        /// Exact public statement bound by the proof.
        pub statement: KagemushaRecursiveSpendPublicStatementV4,
        /// Canonical public operation row independently carried by this bundle.
        pub operation: KagemushaRecursiveSpendOperationVectorV4,
        /// Degree-parameterized recursive proof.
        pub recursive_proof: KagemushaRecursiveSpendProofV4,
    }

    /// Finalized top-up anchor selecting a V4 recursive release.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendTopUpAnchorV4 {
        /// Anchor schema version.
        pub version: u16,
        /// Chain that finalized the top-up.
        pub chain_id: ChainId,
        /// Payer whose online balance funded the anchor.
        pub payer: AccountId,
        /// Exact payer asset, including its balance scope.
        pub asset: AssetId,
        /// Authoritative fixed scale.
        pub asset_scale: u32,
        /// Exact positive amount reserved into escrow.
        pub amount: KagemushaScaledAmountV2,
        /// Confidential root before the finalized transfer.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub initial_root: [u8; 32],
        /// Confidential root finalized by the transfer.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub finalized_root: [u8; 32],
        /// Confidential tree position consumed by the top-up note.
        pub shield_leaf_index: u32,
        /// Exact first spendable note.
        pub current_note: KagemushaSpendableNoteDescriptorV2,
        /// Stable top-up operation identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub topup_operation_id: [u8; 32],
        /// Active shield verifier selected at finalization.
        pub shield_verifier_id: VerifyingKeyId,
        /// Registered shield verifier commitment.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub shield_verifier_commitment: [u8; 32],
        /// Authenticated V4 recursive artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Finalization block height.
        pub finalized_height: u64,
        /// Canonical transaction hash that created the anchor.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub finalized_tx_hash: [u8; 32],
        /// Canonical digest of every preceding receipt field.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub anchor_digest: [u8; 32],
    }

    /// Canonical unsigned ABI-21 online-to-offline fields covered by payer authorization.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendTopUpUnsignedV4 {
        /// Exact chain-request wire version. Only `4` is accepted.
        pub version: u16,
        /// Online asset balance charged for the top-up.
        pub asset: AssetId,
        /// Exact positive amount charged at the live asset-definition scale.
        pub amount: KagemushaScaledAmountV2,
        /// First spendable note produced by the shield transition.
        pub current_note: KagemushaSpendableNoteDescriptorV2,
        /// Proof that inserts this note without consuming a confidential input.
        pub shield_evidence: KagemushaTopUpShieldEvidenceV2,
        /// Authenticated ABI-21 release selected for recursive initialization.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Globally unique replay-stable operation identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Authoritative ABI-21 chain-facing online-to-offline request.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(schema_name = "iroha.torii.v1.offline.top_up.request")]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendTopUpRequestV4 {
        /// Exact chain-request wire version. Only `4` is accepted.
        pub version: u16,
        /// Online asset balance charged for the top-up.
        pub asset: AssetId,
        /// Exact positive amount charged at the live asset-definition scale.
        pub amount: KagemushaScaledAmountV2,
        /// First spendable note produced by the shield transition.
        pub current_note: KagemushaSpendableNoteDescriptorV2,
        /// Proof that inserts this note without consuming a confidential input.
        pub shield_evidence: KagemushaTopUpShieldEvidenceV2,
        /// Authenticated ABI-21 release selected for recursive initialization.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Globally unique replay-stable operation identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Self-contained payer/device authorization.
        pub authorization: KagemushaRequestAuthorizationV2,
    }

    /// Public V4 split transition with an ABI-21 output binding.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendSplitIntentV4 {
        /// Chain inherited from the parent state and receiver request.
        pub chain_id: ChainId,
        /// Asset inherited from the parent state and receiver request.
        pub asset: AssetDefinitionId,
        /// One or two canonical previous branches.
        pub inputs: Vec<KagemushaRecursiveSpendInputBranchV2>,
        /// Canonical finalized top-up references contributing value.
        pub topup_anchor_refs: Vec<KagemushaRecursiveSpendTopUpAnchorRefV2>,
        /// Authoritative asset-definition scale.
        pub asset_scale: u32,
        /// Authenticated V4 release selected for the output proof.
        pub output_artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Exact amount assigned to the recipient output.
        pub transfer_amount: KagemushaScaledAmountV2,
        /// Recipient-owned output note.
        pub recipient_output: KagemushaSpendableNoteDescriptorV2,
        /// Sender-owned remainder, if any.
        pub change_output: Option<KagemushaSpendableNoteDescriptorV2>,
        /// Digest of the receiver's nonce-bound payment request.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Stable idempotency/replay identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Public V4 redemption transition with an optional ABI-21 change binding.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedemptionIntentV4 {
        /// Chain inherited from the input bundle.
        pub chain_id: ChainId,
        /// Asset inherited from the input bundle.
        pub asset: AssetDefinitionId,
        /// Exact note consumed by unshield-v3.
        pub input_note: KagemushaSpendableNoteDescriptorV2,
        /// Canonical live conflict claims consumed by this redemption.
        pub parent_branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Canonical finalized top-up references carried by the parent.
        pub parent_topup_anchor_refs: Vec<KagemushaRecursiveSpendTopUpAnchorRefV2>,
        /// Recursive proof-step count of the parent bundle.
        pub parent_proof_step_count: u32,
        /// Peer-hop count of the parent bundle.
        pub parent_peer_hop_count: u32,
        /// Canonical digest of the complete input bundle.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub parent_bundle_digest: [u8; 32],
        /// Input confidential root exposed by unshield-v3.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub input_root: [u8; 32],
        /// Online account receiving the public credit.
        pub recipient: AccountId,
        /// Exact credited amount at the authoritative scale.
        pub public_amount: KagemushaScaledAmountV2,
        /// Proof-bound change descriptor.
        pub change_output: Option<KagemushaSpendableNoteDescriptorV2>,
        /// Authenticated V4 output release, present exactly with change.
        pub change_artifact_binding: Option<KagemushaRecursiveSpendArtifactBindingV4>,
        /// Canonical unshield-v3 public words.
        pub unshield_public_inputs: KagemushaUnshieldPublicInputsBindingV2,
        /// Digest of the unshield words exposed by the V4 transition circuit.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub unshield_public_inputs_digest: [u8; 32],
        /// Stable authorization/idempotency operation id.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// ABI-21 local initialization request.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendInitRequestV4 {
        /// Finalized chain receipt consumed by the initial proof.
        pub topup_anchor: KagemushaRecursiveSpendTopUpAnchorV4,
        /// Offline-verifiable finality proof for the compact anchor reference.
        pub topup_finality_proof: KagemushaTopUpFinalityProofV2,
        /// Exact content-addressed validator roster.
        pub topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactV2,
        /// Authenticated V4 artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
    }

    /// ABI-21 initialization result.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendInitResultV4 {
        /// Independently spendable state created from the finalized top-up.
        pub bundle: KagemushaRecursiveSpendBundleV4,
        /// Proof-bound membership state and next-zero frontier for the initialized note.
        pub membership_witness: KagemushaNoteMembershipWitnessV2,
        /// Complete offline-verifiable origin provenance for the initialized branch.
        pub topup_provenance: KagemushaRecursiveSpendTopUpProvenanceV4,
        /// Circuit-exposed digest of the complete public statement.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub public_statement_digest: [u8; 32],
    }

    /// One V4 previous-proof package consumed by append.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendAppendInputV4 {
        /// Previous spendable ABI-21 recursive state.
        pub previous_bundle: KagemushaRecursiveSpendBundleV4,
        /// Complete authenticated top-up provenance required to verify this parent offline.
        pub topup_provenance: KagemushaRecursiveSpendTopUpProvenanceV4,
    }

    /// ABI-21 recursive append request.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendAppendRequestV4 {
        /// One or two previous-proof packages in canonical order.
        pub previous_inputs: Vec<KagemushaRecursiveSpendAppendInputV4>,
        /// Confidential-transfer proof containing both output commitments.
        pub confidential_transfer_proof: ProofAttachment,
        /// Scale, outputs, replay id, and V4 output release.
        pub split: KagemushaRecursiveSpendSplitIntentV4,
        /// Signed proof-evaluation snapshot; verifiers must also be live at execution.
        pub block_height: u64,
    }

    /// Result of one ABI-21 recursive split append.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendSplitResultV4 {
        /// Exact value-conserving transition shared by both branches.
        pub split: KagemushaRecursiveSpendSplitIntentV4,
        /// Circuit-exposed binding to the split and parent accumulator.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub split_binding_digest: [u8; 32],
        /// Receiver-owned independently spendable output.
        pub recipient_bundle: KagemushaRecursiveSpendBundleV4,
        /// Proof-bound membership state for the recipient output.
        pub recipient_membership_witness: KagemushaNoteMembershipWitnessV2,
        /// Complete offline-verifiable provenance for the recipient branch.
        pub recipient_topup_provenance: KagemushaRecursiveSpendTopUpProvenanceV4,
        /// Sender-owned remainder, present only for a partial transfer.
        pub change_bundle: Option<KagemushaRecursiveSpendBundleV4>,
        /// Proof-bound membership state for sender change.
        pub change_membership_witness: Option<KagemushaNoteMembershipWitnessV2>,
        /// Complete provenance for sender change, present exactly with change.
        pub change_topup_provenance: Option<KagemushaRecursiveSpendTopUpProvenanceV4>,
    }

    /// Recipient-only ABI-21 peer payload emitted from a local split result.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendPeerPaymentV4 {
        /// Receiver-owned independently spendable ABI-21 branch.
        pub recipient_bundle: KagemushaRecursiveSpendBundleV4,
        /// Proof-bound membership state required for the recipient's next spend.
        pub recipient_membership_witness: KagemushaNoteMembershipWitnessV2,
        /// Complete authenticated provenance needed for offline receiver verification.
        pub topup_provenance: KagemushaRecursiveSpendTopUpProvenanceV4,
    }

    /// Complete finalized V4 origin carried to an offline receiver.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
        /// Complete finalized ABI-21 top-up receipt.
        pub topup_anchor: KagemushaRecursiveSpendTopUpAnchorV4,
        /// Consensus proof for the compact anchor reference.
        pub topup_finality_proof: KagemushaTopUpFinalityProofV2,
    }

    /// Complete authenticated top-up provenance carried by every spendable ABI-21 branch.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendTopUpProvenanceV4 {
        /// Exact manifest-bound validator roster shared by every origin proof.
        pub topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactV2,
        /// Complete evidence in the exact order of the branch statement's anchor references.
        pub topup_finality_evidence: Vec<KagemushaRecursiveSpendTopUpFinalityEvidenceV4>,
    }

    /// ABI-21 receiver-verification request.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendVerifyRequestV4 {
        /// Scale- and split-bound ABI-21 recursive bundle.
        pub bundle: KagemushaRecursiveSpendBundleV4,
        /// Receiver request that the final branch must match.
        pub recipient_request: KagemushaRecipientPaymentRequestV2,
        /// Complete branch provenance received with the peer payment.
        pub topup_provenance: KagemushaRecursiveSpendTopUpProvenanceV4,
        /// Maximum hop count accepted by the receiver.
        pub maximum_hops: u32,
        /// Expected authenticated V4 artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Signed proof-evaluation snapshot; verifiers must also be live at receipt time.
        pub block_height: u64,
        /// Authoritative current Unix time in milliseconds.
        pub verified_at_ms: u64,
    }

    /// Opaque-safe summary decoded from an ABI-21 bundle.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendBundleSummaryV4 {
        /// Asset definition bound by the proof.
        pub asset: AssetDefinitionId,
        /// Exact current spendable amount.
        pub amount: KagemushaScaledAmountV2,
        /// Current note commitment.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub note_commitment: [u8; 32],
        /// Current note nullifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub spend_nullifier: [u8; 32],
        /// Current peer-hop count.
        pub hop_count: u32,
        /// Current recursive transition count.
        pub proof_step_count: u32,
        /// Canonical transition-bound conflict claims.
        pub branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Authenticated V4 artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Recursive verifier selected by the proof.
        pub verifier_key_id: VerifyingKeyId,
        /// Canonical identity digest of the complete opaque bundle.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub bundle_digest: [u8; 32],
    }

    /// Typed ABI-21 receiver-verification result.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendVerifyResultV4 {
        /// Cryptographic proof and all public bindings verified.
        pub valid: bool,
        /// Bundle satisfies current chain admission rules.
        pub chain_admissible: bool,
        /// Persisted lineage material can be redeemed.
        pub lineage_redeemable: bool,
        /// Chain supports redemption without a record-backed witness.
        pub witnessless_redemption_supported: bool,
        /// Verified ABI-21 bundle summary.
        pub summary: KagemushaRecursiveSpendBundleSummaryV4,
        /// Canonical receiver request digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Digest binding request, output, and bundle.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub request_output_binding_digest: [u8; 32],
        /// Active recursive verifier record identifier.
        pub verifier_key_id: VerifyingKeyId,
        /// Active V4 recursive circuit id.
        pub verifier_circuit_id: String,
        /// Inclusive verifier activation height.
        pub verifier_activation_height: Option<u64>,
        /// Exclusive verifier withdrawal height.
        pub verifier_withdraw_height: Option<u64>,
        /// Height used for activation-window verification.
        pub verified_at_block_height: u64,
        /// Authoritative Unix time used for acceptance.
        pub verified_at_ms: u64,
    }

    /// Proof-bound V4 offline change child created by partial redemption.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemChangeBranchV4 {
        /// Exact change descriptor exposed by unshield-v3.
        pub output: KagemushaSpendableNoteDescriptorV2,
        /// Deterministic transition-bound change claims.
        pub branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Recursive proof making the child independently spendable.
        pub bundle: KagemushaRecursiveSpendBundleV4,
    }

    /// ABI-21 native redemption builder input.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemBuildRequestV4 {
        /// Spendable recursive state being redeemed.
        pub bundle: KagemushaRecursiveSpendBundleV4,
        /// Online account credited by redemption.
        pub recipient: AccountId,
        /// Exact public amount and authoritative asset scale.
        pub public_amount: KagemushaScaledAmountV2,
        /// Unshield-v3 proof for credit and optional change.
        pub unshield_proof: ProofAttachment,
        /// Exact public V4 redemption transition.
        pub redemption: KagemushaRecursiveSpendRedemptionIntentV4,
        /// Signed proof-evaluation snapshot, bounded by the eventual execution height.
        pub block_height: u64,
        /// Stable idempotency identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Canonical unsigned ABI-21 chain redemption fields.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemUnsignedV4 {
        /// Exact chain-request wire version. Only `4` is accepted.
        pub version: u16,
        /// Scale-carrying recursive state being redeemed.
        pub bundle: KagemushaRecursiveSpendBundleV4,
        /// Online account credited by redemption.
        pub recipient: AccountId,
        /// Exact public amount and authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
        /// Unshield-v3 proof bound to the current note and optional change.
        pub redeem_proof: ProofAttachment,
        /// Canonical public V4 redemption intent.
        pub redemption: KagemushaRecursiveSpendRedemptionIntentV4,
        /// All-or-none proof-bound V4 change child.
        pub offline_change: Option<KagemushaRecursiveSpendRedeemChangeBranchV4>,
        /// Signed proof-evaluation snapshot, bounded by the eventual execution height.
        pub block_height: u64,
        /// Stable idempotency identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Prepared unsigned ABI-21 redemption result.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemBuildResultV4 {
        /// Complete unsigned V4 chain-request fields.
        pub unsigned: KagemushaRecursiveSpendRedeemUnsignedV4,
        /// Exact digest that device authorization must sign.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub authorization_digest: [u8; 32],
        /// Independently spendable proof-bound change.
        pub offline_change_bundle: Option<KagemushaRecursiveSpendBundleV4>,
        /// Membership state for proof-bound change.
        pub offline_change_membership_witness: Option<KagemushaNoteMembershipWitnessV2>,
        /// Complete inherited origin provenance for proof-bound change.
        pub offline_change_topup_provenance: Option<KagemushaRecursiveSpendTopUpProvenanceV4>,
        /// Stable operation identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Versioned ABI-21 offline-to-online request.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(schema_name = "iroha.torii.v1.offline.redeem.request")]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendRedeemRequestV4 {
        /// Exact chain-request wire version. Only `4` is accepted.
        pub version: u16,
        /// Scale-carrying recursive state being redeemed.
        pub bundle: KagemushaRecursiveSpendBundleV4,
        /// Online account credited by redemption.
        pub recipient: AccountId,
        /// Exact public amount and authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
        /// Unshield-v3 proof bound to the current note and optional change.
        pub redeem_proof: ProofAttachment,
        /// Canonical public V4 redemption intent.
        pub redemption: KagemushaRecursiveSpendRedemptionIntentV4,
        /// All-or-none proof-bound V4 change child.
        pub offline_change: Option<KagemushaRecursiveSpendRedeemChangeBranchV4>,
        /// Signed proof-evaluation snapshot; chain execution also checks the current window.
        pub block_height: u64,
        /// Globally unique idempotency identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Self-contained recipient/device authorization.
        pub authorization: KagemushaRequestAuthorizationV2,
    }

    /// Typed native ABI-21 redemption output.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemResultV4 {
        /// Exact ABI-21 result wire version. Only `4` is accepted.
        pub version: u16,
        /// Canonical `KagemushaRecursiveSpendRedeemRequestV4` archive.
        pub redeem_request_archive: Vec<u8>,
        /// Proof-bound offline change branch.
        pub offline_change_bundle: Option<KagemushaRecursiveSpendBundleV4>,
        /// Membership state for proof-bound change.
        pub offline_change_membership_witness: Option<KagemushaNoteMembershipWitnessV2>,
        /// Complete inherited origin provenance for proof-bound change.
        pub offline_change_topup_provenance: Option<KagemushaRecursiveSpendTopUpProvenanceV4>,
        /// Stable operation identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }
}

/// Return the release-qualified verifier-key registry identifier for one ABI-21 parity.
///
/// The manifest digest suffix keeps verifier records for overlapping retained
/// releases distinct while preserving the fixed Eq/Ep circuit identity inside
/// each [`VerifyingKeyRecord`].
#[must_use]
pub fn kagemusha_recursive_spend_verifier_key_id_v4(
    parity: KagemushaPastaCycleParityV1,
    manifest_sha256: [u8; 32],
) -> VerifyingKeyId {
    let circuit_id = match parity {
        KagemushaPastaCycleParityV1::StepEq => KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
        KagemushaPastaCycleParityV1::StepEp => KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
    };
    VerifyingKeyId::new(
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        format!("{circuit_id}-{}", hex::encode(manifest_sha256)),
    )
}

/// On-chain platform-attested registration for a Kagemusha device key.
///
/// This is the device-bound trust anchor used by top-up and redemption authorization.
/// The report and evidence bytes are included so consensus has enough material to
/// perform deterministic platform checks; the hashes provide stable replay keys and
/// compact audit anchors.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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

/// Governed Offline device-attestation verifier policy.
///
/// Nodes require this policy to be installed in chain state before accepting
/// hardware-backed offline registration or transaction authorization. The
/// first-release platform roots are accepted only when included in that
/// explicit governed policy; absence of policy state fails closed. Operators
/// can rotate roots, publish deterministic revocations, and restrict accepted
/// app identities without relying on external middleware state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineDeviceAttestationPolicy {
    /// Policy format marker.
    pub version: u16,
    /// Trusted platform roots accepted by the on-chain verifier.
    pub trusted_roots: Vec<OfflineDeviceAttestationTrustedRoot>,
    /// SHA-256 digests of revoked certificate DER payloads.
    pub revoked_certificate_sha256: Vec<Vec<u8>>,
    /// Accepted iOS App Attest app identities.
    pub ios_apps: Vec<OfflineIosAppAttestationPolicy>,
    /// Accepted Android `KeyMint` app identities.
    pub android_apps: Vec<OfflineAndroidAppAttestationPolicy>,
    /// Explicitly enables iOS registration and online assertions when a matching
    /// entry exists in `ios_apps`.
    ///
    /// iOS App Attest is disabled when this is false; there is no implicit app
    /// identity or legacy-authData fallback.
    pub require_ios_app_policy: bool,
    /// Explicitly enables Android registration when a matching entry exists in `android_apps`.
    ///
    /// Android `KeyMint` is disabled when this is false; there is no implicit
    /// unlisted-package or signing-certificate fallback.
    pub require_android_app_policy: bool,
}

/// Trusted platform root certificate for Offline device attestation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineDeviceAttestationTrustedRoot {
    /// Platform class, for example `ios-appattest` or `android-keymint`.
    pub platform: String,
    /// Root certificate DER bytes.
    pub der: Vec<u8>,
    /// Optional governance activation time in Unix milliseconds.
    pub not_before_ms: Option<u64>,
    /// Optional governance expiry time in Unix milliseconds.
    pub not_after_ms: Option<u64>,
}

/// Allowed iOS App Attest app identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineIosAppAttestationPolicy {
    /// Apple App ID prefix (normally the Apple Developer Team ID).
    pub team_id: String,
    /// iOS bundle identifier.
    pub bundle_id: String,
    /// App Attest environment, either `production` or `development`.
    pub environment: String,
    /// Allowed Apple validation categories from extension-bearing App Attest data.
    pub allowed_validation_categories: Vec<u32>,
    /// Allowed application bundle versions from extension-bearing App Attest data.
    pub allowed_bundle_versions: Vec<String>,
    /// Whether legacy App Attest attestation and assertion authData without extensions remains accepted.
    pub allow_legacy_auth_data_without_extensions: bool,
}

/// Allowed Android `KeyMint` app identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineAndroidAppAttestationPolicy {
    /// Android package name.
    pub package_name: String,
    /// Allowed Android signing certificate SHA-256 digests.
    pub signing_certificate_sha256: Vec<Vec<u8>>,
}

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
    fn serialize<W: std::io::Write>(&self, mut writer: W) -> Result<(), norito::Error> {
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
    fn serialize<W: std::io::Write>(&self, mut writer: W) -> Result<(), norito::Error> {
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
    /// The preimage intentionally excludes the attestation report, evidence
    /// hashes, and assertion public key because those values are learned from
    /// the platform response after the challenge is created. Android `KeyMint`
    /// additionally uses a platform-specific preimage without `key_id`, because
    /// its canonical key id is the SHA-256 of that not-yet-generated assertion
    /// public key. Admission binds the reported credential/certificate public
    /// key to `assertion_public_key` and then validates `key_id` before
    /// constructing the key certificate.
    ///
    /// # Errors
    ///
    /// Returns an error when the challenge preimage cannot be serialized with Norito.
    pub fn canonical_challenge_hash(&self) -> Result<Hash, norito::Error> {
        if self.platform == OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_PLATFORM {
            return to_bytes(&self.android_keymint_challenge_preimage()).map(Hash::new);
        }
        to_bytes(&self.challenge_preimage()).map(Hash::new)
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
        to_bytes(&self.challenge_preimage())
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

    /// Return whether two claims select overlapping value or incompatible
    /// transition histories.
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
    /// Native implementations additionally decode the opaque prover material
    /// before returning it and enforce that its schema contains no receiver
    /// spend secret or output diversifier.
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
        if self.recipient_output.chain_id != request.chain_id {
            return Err(KagemushaValidationError::RecursiveSpendChainMismatch);
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
        if self.recipient_output.chain_id != self.chain_id {
            return Err(KagemushaValidationError::RecursiveSpendChainMismatch);
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
        Ok(to_bytes(
            &KagemushaRecipientPaymentRequestSigningPreimageV2 {
                domain: KAGEMUSHA_RECIPIENT_PAYMENT_REQUEST_SIGNING_DOMAIN_V2.to_owned(),
                payload: self.clone(),
            },
        )?)
    }
}

impl KagemushaRecipientPaymentRequestV2 {
    /// Chain namespace for the requested offline note.
    #[must_use]
    pub fn chain_id(&self) -> &ChainId {
        &self.chain_id
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
            chain_id: payload.chain_id,
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
            chain_id: self.chain_id.clone(),
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
        let encoded_len = to_bytes(self)?.len();
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
        Ok(to_bytes(&KagemushaRequestAuthorizationSigningPreimageV2 {
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
        })?)
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
    /// Consensus verifies the signature only after resolving
    /// `registration_hash` to the exact validated registration and its P-256
    /// assertion public key.
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
                    && assertion.authenticator_data[32] & !0x80 == 0
                    && ((assertion.authenticator_data[32] & 0x80 == 0
                        && assertion.authenticator_data.len()
                            == KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_BYTES_V1)
                        || (assertion.authenticator_data[32] & 0x80 != 0
                            && assertion.authenticator_data.len()
                                > KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_BYTES_V1)) =>
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
        if now_ms < self.issued_at_ms || now_ms > self.expires_at_ms {
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
    /// Returns [`KagemushaValidationError`] when the amount is non-positive, wider
    /// than `u128`, has more precision than the asset, or overflows while being
    /// normalized to the asset scale.
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
    #[must_use]
    pub fn public_quantity(self) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(self.atomic_units, self.scale))
            .expect("a u128 scaled amount is always a non-negative quantity")
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
    /// Native verification must additionally recompute both Poseidon paths:
    /// the real path from the proof-bound note commitment and the dummy path
    /// from the canonical empty leaf.
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

impl KagemushaRecursiveSpendStateBoundaryV2 {
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
        // the common live/bootstrap selector remains the final (64th) cell so
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
            || phase_count != self.num_lookup_advice_per_phase.len()
            || self.num_advice_per_phase.iter().any(|count| *count != 8)
            || self
                .num_lookup_advice_per_phase
                .iter()
                .any(|count| *count != 1)
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
        let encoded =
            to_bytes(self).map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.circuit_params.encoding",
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
            self.ignored_cargo_lock_sha256,
            self.combined_source_fingerprint_sha256,
        ]
        .into_iter()
        .all(|digest| digest != [0; 32]);
        if self.schema != KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_SCHEMA_V1
            || !is_kagemusha_source_commit(&self.base_commit)
            || self.base_commit != self.source_commit
            || !self.source_repo_dirty
            || !nonzero_digests
            || untracked_count != Some(self.untracked_path_mode_blob_oid_manifest.len())
            || untracked_count.is_none_or(|count| {
                count > KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_UNTRACKED_FILES_V1
            })
            || self.ignored_cargo_lock_size_bytes == 0
            || self.ignored_cargo_lock_size_bytes
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
        if combined_sha256 != self.combined_source_fingerprint_sha256 || !derived_dirty {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.reviewed_source_closure.fingerprint",
            });
        }
        Ok(())
    }

    fn canonical_descriptor_bytes(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        self.validate()?;
        let mut out = String::new();
        out.push_str("{\"base_commit\":");
        append_python_ascii_json_string(&mut out, &self.base_commit);
        out.push_str(",\"combined_source_fingerprint_sha256\":\"");
        out.push_str(&hex::encode(self.combined_source_fingerprint_sha256));
        out.push_str("\",\"ignored_cargo_lock_sha256\":\"");
        out.push_str(&hex::encode(self.ignored_cargo_lock_sha256));
        out.push_str("\",\"ignored_cargo_lock_size_bytes\":");
        out.push_str(&self.ignored_cargo_lock_size_bytes.to_string());
        out.push_str(",\"schema\":");
        append_python_ascii_json_string(&mut out, &self.schema);
        out.push_str(",\"source_commit\":");
        append_python_ascii_json_string(&mut out, &self.source_commit);
        out.push_str(",\"source_repo_dirty\":true,\"source_tree_sha256\":\"");
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

impl KagemushaRecursiveSpendArtifactManifestV4 {
    /// Validate the complete, explicitly versioned V4 release shape.
    ///
    /// This validates content binding only. A V4 release attestation must be
    /// authenticated separately before any artifact is used.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        self.validate_with_attestation_state(true)
    }

    /// Validate an immutable V4 release candidate before its attestation exists.
    ///
    /// Candidate manifests precede external evidence, so benchmark, review,
    /// and attestation digests must all remain zero. They are not valid release
    /// manifests and must never be accepted by production artifact readers.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_unsigned_candidate(&self) -> Result<(), KagemushaValidationError> {
        self.validate_with_attestation_state(false)
    }

    /// Return the SHA-256 identity of the canonical finalized V4 manifest.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn canonical_sha256(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        Ok(Sha256::digest(to_bytes(self)?).into())
    }

    /// Reconstruct the byte-exact immutable candidate that preceded this finalized manifest.
    ///
    /// Finalization fills only the two evidence digests and the release-attestation
    /// digest. Clearing exactly those fields must therefore recover a valid, clean
    /// candidate; a closure mismatch or otherwise invalid finalized manifest fails closed.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn immutable_candidate(
        &self,
    ) -> Result<KagemushaRecursiveSpendCandidateV4, KagemushaValidationError> {
        self.validate()?;
        let mut manifest = self.clone();
        manifest.benchmark_evidence_sha256 = [0; 32];
        manifest.cryptographic_review_sha256 = [0; 32];
        manifest.release_attestation_sha256 = [0; 32];
        let candidate = KagemushaRecursiveSpendCandidateV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4,
            manifest,
        };
        candidate.validate()?;
        Ok(candidate)
    }

    fn validate_with_attestation_state(
        &self,
        finalized: bool,
    ) -> Result<(), KagemushaValidationError> {
        let measured_step_bytes = self.profiles.iter().try_fold(0_u32, |sum, profile| {
            sum.checked_add(profile.step_proof_size_bytes)
        });
        let reviewed_source_closure_valid = self.reviewed_source_closure.validate().is_ok()
            && self.reviewed_source_closure.source_commit == self.source_commit
            && self.reviewed_source_closure.source_tree_sha256 == self.source_tree_sha256
            && self.reviewed_source_closure.source_repo_dirty == self.source_repo_dirty
            && self
                .reviewed_source_closure
                .canonical_descriptor_sha256()
                .is_ok_and(|sha256| sha256 == self.reviewed_source_closure_descriptor_sha256);
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4
            || self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4
            || !is_kagemusha_portable_identifier(&self.generation)
            || !is_kagemusha_source_commit(&self.source_commit)
            || self.source_tree_sha256 == [0; 32]
            || !self.source_repo_dirty
            || !reviewed_source_closure_valid
            || !is_kagemusha_chain_id(&self.chain_id)
            || self.asset_scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
            || self.activation_height == 0
            || self.withdrawal_height <= self.activation_height
            || self.max_proof_bytes == 0
            || self.max_proof_bytes > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
            || measured_step_bytes.is_none_or(|minimum| self.max_proof_bytes <= minimum)
            || self.profiles.len() != 2
            || self.profiles[0].parity != KagemushaPastaCycleParityV1::StepEq
            || self.profiles[1].parity != KagemushaPastaCycleParityV1::StepEp
            || self.topup_finality_roster_artifact.artifact_generation != self.generation
            || (finalized && self.benchmark_evidence_sha256 == [0; 32])
            || (finalized && self.cryptographic_review_sha256 == [0; 32])
            || (finalized && self.release_attestation_sha256 == [0; 32])
            || (!finalized && self.benchmark_evidence_sha256 != [0; 32])
            || (!finalized && self.cryptographic_review_sha256 != [0; 32])
            || (!finalized && self.release_attestation_sha256 != [0; 32])
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.artifact_manifest",
            });
        }
        self.topup_finality_roster_artifact.validate()?;
        let mut names = std::collections::BTreeSet::new();
        let mut digests = std::collections::BTreeSet::new();
        names.insert(
            self.topup_finality_roster_artifact
                .file_name
                .to_ascii_lowercase(),
        );
        digests.insert(self.topup_finality_roster_artifact.sha256);
        let mut structure_digests = std::collections::BTreeSet::new();
        for profile in &self.profiles {
            profile.validate()?;
            let _ = profile.circuit_params_sha256()?;
            if !structure_digests.insert(profile.compiled_protocol_structure_sha256) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.artifact_manifest.profile_identity",
                });
            }
            for artifact in &profile.artifacts {
                let name_is_new = names.insert(artifact.file_name.to_ascii_lowercase());
                let framed_digest_is_new = digests.insert(artifact.sha256);
                let payload_digest_is_new = digests.insert(artifact.payload_sha256);
                if !name_is_new || !framed_digest_is_new || !payload_digest_is_new {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "pasta_cycle.v4.artifact_manifest.artifact_identity",
                    });
                }
            }
        }
        if finalized {
            for evidence_digest in [
                self.benchmark_evidence_sha256,
                self.cryptographic_review_sha256,
            ] {
                if !digests.insert(evidence_digest) {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "pasta_cycle.v4.artifact_manifest.evidence_sha256",
                    });
                }
            }
            if !digests.insert(self.release_attestation_sha256) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.artifact_manifest.evidence_sha256",
                });
            }
        }
        Ok(())
    }

    /// Build the non-circular V4 subject signed by every release authority.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when the signing subject is invalid or cannot be encoded canonically.
    pub fn release_attestation_subject(
        &self,
    ) -> Result<KagemushaRecursiveSpendReleaseAttestationSubjectV4, KagemushaReleaseVerificationError>
    {
        self.validate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        self.release_attestation_subject_from_validated_manifest()
    }

    fn release_attestation_subject_from_validated_manifest(
        &self,
    ) -> Result<KagemushaRecursiveSpendReleaseAttestationSubjectV4, KagemushaReleaseVerificationError>
    {
        let mut subject_manifest = self.clone();
        subject_manifest.release_attestation_sha256 = [0; 32];
        let subject_bytes = to_bytes(&subject_manifest)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        Ok(KagemushaRecursiveSpendReleaseAttestationSubjectV4 {
            manifest_subject_sha256: Sha256::digest(subject_bytes).into(),
            generation: self.generation.clone(),
            source_commit: self.source_commit.clone(),
            source_tree_sha256: self.source_tree_sha256,
            source_repo_dirty: self.source_repo_dirty,
            reviewed_source_closure_descriptor_sha256: self
                .reviewed_source_closure_descriptor_sha256,
            benchmark_evidence_sha256: self.benchmark_evidence_sha256,
            cryptographic_review_sha256: self.cryptographic_review_sha256,
        })
    }
}

impl KagemushaRecursiveSpendCandidateV4 {
    /// Validate the reviewed-source-closure-bound pre-evidence candidate contract.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.candidate",
            });
        }
        self.manifest.validate_unsigned_candidate()
    }

    /// Return the SHA-256 identity of the canonical candidate record.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn sha256(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        Ok(Sha256::digest(to_bytes(self)?).into())
    }

    /// Build the exact candidate-bound subject signed by cryptographic reviewers.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the signing subject is invalid or cannot be encoded canonically.
    pub fn cryptographic_review_subject(
        &self,
    ) -> Result<KagemushaRecursiveSpendCryptographicReviewSubjectV4, KagemushaValidationError> {
        let candidate_sha256 = self.sha256()?;
        Ok(KagemushaRecursiveSpendCryptographicReviewSubjectV4 {
            candidate_sha256,
            generation: self.manifest.generation.clone(),
            source_commit: self.manifest.source_commit.clone(),
            source_tree_sha256: self.manifest.source_tree_sha256,
            source_repo_dirty: self.manifest.source_repo_dirty,
            reviewed_source_closure_descriptor_sha256: self
                .manifest
                .reviewed_source_closure_descriptor_sha256,
            chain_id: self.manifest.chain_id.clone(),
            asset: self.manifest.asset.clone(),
            bridge_abi_version: self.manifest.bridge_abi_version,
        })
    }
}

impl KagemushaRecursiveSpendCryptographicReviewCheckV4 {
    /// Exact canonical check order required by every production V4 review.
    pub const ALL: [Self; KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_CHECK_COUNT_V4] = [
        Self::RecursiveCircuitConstraintCoverage,
        Self::RecursiveCycleAndTranscriptBinding,
        Self::PublicInputAndStateTransitionBinding,
        Self::ArtifactParameterAndVerifyingKeyBinding,
        Self::NullifierReplayAndFinalityBinding,
        Self::ParserCanonicalizationAndResourceBounds,
    ];
}

impl KagemushaRecursiveSpendCryptographicReviewPayloadV4 {
    /// Construct the canonical approved-review payload for an immutable candidate.
    ///
    /// The six check-evidence digests must follow
    /// [`KagemushaRecursiveSpendCryptographicReviewCheckV4::ALL`]. Final release
    /// authentication still validates every digest and reviewer signature.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn approved(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        report_sha256: [u8; 32],
        check_evidence_sha256: [[u8; 32];
            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_CHECK_COUNT_V4],
    ) -> Result<Self, KagemushaValidationError> {
        let subject = candidate.cryptographic_review_subject()?;
        let mut evidence_digests = std::collections::BTreeSet::new();
        evidence_digests.insert(subject.candidate_sha256);
        if report_sha256 == [0; 32] || !evidence_digests.insert(report_sha256) {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.cryptographic_review_evidence",
            });
        }
        for evidence_sha256 in &check_evidence_sha256 {
            if *evidence_sha256 == [0; 32] || !evidence_digests.insert(*evidence_sha256) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.cryptographic_review_evidence",
                });
            }
        }
        let checks = KagemushaRecursiveSpendCryptographicReviewCheckV4::ALL
            .into_iter()
            .zip(check_evidence_sha256)
            .map(|(check, evidence_sha256)| {
                KagemushaRecursiveSpendCryptographicReviewCheckResultV4 {
                    check,
                    status: KagemushaRecursiveSpendCryptographicReviewCheckStatusV4::Passed,
                    evidence_sha256,
                }
            })
            .collect();
        Ok(Self {
            domain: KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_DOMAIN_V4.to_owned(),
            subject,
            decision: KagemushaRecursiveSpendCryptographicReviewDecisionV4::Approved,
            report_sha256,
            artifact_roles: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
                .map(str::to_owned)
                .to_vec(),
            checks,
        })
    }
}

impl KagemushaRecursiveSpendReleaseApprovalRoleV1 {
    const fn index(self) -> usize {
        match self {
            Self::Release => 0,
            Self::CryptographicReview => 1,
            Self::PhysicalDeviceBenchmark => 2,
        }
    }
}

impl KagemushaRecursiveSpendReleaseAttestationSubjectV4 {
    /// Return the exact V4 domain- and role-separated approval payload.
    #[must_use]
    pub fn approval_payload(
        &self,
        role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
    ) -> KagemushaRecursiveSpendReleaseApprovalPayloadV4 {
        KagemushaRecursiveSpendReleaseApprovalPayloadV4 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_APPROVAL_DOMAIN_V4.to_owned(),
            role,
            subject: self.clone(),
        }
    }
}

impl KagemushaRecursiveSpendReleasePolicyV1 {
    /// Validate canonical role order, thresholds, signer order, and role independence.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaReleaseVerificationError> {
        let expected_roles = [
            KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
        ];
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1
            || !is_kagemusha_portable_identifier(&self.policy_id)
            || self.roles.len() != expected_roles.len()
        {
            return Err(KagemushaReleaseVerificationError::InvalidPolicy);
        }

        let mut all_signers = std::collections::BTreeSet::new();
        for (role_policy, expected_role) in self.roles.iter().zip(expected_roles) {
            let signer_count = role_policy.authorized_signers.len();
            if role_policy.role != expected_role
                || signer_count == 0
                || signer_count > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
                || role_policy.threshold == 0
                || usize::from(role_policy.threshold) > signer_count
                || !role_policy
                    .authorized_signers
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                || role_policy
                    .authorized_signers
                    .iter()
                    .any(|signer| !all_signers.insert(signer.clone()))
            {
                return Err(KagemushaReleaseVerificationError::InvalidPolicy);
            }
        }
        Ok(())
    }

    fn role_policy(
        &self,
        role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
    ) -> Option<&KagemushaRecursiveSpendReleaseRolePolicyV1> {
        self.roles
            .get(role.index())
            .filter(|policy| policy.role == role)
    }
}

impl KagemushaRecursiveSpendCryptographicReviewEvidenceV4 {
    fn validate_against_candidate(
        &self,
        candidate: &KagemushaRecursiveSpendCandidateV4,
    ) -> Result<Vec<PublicKey>, KagemushaReleaseVerificationError> {
        let expected_subject = candidate
            .cryptographic_review_subject()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidCryptographicReview)?;
        let expected_artifact_roles =
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.map(str::to_owned);
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_SCHEMA_V4
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_VERSION_V4
            || self.payload.domain != KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_DOMAIN_V4
            || self.payload.subject != expected_subject
            || self.payload.decision
                != KagemushaRecursiveSpendCryptographicReviewDecisionV4::Approved
            || self.payload.artifact_roles != expected_artifact_roles
            || self.payload.checks.len()
                != KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_CHECK_COUNT_V4
            || self.approvals.is_empty()
            || self.approvals.len() > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
            || !self
                .approvals
                .windows(2)
                .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
        }

        let mut evidence_digests = std::collections::BTreeSet::new();
        evidence_digests.insert(self.payload.subject.candidate_sha256);
        if self.payload.report_sha256 == [0; 32]
            || !evidence_digests.insert(self.payload.report_sha256)
        {
            return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
        }
        for (result, expected_check) in self
            .payload
            .checks
            .iter()
            .zip(KagemushaRecursiveSpendCryptographicReviewCheckV4::ALL)
        {
            if result.check != expected_check
                || result.status != KagemushaRecursiveSpendCryptographicReviewCheckStatusV4::Passed
                || result.evidence_sha256 == [0; 32]
                || !evidence_digests.insert(result.evidence_sha256)
            {
                return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
            }
        }

        let mut reviewer_keys = Vec::with_capacity(self.approvals.len());
        for approval in &self.approvals {
            approval
                .signature
                .verify(&approval.public_key, &self.payload)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidSignature {
                    role: KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
                })?;
            reviewer_keys.push(approval.public_key.clone());
        }
        Ok(reviewer_keys)
    }

    /// Decode canonical Norito review bytes and validate their candidate binding.
    ///
    /// This structural entry point verifies every embedded signature. Release
    /// authentication additionally authorizes those identities against the local
    /// policy and binds the exact same reviewer set into the release attestation.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_canonical_bytes_against_candidate(
        bytes: &[u8],
        candidate: &KagemushaRecursiveSpendCandidateV4,
    ) -> Result<Vec<PublicKey>, KagemushaReleaseVerificationError> {
        if bytes.is_empty()
            || bytes.len() > KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4
        {
            return Err(KagemushaReleaseVerificationError::EvidenceMismatch {
                role: KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            });
        }
        let decode_limits = norito::core::DecodeLimits::new(
            16 * 1024,
            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
            128 * 1024,
            4 * KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
            64,
        );
        let evidence: Self = norito::decode_from_bytes_with_limits(bytes, decode_limits)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidCryptographicReview)?;
        let canonical_bytes = to_bytes(&evidence)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidCryptographicReview)?;
        if canonical_bytes != bytes {
            return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
        }
        evidence.validate_against_candidate(candidate)
    }

    fn authenticate_canonical_bytes(
        bytes: &[u8],
        candidate: &KagemushaRecursiveSpendCandidateV4,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
    ) -> Result<Vec<PublicKey>, KagemushaReleaseVerificationError> {
        policy.validate()?;
        let reviewer_keys = Self::validate_canonical_bytes_against_candidate(bytes, candidate)?;
        let role = KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview;
        let role_policy = policy
            .role_policy(role)
            .ok_or(KagemushaReleaseVerificationError::InvalidPolicy)?;
        for public_key in &reviewer_keys {
            if role_policy
                .authorized_signers
                .binary_search(public_key)
                .is_err()
            {
                return Err(KagemushaReleaseVerificationError::UnknownSigner { role });
            }
        }
        let collected = u16::try_from(reviewer_keys.len())
            .map_err(|_| KagemushaReleaseVerificationError::InvalidCryptographicReview)?;
        if collected < role_policy.threshold {
            return Err(KagemushaReleaseVerificationError::InsufficientThreshold {
                role,
                collected,
                required: role_policy.threshold,
            });
        }
        Ok(reviewer_keys)
    }
}

impl KagemushaAuthenticatedReleaseV4 {
    fn verify_attestation(
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
        attestation: &KagemushaRecursiveSpendReleaseAttestationV4,
    ) -> Result<Self, KagemushaReleaseVerificationError> {
        manifest
            .validate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        policy.validate()?;

        let expected_subject = manifest.release_attestation_subject()?;
        if attestation.schema != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V4
            || attestation.version != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4
            || attestation.subject != expected_subject
            || attestation.approvals.is_empty()
            || attestation.approvals.len() > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
        {
            return Err(KagemushaReleaseVerificationError::InvalidAttestation);
        }

        let attestation_bytes = to_bytes(attestation)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidAttestation)?;
        let attestation_sha256: [u8; 32] = Sha256::digest(attestation_bytes).into();
        if attestation_sha256 != manifest.release_attestation_sha256 {
            return Err(KagemushaReleaseVerificationError::InvalidAttestation);
        }

        let mut counts = [0_u16; 3];
        let mut approved_signers = Vec::with_capacity(attestation.approvals.len());
        let mut previous: Option<(KagemushaRecursiveSpendReleaseApprovalRoleV1, &PublicKey)> = None;
        for approval in &attestation.approvals {
            let identity = (approval.role, &approval.public_key);
            if previous.is_some_and(|previous| previous >= identity) {
                return Err(KagemushaReleaseVerificationError::DuplicateOrUnorderedSigner);
            }
            previous = Some(identity);
            let role_policy = policy.role_policy(approval.role).ok_or(
                KagemushaReleaseVerificationError::UnknownSigner {
                    role: approval.role,
                },
            )?;
            if role_policy
                .authorized_signers
                .binary_search(&approval.public_key)
                .is_err()
            {
                return Err(KagemushaReleaseVerificationError::UnknownSigner {
                    role: approval.role,
                });
            }
            let payload = expected_subject.approval_payload(approval.role);
            approval
                .signature
                .verify(&approval.public_key, &payload)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidSignature {
                    role: approval.role,
                })?;
            counts[approval.role.index()] = counts[approval.role.index()].saturating_add(1);
            approved_signers.push(KagemushaRecursiveSpendApprovedSignerV1 {
                role: approval.role,
                public_key: approval.public_key.clone(),
            });
        }
        for role_policy in &policy.roles {
            let collected = counts[role_policy.role.index()];
            if collected < role_policy.threshold {
                return Err(KagemushaReleaseVerificationError::InsufficientThreshold {
                    role: role_policy.role,
                    collected,
                    required: role_policy.threshold,
                });
            }
        }

        let manifest_sha256 = Sha256::digest(
            to_bytes(manifest).map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?,
        )
        .into();
        let release_policy_sha256 = Sha256::digest(
            to_bytes(policy).map_err(|_| KagemushaReleaseVerificationError::InvalidPolicy)?,
        )
        .into();
        Ok(Self {
            manifest: manifest.clone(),
            manifest_sha256,
            release_attestation_sha256: attestation_sha256,
            release_policy_sha256,
            approved_signers,
        })
    }

    /// Authenticate a V4 release and hash-check its exact evidence files.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when structural, policy, signature, or cryptographic authentication fails.
    pub fn verify(
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
        attestation: &KagemushaRecursiveSpendReleaseAttestationV4,
        benchmark_evidence: &[u8],
        cryptographic_review: &[u8],
    ) -> Result<Self, KagemushaReleaseVerificationError> {
        for (role, bytes, expected_digest, maximum_bytes) in [
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
                benchmark_evidence,
                manifest.benchmark_evidence_sha256,
                KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
            ),
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
                cryptographic_review,
                manifest.cryptographic_review_sha256,
                KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
            ),
        ] {
            if bytes.is_empty()
                || bytes.len() > maximum_bytes
                || <[u8; 32]>::from(Sha256::digest(bytes)) != expected_digest
            {
                return Err(KagemushaReleaseVerificationError::EvidenceMismatch { role });
            }
        }
        let candidate = manifest
            .immutable_candidate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        let review_signers =
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::authenticate_canonical_bytes(
                cryptographic_review,
                &candidate,
                policy,
            )?;
        let authenticated = Self::verify_attestation(manifest, policy, attestation)?;
        let attested_review_signers = authenticated
            .approved_signers
            .iter()
            .filter(|signer| {
                signer.role == KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview
            })
            .map(|signer| signer.public_key.clone())
            .collect::<Vec<_>>();
        if review_signers != attested_review_signers {
            return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
        }
        Ok(authenticated)
    }

    /// Authenticated V4 manifest selected by this runtime proof.
    #[must_use]
    pub fn manifest(&self) -> &KagemushaRecursiveSpendArtifactManifestV4 {
        &self.manifest
    }

    /// SHA-256 of the exact canonical V4 manifest.
    #[must_use]
    pub const fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }

    /// SHA-256 of the exact signed V4 release envelope.
    #[must_use]
    pub const fn release_attestation_sha256(&self) -> [u8; 32] {
        self.release_attestation_sha256
    }

    /// SHA-256 of the exact locally trusted release policy.
    #[must_use]
    pub const fn release_policy_sha256(&self) -> [u8; 32] {
        self.release_policy_sha256
    }

    /// Canonically ordered role/signer identities whose V4 approvals verified.
    #[must_use]
    pub fn approved_signers(&self) -> &[KagemushaRecursiveSpendApprovedSignerV1] {
        &self.approved_signers
    }
}

impl KagemushaRecursiveSpendPromotedReleaseV4 {
    /// Validate the standalone ABI-21 promotion marker.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaReleaseVerificationError> {
        let digests = [
            self.candidate_sha256,
            self.manifest_sha256,
            self.release_attestation_sha256,
            self.release_policy_sha256,
        ];
        let digests_are_distinct_and_nonzero = digests.iter().all(|digest| *digest != [0; 32])
            && digests
                .iter()
                .enumerate()
                .all(|(index, digest)| !digests[..index].contains(digest));
        let signers_are_canonical = !self.approved_signers.is_empty()
            && self.approved_signers.len() <= KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
            && self
                .approved_signers
                .windows(2)
                .all(|pair| pair[0] < pair[1]);
        let mut represented_roles = [false; 3];
        for signer in &self.approved_signers {
            represented_roles[signer.role.index()] = true;
        }
        let expected_artifact_roles =
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.map(str::to_owned);

        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4
            || !is_kagemusha_portable_identifier(&self.generation)
            || !digests_are_distinct_and_nonzero
            || !signers_are_canonical
            || represented_roles
                .into_iter()
                .any(|represented| !represented)
            || !self.artifact_inventory_verified
            || self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
            || self.artifact_roles != expected_artifact_roles
            || self.max_proof_bytes == 0
            || self.max_proof_bytes > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }

    /// Require this marker to identify one exact authenticated V4 release.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_against_authenticated_release(
        &self,
        release: &KagemushaAuthenticatedReleaseV4,
    ) -> Result<(), KagemushaReleaseVerificationError> {
        self.validate()?;
        let candidate_sha256 = release
            .manifest()
            .immutable_candidate()
            .and_then(|candidate| candidate.sha256())
            .map_err(|_| KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
        if self.candidate_sha256 != candidate_sha256
            || self.generation != release.manifest().generation
            || self.manifest_sha256 != release.manifest_sha256()
            || self.release_attestation_sha256 != release.release_attestation_sha256()
            || self.release_policy_sha256 != release.release_policy_sha256()
            || self.approved_signers != release.approved_signers()
            || self.max_proof_bytes != release.manifest().max_proof_bytes
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }

    /// Require this marker to bind the immutable candidate and finalized release.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_against_candidate_and_authenticated_release(
        &self,
        candidate: &KagemushaRecursiveSpendCandidateV4,
        release: &KagemushaAuthenticatedReleaseV4,
    ) -> Result<(), KagemushaReleaseVerificationError> {
        self.validate_against_authenticated_release(release)?;
        let candidate_sha256 = candidate
            .sha256()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
        if self.candidate_sha256 != candidate_sha256
            || candidate.manifest.generation != release.manifest().generation
            || candidate.manifest.source_commit != release.manifest().source_commit
            || candidate.manifest.source_tree_sha256 != release.manifest().source_tree_sha256
            || candidate.manifest.source_repo_dirty != release.manifest().source_repo_dirty
            || candidate.manifest.reviewed_source_closure
                != release.manifest().reviewed_source_closure
            || candidate.manifest.reviewed_source_closure_descriptor_sha256
                != release.manifest().reviewed_source_closure_descriptor_sha256
            || candidate.manifest.chain_id != release.manifest().chain_id
            || candidate.manifest.asset != release.manifest().asset
            || candidate.manifest.asset_scale != release.manifest().asset_scale
            || candidate.manifest.activation_height != release.manifest().activation_height
            || candidate.manifest.withdrawal_height != release.manifest().withdrawal_height
            || candidate.manifest.max_proof_bytes != release.manifest().max_proof_bytes
            || candidate.manifest.profiles != release.manifest().profiles
            || candidate.manifest.topup_finality_roster_artifact
                != release.manifest().topup_finality_roster_artifact
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendReleaseRecordV4 {
    /// Validate deterministic release hashes without consulting local trust policy.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaReleaseVerificationError> {
        self.manifest
            .validate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        self.promotion_record.validate()?;
        let manifest_sha256: [u8; 32] = Sha256::digest(
            to_bytes(&self.manifest)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?,
        )
        .into();
        let attestation_sha256: [u8; 32] = Sha256::digest(
            to_bytes(&self.release_attestation)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidAttestation)?,
        )
        .into();
        let summaries = [
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
                self.physical_device_benchmark_summary.as_slice(),
                self.manifest.benchmark_evidence_sha256,
                KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
            ),
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
                self.cryptographic_review_summary.as_slice(),
                self.manifest.cryptographic_review_sha256,
                KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
            ),
        ];
        for (role, summary, expected_sha256, maximum_bytes) in summaries {
            if summary.is_empty()
                || summary.len() > maximum_bytes
                || <[u8; 32]>::from(Sha256::digest(summary)) != expected_sha256
            {
                return Err(KagemushaReleaseVerificationError::EvidenceMismatch { role });
            }
        }
        let candidate = self
            .manifest
            .immutable_candidate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
            &self.cryptographic_review_summary,
            &candidate,
        )?;
        if attestation_sha256 != self.manifest.release_attestation_sha256
            || self.promotion_record.generation != self.manifest.generation
            || self.promotion_record.manifest_sha256 != manifest_sha256
            || self.promotion_record.release_attestation_sha256 != attestation_sha256
            || self.promotion_record.max_proof_bytes != self.manifest.max_proof_bytes
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }

    /// Authenticate every signed release field against the configured policy.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when structural, policy, signature, or cryptographic authentication fails.
    pub fn authenticate(
        &self,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
    ) -> Result<KagemushaAuthenticatedReleaseV4, KagemushaReleaseVerificationError> {
        self.validate_structure()?;
        let release = KagemushaAuthenticatedReleaseV4::verify(
            &self.manifest,
            policy,
            &self.release_attestation,
            &self.physical_device_benchmark_summary,
            &self.cryptographic_review_summary,
        )?;
        self.promotion_record
            .validate_against_authenticated_release(&release)?;
        Ok(release)
    }
}

impl KagemushaRecursiveSpendReleaseActivationV4 {
    /// Validate the release-bound Eq/Ep registry shape before consensus admission.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaReleaseVerificationError> {
        self.release_record.validate_structure()?;
        let manifest_sha256 = self
            .release_record
            .manifest
            .canonical_sha256()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        let expected_vesta_verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            manifest_sha256,
        );
        let expected_pallas_verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEp,
            manifest_sha256,
        );
        if self.configured_policy_sha256 == [0; 32]
            || self.configured_policy_sha256
                != self.release_record.promotion_record.release_policy_sha256
            || self.step_eq_verifier_key_id != expected_vesta_verifier_key_id
            || self.step_ep_verifier_key_id != expected_pallas_verifier_key_id
            || !self.step_eq_verifier_key_id.is_portable_registry_id()
            || !self.step_ep_verifier_key_id.is_portable_registry_id()
            || self.step_eq_verifier_record.version == 0
            || self.step_eq_verifier_record.version != self.step_ep_verifier_record.version
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        self.validate_verifier_record(
            &self.step_eq_verifier_record,
            KagemushaPastaCycleParityV1::StepEq,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
        )?;
        self.validate_verifier_record(
            &self.step_ep_verifier_record,
            KagemushaPastaCycleParityV1::StepEp,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V4,
        )?;
        Ok(())
    }

    fn validate_verifier_record(
        &self,
        record: &VerifyingKeyRecord,
        parity: KagemushaPastaCycleParityV1,
        expected_curve: &str,
    ) -> Result<(), KagemushaReleaseVerificationError> {
        let manifest = &self.release_record.manifest;
        let profile = manifest
            .profiles
            .iter()
            .find(|profile| profile.parity == parity)
            .ok_or(KagemushaReleaseVerificationError::InvalidManifest)?;
        let descriptor = profile
            .artifacts
            .get(2)
            .filter(|artifact| artifact.kind == KagemushaPastaCycleArtifactKindV4::VerifyingKey)
            .ok_or(KagemushaReleaseVerificationError::InvalidManifest)?;
        let key = record
            .key
            .as_ref()
            .ok_or(KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
        let key_len = u64::try_from(key.bytes.len())
            .map_err(|_| KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
        if record.circuit_id != profile.circuit_id
            || record
                .owner_manifest_id
                .as_deref()
                .is_none_or(str::is_empty)
            || record.namespace != KAGEMUSHA_VERIFIER_NAMESPACE
            || record.backend != BackendTag::Halo2IpaPasta
            || record.curve != expected_curve
            || record.public_inputs_schema_hash == [0; 32]
            || record.commitment == [0; 32]
            || u64::from(record.vk_len) != key_len
            || record.max_proof_bytes != manifest.max_proof_bytes
            || record.activation_height != Some(manifest.activation_height)
            || record.withdraw_height.is_some()
            || record.status != ConfidentialStatus::Active
            || key.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
            || key_len != descriptor.payload_size_bytes
            || <[u8; 32]>::from(Sha256::digest(&key.bytes)) != descriptor.payload_sha256
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }
}

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
        let digest: [u8; 32] = Sha256::digest(canonical_manifest_bytes).into();
        if self.generation != manifest.generation || self.manifest_sha256 != digest {
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
    /// `max_proof_bytes` is deliberately release-specific: it must come from
    /// the authenticated V4 manifest selected by the installed artifact
    /// handle, rather than from a compile-time default.
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
        let manifest_sha256: [u8; 32] = Sha256::digest(to_bytes(manifest).map_err(|_| {
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

    /// Validate the envelope in an exact chain, asset, scale, and height context.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_against_manifest_for_context(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        expected_chain_id: &ChainId,
        expected_asset: &AssetDefinitionId,
        expected_asset_scale: u32,
        block_height: u64,
    ) -> Result<(), KagemushaValidationError> {
        self.validate_against_manifest(manifest)?;
        if &manifest.chain_id != expected_chain_id
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
/// Identifiers use the same single-component restrictions as artifact file
/// names so release caches cannot alias punctuation-only or Windows device
/// names across build hosts.
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

fn is_kagemusha_chain_id(value: &ChainId) -> bool {
    let value = value.as_str();
    !value.is_empty()
        && value.len() <= 128
        && value.trim() == value
        && !value.chars().any(char::is_control)
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
            || self.leaf_index >= KAGEMUSHA_TOPUP_SHIELD_TREE_CAPACITY_V2
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
    /// Validate a non-zero chain-resolvable identity pair.
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
        let next_roster_too_large = self.next_epoch_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.roster.len() > KAGEMUSHA_TOPUP_FINALITY_MAX_VALIDATORS_V2
                || snapshot.validator_set_pops.len() > KAGEMUSHA_TOPUP_FINALITY_MAX_VALIDATORS_V2
        });
        let parent_signers_too_large = self.parent_commit_qc.as_ref().is_some_and(|parent| {
            parent.signers.len() > KAGEMUSHA_TOPUP_FINALITY_MAX_VALIDATORS_V2
        });
        if self.protocol_version != PROTOCOL_VERSION
            || self.height == 0
            || !is_kagemusha_chain_id(&self.chain_id)
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
            chain_id: self.chain_id.clone(),
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
            || certificate.proposal_round.view > certificate.round.view
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
    /// Validate the exact ordered roster, powers, and activation window without
    /// performing proof-of-possession pairings.
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
                entry.power == 0
                    || !matches!(
                        entry.validator.public_key().try_algorithm(),
                        Ok(Algorithm::BlsNormal)
                    )
            })
            || (self.consensus_mode == ConsensusMode::Permissioned
                && self.validator_set.iter().any(|entry| entry.power != 1))
            || DualQuorum::from_roster(&self.validator_set).is_err()
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.roster_window.validator_set",
            });
        }
        Ok(())
    }

    /// Validate the complete roster window, including every BLS proof of
    /// possession. Callers handling repeated proofs should cache success by the
    /// authenticated roster-archive digest.
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
    /// Validate chain-scoped, strictly ordered, non-overlapping trust windows
    /// without performing BLS proof-of-possession pairings.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaValidationError> {
        if self.version != KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2
            || !is_kagemusha_chain_id(&self.chain_id)
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
    /// Return the domain-separated digest exposed by the redemption-change circuit.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        kagemusha_poseidon_preimage(&KagemushaUnshieldPublicInputsDigestPreimageV2 {
            domain: KAGEMUSHA_UNSHIELD_PUBLIC_INPUTS_DIGEST_DOMAIN_V2.to_owned(),
            public_inputs: *self,
        })
    }
}

#[cfg(test)]
mod kagemusha_v4_artifact_contract_tests {
    use norito::core::{DecodeFromSlice as _, NoritoDeserialize as _};

    use crate::{
        domain::DomainId,
        isi::{InstructionBox, offline::ActivateKagemushaRecursiveReleaseV4},
    };

    use super::*;

    fn digest(label: &[u8]) -> [u8; 32] {
        Sha256::digest(label).into()
    }

    fn reviewed_source_closure() -> KagemushaReviewedSourceClosureV1 {
        let source_commit = "1234567890abcdef1234567890abcdef12345678".to_owned();
        let entry = KagemushaReviewedSourceClosureManifestEntryV1 {
            blob_sha256: digest(b"reviewed untracked source bytes"),
            git_blob_oid: "abcdef1234567890abcdef1234567890abcdef12".to_owned(),
            git_mode: "100644".to_owned(),
            path: "reviewed-untracked-source.rs".to_owned(),
            path_bytes_base64: BASE64_STANDARD.encode(b"reviewed-untracked-source.rs"),
        };
        let entry_json = kagemusha_reviewed_source_manifest_entry_json(&entry);
        let manifest_sha256 = Sha256::digest(format!("{entry_json}\n")).into();
        let tracked_binary_diff_sha256 = digest(b"reviewed tracked binary diff");
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
            source_repo_dirty: true,
            source_tree_sha256: digest(b"v4 artifact test source tree"),
            tracked_binary_diff_sha256,
            untracked_file_count: 1,
            untracked_path_mode_blob_oid_manifest: vec![entry],
            untracked_path_mode_blob_oid_manifest_sha256: manifest_sha256,
            ignored_cargo_lock_size_bytes: 123,
            ignored_cargo_lock_sha256: digest(b"reviewed ignored Cargo.lock"),
            combined_source_fingerprint_sha256: combined.finalize().into(),
        }
    }

    fn circuit_params() -> KagemushaStepCircuitParamsV4 {
        let k = KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
        let layout =
            KagemushaPastaPublicLayoutV4::for_ipa_round_count(k).expect("test V4 public layout");
        KagemushaStepCircuitParamsV4 {
            version: KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
            k,
            num_advice_per_phase: vec![8],
            num_lookup_advice_per_phase: vec![1],
            num_fixed: 1,
            lookup_bits: k - 1,
            num_instance_columns: 1,
            public_input_limbs: layout.instance_column_limbs,
            minimum_unusable_rows: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
            max_parent_proof_bytes: 8_192,
        }
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
        params: KagemushaStepCircuitParamsV4,
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
        KagemushaRecursiveSpendArtifactManifestV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
            generation: "v4-artifact-test-release".to_owned(),
            source_commit: reviewed_source_closure.source_commit.clone(),
            source_tree_sha256: reviewed_source_closure.source_tree_sha256,
            source_repo_dirty: true,
            reviewed_source_closure,
            reviewed_source_closure_descriptor_sha256,
            chain_id: ChainId::from("v4-artifact-test-chain"),
            asset: AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").expect("test domain"),
                "rose".parse().expect("test asset name"),
            ),
            asset_scale: 9,
            activation_height: 1,
            withdrawal_height: 100,
            max_proof_bytes: KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
            profiles: vec![
                profile(KagemushaPastaCycleParityV1::StepEq, params.clone(), 1),
                profile(KagemushaPastaCycleParityV1::StepEp, params, 11),
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
            benchmark_evidence_sha256: digest(b"v4 artifact test benchmark"),
            cryptographic_review_sha256: digest(b"v4 artifact test review"),
            release_attestation_sha256: digest(b"v4 artifact test attestation"),
        }
    }

    fn unsigned_candidate(
        template: &KagemushaRecursiveSpendArtifactManifestV4,
    ) -> KagemushaRecursiveSpendCandidateV4 {
        let mut manifest = template.clone();
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
        let payload = KagemushaRecursiveSpendCryptographicReviewPayloadV4::approved(
            candidate,
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
        to_bytes(&KagemushaRecursiveSpendCryptographicReviewEvidenceV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_VERSION_V4,
            payload,
            approvals,
        })
        .expect("canonical signed review evidence")
    }

    fn promoted_release() -> KagemushaRecursiveSpendPromotedReleaseV4 {
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
            candidate_sha256: digest(b"v4 promotion candidate"),
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
            revoked_certificate_sha256: vec![vec![0x51; 32]],
            ios_apps: vec![OfflineIosAppAttestationPolicy {
                team_id: "WIRETEAM1".to_owned(),
                bundle_id: "com.example.wire".to_owned(),
                environment: "production".to_owned(),
                allowed_validation_categories: vec![1, 10],
                allowed_bundle_versions: vec!["1.0".to_owned()],
                allow_legacy_auth_data_without_extensions: false,
            }],
            android_apps: vec![OfflineAndroidAppAttestationPolicy {
                package_name: "com.example.wire".to_owned(),
                signing_certificate_sha256: vec![vec![0x61; 32]],
            }],
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
        let chain_id = ChainId::from("v4-wire-test-chain");
        let definition = AssetDefinitionId::new(
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
            chain_id,
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
    fn v4_promotion_record_is_distinct_and_fail_closed() {
        let record = promoted_release();
        record.validate().expect("valid V4 promotion record");

        let mut tampered = record.clone();
        tampered.schema = "retired-promoted-release".to_owned();
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );

        let mut tampered = record.clone();
        tampered.version = KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1;
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );

        let mut tampered = record.clone();
        tampered.generation = "not portable/".to_owned();
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );

        let mut tampered = record.clone();
        tampered.manifest_sha256 = [0; 32];
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );

        let mut tampered = record.clone();
        tampered.release_policy_sha256 = tampered.release_attestation_sha256;
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );

        let mut tampered = record.clone();
        tampered.approved_signers.swap(0, 1);
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );

        let mut tampered = record.clone();
        tampered.approved_signers.pop();
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );

        let mut tampered = record.clone();
        let duplicate_signer = tampered.approved_signers[0].clone();
        tampered.approved_signers.insert(1, duplicate_signer);
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );

        let mut tampered = record.clone();
        tampered.artifact_inventory_verified = false;
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );

        let mut tampered = record.clone();
        tampered.bridge_abi_version = KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4 - 1;
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );

        let mut tampered = record.clone();
        tampered.artifact_roles.swap(0, 1);
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );

        let mut tampered = record.clone();
        tampered.max_proof_bytes = 0;
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );

        let mut tampered = record;
        tampered.max_proof_bytes = KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 + 1;
        assert_eq!(
            tampered.validate(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );
    }

    #[test]
    fn v4_release_generation_profile_is_exact_compact_geometry() {
        let reviewed = circuit_params();
        reviewed
            .validate_release_generation_profile()
            .expect("reviewed compact degree-16 generation profile");

        let mut uncalibrated = reviewed.clone();
        uncalibrated.num_advice_per_phase = vec![1];
        uncalibrated.num_lookup_advice_per_phase = vec![1];
        assert!(uncalibrated.validate().is_err());
        assert!(
            uncalibrated.validate_release_generation_profile().is_err(),
            "uncalibrated geometry must not authorize release generation"
        );

        let mut phantom_phase = reviewed.clone();
        phantom_phase.num_advice_per_phase.push(1);
        phantom_phase.num_lookup_advice_per_phase.push(0);
        assert!(
            phantom_phase.validate().is_err(),
            "Kagemusha must reject an unconstrained speculative advice phase"
        );

        let mut unreviewed_degree = reviewed;
        unreviewed_degree.k = KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4 + 1;
        unreviewed_degree.lookup_bits = unreviewed_degree.k - 1;
        assert!(unreviewed_degree.validate().is_err());
        assert!(
            unreviewed_degree
                .validate_release_generation_profile()
                .is_err()
        );
    }

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
        assert_eq!(minimum_layout.accumulator_limbs, 36);
        assert_eq!(minimum_layout.live_selector_offset, 63);
        assert_eq!(minimum_layout.instance_column_limbs, 64);
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
        below_minimum.public_input_limbs = 64;
        assert!(below_minimum.validate().is_err());
        let mut above_maximum = circuit_params();
        above_maximum.k = KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4 + 1;
        above_maximum.lookup_bits = above_maximum.k - 1;
        above_maximum.public_input_limbs = 64;
        assert!(above_maximum.validate().is_err());

        for schema in [
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PUBLIC_INPUTS_SCHEMA_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PUBLIC_INPUTS_SCHEMA_V4,
        ] {
            let schema = core::str::from_utf8(schema).expect("static schema is UTF-8");
            assert!(schema.contains("\"elements\":64"));
            assert!(schema.contains("\"ipa_round_count\":16"));
            assert!(schema.contains("\"semantic_authority\":\"step_eq\""));
            assert!(!schema.contains("\"state_layout_version\":2"));
            assert!(!schema.contains("\"state_limbs\":890"));
            assert!(schema.contains("\"ipa_accumulator\":{\"wire_version\":5,\"elements\":36"));
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

    #[test]
    fn v4_profiles_bind_exact_four_role_inventory_and_inline_params() {
        let manifest = manifest();
        manifest.validate().expect("valid four-role V4 manifest");
        assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.len(), 8);
        for profile in &manifest.profiles {
            assert_eq!(profile.artifacts.len(), 4);
            profile
                .circuit_params
                .validate()
                .expect("valid inline circuit parameters");
            assert_eq!(
                profile
                    .artifacts
                    .iter()
                    .map(|artifact| artifact.kind)
                    .collect::<Vec<_>>(),
                vec![
                    KagemushaPastaCycleArtifactKindV4::ParamsIpa,
                    KagemushaPastaCycleArtifactKindV4::ProvingKey,
                    KagemushaPastaCycleArtifactKindV4::VerifyingKey,
                    KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
                ]
            );
            assert_eq!(
                profile
                    .bootstrap_artifact()
                    .expect("bootstrap descriptor")
                    .kind,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness
            );
        }

        let mut tampered = manifest.clone();
        tampered.profiles[0].circuit_params.num_fixed = 0;
        assert!(tampered.validate().is_err());

        let mut separate_params_file = manifest.clone();
        let mut rejected_artifact = separate_params_file.profiles[0].artifacts[0].clone();
        rejected_artifact.file_name = ["step-eq.circuit-", "params.krv4"].concat();
        rejected_artifact.sha256 = digest(b"rejected separate circuit parameters frame");
        rejected_artifact.payload_sha256 = digest(b"rejected separate circuit parameters");
        separate_params_file.profiles[0]
            .artifacts
            .insert(1, rejected_artifact);
        assert!(
            separate_params_file.validate().is_err(),
            "a separate circuit-parameter file must not extend the exact inventory"
        );

        let mut reordered = manifest;
        reordered.profiles[0].artifacts.swap(1, 2);
        assert!(reordered.validate().is_err());
    }

    #[test]
    fn v4_artifact_contract_source_guard_is_exhaustive() {
        fn canonical_index(kind: KagemushaPastaCycleArtifactKindV4) -> usize {
            match kind {
                KagemushaPastaCycleArtifactKindV4::ParamsIpa => 0,
                KagemushaPastaCycleArtifactKindV4::ProvingKey => 1,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey => 2,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness => 3,
            }
        }

        let kinds = [
            KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            KagemushaPastaCycleArtifactKindV4::ProvingKey,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        ];
        assert_eq!(kinds.map(canonical_index), [0, 1, 2, 3]);
        assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.len(), 8);
        assert!(
            manifest()
                .profiles
                .iter()
                .all(|profile| profile.artifacts.len() == 4)
        );

        let source = include_str!("mod.rs");
        assert!(source.contains("KagemushaPastaCycleFramedArtifactHeaderV4"));
        for forbidden in [
            concat!("Circuit", "Params,"),
            concat!("CIRCUIT_", "PARAMS_FILE_NAME_V4"),
            concat!("circuit-", "params.krv4"),
        ] {
            assert!(
                !source.contains(forbidden),
                "rejected V4 artifact contract marker is present: {forbidden}"
            );
        }
    }

    #[test]
    fn compact_recursive_state_boundary_has_a_distinct_v5_protocol() {
        assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V5, 5);
        assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5, 5);
        assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5, 138);
        assert_eq!(
            KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_DOMAIN_V5,
            b"iroha:kagemusha:recursive-state-boundary:v5"
        );
        assert_eq!(
            KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_DOMAIN_V1,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_DOMAIN_V5
        );
    }

    #[test]
    fn v4_envelope_uses_verifying_key_role_at_canonical_index() {
        let manifest = manifest();
        manifest.validate().expect("valid V4 manifest");
        let [step_eq, step_ep] = manifest.profiles.as_slice() else {
            panic!("test manifest must have Eq/Ep profiles");
        };
        let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2];
        state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2;
        let mut envelope = KagemushaPastaCycleProofEnvelopeV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
            proof_backend: manifest.proof_backend.clone(),
            transcript_profile: manifest.transcript_profile.clone(),
            step_eq_circuit_id: step_eq.circuit_id.clone(),
            step_ep_circuit_id: step_ep.circuit_id.clone(),
            artifact_generation: manifest.generation.clone(),
            manifest_sha256: digest(&to_bytes(&manifest).expect("canonical manifest")),
            step_eq_parameter_generation: step_eq.parameter_generation.clone(),
            step_ep_parameter_generation: step_ep.parameter_generation.clone(),
            step_eq_circuit_params_sha256: step_eq
                .circuit_params
                .sha256()
                .expect("Eq params identity"),
            step_ep_circuit_params_sha256: step_ep
                .circuit_params
                .sha256()
                .expect("Ep params identity"),
            step_eq_verifier_key_sha256: step_eq.artifacts[2].payload_sha256,
            step_ep_verifier_key_sha256: step_ep.artifacts[2].payload_sha256,
            state_boundary: KagemushaRecursiveSpendStateBoundaryV2 {
                layout_version: KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V2,
                state_limbs,
            },
            proof: ProofBox::new(manifest.proof_backend.clone().into(), vec![0xA5]),
        };
        envelope
            .validate_against_manifest(&manifest)
            .expect("V4 envelope binds verifying-key role at index two");

        envelope.step_eq_verifier_key_sha256 = step_eq.artifacts[1].payload_sha256;
        assert!(envelope.validate_against_manifest(&manifest).is_err());
    }

    #[test]
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

        let mut params_tamper = manifest.clone();
        params_tamper.profiles[0]
            .circuit_params
            .minimum_unusable_rows += 1;
        assert_ne!(
            second_subject,
            params_tamper
                .release_attestation_subject()
                .expect("valid modified inline params subject")
        );
        let mut bootstrap_tamper = manifest.clone();
        bootstrap_tamper.profiles[0].artifacts[3].payload_sha256[0] ^= 1;
        assert_ne!(
            second_subject,
            bootstrap_tamper
                .release_attestation_subject()
                .expect("valid modified bootstrap subject")
        );

        let policy = KagemushaRecursiveSpendReleasePolicyV1 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            policy_id: "v4-artifact-test-policy".to_owned(),
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
            digest(&to_bytes(&attestation).expect("canonical V4 attestation"));
        let authenticated = KagemushaAuthenticatedReleaseV4::verify(
            &manifest,
            &policy,
            &attestation,
            &benchmark,
            &review,
        )
        .expect("fully authenticated V4 release");
        assert_eq!(authenticated.manifest(), &manifest);
        assert_eq!(authenticated.approved_signers().len(), roles.len());

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
        mismatched_manifest.release_attestation_sha256 =
            digest(&to_bytes(&mismatched_attestation).expect("mismatched canonical attestation"));
        assert_eq!(
            KagemushaAuthenticatedReleaseV4::verify(
                &mismatched_manifest,
                &mismatched_policy,
                &mismatched_attestation,
                &benchmark,
                &review,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );

        let mut signed_params_tamper = manifest.clone();
        signed_params_tamper.profiles[0]
            .circuit_params
            .minimum_unusable_rows += 1;
        assert_eq!(
            KagemushaAuthenticatedReleaseV4::verify(
                &signed_params_tamper,
                &policy,
                &attestation,
                &benchmark,
                &review,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );
        let mut signed_bootstrap_tamper = manifest;
        signed_bootstrap_tamper.profiles[0].artifacts[3].payload_sha256[0] ^= 1;
        assert_eq!(
            KagemushaAuthenticatedReleaseV4::verify(
                &signed_bootstrap_tamper,
                &policy,
                &attestation,
                &benchmark,
                &review,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );
    }

    #[test]
    fn v4_cryptographic_review_is_canonical_signed_and_candidate_bound() {
        let candidate = unsigned_candidate(&manifest());
        let reviewer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let review_bytes = signed_review_bytes(&candidate, &[&reviewer]);
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                &review_bytes,
                &candidate,
            )
            .expect("canonical signed review"),
            vec![reviewer.public_key().clone()]
        );

        assert!(
            KagemushaRecursiveSpendCryptographicReviewPayloadV4::approved(
                &candidate,
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
            ),
            Err(KagemushaReleaseVerificationError::EvidenceMismatch {
                role: KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            })
        );

        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                b"approved by independent review",
                &candidate,
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
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );

        let review: KagemushaRecursiveSpendCryptographicReviewEvidenceV4 =
            norito::decode_from_bytes(&review_bytes).expect("decode test review");
        let mut rejected = review.clone();
        rejected.payload.decision = KagemushaRecursiveSpendCryptographicReviewDecisionV4::Rejected;
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                &to_bytes(&rejected).expect("rejected review bytes"),
                &candidate,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );

        let mut failed_check = review.clone();
        failed_check.payload.checks[0].status =
            KagemushaRecursiveSpendCryptographicReviewCheckStatusV4::Failed;
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                &to_bytes(&failed_check).expect("failed-check review bytes"),
                &candidate,
            ),
            Err(KagemushaReleaseVerificationError::InvalidCryptographicReview)
        );

        let mut duplicate_digest = review.clone();
        duplicate_digest.payload.checks[1].evidence_sha256 =
            duplicate_digest.payload.checks[0].evidence_sha256;
        assert_eq!(
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
                &to_bytes(&duplicate_digest).expect("duplicate-digest review bytes"),
                &candidate,
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
                &to_bytes(&invalid_signature).expect("invalid-signature review bytes"),
                &candidate,
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

        let mut candidate_manifest = finalized.clone();
        candidate_manifest.benchmark_evidence_sha256 = [0; 32];
        candidate_manifest.cryptographic_review_sha256 = [0; 32];
        candidate_manifest.release_attestation_sha256 = [0; 32];
        assert!(
            candidate_manifest.validate().is_err(),
            "production manifest validation must reject a pre-evidence candidate"
        );
        candidate_manifest
            .validate_unsigned_candidate()
            .expect("valid unsigned V4 candidate");
        let candidate = KagemushaRecursiveSpendCandidateV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4,
            manifest: candidate_manifest,
        };
        candidate.validate().expect("valid candidate record");
        assert_ne!(candidate.sha256().expect("candidate digest"), [0; 32]);

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

    #[test]
    fn v4_activation_wire_binds_policy_and_rejects_legacy_one_field_layout() {
        let policy = device_attestation_policy_wire_fixture();
        let instruction = ActivateKagemushaRecursiveReleaseV4::new(
            release_activation_wire_fixture(),
            policy.clone(),
        );

        let boxed = InstructionBox::from(instruction.clone());
        let bytes = norito::core::to_bytes(&boxed).expect("serialize composite activation");
        let archived = norito::core::from_bytes::<InstructionBox>(&bytes)
            .expect("decode composite activation archive");
        let decoded = InstructionBox::try_deserialize(archived)
            .expect("deserialize composite activation instruction");
        assert_eq!(
            decoded
                .as_any()
                .downcast_ref::<ActivateKagemushaRecursiveReleaseV4>(),
            Some(&instruction),
            "the embedded device policy must survive the actual instruction-box wire path",
        );
        assert_eq!(instruction.device_attestation_policy(), &policy);

        let encoded = instruction.encode();
        let flags = norito::core::default_encode_flags();
        assert_eq!(
            flags & norito::core::header_flags::PACKED_STRUCT,
            0,
            "legacy-layout fixture requires the canonical AoS encoding",
        );
        let (roundtrip, used) = ActivateKagemushaRecursiveReleaseV4::decode_from_slice(&encoded)
            .expect("composite activation payload must roundtrip");
        assert_eq!(used, encoded.len());
        assert_eq!(roundtrip, instruction);

        let mut legacy_len = 0usize;
        crate::isi::read_aos_field(&encoded, &mut legacy_len, flags)
            .expect("read the former activation-only field");
        assert!(legacy_len < encoded.len());
        assert!(
            ActivateKagemushaRecursiveReleaseV4::decode_from_slice(&encoded[..legacy_len]).is_err(),
            "legacy one-field activation bytes must fail closed instead of defaulting a policy",
        );
    }

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

#[cfg(test)]
mod device_authority_p256_tests {
    use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

    use super::*;
    use crate::domain::DomainId;

    const P256_ORDER: [u8; 32] = [
        0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63,
        0x25, 0x51,
    ];
    const P256_HALF_ORDER: [u8; 32] = [
        0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42, 0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31,
        0x92, 0xa8,
    ];

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes((&[seed; 32]).into()).expect("non-zero P-256 test scalar")
    }

    fn device_public_key(key: &SigningKey) -> KagemushaDevicePublicKeyV2 {
        KagemushaDevicePublicKeyV2::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("canonical uncompressed test key")
    }

    fn sign(key: &SigningKey, message: &[u8]) -> KagemushaDeviceSignatureV2 {
        let signature: P256Signature = key.sign(message);
        let signature = signature.normalize_s().unwrap_or(signature);
        KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_slice())
            .expect("canonical low-S test signature")
    }

    fn scalar_pair(r: [u8; 32], s: [u8; 32]) -> [u8; 64] {
        let mut raw = [0_u8; 64];
        raw[..32].copy_from_slice(&r);
        raw[32..].copy_from_slice(&s);
        raw
    }

    fn one() -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[31] = 1;
        value
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("deterministic account key")
                .public_key()
                .clone(),
        )
    }

    fn asset(name: &str) -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("test domain"),
            name.parse().expect("test asset name"),
        )
    }

    fn placeholder_signature() -> KagemushaDeviceSignatureV2 {
        KagemushaDeviceSignatureV2::from_raw_bytes(&scalar_pair(one(), one()))
            .expect("valid low-S placeholder")
    }

    fn authorization(
        assertion_key: &SigningKey,
        ios_authenticator_data: Option<Vec<u8>>,
    ) -> KagemushaRequestAuthorizationV2 {
        let hardware_assertion = match ios_authenticator_data {
            None => KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(
                KagemushaAndroidKeyMintHardwareAssertionV1 {
                    signature: placeholder_signature(),
                },
            ),
            Some(authenticator_data) => KagemushaOnlineHardwareAssertionV1::IosAppAttest(
                KagemushaIosAppAttestHardwareAssertionV1 {
                    authenticator_data,
                    signature: placeholder_signature(),
                },
            ),
        };
        let mut authorization = KagemushaRequestAuthorizationV2 {
            authority: account(21),
            device_id: "hardware-device-21".to_owned(),
            asset_definition_id: asset("cash"),
            operation_id: [0x21; 32],
            issued_at_ms: 1_800_000_000_000,
            expires_at_ms: 1_800_000_030_000,
            nonce: [0x22; 32],
            payload_digest: [0x23; 32],
            registration_hash: [0x24; 32],
            hardware_assertion,
        };
        let signing_bytes = authorization
            .signing_bytes()
            .expect("authorization signing bytes");
        let signed_message = match &authorization.hardware_assertion {
            KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(_) => signing_bytes,
            KagemushaOnlineHardwareAssertionV1::IosAppAttest(assertion) => [
                assertion.authenticator_data.as_slice(),
                signing_bytes.as_slice(),
            ]
            .concat(),
        };
        authorization.set_hardware_signature(sign(assertion_key, &signed_message));
        authorization
    }

    fn recipient_payment_request(
        receiver_key: &SigningKey,
        issued_at_ms: u64,
        expires_at_ms: u64,
    ) -> KagemushaRecipientPaymentRequestV2 {
        let chain_id: ChainId = "kagemusha-request-boundary".parse().expect("test chain id");
        let asset = asset("cash");
        let amount = KagemushaScaledAmountV2::new(500, 2).expect("test amount");
        let receiver_public_key = device_public_key(receiver_key);
        let payload = KagemushaRecipientPaymentRequestSigningPayloadV2 {
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            amount,
            recipient: account(51),
            recipient_key_reference: kagemusha_receiver_key_reference_v2(&receiver_public_key)
                .expect("receiver key reference"),
            receiver_device_id: "receiver-device-51".to_owned(),
            receiver_public_key,
            request_id: [0x51; 32],
            issued_at_ms,
            expires_at_ms,
            recipient_output: KagemushaSpendableNoteDescriptorV2 {
                chain_id,
                asset,
                note_commitment: [0x52; 32],
                spend_nullifier: [0x53; 32],
                amount,
            },
            sender_output_prover_material: vec![0x54],
        };
        let signature = sign(
            receiver_key,
            &payload.signing_bytes().expect("request signing bytes"),
        );
        KagemushaRecipientPaymentRequestV2::from_signed_payload(payload, signature)
            .expect("signed recipient request")
    }

    fn recipient_payment_bundle(
        request: &KagemushaRecipientPaymentRequestV2,
    ) -> KagemushaRecursiveSpendBundleV4 {
        let anchor = KagemushaRecursiveSpendTopUpAnchorRefV2 {
            topup_operation_id: [0x55; 32],
            anchor_digest: [0x56; 32],
        };
        let lineage_root =
            kagemusha_recursive_spend_lineage_root_v2(anchor.anchor_digest).expect("lineage root");
        let artifact_binding = KagemushaRecursiveSpendArtifactBindingV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: "acknowledgement-expiry-test".to_owned(),
            manifest_sha256: [0x57; 32],
        };
        let verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            artifact_binding.manifest_sha256,
        );
        let recipient_request_digest = request.digest().expect("request digest");
        let operation_id = [0x58; 32];
        let statement = KagemushaRecursiveSpendPublicStatementV4 {
            chain_id: request.chain_id().clone(),
            asset: request.asset().clone(),
            asset_scale: request.amount().scale,
            final_root: [0x59; 32],
            next_zero_leaf_index: 1,
            topup_anchor_refs: vec![anchor],
            proof_step_count: 2,
            peer_hop_count: 1,
            current_note: request.recipient_output().clone(),
            branch_claims: vec![
                KagemushaRecursiveSpendBranchClaimV2::root(lineage_root)
                    .expect("root branch claim"),
            ],
            transition: Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(
                KagemushaRecursiveSpendPeerSplitTransitionV4 {
                    binding_digest: [0x5a; 32],
                    branch: KagemushaRecursiveSpendBranchV2::Recipient,
                    recipient_request_digest,
                    operation_id,
                    parent_max_proof_step_count: 1,
                    parent_max_peer_hop_count: 0,
                },
            )),
            artifact_binding: artifact_binding.clone(),
            verifier_key_id: verifier_key_id.clone(),
        };
        let public_statement_digest = statement.digest().expect("statement digest");
        let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2];
        state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2;
        let proof_envelope = KagemushaPastaCycleProofEnvelopeV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
            step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
            step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4.to_owned(),
            artifact_generation: artifact_binding.generation,
            manifest_sha256: artifact_binding.manifest_sha256,
            step_eq_parameter_generation: "ack-expiry-eq-params".to_owned(),
            step_ep_parameter_generation: "ack-expiry-ep-params".to_owned(),
            step_eq_circuit_params_sha256: [0x5b; 32],
            step_ep_circuit_params_sha256: [0x5c; 32],
            step_eq_verifier_key_sha256: [0x5d; 32],
            step_ep_verifier_key_sha256: [0x5e; 32],
            state_boundary: KagemushaRecursiveSpendStateBoundaryV2::new(state_limbs)
                .expect("state boundary"),
            proof: ProofBox::new(
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.into(),
                vec![0x5f],
            ),
        };
        let mut operation_limbs = [0; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4];
        operation_limbs[0] = 1;
        let bundle = KagemushaRecursiveSpendBundleV4 {
            statement,
            operation: KagemushaRecursiveSpendOperationVectorV4 {
                limbs: operation_limbs,
            },
            recursive_proof: KagemushaRecursiveSpendProofV4 {
                verifier_key_id,
                public_statement_digest,
                proof_envelope,
            },
        };
        bundle
            .validate_public_binding()
            .expect("recipient bundle binding");
        bundle
    }

    fn receiver_acknowledgement(
        receiver_key: &SigningKey,
        request: &KagemushaRecipientPaymentRequestV2,
        bundle: &KagemushaRecursiveSpendBundleV4,
        accepted_at_ms: u64,
    ) -> KagemushaReceiverAcknowledgementV2 {
        let KagemushaRecursiveSpendTransitionV4::PeerSplit(transition) = bundle
            .statement
            .transition
            .as_ref()
            .expect("peer-split transition")
        else {
            panic!("recipient bundle must carry a peer-split transition")
        };
        let payload = KagemushaReceiverAcknowledgementPayloadV2 {
            operation_id: transition.operation_id,
            recipient_request_digest: request.digest().expect("request digest"),
            payment_bundle_digest: bundle.digest().expect("bundle digest"),
            recipient_commitment: request.recipient_output().note_commitment,
            accepted_at_ms,
            receiver_device_id: request.receiver_device_id().to_owned(),
            receiver_key_reference: kagemusha_receiver_key_reference_v2(
                request.receiver_public_key(),
            )
            .expect("receiver key reference"),
            receiver_public_key: *request.receiver_public_key(),
        };
        let signature = sign(
            receiver_key,
            &payload
                .signing_bytes()
                .expect("acknowledgement signing bytes"),
        );
        KagemushaReceiverAcknowledgementV2 { payload, signature }
    }

    #[test]
    fn device_public_key_accepts_only_canonical_uncompressed_p256() {
        let key = signing_key(7);
        let canonical = key.verifying_key().to_encoded_point(false);
        let parsed =
            KagemushaDevicePublicKeyV2::from_sec1_bytes(canonical.as_bytes()).expect("valid key");
        parsed.validate().expect("decoded key revalidates");
        assert_eq!(parsed.as_sec1_bytes().as_slice(), canonical.as_bytes());

        for malformed in [
            Vec::new(),
            canonical.as_bytes()[..64].to_vec(),
            [canonical.as_bytes(), &[0_u8]].concat(),
            key.verifying_key()
                .to_encoded_point(true)
                .as_bytes()
                .to_vec(),
            vec![0_u8; 65],
        ] {
            assert!(
                KagemushaDevicePublicKeyV2::from_sec1_bytes(&malformed).is_err(),
                "malformed key unexpectedly accepted: {} bytes",
                malformed.len()
            );
        }

        let mut wrong_prefix = canonical.as_bytes().to_vec();
        wrong_prefix[0] = 0x06;
        assert!(KagemushaDevicePublicKeyV2::from_sec1_bytes(&wrong_prefix).is_err());

        let mut off_curve = canonical.as_bytes().to_vec();
        off_curve[64] ^= 0x02;
        assert!(KagemushaDevicePublicKeyV2::from_sec1_bytes(&off_curve).is_err());

        assert_eq!(
            norito::codec::Encode::encode(&parsed),
            canonical.as_bytes(),
            "the key newtype must be wire-transparent"
        );

        // Invalid points are rejected by serialization and deserialization,
        // not merely by higher-level request validation.
        let malformed = KagemushaDevicePublicKeyV2([0_u8; 65]);
        assert!(to_bytes(&malformed).is_err());
        let mut malformed_bytes = &[0_u8; 65][..];
        assert!(
            <KagemushaDevicePublicKeyV2 as norito::codec::Decode>::decode(&mut malformed_bytes)
                .is_err()
        );
    }

    #[test]
    fn device_signature_rejects_bad_width_scalars_and_high_s() {
        for malformed in [vec![], vec![0_u8; 63], vec![0_u8; 65]] {
            assert!(KagemushaDeviceSignatureV2::from_raw_bytes(&malformed).is_err());
        }
        let der = P256Signature::from_slice(&scalar_pair(one(), one()))
            .expect("valid scalar pair")
            .to_der();
        assert!(KagemushaDeviceSignatureV2::from_raw_bytes(der.as_bytes()).is_err());
        assert!(KagemushaDeviceSignatureV2::from_raw_bytes(&scalar_pair([0; 32], one())).is_err());
        assert!(KagemushaDeviceSignatureV2::from_raw_bytes(&scalar_pair(one(), [0; 32])).is_err());
        assert!(
            KagemushaDeviceSignatureV2::from_raw_bytes(&scalar_pair(P256_ORDER, one())).is_err()
        );
        assert!(
            KagemushaDeviceSignatureV2::from_raw_bytes(&scalar_pair(one(), P256_ORDER)).is_err()
        );
        let mut high_s = P256_HALF_ORDER;
        high_s[31] += 1;
        let high_s = scalar_pair(one(), high_s);
        assert!(KagemushaDeviceSignatureV2::from_raw_bytes(&high_s).is_err());

        let malformed = KagemushaDeviceSignatureV2(high_s);
        assert!(to_bytes(&malformed).is_err());
        let mut malformed_bytes = malformed.0.as_slice();
        assert!(
            <KagemushaDeviceSignatureV2 as norito::codec::Decode>::decode(&mut malformed_bytes)
                .is_err()
        );

        let valid = KagemushaDeviceSignatureV2::from_raw_bytes(&scalar_pair(one(), one()))
            .expect("valid low-S signature");
        assert_eq!(
            norito::codec::Encode::encode(&valid),
            scalar_pair(one(), one()),
            "the signature newtype must be wire-transparent"
        );
    }

    #[test]
    fn ecdsa_sha256_verification_is_key_and_message_bound() {
        let key = signing_key(9);
        let wrong_key = signing_key(10);
        let public_key = device_public_key(&key);
        let message = b"kagemusha fixed P-256 authority";
        let signature = sign(&key, message);
        signature
            .verify(&public_key, message)
            .expect("valid signature");
        assert!(
            signature
                .verify(&public_key, b"substituted message")
                .is_err()
        );
        assert!(
            signature
                .verify(&device_public_key(&wrong_key), message)
                .is_err()
        );
    }

    #[test]
    fn recipient_payment_request_expiry_is_exclusive() {
        let issued_at_ms = 1_800_000_000_000;
        let expires_at_ms = issued_at_ms + 30_000;
        let request = recipient_payment_request(&signing_key(11), issued_at_ms, expires_at_ms);

        request
            .validate_at(issued_at_ms)
            .expect("request is valid at issuance");
        request
            .validate_at(expires_at_ms - 1)
            .expect("request is valid immediately before expiry");
        assert!(request.validate_at(issued_at_ms - 1).is_err());
        assert!(request.validate_at(expires_at_ms).is_err());
        assert!(request.validate_at(expires_at_ms + 1).is_err());
    }

    #[test]
    fn receiver_acknowledgement_expiry_is_exclusive() {
        let issued_at_ms = 1_800_000_000_000;
        let expires_at_ms = issued_at_ms + 30_000;
        let receiver_key = signing_key(12);
        let request = recipient_payment_request(&receiver_key, issued_at_ms, expires_at_ms);
        let bundle = recipient_payment_bundle(&request);

        receiver_acknowledgement(&receiver_key, &request, &bundle, expires_at_ms - 1)
            .validate_for_payment_v4(&request, &bundle)
            .expect("acknowledgement is valid immediately before expiry");
        assert!(
            receiver_acknowledgement(&receiver_key, &request, &bundle, expires_at_ms)
                .validate_for_payment_v4(&request, &bundle)
                .is_err(),
            "acknowledgement at the exclusive expiry must fail closed",
        );
    }

    #[test]
    fn online_android_assertion_binds_every_authorization_coordinate_and_key() {
        let key = signing_key(31);
        let wrong_key = signing_key(32);
        let authorization = authorization(&key, None);
        let public_key = key.verifying_key().to_encoded_point(false);
        authorization
            .validate_for_payload(authorization.payload_digest)
            .expect("valid authorization structure");
        authorization
            .verify_hardware_signature(public_key.as_bytes())
            .expect("exact registered key verifies");
        assert!(
            authorization
                .verify_hardware_signature(
                    wrong_key.verifying_key().to_encoded_point(false).as_bytes(),
                )
                .is_err(),
            "a substituted assertion key must fail",
        );

        let mut mutations = Vec::new();
        let mut changed = authorization.clone();
        changed.authority = account(22);
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.device_id = "hardware-device-22".to_owned();
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.asset_definition_id = asset("other_cash");
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.operation_id = [0x31; 32];
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.issued_at_ms += 1;
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.expires_at_ms += 1;
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.nonce = [0x32; 32];
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.payload_digest = [0x33; 32];
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.registration_hash = [0x34; 32];
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.hardware_assertion = KagemushaOnlineHardwareAssertionV1::IosAppAttest(
            KagemushaIosAppAttestHardwareAssertionV1 {
                authenticator_data: vec![0; 37],
                signature: match &authorization.hardware_assertion {
                    KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(assertion) => {
                        assertion.signature
                    }
                    KagemushaOnlineHardwareAssertionV1::IosAppAttest(_) => unreachable!(),
                },
            },
        );
        mutations.push(changed);

        for mutation in mutations {
            assert!(
                mutation
                    .verify_hardware_signature(public_key.as_bytes())
                    .is_err(),
                "every account/device/asset/platform/hash/time/operation coordinate is signed",
            );
        }
    }

    #[test]
    fn online_ios_assertion_binds_authenticator_data_and_client_data_hash() {
        let key = signing_key(41);
        let mut authenticator_data = vec![0_u8; 37];
        authenticator_data[..32].copy_from_slice(&[0x41; 32]);
        authenticator_data[36] = 1;
        let authorization = authorization(&key, Some(authenticator_data));
        let public_key = key.verifying_key().to_encoded_point(false);
        authorization
            .verify_hardware_signature(public_key.as_bytes())
            .expect("exact App Attest assertion verifies");

        let mut changed_counter = authorization.clone();
        let KagemushaOnlineHardwareAssertionV1::IosAppAttest(assertion) =
            &mut changed_counter.hardware_assertion
        else {
            unreachable!()
        };
        assertion.authenticator_data[36] = 2;
        assert!(
            changed_counter
                .verify_hardware_signature(public_key.as_bytes())
                .is_err(),
            "the signature must bind the exact authenticatorData counter",
        );

        let mut wrong_length = authorization;
        let KagemushaOnlineHardwareAssertionV1::IosAppAttest(assertion) =
            &mut wrong_length.hardware_assertion
        else {
            unreachable!()
        };
        assertion.authenticator_data.truncate(36);
        assert!(
            wrong_length
                .validate_for_payload(wrong_length.payload_digest)
                .is_err(),
            "truncated assertion authData must fail at typed ingress",
        );
    }
}

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
        Ok(to_bytes(
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
        let encoded_len = to_bytes(self)?.len();
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
        Ok(to_bytes(self)?)
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
    chain_id: ChainId,
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
            chain_id: self.chain_id.clone(),
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
            || self.current_note.chain_id != self.chain_id
            || self.current_note.asset != *self.asset.definition()
            || self.initial_root == [0; 32]
            || self.finalized_root == [0; 32]
            || self.initial_root == self.finalized_root
            || self.shield_leaf_index >= KAGEMUSHA_TOPUP_SHIELD_TREE_CAPACITY_V2
            || self.topup_operation_id == [0; 32]
            || self.shield_verifier_id.backend.is_empty()
            || self.shield_verifier_id.name.is_empty()
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
        let encoded_len = to_bytes(self)?.len();
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
            || self.topup_finality_roster_artifact.chain_id != self.topup_anchor.chain_id
            || self.topup_finality_roster_artifact.artifact_generation
                != self.artifact_binding.generation
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "init_request.v4",
            });
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendSplitIntentV4 {
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
        if self.inputs.is_empty()
            || self.inputs.len() > KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
            || self.asset_scale != self.transfer_amount.scale
            || self.recipient_request_digest == [0; 32]
            || self.operation_id == [0; 32]
            || self.recipient_output.chain_id != self.chain_id
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
                || input.input_note.chain_id != self.chain_id
                || input.input_note.asset != self.asset
                || input.input_note.amount.scale != self.asset_scale
                || input.input_root == [0; 32]
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
                if change.chain_id != self.chain_id
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
        kagemusha_poseidon_preimage(&KagemushaRecursiveSpendSplitBindingDigestPreimageV4 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_SPLIT_BINDING_DIGEST_DOMAIN_V4.to_owned(),
            split: self.clone(),
        })
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
        let binding = self.binding_digest()?;
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
        if self.input_note.chain_id != self.chain_id {
            return Err(KagemushaValidationError::RecursiveSpendChainMismatch);
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
            || self.unshield_public_inputs.chain_tag == zero
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
                if change.chain_id != self.chain_id {
                    return Err(KagemushaValidationError::RecursiveSpendChainMismatch);
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
        if self.current_note.chain_id != self.chain_id
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
            None if self.proof_step_count == 1 && self.peer_hop_count == 0 => {}
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

    #[test]
    fn abi21_lifecycle_digest_domains_are_distinct_from_v2() {
        let v4 = [
            KAGEMUSHA_TOPUP_PAYLOAD_DIGEST_DOMAIN_V4,
            KAGEMUSHA_TOPUP_ANCHOR_DIGEST_DOMAIN_V4,
            KAGEMUSHA_RECURSIVE_SPEND_SPLIT_BINDING_DIGEST_DOMAIN_V4,
            KAGEMUSHA_REDEMPTION_TRANSITION_DIGEST_DOMAIN_V4,
            KAGEMUSHA_RECURSIVE_SPEND_PUBLIC_STATEMENT_DIGEST_DOMAIN_V4,
        ];
        assert_eq!(
            v4.into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            v4.len()
        );
        assert_ne!(
            KAGEMUSHA_TOPUP_PAYLOAD_DIGEST_DOMAIN_V4,
            KAGEMUSHA_TOPUP_PAYLOAD_DIGEST_DOMAIN_V2
        );
        assert_ne!(
            KAGEMUSHA_TOPUP_ANCHOR_DIGEST_DOMAIN_V4,
            KAGEMUSHA_TOPUP_ANCHOR_DIGEST_DOMAIN_V2
        );
        assert_ne!(
            KAGEMUSHA_RECURSIVE_SPEND_SPLIT_BINDING_DIGEST_DOMAIN_V4,
            KAGEMUSHA_RECURSIVE_SPEND_SPLIT_BINDING_DIGEST_DOMAIN_V2
        );
        assert_ne!(
            KAGEMUSHA_REDEMPTION_TRANSITION_DIGEST_DOMAIN_V4,
            KAGEMUSHA_REDEMPTION_TRANSITION_DIGEST_DOMAIN_V2
        );
        assert_ne!(
            KAGEMUSHA_RECURSIVE_SPEND_PUBLIC_STATEMENT_DIGEST_DOMAIN_V4,
            KAGEMUSHA_RECURSIVE_SPEND_PUBLIC_STATEMENT_DIGEST_DOMAIN_V2
        );
    }

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
    /// Require all 135 encoded Pallas elements to be canonical and the row to
    /// be non-empty. Comparison is exact on little-endian limbs and performs no
    /// modular reduction.
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
        let encoded_len = to_bytes(self)?.len();
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
            || self.bundle.statement.chain_id != anchor.chain_id
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
                || statement.chain_id != self.split.chain_id
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
        if statement.chain_id != self.chain_id
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
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the source value is invalid or the requested canonical result cannot be derived.
    pub fn operation_id(&self) -> Result<[u8; 32], KagemushaValidationError> {
        Ok(self.recipient_split_transition()?.operation_id)
    }

    /// Return the receiver-request digest bound by the recipient transition.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the source value is invalid or the requested canonical result cannot be derived.
    pub fn recipient_request_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        Ok(self.recipient_split_transition()?.recipient_request_digest)
    }

    /// Validate the recipient branch, membership state, and ABI-21 peer-size ceiling.
    ///
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
        let encoded_len = to_bytes(self)?.len();
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
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.topup_anchor.validate_public_binding()?;
        self.topup_finality_proof.validate_structure()?;
        let anchor_ref = self.topup_anchor.compact_ref()?;
        let anchor_len = to_bytes(&self.topup_anchor)?.len();
        let proof_len = to_bytes(&self.topup_finality_proof)?.len();
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
        let roster_len = to_bytes(&self.topup_finality_roster_artifact)?.len();
        let provenance_len = to_bytes(self)?.len();
        if block_height == Some(0)
            || roster_len == 0
            || roster_len > KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_USIZE_V2
            || provenance_len == 0
            || provenance_len > KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4
            || self.topup_finality_roster_artifact.chain_id != statement.chain_id
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
            let height_context = &evidence.topup_finality_proof.commit_qc.height_context;
            let finality_height_matches_anchor = height_context.height == anchor.finalized_height;
            let window = self
                .topup_finality_roster_artifact
                .window_at(anchor.finalized_height)?;
            if anchor.chain_id != statement.chain_id
                || anchor.asset.definition() != &statement.asset
                || anchor.asset_scale != statement.asset_scale
                || anchor.artifact_binding != statement.artifact_binding
                || block_height.is_some_and(|height| anchor.finalized_height > height)
                || height_context.chain_id != statement.chain_id
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

#[cfg(test)]
mod kagemusha_v4_topup_provenance_tests {
    use iroha_crypto::HashOf;

    use super::*;
    use crate::{
        block::consensus_v2::{BlockSubject, ConsensusRound, ExecutionCommitment},
        domain::DomainId,
        peer::PeerId,
    };

    struct Fixture {
        statement: KagemushaRecursiveSpendPublicStatementV4,
        provenance: KagemushaRecursiveSpendTopUpProvenanceV4,
    }

    fn execution_commitment(seed: u8) -> ExecutionCommitment {
        let ordinary_writes_root = Hash::new([seed, 3]);
        let topup_anchor_root = Hash::new([seed, 4]);
        ExecutionCommitment::new(
            Hash::new([seed, 1]),
            ExecutionCommitment::topup_post_state_root(1, ordinary_writes_root, topup_anchor_root),
            ordinary_writes_root,
            Some(topup_anchor_root),
            1,
            Hash::new([seed, 5]),
        )
        .expect("test execution commitment")
    }

    fn evidence(
        chain_id: &ChainId,
        asset: &AssetDefinitionId,
        binding: &KagemushaRecursiveSpendArtifactBindingV4,
        seed: u8,
    ) -> KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
        let payer_key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic payer key");
        let payer = AccountId::new(payer_key.public_key().clone());
        let amount = KagemushaScaledAmountV2::new(500, 2).expect("test amount");
        let anchor = KagemushaRecursiveSpendTopUpAnchorV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
            chain_id: chain_id.clone(),
            payer: payer.clone(),
            asset: AssetId::new(asset.clone(), payer),
            asset_scale: 2,
            amount,
            initial_root: [seed.wrapping_add(1); 32],
            finalized_root: [seed.wrapping_add(2); 32],
            shield_leaf_index: u32::from(seed),
            current_note: KagemushaSpendableNoteDescriptorV2 {
                chain_id: chain_id.clone(),
                asset: asset.clone(),
                note_commitment: [seed.wrapping_add(3); 32],
                spend_nullifier: [seed.wrapping_add(4); 32],
                amount,
            },
            topup_operation_id: [seed.wrapping_add(5); 32],
            shield_verifier_id: VerifyingKeyId::new("halo2/ipa", "topup-shield-v2"),
            shield_verifier_commitment: [seed.wrapping_add(6); 32],
            artifact_binding: binding.clone(),
            finalized_height: 42,
            finalized_tx_hash: [seed.wrapping_add(7); 32],
            anchor_digest: [0; 32],
        }
        .finalize_digest()
        .expect("test anchor");
        let context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new([seed, 8])));
        let round = ConsensusRound {
            context_id,
            height: anchor.finalized_height,
            view: 0,
        };
        let certificate = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject: BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 9])),
                payload_hash: Hash::new([seed, 10]),
            },
            execution_commitment: execution_commitment(seed),
            signers: vec![0],
            aggregate_signature: vec![seed; 96],
        };
        let proof = KagemushaTopUpFinalityProofV2 {
            version: KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
            anchor: anchor.compact_ref().expect("test anchor ref"),
            commit_qc: KagemushaTopUpFinalityCompactQcV2 {
                height_context: KagemushaTopUpFinalityHeightContextV2 {
                    context_id,
                    chain_id: chain_id.clone(),
                    protocol_version: PROTOCOL_VERSION,
                    height: anchor.finalized_height,
                    epoch: 0,
                    epoch_end_height: 100,
                    next_epoch_snapshot: None,
                    mode: ConsensusMode::Permissioned,
                    parent_commit_qc: None,
                    snapshot_bootstrap: None,
                    nexus_amx_context_hash: Hash::new([seed, 11]),
                    da_layout: DataAvailabilityLayout {
                        encoding: crate::block::consensus_v2::PayloadEncoding::Plain,
                        chunk_size_bytes: 1024,
                        data_shards: 0,
                        parity_shards: 0,
                        max_payload_size_bytes: 4096,
                        max_chunk_count: 4,
                    },
                    leader_seed: [seed.wrapping_add(12); 32],
                },
                certificate,
            },
            anchor_path: KagemushaTopUpAnchorMerkleProofV2 {
                leaf_index: 0,
                leaf_count: 1,
                siblings: Vec::new(),
            },
        };
        KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
            topup_anchor: anchor,
            topup_finality_proof: proof,
        }
    }

    fn fixture_with_seeds(seeds: &[u8]) -> Fixture {
        let chain_id = ChainId::from("kagemusha-provenance-test-chain");
        let asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("test domain"),
            "rose".parse().expect("test asset name"),
        );
        let binding = KagemushaRecursiveSpendArtifactBindingV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: "provenance-test-release".to_owned(),
            manifest_sha256: [0x51; 32],
        };
        let validator_key = KeyPair::try_from_seed(vec![0x61; 32], Algorithm::BlsNormal)
            .expect("deterministic validator key");
        let roster = KagemushaTopUpFinalityRosterArtifactV2 {
            version: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
            chain_id: chain_id.clone(),
            artifact_generation: binding.generation.clone(),
            windows: vec![KagemushaTopUpFinalityRosterWindowV2 {
                activates_at_height: 1,
                withdraws_at_height: 100,
                consensus_mode: ConsensusMode::Permissioned,
                validator_set: vec![ValidatorPower {
                    validator: PeerId::new(validator_key.public_key().clone()),
                    power: 1,
                }],
                validator_set_pops: vec![[0x62; 96]],
            }],
        };
        let mut evidence = seeds
            .iter()
            .map(|seed| evidence(&chain_id, &asset, &binding, *seed))
            .collect::<Vec<_>>();
        evidence.sort_unstable_by_key(|item| item.topup_anchor.compact_ref().expect("anchor ref"));
        let topup_anchor_refs = evidence
            .iter()
            .map(|item| item.topup_anchor.compact_ref().expect("anchor ref"))
            .collect();
        let statement = KagemushaRecursiveSpendPublicStatementV4 {
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            asset_scale: 2,
            final_root: [0x71; 32],
            next_zero_leaf_index: 7,
            topup_anchor_refs,
            proof_step_count: 2,
            peer_hop_count: 1,
            current_note: KagemushaSpendableNoteDescriptorV2 {
                chain_id,
                asset,
                note_commitment: [0x72; 32],
                spend_nullifier: [0x73; 32],
                amount: KagemushaScaledAmountV2::new(400, 2).expect("branch amount"),
            },
            branch_claims: Vec::new(),
            transition: None,
            artifact_binding: binding.clone(),
            verifier_key_id: kagemusha_recursive_spend_verifier_key_id_v4(
                KagemushaPastaCycleParityV1::StepEq,
                binding.manifest_sha256,
            ),
        };
        Fixture {
            statement,
            provenance: KagemushaRecursiveSpendTopUpProvenanceV4 {
                topup_finality_roster_artifact: roster,
                topup_finality_evidence: evidence,
            },
        }
    }

    fn rejects(
        fixture: &Fixture,
        provenance: &KagemushaRecursiveSpendTopUpProvenanceV4,
        height: u64,
    ) {
        assert!(
            provenance
                .validate_for_statement_at_height(&fixture.statement, Some(height))
                .is_err()
        );
    }

    #[test]
    fn provenance_rejects_zero_many_duplicate_reordered_and_wrong_refs() {
        let fixture = fixture_with_seeds(&[0x11, 0x21]);
        fixture
            .provenance
            .validate_for_statement_at_height(&fixture.statement, Some(50))
            .expect("canonical two-origin provenance");

        let mut zero = fixture.provenance.clone();
        zero.topup_finality_evidence.clear();
        rejects(&fixture, &zero, 50);

        let mut many = fixture.provenance.clone();
        many.topup_finality_evidence
            .push(fixture.provenance.topup_finality_evidence[0].clone());
        rejects(&fixture, &many, 50);

        let mut duplicate = fixture.provenance.clone();
        duplicate.topup_finality_evidence[1] = duplicate.topup_finality_evidence[0].clone();
        rejects(&fixture, &duplicate, 50);

        let mut reordered = fixture.provenance.clone();
        reordered.topup_finality_evidence.reverse();
        rejects(&fixture, &reordered, 50);

        let mut wrong_ref_fixture = fixture_with_seeds(&[0x11]);
        wrong_ref_fixture.statement.topup_anchor_refs[0] =
            KagemushaRecursiveSpendTopUpAnchorRefV2 {
                topup_operation_id: [0xe1; 32],
                anchor_digest: [0xe2; 32],
            };
        rejects(&wrong_ref_fixture, &wrong_ref_fixture.provenance, 50);
    }

    #[test]
    fn provenance_rejects_wrong_context_binding_window_height_qc_and_size() {
        let fixture = fixture_with_seeds(&[0x31]);

        let mut wrong_chain = fixture.statement.clone();
        wrong_chain.chain_id = ChainId::from("wrong-chain");
        assert!(
            fixture
                .provenance
                .validate_for_statement_at_height(&wrong_chain, Some(50))
                .is_err()
        );

        let mut wrong_asset = fixture.statement.clone();
        wrong_asset.asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("test domain"),
            "wrong".parse().expect("test asset name"),
        );
        assert!(
            fixture
                .provenance
                .validate_for_statement_at_height(&wrong_asset, Some(50))
                .is_err()
        );

        let mut wrong_scale = fixture.statement.clone();
        wrong_scale.asset_scale = 3;
        assert!(
            fixture
                .provenance
                .validate_for_statement_at_height(&wrong_scale, Some(50))
                .is_err()
        );

        let mut wrong_binding = fixture.statement.clone();
        wrong_binding.artifact_binding.manifest_sha256[0] ^= 1;
        assert!(
            fixture
                .provenance
                .validate_for_statement_at_height(&wrong_binding, Some(50))
                .is_err()
        );

        let mut wrong_generation = fixture.provenance.clone();
        wrong_generation
            .topup_finality_roster_artifact
            .artifact_generation = "other-release".to_owned();
        rejects(&fixture, &wrong_generation, 50);

        let mut wrong_window = fixture.provenance.clone();
        wrong_window.topup_finality_roster_artifact.windows[0].withdraws_at_height = 42;
        rejects(&fixture, &wrong_window, 50);

        rejects(&fixture, &fixture.provenance, 41);

        let mut wrong_qc = fixture.provenance.clone();
        wrong_qc.topup_finality_evidence[0]
            .topup_finality_proof
            .commit_qc
            .height_context
            .height = 43;
        rejects(&fixture, &wrong_qc, 50);

        let mut oversized = fixture.provenance.clone();
        let mut parent_qc = oversized.topup_finality_evidence[0]
            .topup_finality_proof
            .commit_qc
            .certificate
            .clone();
        parent_qc.aggregate_signature =
            vec![0x81; KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_USIZE_V2 + 1];
        oversized.topup_finality_evidence[0]
            .topup_finality_proof
            .commit_qc
            .height_context
            .parent_commit_qc = Some(parent_qc);
        rejects(&fixture, &oversized, 50);
    }

    #[test]
    fn compact_qc_rejects_foreign_or_future_proposal_origin() {
        let fixture = fixture_with_seeds(&[0x32]);
        let compact_qc = &fixture.provenance.topup_finality_evidence[0]
            .topup_finality_proof
            .commit_qc;
        compact_qc
            .validate_structure()
            .expect("fixture compact QC structure");

        let mut future = compact_qc.clone();
        future.certificate.proposal_round.view = future.certificate.round.view.saturating_add(1);
        assert!(future.validate_structure().is_err());

        let mut foreign_context = compact_qc.clone();
        foreign_context.certificate.proposal_round.context_id = HeightContextId(
            HashOf::from_untyped_unchecked(Hash::new(b"foreign compact QC proposal context")),
        );
        assert!(foreign_context.validate_structure().is_err());

        let mut foreign_height = compact_qc.clone();
        foreign_height.certificate.proposal_round.height =
            foreign_height.certificate.round.height.saturating_add(1);
        assert!(foreign_height.validate_structure().is_err());
    }

    #[test]
    fn provenance_merge_requires_one_exact_roster_and_exact_shared_evidence() {
        let left = fixture_with_seeds(&[0x11]);
        let right = fixture_with_seeds(&[0x21]);
        let merged = KagemushaRecursiveSpendTopUpProvenanceV4::merge_for_statements_at(
            &[
                (&left.statement, &left.provenance),
                (&right.statement, &right.provenance),
            ],
            50,
        )
        .expect("two exact inventories merge canonically");
        assert_eq!(merged.topup_finality_evidence.len(), 2);
        assert!(
            merged
                .anchor_refs()
                .expect("merged refs")
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );

        let mut wrong_roster = right.provenance.clone();
        wrong_roster.topup_finality_roster_artifact.windows[0].validator_set_pops[0][0] ^= 1;
        assert!(
            KagemushaRecursiveSpendTopUpProvenanceV4::merge_for_statements_at(
                &[
                    (&left.statement, &left.provenance),
                    (&right.statement, &wrong_roster),
                ],
                50,
            )
            .is_err()
        );

        let shared = fixture_with_seeds(&[0x41]);
        let coalesced = KagemushaRecursiveSpendTopUpProvenanceV4::merge_for_statements_at(
            &[
                (&shared.statement, &shared.provenance),
                (&shared.statement, &shared.provenance),
            ],
            50,
        )
        .expect("identical shared origin is coalesced");
        assert_eq!(coalesced.topup_finality_evidence.len(), 1);

        let mut conflicting = shared.provenance.clone();
        conflicting.topup_finality_evidence[0]
            .topup_finality_proof
            .commit_qc
            .certificate
            .aggregate_signature[0] ^= 1;
        assert!(
            KagemushaRecursiveSpendTopUpProvenanceV4::merge_for_statements_at(
                &[
                    (&shared.statement, &shared.provenance),
                    (&shared.statement, &conflicting),
                ],
                50,
            )
            .is_err()
        );
    }
}

impl KagemushaRecursiveSpendVerifyRequestV4 {
    /// Validate the terminal receiver request and every V4 proof/provenance binding.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.bundle.validate_public_binding()?;
        self.artifact_binding.validate()?;
        self.recipient_request.validate_at(self.verified_at_ms)?;
        let statement = &self.bundle.statement;
        if self.verified_at_ms == 0
            || self.block_height == 0
            || self.maximum_hops == 0
            || self.maximum_hops > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
            || statement.peer_hop_count > self.maximum_hops
            || self.artifact_binding != statement.artifact_binding
            || self.recipient_request.chain_id != statement.chain_id
            || self.recipient_request.asset != statement.asset
            || self.recipient_request.amount.scale != statement.asset_scale
            || statement.current_note != self.recipient_request.recipient_output
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.v4",
            });
        }
        self.topup_provenance
            .validate_for_bundle_at(&self.bundle, self.block_height)?;
        let Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(transition)) =
            statement.transition.as_ref()
        else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.v4.transition",
            });
        };
        if transition.branch != KagemushaRecursiveSpendBranchV2::Recipient
            || transition.recipient_request_digest != self.recipient_request.digest()?
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.v4.recipient_binding",
            });
        }
        let encoded_len = to_bytes(self)?.len();
        if encoded_len > KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_MAX_BYTES_V4 {
            return Err(KagemushaValidationError::EncodedSizeExceeded {
                actual: encoded_len,
                max: KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_MAX_BYTES_V4,
            });
        }
        Ok(())
    }

    /// Return the V4-domain binding of request, exact output note, and opaque bundle.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn request_output_binding_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRequestOutputBindingDigestPreimageV4 {
            domain: KAGEMUSHA_REQUEST_OUTPUT_BINDING_DIGEST_DOMAIN_V4.to_owned(),
            recipient_request_digest: self.recipient_request.digest()?,
            recipient_output: self.bundle.statement.current_note.clone(),
            bundle_digest: self.bundle.digest()?,
        })
    }
}

impl KagemushaRecursiveSpendVerifyResultV4 {
    /// Enforce the single successful ABI-21 receiver-acceptance contract.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.summary.amount.validate()?;
        self.summary.artifact_binding.validate()?;
        let expected_verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            self.summary.artifact_binding.manifest_sha256,
        );
        validate_kagemusha_recursive_spend_branch_claims_v2(&self.summary.branch_claims)?;
        let activation_window_valid = matches!(
            (self.verifier_activation_height, self.verifier_withdraw_height),
            (Some(activation), Some(withdrawal))
                if activation > 0
                    && activation < withdrawal
                    && self.verified_at_block_height >= activation
                    && self.verified_at_block_height < withdrawal
        );
        if !self.valid
            || !self.chain_admissible
            || !self.lineage_redeemable
            || !self.witnessless_redemption_supported
            || self.recipient_request_digest == [0; 32]
            || self.request_output_binding_digest == [0; 32]
            || self.summary.bundle_digest == [0; 32]
            || self.verifier_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4
            || self.verified_at_block_height == 0
            || self.verified_at_ms == 0
            || !activation_window_valid
            || self.summary.verifier_key_id != self.verifier_key_id
            || self.verifier_key_id != expected_verifier_key_id
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_result.v4",
            });
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendRedeemBuildRequestV4 {
    /// Validate the common full/partial ABI-21 redemption-builder input.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.bundle.validate_public_binding()?;
        self.redemption.validate_public_binding()?;
        self.public_amount.validate()?;
        validate_kagemusha_redeem_proof_attachment_v2(&self.unshield_proof)?;
        let statement = &self.bundle.statement;
        if self.block_height == 0
            || self.operation_id == [0; 32]
            || self.operation_id != self.redemption.operation_id
            || self.recipient != self.redemption.recipient
            || self.public_amount != self.redemption.public_amount
            || self.redemption.chain_id != statement.chain_id
            || self.redemption.asset != statement.asset
            || self.redemption.parent_bundle_digest != self.bundle.digest()?
            || self.redemption.input_note != statement.current_note
            || self.redemption.input_root != statement.final_root
            || self.redemption.parent_branch_claims != statement.branch_claims
            || self.redemption.parent_topup_anchor_refs != statement.topup_anchor_refs
            || self.redemption.parent_proof_step_count != statement.proof_step_count
            || self.redemption.parent_peer_hop_count != statement.peer_hop_count
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_request.v4",
            });
        }
        match (
            &self.redemption.change_output,
            &self.redemption.change_artifact_binding,
        ) {
            (None, None) => Ok(()),
            (Some(_), Some(binding)) if binding == &statement.artifact_binding => Ok(()),
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_request.v4.change",
            }),
        }
    }
}

impl KagemushaRecursiveSpendRedeemChangeBranchV4 {
    /// Validate the sole continuing child of a partial redemption.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_redemption(
        &self,
        input_bundle: &KagemushaRecursiveSpendBundleV4,
        redemption: &KagemushaRecursiveSpendRedemptionIntentV4,
    ) -> Result<(), KagemushaValidationError> {
        input_bundle.validate_public_binding()?;
        redemption.validate_public_binding()?;
        self.output.validate_public_binding()?;
        self.bundle.validate_public_binding()?;
        let expected_output = redemption.change_output.as_ref().ok_or(
            KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "offline_change.v4",
            },
        )?;
        let expected_binding = redemption.change_artifact_binding.as_ref().ok_or(
            KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "offline_change.v4",
            },
        )?;
        let expected_claims = redemption.change_branch_claims()?;
        if &self.output != expected_output
            || self.branch_claims != expected_claims
            || self.bundle.statement.current_note != self.output
            || self.bundle.statement.branch_claims != self.branch_claims
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "offline_change.v4.branch",
            });
        }
        let Some(KagemushaRecursiveSpendTransitionV4::RedemptionChange(transition)) =
            self.bundle.statement.transition.as_ref()
        else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_change.v4.transition",
            });
        };
        if transition.binding_digest != redemption.binding_digest()?
            || transition.parent_bundle_digest != redemption.parent_bundle_digest
            || transition.operation_id != redemption.operation_id
            || transition.parent_proof_step_count != redemption.parent_proof_step_count
            || transition.parent_peer_hop_count != redemption.parent_peer_hop_count
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_change.v4.transition",
            });
        }
        let input = &input_bundle.statement;
        let change = &self.bundle.statement;
        let expected_verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            expected_binding.manifest_sha256,
        );
        if redemption.parent_bundle_digest != input_bundle.digest()?
            || redemption.input_note != input.current_note
            || redemption.parent_branch_claims != input.branch_claims
            || redemption.parent_topup_anchor_refs != input.topup_anchor_refs
            || redemption.parent_proof_step_count != input.proof_step_count
            || redemption.parent_peer_hop_count != input.peer_hop_count
            || redemption.input_root != input.final_root
            || change.chain_id != input.chain_id
            || change.asset != input.asset
            || change.asset_scale != input.asset_scale
            || change.final_root == input.final_root
            || input.next_zero_leaf_index.checked_add(1) != Some(change.next_zero_leaf_index)
            || change.topup_anchor_refs != input.topup_anchor_refs
            || input.proof_step_count == 0
            || input.proof_step_count.checked_add(1) != Some(change.proof_step_count)
            || change.peer_hop_count != input.peer_hop_count
            || &change.artifact_binding != expected_binding
            || change.verifier_key_id != expected_verifier_key_id
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_change.v4.parent_binding",
            });
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendRedeemUnsignedV4 {
    /// Validate exact full-terminal or partial-with-one-change redemption semantics.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.bundle.validate_public_binding()?;
        self.redemption.validate_public_binding()?;
        self.amount.validate()?;
        validate_kagemusha_redeem_proof_attachment_v2(&self.redeem_proof)?;
        let statement = &self.bundle.statement;
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4
            || self.block_height == 0
            || self.operation_id == [0; 32]
            || self.operation_id != self.redemption.operation_id
            || self.recipient != self.redemption.recipient
            || self.amount != self.redemption.public_amount
            || self.redemption.chain_id != statement.chain_id
            || self.redemption.asset != statement.asset
            || self.redemption.parent_bundle_digest != self.bundle.digest()?
            || self.redemption.input_note != statement.current_note
            || self.redemption.input_root != statement.final_root
            || self.redemption.parent_branch_claims != statement.branch_claims
            || self.redemption.parent_topup_anchor_refs != statement.topup_anchor_refs
            || self.redemption.parent_proof_step_count != statement.proof_step_count
            || self.redemption.parent_peer_hop_count != statement.peer_hop_count
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_request.v4",
            });
        }
        match (&self.redemption.change_output, &self.offline_change) {
            (None, None) => Ok(()),
            (Some(_), Some(change)) => {
                change.validate_for_redemption(&self.bundle, &self.redemption)
            }
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_request.v4.offline_change",
            }),
        }
    }

    /// Return the exact V4 authorization payload digest.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRedeemUnsignedPayloadDigestPreimageV4 {
            domain: KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V4.to_owned(),
            version: self.version,
            bundle: self.bundle.clone(),
            recipient: self.recipient.clone(),
            amount: self.amount,
            redeem_proof: self.redeem_proof.clone(),
            redemption: self.redemption.clone(),
            offline_change: self.offline_change.clone(),
            block_height: self.block_height,
            operation_id: self.operation_id,
        })
    }

    /// Attach the matching recipient authorization without altering any signed field.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn into_request(
        self,
        authorization: KagemushaRequestAuthorizationV2,
    ) -> Result<KagemushaRecursiveSpendRedeemRequestV4, KagemushaValidationError> {
        let request = KagemushaRecursiveSpendRedeemRequestV4 {
            version: self.version,
            bundle: self.bundle,
            recipient: self.recipient,
            amount: self.amount,
            redeem_proof: self.redeem_proof,
            redemption: self.redemption,
            offline_change: self.offline_change,
            block_height: self.block_height,
            operation_id: self.operation_id,
            authorization,
        };
        request.validate_public_binding()?;
        Ok(request)
    }
}

impl KagemushaRecursiveSpendRedeemBuildResultV4 {
    /// Validate the atomic unsigned request plus its optional change/witness package.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.unsigned.validate_public_binding()?;
        if self.operation_id == [0; 32]
            || self.operation_id != self.unsigned.operation_id
            || self.authorization_digest != self.unsigned.digest()?
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_result.v4",
            });
        }
        match (
            &self.unsigned.offline_change,
            &self.offline_change_bundle,
            &self.offline_change_membership_witness,
            &self.offline_change_topup_provenance,
        ) {
            (None, None, None, None) => Ok(()),
            (Some(branch), Some(bundle), Some(witness), Some(provenance))
                if &branch.bundle == bundle =>
            {
                branch.validate_for_redemption(&self.unsigned.bundle, &self.unsigned.redemption)?;
                witness.validate_for_statement_v4(&bundle.statement)?;
                provenance.validate_for_bundle(bundle)
            }
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_result.v4.offline_change",
            }),
        }
    }

    /// Validate this prepared result against the exact builder input.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_request(
        &self,
        request: &KagemushaRecursiveSpendRedeemBuildRequestV4,
    ) -> Result<(), KagemushaValidationError> {
        request.validate_public_binding()?;
        self.validate_public_binding()?;
        if self.operation_id != request.operation_id
            || self.unsigned.bundle != request.bundle
            || self.unsigned.recipient != request.recipient
            || self.unsigned.amount != request.public_amount
            || self.unsigned.redeem_proof != request.unshield_proof
            || self.unsigned.redemption != request.redemption
            || self.unsigned.block_height != request.block_height
            || self.offline_change_bundle.is_some() != request.redemption.change_output.is_some()
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_result.v4.request",
            });
        }
        Ok(())
    }

    /// Attach authorization and retain local change membership state.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn into_redeem_result(
        self,
        authorization: KagemushaRequestAuthorizationV2,
    ) -> Result<KagemushaRecursiveSpendRedeemResultV4, KagemushaValidationError> {
        self.validate_public_binding()?;
        let operation_id = self.operation_id;
        let offline_change_bundle = self.offline_change_bundle;
        let offline_change_membership_witness = self.offline_change_membership_witness;
        let offline_change_topup_provenance = self.offline_change_topup_provenance;
        let request = self.unsigned.into_request(authorization)?;
        let result = KagemushaRecursiveSpendRedeemResultV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            redeem_request_archive: to_bytes(&request)?,
            offline_change_bundle,
            offline_change_membership_witness,
            offline_change_topup_provenance,
            operation_id,
        };
        result.validate_public_binding()?;
        Ok(result)
    }
}

impl KagemushaRecursiveSpendRedeemRequestV4 {
    /// Reconstruct the exact canonical V4 fields covered by authorization.
    #[must_use]
    pub fn unsigned_payload(&self) -> KagemushaRecursiveSpendRedeemUnsignedV4 {
        KagemushaRecursiveSpendRedeemUnsignedV4 {
            version: self.version,
            bundle: self.bundle.clone(),
            recipient: self.recipient.clone(),
            amount: self.amount,
            redeem_proof: self.redeem_proof.clone(),
            redemption: self.redemption.clone(),
            offline_change: self.offline_change.clone(),
            block_height: self.block_height,
            operation_id: self.operation_id,
        }
    }

    /// Validate exact conservation and the self-contained recipient authorization.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        let encoded_len = to_bytes(self)?.len();
        ensure_kagemusha_encoded_size_at_most(
            encoded_len,
            KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4,
        )?;
        let unsigned = self.unsigned_payload();
        unsigned.validate_public_binding()?;
        if self.authorization.operation_id != self.operation_id
            || self.authorization.authority != self.recipient
            || self.authorization.asset_definition_id != self.bundle.statement.asset
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "authorization.v4",
            });
        }
        self.authorization.validate_for_payload(unsigned.digest()?)
    }

    /// Return the digest of every unsigned V4 redemption field.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn unsigned_payload_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.unsigned_payload().digest()
    }

    /// Verify recipient authorization at authoritative Torii time.
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

impl KagemushaRecursiveSpendRedeemResultV4 {
    /// Validate the canonical request archive and terminal/change result shape.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4
            || self.operation_id == [0; 32]
            || self.redeem_request_archive.is_empty()
            || self.redeem_request_archive.len()
                > KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_result.v4",
            });
        }
        let request: KagemushaRecursiveSpendRedeemRequestV4 =
            norito::decode_from_bytes(&self.redeem_request_archive).map_err(|_| {
                KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "redeem_result.v4.request_archive",
                }
            })?;
        request.validate_public_binding()?;
        if request.operation_id != self.operation_id {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_result.v4.operation_id",
            });
        }
        match (
            &request.offline_change,
            &self.offline_change_bundle,
            &self.offline_change_membership_witness,
            &self.offline_change_topup_provenance,
        ) {
            (None, None, None, None) => Ok(()),
            (Some(branch), Some(bundle), Some(witness), Some(provenance))
                if &branch.bundle == bundle =>
            {
                branch.validate_for_redemption(&request.bundle, &request.redemption)?;
                witness.validate_for_statement_v4(&bundle.statement)?;
                provenance.validate_for_bundle(bundle)
            }
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_result.v4.offline_change",
            }),
        }
    }
}

#[cfg(test)]
mod kagemusha_v4_lifecycle_additional_domain_tests {
    use super::*;

    #[test]
    fn abi21_bundle_request_and_redemption_domains_are_unique() {
        let v4 = [
            KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_DIGEST_DOMAIN_V4,
            KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V4,
            KAGEMUSHA_REQUEST_OUTPUT_BINDING_DIGEST_DOMAIN_V4,
        ];
        assert_eq!(
            v4.into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            v4.len()
        );
        assert_ne!(
            KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_DIGEST_DOMAIN_V4,
            KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_DIGEST_DOMAIN_V2
        );
        assert_ne!(
            KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V4,
            KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V2
        );
    }

    #[test]
    fn abi21_chain_request_size_caps_are_inclusive_and_fail_one_byte_over() {
        for maximum in [
            KAGEMUSHA_RECURSIVE_SPEND_TOPUP_REQUEST_MAX_BYTES_V4,
            KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4,
        ] {
            ensure_kagemusha_encoded_size_at_most(maximum, maximum)
                .expect("the exact canonical request limit is accepted");
            assert!(matches!(
                ensure_kagemusha_encoded_size_at_most(maximum + 1, maximum),
                Err(KagemushaValidationError::EncodedSizeExceeded { actual, max })
                    if actual == maximum + 1 && max == maximum
            ));
        }
    }
}
