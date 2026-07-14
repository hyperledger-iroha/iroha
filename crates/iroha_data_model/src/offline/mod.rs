//! Canonical Kagemusha offline-cash models.
//!
//! The module exposes one lifecycle: exact online top-up, recursive
//! offline split/spend, and exact online redemption.

use iroha_crypto::{Algorithm, Hash, KeyPair, PublicKey, Signature, SignatureOf};
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
    proof::{ProofAttachment, ProofBox, VerifyingKeyId},
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
/// including all 4,096 bounded PoPs plus maximum current and parent signer
/// lists. The exact maximum wire-shape test below pins the encoded size below
/// this 2 MiB ingress cap.
pub const KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_V2: u64 = 2 * 1024 * 1024;
/// Maximum canonical Norito bytes accepted for one complete validated top-up anchor.
pub const KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_V2: u64 = 64 * 1024;
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
        + KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2 as usize
        + KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
            * (KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_V2 as usize
                + KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_V2 as usize)
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
/// Domain separator for the field-neutral recursive continuing-state boundary.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:recursive-state-boundary:v1";
/// Shared verifier role id for confidential transfer evidence.
pub const KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2: &str = "confidential_transfer_v2_verifier_record";
/// Verifier role for public-to-confidential Kagemusha top-up shielding.
pub const KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2: &str =
    "kagemusha_topup_shield_v2_verifier_record";
/// Shared verifier role id for unshield evidence.
pub const KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2: &str = "confidential_unshield_v3_verifier_record";
/// Chain verifier role for the EqAffine/Vesta half of every recursive step.
pub const KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V3: &str =
    "kagemusha_recursive_step_eq_v3_verifier_record";
/// Chain verifier role for the EpAffine/Pallas half of every recursive step.
pub const KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V3: &str =
    "kagemusha_recursive_step_ep_v3_verifier_record";
/// Shared verifier purpose for top-up and offline split evidence.
pub const KAGEMUSHA_VERIFIER_PURPOSE_TRANSFER_V2: &str = "offline_split";
/// Verifier purpose for the public-to-confidential top-up transition.
pub const KAGEMUSHA_VERIFIER_PURPOSE_TOPUP_SHIELD_V2: &str = "online_to_offline_topup_shield";
/// Shared verifier purpose for offline-to-online redemption.
pub const KAGEMUSHA_VERIFIER_PURPOSE_UNSHIELD_V2: &str = "offline_to_online_redemption";
/// Shared verifier purpose for either half of a value-conserving recursive step.
pub const KAGEMUSHA_VERIFIER_PURPOSE_STEP_V3: &str = "kagemusha_recursive_spend_step";
/// Domain separator for the self-contained V2 request authorization signature.
pub const KAGEMUSHA_REQUEST_AUTHORIZATION_DOMAIN_V2: &str =
    "iroha:kagemusha:v2:request-authorization";
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
/// Native bridge ABI that first advertises the fail-closed Pasta-cycle V3 contract.
pub const KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3: u32 = 19;
/// Native bridge ABI for the degree-parameterized recursive-spend V4 contract.
///
/// V4 is deliberately not an alias for ABI 19: its public accumulator layout,
/// fold transcripts, key parsing parameters, and artifact framing all depend
/// on an authenticated IPA degree.
pub const KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 20;
/// Exact schema identifier for the production recursive-spend artifact manifest.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3: &str =
    "kagemusha.offline.recursive_spend.artifact_manifest.v3";
/// Exact schema identifier for the degree-parameterized artifact manifest.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4: &str =
    "kagemusha.offline.recursive_spend.artifact_manifest.v4";
/// Proof-system profile selected by the V3 recursive-spend release contract.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1: &str = "halo2/ipa-pasta-cycle-v1";
/// V3 release-metadata spelling for the exact V1 Pasta-cycle backend.
///
/// ABI 19 advertises a V3 artifact manifest, while the proof envelope and
/// backend profile remain the first exact-state wire contract.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3: &str =
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1;
/// Degree-parameterized Pasta-cycle backend selected only by ABI 20 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4: &str = "halo2/ipa-pasta-cycle-v4";
/// Poseidon transcript profile shared by both proofs in a Pasta-cycle pair.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1: &str =
    "kagemusha-pasta-cycle-poseidon-v1";
/// V3 release-metadata spelling for the exact V1 transcript profile.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V3: &str =
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1;
/// Transcript contract for V4 proofs and degree-sized BGH19 folds.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4: &str =
    "kagemusha-pasta-cycle-poseidon-v4";
/// Circuit id for the EqAffine/Vesta half of every recursive step.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1: &str =
    "kagemusha-recursive-spend-step-eq-two-parent-exact-state-v1";
/// Circuit id for the Eq half with public operation and compiled-protocol identity.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V2: &str =
    "kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2";
/// V3 release-metadata spelling for the current Eq circuit identifier.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3: &str =
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V2;
/// Eq/Fp recursive-step circuit with authenticated dynamic IPA layout.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4: &str =
    "kagemusha-recursive-spend-step-eq-authenticated-layout-v4";
/// Circuit id for the EpAffine/Pallas half of every recursive step.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1: &str =
    "kagemusha-recursive-spend-step-ep-two-parent-exact-state-v1";
/// Circuit id for the Ep half with public operation and compiled-protocol identity.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V2: &str =
    "kagemusha-recursive-spend-step-ep-two-parent-operation-protocol-v2";
/// V3 release-metadata spelling for the current Ep circuit identifier.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3: &str =
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V2;
/// Ep/Fq recursive-step circuit with authenticated dynamic IPA layout.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4: &str =
    "kagemusha-recursive-spend-step-ep-authenticated-layout-v4";
/// Verifying-key curve for the EqAffine recursive-step half.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V3: &str = "vesta";
/// Verifying-key curve for the EpAffine recursive-step half.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V3: &str = "pallas";
/// Number of canonical Pasta field elements exposed by one Step operation.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_OPERATION_FIELD_ELEMENTS_V3: usize = 135;
/// Exact `u32` limbs used by the eight-limb encoding of every operation element.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_OPERATION_LIMBS_V3: usize =
    KAGEMUSHA_RECURSIVE_SPEND_STEP_OPERATION_FIELD_ELEMENTS_V3 * 8;
/// Exact number of `u32` values in the current single-column Step ABI.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_PUBLIC_INPUT_LIMBS_V3: usize = 4_024;
/// Canonical field-neutral public inputs enforced by the EqAffine/Vesta step circuit.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PUBLIC_INPUTS_SCHEMA_V3: &[u8] = br#"{"schema":"kagemusha_recursive_spend_step_eq_two_parent_operation_protocol_v2","layout":"single_column_u32","limbs":4024,"state_vector":{"layout_version":1,"limbs":889,"parent_slots":2},"operation":{"field_elements":135,"limbs_per_element":8,"limbs":1080,"encoding":"canonical_pasta_scalar_le_u32"},"compiled_protocol_identity":{"wire_version":1,"hash":"sha256","limbs":8},"ipa_accumulator":{"wire_version":1,"rounds":12,"limbs":106},"public_inputs":["public_statement_digest_u32[8]","operation_u32[1080]","parent_count_u32","parent_states_u32[2][889]","result_state_u32[889]","manifest_sha256_u32[8]","step_eq_compiled_protocol_sha256_u32[8]","step_ep_compiled_protocol_sha256_u32[8]","parent_eq_lineage_accumulator_u32[106]","parent_ep_lineage_accumulator_u32[106]","parent_eq_deferred_sha256_u32[2][8]","parent_ep_deferred_sha256_u32[2][8]"]}"#;
/// Canonical field-neutral public inputs enforced by the EpAffine/Pallas step circuit.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PUBLIC_INPUTS_SCHEMA_V3: &[u8] = br#"{"schema":"kagemusha_recursive_spend_step_ep_two_parent_operation_protocol_v2","layout":"single_column_u32","limbs":4024,"state_vector":{"layout_version":1,"limbs":889,"parent_slots":2},"operation":{"field_elements":135,"limbs_per_element":8,"limbs":1080,"encoding":"canonical_pasta_scalar_le_u32"},"compiled_protocol_identity":{"wire_version":1,"hash":"sha256","limbs":8},"ipa_accumulator":{"wire_version":1,"rounds":12,"limbs":106},"public_inputs":["public_statement_digest_u32[8]","operation_u32[1080]","parent_count_u32","parent_states_u32[2][889]","result_state_u32[889]","manifest_sha256_u32[8]","step_eq_compiled_protocol_sha256_u32[8]","step_ep_compiled_protocol_sha256_u32[8]","parent_eq_lineage_accumulator_u32[106]","parent_ep_lineage_accumulator_u32[106]","parent_eq_deferred_sha256_u32[2][8]","parent_ep_deferred_sha256_u32[2][8]"]}"#;
/// Version of the canonical cross-field state boundary.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V1: u16 = 1;
/// Version stored in limb zero of the exact cross-field recursive state.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V1: u32 = 1;
/// Exact number of unreduced `u32` limbs carried between both Pasta fields.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1: usize = 889;
/// Version of the canonical Pasta-cycle proof envelope.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V1: u16 = 1;
/// Envelope version for the operation/protocol-identity Step ABI.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V2: u16 = 2;
/// V3 release spelling for the current proof-envelope wire version.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V3: u16 =
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V2;
/// Proof-envelope version for the authenticated dynamic-layout V4 wire.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4: u16 = 4;
/// Version of the production recursive-spend artifact manifest.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3: u16 = 3;
/// Version of the degree-parameterized recursive-spend artifact manifest.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4: u16 = 4;
/// Schema identifier for a trusted Kagemusha V3 release-signing policy.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1: &str =
    "kagemusha.offline.recursive_spend.release_policy.v1";
/// Schema identifier for the signed Kagemusha V3 release envelope.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V1: &str =
    "kagemusha.offline.recursive_spend.release_attestation.v1";
/// Schema identifier for a signed V4 recursive-spend release envelope.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V4: &str =
    "kagemusha.offline.recursive_spend.release_attestation.v4";
/// Schema identifier for the machine-readable promotion record emitted after verification.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V3: &str =
    "kagemusha.offline.recursive_spend.promoted_release.v3";
/// Schema identifier for an authenticated ABI-20/V4 promotion record.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4: &str =
    "kagemusha.offline.recursive_spend.promoted_release.v4";
/// Domain separator for role-specific V3 release approvals.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_APPROVAL_DOMAIN_V1: &str =
    "iroha:kagemusha:recursive-spend-release-approval:v1";
/// Domain separator for role-specific V4 release approvals.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_APPROVAL_DOMAIN_V4: &str =
    "iroha:kagemusha:recursive-spend-release-approval:v4";
/// Current release policy, attestation, and promotion-record version.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1: u16 = 1;
/// Signed-envelope version for explicitly degree-parameterized V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4: u16 = 4;
/// Defensive upper bound for authorized signers or supplied approvals.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1: usize = 64;
/// Maximum signed review or physical-device evidence file accepted by promotion tooling.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1: usize = 16 * 1024 * 1024;
/// Canonical Norito file containing the signed V3 release envelope.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V1: &str =
    "release-attestation.norito";
/// Canonical Norito file containing the signed V4 release envelope.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4: &str =
    "release-attestation-v4.norito";
/// Canonical opaque file containing signed physical-device benchmark evidence.
pub const KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1: &str =
    "physical-device-benchmark.evidence";
/// Canonical opaque file containing the independent cryptographic review artifact.
pub const KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1: &str =
    "cryptographic-review.evidence";
/// Maximum release proof payload carried by a V2 recursive bundle.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3: u32 = 21_764;
/// Canonical IPA domain exponent for both V3 Pasta-cycle profiles.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1: u32 = 12;
/// V3 release-metadata spelling for the exact V1 IPA domain exponent.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V3: u32 =
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1;
/// Version of the canonical authenticated V4 circuit configuration.
pub const KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4: u16 = 4;
/// Lowest degree admitted for the complete fixed-shape V4 Step circuit.
///
/// This is a rejection floor, not a claim that every calibrated circuit fits
/// degree 20.
pub const KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4: u32 = 20;
/// Highest degree whose serialized Pasta parameters fit the release artifact
/// corridor with a conservative margin.
pub const KAGEMUSHA_STEP_CIRCUIT_MAXIMUM_K_V4: u32 = 21;
/// Minimum unusable-row reservation required by the Halo2 base circuit.
pub const KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4: u32 = 9;
/// Maximum supported challenge-phase vector length.
pub const KAGEMUSHA_STEP_CIRCUIT_MAX_PHASES_V4: usize = 3;
/// Maximum configured columns of any one class in a phase.
pub const KAGEMUSHA_STEP_CIRCUIT_MAX_COLUMNS_V4: u32 = 256;
/// Domain separator for canonical V4 circuit-parameter identities.
pub const KAGEMUSHA_STEP_CIRCUIT_PARAMS_SHA256_DOMAIN_V4: &[u8] =
    b"iroha:kagemusha:step-circuit-params:v4";
/// Version of the degree-parameterized accumulated-opening wire.
pub const KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4: u16 = 4;
/// Version of the V4 Eq/Ep proof-pair wire.
pub const KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4: u16 = 4;
/// Version of one authenticated selector-zero bootstrap witness payload.
pub const KAGEMUSHA_PASTA_BOOTSTRAP_WITNESS_VERSION_V4: u16 = 4;
/// Domain separator for canonical V4 bootstrap witness identities.
pub const KAGEMUSHA_PASTA_BOOTSTRAP_WITNESS_SHA256_DOMAIN_V4: &[u8] =
    b"iroha:kagemusha:pasta-bootstrap-witness:v4";
/// Public selector used only by the manifest-independent bootstrap circuit.
pub const KAGEMUSHA_PASTA_PUBLIC_BOOTSTRAP_SELECTOR_V4: u32 = 0;
/// Public selector required by every ordinary live V4 Step proof.
pub const KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4: u32 = 1;
/// Absolute defensive ceiling for one measured V4 Step proof transcript.
pub const KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4: u32 = 1024 * 1024;
/// Absolute defensive ceiling for one canonical V4 Eq/Ep proof-pair payload.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4: u32 = 16 * 1024 * 1024;
/// Maximum size of any one content-addressed V3 artifact file.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3: u64 = 256 * 1024 * 1024;
/// Framing magic for a streamed V3 recursive-spend key artifact.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V3: &[u8; 8] = b"KRV3KEY\0";
/// Framing magic for a streamed V4 recursive-spend key artifact.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4: &[u8; 8] = b"KRV4KEY\0";
/// Exact public-header layout version used by every `KRV4KEY` file.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4: u16 = 4;
/// Defensive upper bound for the canonical Norito header preceding a V4 payload.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_MAX_BYTES_V4: u32 = 64 * 1024;
/// Maximum size of any one V4 content-addressed artifact file.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4: u64 = 256 * 1024 * 1024;
/// Canonical EqAffine/Vesta step parameter package file name.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V3: &str =
    "step-eq.parameters.krv3";
/// Canonical EqAffine/Vesta step proving-key package file name.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V3: &str =
    "step-eq.proving-key.krv3";
/// Canonical EqAffine/Vesta step verifying-key package file name.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V3: &str =
    "step-eq.verifying-key.krv3";
/// Canonical EpAffine/Pallas step parameter package file name.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V3: &str =
    "step-ep.parameters.krv3";
/// Canonical EpAffine/Pallas step proving-key package file name.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V3: &str =
    "step-ep.proving-key.krv3";
/// Canonical EpAffine/Pallas step verifying-key package file name.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V3: &str =
    "step-ep.verifying-key.krv3";
/// Canonical Eq parameter package file name for V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V4: &str =
    "step-eq.parameters.krv4";
/// Canonical Eq processed proving-key package file name for V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4: &str =
    "step-eq.proving-key.krv4";
/// Canonical Eq processed verifier-key package file name for V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4: &str =
    "step-eq.verifying-key.krv4";
/// Canonical Eq final-key selector-zero bootstrap-witness package for V4 runtime.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4: &str =
    "step-eq.bootstrap-witness.krv4";
/// Canonical Ep parameter package file name for V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V4: &str =
    "step-ep.parameters.krv4";
/// Canonical Ep processed proving-key package file name for V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4: &str =
    "step-ep.proving-key.krv4";
/// Canonical Ep processed verifier-key package file name for V4 releases.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4: &str =
    "step-ep.verifying-key.krv4";
/// Canonical Ep final-key selector-zero bootstrap-witness package for V4 runtime.
pub const KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4: &str =
    "step-ep.bootstrap-witness.krv4";
/// Exact ordered cryptographic artifact roles required by an ABI-20 release.
///
/// The order is part of the capability contract and follows the canonical
/// Eq-then-Ep profile order and the four-artifact order within each profile.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4: [&str; 8] = [
    "step_eq_parameters",
    "step_eq_proving_key",
    "step_eq_verifying_key",
    "step_eq_bootstrap_witness",
    "step_ep_parameters",
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
/// Whether the branch-safe fractional recursive-spend V2 circuit is linked.
///
/// The public V2 statement is defined so SDKs can converge on one wire contract,
/// but it must remain fail-closed until the recursive proof binds both sibling
/// branches and their independent redemption nullifiers.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE: bool = false;
/// Canonical verifier-record namespace for Kagemusha proof admission.
pub const KAGEMUSHA_VERIFIER_NAMESPACE: &str = "offline_kagemusha";
/// Transparent backend used by the independent confidential transfer circuits.
pub const KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND: &str = "halo2/ipa";

/// Registry schema hash for the V3 StepEq verifier record.
#[must_use]
pub fn kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v3() -> [u8; 32] {
    Hash::new(KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PUBLIC_INPUTS_SCHEMA_V3).into()
}

/// Registry schema hash for the V3 StepEp verifier record.
#[must_use]
pub fn kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v3() -> [u8; 32] {
    Hash::new(KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PUBLIC_INPUTS_SCHEMA_V3).into()
}

/// Compute the SHA-256 content identifier used by Kagemusha V3 release files.
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

/// Stable failure returned while authenticating or promoting a V3 or V4 artifact release.
#[allow(variant_size_differences)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KagemushaReleaseVerificationError {
    /// The release manifest fails its versioned structural contract.
    InvalidManifest,
    /// The locally trusted role policy is malformed.
    InvalidPolicy,
    /// The signed release envelope is malformed or does not bind the manifest.
    InvalidAttestation,
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

/// Runtime proof that a manifest, evidence set, and role thresholds were authenticated.
///
/// Fields are private so unsigned manifests cannot be passed to production release loaders by
/// constructing this value directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaAuthenticatedReleaseV3 {
    manifest: KagemushaRecursiveSpendArtifactManifestV3,
    manifest_sha256: [u8; 32],
    release_attestation_sha256: [u8; 32],
    release_policy_sha256: [u8; 32],
    approved_signers: Vec<KagemushaRecursiveSpendApprovedSignerV1>,
}

/// Runtime proof that a V4 manifest, evidence set, and role thresholds were authenticated.
///
/// This is a separate type from [`KagemushaAuthenticatedReleaseV3`], so no
/// caller can pass degree-parameterized material to a historical ABI-19 loader.
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
        /// Absolute Unix expiry in milliseconds.
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
        /// Absolute Unix expiry in milliseconds.
        pub expires_at_ms: u64,
        /// Requested recipient output descriptor.
        pub recipient_output: KagemushaSpendableNoteDescriptorV2,
        /// Peer-carried opaque output-opening archive consumed by the sender prover.
        pub sender_output_prover_material: Vec<u8>,
        /// Receiver-device signature over the canonical unsigned fields.
        pub signature: KagemushaDeviceSignatureV2,
    }

    /// Self-contained payer/recipient authorization carried inside one V2 archive.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRequestAuthorizationV2 {
        /// Account whose key signs this request.
        pub authority: AccountId,
        /// Registered device identifier used for policy/App-Attest lookup.
        pub device_id: String,
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
        /// SHA-256 of App-Attest evidence, present exactly when evidence is attached.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub app_attest_evidence_sha256: Option<[u8; 32]>,
        /// Platform evidence verified against the registered device lineage.
        pub app_attest_evidence: Option<Vec<u8>>,
        /// Authority signature over the canonical authorization signing bytes.
        pub signature: Signature,
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

    /// Versioned first-hop request binding finalized top-up provenance.
    ///
    /// The embedded consensus proof is authenticated data without a semantic
    /// total order; canonical wire encoding does not depend on Rust ordering.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendInitRequestV2 {
        /// Finalized chain receipt that this local recursive init must consume.
        pub topup_anchor: KagemushaRecursiveSpendTopUpAnchorV2,
        /// Offline-verifiable proof that consensus finalized this exact anchor.
        pub topup_finality_proof: KagemushaTopUpFinalityProofV2,
        /// Exact content-addressed validator roster selected by the installed
        /// release manifest. Embedding it makes init one atomic native trust
        /// decision instead of relying on a preceding verifier call.
        pub topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactV2,
        /// Authenticated artifact release installed before offline operation.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
    }

    /// Typed native result for the initial recursive state.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendInitResultV2 {
        /// Independently spendable state created from the finalized top-up.
        pub bundle: KagemushaRecursiveSpendBundleV2,
        /// Circuit-exposed digest of the complete public statement.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub public_statement_digest: [u8; 32],
    }

    /// Canonical unsigned online-to-offline fields covered by payer authorization.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendTopUpUnsignedV2 {
        /// Online asset balance charged for the top-up.
        pub asset: AssetId,
        /// Exact positive amount charged at the live asset-definition scale.
        pub amount: KagemushaScaledAmountV2,
        /// First spendable note produced by the shield transition.
        pub current_note: KagemushaSpendableNoteDescriptorV2,
        /// Proof that inserts this note without consuming a confidential input.
        pub shield_evidence: KagemushaTopUpShieldEvidenceV2,
        /// Authenticated recursive artifact release selected for later init.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
        /// Globally unique replay-stable operation identifier copied into the finalized anchor.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Versioned chain-facing online-to-offline request.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(schema_name = "iroha.torii.v1.offline.top_up.request")]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendTopUpRequestV2 {
        /// Online asset balance charged for the top-up.
        pub asset: AssetId,
        /// Exact positive amount charged at the live asset-definition scale.
        pub amount: KagemushaScaledAmountV2,
        /// First spendable note produced by the shield transition.
        pub current_note: KagemushaSpendableNoteDescriptorV2,
        /// Proof that inserts this note without consuming a confidential input.
        pub shield_evidence: KagemushaTopUpShieldEvidenceV2,
        /// Authenticated recursive artifact release selected for the later
        /// local init. The chain records the binding but never fetches files.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
        /// Replay-stable operation identifier copied into the finalized anchor.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Self-contained payer/device authorization.
        pub authorization: KagemushaRequestAuthorizationV2,
    }

    /// Finalized chain anchor consumed by the local V2 init prover and later redemption.
    ///
    /// Top-up is deliberately two-phase: the chain first settles the public
    /// debit and confidential transfer, then the wallet proves the initial
    /// Recursive state proof bound to this immutable receipt.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendTopUpAnchorV2 {
        /// Anchor schema version.
        pub version: u16,
        /// Chain that finalized the top-up.
        pub chain_id: ChainId,
        /// Payer whose online balance funded the anchor.
        pub payer: AccountId,
        /// Exact payer asset, including its balance scope.
        pub asset: AssetId,
        /// Authoritative fixed scale read from the live asset definition.
        pub asset_scale: u32,
        /// Exact positive amount debited and reserved into escrow.
        pub amount: KagemushaScaledAmountV2,
        /// Confidential root before the finalized transfer.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub initial_root: [u8; 32],
        /// Confidential root finalized by the transfer.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub finalized_root: [u8; 32],
        /// Confidential tree position at which the top-up note was inserted.
        pub shield_leaf_index: u32,
        /// Exact first spendable note requested by the payer.
        pub current_note: KagemushaSpendableNoteDescriptorV2,
        /// Stable top-up operation identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub topup_operation_id: [u8; 32],
        /// Active top-up shield verifier selected at finalization.
        pub shield_verifier_id: VerifyingKeyId,
        /// Registered shield verifier commitment.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub shield_verifier_commitment: [u8; 32],
        /// Authenticated recursive artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
        /// Block height at which the transfer and public debit finalized.
        pub finalized_height: u64,
        /// Canonical signed transaction hash that created this anchor.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub finalized_tx_hash: [u8; 32],
        /// Canonical digest of every preceding receipt field.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub anchor_digest: [u8; 32],
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
        /// Audited snapshot anchor when no parent CommitQC exists.
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
        /// Commit QC with its roster PoPs supplied by the trusted artifact.
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

    /// Typed previous-proof package consumed by one append input.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendAppendInputV2 {
        /// Previous spendable recursive state.
        pub previous_bundle: KagemushaRecursiveSpendBundleV2,
    }

    /// Secret-free native builder input for one canonical V2 split intent.
    ///
    /// Parent claims, roots, counts, anchors, chain, asset, scale, and lineage
    /// mode are deliberately absent. The native builder derives them from the
    /// validated opaque parent bundles so SDK callers cannot forge provenance.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendSplitIntentBuildRequestV2 {
        /// One or two parent bundles in strict canonical bundle-digest order.
        pub previous_bundles: Vec<KagemushaRecursiveSpendBundleV2>,
        /// Authenticated artifact release selected for both outputs.
        pub output_artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
        /// Exact amount assigned to the receiver.
        pub transfer_amount: KagemushaScaledAmountV2,
        /// Receiver-owned proof output.
        pub recipient_output: KagemushaSpendableNoteDescriptorV2,
        /// Sender-owned remainder, present exactly for a partial transfer.
        pub change_output: Option<KagemushaSpendableNoteDescriptorV2>,
        /// Digest of the already verified receiver payment request.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Stable idempotency/replay identifier for the split.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Public statement that the branch-safe V2 recursive circuit must bind.
    ///
    /// It conserves an input note into a recipient output and optional sender
    /// change, while binding a nonce-bearing receiver request, stable operation
    /// id, and the parent recursive lineage.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendSplitIntentV2 {
        /// Chain inherited from the parent state and receiver request.
        pub chain_id: ChainId,
        /// Asset inherited from the parent state and receiver request.
        pub asset: AssetDefinitionId,
        /// One or two canonical previous branches, strictly ordered by bundle
        /// digest. Every consumed conflict coordinate is retained in each
        /// output so a joined note remains replay-safe after later splits.
        pub inputs: Vec<KagemushaRecursiveSpendInputBranchV2>,
        /// Canonical finalized top-up references contributing value to the inputs.
        pub topup_anchor_refs: Vec<KagemushaRecursiveSpendTopUpAnchorRefV2>,
        /// Authoritative asset-definition scale inherited from the parent state.
        pub asset_scale: u32,
        /// Authenticated artifact release selected for the output proof.
        pub output_artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
        /// Exact amount assigned to the recipient output.
        pub transfer_amount: KagemushaScaledAmountV2,
        /// Recipient-owned output note.
        pub recipient_output: KagemushaSpendableNoteDescriptorV2,
        /// Sender-owned remainder; present exactly for a partial transfer.
        pub change_output: Option<KagemushaSpendableNoteDescriptorV2>,
        /// Digest of the receiver's nonce-bound payment request.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Stable idempotency/replay identifier for this split.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Versioned recursive append request for a split transition.
    ///
    /// A successful native append returns one
    /// [`KagemushaRecursiveSpendSplitResultV2`] containing the independently
    /// spendable recipient and optional change branches.
    ///
    /// The current bridge deliberately exposes no prover entrypoint for this
    /// request while [`KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE`]
    /// is false.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendAppendRequestV2 {
        /// One or two typed previous-proof packages in the exact canonical
        /// order used by `split.inputs`.
        pub previous_inputs: Vec<KagemushaRecursiveSpendAppendInputV2>,
        /// Confidential-transfer-v2 proof containing both output commitments.
        pub confidential_transfer_proof: ProofAttachment,
        /// Scale, recipient, change, replay, and lineage statement.
        pub split: KagemushaRecursiveSpendSplitIntentV2,
        /// Authoritative verifier activation height used for every input proof.
        pub block_height: u64,
    }

    /// Typed native builder input shared by full and partial redemption.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemBuildRequestV2 {
        /// Spendable recursive state being redeemed.
        pub bundle: KagemushaRecursiveSpendBundleV2,
        /// Online account credited by redemption.
        pub recipient: AccountId,
        /// Exact public amount and authoritative asset scale.
        pub public_amount: KagemushaScaledAmountV2,
        /// Unshield-v3 proof for the public credit and optional change output.
        pub unshield_proof: ProofAttachment,
        /// Exact public redemption transition.
        pub redemption: KagemushaRecursiveSpendRedemptionIntentV2,
        /// Height used for installed verifier activation-window checks.
        pub block_height: u64,
        /// Stable idempotency identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Prepared unsigned redemption returned before device authorization.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemBuildResultV2 {
        /// Complete unsigned chain request fields.
        pub unsigned: KagemushaRecursiveSpendRedeemUnsignedV2,
        /// Exact digest that the device authorization must sign.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub authorization_digest: [u8; 32],
        /// Independently spendable proof-bound change, only for partial redemption.
        pub offline_change_bundle: Option<KagemushaRecursiveSpendBundleV2>,
        /// Membership state for the proof-bound change, present exactly with its bundle.
        pub offline_change_membership_witness: Option<KagemushaNoteMembershipWitnessV2>,
        /// Stable operation identifier copied from the request.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
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

    /// Canonical unshield-v3 public words cross-checked by the V2 redemption transition.
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

    /// Public transition from one live branch into an online credit and optional change child.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedemptionIntentV2 {
        /// Chain inherited from the input bundle.
        pub chain_id: ChainId,
        /// Asset inherited from the input bundle.
        pub asset: AssetDefinitionId,
        /// Exact note consumed by unshield-v3.
        pub input_note: KagemushaSpendableNoteDescriptorV2,
        /// Canonical live conflict claims consumed by this redemption.
        pub parent_branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Canonical finalized top-up references carried by the parent bundle.
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
        /// Proof-bound change descriptor; absent for full redemption.
        pub change_output: Option<KagemushaSpendableNoteDescriptorV2>,
        /// Authenticated output artifact release, present exactly with change.
        pub change_artifact_binding: Option<KagemushaRecursiveSpendArtifactBindingV3>,
        /// Canonical unshield-v3 words parsed from the attached proof.
        pub unshield_public_inputs: KagemushaUnshieldPublicInputsBindingV2,
        /// Digest of `unshield_public_inputs` exposed by the V2 transition circuit.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub unshield_public_inputs_digest: [u8; 32],
        /// Stable authorization/idempotency operation id.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Secret-free native builder input for a canonical V2 redemption intent.
    ///
    /// Every parent identity and provenance field is derived from
    /// `previous_bundle`; callers supply only the requested public credit,
    /// optional proof-bound change, parsed unshield words, and operation id.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedemptionIntentBuildRequestV2 {
        /// Opaque live bundle consumed by unshield-v3.
        pub previous_bundle: KagemushaRecursiveSpendBundleV2,
        /// Online account receiving the public credit.
        pub recipient: AccountId,
        /// Exact public credit at the parent asset scale.
        pub public_amount: KagemushaScaledAmountV2,
        /// Proof-bound offline remainder for a partial redemption.
        pub change_output: Option<KagemushaSpendableNoteDescriptorV2>,
        /// Authenticated output artifact release, present exactly with `change_output`.
        pub change_artifact_binding: Option<KagemushaRecursiveSpendArtifactBindingV3>,
        /// Canonical unshield-v3 public words parsed by the privacy API.
        pub unshield_public_inputs: KagemushaUnshieldPublicInputsBindingV2,
        /// Digest of `unshield_public_inputs` exposed by the transition proof.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub unshield_public_inputs_digest: [u8; 32],
        /// Stable authorization/idempotency operation id.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Peer-to-peer split transition carried by a recursive output statement.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendPeerSplitTransitionV2 {
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

    /// Partial-redemption change transition carried by its surviving child statement.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedemptionChangeTransitionV2 {
        /// Circuit-exposed digest of the exact local redemption/change intent.
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

    /// Mutually exclusive transition that produced the current recursive state.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "transition", content = "value", rename_all = "snake_case")]
    pub enum KagemushaRecursiveSpendTransitionV2 {
        /// Ordinary offline peer split.
        PeerSplit(KagemushaRecursiveSpendPeerSplitTransitionV2),
        /// Proof-bound partial-redemption change child.
        RedemptionChange(KagemushaRecursiveSpendRedemptionChangeTransitionV2),
    }

    /// Canonical public statement that the V2 recursive circuit must expose.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendPublicStatementV2 {
        /// Chain that scopes this cash state.
        pub chain_id: ChainId,
        /// Asset committed by every note in the transition.
        pub asset: AssetDefinitionId,
        /// Authoritative asset scale.
        pub asset_scale: u32,
        /// Root after the current transition.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub final_root: [u8; 32],
        /// One or two canonical finalized top-up references funding this state.
        pub topup_anchor_refs: Vec<KagemushaRecursiveSpendTopUpAnchorRefV2>,
        /// Total recursive proof transitions including top-up and redemption-change splits.
        pub proof_step_count: u32,
        /// Number of peer-to-peer spends after top-up; top-up itself is zero.
        pub peer_hop_count: u32,
        /// Current independently spendable note.
        pub current_note: KagemushaSpendableNoteDescriptorV2,
        /// Transition-bound conflict claims for replay and split-choice checks.
        pub branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Mutually exclusive transition producing this state; absent only for init.
        pub transition: Option<KagemushaRecursiveSpendTransitionV2>,
        /// Authenticated proving-artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
        /// Recursive verifier selected by the proof.
        pub verifier_key_id: VerifyingKeyId,
    }

    /// V2 recursive proof whose public instance includes the statement digest.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendProofV2 {
        /// Verifier selected by the statement.
        pub verifier_key_id: VerifyingKeyId,
        /// Circuit-exposed digest of `KagemushaRecursiveSpendPublicStatementV2`.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub public_statement_digest: [u8; 32],
        /// Opaque proof envelope. Wallet code never interprets these bytes.
        pub proof: ProofBox,
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

    /// V3 release-metadata name for the first exact-state Pasta parity wire.
    pub type KagemushaPastaCycleParityV3 = KagemushaPastaCycleParityV1;

    /// Canonical exact state vector carried across the Pasta field boundary.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendStateBoundaryV1 {
        /// State-boundary layout version.
        pub layout_version: u16,
        /// All 889 canonical `u32` limbs, without field reduction or hash compression.
        pub state_limbs: Vec<u32>,
    }

    /// Versioned constant-size proof transported by a recursive-spend bundle.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPastaCycleProofEnvelopeV1 {
        /// Proof-envelope format version.
        pub version: u16,
        /// Exact paired-proof Pasta backend profile.
        pub proof_backend: String,
        /// Exact circuit-native transcript profile.
        pub transcript_profile: String,
        /// Exact Eq/Fp recursive-step circuit.
        pub step_eq_circuit_id: String,
        /// Exact Ep/Fq recursive-step circuit.
        pub step_ep_circuit_id: String,
        /// Human-readable release generation; `manifest_sha256` is the trust binding.
        pub artifact_generation: String,
        /// SHA-256 of the exact authenticated release manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub manifest_sha256: [u8; 32],
        /// Canonical Eq `ParamsIPA` generation identifier.
        pub step_eq_parameter_generation: String,
        /// Canonical Ep `ParamsIPA` generation identifier.
        pub step_ep_parameter_generation: String,
        /// SHA-256 of the exact Eq verifier key selected by the envelope.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub step_eq_verifier_key_sha256: [u8; 32],
        /// SHA-256 of the exact Ep verifier key selected by the envelope.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub step_ep_verifier_key_sha256: [u8; 32],
        /// Canonical cross-field state boundary exposed by the proof.
        pub state_boundary: KagemushaRecursiveSpendStateBoundaryV1,
        /// Backend-native proof bytes.
        pub proof: ProofBox,
    }

    /// Operation/protocol-identity proof envelope used by the current release.
    ///
    /// Its field order intentionally matches V1 so framing stays simple, but
    /// its version and circuit identifiers form a distinct, non-interchangeable
    /// wire generation.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPastaCycleProofEnvelopeV2 {
        /// Proof-envelope format version.
        pub version: u16,
        /// Exact paired-proof Pasta backend profile.
        pub proof_backend: String,
        /// Exact circuit-native transcript profile.
        pub transcript_profile: String,
        /// Exact Eq/Fp recursive-step circuit.
        pub step_eq_circuit_id: String,
        /// Exact Ep/Fq recursive-step circuit.
        pub step_ep_circuit_id: String,
        /// Human-readable release generation; `manifest_sha256` is the trust binding.
        pub artifact_generation: String,
        /// SHA-256 of the exact authenticated release manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub manifest_sha256: [u8; 32],
        /// Canonical Eq `ParamsIPA` generation identifier.
        pub step_eq_parameter_generation: String,
        /// Canonical Ep `ParamsIPA` generation identifier.
        pub step_ep_parameter_generation: String,
        /// SHA-256 of the exact Eq verifier key selected by the envelope.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub step_eq_verifier_key_sha256: [u8; 32],
        /// SHA-256 of the exact Ep verifier key selected by the envelope.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub step_ep_verifier_key_sha256: [u8; 32],
        /// Canonical cross-field state boundary exposed by the proof.
        pub state_boundary: KagemushaRecursiveSpendStateBoundaryV1,
        /// Backend-native proof bytes.
        pub proof: ProofBox,
    }

    /// Production V3 spelling for the current proof-envelope wire.
    pub type KagemushaPastaCycleProofEnvelopeV3 = KagemushaPastaCycleProofEnvelopeV2;

    /// Kind of content-addressed material bound to one Pasta proof profile.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "kind", content = "value", rename_all = "snake_case")]
    pub enum KagemushaPastaCycleArtifactKindV3 {
        /// Canonical `ParamsIPA` generator material.
        Parameters,
        /// Processed proving key.
        ProvingKey,
        /// Verifying key.
        VerifyingKey,
    }

    /// One immutable file in a V3 recursive-spend artifact manifest.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPastaCycleArtifactV3 {
        /// Material kind within the parity profile.
        pub kind: KagemushaPastaCycleArtifactKindV3,
        /// Safe single-component file name.
        pub file_name: String,
        /// Exact byte length.
        pub size_bytes: u64,
        /// SHA-256 of the exact file bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub sha256: [u8; 32],
        /// Exact byte length of the unframed key material.
        pub payload_size_bytes: u64,
        /// SHA-256 of the unframed key material.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub payload_sha256: [u8; 32],
    }

    /// Content-addressed trust artifact used to verify top-up finality offline.
    ///
    /// Unlike the six Pasta key packages this file is a canonical Norito
    /// archive, not a `KRV3KEY` package. The explicit circuit, ABI, purpose,
    /// and type fields prevent a same-size file from being substituted across
    /// artifact roles.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaTopUpFinalityRosterArtifactReferenceV2 {
        /// Safe single-component file name.
        pub file_name: String,
        /// Exact byte length of the canonical Norito archive.
        pub size_bytes: u64,
        /// SHA-256 of the exact file bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub sha256: [u8; 32],
        /// Exact generation declared by the authenticated roster archive.
        pub artifact_generation: String,
        /// Native verifier/circuit role consuming this artifact.
        pub circuit_id: String,
        /// Stable product purpose.
        pub purpose: String,
        /// Exact Norito type name contained by the file.
        pub artifact_type: String,
        /// Minimum bridge ABI that can verify this artifact.
        pub required_bridge_abi_version: u32,
    }

    /// Fixed verifier/prover material for one side of the Pasta proof cycle.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPastaCycleProofProfileV1 {
        /// Curve/parity implemented by this profile.
        pub parity: KagemushaPastaCycleParityV1,
        /// Exact fixed circuit id.
        pub circuit_id: String,
        /// Canonical `ParamsIPA` generation identifier.
        pub parameter_generation: String,
        /// Halo2 IPA domain exponent.
        pub ipa_k: u32,
        /// Exactly one parameter, proving-key, and verifying-key file.
        pub artifacts: Vec<KagemushaPastaCycleArtifactV3>,
    }

    /// V3 manifest name for the first exact-state Pasta proof profile.
    pub type KagemushaPastaCycleProofProfileV3 = KagemushaPastaCycleProofProfileV1;

    /// Production release manifest for the paired-proof Pasta backend.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendArtifactManifestV3 {
        /// Exact manifest schema identifier.
        pub schema: String,
        /// Manifest layout version.
        pub version: u16,
        /// Required native bridge ABI.
        pub bridge_abi_version: u32,
        /// Exact paired-proof Pasta backend profile.
        pub proof_backend: String,
        /// Exact circuit-native transcript profile.
        pub transcript_profile: String,
        /// Human-readable release generation; artifact hashes carry content identity.
        pub generation: String,
        /// Lowercase 40-hex source revision.
        pub source_commit: String,
        /// SHA-256 of the exact tracked and untracked source-tree bytes used for the build.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub source_tree_sha256: [u8; 32],
        /// Whether the exact source tree differed from `source_commit` when captured.
        pub source_repo_dirty: bool,
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
        /// Exact maximum proof payload accepted from this release.
        pub max_proof_bytes: u32,
        /// Eq then Ep recursive-step profiles.
        pub profiles: Vec<KagemushaPastaCycleProofProfileV1>,
        /// Validator-roster trust artifact required to authenticate top-up
        /// origins during later offline peer verification.
        pub topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV2,
        /// Digest of signed physical-device benchmark evidence.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub benchmark_evidence_sha256: [u8; 32],
        /// Digest of the independent cryptographic review artifact.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub cryptographic_review_sha256: [u8; 32],
        /// Digest of the signed release attestation.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub release_attestation_sha256: [u8; 32],
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
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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

    impl Default for KagemushaStepCircuitParamsV4 {
        fn default() -> Self {
            Self {
                version: 0,
                k: 0,
                num_advice_per_phase: Vec::new(),
                num_lookup_advice_per_phase: Vec::new(),
                num_fixed: 0,
                lookup_bits: 0,
                num_instance_columns: 0,
                public_input_limbs: 0,
                minimum_unusable_rows: 0,
                max_parent_proof_bytes: 0,
            }
        }
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
        Parameters,
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
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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

    /// Production release manifest for degree-parameterized paired Pasta proofs.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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

    /// Independent authority role required to promote a V3 artifact release.
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

    /// Immutable subject shared by every role-specific release approval.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendReleaseAttestationSubjectV1 {
        /// SHA-256 of the canonical manifest with its attestation-digest slot zeroed.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub manifest_subject_sha256: [u8; 32],
        /// Exact release generation copied from the manifest.
        pub generation: String,
        /// Exact source revision copied from the manifest.
        pub source_commit: String,
        /// Exact source-tree identity copied from the manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub source_tree_sha256: [u8; 32],
        /// Exact dirty-tree state copied from the manifest.
        pub source_repo_dirty: bool,
        /// Digest of the signed physical-device evidence file.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub benchmark_evidence_sha256: [u8; 32],
        /// Digest of the independent cryptographic review file.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub cryptographic_review_sha256: [u8; 32],
    }

    /// Domain-separated value signed for one independent approval role.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendReleaseApprovalPayloadV1 {
        /// Cross-protocol replay separator.
        pub domain: String,
        /// Authority role for which this signature is valid.
        pub role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
        /// Complete release subject approved by the signer.
        pub subject: KagemushaRecursiveSpendReleaseAttestationSubjectV1,
    }

    /// One role-bound signature inside a V3 release attestation.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendReleaseApprovalV1 {
        /// Independent authority role represented by this signature.
        pub role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
        /// Exact signer key selected by the trusted release policy.
        pub public_key: PublicKey,
        /// Signature over the domain-separated role and complete release subject.
        pub signature: SignatureOf<KagemushaRecursiveSpendReleaseApprovalPayloadV1>,
    }

    /// Authenticated release envelope whose digest occupies the V3 manifest slot.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendReleaseAttestationV1 {
        /// Exact attestation schema.
        pub schema: String,
        /// Attestation layout version.
        pub version: u16,
        /// Immutable subject approved by all roles.
        pub subject: KagemushaRecursiveSpendReleaseAttestationSubjectV1,
        /// Strictly ordered, unique role/signer approvals.
        pub approvals: Vec<KagemushaRecursiveSpendReleaseApprovalV1>,
    }

    /// Immutable subject shared by every role-specific V4 release approval.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
        /// Digest of the signed physical-device evidence file.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub benchmark_evidence_sha256: [u8; 32],
        /// Digest of the independent cryptographic review file.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub cryptographic_review_sha256: [u8; 32],
    }

    /// Domain-separated value signed for one independent V4 approval role.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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

    /// Locally trusted policy for authenticating a V3 release envelope.
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

    /// Deterministic deployment marker written only after release and artifact verification.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendPromotedReleaseV3 {
        /// Exact promotion-record schema.
        pub schema: String,
        /// Promotion-record layout version.
        pub version: u16,
        /// Authenticated release generation.
        pub generation: String,
        /// SHA-256 of the complete canonical manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub manifest_sha256: [u8; 32],
        /// SHA-256 of the canonical signed release attestation.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub release_attestation_sha256: [u8; 32],
        /// SHA-256 of the locally trusted release policy.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub release_policy_sha256: [u8; 32],
        /// Role/signer identities whose signatures were verified.
        pub approved_signers: Vec<KagemushaRecursiveSpendApprovedSignerV1>,
        /// Whether every content-addressed V3 artifact was verified before publication.
        pub artifact_inventory_verified: bool,
        /// Compile-time recursive backend status; remains false until all soundness gates close.
        pub proof_backend_available: bool,
        /// Stable remaining backend gates at promotion time.
        pub missing_gates: Vec<String>,
    }

    /// Deterministic ABI-20 deployment marker written only after V4 release verification.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
        /// Exact Eq-then-Ep eight-role artifact inventory selected by ABI-20.
        pub artifact_roles: Vec<String>,
        /// Authenticated release-specific proof-pair byte ceiling.
        pub max_proof_bytes: u32,
    }

    /// Installed authenticated artifact release selected by a recursive operation.
    ///
    /// Callers bind the complete manifest, never an individual proving-key
    /// role. The native backend resolves the parity-specific step profile and key
    /// kind required by each operation from this manifest identity.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendArtifactBindingV3 {
        /// Human-readable authenticated release generation.
        pub generation: String,
        /// SHA-256 of the exact signed manifest bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub manifest_sha256: [u8; 32],
    }

    /// Installed authenticated V4 release selected by a degree-parameterized operation.
    ///
    /// This is intentionally distinct from the V3 binding even though both
    /// carry a generation and manifest digest. Their referenced manifest
    /// schemas, bridge ABIs, and proof wires are not interchangeable.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendArtifactBindingV4 {
        /// Human-readable authenticated V4 release generation.
        pub generation: String,
        /// SHA-256 of the exact signed V4 manifest bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub manifest_sha256: [u8; 32],
    }

    /// Native bridge capability record used by wallets for fail-closed negotiation.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendNativeCapabilitiesV1 {
        /// Native bridge ABI reported by the loaded library.
        pub bridge_abi_version: u32,
        /// Required artifact manifest schema.
        pub artifact_manifest_schema: String,
        /// Required proof backend.
        pub proof_backend: String,
        /// Required transcript profile.
        pub transcript_profile: String,
        /// Proof-envelope format version.
        pub proof_envelope_version: u16,
        /// Cross-field state-boundary format version.
        pub state_boundary_version: u16,
        /// EqAffine/Vesta step circuit id.
        pub step_eq_circuit_id: String,
        /// EpAffine/Pallas step circuit id.
        pub step_ep_circuit_id: String,
        /// Maximum proof payload accepted by the release contract.
        pub max_proof_bytes: u32,
        /// Whether all production proof, audit, and performance gates passed.
        pub proof_backend_available: bool,
        /// Stable release blockers when the backend is unavailable.
        pub missing_gates: Vec<String>,
    }

    /// Versioned recursive bundle plus the split statement its proof must bind.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendBundleV2 {
        /// Exact public statement bound by the recursive proof.
        pub statement: KagemushaRecursiveSpendPublicStatementV2,
        /// Branch-safe recursive proof.
        pub recursive_proof: KagemushaRecursiveSpendProofV2,
    }

    /// All-or-none proof-bound child that survives a partial online redemption.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemChangeBranchV2 {
        /// Exact change descriptor exposed by unshield-v3.
        pub output: KagemushaSpendableNoteDescriptorV2,
        /// Deterministic transition-bound change children of every consumed claim.
        pub branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Recursive proof making that child independently spendable.
        pub bundle: KagemushaRecursiveSpendBundleV2,
    }

    /// Result of one recursive split append.
    ///
    /// The recipient and optional change bundles are independently spendable
    /// branches derived from the same parent transition. They share the exact
    /// split intent and circuit-exposed split digest, but each bundle carries
    /// its own branch-bound public statement and recursive proof.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendSplitResultV2 {
        /// Exact value-conserving transition shared by both branches.
        pub split: KagemushaRecursiveSpendSplitIntentV2,
        /// Circuit-exposed binding to `split` and its parent accumulator.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub split_binding_digest: [u8; 32],
        /// Receiver-owned independently spendable output.
        pub recipient_bundle: KagemushaRecursiveSpendBundleV2,
        /// Proof-bound membership state required for the recipient's next spend.
        pub recipient_membership_witness: KagemushaNoteMembershipWitnessV2,
        /// Sender-owned independently spendable change, present exactly when
        /// the split intent contains `change_output`.
        pub change_bundle: Option<KagemushaRecursiveSpendBundleV2>,
        /// Proof-bound membership state for sender change, present exactly with it.
        pub change_membership_witness: Option<KagemushaNoteMembershipWitnessV2>,
    }

    /// Recipient-only peer payload emitted from a local split result.
    ///
    /// Sender change is deliberately absent. Transports encode this type, not
    /// `KagemushaRecursiveSpendSplitResultV2`, so a receiver never learns or
    /// persists the sender's surviving branch.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendPeerPaymentV2 {
        /// Recipient-owned independently spendable branch. Its peer-split
        /// transition is the sole canonical source of the operation id and
        /// recipient-request digest.
        pub recipient_bundle: KagemushaRecursiveSpendBundleV2,
        /// Proof-bound membership state required for the recipient's next spend.
        pub recipient_membership_witness: KagemushaNoteMembershipWitnessV2,
    }

    /// One complete, independently verifiable finalized origin carried to a
    /// receiver outside the compact recursive bundle.
    ///
    /// The recursive state carries only the compact anchor reference.  The
    /// receiver needs these full bytes to recompute that reference and verify
    /// the Commit QC plus block-local inclusion proof without consulting the
    /// chain.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendTopUpFinalityEvidenceV2 {
        /// Complete finalized receipt whose digest is carried by the Step state.
        pub topup_anchor: KagemushaRecursiveSpendTopUpAnchorV2,
        /// Consensus proof for the exact compact reference of `topup_anchor`.
        pub topup_finality_proof: KagemushaTopUpFinalityProofV2,
    }

    /// Versioned receiver verification request.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendVerifyRequestV2 {
        /// Scale- and split-bound recursive bundle.
        pub bundle: KagemushaRecursiveSpendBundleV2,
        /// Receiver request that the final branch must match exactly.
        pub recipient_request: KagemushaRecipientPaymentRequestV2,
        /// Exact manifest-bound validator roster used for every origin proof.
        ///
        /// A release admits one immutable roster window, so all one-or-two
        /// origins in this request must verify against this single artifact.
        pub topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactV2,
        /// Canonically ordered complete evidence, exactly one entry for every
        /// compact top-up anchor reference carried by the recursive state.
        pub topup_finality_evidence: Vec<KagemushaRecursiveSpendTopUpFinalityEvidenceV2>,
        /// Maximum hop count accepted by the receiver for this artifact set.
        pub maximum_hops: u32,
        /// Expected authenticated artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
        /// Authoritative height used to resolve both installed step verifiers.
        pub block_height: u64,
        /// Authoritative current Unix time in milliseconds used for expiry checks.
        pub verified_at_ms: u64,
    }

    /// Opaque-safe summary decoded from a recursive spend bundle for wallet state.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendBundleSummaryV2 {
        /// Asset definition bound by the recursive proof.
        pub asset: AssetDefinitionId,
        /// Exact current spendable amount.
        pub amount: KagemushaScaledAmountV2,
        /// Current note commitment.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub note_commitment: [u8; 32],
        /// Current note nullifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub spend_nullifier: [u8; 32],
        /// Current hop count.
        pub hop_count: u32,
        /// Current recursive transition count proved by both Pasta halves.
        pub proof_step_count: u32,
        /// Current canonical transition-bound conflict claims.
        pub branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Authenticated artifact release used to produce the proof.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
        /// Recursive verifier selected by the proof.
        pub verifier_key_id: VerifyingKeyId,
        /// Canonical identity digest of the complete opaque bundle.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub bundle_digest: [u8; 32],
    }

    /// Typed receiver-verification result returned by the native bridge.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendVerifyResultV2 {
        /// Cryptographic proof and all public bindings verified.
        pub valid: bool,
        /// Bundle satisfies current chain admission rules.
        pub chain_admissible: bool,
        /// Persisted lineage material can be redeemed.
        pub lineage_redeemable: bool,
        /// Chain supports redemption without a record-backed witness.
        pub witnessless_redemption_supported: bool,
        /// Verified bundle summary.
        pub summary: KagemushaRecursiveSpendBundleSummaryV2,
        /// Canonical receiver request digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Digest binding request, split, branch output, and bundle.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub request_output_binding_digest: [u8; 32],
        /// Active recursive verifier record identifier.
        pub verifier_key_id: VerifyingKeyId,
        /// Active recursive circuit id.
        pub verifier_circuit_id: String,
        /// Inclusive verifier activation height.
        pub verifier_activation_height: Option<u64>,
        /// Exclusive verifier withdrawal height.
        pub verifier_withdraw_height: Option<u64>,
        /// Height used for activation-window verification.
        pub verified_at_block_height: u64,
        /// Authoritative Unix time used to accept the receiver request.
        pub verified_at_ms: u64,
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

    /// Typed native redemption output.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemResultV2 {
        /// Canonical `KagemushaRecursiveSpendRedeemRequestV2` archive submitted to Core API.
        pub redeem_request_archive: Vec<u8>,
        /// Proof-bound offline change branch for partial redemption.
        pub offline_change_bundle: Option<KagemushaRecursiveSpendBundleV2>,
        /// Membership state for the proof-bound change, present exactly with its bundle.
        pub offline_change_membership_witness: Option<KagemushaNoteMembershipWitnessV2>,
        /// Stable operation identifier copied from the request.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Canonical unsigned offline-to-online fields covered by recipient authorization.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemUnsignedV2 {
        /// Scale-carrying recursive state being redeemed.
        pub bundle: KagemushaRecursiveSpendBundleV2,
        /// Online account credited by redemption.
        pub recipient: AccountId,
        /// Exact public amount and authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
        /// Unshield-v3 proof bound to the current note and optional change output.
        pub redeem_proof: ProofAttachment,
        /// Canonical public redemption intent.
        pub redemption: KagemushaRecursiveSpendRedemptionIntentV2,
        /// All-or-none proof-bound partial-redemption change child.
        pub offline_change: Option<KagemushaRecursiveSpendRedeemChangeBranchV2>,
        /// Height used for verifier activation-window checks.
        pub block_height: u64,
        /// Stable idempotency identifier for finality-safe retries.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Versioned offline-to-online request preserving the proof/public scale binding.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(schema_name = "iroha.torii.v1.offline.redeem.request")]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendRedeemRequestV2 {
        /// Scale-carrying recursive state being redeemed.
        pub bundle: KagemushaRecursiveSpendBundleV2,
        /// Online account credited by redemption.
        pub recipient: AccountId,
        /// Exact public amount and authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
        /// Unshield-v3 proof bound to the current note and optional change output.
        pub redeem_proof: ProofAttachment,
        /// Canonical public redemption intent cross-bound by authorization and proof checks.
        pub redemption: KagemushaRecursiveSpendRedemptionIntentV2,
        /// All-or-none proof-bound partial-redemption change child.
        pub offline_change: Option<KagemushaRecursiveSpendRedeemChangeBranchV2>,
        /// Height used for verifier activation-window checks.
        pub block_height: u64,
        /// Globally unique idempotency identifier for finality-safe retries.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Self-contained recipient/device authorization.
        pub authorization: KagemushaRequestAuthorizationV2,
    }

    /// Native capability record for the explicitly versioned ABI-20 backend.
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
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
        pub state_boundary: KagemushaRecursiveSpendStateBoundaryV1,
        /// Canonical adapter-owned V4 Eq/Ep proof-pair bytes.
        pub proof: ProofBox,
    }

    /// Canonical public statement bound by an ABI-20 recursive proof.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
        /// Binding-only semantic transition; this V2-named leaf contains no artifact binding.
        pub transition: Option<KagemushaRecursiveSpendTransitionV2>,
        /// Authenticated V4 proving-artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Recursive verifier selected by the proof.
        pub verifier_key_id: VerifyingKeyId,
    }

    /// V4 recursive proof whose public instance includes the statement digest.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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

    /// Independently spendable ABI-20 recursive state.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendBundleV4 {
        /// Exact public statement bound by the proof.
        pub statement: KagemushaRecursiveSpendPublicStatementV4,
        /// Degree-parameterized recursive proof.
        pub recursive_proof: KagemushaRecursiveSpendProofV4,
    }

    /// Finalized top-up anchor selecting a V4 recursive release.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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

    /// Public V4 split transition with an ABI-20 output binding.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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

    /// Public V4 redemption transition with an optional ABI-20 change binding.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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

    /// ABI-20 local initialization request.
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

    /// ABI-20 initialization result.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendInitResultV4 {
        /// Independently spendable state created from the finalized top-up.
        pub bundle: KagemushaRecursiveSpendBundleV4,
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
        /// Previous spendable ABI-20 recursive state.
        pub previous_bundle: KagemushaRecursiveSpendBundleV4,
    }

    /// ABI-20 recursive append request.
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
        /// Authoritative verifier activation height.
        pub block_height: u64,
    }

    /// Result of one ABI-20 recursive split append.
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
        /// Sender-owned remainder, present only for a partial transfer.
        pub change_bundle: Option<KagemushaRecursiveSpendBundleV4>,
        /// Proof-bound membership state for sender change.
        pub change_membership_witness: Option<KagemushaNoteMembershipWitnessV2>,
    }

    /// Complete finalized V4 origin carried to an offline receiver.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
        /// Complete finalized ABI-20 top-up receipt.
        pub topup_anchor: KagemushaRecursiveSpendTopUpAnchorV4,
        /// Consensus proof for the compact anchor reference.
        pub topup_finality_proof: KagemushaTopUpFinalityProofV2,
    }

    /// ABI-20 receiver-verification request.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendVerifyRequestV4 {
        /// Scale- and split-bound ABI-20 recursive bundle.
        pub bundle: KagemushaRecursiveSpendBundleV4,
        /// Receiver request that the final branch must match.
        pub recipient_request: KagemushaRecipientPaymentRequestV2,
        /// Exact manifest-bound validator roster.
        pub topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactV2,
        /// Complete evidence for every compact top-up reference.
        pub topup_finality_evidence: Vec<KagemushaRecursiveSpendTopUpFinalityEvidenceV4>,
        /// Maximum hop count accepted by the receiver.
        pub maximum_hops: u32,
        /// Expected authenticated V4 artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Authoritative verifier activation height.
        pub block_height: u64,
        /// Authoritative current Unix time in milliseconds.
        pub verified_at_ms: u64,
    }

    /// Opaque-safe summary decoded from an ABI-20 bundle.
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

    /// Typed ABI-20 receiver-verification result.
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
        /// Verified ABI-20 bundle summary.
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
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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

    /// ABI-20 native redemption builder input.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
        /// Height used for verifier activation-window checks.
        pub block_height: u64,
        /// Stable idempotency identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Canonical unsigned ABI-20 chain redemption fields.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemUnsignedV4 {
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
        /// Height used for verifier activation-window checks.
        pub block_height: u64,
        /// Stable idempotency identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Prepared unsigned ABI-20 redemption result.
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
        /// Stable operation identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }

    /// Versioned ABI-20 offline-to-online request.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendRedeemRequestV4 {
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
        /// Height used for verifier activation-window checks.
        pub block_height: u64,
        /// Globally unique idempotency identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Self-contained recipient/device authorization.
        pub authorization: KagemushaRequestAuthorizationV2,
    }

    /// Typed native ABI-20 redemption output.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemResultV4 {
        /// Canonical `KagemushaRecursiveSpendRedeemRequestV4` archive.
        pub redeem_request_archive: Vec<u8>,
        /// Proof-bound offline change branch.
        pub offline_change_bundle: Option<KagemushaRecursiveSpendBundleV4>,
        /// Membership state for proof-bound change.
        pub offline_change_membership_witness: Option<KagemushaNoteMembershipWitnessV2>,
        /// Stable operation identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }
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

/// Android KeyMint challenge inputs available before the attested key is generated.
///
/// Android derives the final registration `key_id` from the public key created by
/// KeyMint. Consequently, this first-phase challenge deliberately has no `key_id`
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
    /// Android package name expected in the KeyMint attestation application id.
    pub android_package_name: Option<String>,
    /// Android signing-certificate SHA-256 expected in the attestation application id.
    pub android_signing_certificate_sha256: Option<Vec<u8>>,
    /// Ed25519 public key bytes for local note/proof signatures.
    pub public_key: KagemushaDevicePublicKeyV2,
    /// Hardware assertion scheme bound to this note key.
    pub assertion_scheme: String,
    /// Hardware assertion key algorithm.
    pub assertion_key_algorithm: String,
    /// Hardware one-use limit exposed by KeyMint.
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
/// Nodes use this chain-stored policy when present and otherwise fall back to
/// the built-in first-release platform roots. Operators can publish this policy
/// to rotate roots, publish deterministic revocations, and restrict accepted app
/// identities without relying on external middleware state.
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
    /// When true, iOS registration requires a matching entry in `ios_apps`.
    pub require_ios_app_policy: bool,
    /// When true, Android registration requires a matching entry in `android_apps`.
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
    /// Apple Developer Team ID.
    pub team_id: String,
    /// iOS bundle identifier.
    pub bundle_id: String,
    /// App Attest environment, either `production` or `development`.
    pub environment: String,
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

/// KeyMint uses this separate schema because `key_id` is derived from the key
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
    operation_id: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
    nonce: [u8; 32],
    payload_digest: [u8; 32],
    app_attest_evidence_sha256: Option<[u8; 32]>,
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

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaTopUpUnsignedPayloadDigestPreimageV2 {
    domain: String,
    asset: AssetId,
    amount: KagemushaScaledAmountV2,
    current_note: KagemushaSpendableNoteDescriptorV2,
    shield_evidence: KagemushaTopUpShieldEvidenceV2,
    artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
    operation_id: [u8; 32],
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
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        Self::from_raw_bytes(&self.0).map(|_| ())
    }

    /// Return the canonical fixed-width `r || s` bytes.
    #[must_use]
    pub const fn as_raw_bytes(&self) -> &[u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2] {
        &self.0
    }

    /// Verify ECDSA-P256-SHA256 under the fixed Kagemusha authority profile.
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
struct KagemushaTopUpAnchorDigestPreimageV2 {
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
    artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
    finalized_height: u64,
    finalized_tx_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRedeemUnsignedPayloadDigestPreimageV2 {
    domain: String,
    bundle: KagemushaRecursiveSpendBundleV2,
    recipient: AccountId,
    amount: KagemushaScaledAmountV2,
    redeem_proof: ProofAttachment,
    redemption: KagemushaRecursiveSpendRedemptionIntentV2,
    offline_change: Option<KagemushaRecursiveSpendRedeemChangeBranchV2>,
    block_height: u64,
    operation_id: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRedemptionTransitionDigestPreimageV2 {
    domain: String,
    intent: KagemushaRecursiveSpendRedemptionIntentV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaUnshieldPublicInputsDigestPreimageV2 {
    domain: String,
    public_inputs: KagemushaUnshieldPublicInputsBindingV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRecursiveSpendBundleDigestPreimageV2 {
    domain: String,
    bundle: KagemushaRecursiveSpendBundleV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRecursiveSpendPublicStatementDigestPreimageV2 {
    domain: String,
    statement: KagemushaRecursiveSpendPublicStatementV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRecursiveSpendSplitBindingDigestPreimageV2 {
    domain: String,
    split: KagemushaRecursiveSpendSplitIntentV2,
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
            public_key: self.public_key.clone(),
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
            public_key: self.public_key.clone(),
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
    /// the platform response after the challenge is created. Android KeyMint
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
            public_key: self.public_key.clone(),
            assertion_scheme: self.assertion_scheme.clone(),
            assertion_key_algorithm: self.assertion_key_algorithm.clone(),
            assertion_usage_count_limit: self.assertion_usage_count_limit,
            one_use: self.one_use,
            recent_block_height: self.recent_block_height,
            recent_block_hash: self.recent_block_hash,
            expires_at_ms: self.expires_at_ms,
        }
    }

    /// Return the canonical Norito preimage bytes embedded into the KeyMint challenge hash.
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
    pub fn root(lineage_root: [u8; 32]) -> Result<Self, KagemushaValidationError> {
        let claim = Self {
            path: KagemushaRecursiveSpendBranchPathV2::root(lineage_root)?,
            transition_tags: Vec::new(),
        };
        claim.validate()?;
        Ok(claim)
    }

    /// Append one output edge and bind it to the exact producing transition.
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
            if previous.path.lineage_root != claim.path.lineage_root {
                continue;
            }
            let shared_depth = previous.path.depth.min(claim.path.depth);
            for parent_depth in 0..shared_depth {
                if previous.path.prefix(parent_depth)? == claim.path.prefix(parent_depth)?
                    && previous.transition_tag_at(parent_depth)
                        != claim.transition_tag_at(parent_depth)
                {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "branch_claims.transition_choice",
                    });
                }
            }
        }
    }
    Ok(())
}

impl KagemushaRecipientOutputDerivationRequestV2 {
    /// Validate the public, secret-free derivation context.
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
    /// Construct the canonical request from prevalidated fields and a device signature.
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
            receiver_public_key: self.receiver_public_key.clone(),
            request_id: self.request_id,
            issued_at_ms: self.issued_at_ms,
            expires_at_ms: self.expires_at_ms,
            recipient_output: self.recipient_output.clone(),
            sender_output_prover_material: self.sender_output_prover_material.clone(),
        }
    }

    /// Validate the exact signed request and opaque sender-prover material.
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

    /// Verify request authentication and lifetime at the sender's current time.
    pub fn validate_at(&self, now_ms: u64) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        if now_ms < self.issued_at_ms || now_ms > self.expires_at_ms {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "recipient_request.expires_at_ms",
            });
        }
        Ok(())
    }

    /// Return the canonical request digest bound by the split proof.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRecipientPaymentRequestDigestPreimageV2 {
            domain: KAGEMUSHA_RECIPIENT_PAYMENT_REQUEST_DIGEST_DOMAIN_V2.to_owned(),
            request: self.clone(),
        })
    }
}

impl KagemushaRequestAuthorizationV2 {
    /// Return the exact domain-separated bytes the authority must sign.
    ///
    /// Native bridges may construct the public fields with a placeholder
    /// signature, request user-presence signing from the device key, then
    /// replace only `signature`; the signature itself is deliberately excluded
    /// from this preimage.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        Ok(to_bytes(&KagemushaRequestAuthorizationSigningPreimageV2 {
            domain: KAGEMUSHA_REQUEST_AUTHORIZATION_DOMAIN_V2.to_owned(),
            authority: self.authority.clone(),
            device_id: self.device_id.clone(),
            operation_id: self.operation_id,
            issued_at_ms: self.issued_at_ms,
            expires_at_ms: self.expires_at_ms,
            nonce: self.nonce,
            payload_digest: self.payload_digest,
            app_attest_evidence_sha256: self.app_attest_evidence_sha256,
        })?)
    }

    /// Verify structure, evidence digest, payload binding, and account signature.
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
        match (&self.app_attest_evidence, self.app_attest_evidence_sha256) {
            (None, None) => {}
            (Some(evidence), Some(expected))
                if !evidence.is_empty()
                    && evidence.len() <= 16 * 1024
                    && <[u8; 32]>::from(Sha256::digest(evidence)) == expected => {}
            _ => {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "authorization.app_attest_evidence",
                });
            }
        }
        self.signature
            .verify(self.authority.signatory(), &self.signing_bytes()?)
            .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "authorization.signature",
            })
    }

    /// Verify the signed request is live at the authoritative Torii time.
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
    pub fn validate_for_root(&self, root: [u8; 32]) -> Result<(), KagemushaValidationError> {
        self.validate_structure()?;
        if self.input_path.root != root {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "membership_witness.root",
            });
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendStateBoundaryV1 {
    /// Construct the field-neutral boundary from the complete exact state.
    pub fn new(state_limbs: Vec<u32>) -> Result<Self, KagemushaValidationError> {
        let boundary = Self {
            layout_version: KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V1,
            state_limbs,
        };
        boundary.validate()?;
        Ok(boundary)
    }

    /// Recover the exact canonical limbs without field reduction.
    pub fn exact_state(&self) -> Result<&[u32], KagemushaValidationError> {
        self.validate()?;
        Ok(&self.state_limbs)
    }

    /// Validate the canonical cross-field state boundary.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.layout_version != KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V1
            || self.state_limbs.len() != KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1
            || self.state_limbs.first().copied()
                != Some(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V1)
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.state_boundary",
            });
        }
        Ok(())
    }
}

impl KagemushaPastaCycleProofEnvelopeV1 {
    /// Validate the fixed proof-envelope shape.
    ///
    /// Production verification must additionally call
    /// [`Self::validate_against_manifest`] so the otherwise well-formed
    /// generation and verifier-key fields are bound to an authenticated
    /// release manifest.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V1
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1
            || !is_kagemusha_v3_portable_identifier(&self.artifact_generation)
            || !is_kagemusha_v3_portable_identifier(&self.step_eq_parameter_generation)
            || !is_kagemusha_v3_portable_identifier(&self.step_ep_parameter_generation)
            || self.manifest_sha256 == [0; 32]
            || self.step_eq_verifier_key_sha256 == [0; 32]
            || self.step_ep_verifier_key_sha256 == [0; 32]
            || self.step_eq_verifier_key_sha256 == self.step_ep_verifier_key_sha256
            || self.proof.backend.as_str() != "halo2/ipa"
            || self.proof.bytes.is_empty()
            || self.proof.bytes.len()
                > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3 as usize
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope",
            });
        }
        if self.step_eq_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1
            || self.step_ep_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1
            || self.step_eq_circuit_id == self.step_ep_circuit_id
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.circuit_pair",
            });
        }
        self.state_boundary.validate()
    }

    /// Validate this envelope against the exact artifact release that verifies it.
    pub fn validate_against_manifest(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV3,
    ) -> Result<(), KagemushaValidationError> {
        manifest.validate()?;
        self.validate()?;

        if self.artifact_generation != manifest.generation {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.artifact_generation",
            });
        }
        let manifest_sha256: [u8; 32] = Sha256::digest(to_bytes(manifest).map_err(|_| {
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.manifest_sha256",
            }
        })?)
        .into();
        if self.manifest_sha256 != manifest_sha256 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.manifest_sha256",
            });
        }
        let [step_eq, step_ep] = manifest.profiles.as_slice() else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.profile_pair",
            });
        };
        if self.step_eq_circuit_id != step_eq.circuit_id
            || self.step_ep_circuit_id != step_ep.circuit_id
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.circuit_pair",
            });
        }
        if self.step_eq_parameter_generation != step_eq.parameter_generation
            || self.step_ep_parameter_generation != step_ep.parameter_generation
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.parameter_generation_pair",
            });
        }
        if self.step_eq_verifier_key_sha256 != step_eq.artifacts[2].payload_sha256
            || self.step_ep_verifier_key_sha256 != step_ep.artifacts[2].payload_sha256
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.verifier_key_sha256_pair",
            });
        }
        Ok(())
    }

    /// Validate this envelope for an exact chain/asset release context and block height.
    pub fn validate_against_manifest_for_context(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV3,
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
                field: "pasta_cycle.proof_envelope.release_context",
            });
        }
        Ok(())
    }
}

impl KagemushaPastaCycleProofEnvelopeV2 {
    /// Validate the current operation/protocol-identity proof-envelope shape.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V3
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V3
            || !is_kagemusha_v3_portable_identifier(&self.artifact_generation)
            || !is_kagemusha_v3_portable_identifier(&self.step_eq_parameter_generation)
            || !is_kagemusha_v3_portable_identifier(&self.step_ep_parameter_generation)
            || self.manifest_sha256 == [0; 32]
            || self.step_eq_verifier_key_sha256 == [0; 32]
            || self.step_ep_verifier_key_sha256 == [0; 32]
            || self.step_eq_verifier_key_sha256 == self.step_ep_verifier_key_sha256
            || self.proof.backend.as_str() != "halo2/ipa"
            || self.proof.bytes.is_empty()
            || self.proof.bytes.len()
                > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3 as usize
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope",
            });
        }
        if self.step_eq_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3
            || self.step_ep_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3
            || self.step_eq_circuit_id == self.step_ep_circuit_id
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.circuit_pair",
            });
        }
        self.state_boundary.validate()
    }

    /// Bind this envelope to the exact authenticated artifact manifest.
    pub fn validate_against_manifest(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV3,
    ) -> Result<(), KagemushaValidationError> {
        manifest.validate()?;
        self.validate()?;

        if self.artifact_generation != manifest.generation {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.artifact_generation",
            });
        }
        let manifest_sha256: [u8; 32] = Sha256::digest(to_bytes(manifest).map_err(|_| {
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.manifest_sha256",
            }
        })?)
        .into();
        if self.manifest_sha256 != manifest_sha256 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.manifest_sha256",
            });
        }
        let [step_eq, step_ep] = manifest.profiles.as_slice() else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.profile_pair",
            });
        };
        if self.step_eq_circuit_id != step_eq.circuit_id
            || self.step_ep_circuit_id != step_ep.circuit_id
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.circuit_pair",
            });
        }
        if self.step_eq_parameter_generation != step_eq.parameter_generation
            || self.step_ep_parameter_generation != step_ep.parameter_generation
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.parameter_generation_pair",
            });
        }
        if self.step_eq_verifier_key_sha256 != step_eq.artifacts[2].payload_sha256
            || self.step_ep_verifier_key_sha256 != step_ep.artifacts[2].payload_sha256
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.verifier_key_sha256_pair",
            });
        }
        Ok(())
    }

    /// Validate this envelope for an exact chain/asset release context and block height.
    pub fn validate_against_manifest_for_context(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV3,
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
                field: "pasta_cycle.proof_envelope.release_context",
            });
        }
        Ok(())
    }
}

impl KagemushaPastaCycleArtifactV3 {
    /// Validate one immutable V3 artifact file descriptor.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if !is_kagemusha_v3_portable_file_name(&self.file_name)
            || self.size_bytes == 0
            || self.size_bytes > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3
            || self.sha256 == [0; 32]
            || self.payload_size_bytes == 0
            || self.payload_size_bytes >= self.size_bytes
            || self.payload_size_bytes > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3
            || self.payload_sha256 == [0; 32]
            || self.payload_sha256 == self.sha256
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.artifact",
            });
        }
        Ok(())
    }
}

impl KagemushaTopUpFinalityRosterArtifactReferenceV2 {
    /// Validate the exact role-specific finality-roster file contract.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if !is_kagemusha_v3_portable_file_name(&self.file_name)
            || self.file_name != KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2
            || self.size_bytes == 0
            || self.size_bytes > KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2
            || self.sha256 == [0; 32]
            || !is_kagemusha_v3_portable_identifier(&self.artifact_generation)
            || self.circuit_id != KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2
            || self.purpose != KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2
            || self.artifact_type != KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2
            || self.required_bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.roster_artifact_reference",
            });
        }
        Ok(())
    }
}

impl KagemushaPastaCycleProofProfileV1 {
    /// Validate one fixed parity profile and its exact three-file inventory.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        let (expected_circuit, expected_file_names) = match self.parity {
            KagemushaPastaCycleParityV1::StepEq => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V3,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V3,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V3,
                ],
            ),
            KagemushaPastaCycleParityV1::StepEp => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V3,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V3,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V3,
                ],
            ),
        };
        if self.circuit_id != expected_circuit
            || !is_kagemusha_v3_portable_identifier(&self.parameter_generation)
            || self.ipa_k != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1
            || self.artifacts.len() != 3
            || self.artifacts[0].kind != KagemushaPastaCycleArtifactKindV3::Parameters
            || self.artifacts[1].kind != KagemushaPastaCycleArtifactKindV3::ProvingKey
            || self.artifacts[2].kind != KagemushaPastaCycleArtifactKindV3::VerifyingKey
            || self
                .artifacts
                .iter()
                .zip(expected_file_names)
                .any(|(artifact, expected)| artifact.file_name != expected)
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.profile",
            });
        }
        let mut names = std::collections::BTreeSet::new();
        for artifact in &self.artifacts {
            artifact.validate()?;
            if !names.insert(artifact.file_name.as_str()) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.profile.artifact_name",
                });
            }
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendArtifactManifestV3 {
    /// Validate the shape and internal consistency of an artifact release.
    ///
    /// This does not authenticate the release attestation. Callers must verify
    /// that attestation before treating the manifest or any bound envelope as
    /// production material.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3
            || self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1
            || !is_kagemusha_v3_portable_identifier(&self.generation)
            || !is_kagemusha_v3_source_commit(&self.source_commit)
            || self.source_tree_sha256 == [0; 32]
            || !is_kagemusha_v3_chain_id(&self.chain_id)
            || self.asset_scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
            || self.activation_height == 0
            || self.withdrawal_height <= self.activation_height
            || self.max_proof_bytes != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3
            || self.profiles.len() != 2
            || self.profiles[0].parity != KagemushaPastaCycleParityV1::StepEq
            || self.profiles[1].parity != KagemushaPastaCycleParityV1::StepEp
            || self.topup_finality_roster_artifact.artifact_generation != self.generation
            || self.benchmark_evidence_sha256 == [0; 32]
            || self.cryptographic_review_sha256 == [0; 32]
            || self.release_attestation_sha256 == [0; 32]
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.artifact_manifest",
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
        for profile in &self.profiles {
            profile.validate()?;
            for artifact in &profile.artifacts {
                if !names.insert(artifact.file_name.to_ascii_lowercase()) {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "pasta_cycle.artifact_manifest.artifact_name",
                    });
                }
                if !digests.insert(artifact.sha256) {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "pasta_cycle.artifact_manifest.artifact_sha256",
                    });
                }
                if !digests.insert(artifact.payload_sha256) {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "pasta_cycle.artifact_manifest.artifact_payload_sha256",
                    });
                }
            }
        }
        for evidence_digest in [
            self.benchmark_evidence_sha256,
            self.cryptographic_review_sha256,
            self.release_attestation_sha256,
        ] {
            if !digests.insert(evidence_digest) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.artifact_manifest.evidence_sha256",
                });
            }
        }
        Ok(())
    }

    /// Build the non-circular subject signed by every release authority.
    ///
    /// The manifest's `release_attestation_sha256` slot is zeroed only for this digest. The final
    /// canonical manifest still binds the digest of the complete signed envelope.
    pub fn release_attestation_subject(
        &self,
    ) -> Result<KagemushaRecursiveSpendReleaseAttestationSubjectV1, KagemushaReleaseVerificationError>
    {
        let mut subject_manifest = self.clone();
        subject_manifest.release_attestation_sha256 = [0; 32];
        let subject_bytes = to_bytes(&subject_manifest)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        Ok(KagemushaRecursiveSpendReleaseAttestationSubjectV1 {
            manifest_subject_sha256: Sha256::digest(subject_bytes).into(),
            generation: self.generation.clone(),
            source_commit: self.source_commit.clone(),
            source_tree_sha256: self.source_tree_sha256,
            source_repo_dirty: self.source_repo_dirty,
            benchmark_evidence_sha256: self.benchmark_evidence_sha256,
            cryptographic_review_sha256: self.cryptographic_review_sha256,
        })
    }
}

impl KagemushaPastaPublicLayoutV4 {
    /// Derive every dynamic offset from the authenticated IPA round count.
    pub fn for_ipa_round_count(ipa_round_count: u32) -> Result<Self, KagemushaValidationError> {
        let accumulator_limbs = ipa_round_count
            .checked_add(1)
            .and_then(|value| value.checked_mul(8))
            .and_then(|value| value.checked_add(2))
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.public_layout.accumulator_limbs",
            })?;
        let semantic_prefix = u32::try_from(
            8 + KAGEMUSHA_RECURSIVE_SPEND_STEP_OPERATION_LIMBS_V3
                + 1
                + 3 * KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1
                + 8
                + 16,
        )
        .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "pasta_cycle.v4.public_layout.semantic_prefix",
        })?;
        let parent_eq_accumulator_offset = semantic_prefix;
        let parent_ep_accumulator_offset = parent_eq_accumulator_offset
            .checked_add(accumulator_limbs)
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.public_layout.ep_accumulator_offset",
            })?;
        let parent_eq_deferred_offset = parent_ep_accumulator_offset
            .checked_add(accumulator_limbs)
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.public_layout.eq_deferred_offset",
            })?;
        let parent_ep_deferred_offset = parent_eq_deferred_offset.checked_add(16).ok_or(
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.public_layout.ep_deferred_offset",
            },
        )?;
        let live_selector_offset = parent_ep_deferred_offset.checked_add(16).ok_or(
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
            || phase_count == 0
            || phase_count > KAGEMUSHA_STEP_CIRCUIT_MAX_PHASES_V4
            || phase_count != self.num_lookup_advice_per_phase.len()
            || self
                .num_advice_per_phase
                .iter()
                .any(|count| *count == 0 || *count > KAGEMUSHA_STEP_CIRCUIT_MAX_COLUMNS_V4)
            || self
                .num_lookup_advice_per_phase
                .iter()
                .any(|count| *count > KAGEMUSHA_STEP_CIRCUIT_MAX_COLUMNS_V4)
            || self
                .num_lookup_advice_per_phase
                .iter()
                .all(|count| *count == 0)
            || self.num_fixed == 0
            || self.num_fixed > KAGEMUSHA_STEP_CIRCUIT_MAX_COLUMNS_V4
            || self.lookup_bits == 0
            || self.lookup_bits >= self.k
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

    /// Domain-separated identity of the canonical authenticated parameters.
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
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if !is_kagemusha_v3_portable_file_name(&self.file_name)
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
            || !is_kagemusha_v3_portable_identifier(&self.generation)
            || self.circuit_id != expected_circuit_id
            || !is_kagemusha_v3_portable_identifier(&self.parameter_generation)
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
    pub fn validate_against_manifest(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        descriptor: &KagemushaPastaCycleArtifactV4,
    ) -> Result<(), KagemushaValidationError> {
        manifest.validate()?;
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
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if !is_kagemusha_v3_portable_file_name(&self.file_name)
            || self.file_name != KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4
            || self.size_bytes == 0
            || self.size_bytes > KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2
            || self.sha256 == [0; 32]
            || !is_kagemusha_v3_portable_identifier(&self.artifact_generation)
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
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        let (expected_circuit, expected_file_names) = match self.parity {
            KagemushaPastaCycleParityV1::StepEq => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
                ],
            ),
            KagemushaPastaCycleParityV1::StepEp => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
                ],
            ),
        };
        self.circuit_params.validate()?;
        if self.circuit_id != expected_circuit
            || !is_kagemusha_v3_portable_identifier(&self.parameter_generation)
            || self.ipa_k != self.circuit_params.k
            || self.compiled_protocol_structure_sha256 == [0; 32]
            || self.step_proof_size_bytes == 0
            || self.step_proof_size_bytes != self.circuit_params.max_parent_proof_bytes
            || self.artifacts.len() != 4
            || self.artifacts[0].kind != KagemushaPastaCycleArtifactKindV4::Parameters
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
    pub fn circuit_params_sha256(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        self.circuit_params.sha256()
    }
}

impl KagemushaRecursiveSpendArtifactManifestV4 {
    /// Validate the complete, explicitly versioned V4 release shape.
    ///
    /// This validates content binding only. A V4 release attestation must be
    /// authenticated separately before any artifact is used.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        self.validate_with_attestation_state(true)
    }

    /// Validate an immutable V4 release candidate before its attestation exists.
    ///
    /// Candidate manifests commit every release field except the necessarily
    /// circular attestation digest. They are not valid release manifests and
    /// must never be accepted by production artifact readers.
    pub fn validate_unsigned_candidate(&self) -> Result<(), KagemushaValidationError> {
        self.validate_with_attestation_state(false)
    }

    fn validate_with_attestation_state(
        &self,
        finalized: bool,
    ) -> Result<(), KagemushaValidationError> {
        let measured_step_bytes = self.profiles.iter().try_fold(0_u32, |sum, profile| {
            sum.checked_add(profile.step_proof_size_bytes)
        });
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4
            || self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4
            || !is_kagemusha_v3_portable_identifier(&self.generation)
            || !is_kagemusha_v3_source_commit(&self.source_commit)
            || self.source_tree_sha256 == [0; 32]
            || !is_kagemusha_v3_chain_id(&self.chain_id)
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
            || self.benchmark_evidence_sha256 == [0; 32]
            || self.cryptographic_review_sha256 == [0; 32]
            || (finalized && self.release_attestation_sha256 == [0; 32])
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
        if finalized && !digests.insert(self.release_attestation_sha256) {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.artifact_manifest.evidence_sha256",
            });
        }
        Ok(())
    }

    /// Build the non-circular V4 subject signed by every release authority.
    pub fn release_attestation_subject(
        &self,
    ) -> Result<KagemushaRecursiveSpendReleaseAttestationSubjectV4, KagemushaReleaseVerificationError>
    {
        self.validate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        self.release_attestation_subject_from_validated_manifest()
    }

    /// Build the signing subject for a validated unsigned V4 candidate.
    ///
    /// The returned subject is byte-for-byte identical to the subject derived
    /// from the corresponding finalized manifest after its attestation digest
    /// is populated.
    pub fn release_attestation_candidate_subject(
        &self,
    ) -> Result<KagemushaRecursiveSpendReleaseAttestationSubjectV4, KagemushaReleaseVerificationError>
    {
        self.validate_unsigned_candidate()
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
            benchmark_evidence_sha256: self.benchmark_evidence_sha256,
            cryptographic_review_sha256: self.cryptographic_review_sha256,
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

impl KagemushaRecursiveSpendReleaseAttestationSubjectV1 {
    /// Return the exact domain- and role-separated payload signed by one authority.
    #[must_use]
    pub fn approval_payload(
        &self,
        role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
    ) -> KagemushaRecursiveSpendReleaseApprovalPayloadV1 {
        KagemushaRecursiveSpendReleaseApprovalPayloadV1 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_APPROVAL_DOMAIN_V1.to_owned(),
            role,
            subject: self.clone(),
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
    pub fn validate(&self) -> Result<(), KagemushaReleaseVerificationError> {
        let expected_roles = [
            KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
        ];
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1
            || !is_kagemusha_v3_portable_identifier(&self.policy_id)
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

impl KagemushaAuthenticatedReleaseV3 {
    /// Verify the signed envelope after the public entry point has hash-checked evidence bytes.
    fn verify_attestation(
        manifest: &KagemushaRecursiveSpendArtifactManifestV3,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
        attestation: &KagemushaRecursiveSpendReleaseAttestationV1,
    ) -> Result<Self, KagemushaReleaseVerificationError> {
        manifest
            .validate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        policy.validate()?;

        let expected_subject = manifest.release_attestation_subject()?;
        if attestation.schema != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V1
            || attestation.version != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1
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

    /// Authenticate a release and hash-check the exact evidence files it references.
    pub fn verify(
        manifest: &KagemushaRecursiveSpendArtifactManifestV3,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
        attestation: &KagemushaRecursiveSpendReleaseAttestationV1,
        benchmark_evidence: &[u8],
        cryptographic_review: &[u8],
    ) -> Result<Self, KagemushaReleaseVerificationError> {
        for (role, bytes, expected_digest) in [
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
                benchmark_evidence,
                manifest.benchmark_evidence_sha256,
            ),
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
                cryptographic_review,
                manifest.cryptographic_review_sha256,
            ),
        ] {
            if bytes.is_empty()
                || bytes.len() > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1
                || <[u8; 32]>::from(Sha256::digest(bytes)) != expected_digest
            {
                return Err(KagemushaReleaseVerificationError::EvidenceMismatch { role });
            }
        }
        Self::verify_attestation(manifest, policy, attestation)
    }

    /// Authenticated manifest selected by this runtime proof.
    #[must_use]
    pub fn manifest(&self) -> &KagemushaRecursiveSpendArtifactManifestV3 {
        &self.manifest
    }

    /// SHA-256 of the exact canonical manifest.
    #[must_use]
    pub const fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }

    /// SHA-256 of the exact signed release envelope.
    #[must_use]
    pub const fn release_attestation_sha256(&self) -> [u8; 32] {
        self.release_attestation_sha256
    }

    /// SHA-256 of the exact locally trusted release policy.
    #[must_use]
    pub const fn release_policy_sha256(&self) -> [u8; 32] {
        self.release_policy_sha256
    }

    /// Canonically ordered role/signer identities whose approvals were verified.
    #[must_use]
    pub fn approved_signers(&self) -> &[KagemushaRecursiveSpendApprovedSignerV1] {
        &self.approved_signers
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
    pub fn verify(
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
        attestation: &KagemushaRecursiveSpendReleaseAttestationV4,
        benchmark_evidence: &[u8],
        cryptographic_review: &[u8],
    ) -> Result<Self, KagemushaReleaseVerificationError> {
        for (role, bytes, expected_digest) in [
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
                benchmark_evidence,
                manifest.benchmark_evidence_sha256,
            ),
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
                cryptographic_review,
                manifest.cryptographic_review_sha256,
            ),
        ] {
            if bytes.is_empty()
                || bytes.len() > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1
                || <[u8; 32]>::from(Sha256::digest(bytes)) != expected_digest
            {
                return Err(KagemushaReleaseVerificationError::EvidenceMismatch { role });
            }
        }
        Self::verify_attestation(manifest, policy, attestation)
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
    /// Validate the standalone ABI-20 promotion marker.
    pub fn validate(&self) -> Result<(), KagemushaReleaseVerificationError> {
        let digests = [
            self.manifest_sha256,
            self.release_attestation_sha256,
            self.release_policy_sha256,
        ];
        let digests_are_distinct_and_nonzero = digests.iter().all(|digest| *digest != [0; 32])
            && digests[0] != digests[1]
            && digests[0] != digests[2]
            && digests[1] != digests[2];
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
            || !is_kagemusha_v3_portable_identifier(&self.generation)
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
    pub fn validate_against_authenticated_release(
        &self,
        release: &KagemushaAuthenticatedReleaseV4,
    ) -> Result<(), KagemushaReleaseVerificationError> {
        self.validate()?;
        if self.generation != release.manifest().generation
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
}

impl KagemushaRecursiveSpendArtifactBindingV3 {
    /// Validate a complete authenticated manifest identity.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if !is_kagemusha_v3_portable_identifier(&self.generation) {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "artifact_binding.generation",
            });
        }
        if self.manifest_sha256 == [0; 32] {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "artifact_binding.manifest_sha256",
            });
        }
        Ok(())
    }

    /// Require this binding to identify the supplied validated manifest bytes.
    pub fn validate_manifest(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV3,
        canonical_manifest_bytes: &[u8],
    ) -> Result<(), KagemushaValidationError> {
        self.validate()?;
        manifest.validate()?;
        let digest: [u8; 32] = Sha256::digest(canonical_manifest_bytes).into();
        if self.generation != manifest.generation || self.manifest_sha256 != digest {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "artifact_binding.manifest",
            });
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendArtifactBindingV4 {
    /// Validate a complete authenticated V4 manifest identity.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if !is_kagemusha_v3_portable_identifier(&self.generation) || self.manifest_sha256 == [0; 32]
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "artifact_binding.v4",
            });
        }
        Ok(())
    }

    /// Require this binding to identify supplied canonical V4 manifest bytes.
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
    /// Validate an installed ABI-20 backend capability record.
    ///
    /// `max_proof_bytes` is deliberately release-specific: it must come from
    /// the authenticated V4 manifest selected by the installed artifact
    /// handle, rather than from a compile-time default.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        let expected_roles = KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.map(str::to_owned);
        let missing_gates_are_canonical = !self.missing_gates.is_empty()
            && self.missing_gates.len() <= KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
            && self
                .missing_gates
                .iter()
                .all(|gate| is_kagemusha_v3_portable_identifier(gate))
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
    /// Validate the fixed ABI-20 envelope shape before release lookup.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4
            || self.step_eq_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4
            || self.step_ep_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4
            || self.step_eq_circuit_id == self.step_ep_circuit_id
            || !is_kagemusha_v3_portable_identifier(&self.artifact_generation)
            || !is_kagemusha_v3_portable_identifier(&self.step_eq_parameter_generation)
            || !is_kagemusha_v3_portable_identifier(&self.step_ep_parameter_generation)
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
    pub fn validate_against_manifest(
        &self,
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    ) -> Result<(), KagemushaValidationError> {
        manifest.validate()?;
        self.validate()?;
        let [step_eq, step_ep] = manifest.profiles.as_slice() else {
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
            || self.step_eq_circuit_id != step_eq.circuit_id
            || self.step_ep_circuit_id != step_ep.circuit_id
            || self.step_eq_parameter_generation != step_eq.parameter_generation
            || self.step_ep_parameter_generation != step_ep.parameter_generation
            || self.step_eq_circuit_params_sha256 != step_eq.circuit_params_sha256()?
            || self.step_ep_circuit_params_sha256 != step_ep.circuit_params_sha256()?
            || self.step_eq_verifier_key_sha256 != step_eq.artifacts[2].payload_sha256
            || self.step_ep_verifier_key_sha256 != step_ep.artifacts[2].payload_sha256
            || self.proof.bytes.len() > manifest.max_proof_bytes as usize
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.proof_envelope.manifest_binding",
            });
        }
        Ok(())
    }

    /// Validate the envelope in an exact chain, asset, scale, and height context.
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

impl KagemushaRecursiveSpendNativeCapabilitiesV1 {
    /// Validate that a capability record exactly describes this bridge contract.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3
            || self.artifact_manifest_schema
                != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1
            || self.proof_envelope_version
                != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V3
            || self.state_boundary_version != KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V1
            || self.step_eq_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3
            || self.step_ep_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3
            || self.max_proof_bytes != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3
            || self.proof_backend_available != KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE
            || (self.proof_backend_available && !self.missing_gates.is_empty())
            || (!self.proof_backend_available && self.missing_gates != kagemusha_v3_missing_gates())
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.capabilities",
            });
        }
        Ok(())
    }
}

/// Return the canonical native bridge capability record for this build.
#[must_use]
pub fn kagemusha_recursive_spend_native_capabilities_v1()
-> KagemushaRecursiveSpendNativeCapabilitiesV1 {
    KagemushaRecursiveSpendNativeCapabilitiesV1 {
        bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
        artifact_manifest_schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3.to_owned(),
        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1.to_owned(),
        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1.to_owned(),
        proof_envelope_version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V3,
        state_boundary_version: KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V1,
        step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3.to_owned(),
        step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3.to_owned(),
        max_proof_bytes: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3,
        proof_backend_available: KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE,
        missing_gates: if KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE {
            Vec::new()
        } else {
            kagemusha_v3_missing_gates()
        },
    }
}

fn kagemusha_v3_missing_gates() -> Vec<String> {
    [
        "paired_deferred_verifier",
        "proof_bound_output_membership_witnesses",
        "authenticated_release_envelope",
        "independent_cryptographic_review",
        "physical_device_performance_evidence",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// Return whether `value` is a canonical cross-platform V3 artifact identifier.
///
/// Identifiers use the same single-component restrictions as artifact file
/// names so release caches cannot alias punctuation-only or Windows device
/// names across build hosts.
#[must_use]
pub fn is_kagemusha_v3_portable_identifier(value: &str) -> bool {
    is_kagemusha_v3_portable_file_name(value)
}

fn is_kagemusha_v3_portable_file_name(value: &str) -> bool {
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

fn is_kagemusha_v3_source_commit(value: &str) -> bool {
    value.len() == 40
        && value.bytes().any(|byte| byte != b'0')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_kagemusha_v3_chain_id(value: &ChainId) -> bool {
    let value = value.as_str();
    !value.is_empty()
        && value.len() <= 128
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

impl KagemushaTopUpShieldEvidenceV2 {
    /// Validate the typed proof envelope before authoritative ledger checks.
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

impl KagemushaRecursiveSpendTopUpUnsignedV2 {
    /// Validate every proof, amount, note, operation, and artifact field before signing.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.amount.validate()?;
        self.current_note.validate_public_binding()?;
        self.artifact_binding.validate()?;
        self.shield_evidence.validate_public_binding()?;
        if self.current_note.asset != *self.asset.definition() {
            return Err(KagemushaValidationError::RecursiveSpendAssetMismatch);
        }
        if self.current_note.amount != self.amount {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote { field: "amount" });
        }
        if self.operation_id == [0; 32] {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "operation_id",
            });
        }
        if self.current_note.note_commitment == self.current_note.spend_nullifier {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "current_note",
            });
        }
        Ok(())
    }

    /// Return the canonical digest placed into the authorization payload.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaTopUpUnsignedPayloadDigestPreimageV2 {
            domain: KAGEMUSHA_TOPUP_PAYLOAD_DIGEST_DOMAIN_V2.to_owned(),
            asset: self.asset.clone(),
            amount: self.amount,
            current_note: self.current_note.clone(),
            shield_evidence: self.shield_evidence.clone(),
            artifact_binding: self.artifact_binding.clone(),
            operation_id: self.operation_id,
        })
    }

    /// Attach a matching payer authorization and produce the chain-facing request.
    pub fn into_request(
        self,
        authorization: KagemushaRequestAuthorizationV2,
    ) -> Result<KagemushaRecursiveSpendTopUpRequestV2, KagemushaValidationError> {
        let request = KagemushaRecursiveSpendTopUpRequestV2 {
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

impl KagemushaRecursiveSpendTopUpRequestV2 {
    /// Construct and validate a scale-bound online-to-offline request.
    pub fn new(
        asset: AssetId,
        amount: KagemushaScaledAmountV2,
        current_note: KagemushaSpendableNoteDescriptorV2,
        shield_evidence: KagemushaTopUpShieldEvidenceV2,
        artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
        operation_id: [u8; 32],
        authorization: KagemushaRequestAuthorizationV2,
    ) -> Result<Self, KagemushaValidationError> {
        KagemushaRecursiveSpendTopUpUnsignedV2 {
            asset,
            amount,
            current_note,
            shield_evidence,
            artifact_binding,
            operation_id,
        }
        .into_request(authorization)
    }

    /// Reconstruct the exact canonical fields that were signed.
    #[must_use]
    pub fn unsigned_payload(&self) -> KagemushaRecursiveSpendTopUpUnsignedV2 {
        KagemushaRecursiveSpendTopUpUnsignedV2 {
            asset: self.asset.clone(),
            amount: self.amount,
            current_note: self.current_note.clone(),
            shield_evidence: self.shield_evidence.clone(),
            artifact_binding: self.artifact_binding.clone(),
            operation_id: self.operation_id,
        }
    }

    /// Validate the charged asset and exact first-hop amount binding.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        let unsigned = self.unsigned_payload();
        unsigned.validate_public_binding()?;
        if self.asset.account() != &self.authorization.authority {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "authorization.authority",
            });
        }
        if self.authorization.operation_id != self.operation_id {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "authorization.operation_id",
            });
        }
        self.authorization
            .validate_for_payload(unsigned.digest()?)?;
        Ok(())
    }

    /// Digest of all unsigned debit/proof fields covered by authorization.
    pub fn unsigned_payload_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.unsigned_payload().digest()
    }

    /// Verify authorization against authoritative Torii time.
    pub fn validate_authorization_at(&self, now_ms: u64) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        self.authorization
            .validate_for_payload_at(self.unsigned_payload_digest()?, now_ms)
    }
}

impl KagemushaRecursiveSpendTopUpAnchorV2 {
    /// Populate and validate the canonical receipt digest.
    pub fn finalize_digest(mut self) -> Result<Self, KagemushaValidationError> {
        self.anchor_digest = self.compute_anchor_digest()?;
        self.validate_public_binding()?;
        Ok(self)
    }

    /// Compute the digest bound into the later recursive init/redemption proof.
    pub fn compute_anchor_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        kagemusha_poseidon_preimage(&KagemushaTopUpAnchorDigestPreimageV2 {
            domain: KAGEMUSHA_TOPUP_ANCHOR_DIGEST_DOMAIN_V2.to_owned(),
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

    /// Validate the immutable transfer, amount, operation, and finality bindings.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.amount.validate()?;
        self.current_note.validate_public_binding()?;
        self.artifact_binding.validate()?;
        if self.version != 2
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
                field: "topup_anchor",
            });
        }
        Ok(())
    }

    /// Return the compact identity carried by peer and redemption archives.
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

impl KagemushaRecursiveSpendTopUpAnchorRefV2 {
    /// Validate a non-zero chain-resolvable identity pair.
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
            || !is_kagemusha_v3_chain_id(&self.chain_id)
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
    pub fn validate_structure(&self) -> Result<(), KagemushaValidationError> {
        let context = &self.height_context;
        let certificate = &self.certificate;
        context.validate_structure()?;
        if certificate.round.context_id != context.context_id
            || certificate.round.height != context.height
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
            || self.siblings.iter().any(|sibling| *sibling == [0; 32])
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
    pub fn validate_structure(&self) -> Result<(), KagemushaValidationError> {
        if self.version != KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2
            || !is_kagemusha_v3_chain_id(&self.chain_id)
            || !is_kagemusha_v3_portable_identifier(&self.artifact_generation)
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
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        self.validate_structure()?;
        for window in &self.windows {
            window.validate()?;
        }
        Ok(())
    }

    /// Select exactly one trusted roster for `height`.
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
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        kagemusha_poseidon_preimage(&KagemushaUnshieldPublicInputsDigestPreimageV2 {
            domain: KAGEMUSHA_UNSHIELD_PUBLIC_INPUTS_DIGEST_DOMAIN_V2.to_owned(),
            public_inputs: *self,
        })
    }
}

impl KagemushaRecursiveSpendRedemptionIntentV2 {
    /// Validate exact full/partial conservation and canonical unshield public words.
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
                field: "redemption",
            });
        }
        let zero = [0u8; 32];
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
                field: "redemption.unshield_public_inputs",
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
                        field: "redemption.change_output",
                    });
                }
            }
            _ => {
                return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                    field: "redemption.change_output",
                });
            }
        }
        Ok(())
    }

    /// Return the domain-separated circuit binding for this redemption transition.
    pub fn binding_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRedemptionTransitionDigestPreimageV2 {
            domain: KAGEMUSHA_REDEMPTION_TRANSITION_DIGEST_DOMAIN_V2.to_owned(),
            intent: self.clone(),
        })
    }
}

impl KagemushaRecursiveSpendRedemptionIntentBuildRequestV2 {
    /// Consume an opaque parent bundle and derive every parent-bound redemption field.
    pub fn into_intent(
        self,
    ) -> Result<KagemushaRecursiveSpendRedemptionIntentV2, KagemushaValidationError> {
        self.previous_bundle.validate_public_binding()?;
        let parent_bundle_digest = self.previous_bundle.digest()?;
        let statement = self.previous_bundle.statement;
        if self
            .change_artifact_binding
            .as_ref()
            .is_some_and(|binding| binding != &statement.artifact_binding)
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redemption.change_artifact_binding",
            });
        }
        let intent = KagemushaRecursiveSpendRedemptionIntentV2 {
            chain_id: statement.chain_id,
            asset: statement.asset,
            input_note: statement.current_note,
            parent_branch_claims: statement.branch_claims,
            parent_topup_anchor_refs: statement.topup_anchor_refs,
            parent_proof_step_count: statement.proof_step_count,
            parent_peer_hop_count: statement.peer_hop_count,
            parent_bundle_digest,
            input_root: statement.final_root,
            recipient: self.recipient,
            public_amount: self.public_amount,
            change_output: self.change_output,
            change_artifact_binding: self.change_artifact_binding,
            unshield_public_inputs: self.unshield_public_inputs,
            unshield_public_inputs_digest: self.unshield_public_inputs_digest,
            operation_id: self.operation_id,
        };
        intent.validate_public_binding()?;
        Ok(intent)
    }
}

impl KagemushaRecursiveSpendTransitionV2 {
    /// Return the peer-split transition when this is an ordinary offline hop.
    #[must_use]
    pub fn as_peer_split(&self) -> Option<&KagemushaRecursiveSpendPeerSplitTransitionV2> {
        match self {
            Self::PeerSplit(transition) => Some(transition),
            Self::RedemptionChange(_) => None,
        }
    }

    /// Return the partial-redemption change transition when present.
    #[must_use]
    pub fn as_redemption_change(
        &self,
    ) -> Option<&KagemushaRecursiveSpendRedemptionChangeTransitionV2> {
        match self {
            Self::PeerSplit(_) => None,
            Self::RedemptionChange(transition) => Some(transition),
        }
    }
}

impl KagemushaRecursiveSpendPeerSplitTransitionV2 {
    /// Project the compact proof-bound transition carried by each output bundle.
    pub fn from_intent(
        intent: &KagemushaRecursiveSpendSplitIntentV2,
        branch: KagemushaRecursiveSpendBranchV2,
    ) -> Result<Self, KagemushaValidationError> {
        intent.validate_public_binding()?;
        if matches!(branch, KagemushaRecursiveSpendBranchV2::Change)
            && intent.change_output.is_none()
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split.change_output",
            });
        }
        Ok(Self {
            binding_digest: intent.binding_digest()?,
            branch,
            recipient_request_digest: intent.recipient_request_digest,
            operation_id: intent.operation_id,
            parent_max_proof_step_count: intent
                .inputs
                .iter()
                .map(|input| input.proof_step_count)
                .max()
                .unwrap_or(0),
            parent_max_peer_hop_count: intent
                .inputs
                .iter()
                .map(|input| input.peer_hop_count)
                .max()
                .unwrap_or(0),
        })
    }
}

impl KagemushaRecursiveSpendRedemptionChangeTransitionV2 {
    /// Project the compact proof-bound transition carried by a change bundle.
    pub fn from_intent(
        intent: &KagemushaRecursiveSpendRedemptionIntentV2,
    ) -> Result<Self, KagemushaValidationError> {
        intent.validate_public_binding()?;
        if intent.change_output.is_none() || intent.change_artifact_binding.is_none() {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "redemption.change_output",
            });
        }
        Ok(Self {
            binding_digest: intent.binding_digest()?,
            parent_bundle_digest: intent.parent_bundle_digest,
            operation_id: intent.operation_id,
            parent_proof_step_count: intent.parent_proof_step_count,
            parent_peer_hop_count: intent.parent_peer_hop_count,
        })
    }
}

impl KagemushaRecursiveSpendRedeemBuildResultV2 {
    /// Validate the atomic unsigned request and optional change/witness package.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.unsigned.validate_public_binding()?;
        if self.operation_id == [0; 32]
            || self.operation_id != self.unsigned.operation_id
            || self.authorization_digest != self.unsigned.digest()?
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_result",
            });
        }
        match (
            &self.unsigned.offline_change,
            &self.offline_change_bundle,
            &self.offline_change_membership_witness,
        ) {
            (None, None, None) => Ok(()),
            (Some(branch), Some(bundle), Some(witness)) if &branch.bundle == bundle => {
                branch.validate_for_redemption(&self.unsigned.bundle, &self.unsigned.redemption)?;
                witness.validate_for_root(bundle.statement.final_root)
            }
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_result.offline_change",
            }),
        }
    }

    /// Validate the prepared unsigned request and optional proof-bound change.
    pub fn validate_for_request(
        &self,
        request: &KagemushaRecursiveSpendRedeemBuildRequestV2,
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
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_result",
            });
        }
        if self.offline_change_bundle.is_some() != request.redemption.change_output.is_some() {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_result.offline_change",
            });
        }
        Ok(())
    }

    /// Attach recipient authorization without dropping the local change witness.
    pub fn into_redeem_result(
        self,
        authorization: KagemushaRequestAuthorizationV2,
    ) -> Result<KagemushaRecursiveSpendRedeemResultV2, KagemushaValidationError> {
        self.validate_public_binding()?;
        let operation_id = self.operation_id;
        let offline_change_bundle = self.offline_change_bundle;
        let offline_change_membership_witness = self.offline_change_membership_witness;
        let request = self.unsigned.into_request(authorization)?;
        let result = KagemushaRecursiveSpendRedeemResultV2 {
            redeem_request_archive: to_bytes(&request)?,
            offline_change_bundle,
            offline_change_membership_witness,
            operation_id,
        };
        result.validate_public_binding()?;
        Ok(result)
    }
}

impl KagemushaRecursiveSpendSplitResultV2 {
    /// Construct and validate independently spendable recipient/change branches.
    pub fn new(
        split: KagemushaRecursiveSpendSplitIntentV2,
        split_binding_digest: [u8; 32],
        recipient_bundle: KagemushaRecursiveSpendBundleV2,
        recipient_membership_witness: KagemushaNoteMembershipWitnessV2,
        change_bundle: Option<KagemushaRecursiveSpendBundleV2>,
        change_membership_witness: Option<KagemushaNoteMembershipWitnessV2>,
    ) -> Result<Self, KagemushaValidationError> {
        let result = Self {
            split,
            split_binding_digest,
            recipient_bundle,
            recipient_membership_witness,
            change_bundle,
            change_membership_witness,
        };
        result.validate_public_binding()?;
        Ok(result)
    }

    /// Validate conservation plus the shared-parent and disjoint-branch bindings.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.split.validate_public_binding()?;
        if self.split_binding_digest != self.split.binding_digest()? {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "split_binding_digest",
            });
        }
        self.validate_branch(
            &self.recipient_bundle,
            KagemushaRecursiveSpendBranchV2::Recipient,
        )?;
        self.recipient_membership_witness
            .validate_for_root(self.recipient_bundle.statement.final_root)?;

        match (
            &self.split.change_output,
            &self.change_bundle,
            &self.change_membership_witness,
        ) {
            (None, None, None) => Ok(()),
            (Some(_), Some(change_bundle), Some(change_membership_witness)) => {
                self.validate_branch(change_bundle, KagemushaRecursiveSpendBranchV2::Change)?;
                change_membership_witness.validate_for_root(change_bundle.statement.final_root)?;
                let recipient = &self.recipient_bundle.statement;
                let change = &change_bundle.statement;
                if recipient.chain_id != change.chain_id
                    || recipient.final_root != change.final_root
                    || recipient.topup_anchor_refs != change.topup_anchor_refs
                    || recipient.proof_step_count != change.proof_step_count
                    || recipient.peer_hop_count != change.peer_hop_count
                    || recipient.branch_claims.len() != change.branch_claims.len()
                    || recipient.artifact_binding != change.artifact_binding
                    || recipient.verifier_key_id != change.verifier_key_id
                    || self.recipient_membership_witness.leaf_index
                        == change_membership_witness.leaf_index
                    || self.recipient_membership_witness.dummy_input_path
                        != change_membership_witness.dummy_input_path
                {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "split.parent_branch_lineage",
                    });
                }
                if recipient.asset != change.asset {
                    return Err(KagemushaValidationError::RecursiveSpendAssetMismatch);
                }
                if self.recipient_bundle == *change_bundle {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "split.distinct_branches",
                    });
                }
                if recipient.branch_claims.iter().any(|recipient_claim| {
                    change
                        .branch_claims
                        .iter()
                        .any(|change_claim| recipient_claim.path.conflicts_with(change_claim.path))
                }) {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "split.branch_claims",
                    });
                }
                Ok(())
            }
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split.change_bundle",
            }),
        }
    }

    fn validate_branch(
        &self,
        bundle: &KagemushaRecursiveSpendBundleV2,
        expected_branch: KagemushaRecursiveSpendBranchV2,
    ) -> Result<(), KagemushaValidationError> {
        self.split.validate_output_bundle(bundle, expected_branch)
    }

    /// Fail closed until branch-independent V2 proving is linked.
    pub fn ensure_proof_backend_available(&self) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        Err(KagemushaValidationError::RecursiveSpendV2ProofBackendUnavailable)
    }
}

#[cfg(test)]
mod kagemusha_v4_artifact_contract_tests {
    use crate::domain::DomainId;

    use super::*;

    fn digest(label: &[u8]) -> [u8; 32] {
        Sha256::digest(label).into()
    }

    fn circuit_params() -> KagemushaStepCircuitParamsV4 {
        let k = KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
        let layout =
            KagemushaPastaPublicLayoutV4::for_ipa_round_count(k).expect("test V4 public layout");
        KagemushaStepCircuitParamsV4 {
            version: KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
            k,
            num_advice_per_phase: vec![8, 1, 1],
            num_lookup_advice_per_phase: vec![1, 0, 0],
            num_fixed: 1,
            lookup_bits: k - 1,
            num_instance_columns: 1,
            public_input_limbs: layout.instance_column_limbs,
            minimum_unusable_rows: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
            max_parent_proof_bytes: 4_096,
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
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
                ],
            ),
            KagemushaPastaCycleParityV1::StepEp => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
                ],
            ),
        };
        let kinds = [
            KagemushaPastaCycleArtifactKindV4::Parameters,
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
        KagemushaRecursiveSpendArtifactManifestV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
            generation: "v4-artifact-test-release".to_owned(),
            source_commit: "1234567890abcdef1234567890abcdef12345678".to_owned(),
            source_tree_sha256: digest(b"v4 artifact test source tree"),
            source_repo_dirty: true,
            chain_id: ChainId::from("v4-artifact-test-chain"),
            asset: AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").expect("test domain"),
                "rose".parse().expect("test asset name"),
            ),
            asset_scale: 9,
            activation_height: 1,
            withdrawal_height: 100,
            max_proof_bytes: 9_000,
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
            manifest_sha256: digest(b"v4 promotion manifest"),
            release_attestation_sha256: digest(b"v4 promotion attestation"),
            release_policy_sha256: digest(b"v4 promotion policy"),
            approved_signers,
            artifact_inventory_verified: true,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            artifact_roles: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
                .map(str::to_owned)
                .to_vec(),
            max_proof_bytes: 9_000,
        }
    }

    #[test]
    fn v4_promotion_record_is_distinct_and_fail_closed() {
        let record = promoted_release();
        record.validate().expect("valid V4 promotion record");

        let mut tampered = record.clone();
        tampered.schema = KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V3.to_owned();
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
        tampered.bridge_abi_version = KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3;
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
                    KagemushaPastaCycleArtifactKindV4::Parameters,
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
                KagemushaPastaCycleArtifactKindV4::Parameters => 0,
                KagemushaPastaCycleArtifactKindV4::ProvingKey => 1,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey => 2,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness => 3,
            }
        }

        let kinds = [
            KagemushaPastaCycleArtifactKindV4::Parameters,
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
    fn v4_envelope_uses_verifying_key_role_at_canonical_index() {
        let manifest = manifest();
        manifest.validate().expect("valid V4 manifest");
        let [step_eq, step_ep] = manifest.profiles.as_slice() else {
            panic!("test manifest must have Eq/Ep profiles");
        };
        let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1];
        state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V1;
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
            state_boundary: KagemushaRecursiveSpendStateBoundaryV1 {
                layout_version: KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V1,
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
        let review = b"independent V4 cryptographic review evidence".to_vec();
        let mut manifest = manifest();
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
        params_tamper.profiles[0].circuit_params.num_fixed += 1;
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

        let mut signed_params_tamper = manifest.clone();
        signed_params_tamper.profiles[0].circuit_params.num_fixed += 1;
        assert_eq!(
            KagemushaAuthenticatedReleaseV4::verify(
                &signed_params_tamper,
                &policy,
                &attestation,
                &benchmark,
                &review,
            ),
            Err(KagemushaReleaseVerificationError::InvalidAttestation)
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
            Err(KagemushaReleaseVerificationError::InvalidAttestation)
        );
    }

    #[test]
    fn v4_unsigned_candidate_subject_matches_finalized_manifest_subject() {
        let finalized = manifest();
        finalized.validate().expect("valid finalized V4 manifest");
        assert!(
            finalized.validate_unsigned_candidate().is_err(),
            "a finalized manifest must not be accepted as an unsigned candidate"
        );

        let mut candidate = finalized.clone();
        candidate.release_attestation_sha256 = [0; 32];
        assert!(
            candidate.validate().is_err(),
            "production manifest validation must reject an unsigned candidate"
        );
        candidate
            .validate_unsigned_candidate()
            .expect("valid unsigned V4 candidate");
        assert_eq!(
            candidate
                .release_attestation_candidate_subject()
                .expect("candidate signing subject"),
            finalized
                .release_attestation_subject()
                .expect("finalized signing subject")
        );
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

    fn assert_v4_schema_excludes_v3_binding<T: iroha_schema::IntoSchema>() {
        let schema = T::schema();
        assert!(
            !schema.contains_key::<KagemushaRecursiveSpendArtifactBindingV3>(),
            "{} unexpectedly reaches the historical V3 artifact binding",
            core::any::type_name::<T>()
        );
    }

    #[test]
    fn all_v4_lifecycle_schemas_exclude_v3_artifact_bindings() {
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendNativeCapabilitiesV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaPastaCycleProofEnvelopeV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendPublicStatementV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendProofV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendBundleV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendTopUpAnchorV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendSplitIntentV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendRedemptionIntentV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendInitRequestV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendInitResultV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendAppendInputV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendAppendRequestV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendSplitResultV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendTopUpFinalityEvidenceV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendVerifyRequestV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendBundleSummaryV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendVerifyResultV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendRedeemChangeBranchV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendRedeemBuildRequestV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendRedeemUnsignedV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendRedeemBuildResultV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendRedeemRequestV4>();
        assert_v4_schema_excludes_v3_binding::<KagemushaRecursiveSpendRedeemResultV4>();

        for schema in [
            KagemushaRecursiveSpendBundleV4::schema(),
            KagemushaRecursiveSpendInitRequestV4::schema(),
            KagemushaRecursiveSpendAppendRequestV4::schema(),
            KagemushaRecursiveSpendVerifyRequestV4::schema(),
            KagemushaRecursiveSpendRedeemRequestV4::schema(),
        ] {
            assert!(schema.contains_key::<KagemushaRecursiveSpendArtifactBindingV4>());
        }
    }
}

#[cfg(test)]
mod kagemusha_release_auth_tests {
    use crate::domain::DomainId;

    use super::*;

    type ReleaseFixture = (
        KagemushaRecursiveSpendArtifactManifestV3,
        KagemushaRecursiveSpendReleasePolicyV1,
        KagemushaRecursiveSpendReleaseAttestationV1,
        Vec<u8>,
        Vec<u8>,
    );

    fn digest(label: &[u8]) -> [u8; 32] {
        kagemusha_recursive_spend_release_sha256(label)
    }

    fn artifact(
        kind: KagemushaPastaCycleArtifactKindV3,
        file_name: &str,
        seed: u8,
    ) -> KagemushaPastaCycleArtifactV3 {
        KagemushaPastaCycleArtifactV3 {
            kind,
            file_name: file_name.to_owned(),
            size_bytes: 96,
            sha256: digest(&[b'f', seed]),
            payload_size_bytes: 32,
            payload_sha256: digest(&[b'p', seed]),
        }
    }

    fn profile(
        parity: KagemushaPastaCycleParityV1,
        circuit_id: &str,
        file_names: [&str; 3],
        seed: u8,
    ) -> KagemushaPastaCycleProofProfileV1 {
        KagemushaPastaCycleProofProfileV1 {
            parity,
            circuit_id: circuit_id.to_owned(),
            parameter_generation: "release-auth-test-params".to_owned(),
            ipa_k: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
            artifacts: vec![
                artifact(
                    KagemushaPastaCycleArtifactKindV3::Parameters,
                    file_names[0],
                    seed,
                ),
                artifact(
                    KagemushaPastaCycleArtifactKindV3::ProvingKey,
                    file_names[1],
                    seed + 1,
                ),
                artifact(
                    KagemushaPastaCycleArtifactKindV3::VerifyingKey,
                    file_names[2],
                    seed + 2,
                ),
            ],
        }
    }

    fn fixture() -> ReleaseFixture {
        let benchmark = b"signed physical-device benchmark evidence".to_vec();
        let review = b"independent cryptographic review evidence".to_vec();
        let generation = "release-auth-test-generation";
        let mut manifest = KagemushaRecursiveSpendArtifactManifestV3 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1.to_owned(),
            generation: generation.to_owned(),
            source_commit: "1234567890abcdef1234567890abcdef12345678".to_owned(),
            source_tree_sha256: [0x5A; 32],
            source_repo_dirty: true,
            chain_id: ChainId::from("release-auth-test-chain"),
            asset: AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").expect("test domain"),
                "rose".parse().expect("test asset name"),
            ),
            asset_scale: 9,
            activation_height: 1,
            withdrawal_height: 100,
            max_proof_bytes: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3,
            profiles: vec![
                profile(
                    KagemushaPastaCycleParityV1::StepEq,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3,
                    [
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V3,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V3,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V3,
                    ],
                    1,
                ),
                profile(
                    KagemushaPastaCycleParityV1::StepEp,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3,
                    [
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V3,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V3,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V3,
                    ],
                    4,
                ),
            ],
            topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV2 {
                file_name: KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2.to_owned(),
                size_bytes: 128,
                sha256: digest(b"release-auth-test-roster"),
                artifact_generation: generation.to_owned(),
                circuit_id: KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2.to_owned(),
                purpose: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2.to_owned(),
                artifact_type: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2.to_owned(),
                required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            },
            benchmark_evidence_sha256: digest(&benchmark),
            cryptographic_review_sha256: digest(&review),
            release_attestation_sha256: digest(b"release-auth-test-attestation-placeholder"),
        };
        let key_pairs = [
            KeyPair::from_seed(vec![11; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![12; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![13; 32], Algorithm::Ed25519),
        ];
        let roles = [
            KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
        ];
        let policy = KagemushaRecursiveSpendReleasePolicyV1 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            policy_id: "release-auth-test-policy".to_owned(),
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
            .expect("test manifest subject");
        let attestation = KagemushaRecursiveSpendReleaseAttestationV1 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            subject: subject.clone(),
            approvals: roles
                .iter()
                .zip(&key_pairs)
                .map(
                    |(&role, key_pair)| KagemushaRecursiveSpendReleaseApprovalV1 {
                        role,
                        public_key: key_pair.public_key().clone(),
                        signature: SignatureOf::try_new(
                            key_pair.private_key(),
                            &subject.approval_payload(role),
                        )
                        .expect("test release signature"),
                    },
                )
                .collect(),
        };
        manifest.release_attestation_sha256 =
            digest(&to_bytes(&attestation).expect("encode test release attestation"));
        manifest.validate().expect("valid release-auth fixture");
        policy.validate().expect("valid release-auth policy");
        (manifest, policy, attestation, benchmark, review)
    }

    fn rebind_attestation_digest(
        manifest: &mut KagemushaRecursiveSpendArtifactManifestV3,
        attestation: &KagemushaRecursiveSpendReleaseAttestationV1,
    ) {
        manifest.release_attestation_sha256 =
            digest(&to_bytes(attestation).expect("encode modified test attestation"));
    }

    #[test]
    fn release_attestation_authenticates_all_independent_roles_and_evidence() {
        let (manifest, policy, attestation, benchmark, review) = fixture();
        let authenticated = KagemushaAuthenticatedReleaseV3::verify(
            &manifest,
            &policy,
            &attestation,
            &benchmark,
            &review,
        )
        .expect("fully authenticated release");
        assert_eq!(authenticated.approved_signers().len(), 3);
        assert_eq!(authenticated.manifest(), &manifest);
    }

    #[test]
    fn release_attestation_rejects_mismatched_evidence_bytes() {
        let (manifest, policy, attestation, benchmark, _) = fixture();
        assert_eq!(
            KagemushaAuthenticatedReleaseV3::verify(
                &manifest,
                &policy,
                &attestation,
                &benchmark,
                b"substituted cryptographic review",
            ),
            Err(KagemushaReleaseVerificationError::EvidenceMismatch {
                role: KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            })
        );
        assert_eq!(
            KagemushaAuthenticatedReleaseV3::verify(
                &manifest,
                &policy,
                &attestation,
                b"",
                b"release-auth-test-review",
            ),
            Err(KagemushaReleaseVerificationError::EvidenceMismatch {
                role: KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
            })
        );
    }

    #[test]
    fn unsigned_release_attestation_is_rejected() {
        let (mut manifest, policy, mut attestation, _, _) = fixture();
        attestation.approvals.clear();
        rebind_attestation_digest(&mut manifest, &attestation);
        assert_eq!(
            KagemushaAuthenticatedReleaseV3::verify_attestation(&manifest, &policy, &attestation,),
            Err(KagemushaReleaseVerificationError::InvalidAttestation)
        );
    }

    #[test]
    fn manifest_tampering_after_approval_is_rejected() {
        let (mut manifest, policy, attestation, _, _) = fixture();
        manifest.source_commit = "abcdefabcdefabcdefabcdefabcdefabcdefabcd".to_owned();
        assert_eq!(
            KagemushaAuthenticatedReleaseV3::verify_attestation(&manifest, &policy, &attestation,),
            Err(KagemushaReleaseVerificationError::InvalidAttestation)
        );
    }

    #[test]
    fn source_tree_tampering_after_approval_is_rejected() {
        let (mut manifest, policy, attestation, _, _) = fixture();
        manifest.source_tree_sha256[0] ^= 1;
        assert_eq!(
            KagemushaAuthenticatedReleaseV3::verify_attestation(&manifest, &policy, &attestation,),
            Err(KagemushaReleaseVerificationError::InvalidAttestation)
        );

        let (mut manifest, policy, attestation, _, _) = fixture();
        manifest.source_repo_dirty = false;
        assert_eq!(
            KagemushaAuthenticatedReleaseV3::verify_attestation(&manifest, &policy, &attestation,),
            Err(KagemushaReleaseVerificationError::InvalidAttestation)
        );
    }

    #[test]
    fn approval_signatures_cannot_be_substituted_across_roles() {
        let (mut manifest, policy, mut attestation, _, _) = fixture();
        attestation.approvals[0].role =
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview;
        attestation.approvals[1].role = KagemushaRecursiveSpendReleaseApprovalRoleV1::Release;
        attestation.approvals.sort_by(|left, right| {
            left.role
                .cmp(&right.role)
                .then_with(|| left.public_key.cmp(&right.public_key))
        });
        rebind_attestation_digest(&mut manifest, &attestation);
        assert!(matches!(
            KagemushaAuthenticatedReleaseV3::verify_attestation(&manifest, &policy, &attestation,),
            Err(KagemushaReleaseVerificationError::UnknownSigner { .. })
        ));
    }
}

#[cfg(test)]
mod device_authority_p256_tests {
    use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

    use super::*;

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
}

impl KagemushaRecursiveSpendPeerPaymentV2 {
    /// Project the recipient-only transport envelope from a local split result.
    pub fn from_split_result(
        result: &KagemushaRecursiveSpendSplitResultV2,
    ) -> Result<Self, KagemushaValidationError> {
        result.validate_public_binding()?;
        let payment = Self {
            recipient_bundle: result.recipient_bundle.clone(),
            recipient_membership_witness: result.recipient_membership_witness.clone(),
        };
        payment.validate_public_binding()?;
        Ok(payment)
    }

    /// Return the canonical recipient peer-split transition embedded in this payment.
    pub fn recipient_split_transition(
        &self,
    ) -> Result<&KagemushaRecursiveSpendPeerSplitTransitionV2, KagemushaValidationError> {
        self.recipient_bundle.validate_public_binding()?;
        let Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(transition)) =
            self.recipient_bundle.statement.transition.as_ref()
        else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "peer_payment.transition",
            });
        };
        if transition.branch != KagemushaRecursiveSpendBranchV2::Recipient {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "peer_payment.binding",
            });
        }
        Ok(transition)
    }

    /// Return the canonical split operation identifier from the embedded transition.
    pub fn operation_id(&self) -> Result<[u8; 32], KagemushaValidationError> {
        Ok(self.recipient_split_transition()?.operation_id)
    }

    /// Return the canonical recipient-request digest from the embedded transition.
    pub fn recipient_request_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        Ok(self.recipient_split_transition()?.recipient_request_digest)
    }

    /// Validate the recipient branch, derived replay identity, and peer-size contract.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        let transition = self.recipient_split_transition()?;
        self.recipient_membership_witness
            .validate_for_root(self.recipient_bundle.statement.final_root)?;
        if transition.operation_id == [0; 32] || transition.recipient_request_digest == [0; 32] {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "peer_payment.binding",
            });
        }
        let encoded_len = to_bytes(self)?.len();
        if encoded_len > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2 {
            return Err(KagemushaValidationError::EncodedSizeExceeded {
                max: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2,
                actual: encoded_len,
            });
        }
        Ok(())
    }
}

/// Derive the canonical public reference carried by a receiver payment request.
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
    /// Verify the ACK against the exact request and recipient bundle.
    ///
    /// Callers must additionally check the device key against the registered
    /// receiver device lineage. A sender may commit reserved inputs only after
    /// this function and that registry check both succeed.
    pub fn validate_for_payment(
        &self,
        recipient_request: &KagemushaRecipientPaymentRequestV2,
        recipient_bundle: &KagemushaRecursiveSpendBundleV2,
    ) -> Result<(), KagemushaValidationError> {
        self.payload.validate_public_binding()?;
        recipient_request.validate_public_binding()?;
        recipient_bundle.validate_public_binding()?;
        let Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(transition)) =
            recipient_bundle.statement.transition.as_ref()
        else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "receiver_acknowledgement.split",
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
            || self.payload.accepted_at_ms > recipient_request.expires_at_ms
            || self.payload.accepted_at_ms < recipient_request.issued_at_ms
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "receiver_acknowledgement.binding",
            });
        }
        self.signature
            .verify(
                &self.payload.receiver_public_key,
                &self.payload.signing_bytes()?,
            )
            .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "receiver_acknowledgement.signature",
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

    /// Return the canonical ACK archive to persist and replay byte-for-byte.
    pub fn canonical_archive_for_payment(
        &self,
        recipient_request: &KagemushaRecipientPaymentRequestV2,
        recipient_bundle: &KagemushaRecursiveSpendBundleV2,
    ) -> Result<Vec<u8>, KagemushaValidationError> {
        self.validate_for_payment(recipient_request, recipient_bundle)?;
        Ok(to_bytes(self)?)
    }

    /// Return the canonical identity digest of the signed acknowledgement.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        kagemusha_poseidon_preimage(&KagemushaReceiverAcknowledgementDigestPreimageV2 {
            domain: KAGEMUSHA_RECEIVER_ACKNOWLEDGEMENT_DIGEST_DOMAIN_V2.to_owned(),
            acknowledgement: self.clone(),
        })
    }

    /// Build the typed successful native verification result.
    pub fn verified_result(
        &self,
        recipient_request: &KagemushaRecipientPaymentRequestV2,
        recipient_bundle: &KagemushaRecursiveSpendBundleV2,
    ) -> Result<KagemushaReceiverAcknowledgementVerifyResultV2, KagemushaValidationError> {
        self.validate_for_payment(recipient_request, recipient_bundle)?;
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

impl KagemushaRecursiveSpendRedeemResultV2 {
    /// Validate the canonical request archive and optional change bundle shape.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        if self.operation_id == [0; 32]
            || self.redeem_request_archive.is_empty()
            || self.redeem_request_archive.len()
                > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_result",
            });
        }
        match (
            &self.offline_change_bundle,
            &self.offline_change_membership_witness,
        ) {
            (None, None) => {}
            (Some(change), Some(witness)) => {
                change.validate_public_binding()?;
                witness.validate_for_root(change.statement.final_root)?;
            }
            _ => {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "redeem_result.offline_change",
                });
            }
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendRedeemChangeBranchV2 {
    /// Validate the dedicated change child against the exact consumed input and intent.
    pub fn validate_for_redemption(
        &self,
        input_bundle: &KagemushaRecursiveSpendBundleV2,
        redemption: &KagemushaRecursiveSpendRedemptionIntentV2,
    ) -> Result<(), KagemushaValidationError> {
        input_bundle.validate_public_binding()?;
        redemption.validate_public_binding()?;
        self.output.validate_public_binding()?;
        self.bundle.validate_public_binding()?;
        let expected_output = redemption.change_output.as_ref().ok_or(
            KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "offline_change",
            },
        )?;
        let binding_digest = redemption.binding_digest()?;
        let mut expected_claims = redemption
            .parent_branch_claims
            .iter()
            .map(|claim| claim.child(KagemushaRecursiveSpendBranchV2::Change, binding_digest))
            .collect::<Result<Vec<_>, _>>()?;
        expected_claims.sort_unstable_by_key(|claim| claim.path);
        validate_kagemusha_recursive_spend_branch_claims_v2(&expected_claims)?;
        if &self.output != expected_output
            || self.branch_claims != expected_claims
            || self.bundle.statement.current_note != self.output
            || self.bundle.statement.branch_claims != self.branch_claims
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "offline_change.branch",
            });
        }
        let Some(KagemushaRecursiveSpendTransitionV2::RedemptionChange(transition)) =
            self.bundle.statement.transition.as_ref()
        else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_change.transition",
            });
        };
        if transition.binding_digest != redemption.binding_digest()?
            || transition.parent_bundle_digest != redemption.parent_bundle_digest
            || transition.operation_id != redemption.operation_id
            || transition.parent_proof_step_count != redemption.parent_proof_step_count
            || transition.parent_peer_hop_count != redemption.parent_peer_hop_count
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_change.transition",
            });
        }
        let input = &input_bundle.statement;
        let change = &self.bundle.statement;
        if redemption.parent_bundle_digest != input_bundle.digest()?
            || redemption.input_note != input.current_note
            || redemption.parent_branch_claims != input.branch_claims
            || redemption.input_root != input.final_root
            || change.chain_id != input.chain_id
            || change.asset != input.asset
            || change.asset_scale != input.asset_scale
            || change.final_root == input.final_root
            || change.topup_anchor_refs != input.topup_anchor_refs
            || change.proof_step_count != input.proof_step_count.saturating_add(1)
            || change.peer_hop_count != input.peer_hop_count
            || redemption.change_artifact_binding.as_ref() != Some(&change.artifact_binding)
            || change.verifier_key_id.name != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_change.parent_binding",
            });
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendInitRequestV2 {
    /// Construct the canonical Kagemusha init request.
    pub fn new(
        topup_anchor: KagemushaRecursiveSpendTopUpAnchorV2,
        topup_finality_proof: KagemushaTopUpFinalityProofV2,
        topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactV2,
        artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
    ) -> Result<Self, KagemushaValidationError> {
        let request = Self {
            topup_anchor,
            topup_finality_proof,
            topup_finality_roster_artifact,
            artifact_binding,
        };
        request.validate_public_binding()?;
        Ok(request)
    }

    /// Validate finalized provenance and the authenticated artifact identity.
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
                field: "init_request",
            });
        }
        Ok(())
    }

    /// Fail closed until the authenticated two-layer Pasta backend is linked.
    pub fn ensure_proof_backend_available(&self) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        Err(KagemushaValidationError::RecursiveSpendV2ProofBackendUnavailable)
    }
}

impl KagemushaRecursiveSpendInitResultV2 {
    /// Validate the initialized bundle and circuit-exposed statement digest.
    pub fn validate_for_request(
        &self,
        request: &KagemushaRecursiveSpendInitRequestV2,
    ) -> Result<(), KagemushaValidationError> {
        request.validate_public_binding()?;
        self.bundle.validate_public_binding()?;
        if self.public_statement_digest == [0; 32]
            || self.public_statement_digest != self.bundle.statement.digest()?
            || self.public_statement_digest != self.bundle.recursive_proof.public_statement_digest
            || self.bundle.statement.chain_id != request.topup_anchor.chain_id
            || self.bundle.statement.asset != request.topup_anchor.asset.definition().clone()
            || self.bundle.statement.asset_scale != request.topup_anchor.asset_scale
            || self.bundle.statement.current_note != request.topup_anchor.current_note
            || self.bundle.statement.artifact_binding != request.artifact_binding
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "init_result",
            });
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendSplitIntentV2 {
    /// Construct an exact, branch-safe split statement.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        chain_id: ChainId,
        asset: AssetDefinitionId,
        inputs: Vec<KagemushaRecursiveSpendInputBranchV2>,
        topup_anchor_refs: Vec<KagemushaRecursiveSpendTopUpAnchorRefV2>,
        asset_scale: u32,
        output_artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
        transfer_amount: KagemushaScaledAmountV2,
        recipient_output: KagemushaSpendableNoteDescriptorV2,
        change_output: Option<KagemushaSpendableNoteDescriptorV2>,
        recipient_request_digest: [u8; 32],
        operation_id: [u8; 32],
    ) -> Result<Self, KagemushaValidationError> {
        let split = Self {
            chain_id,
            asset,
            inputs,
            topup_anchor_refs,
            asset_scale,
            output_artifact_binding,
            transfer_amount,
            recipient_output,
            change_output,
            recipient_request_digest,
            operation_id,
        };
        split.validate_public_binding()?;
        Ok(split)
    }

    /// Validate exact conservation, canonical parents, and disjoint outputs.
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
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof { field: "split" });
        }
        let mut input_total = 0u128;
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
                    field: "split.inputs",
                });
            }
            previous_digest = Some(input.bundle_digest);
            input_total = input_total
                .checked_add(input.input_note.amount.atomic_units)
                .ok_or(KagemushaValidationError::InvalidRecursiveSpendNote {
                    field: "split.input_amount",
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
                field: "split.topup_anchor_refs",
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
                field: "split.change_output",
            })?;
        if output_total != input_total {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split.conservation",
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
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split.output_material",
            });
        }
        let unique = output_material
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if unique.len() != output_material.len() {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split.output_material",
            });
        }
        Ok(())
    }

    /// Return the exact input total after validation.
    pub fn input_amount(&self) -> Result<KagemushaScaledAmountV2, KagemushaValidationError> {
        self.validate_public_binding()?;
        let atomic_units = self
            .inputs
            .iter()
            .try_fold(0u128, |sum, input| {
                sum.checked_add(input.input_note.amount.atomic_units)
            })
            .ok_or(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split.input_amount",
            })?;
        KagemushaScaledAmountV2::new(atomic_units, self.asset_scale)
    }

    /// Return the canonical transition binding digest.
    pub fn binding_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRecursiveSpendSplitBindingDigestPreimageV2 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_SPLIT_BINDING_DIGEST_DOMAIN_V2.to_owned(),
            split: self.clone(),
        })
    }

    /// Derive conflict claims for one independently spendable child.
    pub fn output_branch_claims(
        &self,
        branch: KagemushaRecursiveSpendBranchV2,
    ) -> Result<Vec<KagemushaRecursiveSpendBranchClaimV2>, KagemushaValidationError> {
        self.validate_public_binding()?;
        if matches!(branch, KagemushaRecursiveSpendBranchV2::Change) && self.change_output.is_none()
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "split.change_output",
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

    /// Validate a proof-bearing child against this exact split.
    pub fn validate_output_bundle(
        &self,
        bundle: &KagemushaRecursiveSpendBundleV2,
        branch: KagemushaRecursiveSpendBranchV2,
    ) -> Result<(), KagemushaValidationError> {
        bundle.validate_public_binding()?;
        let transition = KagemushaRecursiveSpendPeerSplitTransitionV2::from_intent(self, branch)?;
        let expected_note = match branch {
            KagemushaRecursiveSpendBranchV2::Recipient => &self.recipient_output,
            KagemushaRecursiveSpendBranchV2::Change => self.change_output.as_ref().ok_or(
                KagemushaValidationError::InvalidRecursiveSpendNote {
                    field: "split.change_output",
                },
            )?,
        };
        if bundle.statement.chain_id != self.chain_id
            || bundle.statement.asset != self.asset
            || bundle.statement.asset_scale != self.asset_scale
            || bundle.statement.current_note != *expected_note
            || bundle.statement.branch_claims != self.output_branch_claims(branch)?
            || bundle.statement.topup_anchor_refs != self.topup_anchor_refs
            || bundle.statement.artifact_binding != self.output_artifact_binding
            || bundle.statement.transition.as_ref()
                != Some(&KagemushaRecursiveSpendTransitionV2::PeerSplit(transition))
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "split.output_bundle",
            });
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendSplitIntentBuildRequestV2 {
    /// Derive every provenance field from validated parent bundles.
    pub fn into_intent(
        mut self,
    ) -> Result<KagemushaRecursiveSpendSplitIntentV2, KagemushaValidationError> {
        if self.previous_bundles.is_empty()
            || self.previous_bundles.len() > KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "split_builder.previous_bundles",
            });
        }
        self.output_artifact_binding.validate()?;
        let mut keyed = self
            .previous_bundles
            .drain(..)
            .map(|bundle| {
                bundle.validate_public_binding()?;
                Ok((bundle.digest()?, bundle))
            })
            .collect::<Result<Vec<_>, KagemushaValidationError>>()?;
        keyed.sort_unstable_by_key(|(digest, _)| *digest);
        if keyed.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "split_builder.previous_bundles",
            });
        }
        let first = &keyed[0].1.statement;
        let chain_id = first.chain_id.clone();
        let asset = first.asset.clone();
        let asset_scale = first.asset_scale;
        let mut anchor_refs = std::collections::BTreeSet::new();
        let mut inputs = Vec::with_capacity(keyed.len());
        for (digest, bundle) in keyed {
            let statement = bundle.statement;
            if statement.chain_id != chain_id
                || statement.asset != asset
                || statement.asset_scale != asset_scale
            {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "split_builder.context",
                });
            }
            anchor_refs.extend(statement.topup_anchor_refs.iter().copied());
            inputs.push(KagemushaRecursiveSpendInputBranchV2 {
                bundle_digest: digest,
                input_note: statement.current_note,
                branch_claims: statement.branch_claims,
                input_root: statement.final_root,
                proof_step_count: statement.proof_step_count,
                peer_hop_count: statement.peer_hop_count,
            });
        }
        KagemushaRecursiveSpendSplitIntentV2::new(
            chain_id,
            asset,
            inputs,
            anchor_refs.into_iter().collect(),
            asset_scale,
            self.output_artifact_binding,
            self.transfer_amount,
            self.recipient_output,
            self.change_output,
            self.recipient_request_digest,
            self.operation_id,
        )
    }
}

impl KagemushaRecursiveSpendAppendRequestV2 {
    /// Validate canonical parent ordering, transfer-proof shape, and split binding.
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
                field: "append_request",
            });
        }
        for (typed, expected) in self.previous_inputs.iter().zip(&self.split.inputs) {
            typed.previous_bundle.validate_public_binding()?;
            if typed.previous_bundle.digest()? != expected.bundle_digest
                || typed.previous_bundle.statement.current_note != expected.input_note
                || typed.previous_bundle.statement.branch_claims != expected.branch_claims
                || typed.previous_bundle.statement.final_root != expected.input_root
                || typed.previous_bundle.statement.proof_step_count != expected.proof_step_count
                || typed.previous_bundle.statement.peer_hop_count != expected.peer_hop_count
            {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "append_request.previous_inputs",
                });
            }
        }
        Ok(())
    }

    /// Fail closed until the authenticated two-layer Pasta backend is linked.
    pub fn ensure_proof_backend_available(&self) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        Err(KagemushaValidationError::RecursiveSpendV2ProofBackendUnavailable)
    }
}

impl KagemushaRecursiveSpendRedeemBuildRequestV2 {
    /// Validate the common full/partial redemption builder input.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.bundle.validate_public_binding()?;
        self.redemption.validate_public_binding()?;
        self.public_amount.validate()?;
        validate_kagemusha_redeem_proof_attachment_v2(&self.unshield_proof)?;
        if self.block_height == 0
            || self.operation_id == [0; 32]
            || self.operation_id != self.redemption.operation_id
            || self.recipient != self.redemption.recipient
            || self.public_amount != self.redemption.public_amount
            || self.redemption.parent_bundle_digest != self.bundle.digest()?
            || self.redemption.input_note != self.bundle.statement.current_note
            || self.redemption.input_root != self.bundle.statement.final_root
            || self.redemption.parent_branch_claims != self.bundle.statement.branch_claims
            || self.redemption.parent_topup_anchor_refs != self.bundle.statement.topup_anchor_refs
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_request",
            });
        }
        match (
            &self.redemption.change_output,
            &self.redemption.change_artifact_binding,
        ) {
            (None, None) => {}
            (Some(_), Some(binding)) if binding == &self.bundle.statement.artifact_binding => {}
            _ => {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "redeem_build_request.change",
                });
            }
        }
        Ok(())
    }

    /// Fail closed until the authenticated two-layer Pasta backend is linked.
    pub fn ensure_proof_backend_available(&self) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        Err(KagemushaValidationError::RecursiveSpendV2ProofBackendUnavailable)
    }
}

impl KagemushaRecursiveSpendPublicStatementV2 {
    /// Validate the canonical recursive state statement.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.current_note.validate_public_binding()?;
        self.artifact_binding.validate()?;
        validate_kagemusha_root("final_root", self.final_root)?;
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
            || self.proof_step_count == 0
            || self.proof_step_count > KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2
            || self.peer_hop_count > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
            || claim_roots != lineage_roots
            || self.verifier_key_id.name != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3
            || self.verifier_key_id.backend.as_str()
                != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "public_statement",
            });
        }
        match &self.transition {
            None if self.proof_step_count == 1 && self.peer_hop_count == 0 => {}
            Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(transition))
                if transition.binding_digest != [0; 32]
                    && transition.recipient_request_digest != [0; 32]
                    && transition.operation_id != [0; 32]
                    && self.proof_step_count
                        == transition.parent_max_proof_step_count.saturating_add(1)
                    && self.peer_hop_count
                        == transition.parent_max_peer_hop_count.saturating_add(1) => {}
            Some(KagemushaRecursiveSpendTransitionV2::RedemptionChange(transition))
                if transition.binding_digest != [0; 32]
                    && transition.parent_bundle_digest != [0; 32]
                    && transition.operation_id != [0; 32]
                    && self.proof_step_count
                        == transition.parent_proof_step_count.saturating_add(1)
                    && self.peer_hop_count == transition.parent_peer_hop_count => {}
            _ => {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "public_statement.transition",
                });
            }
        }
        Ok(())
    }

    /// Return the circuit-exposed statement digest.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRecursiveSpendPublicStatementDigestPreimageV2 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_PUBLIC_STATEMENT_DIGEST_DOMAIN_V2.to_owned(),
            statement: self.clone(),
        })
    }
}

impl KagemushaRecursiveSpendBundleV2 {
    /// Validate statement/proof identity and the constant-size peer envelope.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.statement.validate_public_binding()?;
        if self.recursive_proof.verifier_key_id != self.statement.verifier_key_id
            || self.recursive_proof.public_statement_digest != self.statement.digest()?
            || self.recursive_proof.proof.backend.as_str()
                != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
            || self.recursive_proof.proof.bytes.is_empty()
            || self.recursive_proof.proof.bytes.len()
                > usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3)
                    .unwrap_or(usize::MAX)
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "recursive_proof",
            });
        }
        let encoded_len = to_bytes(self)?.len();
        if encoded_len > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2 {
            return Err(KagemushaValidationError::EncodedSizeExceeded {
                actual: encoded_len,
                max: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2,
            });
        }
        Ok(())
    }

    /// Return a canonical identity digest for idempotency and parent binding.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRecursiveSpendBundleDigestPreimageV2 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_DIGEST_DOMAIN_V2.to_owned(),
            bundle: self.clone(),
        })
    }

    /// Decode only wallet-visible fields while keeping proof bytes opaque.
    pub fn summary(
        &self,
    ) -> Result<KagemushaRecursiveSpendBundleSummaryV2, KagemushaValidationError> {
        self.validate_public_binding()?;
        Ok(KagemushaRecursiveSpendBundleSummaryV2 {
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

    /// Fail closed until the authenticated two-layer Pasta backend is linked.
    pub fn ensure_proof_backend_available(&self) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        Err(KagemushaValidationError::RecursiveSpendV2ProofBackendUnavailable)
    }
}

impl KagemushaRecursiveSpendTopUpFinalityEvidenceV2 {
    /// Validate the bounded full anchor and the proof's exact compact binding.
    ///
    /// This is intentionally structural. The native receiver verifier must
    /// still authenticate the roster and execute the QC/Merkle cryptography.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.topup_anchor.validate_public_binding()?;
        self.topup_finality_proof.validate_structure()?;
        let anchor_ref = self.topup_anchor.compact_ref()?;
        let anchor_len = to_bytes(&self.topup_anchor)?.len();
        let proof_len = to_bytes(&self.topup_finality_proof)?.len();
        if anchor_len == 0
            || anchor_len > KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_V2 as usize
            || proof_len == 0
            || proof_len > KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_V2 as usize
            || self.topup_finality_proof.anchor != anchor_ref
            || self.topup_finality_proof.commit_qc.height_context.height
                != self.topup_anchor.finalized_height
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.topup_finality_evidence",
            });
        }
        Ok(())
    }

    /// Validate the exact canonical evidence inventory carried for a Step state.
    pub fn validate_ordered_set(
        evidence: &[Self],
        expected_refs: &[KagemushaRecursiveSpendTopUpAnchorRefV2],
    ) -> Result<(), KagemushaValidationError> {
        if evidence.is_empty()
            || evidence.len() != expected_refs.len()
            || evidence.len() > KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.topup_finality_evidence_count",
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for (evidence, expected_ref) in evidence.iter().zip(expected_refs) {
            evidence.validate_public_binding()?;
            let actual_ref = evidence.topup_anchor.compact_ref()?;
            if actual_ref != *expected_ref || !seen.insert(actual_ref) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "verify_request.topup_finality_evidence_order",
                });
            }
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendVerifyRequestV2 {
    /// Validate receiver request, artifact, hop, and exact recipient output.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.bundle.validate_public_binding()?;
        self.artifact_binding.validate()?;
        self.recipient_request.validate_at(self.verified_at_ms)?;
        self.topup_finality_roster_artifact.validate_structure()?;
        let roster_len = to_bytes(&self.topup_finality_roster_artifact)?.len();
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
            || roster_len == 0
            || roster_len > KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2 as usize
            || self.topup_finality_roster_artifact.chain_id != statement.chain_id
            || self.topup_finality_roster_artifact.artifact_generation
                != self.artifact_binding.generation
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request",
            });
        }
        KagemushaRecursiveSpendTopUpFinalityEvidenceV2::validate_ordered_set(
            &self.topup_finality_evidence,
            &statement.topup_anchor_refs,
        )?;

        let mut anchored_total = 0_u128;
        for evidence in &self.topup_finality_evidence {
            let anchor = &evidence.topup_anchor;
            if anchor.chain_id != statement.chain_id
                || anchor.asset.definition() != &statement.asset
                || anchor.asset_scale != statement.asset_scale
                || anchor.artifact_binding != self.artifact_binding
                || anchor.finalized_height > self.block_height
                || evidence
                    .topup_finality_proof
                    .commit_qc
                    .height_context
                    .chain_id
                    != statement.chain_id
                || self
                    .topup_finality_roster_artifact
                    .window_at(anchor.finalized_height)
                    .is_err()
            {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "verify_request.topup_finality_evidence_binding",
                });
            }
            anchored_total = anchored_total
                .checked_add(anchor.amount.atomic_units)
                .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "verify_request.topup_finality_amount",
                })?;
        }
        if anchored_total < statement.current_note.amount.atomic_units {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.topup_finality_amount",
            });
        }

        let Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(transition)) =
            statement.transition.as_ref()
        else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.transition",
            });
        };
        if transition.branch != KagemushaRecursiveSpendBranchV2::Recipient
            || transition.recipient_request_digest != self.recipient_request.digest()?
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.recipient_binding",
            });
        }
        let encoded_len = to_bytes(self)?.len();
        if encoded_len > KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_MAX_BYTES_V2 {
            return Err(KagemushaValidationError::EncodedSizeExceeded {
                actual: encoded_len,
                max: KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_MAX_BYTES_V2,
            });
        }
        Ok(())
    }

    /// Fail closed until the authenticated two-layer Pasta backend is linked.
    pub fn ensure_proof_backend_available(&self) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        self.bundle.ensure_proof_backend_available()
    }
}

impl KagemushaRecursiveSpendVerifyResultV2 {
    /// Enforce the single Kagemusha acceptance contract.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        if !self.valid
            || !self.chain_admissible
            || !self.lineage_redeemable
            || !self.witnessless_redemption_supported
            || self.recipient_request_digest == [0; 32]
            || self.request_output_binding_digest == [0; 32]
            || self.verifier_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3
            || self.verified_at_block_height == 0
            || self.verified_at_ms == 0
            || self.summary.verifier_key_id != self.verifier_key_id
            || self.verifier_key_id.name != self.verifier_circuit_id
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_result",
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

impl KagemushaRecursiveSpendRedeemUnsignedV2 {
    /// Validate exact full/partial redemption fields.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.bundle.validate_public_binding()?;
        self.redemption.validate_public_binding()?;
        self.amount.validate()?;
        validate_kagemusha_redeem_proof_attachment_v2(&self.redeem_proof)?;
        if self.block_height == 0
            || self.operation_id == [0; 32]
            || self.operation_id != self.redemption.operation_id
            || self.recipient != self.redemption.recipient
            || self.amount != self.redemption.public_amount
            || self.redemption.parent_bundle_digest != self.bundle.digest()?
            || self.redemption.input_note != self.bundle.statement.current_note
            || self.redemption.input_root != self.bundle.statement.final_root
            || self.redemption.parent_branch_claims != self.bundle.statement.branch_claims
            || self.redemption.parent_topup_anchor_refs != self.bundle.statement.topup_anchor_refs
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_request",
            });
        }
        match (&self.redemption.change_output, &self.offline_change) {
            (None, None) => {}
            (Some(_), Some(change)) => {
                change.validate_for_redemption(&self.bundle, &self.redemption)?;
            }
            _ => {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "redeem_request.offline_change",
                });
            }
        }
        Ok(())
    }

    /// Digest of all unsigned redemption fields covered by authorization.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRedeemUnsignedPayloadDigestPreimageV2 {
            domain: KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V2.to_owned(),
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

    /// Attach matching recipient authorization.
    pub fn into_request(
        self,
        authorization: KagemushaRequestAuthorizationV2,
    ) -> Result<KagemushaRecursiveSpendRedeemRequestV2, KagemushaValidationError> {
        let request = KagemushaRecursiveSpendRedeemRequestV2 {
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

impl KagemushaRecursiveSpendRedeemRequestV2 {
    /// Reconstruct the exact canonical fields covered by authorization.
    #[must_use]
    pub fn unsigned_payload(&self) -> KagemushaRecursiveSpendRedeemUnsignedV2 {
        KagemushaRecursiveSpendRedeemUnsignedV2 {
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

    /// Validate exact conservation and self-contained recipient authorization.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        let unsigned = self.unsigned_payload();
        unsigned.validate_public_binding()?;
        if self.authorization.operation_id != self.operation_id
            || self.authorization.authority != self.recipient
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "authorization",
            });
        }
        self.authorization.validate_for_payload(unsigned.digest()?)
    }

    /// Digest of unsigned redemption fields.
    pub fn unsigned_payload_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.unsigned_payload().digest()
    }

    /// Verify authorization at authoritative Torii time.
    pub fn validate_authorization_at(&self, now_ms: u64) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        self.authorization
            .validate_for_payload_at(self.unsigned_payload_digest()?, now_ms)
    }

    /// Fail closed until the authenticated two-layer Pasta backend is linked.
    pub fn ensure_proof_backend_available(&self) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        Err(KagemushaValidationError::RecursiveSpendV2ProofBackendUnavailable)
    }
}

/// Domain separator for ABI-20 finalized top-up anchor receipts.
pub const KAGEMUSHA_TOPUP_ANCHOR_DIGEST_DOMAIN_V4: &str = "iroha:kagemusha:v4:topup-anchor";
/// Domain separator for an ABI-20 split transition binding.
pub const KAGEMUSHA_RECURSIVE_SPEND_SPLIT_BINDING_DIGEST_DOMAIN_V4: &str =
    "iroha:kagemusha:v4:split-binding";
/// Domain separator for an ABI-20 redemption transition binding.
pub const KAGEMUSHA_REDEMPTION_TRANSITION_DIGEST_DOMAIN_V4: &str =
    "iroha:kagemusha:v4:redemption-transition";
/// Domain separator for an ABI-20 recursive public statement.
pub const KAGEMUSHA_RECURSIVE_SPEND_PUBLIC_STATEMENT_DIGEST_DOMAIN_V4: &str =
    "iroha:kagemusha:v4:public-statement";
/// Exact finalized-anchor schema carried by an ABI-20 init request.
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
    /// Populate and validate the canonical ABI-20 receipt digest.
    pub fn finalize_digest(mut self) -> Result<Self, KagemushaValidationError> {
        self.anchor_digest = self.compute_anchor_digest()?;
        self.validate_public_binding()?;
        Ok(self)
    }

    /// Compute the V4-domain digest of every immutable receipt field.
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

    /// Return the stable compact identity retained by ABI-20 descendants.
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

impl KagemushaRecursiveSpendInitRequestV4 {
    /// Validate finalized provenance and its exact authenticated ABI-20 release.
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

    /// Return the V4-domain transition binding consumed by ABI-20 Step.
    pub fn binding_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRecursiveSpendSplitBindingDigestPreimageV4 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_SPLIT_BINDING_DIGEST_DOMAIN_V4.to_owned(),
            split: self.clone(),
        })
    }

    /// Derive the deterministic conflict claims for one ABI-20 child.
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
    pub fn binding_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRedemptionTransitionDigestPreimageV4 {
            domain: KAGEMUSHA_REDEMPTION_TRANSITION_DIGEST_DOMAIN_V4.to_owned(),
            intent: self.clone(),
        })
    }

    /// Derive the exact continuing change claims for a partial redemption.
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
    /// Validate the canonical ABI-20 recursive-state statement.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.current_note.validate_public_binding()?;
        self.artifact_binding.validate()?;
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
            || self.proof_step_count == 0
            || self.proof_step_count > KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2
            || self.peer_hop_count > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
            || claim_roots != lineage_roots
            || self.verifier_key_id.name != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4
            || self.verifier_key_id.backend.as_str()
                != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "public_statement.v4",
            });
        }
        match &self.transition {
            None if self.proof_step_count == 1 && self.peer_hop_count == 0 => {}
            Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(transition))
                if transition.binding_digest != [0; 32]
                    && transition.recipient_request_digest != [0; 32]
                    && transition.operation_id != [0; 32]
                    && transition.parent_max_proof_step_count > 0
                    && transition.parent_max_proof_step_count.checked_add(1)
                        == Some(self.proof_step_count)
                    && transition.parent_max_peer_hop_count.checked_add(1)
                        == Some(self.peer_hop_count) => {}
            Some(KagemushaRecursiveSpendTransitionV2::RedemptionChange(transition))
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

    /// Return the V4-domain digest exposed by the ABI-20 Step instance.
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
    fn abi20_lifecycle_digest_domains_are_distinct_from_v2() {
        let v4 = [
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
}
/// Domain separator for ABI-20 recursive bundle identity digests.
pub const KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_DIGEST_DOMAIN_V4: &str =
    "iroha:kagemusha:v4:recursive-spend-bundle";
/// Domain separator for ABI-20 unsigned redemption authorization payloads.
pub const KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V4: &str = "iroha:kagemusha:v4:redeem-payload";
/// Domain separator binding an accepted receiver request to its exact output bundle.
pub const KAGEMUSHA_REQUEST_OUTPUT_BINDING_DIGEST_DOMAIN_V4: &str =
    "iroha:kagemusha:v4:request-output-binding";
/// Maximum canonical archive size of one ABI-20 proof-bearing bundle.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4: usize = 20 * 1024 * 1024;
/// Maximum canonical ABI-20 receiver-verification request size.
pub const KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_MAX_BYTES_V4: usize = 64 * 1024 * 1024;
/// Maximum canonical ABI-20 redemption request archive size.
pub const KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4: usize = 48 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRecursiveSpendBundleDigestPreimageV4 {
    domain: String,
    bundle: KagemushaRecursiveSpendBundleV4,
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaRedeemUnsignedPayloadDigestPreimageV4 {
    domain: String,
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

impl KagemushaRecursiveSpendBundleV4 {
    /// Validate the exact V4 statement/proof/release identity while keeping proof bytes opaque.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.statement.validate_public_binding()?;
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
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRecursiveSpendBundleDigestPreimageV4 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_DIGEST_DOMAIN_V4.to_owned(),
            bundle: self.clone(),
        })
    }

    /// Decode only wallet-visible V4 state while preserving the opaque proof payload.
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
    pub fn validate_for_request(
        &self,
        request: &KagemushaRecursiveSpendInitRequestV4,
    ) -> Result<(), KagemushaValidationError> {
        request.validate_public_binding()?;
        self.bundle.validate_public_binding()?;
        let anchor = &request.topup_anchor;
        let expected_claim = KagemushaRecursiveSpendBranchClaimV2::root(anchor.anchor_digest)?;
        if self.public_statement_digest == [0; 32]
            || self.public_statement_digest != self.bundle.statement.digest()?
            || self.public_statement_digest != self.bundle.recursive_proof.public_statement_digest
            || self.bundle.statement.chain_id != anchor.chain_id
            || self.bundle.statement.asset != anchor.asset.definition().clone()
            || self.bundle.statement.asset_scale != anchor.asset_scale
            || self.bundle.statement.final_root != anchor.finalized_root
            || self.bundle.statement.topup_anchor_refs != vec![anchor.compact_ref()?]
            || self.bundle.statement.proof_step_count != 1
            || self.bundle.statement.peer_hop_count != 0
            || self.bundle.statement.current_note != anchor.current_note
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
                || statement.artifact_binding != self.split.output_artifact_binding
            {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "append_request.v4.previous_inputs",
                });
            }
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendSplitIntentV4 {
    /// Validate one independently spendable V4 child against this exact split.
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
        let expected_transition = KagemushaRecursiveSpendTransitionV2::PeerSplit(
            KagemushaRecursiveSpendPeerSplitTransitionV2 {
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
            .validate_for_root(self.recipient_bundle.statement.final_root)?;
        match (
            &self.split.change_output,
            &self.change_bundle,
            &self.change_membership_witness,
        ) {
            (None, None, None) => Ok(()),
            (Some(_), Some(change_bundle), Some(change_witness)) => {
                self.split.validate_output_bundle(
                    change_bundle,
                    KagemushaRecursiveSpendBranchV2::Change,
                )?;
                change_witness.validate_for_root(change_bundle.statement.final_root)?;
                let recipient = &self.recipient_bundle.statement;
                let change = &change_bundle.statement;
                if recipient.final_root != change.final_root
                    || recipient.topup_anchor_refs != change.topup_anchor_refs
                    || recipient.proof_step_count != change.proof_step_count
                    || recipient.peer_hop_count != change.peer_hop_count
                    || recipient.artifact_binding != change.artifact_binding
                    || recipient.verifier_key_id != change.verifier_key_id
                    || self.recipient_membership_witness.leaf_index == change_witness.leaf_index
                    || self.recipient_membership_witness.dummy_input_path
                        != change_witness.dummy_input_path
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

impl KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
    /// Validate one bounded finalized V4 origin and its exact compact reference.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.topup_anchor.validate_public_binding()?;
        self.topup_finality_proof.validate_structure()?;
        let anchor_ref = self.topup_anchor.compact_ref()?;
        let anchor_len = to_bytes(&self.topup_anchor)?.len();
        let proof_len = to_bytes(&self.topup_finality_proof)?.len();
        if anchor_len == 0
            || anchor_len > KAGEMUSHA_TOPUP_FINALITY_ANCHOR_MAX_BYTES_V2 as usize
            || proof_len == 0
            || proof_len > KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_V2 as usize
            || self.topup_finality_proof.anchor != anchor_ref
            || self.topup_finality_proof.commit_qc.height_context.height
                != self.topup_anchor.finalized_height
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.v4.topup_finality_evidence",
            });
        }
        Ok(())
    }

    /// Validate the exact canonical origin inventory carried by a V4 state.
    pub fn validate_ordered_set(
        evidence: &[Self],
        expected_refs: &[KagemushaRecursiveSpendTopUpAnchorRefV2],
    ) -> Result<(), KagemushaValidationError> {
        if evidence.is_empty()
            || evidence.len() != expected_refs.len()
            || evidence.len() > KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.v4.topup_finality_evidence_count",
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for (evidence, expected_ref) in evidence.iter().zip(expected_refs) {
            evidence.validate_public_binding()?;
            let actual_ref = evidence.topup_anchor.compact_ref()?;
            if actual_ref != *expected_ref || !seen.insert(actual_ref) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "verify_request.v4.topup_finality_evidence_order",
                });
            }
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendVerifyRequestV4 {
    /// Validate the terminal receiver request and every V4 proof/provenance binding.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.bundle.validate_public_binding()?;
        self.artifact_binding.validate()?;
        self.recipient_request.validate_at(self.verified_at_ms)?;
        self.topup_finality_roster_artifact.validate_structure()?;
        let roster_len = to_bytes(&self.topup_finality_roster_artifact)?.len();
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
            || roster_len == 0
            || roster_len > KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2 as usize
            || self.topup_finality_roster_artifact.chain_id != statement.chain_id
            || self.topup_finality_roster_artifact.artifact_generation
                != self.artifact_binding.generation
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.v4",
            });
        }
        KagemushaRecursiveSpendTopUpFinalityEvidenceV4::validate_ordered_set(
            &self.topup_finality_evidence,
            &statement.topup_anchor_refs,
        )?;
        let mut anchored_total = 0_u128;
        for evidence in &self.topup_finality_evidence {
            let anchor = &evidence.topup_anchor;
            if anchor.chain_id != statement.chain_id
                || anchor.asset.definition() != &statement.asset
                || anchor.asset_scale != statement.asset_scale
                || anchor.artifact_binding != self.artifact_binding
                || anchor.finalized_height > self.block_height
                || evidence
                    .topup_finality_proof
                    .commit_qc
                    .height_context
                    .chain_id
                    != statement.chain_id
                || self
                    .topup_finality_roster_artifact
                    .window_at(anchor.finalized_height)
                    .is_err()
            {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "verify_request.v4.topup_finality_evidence_binding",
                });
            }
            anchored_total = anchored_total
                .checked_add(anchor.amount.atomic_units)
                .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "verify_request.v4.topup_finality_amount",
                })?;
        }
        if anchored_total < statement.current_note.amount.atomic_units {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.v4.topup_finality_amount",
            });
        }
        let Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(transition)) =
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
    /// Enforce the single successful ABI-20 receiver-acceptance contract.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.summary.amount.validate()?;
        self.summary.artifact_binding.validate()?;
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
            || self.verifier_key_id.name != self.verifier_circuit_id
            || self.verifier_key_id.backend.as_str()
                != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_result.v4",
            });
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendRedeemBuildRequestV4 {
    /// Validate the common full/partial ABI-20 redemption-builder input.
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
        let Some(KagemushaRecursiveSpendTransitionV2::RedemptionChange(transition)) =
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
            || change.topup_anchor_refs != input.topup_anchor_refs
            || input.proof_step_count == 0
            || input.proof_step_count.checked_add(1) != Some(change.proof_step_count)
            || change.peer_hop_count != input.peer_hop_count
            || &change.artifact_binding != expected_binding
            || change.verifier_key_id.name != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4
            || change.verifier_key_id.backend.as_str()
                != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
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
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.bundle.validate_public_binding()?;
        self.redemption.validate_public_binding()?;
        self.amount.validate()?;
        validate_kagemusha_redeem_proof_attachment_v2(&self.redeem_proof)?;
        let statement = &self.bundle.statement;
        if self.block_height == 0
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
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRedeemUnsignedPayloadDigestPreimageV4 {
            domain: KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V4.to_owned(),
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
    pub fn into_request(
        self,
        authorization: KagemushaRequestAuthorizationV2,
    ) -> Result<KagemushaRecursiveSpendRedeemRequestV4, KagemushaValidationError> {
        let request = KagemushaRecursiveSpendRedeemRequestV4 {
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
        ) {
            (None, None, None) => Ok(()),
            (Some(branch), Some(bundle), Some(witness)) if &branch.bundle == bundle => {
                branch.validate_for_redemption(&self.unsigned.bundle, &self.unsigned.redemption)?;
                witness.validate_for_root(bundle.statement.final_root)
            }
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_result.v4.offline_change",
            }),
        }
    }

    /// Validate this prepared result against the exact builder input.
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
    pub fn into_redeem_result(
        self,
        authorization: KagemushaRequestAuthorizationV2,
    ) -> Result<KagemushaRecursiveSpendRedeemResultV4, KagemushaValidationError> {
        self.validate_public_binding()?;
        let operation_id = self.operation_id;
        let offline_change_bundle = self.offline_change_bundle;
        let offline_change_membership_witness = self.offline_change_membership_witness;
        let request = self.unsigned.into_request(authorization)?;
        let result = KagemushaRecursiveSpendRedeemResultV4 {
            redeem_request_archive: to_bytes(&request)?,
            offline_change_bundle,
            offline_change_membership_witness,
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
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        let unsigned = self.unsigned_payload();
        unsigned.validate_public_binding()?;
        if self.authorization.operation_id != self.operation_id
            || self.authorization.authority != self.recipient
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "authorization.v4",
            });
        }
        self.authorization.validate_for_payload(unsigned.digest()?)
    }

    /// Return the digest of every unsigned V4 redemption field.
    pub fn unsigned_payload_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.unsigned_payload().digest()
    }

    /// Verify recipient authorization at authoritative Torii time.
    pub fn validate_authorization_at(&self, now_ms: u64) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        self.authorization
            .validate_for_payload_at(self.unsigned_payload_digest()?, now_ms)
    }
}

impl KagemushaRecursiveSpendRedeemResultV4 {
    /// Validate the canonical request archive and terminal/change result shape.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        if self.operation_id == [0; 32]
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
        ) {
            (None, None, None) => Ok(()),
            (Some(branch), Some(bundle), Some(witness)) if &branch.bundle == bundle => {
                branch.validate_for_redemption(&request.bundle, &request.redemption)?;
                witness.validate_for_root(bundle.statement.final_root)
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
    fn abi20_bundle_request_and_redemption_domains_are_unique() {
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
}
