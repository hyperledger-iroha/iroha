//! Offline note models.
//!
//! Offline is the first production offline note surface. The legacy
//! allowance, witness-lineage, plaintext receipt, and aggregate proof models are
//! intentionally absent from this module.

use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model_derive::model;
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    to_bytes,
};

pub use self::model::*;
use crate::{
    ChainId,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    proof::{ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
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
/// Canonical Offline key-certificate format marker for the first release.
pub const OFFLINE_NOTE_KEY_CERTIFICATE_VERSION: u16 = 1;
/// Domain-separation tag for wallet-derived Offline Note note commitments.
pub const OFFLINE_NOTE_NOTE_COMMITMENT_DOMAIN: &str = "iroha:offline-note:note-commitment";
/// Domain-separation tag for wallet-derived Offline Note input nullifiers.
pub const OFFLINE_NOTE_INPUT_NULLIFIER_DOMAIN: &str = "iroha:offline-note:input-nullifier";
/// Domain-separation tag for wallet-derived Offline Note payment token identifiers.
pub const OFFLINE_NOTE_PAYMENT_TOKEN_ID_DOMAIN: &str = "iroha:offline-note:payment-token-id";
/// Domain-separation tag for compact Kagemusha folded-proof public inputs.
pub const KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN: &str = "iroha:kagemusha:v1:folded-public-inputs";
/// Current Kagemusha aggregation mode: every private hop proof is verified before folding.
pub const KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1: u16 = 1;
/// Reserved Kagemusha aggregation mode for future in-circuit recursive proof aggregation.
pub const KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1: u16 = 2;
/// Return `true` when this release accepts the Kagemusha aggregation mode.
#[must_use]
pub const fn is_supported_kagemusha_aggregation_mode(mode: u16) -> bool {
    mode == KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
}

/// Return the stable rejection reason for an unsupported Kagemusha aggregation mode.
#[must_use]
pub const fn unsupported_kagemusha_aggregation_mode_reason(mode: u16) -> &'static str {
    match mode {
        KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1 => {
            "reserved for future in-circuit recursive aggregation; no recursive verifier is shipped in this release"
        }
        _ => "unsupported or unknown Kagemusha aggregation mode",
    }
}

/// Return `true` when `backend` is accepted for Kagemusha proof transcript material.
#[must_use]
pub fn is_supported_kagemusha_proof_backend(backend: &str) -> bool {
    if is_trusted_setup_kagemusha_backend(backend) || is_developer_only_kagemusha_backend(backend) {
        return false;
    }
    backend == "halo2/ipa"
        || backend == "stark/fri"
        || backend
            .strip_prefix("stark/fri/")
            .is_some_and(|profile| !profile.trim().is_empty())
}

fn is_trusted_setup_kagemusha_backend(backend: &str) -> bool {
    let backend = backend.to_ascii_lowercase();
    let backend = backend.as_str();
    has_trusted_setup_kagemusha_backend_segment(backend)
        || backend == "groth16"
        || backend.starts_with("groth16/")
        || backend == "kzg"
        || backend.starts_with("kzg/")
        || backend == "bn254"
        || backend == "bn256"
        || backend == "bls12_381"
        || backend == "bls12-381"
        || backend == "halo2/bn254"
        || backend.starts_with("halo2/bn254/")
        || backend.contains("/bn254")
        || backend.contains(":bn254")
        || backend.contains("/bn256")
        || backend.contains(":bn256")
        || backend.contains("/bls12")
        || backend.contains(":bls12")
        || backend == "halo2/kzg"
        || backend.starts_with("halo2/kzg/")
        || backend.contains("/kzg")
        || backend.contains(":kzg")
}

fn has_trusted_setup_kagemusha_backend_segment(backend: &str) -> bool {
    backend
        .split(|ch: char| ch == '/' || ch == ':' || ch.is_ascii_whitespace())
        .any(|segment| {
            matches!(
                segment,
                "groth16" | "kzg" | "bn254" | "bn256" | "bls12" | "bls12_381" | "bls12-381"
            )
        })
}

fn is_developer_only_kagemusha_backend(backend: &str) -> bool {
    let backend = backend.to_ascii_lowercase();
    backend.contains("debug") || backend.contains("mock")
}

fn kagemusha_backend_tag(backend: &str) -> Option<BackendTag> {
    if backend == "halo2/ipa" {
        Some(BackendTag::Halo2IpaPasta)
    } else if is_supported_kagemusha_proof_backend(backend) {
        Some(BackendTag::Stark)
    } else {
        None
    }
}

/// Domain-separation tag for the Poseidon2 Kagemusha aggregation transcript.
pub const KAGEMUSHA_POSEIDON_AGGREGATION_TRANSCRIPT_DOMAIN: &str =
    "iroha:kagemusha:v1:poseidon-aggregation-transcript";
/// Domain-separation tag for Kagemusha per-hop proof public-input statements.
pub const KAGEMUSHA_PROOF_PUBLIC_INPUTS_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:proof-public-inputs";
/// Domain-separation tag for Kagemusha per-hop verifier-key Poseidon2 digests.
pub const KAGEMUSHA_VERIFIER_KEY_DIGEST_DOMAIN: &str = "iroha:kagemusha:v1:verifier-key";
/// Maximum number of private Kagemusha hops folded into one compact token.
pub const KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS: usize = 64;
/// Maximum expected Norito size for chain-visible Kagemusha folded public inputs.
///
/// The compact public transcript must remain independent of hop count; proof
/// bytes are budgeted separately by the verifier-key record.
pub const KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES: usize = 1024;
/// Maximum input nullifiers per Kagemusha fold step.
pub const KAGEMUSHA_FOLD_STEP_MAX_INPUTS: usize = 2;
/// Maximum output commitments per Kagemusha fold step.
pub const KAGEMUSHA_FOLD_STEP_MAX_OUTPUTS: usize = 2;
/// Error returned when Offline Note canonical derivation inputs are invalid.
#[derive(Debug)]
pub enum OfflineNoteDerivationError {
    /// Random secret material must be exactly 32 bytes.
    InvalidRandomBytesLength {
        /// Name of the invalid field.
        field: &'static str,
        /// Expected byte count.
        expected: usize,
        /// Actual byte count.
        actual: usize,
    },
    /// Canonical Norito encoding failed.
    Encode(norito::Error),
}

impl core::fmt::Display for OfflineNoteDerivationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidRandomBytesLength {
                field,
                expected,
                actual,
            } => write!(
                f,
                "Offline Note {field} must be exactly {expected} bytes (found {actual})"
            ),
            Self::Encode(err) => write!(f, "failed to encode Offline Note preimage: {err}"),
        }
    }
}

impl std::error::Error for OfflineNoteDerivationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidRandomBytesLength { .. } => None,
            Self::Encode(err) => Some(err),
        }
    }
}

impl From<norito::Error> for OfflineNoteDerivationError {
    fn from(err: norito::Error) -> Self {
        Self::Encode(err)
    }
}

/// Error returned when compact Kagemusha folded-proof public inputs are invalid.
#[derive(Debug)]
pub enum KagemushaFoldError {
    /// Folded public inputs use an unsupported domain separator.
    InvalidPublicInputDomain {
        /// Expected domain separator.
        expected: &'static str,
        /// Domain separator carried by the token.
        actual: String,
    },
    /// Folded public inputs use an unsupported aggregation mode.
    UnsupportedAggregationMode {
        /// Expected aggregation mode.
        expected: u16,
        /// Aggregation mode carried by the token.
        actual: u16,
        /// Stable reason explaining why the mode is rejected.
        reason: &'static str,
    },
    /// At least one private hop is required.
    Empty,
    /// The private hop count exceeds [`KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS`].
    TooManyHops {
        /// Maximum accepted hop count.
        max: usize,
        /// Actual hop count.
        actual: usize,
    },
    /// A hop does not match the supported 1-to-2 transfer shape.
    InvalidStepShape {
        /// Zero-based hop index.
        hop_index: usize,
        /// Input nullifier count.
        input_count: usize,
        /// Output commitment count.
        output_count: usize,
    },
    /// An input nullifier is repeated within or across folded hops.
    DuplicateInputNullifier {
        /// Zero-based hop index where the duplicate was detected.
        hop_index: usize,
    },
    /// An output commitment is repeated within or across folded hops.
    DuplicateOutputCommitment {
        /// Zero-based hop index where the duplicate was detected.
        hop_index: usize,
    },
    /// A folded-hop input nullifier is all-zero.
    ZeroInputNullifier {
        /// Zero-based hop index where the zero entry was detected.
        hop_index: usize,
    },
    /// A folded-hop output commitment is all-zero.
    ZeroOutputCommitment {
        /// Zero-based hop index where the zero entry was detected.
        hop_index: usize,
    },
    /// A folded-hop proof public-input digest is all-zero.
    ZeroProofPublicInputsDigest {
        /// Zero-based hop index where the zero digest was detected.
        hop_index: usize,
    },
    /// A folded-hop verifier-key commitment is all-zero.
    ZeroVerifierKeyCommitment {
        /// Zero-based hop index where the zero commitment was detected.
        hop_index: usize,
    },
    /// A folded-hop verifier-key Poseidon2 digest is all-zero.
    ZeroVerifierKeyPoseidonDigest {
        /// Zero-based hop index where the zero digest was detected.
        hop_index: usize,
    },
    /// A folded transcript Merkle root is all-zero.
    ZeroFoldedRoot {
        /// Name of the all-zero root field.
        field: &'static str,
    },
    /// A folded hop root transition does not change the Merkle root.
    UnchangedFoldedRootTransition {
        /// Zero-based hop index where the unchanged transition was detected.
        hop_index: usize,
    },
    /// Folded public inputs carry the same initial and final root.
    UnchangedFoldedPublicRoots,
    /// A direct aggregation transcript statement has a non-canonical hop count.
    HopCountMismatch {
        /// Expected hop count from the statement step list.
        expected: usize,
        /// Hop count carried by the statement.
        actual: u32,
    },
    /// A direct aggregation transcript statement has a non-canonical hop index.
    HopIndexMismatch {
        /// Expected zero-based hop index.
        expected: usize,
        /// Hop index carried by the statement step.
        actual: u32,
    },
    /// A direct aggregation transcript statement carries the wrong initial root.
    InitialRootMismatch {
        /// Expected initial root from the first hop.
        expected: [u8; Hash::LENGTH],
        /// Initial root carried by the statement.
        actual: [u8; Hash::LENGTH],
    },
    /// A direct aggregation transcript statement carries the wrong final root.
    FinalRootMismatch {
        /// Expected final root from the last hop.
        expected: [u8; Hash::LENGTH],
        /// Final root carried by the statement.
        actual: [u8; Hash::LENGTH],
    },
    /// A direct aggregation transcript statement has non-canonical input order.
    NonCanonicalInputNullifierOrder {
        /// Zero-based hop index where ordering failed.
        hop_index: usize,
    },
    /// A direct aggregation transcript statement has non-canonical output order.
    NonCanonicalOutputCommitmentOrder {
        /// Zero-based hop index where ordering failed.
        hop_index: usize,
    },
    /// Adjacent folded hops do not connect through the same Merkle root.
    RootDiscontinuity {
        /// Zero-based hop index where the discontinuity was detected.
        hop_index: usize,
        /// Root expected from the previous hop.
        expected: [u8; Hash::LENGTH],
        /// Root supplied by the current hop.
        actual: [u8; Hash::LENGTH],
    },
    /// A compact token proof is not bound to its canonical folded public inputs.
    PublicInputHashMismatch {
        /// Hash computed from the folded public inputs.
        expected: Hash,
        /// Hash declared by the folded proof.
        actual: Hash,
    },
    /// Folded public inputs are not the canonical projection of the aggregation transcript.
    FoldedPublicInputTranscriptMismatch {
        /// Name of the mismatched folded public-input field.
        field: &'static str,
    },
    /// A folded public-input digest column group is all-zero.
    ZeroFoldedPublicInputDigest {
        /// Name of the all-zero folded public-input digest field.
        field: &'static str,
    },
    /// Folded public inputs exceed the compact-token public transcript size budget.
    EncodedSizeExceeded {
        /// Maximum accepted encoded size in bytes.
        max: usize,
        /// Actual encoded size in bytes.
        actual: usize,
    },
    /// A Kagemusha proof public-input statement carries a zero verifier-key hash.
    ZeroProofStatementVerifierKeyHash,
    /// A Kagemusha proof public-input statement carries an empty circuit id.
    EmptyProofStatementCircuitId,
    /// A Kagemusha proof public-input statement carries an empty public-input schema.
    EmptyProofStatementPublicInputsSchema,
    /// A Kagemusha proof public-input statement carries no public instance columns.
    EmptyProofStatementInstanceColumns,
    /// A Kagemusha proof public-input statement carries an empty public instance column.
    EmptyProofStatementInstanceColumn {
        /// Zero-based public instance column index.
        column_index: usize,
    },
    /// A Kagemusha proof public-input statement carries non-canonical auxiliary bytes.
    NonCanonicalProofStatementAuxiliaryBytes {
        /// Actual auxiliary byte count.
        actual: usize,
    },
    /// A Kagemusha verifier-key digest was requested for empty key bytes.
    EmptyVerifierKeyBytes {
        /// Backend label associated with the empty key bytes.
        backend: String,
    },
    /// A Kagemusha folded hop carries an empty verifier-key id name.
    EmptyVerifierKeyIdName {
        /// Zero-based hop index where the empty verifier-key id was found.
        hop_index: usize,
    },
    /// A Kagemusha proof statement or verifier-key digest used an unsupported proof backend.
    UnsupportedProofBackend {
        /// Unsupported backend label.
        backend: String,
    },
    /// A Kagemusha proof public-input statement backend tag does not match its proof backend.
    ProofStatementBackendTagMismatch {
        /// Proof backend label.
        proof_backend: String,
        /// Backend tag carried by the statement.
        envelope_backend: BackendTag,
    },
    /// Canonical Norito encoding failed.
    Encode(norito::Error),
}

impl core::fmt::Display for KagemushaFoldError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidPublicInputDomain { expected, actual } => write!(
                f,
                "Kagemusha folded token domain must be {expected:?} (found {actual:?})"
            ),
            Self::UnsupportedAggregationMode {
                expected,
                actual,
                reason,
            } => write!(
                f,
                "Kagemusha folded token aggregation mode must be {expected} (found {actual}: {reason})"
            ),
            Self::Empty => write!(f, "Kagemusha folded token requires at least one hop"),
            Self::TooManyHops { max, actual } => write!(
                f,
                "Kagemusha folded token supports at most {max} hops (found {actual})"
            ),
            Self::InvalidStepShape {
                hop_index,
                input_count,
                output_count,
            } => write!(
                f,
                "Kagemusha fold hop {hop_index} requires 1 to {KAGEMUSHA_FOLD_STEP_MAX_INPUTS} inputs and 1 to {KAGEMUSHA_FOLD_STEP_MAX_OUTPUTS} outputs (found {input_count} inputs and {output_count} outputs)"
            ),
            Self::DuplicateInputNullifier { hop_index } => {
                write!(
                    f,
                    "Kagemusha fold hop {hop_index} repeats an input nullifier"
                )
            }
            Self::DuplicateOutputCommitment { hop_index } => {
                write!(
                    f,
                    "Kagemusha fold hop {hop_index} repeats an output commitment"
                )
            }
            Self::ZeroInputNullifier { hop_index } => {
                write!(
                    f,
                    "Kagemusha fold hop {hop_index} has a zero input nullifier"
                )
            }
            Self::ZeroOutputCommitment { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} has a zero output commitment"
            ),
            Self::ZeroProofPublicInputsDigest { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} has a zero proof public-input digest"
            ),
            Self::ZeroVerifierKeyCommitment { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} has a zero verifier-key commitment"
            ),
            Self::ZeroVerifierKeyPoseidonDigest { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} has a zero verifier-key Poseidon2 digest"
            ),
            Self::ZeroFoldedRoot { field } => {
                write!(f, "Kagemusha folded root field {field:?} must be non-zero")
            }
            Self::UnchangedFoldedRootTransition { hop_index } => {
                write!(
                    f,
                    "Kagemusha fold hop {hop_index} must change the Merkle root"
                )
            }
            Self::UnchangedFoldedPublicRoots => write!(
                f,
                "Kagemusha folded public inputs require distinct initial and final roots"
            ),
            Self::HopCountMismatch { expected, actual } => write!(
                f,
                "Kagemusha aggregation transcript hop_count must be {expected} (found {actual})"
            ),
            Self::HopIndexMismatch { expected, actual } => write!(
                f,
                "Kagemusha aggregation transcript hop index must be {expected} (found {actual})"
            ),
            Self::InitialRootMismatch { .. } => write!(
                f,
                "Kagemusha aggregation transcript initial root does not match the first hop"
            ),
            Self::FinalRootMismatch { .. } => write!(
                f,
                "Kagemusha aggregation transcript final root does not match the last hop"
            ),
            Self::NonCanonicalInputNullifierOrder { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} input nullifiers must be sorted canonically"
            ),
            Self::NonCanonicalOutputCommitmentOrder { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} output commitments must be sorted canonically"
            ),
            Self::RootDiscontinuity { hop_index, .. } => write!(
                f,
                "Kagemusha fold hop {hop_index} does not continue from the previous root"
            ),
            Self::PublicInputHashMismatch { .. } => write!(
                f,
                "Kagemusha folded proof public-input hash does not match the compact token"
            ),
            Self::FoldedPublicInputTranscriptMismatch { field } => write!(
                f,
                "Kagemusha folded public input field {field:?} does not match the aggregation transcript"
            ),
            Self::ZeroFoldedPublicInputDigest { field } => write!(
                f,
                "Kagemusha folded public input digest field {field:?} must be non-zero"
            ),
            Self::EncodedSizeExceeded { max, actual } => write!(
                f,
                "Kagemusha folded public inputs must encode to at most {max} bytes (found {actual})"
            ),
            Self::ZeroProofStatementVerifierKeyHash => write!(
                f,
                "Kagemusha proof public-input statement verifier-key hash must be non-zero"
            ),
            Self::EmptyProofStatementCircuitId => {
                write!(
                    f,
                    "Kagemusha proof public-input statement circuit id must be non-empty"
                )
            }
            Self::EmptyProofStatementPublicInputsSchema => write!(
                f,
                "Kagemusha proof public-input statement schema must be non-empty"
            ),
            Self::EmptyProofStatementInstanceColumns => write!(
                f,
                "Kagemusha proof public-input statement instance columns must be non-empty"
            ),
            Self::EmptyProofStatementInstanceColumn { column_index } => write!(
                f,
                "Kagemusha proof public-input statement instance column {column_index} must be non-empty"
            ),
            Self::NonCanonicalProofStatementAuxiliaryBytes { actual } => write!(
                f,
                "Kagemusha proof public-input statement auxiliary bytes must be empty (found {actual})"
            ),
            Self::EmptyVerifierKeyBytes { backend } => write!(
                f,
                "Kagemusha verifier-key bytes for backend {backend:?} must be non-empty"
            ),
            Self::EmptyVerifierKeyIdName { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} verifier-key id name must be non-empty"
            ),
            Self::UnsupportedProofBackend { backend } => {
                write!(f, "Kagemusha proof backend {backend:?} is not supported")
            }
            Self::ProofStatementBackendTagMismatch {
                proof_backend,
                envelope_backend,
            } => write!(
                f,
                "Kagemusha proof public-input statement backend tag {envelope_backend:?} does not match proof backend {proof_backend:?}"
            ),
            Self::Encode(err) => {
                write!(f, "failed to encode Kagemusha folded public inputs: {err}")
            }
        }
    }
}

impl std::error::Error for KagemushaFoldError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(err) => Some(err),
            _ => None,
        }
    }
}

impl From<norito::Error> for KagemushaFoldError {
    fn from(err: norito::Error) -> Self {
        Self::Encode(err)
    }
}

/// Derive the deterministic Offline escrow account for an asset definition.
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
    let keypair = KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519);
    AccountId::new(keypair.public_key().clone())
}

#[model]
mod model {
    use super::*;

    /// Compact CA-issued certificate for an Offline one-use note key.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteKeyCertificate {
        /// Certificate format marker.
        pub version: u16,
        /// Platform class, for example `ios-appattest` or `android-keymint`.
        pub platform: String,
        /// Issuer-scoped one-use key identifier.
        pub key_id: String,
        /// Device identifier bound by the offline CA.
        pub device_id: String,
        /// Account authorized to control the note key.
        pub account_id: AccountId,
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
        /// True when the issuer verified hardware one-use semantics.
        pub one_use: bool,
        /// Offline CA signature over the compact certificate payload.
        pub issuer_signature: Signature,
    }

    /// Canonical payload signed by Offline key-certificate issuers.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteKeyCertificatePayload {
        /// Domain separator for the signed payload.
        pub domain: String,
        /// Certificate format marker.
        pub version: u16,
        /// Platform class, for example `ios-appattest` or `android-keymint`.
        pub platform: String,
        /// Issuer-scoped one-use key identifier.
        pub key_id: String,
        /// Device identifier bound by the offline CA.
        pub device_id: String,
        /// Account authorized to control the note key.
        pub account_id: AccountId,
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
        /// True when the issuer verified hardware one-use semantics.
        pub one_use: bool,
    }

    /// Verifier-key-backed recursive proof carried by Offline note tokens.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteRecursiveProof {
        /// Stable verifier key identifier selected by the operator and stored in WSV.
        pub verifier_key_id: VerifyingKeyId,
        /// Public input commitment hash.
        pub public_inputs_hash: Hash,
        /// Compact recursive proof payload encoded as an `OpenVerifyEnvelope`.
        pub proof: ProofBox,
    }

    /// Issuer-side note issuance record for online load/consolidation.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteIssue {
        /// Deterministic note commitment.
        pub note_commitment: Hash,
        /// Owner key certificate for this note.
        pub key_certificate: OfflineNoteKeyCertificate,
        /// Asset held by the note.
        pub asset: AssetId,
        /// Note amount.
        pub amount: Numeric,
    }

    /// Ledger-recognized note claim bound to one compact Offline note certificate.
    ///
    /// Issuer loads create this claim directly; P2P bearer outputs create the same claim only
    /// when their audit lineage is submitted, either before redemption or earlier in the same
    /// transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteIssuedClaim {
        /// Domain separator for the issued-note claim.
        pub domain: String,
        /// Deterministic note commitment recorded at issuance.
        pub note_commitment: Hash,
        /// Certificate payload hash identifying the one-use note key.
        pub key_certificate_payload_hash: Hash,
        /// Asset held by the issued note.
        pub asset: AssetId,
        /// Note amount reserved into offline escrow.
        pub amount: Numeric,
    }

    /// Redeemable note output observed during Offline audit.
    ///
    /// The output is final for offline bearers when received locally. The ledger recognizes it
    /// after the corresponding audit is committed.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteAuditOutputClaim {
        /// Deterministic note commitment created by the audited transfer.
        pub note_commitment: Hash,
        /// Owner key certificate for this output note.
        pub key_certificate: OfflineNoteKeyCertificate,
        /// Asset held by this output note.
        pub asset: AssetId,
        /// Output amount reserved in offline escrow.
        pub amount: Numeric,
    }

    /// Redemption payload submitted online when defunding a bearer note.
    ///
    /// The source claim must already be ledger-recognized. For unanchored P2P bearer outputs,
    /// submit their ordered audit lineage before this redeem instruction in the same transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteRedeem {
        /// Ledger-recognized note commitment consumed by this redemption.
        pub source_note_commitment: Hash,
        /// Nullifiers consumed by the redeeming token.
        pub input_nullifiers: Vec<Hash>,
        /// Compact certificate for the one-use note key that signed the proof.
        pub sender_key_certificate: OfflineNoteKeyCertificate,
        /// Recipient account credited online.
        pub recipient: AccountId,
        /// Asset being redeemed.
        pub asset: AssetId,
        /// Redeemed amount.
        pub amount: Numeric,
        /// Compact recursive proof for the final note state.
        pub recursive_proof: OfflineNoteRecursiveProof,
    }

    /// Public inputs bound by an Offline redemption proof.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteRedeemPublicInputs {
        /// Domain separator for the redemption public inputs.
        pub domain: String,
        /// Ledger-recognized note commitment consumed by this redemption.
        pub source_note_commitment: Hash,
        /// Nullifiers consumed by the redeeming token.
        pub input_nullifiers: Vec<Hash>,
        /// Certificate payload hash identifying the one-use note key.
        pub key_certificate_payload_hash: Hash,
        /// Recipient account credited online.
        pub recipient: AccountId,
        /// Asset being redeemed.
        pub asset: AssetId,
        /// Redeemed amount.
        pub amount: Numeric,
    }

    /// Audit bundle for Offline P2P bearer lineage.
    ///
    /// It is not required for offline transfer finality, but it anchors P2P output claims so the
    /// ledger can later redeem them from offline escrow. Ledger execution checks each output
    /// certificate signature against the output account before recording that new lineage.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteAuditBundle {
        /// Payment token identifier.
        pub token_id: Hash,
        /// Compact certificate for the one-use note key that signed the proof.
        pub sender_key_certificate: OfflineNoteKeyCertificate,
        /// Input nullifiers observed in the token.
        pub input_nullifiers: Vec<Hash>,
        /// Issued input claims consumed by the token.
        pub input_claims: Vec<OfflineNoteIssuedClaim>,
        /// Output note commitments created by the token.
        pub output_commitments: Vec<Hash>,
        /// Redeemable output claims created by the token.
        pub output_claims: Vec<OfflineNoteAuditOutputClaim>,
        /// Optional recursive proof for audit/replay checks.
        pub recursive_proof: OfflineNoteRecursiveProof,
    }

    /// Public inputs bound by an Offline optional audit proof.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteAuditPublicInputs {
        /// Domain separator for the audit public inputs.
        pub domain: String,
        /// Payment token identifier.
        pub token_id: Hash,
        /// Certificate payload hash identifying the one-use note key.
        pub key_certificate_payload_hash: Hash,
        /// Input nullifiers observed in the token.
        pub input_nullifiers: Vec<Hash>,
        /// Issued input claims consumed by the token.
        pub input_claims: Vec<OfflineNoteIssuedClaim>,
        /// Output note commitments created by the token.
        pub output_commitments: Vec<Hash>,
        /// Redeemable output claims created by the token.
        pub output_claims: Vec<OfflineNoteIssuedClaim>,
    }

    /// Origin of a wallet-derived Offline Note note commitment.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteIssuerLoadOrigin {
        /// Wallet operation id sent to Torii.
        pub operation_id: String,
        /// Issuer lineage id updated by Torii.
        pub lineage_id: String,
        /// Local lineage revision after issuing the note.
        pub local_revision: u64,
    }

    /// Origin data for an offline peer-to-peer payment token output.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteP2pOutputOrigin {
        /// Recipient payment request id.
        pub payment_request_id: String,
        /// Output index inside the payment token.
        pub output_index: u32,
    }

    /// Canonical preimage used to derive an Offline Note note commitment.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteCommitmentPreimage {
        /// Domain separator for note commitments.
        pub domain: String,
        /// Chain id that scopes this note.
        pub chain_id: ChainId,
        /// Hash of the owner key certificate payload.
        pub owner_key_certificate_payload_hash: Hash,
        /// Asset held by the note.
        pub asset: AssetId,
        /// Note amount.
        pub amount: Numeric,
        /// Wallet-generated 32-byte note secret.
        pub note_secret: Vec<u8>,
        /// Origin metadata that separates issuer loads from P2P outputs.
        pub origin: OfflineNoteCommitmentOrigin,
    }

    /// Canonical preimage used to derive an Offline Note input nullifier.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteInputNullifierPreimage {
        /// Domain separator for input nullifiers.
        pub domain: String,
        /// Chain id that scopes this nullifier.
        pub chain_id: ChainId,
        /// Commitment of the note being spent.
        pub source_note_commitment: Hash,
        /// Hash of the owner key certificate payload.
        pub owner_key_certificate_payload_hash: Hash,
        /// Wallet-generated 32-byte note secret.
        pub note_secret: Vec<u8>,
    }

    /// Canonical preimage used to derive an Offline Note payment token id.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNotePaymentTokenIdPreimage {
        /// Domain separator for payment token ids.
        pub domain: String,
        /// Chain id that scopes this payment token.
        pub chain_id: ChainId,
        /// Wallet-local payment request id that binds this token to one receive request.
        pub payment_request_id: String,
        /// Wallet-local token creation time in Unix milliseconds.
        pub created_at_ms: u64,
        /// Wallet-generated 32-byte payment token nonce.
        pub token_nonce: Vec<u8>,
        /// Hash of the sender key certificate payload.
        pub sender_key_certificate_payload_hash: Hash,
        /// Input nullifiers consumed by the token.
        pub input_nullifiers: Vec<Hash>,
        /// Output commitments created by the token.
        pub output_commitments: Vec<Hash>,
    }

    /// Canonical public-input statement verified for one Kagemusha private hop proof.
    ///
    /// Wallets and future recursive aggregators hash this statement with
    /// [`kagemusha_proof_public_inputs_statement_digest`] before inserting it into a folded-hop
    /// transcript. The statement is canonical only when `vk_hash` is non-zero and
    /// `envelope_aux` is empty; the private proof payload itself is committed separately by
    /// `proof_hash`.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaProofPublicInputsStatement {
        /// Proof backend label carried by the verified `ProofBox`.
        pub proof_backend: String,
        /// Backend tag carried by the transparent `OpenVerifyEnvelope`.
        pub envelope_backend: BackendTag,
        /// Circuit identifier carried by the transparent `OpenVerifyEnvelope`.
        pub circuit_id: String,
        /// Verifier-key hash carried by the transparent `OpenVerifyEnvelope`.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub vk_hash: [u8; 32],
        /// Public-input schema or descriptor bytes carried by the transparent envelope.
        pub public_inputs_schema: Vec<u8>,
        /// Auxiliary bytes carried by the transparent envelope; Kagemusha statements require this
        /// to be empty.
        pub envelope_aux: Vec<u8>,
        /// Backend-native public input columns that were verified.
        pub instance_columns: Vec<Vec<[u8; 32]>>,
    }

    /// One hop statement inside the Poseidon2 Kagemusha aggregation transcript.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPoseidonAggregationStepStatement {
        /// Zero-based hop index inside the folded transcript.
        pub hop_index: u32,
        /// Recent shielded Merkle root before this private hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root_before: [u8; 32],
        /// Canonicalized input nullifiers consumed by this private hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub input_nullifiers: Vec<[u8; 32]>,
        /// Canonicalized output note commitments created by this private hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub output_commitments: Vec<[u8; 32]>,
        /// Shielded Merkle root after this private hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root_after: [u8; 32],
        /// Domain-separated hash of the transparent per-hop proof payload.
        pub proof_hash: Hash,
        /// Poseidon2 digest of the per-hop proof public input statement.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub proof_public_inputs_digest: [u8; 32],
        /// Verifier key identifier used to verify the per-hop proof.
        pub verifier_key_id: VerifyingKeyId,
        /// Host-side commitment of the verifier-key bytes used for this hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_key_commitment: [u8; 32],
        /// Poseidon2 digest of the verifier-key bytes used for this hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_key_poseidon_digest: [u8; 32],
    }

    /// Canonical Poseidon2 aggregation transcript statement for Kagemusha folding.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPoseidonAggregationTranscriptStatement {
        /// Aggregation mode declared by the folded public inputs.
        pub aggregation_mode: u16,
        /// Chain id that scopes the folded token.
        pub chain_id: ChainId,
        /// Shielded asset definition id.
        pub asset: AssetDefinitionId,
        /// Root before the first folded hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub initial_root: [u8; 32],
        /// Root after the final folded hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub final_root: [u8; 32],
        /// Number of private hops folded into the compact proof.
        pub hop_count: u32,
        /// Ordered canonical hop statements.
        pub steps: Vec<KagemushaPoseidonAggregationStepStatement>,
    }

    /// One private hop folded into a compact Kagemusha payment token.
    ///
    /// These steps are prover/wallet witness material. The chain-visible compact token exposes
    /// only [`KagemushaFoldedPublicInputs`] plus the transparent folded proof. The folded
    /// transcript binds the proof payload, the public input statement that was verified for that
    /// payload, and the verifier-key identity/commitment that was used to verify that payload.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaFoldStep {
        /// Recent shielded Merkle root before this private hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root_before: [u8; 32],
        /// Input nullifiers consumed by this private hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub input_nullifiers: Vec<[u8; 32]>,
        /// Output note commitments created by this private hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub output_commitments: Vec<[u8; 32]>,
        /// Shielded Merkle root after this private hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root_after: [u8; 32],
        /// Domain-separated hash of the transparent per-hop proof payload.
        pub proof_hash: Hash,
        /// Poseidon2 digest of the per-hop proof public input statement.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub proof_public_inputs_digest: [u8; 32],
        /// Verifier key identifier used to verify the per-hop proof.
        pub verifier_key_id: VerifyingKeyId,
        /// Commitment of the verifier-key bytes used to verify the per-hop proof.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_key_commitment: [u8; 32],
        /// Poseidon2 digest of the verifier-key bytes used to verify the per-hop proof.
        ///
        /// This is redundant with [`Self::verifier_key_commitment`] for host checks, but gives
        /// future recursive verifier circuits a hash-friendly public verifier-key binding without
        /// relying on the host hash function inside the circuit.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_key_poseidon_digest: [u8; 32],
    }

    /// One private hop proof plus the verifier key needed for checked compact-token proving.
    ///
    /// This is wallet/prover input material, not a chain-visible token field. Bridge callers use
    /// it to make the prover verify each hop proof before deriving [`KagemushaFoldedPublicInputs`].
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaVerifiedFoldStep {
        /// Recent shielded Merkle root before this private hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root_before: [u8; 32],
        /// Input nullifiers consumed by this private hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub input_nullifiers: Vec<[u8; 32]>,
        /// Output note commitments created by this private hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub output_commitments: Vec<[u8; 32]>,
        /// Shielded Merkle root after this private hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root_after: [u8; 32],
        /// Transparent proof attachment that must verify before this hop can be folded.
        pub attachment: ProofAttachment,
        /// Verifier key used to verify [`Self::attachment`].
        pub verifier_key: VerifyingKeyBox,
    }

    /// Checked Kagemusha compact-token proving input.
    ///
    /// Provers and mobile bridges should prefer this bundle over prebuilt folded public inputs.
    /// It lets the prover verify every private hop proof, bind the actual verifier-key bytes, and
    /// only then emit the compact folded token.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaVerifiedFoldBundle {
        /// Chain id that scopes the folded token.
        pub chain_id: ChainId,
        /// Shielded asset definition id.
        pub asset: AssetDefinitionId,
        /// Ordered private hop proofs to verify and fold.
        pub steps: Vec<KagemushaVerifiedFoldStep>,
    }

    /// Verifier registry record supplied for one checked Kagemusha private hop.
    ///
    /// This is the serializable bridge/wallet form of the WSV lookup result:
    /// `id` is the registry key referenced by a hop proof attachment, and
    /// `record` is the governance-managed verifier metadata for that key.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaVerifiedFoldVerifierRecord {
        /// Verifier-key id referenced by a hop proof attachment.
        pub id: VerifyingKeyId,
        /// Governance-managed verifier metadata for [`Self::id`].
        pub record: VerifyingKeyRecord,
    }

    /// Checked Kagemusha compact-token proving input with verifier records.
    ///
    /// Mobile bridges and WSV-backed prover services should prefer this bundle
    /// when they can fetch verifier records. It lets the prover enforce active
    /// verifier metadata before deriving folded public inputs.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaVerifiedFoldRecordBundle {
        /// Private hop proofs to verify and fold.
        pub bundle: KagemushaVerifiedFoldBundle,
        /// Verifier records referenced by the private hop proofs.
        pub verifier_records: Vec<KagemushaVerifiedFoldVerifierRecord>,
    }

    /// Chain-verifiable public inputs for one compact, folded Kagemusha token.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaFoldedPublicInputs {
        /// Domain separator for folded public inputs.
        pub domain: String,
        /// Aggregation mode proved by the folded circuit.
        ///
        /// `1` is the current checked pre-fold mode, where wallet/prover code verifies each
        /// private hop before building the compact folded transcript. Future recursive modes must
        /// use a new supported value and verifier circuit.
        pub aggregation_mode: u16,
        /// Chain id that scopes the folded token.
        pub chain_id: ChainId,
        /// Shielded asset definition id.
        pub asset: AssetDefinitionId,
        /// Root before the first folded hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub initial_root: [u8; 32],
        /// Root after the final folded hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub final_root: [u8; 32],
        /// Number of private hops folded into the compact proof.
        pub hop_count: u32,
        /// Canonical digest of all folded input nullifiers.
        pub nullifier_digest: Hash,
        /// Canonical digest of all folded output commitments.
        pub output_commitment_digest: Hash,
        /// Canonical digest of the ordered folded-hop transcript.
        pub fold_digest: Hash,
        /// Poseidon2 digest of the ordered folded-hop aggregation transcript.
        ///
        /// This is kept separate from [`Self::fold_digest`], which preserves the ordinary Iroha
        /// hash commitment used by existing host code. Recursive Kagemusha circuits should use
        /// this field as their hash-friendly public accumulator.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub aggregation_transcript_digest: [u8; 32],
    }

    /// Verifier-key-backed transparent proof for a compact folded Kagemusha token.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaFoldedProof {
        /// Stable verifier key identifier selected by the operator and stored in WSV.
        pub verifier_key_id: VerifyingKeyId,
        /// Public input commitment hash.
        pub public_inputs_hash: Hash,
        /// Compact folded proof payload encoded as a transparent `OpenVerifyEnvelope`.
        pub proof: ProofBox,
    }

    /// Compact multi-hop Kagemusha payment token.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaCompactPaymentToken {
        /// Chain-visible folded public inputs.
        pub public_inputs: KagemushaFoldedPublicInputs,
        /// Transparent folded proof bound to `public_inputs`.
        pub folded_proof: KagemushaFoldedProof,
    }
}

/// Origin of a wallet-derived Offline Note note commitment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum OfflineNoteCommitmentOrigin {
    /// Note created by an issuer load operation.
    IssuerLoad(OfflineNoteIssuerLoadOrigin),
    /// Note created as an output of an offline peer-to-peer payment token.
    P2pOutput(OfflineNoteP2pOutputOrigin),
}

const OFFLINE_NOTE_KEY_CERTIFICATE_PAYLOAD_DOMAIN: &str =
    "iroha:offline-note:key-certificate-payload";
const OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN: &str = "iroha:offline-note:issued-claim";
const OFFLINE_NOTE_REDEEM_PUBLIC_INPUTS_DOMAIN: &str = "iroha:offline-note:redeem-public-inputs";
const OFFLINE_NOTE_AUDIT_PUBLIC_INPUTS_DOMAIN: &str = "iroha:offline-note:audit-public-inputs";
const KAGEMUSHA_FOLD_STEP_DIGEST_DOMAIN: &str = "iroha:kagemusha:v1:fold-step";
const KAGEMUSHA_FOLD_NULLIFIER_DIGEST_DOMAIN: &str = "iroha:kagemusha:v1:nullifiers";
const KAGEMUSHA_FOLD_OUTPUT_DIGEST_DOMAIN: &str = "iroha:kagemusha:v1:outputs";
const KAGEMUSHA_FOLD_TRANSCRIPT_DIGEST_DOMAIN: &str = "iroha:kagemusha:v1:fold-transcript";
/// Canonical public-input schema descriptor for Offline recursive note proofs.
pub const OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"offline_note_recursive","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","proof_mode","input_count","output_count","input_amount_sum","output_amount_sum","input_nullifier_sum_limb0","output_commitment_sum_limb0","key_certificate_payload_hash_limb0","source_or_token_limb0","input_claim_hash_sum_limb0","output_claim_hash_sum_limb0","reserved_zero"]}"#;
/// Canonical public-input schema descriptor for Kagemusha folded proofs.
pub const KAGEMUSHA_FOLDED_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"kagemusha_folded_v1","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","aggregation_mode","hop_count","initial_root_limb0","initial_root_limb1","initial_root_limb2","initial_root_limb3","final_root_limb0","final_root_limb1","final_root_limb2","final_root_limb3","nullifier_digest_limb0","nullifier_digest_limb1","nullifier_digest_limb2","nullifier_digest_limb3","output_commitment_digest_limb0","output_commitment_digest_limb1","output_commitment_digest_limb2","output_commitment_digest_limb3","fold_digest_limb0","fold_digest_limb1","fold_digest_limb2","fold_digest_limb3","aggregation_transcript_digest_limb0","aggregation_transcript_digest_limb1","aggregation_transcript_digest_limb2","aggregation_transcript_digest_limb3"]}"#;

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaFoldStepDigestPreimage {
    domain: String,
    hop_index: u32,
    root_before: [u8; Hash::LENGTH],
    input_nullifiers: Vec<[u8; Hash::LENGTH]>,
    output_commitments: Vec<[u8; Hash::LENGTH]>,
    root_after: [u8; Hash::LENGTH],
    proof_hash: Hash,
    proof_public_inputs_digest: [u8; Hash::LENGTH],
    verifier_key_id: VerifyingKeyId,
    verifier_key_commitment: [u8; Hash::LENGTH],
    verifier_key_poseidon_digest: [u8; Hash::LENGTH],
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaProofPublicInputsDigestPreimage {
    domain: String,
    statement: KagemushaProofPublicInputsStatement,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaVerifierKeyDigestPreimage {
    domain: String,
    backend: String,
    verifier_key_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaFoldListDigestPreimage {
    domain: String,
    values: Vec<[u8; Hash::LENGTH]>,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaFoldTranscriptDigestPreimage {
    domain: String,
    chain_id: ChainId,
    asset: AssetDefinitionId,
    step_digests: Vec<Hash>,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaPoseidonAggregationTranscriptPreimage {
    domain: String,
    statement: KagemushaPoseidonAggregationTranscriptStatement,
}

/// Return the registry schema hash required for Offline recursive note verifiers.
#[must_use]
pub fn offline_note_recursive_public_inputs_schema_hash() -> [u8; Hash::LENGTH] {
    Hash::new(OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA).into()
}

/// Return the registry schema hash required for Kagemusha folded proof verifiers.
#[must_use]
pub fn kagemusha_folded_public_inputs_schema_hash() -> [u8; Hash::LENGTH] {
    Hash::new(KAGEMUSHA_FOLDED_PUBLIC_INPUTS_SCHEMA).into()
}

impl From<&OfflineNoteKeyCertificate> for OfflineNoteKeyCertificatePayload {
    fn from(certificate: &OfflineNoteKeyCertificate) -> Self {
        Self {
            domain: OFFLINE_NOTE_KEY_CERTIFICATE_PAYLOAD_DOMAIN.to_owned(),
            version: certificate.version,
            platform: certificate.platform.clone(),
            key_id: certificate.key_id.clone(),
            device_id: certificate.device_id.clone(),
            account_id: certificate.account_id.clone(),
            public_key: certificate.public_key.clone(),
            assertion_scheme: certificate.assertion_scheme.clone(),
            assertion_key_algorithm: certificate.assertion_key_algorithm.clone(),
            assertion_public_key: certificate.assertion_public_key.clone(),
            assertion_usage_count_limit: certificate.assertion_usage_count_limit,
            one_use: certificate.one_use,
        }
    }
}

impl OfflineNoteKeyCertificate {
    /// Canonical payload bytes signed by the Offline certificate issuer.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        let payload = OfflineNoteKeyCertificatePayload::from(self);
        to_bytes(&payload)
    }

    /// Deterministic hash of the canonical certificate payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn payload_hash(&self) -> Result<Hash, norito::Error> {
        self.signing_bytes().map(Hash::new)
    }
}

impl OfflineNoteIssuedClaim {
    /// Build the claim recorded when an Offline note is issued.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_issue(issue: &OfflineNoteIssue) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN.to_owned(),
            note_commitment: issue.note_commitment,
            key_certificate_payload_hash: issue.key_certificate.payload_hash()?,
            asset: issue.asset.clone(),
            amount: issue.amount.clone(),
        })
    }

    /// Build the claim expected when an Offline note is redeemed.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_redemption(redemption: &OfflineNoteRedeem) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN.to_owned(),
            note_commitment: redemption.source_note_commitment,
            key_certificate_payload_hash: redemption.sender_key_certificate.payload_hash()?,
            asset: redemption.asset.clone(),
            amount: redemption.amount.clone(),
        })
    }

    /// Build the claim recorded when an Offline audited output is accepted.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_audit_output(output: &OfflineNoteAuditOutputClaim) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN.to_owned(),
            note_commitment: output.note_commitment,
            key_certificate_payload_hash: output.key_certificate.payload_hash()?,
            asset: output.asset.clone(),
            amount: output.amount.clone(),
        })
    }

    /// Deterministic hash of the issued-note claim.
    ///
    /// # Errors
    ///
    /// Returns an error when the claim cannot be serialized with Norito.
    pub fn claim_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineNoteRedeemPublicInputs {
    /// Build the public inputs committed by an Offline redemption proof.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_redemption(redemption: &OfflineNoteRedeem) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_REDEEM_PUBLIC_INPUTS_DOMAIN.to_owned(),
            source_note_commitment: redemption.source_note_commitment,
            input_nullifiers: redemption.input_nullifiers.clone(),
            key_certificate_payload_hash: redemption.sender_key_certificate.payload_hash()?,
            recipient: redemption.recipient.clone(),
            asset: redemption.asset.clone(),
            amount: redemption.amount.clone(),
        })
    }

    /// Deterministic hash of the redemption public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public inputs cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineNoteAuditPublicInputs {
    /// Build the public inputs committed by an Offline optional audit proof.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_audit(audit: &OfflineNoteAuditBundle) -> Result<Self, norito::Error> {
        let output_claims = audit
            .output_claims
            .iter()
            .map(OfflineNoteIssuedClaim::from_audit_output)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            domain: OFFLINE_NOTE_AUDIT_PUBLIC_INPUTS_DOMAIN.to_owned(),
            token_id: audit.token_id,
            key_certificate_payload_hash: audit.sender_key_certificate.payload_hash()?,
            input_nullifiers: audit.input_nullifiers.clone(),
            input_claims: audit.input_claims.clone(),
            output_commitments: audit.output_commitments.clone(),
            output_claims,
        })
    }

    /// Deterministic hash of the audit public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public inputs cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineNoteAuditBundle {
    /// Deterministic hash that the optional audit proof must expose as public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public-input payload cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        OfflineNoteAuditPublicInputs::from_audit(self)?.public_inputs_hash()
    }
}

impl OfflineNoteRedeem {
    /// Deterministic hash that the recursive proof must expose as public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public-input payload cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        OfflineNoteRedeemPublicInputs::from_redemption(self)?.public_inputs_hash()
    }
}

fn kagemusha_hash_preimage<T: Encode>(value: &T) -> Result<Hash, KagemushaFoldError> {
    Ok(Hash::new(to_bytes(value)?))
}

fn kagemusha_poseidon_preimage<T: Encode>(
    value: &T,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    let bytes = to_bytes(value)?;
    Ok(iroha_zkp_halo2::poseidon::hash_bytes(&bytes))
}

/// Return the canonical Poseidon2 digest for a Kagemusha proof public-input statement.
///
/// The digest is domain-separated from the folded-hop transcript and commits to the transparent
/// envelope metadata plus the exact backend-native public instance columns verified for one hop.
/// Kagemusha fold statements must carry a non-zero verifier-key hash and empty auxiliary bytes.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the statement is non-canonical or cannot be encoded with
/// Norito.
pub fn kagemusha_proof_public_inputs_statement_digest(
    statement: &KagemushaProofPublicInputsStatement,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    let Some(expected_tag) = kagemusha_backend_tag(&statement.proof_backend) else {
        return Err(KagemushaFoldError::UnsupportedProofBackend {
            backend: statement.proof_backend.clone(),
        });
    };
    if statement.envelope_backend != expected_tag {
        return Err(KagemushaFoldError::ProofStatementBackendTagMismatch {
            proof_backend: statement.proof_backend.clone(),
            envelope_backend: statement.envelope_backend.clone(),
        });
    }
    if statement.vk_hash == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroProofStatementVerifierKeyHash);
    }
    if statement.circuit_id.trim().is_empty() {
        return Err(KagemushaFoldError::EmptyProofStatementCircuitId);
    }
    if statement.public_inputs_schema.is_empty() {
        return Err(KagemushaFoldError::EmptyProofStatementPublicInputsSchema);
    }
    if statement.instance_columns.is_empty() {
        return Err(KagemushaFoldError::EmptyProofStatementInstanceColumns);
    }
    if let Some(column_index) = statement
        .instance_columns
        .iter()
        .position(std::vec::Vec::is_empty)
    {
        return Err(KagemushaFoldError::EmptyProofStatementInstanceColumn { column_index });
    }
    if !statement.envelope_aux.is_empty() {
        return Err(
            KagemushaFoldError::NonCanonicalProofStatementAuxiliaryBytes {
                actual: statement.envelope_aux.len(),
            },
        );
    }
    kagemusha_poseidon_preimage(&KagemushaProofPublicInputsDigestPreimage {
        domain: KAGEMUSHA_PROOF_PUBLIC_INPUTS_DIGEST_DOMAIN.to_owned(),
        statement: statement.clone(),
    })
}

/// Return the canonical Poseidon2 digest for a Kagemusha verifier key.
///
/// The digest is domain-separated from folded-hop and proof-statement digests
/// and commits to the backend label plus the exact verifier-key bytes used for
/// one hop proof verification.
///
/// # Errors
///
/// Returns [`KagemushaFoldError::Encode`] when the digest preimage cannot be
/// encoded with Norito.
pub fn kagemusha_verifier_key_poseidon_digest(
    backend: impl Into<String>,
    verifier_key_bytes: &[u8],
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    let backend = backend.into();
    if !is_supported_kagemusha_proof_backend(&backend) {
        return Err(KagemushaFoldError::UnsupportedProofBackend { backend });
    }
    if verifier_key_bytes.is_empty() {
        return Err(KagemushaFoldError::EmptyVerifierKeyBytes { backend });
    }
    kagemusha_poseidon_preimage(&KagemushaVerifierKeyDigestPreimage {
        domain: KAGEMUSHA_VERIFIER_KEY_DIGEST_DOMAIN.to_owned(),
        backend,
        verifier_key_bytes: verifier_key_bytes.to_vec(),
    })
}

/// Return the canonical Poseidon2 digest for a Kagemusha aggregation transcript statement.
///
/// This is the hash-friendly public accumulator that future recursive verifier
/// circuits should recompute from their private per-hop witness. It is
/// domain-separated from proof-statement, verifier-key, and host-side folded-hop
/// hashes.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the statement is non-canonical or cannot
/// be encoded with Norito.
pub fn kagemusha_poseidon_aggregation_transcript_digest(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    validate_kagemusha_aggregation_transcript_statement(statement)?;
    kagemusha_poseidon_preimage(&KagemushaPoseidonAggregationTranscriptPreimage {
        domain: KAGEMUSHA_POSEIDON_AGGREGATION_TRANSCRIPT_DOMAIN.to_owned(),
        statement: statement.clone(),
    })
}

fn kagemusha_list_digest(
    domain: &str,
    values: Vec<[u8; Hash::LENGTH]>,
) -> Result<Hash, KagemushaFoldError> {
    kagemusha_hash_preimage(&KagemushaFoldListDigestPreimage {
        domain: domain.to_owned(),
        values,
    })
}

fn validate_kagemusha_step_shape_and_sets(
    hop_index: usize,
    input_nullifiers: &[[u8; Hash::LENGTH]],
    output_commitments: &[[u8; Hash::LENGTH]],
) -> Result<(), KagemushaFoldError> {
    if input_nullifiers.is_empty()
        || input_nullifiers.len() > KAGEMUSHA_FOLD_STEP_MAX_INPUTS
        || output_commitments.is_empty()
        || output_commitments.len() > KAGEMUSHA_FOLD_STEP_MAX_OUTPUTS
    {
        return Err(KagemushaFoldError::InvalidStepShape {
            hop_index,
            input_count: input_nullifiers.len(),
            output_count: output_commitments.len(),
        });
    }
    if input_nullifiers
        .iter()
        .any(|nullifier| *nullifier == [0u8; Hash::LENGTH])
    {
        return Err(KagemushaFoldError::ZeroInputNullifier { hop_index });
    }
    if output_commitments
        .iter()
        .any(|commitment| *commitment == [0u8; Hash::LENGTH])
    {
        return Err(KagemushaFoldError::ZeroOutputCommitment { hop_index });
    }
    Ok(())
}

fn validate_kagemusha_canonical_set_order(
    hop_index: usize,
    input_nullifiers: &[[u8; Hash::LENGTH]],
    output_commitments: &[[u8; Hash::LENGTH]],
) -> Result<(), KagemushaFoldError> {
    if input_nullifiers.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(KagemushaFoldError::NonCanonicalInputNullifierOrder { hop_index });
    }
    if output_commitments.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(KagemushaFoldError::NonCanonicalOutputCommitmentOrder { hop_index });
    }
    Ok(())
}

fn validate_kagemusha_step_digest_bindings(
    hop_index: usize,
    proof_public_inputs_digest: [u8; Hash::LENGTH],
    verifier_key_commitment: [u8; Hash::LENGTH],
    verifier_key_poseidon_digest: [u8; Hash::LENGTH],
) -> Result<(), KagemushaFoldError> {
    if proof_public_inputs_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroProofPublicInputsDigest { hop_index });
    }
    if verifier_key_commitment == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroVerifierKeyCommitment { hop_index });
    }
    if verifier_key_poseidon_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroVerifierKeyPoseidonDigest { hop_index });
    }
    Ok(())
}

fn validate_kagemusha_fold_root(
    field: &'static str,
    root: [u8; Hash::LENGTH],
) -> Result<(), KagemushaFoldError> {
    if root == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroFoldedRoot { field });
    }
    Ok(())
}

fn validate_kagemusha_root_transition(
    hop_index: usize,
    root_before: [u8; Hash::LENGTH],
    root_after: [u8; Hash::LENGTH],
) -> Result<(), KagemushaFoldError> {
    if root_before == root_after {
        return Err(KagemushaFoldError::UnchangedFoldedRootTransition { hop_index });
    }
    Ok(())
}

fn validate_kagemusha_verifier_key_id(
    hop_index: usize,
    verifier_key_id: &VerifyingKeyId,
) -> Result<(), KagemushaFoldError> {
    if verifier_key_id.name.trim().is_empty() {
        return Err(KagemushaFoldError::EmptyVerifierKeyIdName { hop_index });
    }
    if !is_supported_kagemusha_proof_backend(&verifier_key_id.backend) {
        return Err(KagemushaFoldError::UnsupportedProofBackend {
            backend: verifier_key_id.backend.clone(),
        });
    }
    Ok(())
}

fn validate_kagemusha_aggregation_transcript_statement(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<(), KagemushaFoldError> {
    if !is_supported_kagemusha_aggregation_mode(statement.aggregation_mode) {
        return Err(KagemushaFoldError::UnsupportedAggregationMode {
            expected: KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1,
            actual: statement.aggregation_mode,
            reason: unsupported_kagemusha_aggregation_mode_reason(statement.aggregation_mode),
        });
    }
    if statement.steps.is_empty() {
        return Err(KagemushaFoldError::Empty);
    }
    if statement.steps.len() > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
        return Err(KagemushaFoldError::TooManyHops {
            max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
            actual: statement.steps.len(),
        });
    }
    if usize::try_from(statement.hop_count).ok() != Some(statement.steps.len()) {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: statement.steps.len(),
            actual: statement.hop_count,
        });
    }

    let first = statement.steps.first().expect("validated non-empty steps");
    validate_kagemusha_fold_root("initial_root", statement.initial_root)?;
    validate_kagemusha_fold_root("final_root", statement.final_root)?;
    if statement.initial_root == statement.final_root {
        return Err(KagemushaFoldError::UnchangedFoldedPublicRoots);
    }
    if statement.initial_root != first.root_before {
        return Err(KagemushaFoldError::InitialRootMismatch {
            expected: first.root_before,
            actual: statement.initial_root,
        });
    }

    let mut expected_root = statement.initial_root;
    let mut all_inputs = std::collections::BTreeSet::new();
    let mut all_outputs = std::collections::BTreeSet::new();
    for (hop_index, step) in statement.steps.iter().enumerate() {
        if step.hop_index != u32::try_from(hop_index).expect("hop count is bounded to u32") {
            return Err(KagemushaFoldError::HopIndexMismatch {
                expected: hop_index,
                actual: step.hop_index,
            });
        }
        validate_kagemusha_fold_root("root_before", step.root_before)?;
        validate_kagemusha_fold_root("root_after", step.root_after)?;
        validate_kagemusha_root_transition(hop_index, step.root_before, step.root_after)?;
        validate_kagemusha_verifier_key_id(hop_index, &step.verifier_key_id)?;
        validate_kagemusha_step_shape_and_sets(
            hop_index,
            &step.input_nullifiers,
            &step.output_commitments,
        )?;
        validate_kagemusha_canonical_set_order(
            hop_index,
            &step.input_nullifiers,
            &step.output_commitments,
        )?;
        validate_kagemusha_step_digest_bindings(
            hop_index,
            step.proof_public_inputs_digest,
            step.verifier_key_commitment,
            step.verifier_key_poseidon_digest,
        )?;
        if step.root_before != expected_root {
            return Err(KagemushaFoldError::RootDiscontinuity {
                hop_index,
                expected: expected_root,
                actual: step.root_before,
            });
        }
        for nullifier in &step.input_nullifiers {
            if !all_inputs.insert(*nullifier) {
                return Err(KagemushaFoldError::DuplicateInputNullifier { hop_index });
            }
        }
        for commitment in &step.output_commitments {
            if !all_outputs.insert(*commitment) {
                return Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index });
            }
        }
        expected_root = step.root_after;
    }

    if statement.final_root != expected_root {
        return Err(KagemushaFoldError::FinalRootMismatch {
            expected: expected_root,
            actual: statement.final_root,
        });
    }
    Ok(())
}

struct KagemushaCanonicalFoldParts {
    nullifier_digest: Hash,
    output_commitment_digest: Hash,
    fold_digest: Hash,
    aggregation_statement: KagemushaPoseidonAggregationTranscriptStatement,
}

fn kagemusha_canonical_fold_parts(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    steps: &[KagemushaFoldStep],
) -> Result<KagemushaCanonicalFoldParts, KagemushaFoldError> {
    if steps.is_empty() {
        return Err(KagemushaFoldError::Empty);
    }
    if steps.len() > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
        return Err(KagemushaFoldError::TooManyHops {
            max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
            actual: steps.len(),
        });
    }

    let initial_root = steps[0].root_before;
    validate_kagemusha_fold_root("initial_root", initial_root)?;
    let mut expected_root = initial_root;
    let mut all_inputs = Vec::new();
    let mut all_outputs = Vec::new();
    let mut step_digests = Vec::with_capacity(steps.len());
    let mut aggregation_steps = Vec::with_capacity(steps.len());
    let mut seen_inputs = std::collections::BTreeSet::new();
    let mut seen_outputs = std::collections::BTreeSet::new();

    for (hop_index, step) in steps.iter().enumerate() {
        validate_kagemusha_verifier_key_id(hop_index, &step.verifier_key_id)?;
        validate_kagemusha_step_shape_and_sets(
            hop_index,
            &step.input_nullifiers,
            &step.output_commitments,
        )?;
        validate_kagemusha_step_digest_bindings(
            hop_index,
            step.proof_public_inputs_digest,
            step.verifier_key_commitment,
            step.verifier_key_poseidon_digest,
        )?;
        validate_kagemusha_fold_root("root_before", step.root_before)?;
        validate_kagemusha_fold_root("root_after", step.root_after)?;
        validate_kagemusha_root_transition(hop_index, step.root_before, step.root_after)?;
        if step.root_before != expected_root {
            return Err(KagemushaFoldError::RootDiscontinuity {
                hop_index,
                expected: expected_root,
                actual: step.root_before,
            });
        }

        let mut input_nullifiers = step.input_nullifiers.clone();
        input_nullifiers.sort_unstable();
        for input in &input_nullifiers {
            if !seen_inputs.insert(*input) {
                return Err(KagemushaFoldError::DuplicateInputNullifier { hop_index });
            }
        }

        let mut output_commitments = step.output_commitments.clone();
        output_commitments.sort_unstable();
        for output in &output_commitments {
            if !seen_outputs.insert(*output) {
                return Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index });
            }
        }

        let step_digest = kagemusha_hash_preimage(&KagemushaFoldStepDigestPreimage {
            domain: KAGEMUSHA_FOLD_STEP_DIGEST_DOMAIN.to_owned(),
            hop_index: u32::try_from(hop_index).expect("hop count is bounded to u32"),
            root_before: step.root_before,
            input_nullifiers: input_nullifiers.clone(),
            output_commitments: output_commitments.clone(),
            root_after: step.root_after,
            proof_hash: step.proof_hash,
            proof_public_inputs_digest: step.proof_public_inputs_digest,
            verifier_key_id: step.verifier_key_id.clone(),
            verifier_key_commitment: step.verifier_key_commitment,
            verifier_key_poseidon_digest: step.verifier_key_poseidon_digest,
        })?;
        aggregation_steps.push(KagemushaPoseidonAggregationStepStatement {
            hop_index: u32::try_from(hop_index).expect("hop count is bounded to u32"),
            root_before: step.root_before,
            input_nullifiers: input_nullifiers.clone(),
            output_commitments: output_commitments.clone(),
            root_after: step.root_after,
            proof_hash: step.proof_hash,
            proof_public_inputs_digest: step.proof_public_inputs_digest,
            verifier_key_id: step.verifier_key_id.clone(),
            verifier_key_commitment: step.verifier_key_commitment,
            verifier_key_poseidon_digest: step.verifier_key_poseidon_digest,
        });
        step_digests.push(step_digest);
        all_inputs.extend(input_nullifiers);
        all_outputs.extend(output_commitments);
        expected_root = step.root_after;
    }

    let nullifier_digest =
        kagemusha_list_digest(KAGEMUSHA_FOLD_NULLIFIER_DIGEST_DOMAIN, all_inputs)?;
    let output_commitment_digest =
        kagemusha_list_digest(KAGEMUSHA_FOLD_OUTPUT_DIGEST_DOMAIN, all_outputs)?;
    let fold_digest = kagemusha_hash_preimage(&KagemushaFoldTranscriptDigestPreimage {
        domain: KAGEMUSHA_FOLD_TRANSCRIPT_DIGEST_DOMAIN.to_owned(),
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        step_digests,
    })?;
    if initial_root == expected_root {
        return Err(KagemushaFoldError::UnchangedFoldedPublicRoots);
    }
    let hop_count = u32::try_from(steps.len()).expect("hop count is bounded to u32");
    Ok(KagemushaCanonicalFoldParts {
        nullifier_digest,
        output_commitment_digest,
        fold_digest,
        aggregation_statement: KagemushaPoseidonAggregationTranscriptStatement {
            aggregation_mode: KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1,
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            initial_root,
            final_root: expected_root,
            hop_count,
            steps: aggregation_steps,
        },
    })
}

/// Build the canonical Poseidon2 aggregation transcript statement for Kagemusha folding.
///
/// The builder performs the same shape checks, per-hop canonicalization, duplicate detection, and
/// root-continuity checks as [`kagemusha_folded_public_inputs`]. Future recursive verifier
/// circuits and SDKs should use this function to derive the exact statement that
/// [`kagemusha_poseidon_aggregation_transcript_digest`] hashes.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the witness is empty, oversized, malformed, duplicate, root
/// discontinuous, or cannot be encoded with Norito.
pub fn kagemusha_poseidon_aggregation_transcript_statement(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    steps: &[KagemushaFoldStep],
) -> Result<KagemushaPoseidonAggregationTranscriptStatement, KagemushaFoldError> {
    Ok(kagemusha_canonical_fold_parts(chain_id, asset, steps)?.aggregation_statement)
}

struct KagemushaFoldDigestParts {
    nullifier_digest: Hash,
    output_commitment_digest: Hash,
    fold_digest: Hash,
}

fn kagemusha_fold_digest_parts_from_aggregation_statement(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<KagemushaFoldDigestParts, KagemushaFoldError> {
    validate_kagemusha_aggregation_transcript_statement(statement)?;

    let mut all_inputs = Vec::new();
    let mut all_outputs = Vec::new();
    let mut step_digests = Vec::with_capacity(statement.steps.len());

    for step in &statement.steps {
        let step_digest = kagemusha_hash_preimage(&KagemushaFoldStepDigestPreimage {
            domain: KAGEMUSHA_FOLD_STEP_DIGEST_DOMAIN.to_owned(),
            hop_index: step.hop_index,
            root_before: step.root_before,
            input_nullifiers: step.input_nullifiers.clone(),
            output_commitments: step.output_commitments.clone(),
            root_after: step.root_after,
            proof_hash: step.proof_hash,
            proof_public_inputs_digest: step.proof_public_inputs_digest,
            verifier_key_id: step.verifier_key_id.clone(),
            verifier_key_commitment: step.verifier_key_commitment,
            verifier_key_poseidon_digest: step.verifier_key_poseidon_digest,
        })?;
        step_digests.push(step_digest);
        all_inputs.extend(step.input_nullifiers.iter().copied());
        all_outputs.extend(step.output_commitments.iter().copied());
    }

    Ok(KagemushaFoldDigestParts {
        nullifier_digest: kagemusha_list_digest(
            KAGEMUSHA_FOLD_NULLIFIER_DIGEST_DOMAIN,
            all_inputs,
        )?,
        output_commitment_digest: kagemusha_list_digest(
            KAGEMUSHA_FOLD_OUTPUT_DIGEST_DOMAIN,
            all_outputs,
        )?,
        fold_digest: kagemusha_hash_preimage(&KagemushaFoldTranscriptDigestPreimage {
            domain: KAGEMUSHA_FOLD_TRANSCRIPT_DIGEST_DOMAIN.to_owned(),
            chain_id: statement.chain_id.clone(),
            asset: statement.asset.clone(),
            step_digests,
        })?,
    })
}

/// Project a canonical aggregation transcript statement into folded public inputs.
///
/// Future recursive verifier circuits should produce the same public projection
/// from their private hop witness before proving `kagemusha-folded-v1`.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the statement is non-canonical or cannot
/// be encoded with Norito.
pub fn kagemusha_folded_public_inputs_from_aggregation_statement(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<KagemushaFoldedPublicInputs, KagemushaFoldError> {
    let parts = kagemusha_fold_digest_parts_from_aggregation_statement(statement)?;
    let aggregation_transcript_digest =
        kagemusha_poseidon_aggregation_transcript_digest(statement)?;

    Ok(KagemushaFoldedPublicInputs {
        domain: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN.to_owned(),
        aggregation_mode: statement.aggregation_mode,
        chain_id: statement.chain_id.clone(),
        asset: statement.asset.clone(),
        initial_root: statement.initial_root,
        final_root: statement.final_root,
        hop_count: statement.hop_count,
        nullifier_digest: parts.nullifier_digest,
        output_commitment_digest: parts.output_commitment_digest,
        fold_digest: parts.fold_digest,
        aggregation_transcript_digest,
    })
}

/// Validate that folded public inputs are the canonical projection of an aggregation transcript.
///
/// This is the host-side equivalent of the public projection that recursive
/// Kagemusha verifier circuits must enforce in-circuit.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when either side is non-canonical or when any
/// folded public-input field differs from the aggregation transcript projection.
pub fn kagemusha_validate_folded_public_inputs_against_aggregation_statement(
    public_inputs: &KagemushaFoldedPublicInputs,
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<(), KagemushaFoldError> {
    public_inputs.validate_supported_context()?;
    let expected = kagemusha_folded_public_inputs_from_aggregation_statement(statement)?;

    macro_rules! ensure_field {
        ($field:ident) => {
            if public_inputs.$field != expected.$field {
                return Err(KagemushaFoldError::FoldedPublicInputTranscriptMismatch {
                    field: stringify!($field),
                });
            }
        };
    }

    ensure_field!(chain_id);
    ensure_field!(asset);
    ensure_field!(aggregation_mode);
    ensure_field!(initial_root);
    ensure_field!(final_root);
    ensure_field!(hop_count);
    ensure_field!(nullifier_digest);
    ensure_field!(output_commitment_digest);
    ensure_field!(fold_digest);
    ensure_field!(aggregation_transcript_digest);

    Ok(())
}

/// Build the chain-visible public inputs for a compact folded Kagemusha token.
///
/// The builder canonicalizes nullifier and output order inside each hop because the ledger treats
/// them as sets and appends output commitments deterministically. Adjacent hops must be root
/// continuous, and nullifiers/commitments may not repeat across the folded witness.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the witness is empty, oversized, malformed, duplicate, root
/// discontinuous, or cannot be encoded with Norito.
pub fn kagemusha_folded_public_inputs(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    steps: &[KagemushaFoldStep],
) -> Result<KagemushaFoldedPublicInputs, KagemushaFoldError> {
    let parts = kagemusha_canonical_fold_parts(chain_id, asset, steps)?;

    let aggregation_transcript_digest =
        kagemusha_poseidon_aggregation_transcript_digest(&parts.aggregation_statement)?;

    Ok(KagemushaFoldedPublicInputs {
        domain: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN.to_owned(),
        aggregation_mode: KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1,
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        initial_root: parts.aggregation_statement.initial_root,
        final_root: parts.aggregation_statement.final_root,
        hop_count: parts.aggregation_statement.hop_count,
        nullifier_digest: parts.nullifier_digest,
        output_commitment_digest: parts.output_commitment_digest,
        fold_digest: parts.fold_digest,
        aggregation_transcript_digest,
    })
}

impl KagemushaFoldedPublicInputs {
    /// Validate the domain and aggregation mode supported by this release.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError::InvalidPublicInputDomain`] when the domain separator is not
    /// canonical, or [`KagemushaFoldError::UnsupportedAggregationMode`] when the folded token
    /// declares a future or unknown aggregation mode.
    pub fn validate_supported_context(&self) -> Result<(), KagemushaFoldError> {
        if self.domain != KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN {
            return Err(KagemushaFoldError::InvalidPublicInputDomain {
                expected: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN,
                actual: self.domain.clone(),
            });
        }
        if !is_supported_kagemusha_aggregation_mode(self.aggregation_mode) {
            return Err(KagemushaFoldError::UnsupportedAggregationMode {
                expected: KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1,
                actual: self.aggregation_mode,
                reason: unsupported_kagemusha_aggregation_mode_reason(self.aggregation_mode),
            });
        }
        if self.hop_count == 0 {
            return Err(KagemushaFoldError::Empty);
        }
        if usize::try_from(self.hop_count).map_or(true, |hop_count| {
            hop_count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS
        }) {
            return Err(KagemushaFoldError::TooManyHops {
                max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
                actual: usize::try_from(self.hop_count).unwrap_or(usize::MAX),
            });
        }
        validate_kagemusha_fold_root("initial_root", self.initial_root)?;
        validate_kagemusha_fold_root("final_root", self.final_root)?;
        if self.initial_root == self.final_root {
            return Err(KagemushaFoldError::UnchangedFoldedPublicRoots);
        }
        if self.aggregation_transcript_digest == [0u8; Hash::LENGTH] {
            return Err(KagemushaFoldError::ZeroFoldedPublicInputDigest {
                field: "aggregation_transcript_digest",
            });
        }
        let encoded_len = self.norito_encoded_len()?;
        if encoded_len > KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES {
            return Err(KagemushaFoldError::EncodedSizeExceeded {
                max: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES,
                actual: encoded_len,
            });
        }
        Ok(())
    }

    /// Deterministic hash that the compact folded proof must expose as public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public-input payload cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }

    /// Return the canonical Norito encoded size for folded public inputs.
    ///
    /// Wallets and QR/NFC transports can use this to enforce payload budgets
    /// before attaching backend-specific proof bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the public-input payload cannot be serialized with Norito.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

impl KagemushaCompactPaymentToken {
    /// Validate that the folded proof is bound to the canonical folded public inputs.
    ///
    /// This does not verify the proof cryptographically; it prevents accepting a compact token
    /// whose proof declares public inputs for a different folded transcript before the backend
    /// verifier runs.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError::PublicInputHashMismatch`] when the proof's declared public
    /// input hash differs from the canonical public-input hash, or [`KagemushaFoldError::Encode`]
    /// when the public inputs cannot be serialized.
    pub fn validate_public_input_binding(&self) -> Result<(), KagemushaFoldError> {
        self.public_inputs.validate_supported_context()?;
        let expected = self.public_inputs.public_inputs_hash()?;
        let actual = self.folded_proof.public_inputs_hash;
        if actual != expected {
            return Err(KagemushaFoldError::PublicInputHashMismatch { expected, actual });
        }
        Ok(())
    }

    /// Return the canonical Norito encoded size for this compact payment token.
    ///
    /// This includes the backend proof payload carried by [`KagemushaFoldedProof`].
    ///
    /// # Errors
    ///
    /// Returns an error when the token cannot be serialized with Norito.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

fn validate_offline_note_random_bytes(
    field: &'static str,
    bytes: &[u8],
) -> Result<(), OfflineNoteDerivationError> {
    if bytes.len() != Hash::LENGTH {
        return Err(OfflineNoteDerivationError::InvalidRandomBytesLength {
            field,
            expected: Hash::LENGTH,
            actual: bytes.len(),
        });
    }
    Ok(())
}

/// Derive the canonical Offline Note note commitment from a wallet preimage.
///
/// # Errors
///
/// Returns an error when `note_secret` is not exactly 32 bytes or the preimage
/// cannot be encoded with Norito.
pub fn derive_offline_note_note_commitment(
    preimage: &OfflineNoteCommitmentPreimage,
) -> Result<Hash, OfflineNoteDerivationError> {
    validate_offline_note_random_bytes("note_secret", &preimage.note_secret)?;
    let bytes = to_bytes(preimage)?;
    Ok(Hash::new(bytes))
}

/// Derive the canonical Offline Note input nullifier from a wallet preimage.
///
/// # Errors
///
/// Returns an error when `note_secret` is not exactly 32 bytes or the preimage
/// cannot be encoded with Norito.
pub fn derive_offline_note_input_nullifier(
    preimage: &OfflineNoteInputNullifierPreimage,
) -> Result<Hash, OfflineNoteDerivationError> {
    validate_offline_note_random_bytes("note_secret", &preimage.note_secret)?;
    let bytes = to_bytes(preimage)?;
    Ok(Hash::new(bytes))
}

/// Derive the canonical Offline Note payment token id from a wallet preimage.
///
/// # Errors
///
/// Returns an error when `token_nonce` is not exactly 32 bytes or the preimage
/// cannot be encoded with Norito.
pub fn derive_offline_note_payment_token_id(
    preimage: &OfflineNotePaymentTokenIdPreimage,
) -> Result<Hash, OfflineNoteDerivationError> {
    validate_offline_note_random_bytes("token_nonce", &preimage.token_nonce)?;
    let bytes = to_bytes(preimage)?;
    Ok(Hash::new(bytes))
}

#[cfg(test)]
mod offline_note_tests {
    use iroha_crypto::{Algorithm, KeyPair, PublicKey};

    use super::*;
    use crate::{asset::AssetDefinitionId, confidential::ConfidentialStatus, domain::DomainId};

    fn sample_signature(seed: u8) -> Signature {
        let mut payload = [0u8; 64];
        for (idx, byte) in payload.iter_mut().enumerate() {
            let offset = u8::try_from(idx).expect("index fits into u8");
            *byte = seed.wrapping_add(offset);
        }
        Signature::from_bytes(&payload)
    }

    fn sample_public_key(seed: u8) -> PublicKey {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        key_pair.public_key().clone()
    }

    fn sample_account(seed: u8, domain: &str) -> AccountId {
        let key = sample_public_key(seed);
        let _domain_id = DomainId::try_new(domain, "universal").expect("domain id");
        AccountId::new(key)
    }

    fn fixed_hash(label: &[u8]) -> [u8; Hash::LENGTH] {
        Hash::new(label).into()
    }

    fn kagemusha_asset(name: &str) -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            name.parse().expect("asset name"),
        )
    }

    fn kagemusha_step(
        root_before: [u8; Hash::LENGTH],
        root_after: [u8; Hash::LENGTH],
        input_seed: u8,
        output_seed: u8,
        proof_label: &'static [u8],
    ) -> KagemushaFoldStep {
        let mut proof_inputs_label = proof_label.to_vec();
        proof_inputs_label.extend_from_slice(b":public-inputs");
        KagemushaFoldStep {
            root_before,
            input_nullifiers: vec![
                [input_seed.wrapping_add(1); Hash::LENGTH],
                [input_seed; Hash::LENGTH],
            ],
            output_commitments: vec![
                [output_seed.wrapping_add(1); Hash::LENGTH],
                [output_seed; Hash::LENGTH],
            ],
            root_after,
            proof_hash: Hash::new(proof_label),
            proof_public_inputs_digest: fixed_hash(&proof_inputs_label),
            verifier_key_id: VerifyingKeyId::new("halo2/ipa", "kagemusha-hop-fixture"),
            verifier_key_commitment: fixed_hash(proof_label),
            verifier_key_poseidon_digest: kagemusha_verifier_key_poseidon_digest(
                "halo2/ipa",
                proof_label,
            )
            .expect("verifier-key digest"),
        }
    }

    #[test]
    fn offline_escrow_account_derivation_binds_chain_and_asset_definition() {
        let chain_id: ChainId = "offline-escrow-testnet".parse().expect("chain id");
        let other_chain_id: ChainId = "offline-escrow-mainnet".parse().expect("chain id");
        let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
        let definition_id = AssetDefinitionId::new(
            domain_id.clone(),
            "usd".parse().expect("asset definition name"),
        );
        let other_definition_id =
            AssetDefinitionId::new(domain_id, "eur".parse().expect("asset definition name"));

        let escrow = offline_escrow_account_id(&chain_id, &definition_id);

        assert_eq!(escrow, offline_escrow_account_id(&chain_id, &definition_id));
        assert_ne!(
            escrow,
            offline_escrow_account_id(&other_chain_id, &definition_id)
        );
        assert_ne!(
            escrow,
            offline_escrow_account_id(&chain_id, &other_definition_id)
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn offline_note_claims_and_public_inputs_bind_payload_fields() {
        let account_id = sample_account(0xD4, "offline");
        let definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "usd".parse().expect("asset name"),
        );
        let asset = AssetId::new(definition, account_id.clone());
        let note_public_key = sample_public_key(0xA8);
        let (_algorithm, note_key) = note_public_key.to_bytes();
        let certificate = OfflineNoteKeyCertificate {
            version: OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
            platform: "ios-appattest".to_owned(),
            key_id: "one-use-key".to_owned(),
            device_id: "device-1".to_owned(),
            account_id: account_id.clone(),
            public_key: note_key.to_vec(),
            assertion_scheme: "apple-appattest-counter".to_owned(),
            assertion_key_algorithm: "app-attest-p256".to_owned(),
            assertion_public_key: vec![0x04; 65],
            assertion_usage_count_limit: None,
            one_use: true,
            issuer_signature: sample_signature(0xAB),
        };
        let proof = OfflineNoteRecursiveProof {
            verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-recursive"),
            public_inputs_hash: Hash::new(b"offline-public-inputs"),
            proof: ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE]),
        };
        let note_commitment = Hash::new(b"offline-note-issued-note");
        let issue = OfflineNoteIssue {
            note_commitment,
            key_certificate: certificate.clone(),
            asset: asset.clone(),
            amount: Numeric::new(10, 0),
        };
        let mut redemption = OfflineNoteRedeem {
            source_note_commitment: note_commitment,
            input_nullifiers: vec![Hash::new(b"offline-note-nullifier")],
            sender_key_certificate: certificate.clone(),
            recipient: account_id,
            asset: asset.clone(),
            amount: Numeric::new(10, 0),
            recursive_proof: proof.clone(),
        };

        let issue_claim = OfflineNoteIssuedClaim::from_issue(&issue)
            .expect("issue claim")
            .claim_hash()
            .expect("issue claim hash");
        let redeem_claim = OfflineNoteIssuedClaim::from_redemption(&redemption)
            .expect("redemption claim")
            .claim_hash()
            .expect("redemption claim hash");
        assert_eq!(issue_claim, redeem_claim);
        let redemption_inputs = redemption
            .public_inputs_hash()
            .expect("redemption public inputs hash");
        redemption.source_note_commitment = Hash::new(b"offline-note-other-note");
        assert_ne!(
            redemption_inputs,
            redemption
                .public_inputs_hash()
                .expect("changed redemption public inputs hash")
        );
        assert_ne!(
            issue_claim,
            OfflineNoteIssuedClaim::from_redemption(&redemption)
                .expect("changed redemption claim")
                .claim_hash()
                .expect("changed redemption claim hash")
        );

        let mut audit = OfflineNoteAuditBundle {
            token_id: Hash::new(b"offline-note-audit-token"),
            sender_key_certificate: certificate.clone(),
            input_nullifiers: vec![Hash::new(b"offline-note-audit-nullifier")],
            input_claims: vec![
                OfflineNoteIssuedClaim::from_issue(&issue).expect("audit input claim"),
            ],
            output_commitments: vec![Hash::new(b"offline-note-output-note")],
            output_claims: vec![OfflineNoteAuditOutputClaim {
                note_commitment: Hash::new(b"offline-note-output-note"),
                key_certificate: certificate,
                asset,
                amount: Numeric::new(10, 0),
            }],
            recursive_proof: proof,
        };
        let audit_inputs = audit
            .public_inputs_hash()
            .expect("audit public inputs hash");
        audit.output_commitments = vec![Hash::new(b"offline-note-other-output")];
        assert_ne!(
            audit_inputs,
            audit
                .public_inputs_hash()
                .expect("changed audit public inputs hash")
        );
        audit.output_commitments = vec![Hash::new(b"offline-note-output-note")];
        audit.input_claims[0].amount = Numeric::new(9, 0);
        assert_ne!(
            audit_inputs,
            audit
                .public_inputs_hash()
                .expect("changed audit input claim public inputs hash")
        );
    }

    #[test]
    fn offline_note_wallet_derivations_bind_preimages() {
        let chain_id: ChainId = "offline-note-derivation-chain".parse().expect("chain id");
        let account_id = sample_account(0xD5, "offline");
        let definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "usd".parse().expect("asset name"),
        );
        let asset = AssetId::new(definition, account_id);
        let owner_key_certificate_payload_hash = Hash::new(b"offline-note-owner-cert");
        let note_secret = vec![0xA5; Hash::LENGTH];
        let commitment_preimage = OfflineNoteCommitmentPreimage {
            domain: OFFLINE_NOTE_NOTE_COMMITMENT_DOMAIN.to_owned(),
            chain_id: chain_id.clone(),
            owner_key_certificate_payload_hash,
            asset: asset.clone(),
            amount: Numeric::new(42, 0),
            note_secret: note_secret.clone(),
            origin: OfflineNoteCommitmentOrigin::IssuerLoad(OfflineNoteIssuerLoadOrigin {
                operation_id: "operation-1".to_owned(),
                lineage_id: "lineage-1".to_owned(),
                local_revision: 7,
            }),
        };
        let commitment =
            derive_offline_note_note_commitment(&commitment_preimage).expect("commitment");

        assert_eq!(
            commitment,
            derive_offline_note_note_commitment(&commitment_preimage).expect("repeat commitment")
        );
        let mut changed_commitment_preimage = commitment_preimage.clone();
        changed_commitment_preimage.origin =
            OfflineNoteCommitmentOrigin::P2pOutput(OfflineNoteP2pOutputOrigin {
                payment_request_id: "payment-request-1".to_owned(),
                output_index: 0,
            });
        assert_ne!(
            commitment,
            derive_offline_note_note_commitment(&changed_commitment_preimage)
                .expect("changed origin commitment")
        );

        let nullifier_preimage = OfflineNoteInputNullifierPreimage {
            domain: OFFLINE_NOTE_INPUT_NULLIFIER_DOMAIN.to_owned(),
            chain_id: chain_id.clone(),
            source_note_commitment: commitment,
            owner_key_certificate_payload_hash,
            note_secret: note_secret.clone(),
        };
        let nullifier =
            derive_offline_note_input_nullifier(&nullifier_preimage).expect("nullifier");
        assert_eq!(
            nullifier,
            derive_offline_note_input_nullifier(&nullifier_preimage).expect("repeat nullifier")
        );
        let mut changed_nullifier_preimage = nullifier_preimage.clone();
        changed_nullifier_preimage.note_secret[0] ^= 0x01;
        assert_ne!(
            nullifier,
            derive_offline_note_input_nullifier(&changed_nullifier_preimage)
                .expect("changed secret nullifier")
        );

        let token_preimage = OfflineNotePaymentTokenIdPreimage {
            domain: OFFLINE_NOTE_PAYMENT_TOKEN_ID_DOMAIN.to_owned(),
            chain_id,
            payment_request_id: "payment-request-fixture".to_owned(),
            created_at_ms: 1_700_000_001_000,
            token_nonce: vec![0xC6; Hash::LENGTH],
            sender_key_certificate_payload_hash: owner_key_certificate_payload_hash,
            input_nullifiers: vec![nullifier],
            output_commitments: vec![commitment],
        };
        let token_id =
            derive_offline_note_payment_token_id(&token_preimage).expect("payment token id");
        assert_eq!(
            token_id,
            derive_offline_note_payment_token_id(&token_preimage).expect("repeat payment token id")
        );
        let mut changed_token_preimage = token_preimage.clone();
        changed_token_preimage.token_nonce[0] ^= 0x01;
        assert_ne!(
            token_id,
            derive_offline_note_payment_token_id(&changed_token_preimage)
                .expect("changed nonce payment token id")
        );
        let mut changed_request_token_preimage = token_preimage.clone();
        changed_request_token_preimage.payment_request_id = "payment-request-other".to_owned();
        assert_ne!(
            token_id,
            derive_offline_note_payment_token_id(&changed_request_token_preimage)
                .expect("changed request payment token id")
        );
        let mut changed_created_at_token_preimage = token_preimage.clone();
        changed_created_at_token_preimage.created_at_ms += 1;
        assert_ne!(
            token_id,
            derive_offline_note_payment_token_id(&changed_created_at_token_preimage)
                .expect("changed created_at payment token id")
        );
    }

    #[test]
    fn offline_note_wallet_derivations_reject_short_random_material() {
        let chain_id: ChainId = "offline-note-derivation-chain".parse().expect("chain id");
        let account_id = sample_account(0xD6, "offline");
        let definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "usd".parse().expect("asset name"),
        );
        let asset = AssetId::new(definition, account_id);
        let owner_key_certificate_payload_hash = Hash::new(b"offline-note-owner-cert");
        let commitment_preimage = OfflineNoteCommitmentPreimage {
            domain: OFFLINE_NOTE_NOTE_COMMITMENT_DOMAIN.to_owned(),
            chain_id: chain_id.clone(),
            owner_key_certificate_payload_hash,
            asset,
            amount: Numeric::new(42, 0),
            note_secret: vec![0xA5; Hash::LENGTH - 1],
            origin: OfflineNoteCommitmentOrigin::IssuerLoad(OfflineNoteIssuerLoadOrigin {
                operation_id: "operation-1".to_owned(),
                lineage_id: "lineage-1".to_owned(),
                local_revision: 7,
            }),
        };
        assert!(matches!(
            derive_offline_note_note_commitment(&commitment_preimage),
            Err(OfflineNoteDerivationError::InvalidRandomBytesLength {
                field: "note_secret",
                expected: Hash::LENGTH,
                actual
            }) if actual == Hash::LENGTH - 1
        ));

        let nullifier_preimage = OfflineNoteInputNullifierPreimage {
            domain: OFFLINE_NOTE_INPUT_NULLIFIER_DOMAIN.to_owned(),
            chain_id: chain_id.clone(),
            source_note_commitment: Hash::new(b"source-note"),
            owner_key_certificate_payload_hash,
            note_secret: vec![0xB6; Hash::LENGTH - 1],
        };
        assert!(matches!(
            derive_offline_note_input_nullifier(&nullifier_preimage),
            Err(OfflineNoteDerivationError::InvalidRandomBytesLength {
                field: "note_secret",
                expected: Hash::LENGTH,
                actual
            }) if actual == Hash::LENGTH - 1
        ));

        let token_preimage = OfflineNotePaymentTokenIdPreimage {
            domain: OFFLINE_NOTE_PAYMENT_TOKEN_ID_DOMAIN.to_owned(),
            chain_id,
            payment_request_id: "payment-request-fixture".to_owned(),
            created_at_ms: 1_700_000_001_000,
            token_nonce: vec![0xC7; Hash::LENGTH - 1],
            sender_key_certificate_payload_hash: owner_key_certificate_payload_hash,
            input_nullifiers: vec![Hash::new(b"nullifier")],
            output_commitments: vec![Hash::new(b"commitment")],
        };
        assert!(matches!(
            derive_offline_note_payment_token_id(&token_preimage),
            Err(OfflineNoteDerivationError::InvalidRandomBytesLength {
                field: "token_nonce",
                expected: Hash::LENGTH,
                actual
            }) if actual == Hash::LENGTH - 1
        ));
    }

    #[test]
    fn kagemusha_proof_public_inputs_statement_digest_binds_all_statement_fields() {
        let statement = KagemushaProofPublicInputsStatement {
            proof_backend: "halo2/ipa".to_owned(),
            envelope_backend: BackendTag::Halo2IpaPasta,
            circuit_id: "halo2/ipa:kagemusha-hop-fixture".to_owned(),
            vk_hash: fixed_hash(b"kagemusha-hop-vk"),
            public_inputs_schema: b"kagemusha-hop-public-schema-v1".to_vec(),
            envelope_aux: Vec::new(),
            instance_columns: vec![vec![[0x11; Hash::LENGTH]], vec![[0x22; Hash::LENGTH]]],
        };
        let digest =
            kagemusha_proof_public_inputs_statement_digest(&statement).expect("statement digest");
        assert_ne!(digest, [0u8; Hash::LENGTH]);
        assert_eq!(
            digest,
            kagemusha_proof_public_inputs_statement_digest(&statement)
                .expect("repeat statement digest")
        );

        let mut changed_backend = statement.clone();
        changed_backend.proof_backend = "stark/fri".to_owned();
        changed_backend.envelope_backend = BackendTag::Stark;
        assert_ne!(
            digest,
            kagemusha_proof_public_inputs_statement_digest(&changed_backend)
                .expect("changed backend digest")
        );

        let mut changed_vk = statement.clone();
        changed_vk.vk_hash = fixed_hash(b"kagemusha-hop-other-vk");
        assert_ne!(
            digest,
            kagemusha_proof_public_inputs_statement_digest(&changed_vk)
                .expect("changed verifier-key digest")
        );

        let mut changed_schema = statement.clone();
        changed_schema.public_inputs_schema.push(0xA5);
        assert_ne!(
            digest,
            kagemusha_proof_public_inputs_statement_digest(&changed_schema)
                .expect("changed schema digest")
        );

        let mut changed_instance = statement;
        changed_instance.instance_columns[1][0][0] ^= 0x01;
        assert_ne!(
            digest,
            kagemusha_proof_public_inputs_statement_digest(&changed_instance)
                .expect("changed instance digest")
        );
    }

    #[test]
    fn kagemusha_proof_public_inputs_statement_digest_rejects_noncanonical_metadata() {
        let mut statement = KagemushaProofPublicInputsStatement {
            proof_backend: "halo2/ipa".to_owned(),
            envelope_backend: BackendTag::Halo2IpaPasta,
            circuit_id: "halo2/ipa:kagemusha-hop-fixture".to_owned(),
            vk_hash: fixed_hash(b"kagemusha-hop-vk"),
            public_inputs_schema: b"kagemusha-hop-public-schema-v1".to_vec(),
            envelope_aux: b"kagemusha-hop-aux".to_vec(),
            instance_columns: vec![vec![[0x11; Hash::LENGTH]]],
        };
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::NonCanonicalProofStatementAuxiliaryBytes { actual })
                if actual == b"kagemusha-hop-aux".len()
        ));

        statement.envelope_aux.clear();
        statement.vk_hash = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::ZeroProofStatementVerifierKeyHash)
        ));

        statement.vk_hash = fixed_hash(b"kagemusha-hop-vk");
        statement.proof_backend = "halo2/ipa".to_owned();
        statement.envelope_backend = BackendTag::Stark;
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::ProofStatementBackendTagMismatch {
                proof_backend,
                envelope_backend: BackendTag::Stark
            }) if proof_backend == "halo2/ipa"
        ));

        statement.proof_backend = "halo2/kzg".to_owned();
        statement.envelope_backend = BackendTag::Halo2IpaPasta;
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));

        statement.proof_backend = "halo2/ipa".to_owned();
        statement.envelope_backend = BackendTag::Halo2IpaPasta;
        statement.circuit_id.clear();
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::EmptyProofStatementCircuitId)
        ));

        statement.circuit_id = "halo2/ipa:kagemusha-hop-fixture".to_owned();
        statement.public_inputs_schema.clear();
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::EmptyProofStatementPublicInputsSchema)
        ));

        statement.public_inputs_schema = b"kagemusha-hop-public-schema-v1".to_vec();
        statement.instance_columns.clear();
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::EmptyProofStatementInstanceColumns)
        ));

        statement.instance_columns = vec![Vec::new()];
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::EmptyProofStatementInstanceColumn { column_index: 0 })
        ));
    }

    #[test]
    fn kagemusha_verifier_key_poseidon_digest_binds_backend_and_bytes() {
        let digest = kagemusha_verifier_key_poseidon_digest("halo2/ipa", b"kagemusha-hop-vk")
            .expect("verifier-key digest");
        assert_ne!(digest, [0u8; Hash::LENGTH]);
        assert_eq!(
            digest,
            kagemusha_verifier_key_poseidon_digest("halo2/ipa", b"kagemusha-hop-vk")
                .expect("repeat verifier-key digest")
        );
        assert_ne!(
            digest,
            kagemusha_verifier_key_poseidon_digest("stark/fri", b"kagemusha-hop-vk")
                .expect("backend-mutated verifier-key digest")
        );
        assert_ne!(
            digest,
            kagemusha_verifier_key_poseidon_digest("halo2/ipa", b"kagemusha-other-vk")
                .expect("bytes-mutated verifier-key digest")
        );
        assert!(matches!(
            kagemusha_verifier_key_poseidon_digest("halo2/kzg", b"kagemusha-hop-vk"),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));
        for backend in [
            "kzg",
            "KZG",
            " kzg ",
            "kzg/ceremony-v1",
            "KZG/ceremony-v1",
            "bn254",
            "BN254",
            "\tBN254\n",
            "bn256",
            "bls12_381",
            "halo2/ipa:kzg",
            "halo2/ipa:KZG",
            "halo2/ipa: KZG",
            "stark/fri:kzg",
            "stark/fri:KZG",
            "stark/fri: KZG",
            "stark/fri/kzg",
            "stark/fri/KZG",
            "stark/fri/ KZG",
            "stark/fri/bn254",
            "stark/fri/bls12_381",
        ] {
            assert!(matches!(
                kagemusha_verifier_key_poseidon_digest(backend, b"kagemusha-hop-vk"),
                Err(KagemushaFoldError::UnsupportedProofBackend { backend: rejected })
                    if rejected == backend
            ));
        }
        assert!(matches!(
            kagemusha_verifier_key_poseidon_digest("stark/fri/", b"kagemusha-hop-vk"),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/"
        ));
        for backend in [
            "debug-proof",
            "Debug-Proof",
            "stark/fri/debug",
            "stark/fri/Debug",
            "stark/fri/debug-proof",
            "mock-proof",
            "Mock-Proof",
            "stark/fri/mock",
            "stark/fri/Mock",
            "stark/fri/mock-proof",
            "halo2/ipa:Mock-Proof",
        ] {
            assert!(matches!(
                kagemusha_verifier_key_poseidon_digest(backend, b"kagemusha-hop-vk"),
                Err(KagemushaFoldError::UnsupportedProofBackend { backend: rejected })
                    if rejected == backend
            ));
        }
        assert!(matches!(
            kagemusha_verifier_key_poseidon_digest("halo2/ipa", &[]),
            Err(KagemushaFoldError::EmptyVerifierKeyBytes { backend })
                if backend == "halo2/ipa"
        ));
    }

    #[test]
    fn kagemusha_aggregation_mode_helpers_mark_recursive_mode_reserved() {
        assert!(is_supported_kagemusha_aggregation_mode(
            KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
        ));
        assert!(!is_supported_kagemusha_aggregation_mode(
            KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        ));
        assert!(
            unsupported_kagemusha_aggregation_mode_reason(
                KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
            )
            .contains("reserved for future in-circuit recursive aggregation")
        );
        assert!(
            unsupported_kagemusha_aggregation_mode_reason(0xFFFF)
                .contains("unsupported or unknown")
        );
    }

    #[test]
    fn kagemusha_poseidon_aggregation_transcript_digest_binds_statement_fields() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let root0 = fixed_hash(b"kagemusha-root-0");
        let root1 = fixed_hash(b"kagemusha-root-1");
        let statement = KagemushaPoseidonAggregationTranscriptStatement {
            aggregation_mode: KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1,
            chain_id,
            asset,
            initial_root: root0,
            final_root: root1,
            hop_count: 1,
            steps: vec![KagemushaPoseidonAggregationStepStatement {
                hop_index: 0,
                root_before: root0,
                input_nullifiers: vec![[0x11; Hash::LENGTH]],
                output_commitments: vec![[0x22; Hash::LENGTH]],
                root_after: root1,
                proof_hash: Hash::new(b"kagemusha-hop-proof"),
                proof_public_inputs_digest: fixed_hash(b"kagemusha-hop-public-inputs"),
                verifier_key_id: VerifyingKeyId::new("halo2/ipa", "kagemusha-hop-fixture"),
                verifier_key_commitment: fixed_hash(b"kagemusha-hop-vk"),
                verifier_key_poseidon_digest: kagemusha_verifier_key_poseidon_digest(
                    "halo2/ipa",
                    b"kagemusha-hop-vk",
                )
                .expect("verifier-key digest"),
            }],
        };
        let digest = kagemusha_poseidon_aggregation_transcript_digest(&statement)
            .expect("aggregation transcript digest");
        assert_ne!(digest, [0u8; Hash::LENGTH]);
        assert_eq!(
            digest,
            kagemusha_poseidon_aggregation_transcript_digest(&statement)
                .expect("repeat aggregation transcript digest")
        );

        let mut changed_mode = statement.clone();
        changed_mode.aggregation_mode = KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1;
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&changed_mode),
            Err(KagemushaFoldError::UnsupportedAggregationMode { actual, .. })
                if actual == KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        ));

        let mut changed_hop = statement;
        changed_hop.steps[0].verifier_key_poseidon_digest[0] ^= 0x01;
        assert_ne!(
            digest,
            kagemusha_poseidon_aggregation_transcript_digest(&changed_hop)
                .expect("changed hop statement digest")
        );
    }

    #[test]
    fn kagemusha_poseidon_aggregation_transcript_digest_rejects_noncanonical_statement() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let root0 = fixed_hash(b"kagemusha-root-0");
        let root1 = fixed_hash(b"kagemusha-root-1");
        let root2 = fixed_hash(b"kagemusha-root-2");
        let steps = vec![
            kagemusha_step(root0, root1, 0x20, 0x40, b"proof-hop-0"),
            kagemusha_step(root1, root2, 0x60, 0x80, b"proof-hop-1"),
        ];
        let statement =
            kagemusha_poseidon_aggregation_transcript_statement(&chain_id, &asset, &steps)
                .expect("canonical aggregation statement");

        let mut empty = statement.clone();
        empty.steps.clear();
        empty.hop_count = 0;
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&empty),
            Err(KagemushaFoldError::Empty)
        ));

        let mut bad_hop_count = statement.clone();
        bad_hop_count.hop_count = 1;
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&bad_hop_count),
            Err(KagemushaFoldError::HopCountMismatch {
                expected: 2,
                actual: 1
            })
        ));

        let mut bad_hop_index = statement.clone();
        bad_hop_index.steps[1].hop_index = 7;
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&bad_hop_index),
            Err(KagemushaFoldError::HopIndexMismatch {
                expected: 1,
                actual: 7
            })
        ));

        let mut bad_initial_root = statement.clone();
        bad_initial_root.initial_root = fixed_hash(b"kagemusha-forged-initial-root");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&bad_initial_root),
            Err(KagemushaFoldError::InitialRootMismatch { .. })
        ));

        let mut bad_final_root = statement.clone();
        bad_final_root.final_root = fixed_hash(b"kagemusha-forged-final-root");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&bad_final_root),
            Err(KagemushaFoldError::FinalRootMismatch { .. })
        ));

        let mut zero_initial_root = statement.clone();
        zero_initial_root.initial_root = [0u8; Hash::LENGTH];
        zero_initial_root.steps[0].root_before = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_initial_root),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "initial_root"
            })
        ));

        let mut zero_final_root = statement.clone();
        zero_final_root.final_root = [0u8; Hash::LENGTH];
        zero_final_root.steps[1].root_after = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_final_root),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "final_root"
            })
        ));

        let mut zero_intermediate_root = statement.clone();
        zero_intermediate_root.steps[0].root_after = [0u8; Hash::LENGTH];
        zero_intermediate_root.steps[1].root_before = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_intermediate_root),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "root_after"
            })
        ));

        let mut unchanged_public_roots = statement.clone();
        unchanged_public_roots.final_root = unchanged_public_roots.initial_root;
        unchanged_public_roots.steps[1].root_after = unchanged_public_roots.initial_root;
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&unchanged_public_roots),
            Err(KagemushaFoldError::UnchangedFoldedPublicRoots)
        ));

        let mut discontinuous = statement.clone();
        discontinuous.steps[1].root_before = fixed_hash(b"kagemusha-forged-root-before");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&discontinuous),
            Err(KagemushaFoldError::RootDiscontinuity { hop_index: 1, .. })
        ));

        let mut unsorted_inputs = statement.clone();
        unsorted_inputs.steps[0].input_nullifiers.reverse();
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&unsorted_inputs),
            Err(KagemushaFoldError::NonCanonicalInputNullifierOrder { hop_index: 0 })
        ));

        let mut unsorted_outputs = statement.clone();
        unsorted_outputs.steps[0].output_commitments.reverse();
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&unsorted_outputs),
            Err(KagemushaFoldError::NonCanonicalOutputCommitmentOrder { hop_index: 0 })
        ));

        let mut zero_input = statement.clone();
        zero_input.steps[0].input_nullifiers[0] = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_input),
            Err(KagemushaFoldError::ZeroInputNullifier { hop_index: 0 })
        ));

        let mut zero_output = statement.clone();
        zero_output.steps[0].output_commitments[0] = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_output),
            Err(KagemushaFoldError::ZeroOutputCommitment { hop_index: 0 })
        ));

        let mut zero_proof_inputs = statement.clone();
        zero_proof_inputs.steps[0].proof_public_inputs_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_proof_inputs),
            Err(KagemushaFoldError::ZeroProofPublicInputsDigest { hop_index: 0 })
        ));

        let mut zero_vk_commitment = statement.clone();
        zero_vk_commitment.steps[0].verifier_key_commitment = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_vk_commitment),
            Err(KagemushaFoldError::ZeroVerifierKeyCommitment { hop_index: 0 })
        ));

        let mut zero_vk_poseidon = statement.clone();
        zero_vk_poseidon.steps[0].verifier_key_poseidon_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_vk_poseidon),
            Err(KagemushaFoldError::ZeroVerifierKeyPoseidonDigest { hop_index: 0 })
        ));

        let mut duplicate_input = statement.clone();
        duplicate_input.steps[1].input_nullifiers[0] = duplicate_input.steps[0].input_nullifiers[0];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&duplicate_input),
            Err(KagemushaFoldError::DuplicateInputNullifier { hop_index: 1 })
        ));

        let mut duplicate_output = statement.clone();
        duplicate_output.steps[1].output_commitments[0] =
            duplicate_output.steps[0].output_commitments[0];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&duplicate_output),
            Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index: 1 })
        ));

        let mut empty_vk_id_name = statement.clone();
        empty_vk_id_name.steps[0].verifier_key_id.name = "   ".to_owned();
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&empty_vk_id_name),
            Err(KagemushaFoldError::EmptyVerifierKeyIdName { hop_index: 0 })
        ));

        let mut empty_stark_profile = statement.clone();
        empty_stark_profile.steps[0].verifier_key_id =
            VerifyingKeyId::new("stark/fri/", "kagemusha-hop-fixture");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&empty_stark_profile),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/"
        ));

        let mut developer_only_stark_profile = statement.clone();
        developer_only_stark_profile.steps[0].verifier_key_id =
            VerifyingKeyId::new("stark/fri/debug", "kagemusha-hop-fixture");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&developer_only_stark_profile),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/debug"
        ));

        let mut developer_only_hyphen_profile = developer_only_stark_profile.clone();
        developer_only_hyphen_profile.steps[0].verifier_key_id =
            VerifyingKeyId::new("stark/fri/debug-proof", "kagemusha-hop-fixture");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&developer_only_hyphen_profile),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/debug-proof"
        ));

        let mut trusted_setup_backend = statement;
        trusted_setup_backend.steps[0].verifier_key_id =
            VerifyingKeyId::new("halo2/kzg", "kagemusha-hop-fixture");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&trusted_setup_backend),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));

        let mut trusted_setup_stark_profile = trusted_setup_backend.clone();
        trusted_setup_stark_profile.steps[0].verifier_key_id =
            VerifyingKeyId::new("stark/fri/kzg", "kagemusha-hop-fixture");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&trusted_setup_stark_profile),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/kzg"
        ));
    }

    #[test]
    fn kagemusha_folded_public_inputs_canonicalize_and_bind_transcript() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let root0 = fixed_hash(b"kagemusha-root-0");
        let root1 = fixed_hash(b"kagemusha-root-1");
        let root2 = fixed_hash(b"kagemusha-root-2");
        let steps = vec![
            kagemusha_step(root0, root1, 0x20, 0x40, b"proof-hop-0"),
            kagemusha_step(root1, root2, 0x60, 0x80, b"proof-hop-1"),
        ];

        let public_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &steps).expect("folded inputs");
        assert_eq!(public_inputs.domain, KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN);
        assert_eq!(
            public_inputs.aggregation_mode,
            KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
        );
        assert_eq!(public_inputs.initial_root, root0);
        assert_eq!(public_inputs.final_root, root2);
        assert_eq!(public_inputs.hop_count, 2);
        assert_ne!(
            public_inputs.aggregation_transcript_digest,
            [0u8; Hash::LENGTH]
        );
        let aggregation_statement =
            kagemusha_poseidon_aggregation_transcript_statement(&chain_id, &asset, &steps)
                .expect("aggregation statement");
        assert_eq!(
            aggregation_statement.aggregation_mode,
            KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
        );
        assert_eq!(aggregation_statement.initial_root, root0);
        assert_eq!(aggregation_statement.final_root, root2);
        assert_eq!(aggregation_statement.hop_count, 2);
        assert_eq!(
            aggregation_statement.steps[0].input_nullifiers,
            vec![[0x20; Hash::LENGTH], [0x21; Hash::LENGTH]]
        );
        assert_eq!(
            aggregation_statement.steps[0].output_commitments,
            vec![[0x40; Hash::LENGTH], [0x41; Hash::LENGTH]]
        );
        assert_eq!(
            aggregation_statement.steps[1].input_nullifiers,
            vec![[0x60; Hash::LENGTH], [0x61; Hash::LENGTH]]
        );
        assert_eq!(
            public_inputs.aggregation_transcript_digest,
            kagemusha_poseidon_aggregation_transcript_digest(&aggregation_statement)
                .expect("aggregation statement digest")
        );
        assert_eq!(
            public_inputs,
            kagemusha_folded_public_inputs_from_aggregation_statement(&aggregation_statement)
                .expect("aggregation statement projection")
        );
        kagemusha_validate_folded_public_inputs_against_aggregation_statement(
            &public_inputs,
            &aggregation_statement,
        )
        .expect("aggregation statement must bind folded public inputs");

        let mut reordered = steps.clone();
        reordered[0].input_nullifiers.reverse();
        reordered[0].output_commitments.reverse();
        reordered[1].input_nullifiers.reverse();
        reordered[1].output_commitments.reverse();
        let reordered_inputs = kagemusha_folded_public_inputs(&chain_id, &asset, &reordered)
            .expect("canonical reordered folded inputs");
        assert_eq!(public_inputs, reordered_inputs);
        let reordered_statement =
            kagemusha_poseidon_aggregation_transcript_statement(&chain_id, &asset, &reordered)
                .expect("canonical reordered aggregation statement");
        assert_eq!(aggregation_statement, reordered_statement);

        let public_hash = public_inputs
            .public_inputs_hash()
            .expect("folded public inputs hash");
        let aggregation_digest = public_inputs.aggregation_transcript_digest;
        let mut changed_proof = steps.clone();
        changed_proof[1].proof_hash = Hash::new(b"proof-hop-1-forged");
        let changed_proof_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &changed_proof)
                .expect("changed proof folded inputs");
        assert_ne!(
            public_hash,
            changed_proof_inputs
                .public_inputs_hash()
                .expect("changed proof public hash")
        );
        assert_ne!(
            aggregation_digest,
            changed_proof_inputs.aggregation_transcript_digest
        );

        let mut changed_proof_statement = steps.clone();
        changed_proof_statement[1].proof_public_inputs_digest =
            fixed_hash(b"kagemusha-hop-public-inputs-forged");
        let changed_proof_statement_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &changed_proof_statement)
                .expect("changed proof public-input folded inputs");
        assert_ne!(
            public_hash,
            changed_proof_statement_inputs
                .public_inputs_hash()
                .expect("changed proof public-input public hash")
        );
        assert_ne!(
            aggregation_digest,
            changed_proof_statement_inputs.aggregation_transcript_digest
        );

        let mut changed_vk = steps.clone();
        changed_vk[1].verifier_key_commitment = fixed_hash(b"kagemusha-hop-vk-forged");
        let changed_vk_inputs = kagemusha_folded_public_inputs(&chain_id, &asset, &changed_vk)
            .expect("changed verifier-key folded inputs");
        assert_ne!(
            public_hash,
            changed_vk_inputs
                .public_inputs_hash()
                .expect("changed verifier-key public hash")
        );
        assert_ne!(
            aggregation_digest,
            changed_vk_inputs.aggregation_transcript_digest
        );

        let mut changed_vk_poseidon = steps.clone();
        changed_vk_poseidon[1].verifier_key_poseidon_digest =
            fixed_hash(b"kagemusha-hop-vk-poseidon-forged");
        let changed_vk_poseidon_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &changed_vk_poseidon)
                .expect("changed verifier-key Poseidon digest folded inputs");
        assert_ne!(
            public_hash,
            changed_vk_poseidon_inputs
                .public_inputs_hash()
                .expect("changed verifier-key Poseidon digest public hash")
        );
        assert_ne!(
            aggregation_digest,
            changed_vk_poseidon_inputs.aggregation_transcript_digest
        );

        let mut changed_vk_ref = steps.clone();
        changed_vk_ref[1].verifier_key_id = VerifyingKeyId::new("halo2/ipa", "kagemusha-hop-other");
        let changed_vk_ref_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &changed_vk_ref)
                .expect("changed verifier-key id folded inputs");
        assert_ne!(
            public_hash,
            changed_vk_ref_inputs
                .public_inputs_hash()
                .expect("changed verifier-key id public hash")
        );
        assert_ne!(
            aggregation_digest,
            changed_vk_ref_inputs.aggregation_transcript_digest
        );

        let other_chain_id: ChainId = "kagemusha-fold-other-chain".parse().expect("chain id");
        let other_chain_inputs = kagemusha_folded_public_inputs(&other_chain_id, &asset, &steps)
            .expect("changed chain folded inputs");
        assert_ne!(
            public_hash,
            other_chain_inputs
                .public_inputs_hash()
                .expect("changed chain public hash")
        );
        assert_ne!(
            aggregation_digest,
            other_chain_inputs.aggregation_transcript_digest
        );

        let other_asset = kagemusha_asset("kgm-other");
        let other_asset_inputs = kagemusha_folded_public_inputs(&chain_id, &other_asset, &steps)
            .expect("changed asset folded inputs");
        assert_ne!(
            public_hash,
            other_asset_inputs
                .public_inputs_hash()
                .expect("changed asset public hash")
        );
        assert_ne!(
            aggregation_digest,
            other_asset_inputs.aggregation_transcript_digest
        );
    }

    #[test]
    fn kagemusha_folded_public_inputs_reject_transcript_projection_mismatches() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let root0 = fixed_hash(b"kagemusha-root-0");
        let root1 = fixed_hash(b"kagemusha-root-1");
        let root2 = fixed_hash(b"kagemusha-root-2");
        let steps = vec![
            kagemusha_step(root0, root1, 0x20, 0x40, b"proof-hop-0"),
            kagemusha_step(root1, root2, 0x60, 0x80, b"proof-hop-1"),
        ];
        let public_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &steps).expect("folded inputs");
        let statement =
            kagemusha_poseidon_aggregation_transcript_statement(&chain_id, &asset, &steps)
                .expect("aggregation statement");

        let expect_mismatch = |forged: KagemushaFoldedPublicInputs, field: &'static str| {
            forged
                .public_inputs_hash()
                .expect("forged public inputs remain encodable");
            assert!(matches!(
                kagemusha_validate_folded_public_inputs_against_aggregation_statement(
                    &forged,
                    &statement,
                ),
                Err(KagemushaFoldError::FoldedPublicInputTranscriptMismatch {
                    field: actual
                }) if actual == field
            ));
        };

        let mut forged_chain = public_inputs.clone();
        forged_chain.chain_id = "kagemusha-forged-chain".parse().expect("chain id");
        expect_mismatch(forged_chain, "chain_id");

        let mut forged_asset = public_inputs.clone();
        forged_asset.asset = kagemusha_asset("kgm-forged");
        expect_mismatch(forged_asset, "asset");

        let mut forged_initial_root = public_inputs.clone();
        forged_initial_root.initial_root = fixed_hash(b"kagemusha-forged-initial-root");
        expect_mismatch(forged_initial_root, "initial_root");

        let mut forged_final_root = public_inputs.clone();
        forged_final_root.final_root = fixed_hash(b"kagemusha-forged-final-root");
        expect_mismatch(forged_final_root, "final_root");

        let mut forged_hop_count = public_inputs.clone();
        forged_hop_count.hop_count = 1;
        expect_mismatch(forged_hop_count, "hop_count");

        let mut forged_nullifiers = public_inputs.clone();
        forged_nullifiers.nullifier_digest = Hash::new(b"kagemusha-forged-nullifiers");
        expect_mismatch(forged_nullifiers, "nullifier_digest");

        let mut forged_outputs = public_inputs.clone();
        forged_outputs.output_commitment_digest = Hash::new(b"kagemusha-forged-outputs");
        expect_mismatch(forged_outputs, "output_commitment_digest");

        let mut forged_fold = public_inputs.clone();
        forged_fold.fold_digest = Hash::new(b"kagemusha-forged-fold");
        expect_mismatch(forged_fold, "fold_digest");

        let mut forged_aggregation = public_inputs.clone();
        forged_aggregation.aggregation_transcript_digest =
            fixed_hash(b"kagemusha-forged-aggregation");
        expect_mismatch(forged_aggregation, "aggregation_transcript_digest");

        let mut zero_aggregation = public_inputs.clone();
        zero_aggregation.aggregation_transcript_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_validate_folded_public_inputs_against_aggregation_statement(
                &zero_aggregation,
                &statement,
            ),
            Err(KagemushaFoldError::ZeroFoldedPublicInputDigest {
                field: "aggregation_transcript_digest"
            })
        ));

        let mut forged_domain = public_inputs.clone();
        forged_domain.domain = "iroha:kagemusha:forged-domain".to_owned();
        assert!(matches!(
            kagemusha_validate_folded_public_inputs_against_aggregation_statement(
                &forged_domain,
                &statement,
            ),
            Err(KagemushaFoldError::InvalidPublicInputDomain { .. })
        ));

        let mut forged_mode = public_inputs.clone();
        forged_mode.aggregation_mode = KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1;
        assert!(matches!(
            kagemusha_validate_folded_public_inputs_against_aggregation_statement(
                &forged_mode,
                &statement,
            ),
            Err(KagemushaFoldError::UnsupportedAggregationMode { actual, .. })
                if actual == KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        ));

        let mut forged_statement = statement;
        forged_statement.steps[1].proof_hash = Hash::new(b"kagemusha-forged-hop-proof");
        assert!(matches!(
            kagemusha_validate_folded_public_inputs_against_aggregation_statement(
                &public_inputs,
                &forged_statement,
            ),
            Err(KagemushaFoldError::FoldedPublicInputTranscriptMismatch {
                field: "fold_digest"
            })
        ));
    }

    #[test]
    fn kagemusha_compact_token_binds_folded_proof_public_inputs() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let root0 = fixed_hash(b"kagemusha-root-0");
        let root1 = fixed_hash(b"kagemusha-root-1");
        let public_inputs = kagemusha_folded_public_inputs(
            &chain_id,
            &asset,
            &[kagemusha_step(root0, root1, 0x20, 0x40, b"proof-hop-0")],
        )
        .expect("folded inputs");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("folded public inputs hash");
        let token = KagemushaCompactPaymentToken {
            public_inputs: public_inputs.clone(),
            folded_proof: KagemushaFoldedProof {
                verifier_key_id: VerifyingKeyId::new("halo2/ipa", "kagemusha-folded-v1"),
                public_inputs_hash,
                proof: ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE]),
            },
        };
        token
            .validate_public_input_binding()
            .expect("matching compact token binding");

        let mut forged = token.clone();
        forged.folded_proof.public_inputs_hash = Hash::new(b"forged-folded-public-inputs");
        assert!(matches!(
            forged.validate_public_input_binding(),
            Err(KagemushaFoldError::PublicInputHashMismatch { .. })
        ));

        let mut forged_domain = forged.clone();
        forged_domain.public_inputs.domain = "iroha:kagemusha:forged-domain".to_owned();
        forged_domain.folded_proof.public_inputs_hash = forged_domain
            .public_inputs
            .public_inputs_hash()
            .expect("forged-domain hash");
        assert!(matches!(
            forged_domain.validate_public_input_binding(),
            Err(KagemushaFoldError::InvalidPublicInputDomain { .. })
        ));

        let mut forged_zero_hops = forged_domain.clone();
        forged_zero_hops.public_inputs.domain = KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN.to_owned();
        forged_zero_hops.public_inputs.hop_count = 0;
        forged_zero_hops.folded_proof.public_inputs_hash = forged_zero_hops
            .public_inputs
            .public_inputs_hash()
            .expect("forged-zero-hop hash");
        assert!(matches!(
            forged_zero_hops.validate_public_input_binding(),
            Err(KagemushaFoldError::Empty)
        ));

        let mut forged_zero_aggregation = forged_zero_hops.clone();
        forged_zero_aggregation.public_inputs.hop_count = 1;
        forged_zero_aggregation
            .public_inputs
            .aggregation_transcript_digest = [0u8; Hash::LENGTH];
        forged_zero_aggregation.folded_proof.public_inputs_hash = forged_zero_aggregation
            .public_inputs
            .public_inputs_hash()
            .expect("forged-zero-aggregation hash");
        assert!(matches!(
            forged_zero_aggregation.validate_public_input_binding(),
            Err(KagemushaFoldError::ZeroFoldedPublicInputDigest {
                field: "aggregation_transcript_digest"
            })
        ));

        let mut forged_zero_initial_root = token.clone();
        forged_zero_initial_root.public_inputs.initial_root = [0u8; Hash::LENGTH];
        forged_zero_initial_root.folded_proof.public_inputs_hash = forged_zero_initial_root
            .public_inputs
            .public_inputs_hash()
            .expect("forged-zero-initial-root hash");
        assert!(matches!(
            forged_zero_initial_root.validate_public_input_binding(),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "initial_root"
            })
        ));

        let mut forged_zero_final_root = token.clone();
        forged_zero_final_root.public_inputs.final_root = [0u8; Hash::LENGTH];
        forged_zero_final_root.folded_proof.public_inputs_hash = forged_zero_final_root
            .public_inputs
            .public_inputs_hash()
            .expect("forged-zero-final-root hash");
        assert!(matches!(
            forged_zero_final_root.validate_public_input_binding(),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "final_root"
            })
        ));

        let mut forged_unchanged_roots = token.clone();
        forged_unchanged_roots.public_inputs.final_root =
            forged_unchanged_roots.public_inputs.initial_root;
        forged_unchanged_roots.folded_proof.public_inputs_hash = forged_unchanged_roots
            .public_inputs
            .public_inputs_hash()
            .expect("forged-unchanged-root hash");
        assert!(matches!(
            forged_unchanged_roots.validate_public_input_binding(),
            Err(KagemushaFoldError::UnchangedFoldedPublicRoots)
        ));

        let mut forged_too_many_hops = forged_zero_hops.clone();
        forged_too_many_hops.public_inputs.hop_count =
            u32::try_from(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1).expect("hop count fits");
        forged_too_many_hops.folded_proof.public_inputs_hash = forged_too_many_hops
            .public_inputs
            .public_inputs_hash()
            .expect("forged-too-many-hop hash");
        assert!(matches!(
            forged_too_many_hops.validate_public_input_binding(),
            Err(KagemushaFoldError::TooManyHops { actual, .. })
                if actual == KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
        ));

        let mut forged_mode = forged;
        forged_mode.public_inputs.aggregation_mode =
            KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1;
        forged_mode.folded_proof.public_inputs_hash = forged_mode
            .public_inputs
            .public_inputs_hash()
            .expect("forged-mode hash");
        assert!(matches!(
            forged_mode.validate_public_input_binding(),
            Err(KagemushaFoldError::UnsupportedAggregationMode { actual, .. })
                if actual == KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        ));
        let err = forged_mode
            .validate_public_input_binding()
            .expect_err("reserved recursive mode must be rejected");
        assert!(
            err.to_string()
                .contains("reserved for future in-circuit recursive aggregation")
        );
    }

    #[test]
    fn kagemusha_verified_fold_record_bundle_roundtrips() {
        let chain_id: ChainId = "kagemusha-record-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-record");
        let vk_id = VerifyingKeyId::new("halo2/ipa", "kagemusha-hop-fixture");
        let proof = ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE, 0x01]);
        let mut attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof, vk_id.clone());
        let vk_commitment = fixed_hash(b"kagemusha-record-vk");
        attachment.vk_commitment = Some(vk_commitment);
        let verifier_key = VerifyingKeyBox::new("halo2/ipa".into(), vec![0x42; 48]);
        let step = KagemushaVerifiedFoldStep {
            root_before: fixed_hash(b"kagemusha-record-root-0"),
            input_nullifiers: vec![fixed_hash(b"kagemusha-record-nullifier")],
            output_commitments: vec![fixed_hash(b"kagemusha-record-output")],
            root_after: fixed_hash(b"kagemusha-record-root-1"),
            attachment,
            verifier_key: verifier_key.clone(),
        };
        let bundle = KagemushaVerifiedFoldBundle {
            chain_id,
            asset,
            steps: vec![step],
        };
        let mut record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:tiny-add",
            BackendTag::Halo2IpaPasta,
            "pasta",
            fixed_hash(b"kagemusha-record-schema"),
            vk_commitment,
        );
        record.status = ConfidentialStatus::Active;
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.max_proof_bytes = 4096;
        record.key = Some(verifier_key);
        let record_bundle = KagemushaVerifiedFoldRecordBundle {
            bundle,
            verifier_records: vec![KagemushaVerifiedFoldVerifierRecord { id: vk_id, record }],
        };

        let bytes = to_bytes(&record_bundle).expect("encode record-backed bundle");
        let decoded: KagemushaVerifiedFoldRecordBundle =
            norito::decode_from_bytes(&bytes).expect("decode record-backed bundle");
        assert_eq!(decoded, record_bundle);
    }

    #[test]
    fn kagemusha_folded_public_inputs_stay_size_bounded_at_max_hops() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let roots = (0..=KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS)
            .map(|index| fixed_hash(format!("kagemusha-size-root-{index}").as_bytes()))
            .collect::<Vec<_>>();
        let steps = (0..KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS)
            .map(|index| {
                let input_seed = u8::try_from(index * 2 + 1).expect("bounded input seed");
                let output_seed = u8::try_from(128 + index * 2).expect("bounded output seed");
                let proof_label = format!("kagemusha-size-proof-hop-{index}");
                let mut step = kagemusha_step(
                    roots[index],
                    roots[index + 1],
                    input_seed,
                    output_seed,
                    b"kagemusha-size-proof",
                );
                step.proof_hash = Hash::new(proof_label.as_bytes());
                step.proof_public_inputs_digest =
                    fixed_hash(format!("{proof_label}:public-inputs").as_bytes());
                step.verifier_key_commitment = fixed_hash(format!("{proof_label}:vk").as_bytes());
                step.verifier_key_poseidon_digest =
                    kagemusha_verifier_key_poseidon_digest("halo2/ipa", proof_label.as_bytes())
                        .expect("size verifier-key digest");
                step
            })
            .collect::<Vec<_>>();

        let public_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &steps).expect("folded inputs");
        assert_eq!(
            public_inputs.hop_count,
            u32::try_from(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS).expect("hop count fits")
        );
        let public_inputs_len = public_inputs
            .norito_encoded_len()
            .expect("folded public inputs encoded length");
        assert!(
            public_inputs_len <= KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES,
            "folded public inputs grew to {public_inputs_len} bytes"
        );
        public_inputs
            .validate_supported_context()
            .expect("max-hop folded public inputs stay inside size budget");

        let mut oversized_public_inputs = public_inputs.clone();
        oversized_public_inputs.chain_id = "kagemusha-size-chain-"
            .repeat(KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES)
            .into();
        assert!(matches!(
            oversized_public_inputs.validate_supported_context(),
            Err(KagemushaFoldError::EncodedSizeExceeded { actual, .. })
                if actual > KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES
        ));

        let token = KagemushaCompactPaymentToken {
            public_inputs: public_inputs.clone(),
            folded_proof: KagemushaFoldedProof {
                verifier_key_id: VerifyingKeyId::new("halo2/ipa", "kagemusha-folded-v1"),
                public_inputs_hash: public_inputs
                    .public_inputs_hash()
                    .expect("folded public inputs hash"),
                proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 256]),
            },
        };
        let token_len = token
            .norito_encoded_len()
            .expect("compact token encoded length");
        assert!(
            token_len > public_inputs_len,
            "compact token length should include proof payload"
        );
        token
            .validate_public_input_binding()
            .expect("size-regression token binding");
    }

    #[test]
    fn kagemusha_folded_public_inputs_reject_malformed_witnesses() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let root0 = fixed_hash(b"kagemusha-root-0");
        let root1 = fixed_hash(b"kagemusha-root-1");
        let root2 = fixed_hash(b"kagemusha-root-2");
        let step0 = kagemusha_step(root0, root1, 0x20, 0x40, b"proof-hop-0");
        let step1 = kagemusha_step(root1, root2, 0x60, 0x80, b"proof-hop-1");

        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &[]),
            Err(KagemushaFoldError::Empty)
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(&chain_id, &asset, &[]),
            Err(KagemushaFoldError::Empty)
        ));

        let too_many = vec![step0.clone(); KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &too_many),
            Err(KagemushaFoldError::TooManyHops { actual, .. })
                if actual == KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(&chain_id, &asset, &too_many),
            Err(KagemushaFoldError::TooManyHops { actual, .. })
                if actual == KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
        ));

        let mut trusted_setup_backend = step0.clone();
        trusted_setup_backend.verifier_key_id =
            VerifyingKeyId::new("halo2/kzg", "kagemusha-hop-fixture");
        let trusted_setup_backend_steps = [trusted_setup_backend];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &trusted_setup_backend_steps),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &trusted_setup_backend_steps
            ),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));

        let mut zero_input = step0.clone();
        zero_input.input_nullifiers[0] = [0u8; Hash::LENGTH];
        let zero_input_steps = [zero_input];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_input_steps),
            Err(KagemushaFoldError::ZeroInputNullifier { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_input_steps
            ),
            Err(KagemushaFoldError::ZeroInputNullifier { hop_index: 0 })
        ));

        let mut zero_output = step0.clone();
        zero_output.output_commitments[0] = [0u8; Hash::LENGTH];
        let zero_output_steps = [zero_output];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_output_steps),
            Err(KagemushaFoldError::ZeroOutputCommitment { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_output_steps
            ),
            Err(KagemushaFoldError::ZeroOutputCommitment { hop_index: 0 })
        ));

        let mut zero_proof_inputs = step0.clone();
        zero_proof_inputs.proof_public_inputs_digest = [0u8; Hash::LENGTH];
        let zero_proof_inputs_steps = [zero_proof_inputs];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_proof_inputs_steps),
            Err(KagemushaFoldError::ZeroProofPublicInputsDigest { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_proof_inputs_steps
            ),
            Err(KagemushaFoldError::ZeroProofPublicInputsDigest { hop_index: 0 })
        ));

        let mut zero_vk_commitment = step0.clone();
        zero_vk_commitment.verifier_key_commitment = [0u8; Hash::LENGTH];
        let zero_vk_commitment_steps = [zero_vk_commitment];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_vk_commitment_steps),
            Err(KagemushaFoldError::ZeroVerifierKeyCommitment { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_vk_commitment_steps
            ),
            Err(KagemushaFoldError::ZeroVerifierKeyCommitment { hop_index: 0 })
        ));

        let mut zero_vk_poseidon = step0.clone();
        zero_vk_poseidon.verifier_key_poseidon_digest = [0u8; Hash::LENGTH];
        let zero_vk_poseidon_steps = [zero_vk_poseidon];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_vk_poseidon_steps),
            Err(KagemushaFoldError::ZeroVerifierKeyPoseidonDigest { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_vk_poseidon_steps
            ),
            Err(KagemushaFoldError::ZeroVerifierKeyPoseidonDigest { hop_index: 0 })
        ));

        let mut zero_initial_root = step0.clone();
        zero_initial_root.root_before = [0u8; Hash::LENGTH];
        let zero_initial_root_steps = [zero_initial_root];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_initial_root_steps),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "initial_root"
            })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_initial_root_steps
            ),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "initial_root"
            })
        ));

        let mut zero_root_after = step0.clone();
        zero_root_after.root_after = [0u8; Hash::LENGTH];
        let zero_root_after_steps = [zero_root_after];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_root_after_steps),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "root_after"
            })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_root_after_steps
            ),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "root_after"
            })
        ));

        let mut unchanged_root = step0.clone();
        unchanged_root.root_after = unchanged_root.root_before;
        let unchanged_root_steps = [unchanged_root];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &unchanged_root_steps),
            Err(KagemushaFoldError::UnchangedFoldedRootTransition { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &unchanged_root_steps
            ),
            Err(KagemushaFoldError::UnchangedFoldedRootTransition { hop_index: 0 })
        ));

        let mut empty_input = step0.clone();
        empty_input.input_nullifiers.clear();
        let empty_input_steps = [empty_input];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &empty_input_steps),
            Err(KagemushaFoldError::InvalidStepShape {
                hop_index: 0,
                input_count: 0,
                ..
            })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &empty_input_steps
            ),
            Err(KagemushaFoldError::InvalidStepShape {
                hop_index: 0,
                input_count: 0,
                ..
            })
        ));

        let mut oversized_output = step0.clone();
        oversized_output
            .output_commitments
            .push([0xAB; Hash::LENGTH]);
        let oversized_output_steps = [oversized_output];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &oversized_output_steps),
            Err(KagemushaFoldError::InvalidStepShape {
                hop_index: 0,
                output_count: 3,
                ..
            })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &oversized_output_steps
            ),
            Err(KagemushaFoldError::InvalidStepShape {
                hop_index: 0,
                output_count: 3,
                ..
            })
        ));

        let mut duplicate_input = step1.clone();
        duplicate_input.input_nullifiers[0] = step0.input_nullifiers[0];
        let duplicate_input_steps = [step0.clone(), duplicate_input];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &duplicate_input_steps),
            Err(KagemushaFoldError::DuplicateInputNullifier { hop_index: 1 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &duplicate_input_steps
            ),
            Err(KagemushaFoldError::DuplicateInputNullifier { hop_index: 1 })
        ));

        let mut duplicate_output = step1.clone();
        duplicate_output.output_commitments[0] = duplicate_output.output_commitments[1];
        let duplicate_output_steps = [step0.clone(), duplicate_output];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &duplicate_output_steps),
            Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index: 1 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &duplicate_output_steps
            ),
            Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index: 1 })
        ));

        let mut discontinuous = step1;
        discontinuous.root_before = fixed_hash(b"kagemusha-root-forged");
        let discontinuous_steps = [step0, discontinuous];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &discontinuous_steps),
            Err(KagemushaFoldError::RootDiscontinuity { hop_index: 1, .. })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &discontinuous_steps
            ),
            Err(KagemushaFoldError::RootDiscontinuity { hop_index: 1, .. })
        ));
    }
}
