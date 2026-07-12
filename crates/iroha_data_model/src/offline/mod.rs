//! Canonical Kagemusha offline-cash models.
//!
//! The module exposes one lifecycle: exact online top-up, recursive
//! offline split/spend, and exact online redemption.

use iroha_crypto::{Algorithm, Hash, KeyPair, PublicKey, Signature};
use iroha_data_model_derive::model;
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    to_bytes,
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
/// Fixed depth-16 confidential tree capacity used by top-up shielding.
pub const KAGEMUSHA_TOPUP_SHIELD_TREE_CAPACITY_V2: u32 = 1 << 16;
/// Maximum canonical top-up shield proof envelope accepted at typed ingress.
pub const KAGEMUSHA_TOPUP_SHIELD_MAX_PROOF_BYTES_V2: usize = 192 * 1024;
/// Maximum number of branch decisions carried by one recursive spend lineage.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2: u8 = 64;
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
/// Maximum raw Norito archive size that still fits a 12 KiB `PKK2?.` base64url payload.
///
/// `9_211` raw bytes encode to at most `12_282` unpadded base64url bytes;
/// the six-byte transport discriminator brings the complete text payload to
/// exactly 12 KiB. Wallets must apply the limit to the transported text too.
pub const KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2: usize = 9_211;
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
/// Shared verifier role id for confidential transfer evidence.
pub const KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2: &str = "confidential_transfer_v2_verifier_record";
/// Verifier role for public-to-confidential Kagemusha top-up shielding.
pub const KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2: &str =
    "kagemusha_topup_shield_v2_verifier_record";
/// Shared verifier role id for unshield evidence.
pub const KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2: &str = "confidential_unshield_v3_verifier_record";
/// Chain verifier role for the EqAffine/Vesta transition proof profile.
pub const KAGEMUSHA_VERIFIER_ROLE_TRANSITION_V3: &str =
    "kagemusha_recursive_transition_v3_verifier_record";
/// Chain verifier role for the EpAffine/Pallas state-wrapper proof profile.
pub const KAGEMUSHA_VERIFIER_ROLE_STATE_V3: &str = "kagemusha_recursive_state_v3_verifier_record";
/// Shared verifier purpose for top-up and offline split evidence.
pub const KAGEMUSHA_VERIFIER_PURPOSE_TRANSFER_V2: &str = "offline_split";
/// Verifier purpose for the public-to-confidential top-up transition.
pub const KAGEMUSHA_VERIFIER_PURPOSE_TOPUP_SHIELD_V2: &str = "online_to_offline_topup_shield";
/// Shared verifier purpose for offline-to-online redemption.
pub const KAGEMUSHA_VERIFIER_PURPOSE_UNSHIELD_V2: &str = "offline_to_online_redemption";
/// Chain verifier purpose for every value-conserving recursive transition.
pub const KAGEMUSHA_VERIFIER_PURPOSE_TRANSITION_V3: &str = "kagemusha_recursive_spend_transition";
/// Chain verifier purpose for the constant-size recursive state wrapper.
pub const KAGEMUSHA_VERIFIER_PURPOSE_STATE_V3: &str = "kagemusha_recursive_spend_state";
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
/// Exact schema identifier for the production recursive-spend artifact manifest.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3: &str =
    "kagemusha.offline.recursive_spend.artifact_manifest.v3";
/// Proof-system profile selected by the V3 recursive-spend release contract.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1: &str = "halo2/ipa-pasta-cycle-v1";
/// Poseidon transcript profile shared by both Pasta-cycle proof parities.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1: &str =
    "kagemusha-pasta-cycle-poseidon-v1";
/// Circuit id for the EqAffine/Vesta transition proof.
pub const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_EQ_CIRCUIT_ID_V1: &str =
    "kagemusha-recursive-spend-transition-eq-v1";
/// Circuit id for the EpAffine/Pallas state wrapper proof.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1: &str =
    "kagemusha-recursive-spend-state-ep-v1";
/// Verifying-key curve for the EqAffine transition profile.
pub const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_VERIFIER_CURVE_V3: &str = "vesta";
/// Verifying-key curve for the EpAffine state-wrapper profile.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VERIFIER_CURVE_V3: &str = "pallas";
/// Canonical public inputs for the EqAffine/Vesta transition circuit.
pub const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PUBLIC_INPUTS_SCHEMA_V3: &[u8] = br#"{"schema":"kagemusha_recursive_spend_transition_v3","public_inputs":["public_statement_digest_limb0","public_statement_digest_limb1","public_statement_digest_limb2","public_statement_digest_limb3","previous_state_digest_limb0","previous_state_digest_limb1","previous_state_digest_limb2","previous_state_digest_limb3","result_state_digest_limb0","result_state_digest_limb1","result_state_digest_limb2","result_state_digest_limb3","manifest_sha256_limb0","manifest_sha256_limb1","manifest_sha256_limb2","manifest_sha256_limb3"]}"#;
/// Canonical public inputs for the EpAffine/Pallas recursive state wrapper.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_PUBLIC_INPUTS_SCHEMA_V3: &[u8] = br#"{"schema":"kagemusha_recursive_spend_state_v3","public_inputs":["transition_proof_digest_limb0","transition_proof_digest_limb1","transition_proof_digest_limb2","transition_proof_digest_limb3","state_digest_limb0","state_digest_limb1","state_digest_limb2","state_digest_limb3","manifest_sha256_limb0","manifest_sha256_limb1","manifest_sha256_limb2","manifest_sha256_limb3"]}"#;
/// Version of the canonical cross-field state boundary.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V1: u16 = 1;
/// Version of the canonical Pasta-cycle proof envelope.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V1: u16 = 1;
/// Version of the production recursive-spend artifact manifest.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3: u16 = 3;
/// Maximum release proof payload carried by a V2 recursive bundle.
pub const KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3: u32 = 4_096;
/// Canonical IPA domain exponent for both V3 Pasta-cycle profiles.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1: u32 = 12;
/// Maximum size of any one content-addressed V3 artifact file.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3: u64 = 256 * 1024 * 1024;
/// Framing magic for a streamed V3 recursive-spend key artifact.
pub const KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V3: &[u8; 8] = b"KRV3KEY\0";
/// Canonical EqAffine/Vesta parameter package file name.
pub const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PARAMETERS_FILE_NAME_V3: &str =
    "transition-eq.parameters.krv3";
/// Canonical EqAffine/Vesta proving-key package file name.
pub const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROVING_KEY_FILE_NAME_V3: &str =
    "transition-eq.proving-key.krv3";
/// Canonical EqAffine/Vesta verifying-key package file name.
pub const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_VERIFYING_KEY_FILE_NAME_V3: &str =
    "transition-eq.verifying-key.krv3";
/// Canonical EpAffine/Pallas parameter package file name.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_PARAMETERS_FILE_NAME_V3: &str =
    "state-ep.parameters.krv3";
/// Canonical EpAffine/Pallas proving-key package file name.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_PROVING_KEY_FILE_NAME_V3: &str =
    "state-ep.proving-key.krv3";
/// Canonical EpAffine/Pallas verifying-key package file name.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VERIFYING_KEY_FILE_NAME_V3: &str =
    "state-ep.verifying-key.krv3";
/// Circuit/verifier role used by the compact Commit-QC plus anchor-path verifier.
pub const KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2: &str = "kagemusha-topup-finality-qc-merkle-v2";
/// Canonical release-manifest purpose of the trusted validator-roster artifact.
pub const KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2: &str = "topup_finality_roster";
/// Exact Norito type stored in the finality-roster artifact file.
pub const KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2: &str =
    "iroha_data_model::offline::model::KagemushaTopUpFinalityRosterArtifactV2";
/// Canonical release file name for the top-up finality roster.
pub const KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2: &str = "topup-finality-roster.norito";
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

/// Registry schema hash for the V3 transition verifier record.
#[must_use]
pub fn kagemusha_recursive_spend_transition_public_inputs_schema_hash_v3() -> [u8; 32] {
    Hash::new(KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PUBLIC_INPUTS_SCHEMA_V3).into()
}

/// Registry schema hash for the V3 recursive state verifier record.
#[must_use]
pub fn kagemusha_recursive_spend_state_public_inputs_schema_hash_v3() -> [u8; 32] {
    Hash::new(KAGEMUSHA_RECURSIVE_SPEND_STATE_PUBLIC_INPUTS_SCHEMA_V3).into()
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
    /// The authenticated two-layer Pasta recursion backend is not linked.
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
        pub receiver_public_key: PublicKey,
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
        pub receiver_public_key: PublicKey,
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
        pub signature: Signature,
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

    /// Curve/parity role of one proof in the two-layer Pasta recursion cycle.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "parity", content = "value", rename_all = "snake_case")]
    pub enum KagemushaPastaCycleParityV1 {
        /// EqAffine/Vesta transition proof over the Pallas scalar field.
        TransitionEq,
        /// EpAffine/Pallas wrapper proof over the Vesta scalar field.
        StateEp,
    }

    /// Canonical four-limb state digest carried across the Pasta field boundary.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendStateBoundaryV1 {
        /// State-boundary layout version.
        pub layout_version: u16,
        /// Least-significant canonical 64-bit limb of the state digest.
        pub state_digest_limb0: u64,
        /// Second canonical 64-bit limb of the state digest.
        pub state_digest_limb1: u64,
        /// Third canonical 64-bit limb of the state digest.
        pub state_digest_limb2: u64,
        /// Most-significant canonical 64-bit limb of the state digest.
        pub state_digest_limb3: u64,
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
        /// Exact two-layer proof backend profile.
        pub proof_backend: String,
        /// Exact circuit-native transcript profile.
        pub transcript_profile: String,
        /// Circuit selected by `parity`.
        pub circuit_id: String,
        /// Curve/parity of this proof.
        pub parity: KagemushaPastaCycleParityV1,
        /// Human-readable release generation; `manifest_sha256` is the trust binding.
        pub artifact_generation: String,
        /// SHA-256 of the exact authenticated release manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub manifest_sha256: [u8; 32],
        /// Canonical `ParamsIPA` generation identifier.
        pub parameter_generation: String,
        /// SHA-256 of the exact verifier key selected by the envelope.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_key_sha256: [u8; 32],
        /// Canonical cross-field state boundary exposed by the proof.
        pub state_boundary: KagemushaRecursiveSpendStateBoundaryV1,
        /// Backend-native proof bytes.
        pub proof: ProofBox,
    }

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

    /// Production release manifest for the two-layer Pasta recursive backend.
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
        /// Exact two-layer proof backend profile.
        pub proof_backend: String,
        /// Exact circuit-native transcript profile.
        pub transcript_profile: String,
        /// Human-readable release generation; artifact hashes carry content identity.
        pub generation: String,
        /// Lowercase 40-hex source revision.
        pub source_commit: String,
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
        /// Transition then state proof profiles.
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

    /// Installed authenticated artifact release selected by a recursive operation.
    ///
    /// Callers bind the complete manifest, never an individual proving-key
    /// role. The native backend resolves the transition/state profile and key
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
        /// Transition circuit id.
        pub transition_circuit_id: String,
        /// State wrapper circuit id.
        pub state_circuit_id: String,
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
        /// Sender-owned independently spendable change, present exactly when
        /// the split intent contains `change_output`.
        pub change_bundle: Option<KagemushaRecursiveSpendBundleV2>,
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
    }

    /// Versioned receiver verification request.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendVerifyRequestV2 {
        /// Scale- and split-bound recursive bundle.
        pub bundle: KagemushaRecursiveSpendBundleV2,
        /// Receiver request that the final branch must match exactly.
        pub recipient_request: KagemushaRecipientPaymentRequestV2,
        /// Maximum hop count accepted by the receiver for this artifact set.
        pub maximum_hops: u32,
        /// Expected authenticated artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV3,
        /// Authoritative height used to resolve the installed state verifier.
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
        pub receiver_public_key: PublicKey,
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
        pub signature: Signature,
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
    /// Ed25519 public key bytes for local note/proof signatures.
    pub public_key: Vec<u8>,
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
    pub public_key: Vec<u8>,
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
    public_key: Vec<u8>,
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
    public_key: Vec<u8>,
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
    receiver_public_key: PublicKey,
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
        signature: Signature,
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

impl KagemushaRecursiveSpendStateBoundaryV1 {
    /// Validate the canonical cross-field state boundary.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.layout_version != KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V1
            || (self.state_digest_limb0 == 0
                && self.state_digest_limb1 == 0
                && self.state_digest_limb2 == 0
                && self.state_digest_limb3 == 0)
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
            || !is_kagemusha_v3_portable_identifier(&self.parameter_generation)
            || self.manifest_sha256 == [0; 32]
            || self.verifier_key_sha256 == [0; 32]
            || self.proof.backend.as_str() != "halo2/ipa"
            || self.proof.bytes.is_empty()
            || self.proof.bytes.len()
                > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3 as usize
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope",
            });
        }
        let expected_circuit = match self.parity {
            KagemushaPastaCycleParityV1::TransitionEq => {
                KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_EQ_CIRCUIT_ID_V1
            }
            KagemushaPastaCycleParityV1::StateEp => {
                KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1
            }
        };
        if self.circuit_id != expected_circuit {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.circuit_id",
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
        let profile_index = match self.parity {
            KagemushaPastaCycleParityV1::TransitionEq => 0,
            KagemushaPastaCycleParityV1::StateEp => 1,
        };
        let profile = &manifest.profiles[profile_index];
        if self.circuit_id != profile.circuit_id {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.circuit_id",
            });
        }
        if self.parameter_generation != profile.parameter_generation {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.parameter_generation",
            });
        }
        if self.verifier_key_sha256 != profile.artifacts[2].payload_sha256 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.proof_envelope.verifier_key_sha256",
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
            KagemushaPastaCycleParityV1::TransitionEq => (
                KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_EQ_CIRCUIT_ID_V1,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PARAMETERS_FILE_NAME_V3,
                    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROVING_KEY_FILE_NAME_V3,
                    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_VERIFYING_KEY_FILE_NAME_V3,
                ],
            ),
            KagemushaPastaCycleParityV1::StateEp => (
                KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1,
                [
                    KAGEMUSHA_RECURSIVE_SPEND_STATE_PARAMETERS_FILE_NAME_V3,
                    KAGEMUSHA_RECURSIVE_SPEND_STATE_PROVING_KEY_FILE_NAME_V3,
                    KAGEMUSHA_RECURSIVE_SPEND_STATE_VERIFYING_KEY_FILE_NAME_V3,
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
            || !is_kagemusha_v3_chain_id(&self.chain_id)
            || self.asset_scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
            || self.activation_height == 0
            || self.withdrawal_height <= self.activation_height
            || self.max_proof_bytes != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3
            || self.profiles.len() != 2
            || self.profiles[0].parity != KagemushaPastaCycleParityV1::TransitionEq
            || self.profiles[1].parity != KagemushaPastaCycleParityV1::StateEp
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

impl KagemushaRecursiveSpendNativeCapabilitiesV1 {
    /// Validate that a capability record exactly describes this bridge contract.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3
            || self.artifact_manifest_schema
                != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1
            || self.proof_envelope_version
                != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V1
            || self.state_boundary_version != KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V1
            || self.transition_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_EQ_CIRCUIT_ID_V1
            || self.state_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1
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
        proof_envelope_version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V1,
        state_boundary_version: KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V1,
        transition_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_EQ_CIRCUIT_ID_V1.to_owned(),
        state_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1.to_owned(),
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
        "opposite_field_pasta_loader",
        "cross_field_poseidon_transcript",
        "two_layer_recursive_accumulator",
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
            || self.parent_peer_hop_count > u32::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2)
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
    /// Validate the prepared unsigned request and optional proof-bound change.
    pub fn validate_for_request(
        &self,
        request: &KagemushaRecursiveSpendRedeemBuildRequestV2,
    ) -> Result<(), KagemushaValidationError> {
        request.validate_public_binding()?;
        self.unsigned.validate_public_binding()?;
        if self.operation_id == [0; 32]
            || self.operation_id != request.operation_id
            || self.operation_id != self.unsigned.operation_id
            || self.authorization_digest != self.unsigned.digest()?
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
        match (
            &self.unsigned.offline_change,
            &self.offline_change_bundle,
            &request.redemption.change_output,
        ) {
            (None, None, None) => {}
            (Some(branch), Some(bundle), Some(_)) if &branch.bundle == bundle => {
                branch.validate_for_redemption(&request.bundle, &request.redemption)?;
            }
            _ => {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "redeem_build_result.offline_change",
                });
            }
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendSplitResultV2 {
    /// Construct and validate independently spendable recipient/change branches.
    pub fn new(
        split: KagemushaRecursiveSpendSplitIntentV2,
        split_binding_digest: [u8; 32],
        recipient_bundle: KagemushaRecursiveSpendBundleV2,
        change_bundle: Option<KagemushaRecursiveSpendBundleV2>,
    ) -> Result<Self, KagemushaValidationError> {
        let result = Self {
            split,
            split_binding_digest,
            recipient_bundle,
            change_bundle,
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

        match (&self.split.change_output, &self.change_bundle) {
            (None, None) => Ok(()),
            (Some(_), Some(change_bundle)) => {
                self.validate_branch(change_bundle, KagemushaRecursiveSpendBranchV2::Change)?;
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

impl KagemushaRecursiveSpendPeerPaymentV2 {
    /// Project the recipient-only transport envelope from a local split result.
    pub fn from_split_result(
        result: &KagemushaRecursiveSpendSplitResultV2,
    ) -> Result<Self, KagemushaValidationError> {
        result.validate_public_binding()?;
        let payment = Self {
            recipient_bundle: result.recipient_bundle.clone(),
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
    receiver_public_key: &PublicKey,
) -> Result<[u8; 32], KagemushaValidationError> {
    kagemusha_poseidon_preimage(&KagemushaReceiverKeyReferencePreimageV2 {
        domain: KAGEMUSHA_RECEIVER_KEY_REFERENCE_DOMAIN_V2.to_owned(),
        receiver_public_key: receiver_public_key.clone(),
    })
}

impl KagemushaReceiverAcknowledgementPayloadV2 {
    /// Validate structural fields and the domain-separated public-key reference.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
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
        if let Some(change) = &self.offline_change_bundle {
            change.validate_public_binding()?;
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
            || change.verifier_key_id.name != KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1
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
                || input.peer_hop_count >= u32::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2)
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
            || self.peer_hop_count > u32::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2)
            || claim_roots != lineage_roots
            || self.verifier_key_id.name != KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1
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

impl KagemushaRecursiveSpendVerifyRequestV2 {
    /// Validate receiver request, artifact, hop, and exact recipient output.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.bundle.validate_public_binding()?;
        self.artifact_binding.validate()?;
        self.recipient_request.validate_at(self.verified_at_ms)?;
        if self.verified_at_ms == 0
            || self.block_height == 0
            || self.maximum_hops == 0
            || self.maximum_hops > u32::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2)
            || self.bundle.statement.peer_hop_count > self.maximum_hops
            || self.artifact_binding != self.bundle.statement.artifact_binding
            || self.recipient_request.chain_id != self.bundle.statement.chain_id
            || self.recipient_request.asset != self.bundle.statement.asset
            || self.recipient_request.amount.scale != self.bundle.statement.asset_scale
            || self.bundle.statement.current_note != self.recipient_request.recipient_output
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request",
            });
        }
        let Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(transition)) =
            self.bundle.statement.transition.as_ref()
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
            || self.verifier_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1
            || self.verified_at_block_height == 0
            || self.verified_at_ms == 0
            || self.summary.verifier_key_id != self.verifier_key_id
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
