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
/// Domain-separation tag for reserved Kagemusha recursive aggregation evidence.
pub const KAGEMUSHA_RECURSIVE_AGGREGATION_EVIDENCE_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-aggregation-evidence";
/// Domain-separation tag for recursive aggregation proof public inputs.
pub const KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-aggregation-proof-public-inputs";
/// Domain-separation tag for spendable recursive Kagemusha accumulator state.
pub const KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-accumulator";
/// Domain-separation tag for spendable recursive Kagemusha accumulator digests.
pub const KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-accumulator-digest";
/// Domain-separation tag for streaming recursive Kagemusha lineage updates.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-lineage";
/// Domain-separation tag for streaming recursive Kagemusha verifier-batch updates.
pub const KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_BATCH_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-verifier-batch";
/// Domain-separation tag for streaming recursive Kagemusha fixed-window table-base updates.
pub const KAGEMUSHA_RECURSIVE_SPEND_FIXED_WINDOW_TABLE_BASE_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-fixed-window-table-base";
/// Domain-separation tag for recursive Kagemusha proof artifact digests.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_ARTIFACT_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-proof-artifact";
/// Domain-separation tag for streaming recursive Kagemusha proof-chain updates.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_CHAIN_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-proof-chain";
/// Canonical verifier-witness profile for reserved Kagemusha recursive aggregation evidence.
pub const KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1: &str =
    "pallas-ipa-transparent-v1/vesta-recursive-fixed-window-85x3";
/// Canonical circuit id for proof-carrying Kagemusha recursive aggregation evidence.
pub const KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1: &str =
    "kagemusha-recursive-aggregation-v1";
/// Reserved chain-admission circuit id for lineage-proving recursive spend redemption.
///
/// Proofs under this id must verify the private-hop verifier batch and the
/// recursive spend accumulator transition in-circuit before the chain can
/// accept them for public minting.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1: &str =
    "kagemusha-recursive-spend-lineage-v1";
const KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND: &str = "halo2/ipa";
/// Minimum Pallas IPA opening length accepted by reserved Kagemusha recursive evidence.
pub const KAGEMUSHA_RECURSIVE_PALLAS_IPA_BATCH_MIN_LEN: u32 = 2;
/// Maximum Pallas IPA opening length accepted by reserved Kagemusha recursive evidence.
pub const KAGEMUSHA_RECURSIVE_PALLAS_IPA_BATCH_MAX_LEN: u32 = 128;
/// Current Kagemusha aggregation mode: every private hop proof is verified before folding.
pub const KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1: u16 = 1;
/// Reserved Kagemusha aggregation mode for future in-circuit recursive proof aggregation.
pub const KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1: u16 = 2;
/// SDK-facing Kagemusha spend mode for recursive spend-again-offline cash.
pub const KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1: &str = "recursive_spend_v1";
/// SDK-facing Kagemusha spend mode for legacy checked pre-fold compact tokens.
pub const KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1: &str = "checked_prefold_v1";
/// Return `true` when this release accepts the Kagemusha aggregation mode.
#[must_use]
pub const fn is_supported_kagemusha_aggregation_mode(mode: u16) -> bool {
    mode == KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
}

/// Return the default SDK Kagemusha spend mode for the available native surface.
///
/// Recursive spend bundles are the default product path when ABI 6 recursive
/// spend init/append/verify/redeem is available. Checked pre-fold remains the
/// compatibility fallback for runtimes that only link the older record-backed
/// compact-token surface.
#[must_use]
pub const fn preferred_kagemusha_offline_spend_mode(
    recursive_spend_available: bool,
) -> &'static str {
    if recursive_spend_available {
        KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    } else {
        KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    }
}

/// Return the stable rejection reason for an unsupported Kagemusha aggregation mode.
#[must_use]
pub const fn unsupported_kagemusha_aggregation_mode_reason(mode: u16) -> &'static str {
    match mode {
        KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1 => {
            "reserved for future in-circuit recursive aggregation; private-hop verifier is not yet composed into compact-token admission"
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
            .is_some_and(is_supported_kagemusha_stark_fri_profile)
}

fn is_supported_kagemusha_stark_fri_profile(profile: &str) -> bool {
    !profile.is_empty()
        && profile
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn is_trusted_setup_kagemusha_backend(backend: &str) -> bool {
    let backend = backend.to_ascii_lowercase();
    let backend = backend.as_str();
    has_trusted_setup_kagemusha_backend_segment(backend)
        || has_trusted_setup_kagemusha_backend_compact_label(backend)
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
    const TRUSTED_SETUP_SEGMENTS: &[&str] = &[
        "groth16",
        "kzg",
        "bn254",
        "bn256",
        "bls12",
        "srs",
        "crs",
        "ptau",
        "ceremony",
        "powersoftau",
    ];
    backend
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|segment| TRUSTED_SETUP_SEGMENTS.contains(&segment))
}

fn has_trusted_setup_kagemusha_backend_compact_label(backend: &str) -> bool {
    let compact = backend
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    [
        "groth16",
        "kzg",
        "bn254",
        "bn256",
        "bls12381",
        "bls12",
        "srs",
        "crs",
        "ptau",
        "ceremony",
        "trustedsetup",
        "structuredreferencestring",
        "universalsrs",
        "powersoftau",
    ]
    .iter()
    .any(|token| compact.contains(token))
}

fn is_developer_only_kagemusha_backend(backend: &str) -> bool {
    let backend = backend.to_ascii_lowercase();
    if backend.contains("debug") || backend.contains("mock") {
        return true;
    }
    let compact = backend
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    compact.contains("debug") || compact.contains("mock")
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
    /// An input nullifier and output commitment share the same 32-byte value.
    InputOutputOverlap {
        /// Zero-based hop index where the overlap was detected.
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
    /// Recursive aggregation evidence does not declare the reserved recursive mode.
    RecursiveAggregationEvidenceModeMismatch {
        /// Expected reserved recursive aggregation mode.
        expected: u16,
        /// Aggregation mode carried by the evidence statement.
        actual: u16,
    },
    /// Recursive aggregation evidence witness count does not match the folded hop count.
    RecursiveAggregationWitnessCountMismatch {
        /// Hop count carried by the aggregation statement.
        expected: u32,
        /// Witness count carried by the evidence.
        actual: u32,
    },
    /// Recursive aggregation evidence declares an unsupported verifier-witness profile.
    UnsupportedRecursiveVerifierWitnessProfile {
        /// Expected verifier-witness profile.
        expected: &'static str,
        /// Verifier-witness profile carried by the evidence.
        actual: String,
    },
    /// Recursive aggregation evidence declares an unsupported verifier opening length.
    UnsupportedRecursiveVerifierOpeningLength {
        /// Minimum supported opening length.
        min: u32,
        /// Maximum supported opening length.
        max: u32,
        /// Opening length carried by the evidence.
        actual: u32,
    },
    /// Recursive aggregation evidence declares a non-power-of-two verifier opening length.
    NonPowerOfTwoRecursiveVerifierOpeningLength {
        /// Opening length carried by the evidence.
        actual: u32,
    },
    /// Recursive aggregation evidence carries an all-zero verifier parameter fingerprint.
    ZeroRecursiveVerifierParamsFingerprint,
    /// Recursive aggregation evidence carries an all-zero fixed-window table schedule digest.
    ZeroRecursiveFixedWindowTableScheduleDigest,
    /// Recursive aggregation evidence carries an all-zero fixed-window shared-table manifest digest.
    ZeroRecursiveFixedWindowSharedTableManifestDigest,
    /// Recursive aggregation evidence carries an all-zero fixed-window table-base digest.
    ZeroRecursiveFixedWindowTableBaseDigest,
    /// Recursive aggregation evidence carries an all-zero verifier-witness batch digest.
    ZeroRecursiveVerifierWitnessBatchDigest,
    /// Recursive aggregation proof public inputs use an unsupported domain separator.
    InvalidRecursiveAggregationProofPublicInputDomain {
        /// Expected domain separator.
        expected: &'static str,
        /// Domain separator carried by the recursive proof public inputs.
        actual: String,
    },
    /// Recursive aggregation proof public inputs do not match their evidence.
    RecursiveAggregationProofPublicInputMismatch {
        /// Name of the mismatched public-input field.
        field: &'static str,
    },
    /// Recursive aggregation proof is not bound to its canonical public inputs.
    RecursiveAggregationProofPublicInputHashMismatch {
        /// Hash computed from the recursive proof public inputs.
        expected: Hash,
        /// Hash declared by the recursive proof.
        actual: Hash,
    },
    /// Recursive aggregation proof backend does not match its verifier-key id.
    RecursiveAggregationProofBackendMismatch {
        /// Proof backend label.
        proof_backend: String,
        /// Verifier-key backend label.
        verifier_key_backend: String,
    },
    /// Recursive aggregation proof verifier-key id does not use the canonical circuit id.
    RecursiveAggregationProofCircuitIdMismatch {
        /// Expected circuit id.
        expected: &'static str,
        /// Actual circuit id.
        actual: String,
    },
    /// Recursive aggregation proof carries invalid production proof metadata.
    InvalidRecursiveAggregationProof {
        /// Name of the invalid recursive aggregation proof field.
        field: &'static str,
    },
    /// Recursive spend accumulator uses an unsupported domain separator.
    InvalidRecursiveSpendAccumulatorDomain {
        /// Expected domain separator.
        expected: &'static str,
        /// Actual domain separator.
        actual: String,
    },
    /// Recursive spend accumulator field is not bound to its proof public inputs.
    RecursiveSpendPublicInputMismatch {
        /// Name of the mismatched field.
        field: &'static str,
    },
    /// Recursive spend accumulator has invalid top-up anchor nullifiers.
    InvalidRecursiveSpendTopupAnchor {
        /// Name of the invalid anchor field.
        field: &'static str,
    },
    /// Recursive spend accumulator has an invalid current spendable note.
    InvalidRecursiveSpendNote {
        /// Name of the invalid note field.
        field: &'static str,
    },
    /// Recursive spend redeem request has an invalid recursive spend proof attachment.
    InvalidRecursiveSpendProof {
        /// Name of the invalid recursive spend proof field.
        field: &'static str,
    },
    /// Recursive spend redeem request has an invalid final redeem proof attachment.
    InvalidRecursiveSpendRedeemProof {
        /// Name of the invalid redeem-proof field.
        field: &'static str,
    },
    /// Recursive spend append changed chain id.
    RecursiveSpendChainMismatch,
    /// Recursive spend append changed asset id.
    RecursiveSpendAssetMismatch,
    /// Recursive spend append does not continue from the previous final root.
    RecursiveSpendRootMismatch,
    /// Recursive spend append did not consume the previous spendable note nullifier.
    RecursiveSpendMissingPreviousNullifier,
    /// Recursive spend append introduced an input other than the previous spendable note.
    RecursiveSpendUnexpectedAppendInput,
    /// Recursive spend state does not bind the declared current note commitment.
    RecursiveSpendMissingCurrentNoteCommitment,
    /// Recursive spend verifier context changed across an append.
    RecursiveSpendVerifierContextMismatch {
        /// Name of the mismatched verifier context field.
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
            Self::InputOutputOverlap { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} reuses an input nullifier as an output commitment"
            ),
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
            Self::RecursiveAggregationEvidenceModeMismatch { expected, actual } => write!(
                f,
                "Kagemusha recursive aggregation evidence mode must be {expected} (found {actual})"
            ),
            Self::RecursiveAggregationWitnessCountMismatch { expected, actual } => write!(
                f,
                "Kagemusha recursive aggregation evidence witness count must be {expected} (found {actual})"
            ),
            Self::UnsupportedRecursiveVerifierWitnessProfile { expected, actual } => write!(
                f,
                "Kagemusha recursive aggregation verifier-witness profile must be {expected:?} (found {actual:?})"
            ),
            Self::UnsupportedRecursiveVerifierOpeningLength { min, max, actual } => write!(
                f,
                "Kagemusha recursive aggregation verifier opening length must be {min}..={max} (found {actual})"
            ),
            Self::NonPowerOfTwoRecursiveVerifierOpeningLength { actual } => write!(
                f,
                "Kagemusha recursive aggregation verifier opening length must be a power of two (found {actual})"
            ),
            Self::ZeroRecursiveVerifierParamsFingerprint => write!(
                f,
                "Kagemusha recursive aggregation verifier parameter fingerprint must be non-zero"
            ),
            Self::ZeroRecursiveFixedWindowTableScheduleDigest => write!(
                f,
                "Kagemusha recursive aggregation fixed-window table schedule digest must be non-zero"
            ),
            Self::ZeroRecursiveFixedWindowSharedTableManifestDigest => write!(
                f,
                "Kagemusha recursive aggregation fixed-window shared-table manifest digest must be non-zero"
            ),
            Self::ZeroRecursiveFixedWindowTableBaseDigest => write!(
                f,
                "Kagemusha recursive aggregation fixed-window table base digest must be non-zero"
            ),
            Self::ZeroRecursiveVerifierWitnessBatchDigest => write!(
                f,
                "Kagemusha recursive aggregation verifier-witness batch digest must be non-zero"
            ),
            Self::InvalidRecursiveAggregationProofPublicInputDomain { expected, actual } => write!(
                f,
                "Kagemusha recursive aggregation proof public-input domain must be {expected:?} (found {actual:?})"
            ),
            Self::RecursiveAggregationProofPublicInputMismatch { field } => write!(
                f,
                "Kagemusha recursive aggregation proof public input {field:?} does not match the evidence"
            ),
            Self::RecursiveAggregationProofPublicInputHashMismatch { .. } => write!(
                f,
                "Kagemusha recursive aggregation proof public-input hash does not match its public inputs"
            ),
            Self::RecursiveAggregationProofBackendMismatch {
                proof_backend,
                verifier_key_backend,
            } => write!(
                f,
                "Kagemusha recursive aggregation proof backend {proof_backend:?} must match verifier-key backend {verifier_key_backend:?}"
            ),
            Self::RecursiveAggregationProofCircuitIdMismatch { expected, actual } => write!(
                f,
                "Kagemusha recursive aggregation proof circuit id must be {expected:?} (found {actual:?})"
            ),
            Self::InvalidRecursiveAggregationProof { field } => write!(
                f,
                "Kagemusha recursive aggregation proof field {field:?} is invalid"
            ),
            Self::InvalidRecursiveSpendAccumulatorDomain { expected, actual } => write!(
                f,
                "Kagemusha recursive spend accumulator domain must be {expected:?} (found {actual:?})"
            ),
            Self::RecursiveSpendPublicInputMismatch { field } => write!(
                f,
                "Kagemusha recursive spend accumulator field {field:?} is not bound to the recursive proof public inputs"
            ),
            Self::InvalidRecursiveSpendTopupAnchor { field } => {
                write!(
                    f,
                    "Kagemusha recursive spend top-up anchor field {field:?} is invalid"
                )
            }
            Self::InvalidRecursiveSpendNote { field } => {
                write!(
                    f,
                    "Kagemusha recursive spend current note field {field:?} is invalid"
                )
            }
            Self::InvalidRecursiveSpendProof { field } => write!(
                f,
                "Kagemusha recursive spend proof field {field:?} is invalid"
            ),
            Self::InvalidRecursiveSpendRedeemProof { field } => write!(
                f,
                "Kagemusha recursive spend redeem proof field {field:?} is invalid"
            ),
            Self::RecursiveSpendChainMismatch => {
                write!(f, "Kagemusha recursive spend append changed chain id")
            }
            Self::RecursiveSpendAssetMismatch => {
                write!(f, "Kagemusha recursive spend append changed asset id")
            }
            Self::RecursiveSpendRootMismatch => write!(
                f,
                "Kagemusha recursive spend append does not continue from the previous final root"
            ),
            Self::RecursiveSpendMissingPreviousNullifier => write!(
                f,
                "Kagemusha recursive spend append does not consume the previous spendable note nullifier"
            ),
            Self::RecursiveSpendUnexpectedAppendInput => write!(
                f,
                "Kagemusha recursive spend append may only consume the previous spendable note nullifier"
            ),
            Self::RecursiveSpendMissingCurrentNoteCommitment => write!(
                f,
                "Kagemusha recursive spend state does not create the declared current note commitment"
            ),
            Self::RecursiveSpendVerifierContextMismatch { field } => write!(
                f,
                "Kagemusha recursive spend verifier context field {field:?} changed across append"
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

    /// Reserved-mode evidence binding a native verifier-witness batch to an aggregation transcript.
    ///
    /// This is not chain-accepted compact-token state in this release. It is the
    /// canonical wallet/prover-side statement that mode `2` recursive
    /// aggregation work can use to bind host preflight of native Pallas IPA
    /// verifier witnesses to the same ordered hop transcript that mode `1`
    /// exposes through checked pre-fold public inputs.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveAggregationEvidence {
        /// Canonical ordered aggregation transcript statement using reserved recursive mode `2`.
        pub aggregation_statement: KagemushaPoseidonAggregationTranscriptStatement,
        /// Number of native verifier witnesses validated into the batch.
        pub verifier_witness_count: u32,
        /// Canonical no-trusted-setup verifier-witness profile used by the native batch preflight.
        pub verifier_witness_profile: String,
        /// Pallas IPA opening vector length used by the native verifier-witness batch.
        pub verifier_opening_len: u32,
        /// Transparent parameter fingerprint used by the native verifier-witness batch.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_params_fingerprint: [u8; 32],
        /// Poseidon2 digest of the deterministic shared fixed-window table schedule.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_schedule_digest: [u8; 32],
        /// Poseidon2 digest of the compressed shared fixed-window table row manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_shared_table_manifest_digest: [u8; 32],
        /// Poseidon2 digest of the ordered fixed-window table bases validated by native preflight.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_base_digest: [u8; 32],
        /// Domain-separated digest emitted by the native verifier-witness batch preflight.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_witness_batch_digest: [u8; 32],
    }

    /// Public inputs that a recursive aggregation proof must expose.
    ///
    /// The values are derived from [`KagemushaRecursiveAggregationEvidence`] and
    /// keep a future mode-2 recursive verifier proof bound to the exact
    /// no-trusted-setup verifier-witness batch, opening width, and ordered
    /// aggregation transcript it claims to compress.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveAggregationProofPublicInputs {
        /// Domain separator for recursive aggregation proof public inputs.
        pub domain: String,
        /// Poseidon2 digest of the canonical recursive aggregation evidence.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub evidence_digest: [u8; 32],
        /// Poseidon2 digest of the ordered folded-hop aggregation transcript.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub aggregation_transcript_digest: [u8; 32],
        /// Transparent parameter fingerprint used by the native verifier-witness batch.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_params_fingerprint: [u8; 32],
        /// Poseidon2 digest of the deterministic shared fixed-window table schedule.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_schedule_digest: [u8; 32],
        /// Poseidon2 digest of the compressed shared fixed-window table row manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_shared_table_manifest_digest: [u8; 32],
        /// Poseidon2 digest of the ordered fixed-window table bases validated by native preflight.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_base_digest: [u8; 32],
        /// Domain-separated digest emitted by the native verifier-witness batch preflight.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_witness_batch_digest: [u8; 32],
        /// Streaming recursive spend proof-chain digest.
        ///
        /// Plain recursive aggregation proofs set this to zero. Recursive
        /// spend proofs set it from `KagemushaRecursiveSpendAccumulatorV1` so
        /// append verifiers can bind the exact previous recursive proof
        /// artifact without carrying prior hop bundles.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recursive_proof_chain_digest: [u8; 32],
        /// Scalar-projection digest emitted by the composed recursive verifier slice.
        ///
        /// Plain recursive aggregation proofs and current spend proofs set this
        /// to zero. The one-hop verifier-slice circuit binds these limbs to the
        /// in-circuit scalar projection of its verifier witness, giving the
        /// complete recursive circuit a stable public channel for that
        /// challenge-binding output.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recursive_verifier_scalar_projection_digest: [u8; 32],
        /// Pallas IPA opening vector length used by the recursive verifier proof.
        pub verifier_opening_len: u32,
        /// Number of native verifier witnesses compressed by the recursive proof.
        pub verifier_witness_count: u32,
        /// Number of folded private hops represented by the aggregation transcript.
        pub hop_count: u32,
    }

    /// Transparent proof claiming one recursive aggregation evidence statement.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveAggregationProof {
        /// Stable verifier key identifier for the recursive aggregation proof circuit.
        pub verifier_key_id: VerifyingKeyId,
        /// Public inputs exposed by the recursive aggregation proof.
        pub public_inputs: KagemushaRecursiveAggregationProofPublicInputs,
        /// Public input commitment hash.
        pub public_inputs_hash: Hash,
        /// Transparent proof payload encoded as an `OpenVerifyEnvelope`.
        pub proof: ProofBox,
    }

    /// Recursive aggregation evidence paired with the proof that claims it.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveAggregationProofBundle {
        /// Canonical host-side evidence statement.
        pub evidence: KagemushaRecursiveAggregationEvidence,
        /// Transparent no-trusted-setup recursive proof bound to `evidence`.
        pub recursive_proof: KagemushaRecursiveAggregationProof,
    }

    /// Spendable note descriptor carried by recursive Kagemusha offline cash.
    ///
    /// This is the holder-facing constant-size descriptor needed to receive,
    /// store, re-spend, and later redeem the current cash state. It intentionally
    /// does not expose prior hop proofs.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaSpendableNoteDescriptorV1 {
        /// Current spendable note commitment created by the latest offline hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub note_commitment: [u8; 32],
        /// Nullifier that must be consumed by the next offline hop or final redeem.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub spend_nullifier: [u8; 32],
        /// Public amount represented by this note.
        pub amount: Numeric,
    }

    /// Constant-size recursive Kagemusha spend accumulator.
    ///
    /// The accumulator is the D2D payload state for recursive Kagemusha cash. It
    /// keeps only streaming commitments to prior hops, the public verifier
    /// context, the current spendable note descriptor, and chain/asset/root
    /// bindings. Prior hop proofs and verifier witnesses are not carried.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendAccumulatorV1 {
        /// Domain separator for recursive spend accumulator state.
        pub domain: String,
        /// Chain id that scopes the spendable state.
        pub chain_id: ChainId,
        /// Shielded asset definition id.
        pub asset: AssetDefinitionId,
        /// Root before the first offline hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub initial_root: [u8; 32],
        /// Root after the latest offline hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub final_root: [u8; 32],
        /// First-hop input nullifiers that anchor this recursive cash to its online-to-offline top-up lineage.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub topup_anchor_nullifiers: Vec<[u8; 32]>,
        /// Number of offline hops accumulated.
        pub hop_count: u32,
        /// Streaming digest of ordered hop semantics.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub lineage_digest: [u8; 32],
        /// Streaming digest used as the recursive proof aggregation transcript public input.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub aggregation_transcript_digest: [u8; 32],
        /// Streaming digest of all consumed nullifiers.
        pub nullifier_digest: Hash,
        /// Streaming digest of all output commitments.
        pub output_commitment_digest: Hash,
        /// Streaming host hash of folded hop statements.
        pub fold_digest: Hash,
        /// Streaming digest of recursive proof artifacts consumed by append proofs.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recursive_proof_chain_digest: [u8; 32],
        /// Transparent parameter fingerprint used by the recursive verifier batch.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_params_fingerprint: [u8; 32],
        /// Poseidon2 digest of the deterministic fixed-window table schedule.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_schedule_digest: [u8; 32],
        /// Poseidon2 digest of the shared fixed-window table manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_shared_table_manifest_digest: [u8; 32],
        /// Poseidon2 digest of the fixed-window table bases.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_base_digest: [u8; 32],
        /// Streaming digest of the verifier-witness batch.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_witness_batch_digest: [u8; 32],
        /// Pallas IPA opening vector length used by the recursive proof corridor.
        pub verifier_opening_len: u32,
        /// Current spendable note descriptor.
        pub current_note: KagemushaSpendableNoteDescriptorV1,
    }

    /// Production recursive Kagemusha spend bundle carried between offline holders.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendBundleV1 {
        /// Constant-size recursive spend accumulator.
        pub accumulator: KagemushaRecursiveSpendAccumulatorV1,
        /// Transparent no-trusted-setup proof bound to the accumulator.
        pub recursive_proof: KagemushaRecursiveAggregationProof,
    }

    /// Bridge request for the first recursive Kagemusha spendable state.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendInitRequestV1 {
        /// One-hop record-backed checked Kagemusha bundle.
        pub record_bundle: KagemushaVerifiedFoldRecordBundle,
        /// Norito archive of `Vec<iroha_zkp_halo2::OpenVerifyEnvelope>`.
        pub pallas_open_envelopes_archive: Vec<u8>,
        /// Spendable note created by the first hop.
        pub current_note: KagemushaSpendableNoteDescriptorV1,
    }

    /// Bridge request for appending one offline hop to recursive Kagemusha cash.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendAppendRequestV1 {
        /// Previous spendable recursive state.
        pub previous_bundle: KagemushaRecursiveSpendBundleV1,
        /// One-hop record-backed checked Kagemusha bundle for the new hop.
        pub record_bundle: KagemushaVerifiedFoldRecordBundle,
        /// Norito archive of `Vec<iroha_zkp_halo2::OpenVerifyEnvelope>`.
        pub pallas_open_envelopes_archive: Vec<u8>,
        /// Spendable note created by the appended hop.
        pub current_note: KagemushaSpendableNoteDescriptorV1,
    }

    /// Full record-backed lineage witness for online recursive spend redemption.
    ///
    /// This is chain-submitted audit material for the current production
    /// admission path. It is intentionally not part of the constant-size D2D
    /// recursive spend bundle; wallets only attach it when converting recursive
    /// offline cash back into public assets.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendLineageWitnessV1 {
        /// Ordered private hop proofs plus verifier records for the full lineage.
        pub record_bundle: KagemushaVerifiedFoldRecordBundle,
        /// Norito archive of `Vec<iroha_zkp_halo2::OpenVerifyEnvelope>`, one envelope per hop.
        pub pallas_open_envelopes_archive: Vec<u8>,
        /// Spendable note descriptor created by each hop, in lineage order.
        pub current_notes: Vec<KagemushaSpendableNoteDescriptorV1>,
        /// Recursive proofs produced after each previous hop, in lineage order.
        ///
        /// For an `n`-hop bundle this contains `n - 1` proofs: proof `0` is
        /// bound to the accumulator after hop `0`, proof `1` to the accumulator
        /// after hop `1`, and so on. The final proof is carried by
        /// [`KagemushaRecursiveSpendBundleV1::recursive_proof`].
        pub previous_recursive_proofs: Vec<KagemushaRecursiveAggregationProof>,
    }

    /// Bridge request for verifying a recursive Kagemusha spend bundle.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendVerifyRequestV1 {
        /// Bundle to verify.
        pub bundle: KagemushaRecursiveSpendBundleV1,
    }

    /// Bridge verification result for recursive Kagemusha spend bundles.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendVerifyResultV1 {
        /// True when all public bindings and backend proof verification passed.
        pub valid: bool,
        /// Hop count carried by the verified bundle.
        pub hop_count: u32,
        /// Norito encoded bundle length, used by SDK/CI payload-size checks.
        pub encoded_bytes: u32,
        /// Stable failure reason for diagnostics; empty on success.
        pub reason: String,
        /// True when the verified bundle is directly admissible for online redemption.
        ///
        /// Offline receivers should use [`Self::valid`] for accept/re-spend
        /// decisions. This field is stricter: current semantic recursive spend
        /// bundles can be offline-valid while still requiring record-backed
        /// lineage witness material at redeem time.
        pub chain_admissible: bool,
        /// Stable chain-admission diagnostic; empty when [`Self::chain_admissible`] is true.
        pub chain_admission_reason: String,
    }

    /// Bridge request for preparing an online recursive Kagemusha redemption.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemRequestV1 {
        /// Final holder's recursive spend bundle.
        pub bundle: KagemushaRecursiveSpendBundleV1,
        /// Recipient public account to credit online.
        pub recipient: AccountId,
        /// Public amount to mint on redemption.
        pub public_amount: u128,
        /// Final unshield/redeem proof bound to the current note descriptor.
        pub redeem_proof: ProofAttachment,
        /// Optional record-backed lineage witness required for production minting.
        pub lineage_witness: Option<KagemushaRecursiveSpendLineageWitnessV1>,
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
const KAGEMUSHA_RECURSIVE_SPEND_NULLIFIER_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-nullifiers";
const KAGEMUSHA_RECURSIVE_SPEND_OUTPUT_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-outputs";
const KAGEMUSHA_RECURSIVE_SPEND_FOLD_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-fold-transcript";
/// Canonical public-input schema descriptor for Offline recursive note proofs.
pub const OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"offline_note_recursive","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","proof_mode","input_count","output_count","input_amount_sum","output_amount_sum","input_nullifier_sum_limb0","output_commitment_sum_limb0","key_certificate_payload_hash_limb0","source_or_token_limb0","input_claim_hash_sum_limb0","output_claim_hash_sum_limb0","reserved_zero"]}"#;
/// Canonical public-input schema descriptor for Kagemusha folded proofs.
pub const KAGEMUSHA_FOLDED_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"kagemusha_folded_v1","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","aggregation_mode","hop_count","initial_root_limb0","initial_root_limb1","initial_root_limb2","initial_root_limb3","final_root_limb0","final_root_limb1","final_root_limb2","final_root_limb3","nullifier_digest_limb0","nullifier_digest_limb1","nullifier_digest_limb2","nullifier_digest_limb3","output_commitment_digest_limb0","output_commitment_digest_limb1","output_commitment_digest_limb2","output_commitment_digest_limb3","fold_digest_limb0","fold_digest_limb1","fold_digest_limb2","fold_digest_limb3","aggregation_transcript_digest_limb0","aggregation_transcript_digest_limb1","aggregation_transcript_digest_limb2","aggregation_transcript_digest_limb3"]}"#;
/// Canonical public-input schema descriptor for Kagemusha recursive aggregation proofs.
pub const KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"kagemusha_recursive_aggregation_proof_v1","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","evidence_digest_limb0","evidence_digest_limb1","evidence_digest_limb2","evidence_digest_limb3","aggregation_transcript_digest_limb0","aggregation_transcript_digest_limb1","aggregation_transcript_digest_limb2","aggregation_transcript_digest_limb3","verifier_params_fingerprint_limb0","verifier_params_fingerprint_limb1","verifier_params_fingerprint_limb2","verifier_params_fingerprint_limb3","fixed_window_table_schedule_digest_limb0","fixed_window_table_schedule_digest_limb1","fixed_window_table_schedule_digest_limb2","fixed_window_table_schedule_digest_limb3","fixed_window_shared_table_manifest_digest_limb0","fixed_window_shared_table_manifest_digest_limb1","fixed_window_shared_table_manifest_digest_limb2","fixed_window_shared_table_manifest_digest_limb3","fixed_window_table_base_digest_limb0","fixed_window_table_base_digest_limb1","fixed_window_table_base_digest_limb2","fixed_window_table_base_digest_limb3","verifier_witness_batch_digest_limb0","verifier_witness_batch_digest_limb1","verifier_witness_batch_digest_limb2","verifier_witness_batch_digest_limb3","recursive_proof_chain_digest_limb0","recursive_proof_chain_digest_limb1","recursive_proof_chain_digest_limb2","recursive_proof_chain_digest_limb3","recursive_verifier_scalar_projection_digest_limb0","recursive_verifier_scalar_projection_digest_limb1","recursive_verifier_scalar_projection_digest_limb2","recursive_verifier_scalar_projection_digest_limb3","verifier_opening_len","verifier_witness_count","hop_count"]}"#;

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

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveAggregationEvidencePreimage {
    domain: String,
    evidence: KagemushaRecursiveAggregationEvidence,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendAccumulatorDigestPreimage {
    domain: String,
    accumulator: KagemushaRecursiveSpendAccumulatorV1,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendLineageDigestPreimage {
    domain: String,
    previous_lineage_digest: Option<[u8; Hash::LENGTH]>,
    chain_id: ChainId,
    asset: AssetDefinitionId,
    hop_index: u32,
    step: KagemushaPoseidonAggregationStepStatement,
    current_note: KagemushaSpendableNoteDescriptorV1,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendVerifierBatchDigestPreimage {
    domain: String,
    previous_verifier_witness_batch_digest: Option<[u8; Hash::LENGTH]>,
    hop_index: u32,
    hop_verifier_witness_batch_digest: [u8; Hash::LENGTH],
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendFixedWindowTableBaseDigestPreimage {
    domain: String,
    previous_fixed_window_table_base_digest: Option<[u8; Hash::LENGTH]>,
    hop_index: u32,
    hop_fixed_window_table_base_digest: [u8; Hash::LENGTH],
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendProofArtifactDigestPreimage {
    domain: String,
    recursive_proof: KagemushaRecursiveAggregationProof,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendProofChainDigestPreimage {
    domain: String,
    previous_recursive_proof_chain_digest: Option<[u8; Hash::LENGTH]>,
    previous_recursive_proof_artifact_digest: Option<[u8; Hash::LENGTH]>,
    previous_recursive_proof_public_inputs_hash: Option<Hash>,
    hop_index: u32,
    current_hop_proof_hash: Hash,
    current_hop_proof_public_inputs_digest: [u8; Hash::LENGTH],
    current_hop_verifier_witness_batch_digest: [u8; Hash::LENGTH],
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

/// Return the registry schema hash required for Kagemusha recursive aggregation proof verifiers.
#[must_use]
pub fn kagemusha_recursive_aggregation_proof_public_inputs_schema_hash() -> [u8; Hash::LENGTH] {
    Hash::new(KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA).into()
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
/// hashes. The digest accepts both checked pre-fold and reserved recursive
/// aggregation modes so recursive evidence can bind the same transcript shape;
/// compact-token admission still rejects reserved modes through
/// [`KagemushaFoldedPublicInputs::validate_supported_context`].
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the statement is non-canonical or cannot
/// be encoded with Norito.
pub fn kagemusha_poseidon_aggregation_transcript_digest(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    kagemusha_poseidon_aggregation_transcript_shape_digest(statement)
}

fn kagemusha_poseidon_aggregation_transcript_shape_digest(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    validate_kagemusha_hashable_aggregation_transcript_statement(statement)?;
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

fn validate_kagemusha_hashable_aggregation_transcript_statement(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<(), KagemushaFoldError> {
    match statement.aggregation_mode {
        KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
        | KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1 => {}
        actual => {
            return Err(KagemushaFoldError::UnsupportedAggregationMode {
                expected: KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1,
                actual,
                reason: unsupported_kagemusha_aggregation_mode_reason(actual),
            });
        }
    }
    validate_kagemusha_aggregation_transcript_statement_shape(statement)
}

fn validate_kagemusha_aggregation_transcript_statement_shape(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<(), KagemushaFoldError> {
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
            if all_outputs.contains(nullifier) {
                return Err(KagemushaFoldError::InputOutputOverlap { hop_index });
            }
            if !all_inputs.insert(*nullifier) {
                return Err(KagemushaFoldError::DuplicateInputNullifier { hop_index });
            }
        }
        for commitment in &step.output_commitments {
            if all_inputs.contains(commitment) {
                return Err(KagemushaFoldError::InputOutputOverlap { hop_index });
            }
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

/// Validate reserved-mode recursive aggregation evidence.
///
/// This checks only the canonical host-side evidence shape. It does not make
/// aggregation mode `2` supported for compact-token admission in this release.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the evidence does not declare reserved
/// recursive mode `2`, its hop transcript is non-canonical, its witness count
/// does not match the hop count, its verifier-witness profile or opening length
/// is unsupported, or its verifier parameter, schedule, shared-table manifest,
/// table-base, or batch digest fields are all-zero.
pub fn validate_kagemusha_recursive_aggregation_evidence(
    evidence: &KagemushaRecursiveAggregationEvidence,
) -> Result<(), KagemushaFoldError> {
    if evidence.aggregation_statement.aggregation_mode
        != KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
    {
        return Err(
            KagemushaFoldError::RecursiveAggregationEvidenceModeMismatch {
                expected: KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1,
                actual: evidence.aggregation_statement.aggregation_mode,
            },
        );
    }
    validate_kagemusha_aggregation_transcript_statement_shape(&evidence.aggregation_statement)?;
    if evidence.verifier_witness_count != evidence.aggregation_statement.hop_count {
        return Err(
            KagemushaFoldError::RecursiveAggregationWitnessCountMismatch {
                expected: evidence.aggregation_statement.hop_count,
                actual: evidence.verifier_witness_count,
            },
        );
    }
    if evidence.verifier_witness_profile != KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1 {
        return Err(
            KagemushaFoldError::UnsupportedRecursiveVerifierWitnessProfile {
                expected: KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1,
                actual: evidence.verifier_witness_profile.clone(),
            },
        );
    }
    validate_kagemusha_recursive_verifier_opening_len(evidence.verifier_opening_len)?;
    if evidence.verifier_params_fingerprint == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveVerifierParamsFingerprint);
    }
    if evidence.fixed_window_table_schedule_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableScheduleDigest);
    }
    if evidence.fixed_window_shared_table_manifest_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveFixedWindowSharedTableManifestDigest);
    }
    if evidence.fixed_window_table_base_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableBaseDigest);
    }
    if evidence.verifier_witness_batch_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveVerifierWitnessBatchDigest);
    }
    Ok(())
}

/// Validate the Pallas IPA opening length accepted by reserved recursive evidence.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when `opening_len` is outside the bounded
/// power-of-two corridor used by the first recursive verifier profile.
pub fn validate_kagemusha_recursive_verifier_opening_len(
    opening_len: u32,
) -> Result<(), KagemushaFoldError> {
    if !(KAGEMUSHA_RECURSIVE_PALLAS_IPA_BATCH_MIN_LEN
        ..=KAGEMUSHA_RECURSIVE_PALLAS_IPA_BATCH_MAX_LEN)
        .contains(&opening_len)
    {
        return Err(
            KagemushaFoldError::UnsupportedRecursiveVerifierOpeningLength {
                min: KAGEMUSHA_RECURSIVE_PALLAS_IPA_BATCH_MIN_LEN,
                max: KAGEMUSHA_RECURSIVE_PALLAS_IPA_BATCH_MAX_LEN,
                actual: opening_len,
            },
        );
    }
    if !opening_len.is_power_of_two() {
        return Err(
            KagemushaFoldError::NonPowerOfTwoRecursiveVerifierOpeningLength {
                actual: opening_len,
            },
        );
    }
    Ok(())
}

/// Return the canonical Poseidon2 digest for reserved-mode recursive aggregation evidence.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the evidence is non-canonical or cannot
/// be encoded with Norito.
pub fn kagemusha_recursive_aggregation_evidence_digest(
    evidence: &KagemushaRecursiveAggregationEvidence,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    validate_kagemusha_recursive_aggregation_evidence(evidence)?;
    kagemusha_poseidon_preimage(&KagemushaRecursiveAggregationEvidencePreimage {
        domain: KAGEMUSHA_RECURSIVE_AGGREGATION_EVIDENCE_DOMAIN.to_owned(),
        evidence: evidence.clone(),
    })
}

/// Derive recursive aggregation proof public inputs from canonical evidence.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the evidence is non-canonical or when the
/// derived public-input payload is not valid for the recursive proof corridor.
pub fn kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(
    evidence: &KagemushaRecursiveAggregationEvidence,
) -> Result<KagemushaRecursiveAggregationProofPublicInputs, KagemushaFoldError> {
    validate_kagemusha_recursive_aggregation_evidence(evidence)?;
    let public_inputs = KagemushaRecursiveAggregationProofPublicInputs {
        domain: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_DOMAIN.to_owned(),
        evidence_digest: kagemusha_recursive_aggregation_evidence_digest(evidence)?,
        aggregation_transcript_digest: kagemusha_poseidon_aggregation_transcript_shape_digest(
            &evidence.aggregation_statement,
        )?,
        verifier_params_fingerprint: evidence.verifier_params_fingerprint,
        fixed_window_table_schedule_digest: evidence.fixed_window_table_schedule_digest,
        fixed_window_shared_table_manifest_digest: evidence
            .fixed_window_shared_table_manifest_digest,
        fixed_window_table_base_digest: evidence.fixed_window_table_base_digest,
        verifier_witness_batch_digest: evidence.verifier_witness_batch_digest,
        recursive_proof_chain_digest: [0u8; Hash::LENGTH],
        recursive_verifier_scalar_projection_digest: [0u8; Hash::LENGTH],
        verifier_opening_len: evidence.verifier_opening_len,
        verifier_witness_count: evidence.verifier_witness_count,
        hop_count: evidence.aggregation_statement.hop_count,
    };
    public_inputs.validate_context()?;
    Ok(public_inputs)
}

impl KagemushaRecursiveAggregationProofPublicInputs {
    /// Validate the recursive aggregation proof public-input context.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the domain, digest fields, opening
    /// length, or counts are outside the production recursive proof corridor.
    pub fn validate_context(&self) -> Result<(), KagemushaFoldError> {
        if self.domain != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_DOMAIN {
            return Err(
                KagemushaFoldError::InvalidRecursiveAggregationProofPublicInputDomain {
                    expected: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_DOMAIN,
                    actual: self.domain.clone(),
                },
            );
        }
        if self.evidence_digest == [0u8; Hash::LENGTH] {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "evidence_digest",
                },
            );
        }
        if self.aggregation_transcript_digest == [0u8; Hash::LENGTH] {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "aggregation_transcript_digest",
                },
            );
        }
        if self.verifier_params_fingerprint == [0u8; Hash::LENGTH] {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "verifier_params_fingerprint",
                },
            );
        }
        if self.fixed_window_table_schedule_digest == [0u8; Hash::LENGTH] {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "fixed_window_table_schedule_digest",
                },
            );
        }
        if self.fixed_window_shared_table_manifest_digest == [0u8; Hash::LENGTH] {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "fixed_window_shared_table_manifest_digest",
                },
            );
        }
        if self.fixed_window_table_base_digest == [0u8; Hash::LENGTH] {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "fixed_window_table_base_digest",
                },
            );
        }
        if self.verifier_witness_batch_digest == [0u8; Hash::LENGTH] {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "verifier_witness_batch_digest",
                },
            );
        }
        validate_kagemusha_recursive_verifier_opening_len(self.verifier_opening_len)?;
        if self.hop_count == 0 {
            return Err(KagemushaFoldError::Empty);
        }
        let hop_count =
            usize::try_from(self.hop_count).map_err(|_| KagemushaFoldError::TooManyHops {
                max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
                actual: usize::MAX,
            })?;
        if hop_count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
            return Err(KagemushaFoldError::TooManyHops {
                max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
                actual: hop_count,
            });
        }
        if self.verifier_witness_count != self.hop_count {
            return Err(
                KagemushaFoldError::RecursiveAggregationWitnessCountMismatch {
                    expected: self.hop_count,
                    actual: self.verifier_witness_count,
                },
            );
        }
        Ok(())
    }

    /// Deterministic hash that the recursive aggregation proof must expose.
    ///
    /// # Errors
    ///
    /// Returns an error when the public-input payload cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl KagemushaRecursiveAggregationProof {
    /// Validate that the proof envelope metadata is bound to its public inputs.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the proof uses a non-production
    /// backend, is not the canonical Halo2 IPA recursive proof backend, carries
    /// an empty proof payload, has mismatched backend/circuit metadata, or has
    /// an incorrect public-input hash.
    pub fn validate_public_input_binding(&self) -> Result<(), KagemushaFoldError> {
        self.public_inputs.validate_context()?;
        if !is_supported_kagemusha_proof_backend(&self.proof.backend) {
            return Err(KagemushaFoldError::UnsupportedProofBackend {
                backend: self.proof.backend.clone(),
            });
        }
        if !is_supported_kagemusha_proof_backend(&self.verifier_key_id.backend) {
            return Err(KagemushaFoldError::UnsupportedProofBackend {
                backend: self.verifier_key_id.backend.clone(),
            });
        }
        if self.proof.backend != self.verifier_key_id.backend {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofBackendMismatch {
                    proof_backend: self.proof.backend.clone(),
                    verifier_key_backend: self.verifier_key_id.backend.clone(),
                },
            );
        }
        if self.proof.backend != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND {
            return Err(KagemushaFoldError::InvalidRecursiveAggregationProof {
                field: "proof.backend",
            });
        }
        if self.proof.bytes.is_empty() {
            return Err(KagemushaFoldError::InvalidRecursiveAggregationProof {
                field: "proof.bytes",
            });
        }
        if self.verifier_key_id.name != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofCircuitIdMismatch {
                    expected: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                    actual: self.verifier_key_id.name.clone(),
                },
            );
        }
        let expected = self.public_inputs.public_inputs_hash()?;
        if self.public_inputs_hash != expected {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputHashMismatch {
                    expected,
                    actual: self.public_inputs_hash,
                },
            );
        }
        Ok(())
    }
}

impl KagemushaRecursiveAggregationProofBundle {
    /// Validate that recursive proof public inputs are derived from this evidence.
    ///
    /// This still does not make aggregation mode `2` accepted for compact-token
    /// admission. It provides the canonical proof-carrying surface that a future
    /// recursive verifier can check before backend proof verification.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when evidence, proof metadata, or any
    /// redundant public-input field is not canonical.
    pub fn validate_evidence_binding(&self) -> Result<(), KagemushaFoldError> {
        let expected =
            kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(&self.evidence)?;
        self.recursive_proof.validate_public_input_binding()?;
        validate_kagemusha_recursive_aggregation_proof_public_input_parity(
            &expected,
            &self.recursive_proof.public_inputs,
        )
    }
}

fn validate_kagemusha_recursive_aggregation_proof_public_input_parity(
    expected: &KagemushaRecursiveAggregationProofPublicInputs,
    actual: &KagemushaRecursiveAggregationProofPublicInputs,
) -> Result<(), KagemushaFoldError> {
    macro_rules! ensure_field {
        ($field:ident) => {
            if actual.$field != expected.$field {
                return Err(
                    KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                        field: stringify!($field),
                    },
                );
            }
        };
    }
    ensure_field!(domain);
    ensure_field!(evidence_digest);
    ensure_field!(aggregation_transcript_digest);
    ensure_field!(verifier_params_fingerprint);
    ensure_field!(fixed_window_table_schedule_digest);
    ensure_field!(fixed_window_shared_table_manifest_digest);
    ensure_field!(fixed_window_table_base_digest);
    ensure_field!(verifier_witness_batch_digest);
    ensure_field!(recursive_proof_chain_digest);
    ensure_field!(recursive_verifier_scalar_projection_digest);
    ensure_field!(verifier_opening_len);
    ensure_field!(verifier_witness_count);
    ensure_field!(hop_count);
    Ok(())
}

fn validate_kagemusha_recursive_spend_note(
    note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<(), KagemushaFoldError> {
    if note.note_commitment == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "note_commitment",
        });
    }
    if note.spend_nullifier == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "spend_nullifier",
        });
    }
    if note.note_commitment == note.spend_nullifier {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "spend_nullifier",
        });
    }
    if note.amount.is_zero()
        || note.amount.scale() != 0
        || note.amount.try_mantissa_u128().is_none()
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" });
    }
    Ok(())
}

fn validate_kagemusha_recursive_spend_topup_anchor_nullifiers(
    nullifiers: &[[u8; Hash::LENGTH]],
) -> Result<(), KagemushaFoldError> {
    if nullifiers.is_empty() || nullifiers.len() > KAGEMUSHA_FOLD_STEP_MAX_INPUTS {
        return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
            field: "topup_anchor_nullifiers",
        });
    }
    if nullifiers
        .iter()
        .any(|nullifier| *nullifier == [0u8; Hash::LENGTH])
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
            field: "topup_anchor_nullifiers",
        });
    }
    if nullifiers.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
            field: "topup_anchor_nullifiers",
        });
    }
    Ok(())
}

fn hash_bytes_from_hash(hash: Hash) -> [u8; Hash::LENGTH] {
    hash.into()
}

fn kagemusha_recursive_spend_step_statement(
    hop_index: u32,
    step: &KagemushaFoldStep,
) -> Result<KagemushaPoseidonAggregationStepStatement, KagemushaFoldError> {
    let hop_index_usize = usize::try_from(hop_index).unwrap_or(usize::MAX);
    validate_kagemusha_verifier_key_id(hop_index_usize, &step.verifier_key_id)?;
    validate_kagemusha_step_shape_and_sets(
        hop_index_usize,
        &step.input_nullifiers,
        &step.output_commitments,
    )?;
    validate_kagemusha_step_digest_bindings(
        hop_index_usize,
        step.proof_public_inputs_digest,
        step.verifier_key_commitment,
        step.verifier_key_poseidon_digest,
    )?;
    validate_kagemusha_fold_root("root_before", step.root_before)?;
    validate_kagemusha_fold_root("root_after", step.root_after)?;
    validate_kagemusha_root_transition(hop_index_usize, step.root_before, step.root_after)?;

    let mut input_nullifiers = step.input_nullifiers.clone();
    input_nullifiers.sort_unstable();
    let mut output_commitments = step.output_commitments.clone();
    output_commitments.sort_unstable();
    validate_kagemusha_canonical_set_order(
        hop_index_usize,
        &input_nullifiers,
        &output_commitments,
    )?;

    Ok(KagemushaPoseidonAggregationStepStatement {
        hop_index,
        root_before: step.root_before,
        input_nullifiers,
        output_commitments,
        root_after: step.root_after,
        proof_hash: step.proof_hash,
        proof_public_inputs_digest: step.proof_public_inputs_digest,
        verifier_key_id: step.verifier_key_id.clone(),
        verifier_key_commitment: step.verifier_key_commitment,
        verifier_key_poseidon_digest: step.verifier_key_poseidon_digest,
    })
}

fn kagemusha_recursive_spend_lineage_digest(
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    hop_index: u32,
    step: &KagemushaFoldStep,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    let step = kagemusha_recursive_spend_step_statement(hop_index, step)?;
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendLineageDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_DIGEST_DOMAIN.to_owned(),
        previous_lineage_digest: previous.map(|accumulator| accumulator.lineage_digest),
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        hop_index,
        step,
        current_note: current_note.clone(),
    })
}

fn kagemusha_recursive_spend_verifier_batch_digest(
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    hop_index: u32,
    hop_verifier_witness_batch_digest: [u8; Hash::LENGTH],
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    if hop_verifier_witness_batch_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveVerifierWitnessBatchDigest);
    }
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendVerifierBatchDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_BATCH_DIGEST_DOMAIN.to_owned(),
        previous_verifier_witness_batch_digest: previous
            .map(|accumulator| accumulator.verifier_witness_batch_digest),
        hop_index,
        hop_verifier_witness_batch_digest,
    })
}

fn kagemusha_recursive_spend_fixed_window_table_base_digest(
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    hop_index: u32,
    hop_fixed_window_table_base_digest: [u8; Hash::LENGTH],
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    if hop_fixed_window_table_base_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableBaseDigest);
    }
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendFixedWindowTableBaseDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_FIXED_WINDOW_TABLE_BASE_DIGEST_DOMAIN.to_owned(),
        previous_fixed_window_table_base_digest: previous
            .map(|accumulator| accumulator.fixed_window_table_base_digest),
        hop_index,
        hop_fixed_window_table_base_digest,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KagemushaRecursiveSpendProofCircuit {
    SemanticAggregation,
    Lineage,
}

fn kagemusha_recursive_spend_proof_circuit(
    verifier_key_id: &VerifyingKeyId,
) -> Result<KagemushaRecursiveSpendProofCircuit, KagemushaFoldError> {
    match verifier_key_id.name.as_str() {
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 => {
            Ok(KagemushaRecursiveSpendProofCircuit::SemanticAggregation)
        }
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 => {
            Ok(KagemushaRecursiveSpendProofCircuit::Lineage)
        }
        _ => Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "verifier_key_id.name",
        }),
    }
}

fn validate_kagemusha_recursive_spend_proof_public_input_binding(
    recursive_proof: &KagemushaRecursiveAggregationProof,
) -> Result<KagemushaRecursiveSpendProofCircuit, KagemushaFoldError> {
    recursive_proof.public_inputs.validate_context()?;
    if !is_supported_kagemusha_proof_backend(&recursive_proof.proof.backend) {
        return Err(KagemushaFoldError::UnsupportedProofBackend {
            backend: recursive_proof.proof.backend.clone(),
        });
    }
    if !is_supported_kagemusha_proof_backend(&recursive_proof.verifier_key_id.backend) {
        return Err(KagemushaFoldError::UnsupportedProofBackend {
            backend: recursive_proof.verifier_key_id.backend.clone(),
        });
    }
    if recursive_proof.proof.backend != recursive_proof.verifier_key_id.backend {
        return Err(
            KagemushaFoldError::RecursiveAggregationProofBackendMismatch {
                proof_backend: recursive_proof.proof.backend.clone(),
                verifier_key_backend: recursive_proof.verifier_key_id.backend.clone(),
            },
        );
    }
    if recursive_proof.proof.backend != KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "proof.backend",
        });
    }
    if recursive_proof.proof.bytes.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "proof.bytes",
        });
    }
    let circuit = kagemusha_recursive_spend_proof_circuit(&recursive_proof.verifier_key_id)?;
    let expected_hash = recursive_proof.public_inputs.public_inputs_hash()?;
    if recursive_proof.public_inputs_hash != expected_hash {
        return Err(
            KagemushaFoldError::RecursiveAggregationProofPublicInputHashMismatch {
                expected: expected_hash,
                actual: recursive_proof.public_inputs_hash,
            },
        );
    }
    Ok(circuit)
}

fn expected_kagemusha_recursive_spend_public_inputs_for_proof(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
    recursive_proof: &KagemushaRecursiveAggregationProof,
    circuit: KagemushaRecursiveSpendProofCircuit,
) -> Result<KagemushaRecursiveAggregationProofPublicInputs, KagemushaFoldError> {
    let mut expected = accumulator.recursive_public_inputs()?;
    match circuit {
        KagemushaRecursiveSpendProofCircuit::SemanticAggregation => {}
        KagemushaRecursiveSpendProofCircuit::Lineage => {
            let scalar_projection = recursive_proof
                .public_inputs
                .recursive_verifier_scalar_projection_digest;
            if scalar_projection == [0u8; Hash::LENGTH] {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "recursive_verifier_scalar_projection_digest",
                });
            }
            expected.recursive_verifier_scalar_projection_digest = scalar_projection;
        }
    }
    Ok(expected)
}

fn ensure_recursive_spend_previous_proof_matches(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    previous_recursive_proof: &KagemushaRecursiveAggregationProof,
) -> Result<(), KagemushaFoldError> {
    let circuit =
        validate_kagemusha_recursive_spend_proof_public_input_binding(previous_recursive_proof)?;
    let expected = expected_kagemusha_recursive_spend_public_inputs_for_proof(
        previous,
        previous_recursive_proof,
        circuit,
    )?;
    macro_rules! ensure_field {
        ($field:ident) => {
            if previous_recursive_proof.public_inputs.$field != expected.$field {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                    field: concat!("previous_recursive_proof.", stringify!($field)),
                });
            }
        };
    }
    ensure_field!(domain);
    ensure_field!(evidence_digest);
    ensure_field!(aggregation_transcript_digest);
    ensure_field!(verifier_params_fingerprint);
    ensure_field!(fixed_window_table_schedule_digest);
    ensure_field!(fixed_window_shared_table_manifest_digest);
    ensure_field!(fixed_window_table_base_digest);
    ensure_field!(verifier_witness_batch_digest);
    ensure_field!(recursive_proof_chain_digest);
    ensure_field!(recursive_verifier_scalar_projection_digest);
    ensure_field!(verifier_opening_len);
    ensure_field!(verifier_witness_count);
    ensure_field!(hop_count);
    if previous_recursive_proof.public_inputs_hash != expected.public_inputs_hash()? {
        return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
            field: "previous_recursive_proof.public_inputs_hash",
        });
    }
    Ok(())
}

fn kagemusha_recursive_spend_proof_artifact_digest(
    recursive_proof: &KagemushaRecursiveAggregationProof,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    validate_kagemusha_recursive_spend_proof_public_input_binding(recursive_proof)?;
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendProofArtifactDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_PROOF_ARTIFACT_DIGEST_DOMAIN.to_owned(),
        recursive_proof: recursive_proof.clone(),
    })
}

fn kagemusha_recursive_spend_proof_chain_digest(
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    previous_recursive_proof: Option<&KagemushaRecursiveAggregationProof>,
    hop_index: u32,
    step: &KagemushaFoldStep,
    hop_verifier_witness_batch_digest: [u8; Hash::LENGTH],
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    if hop_verifier_witness_batch_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveVerifierWitnessBatchDigest);
    }
    let previous_recursive_proof_artifact_digest = match (previous, previous_recursive_proof) {
        (Some(previous), Some(previous_recursive_proof)) => {
            ensure_recursive_spend_previous_proof_matches(previous, previous_recursive_proof)?;
            Some(kagemusha_recursive_spend_proof_artifact_digest(
                previous_recursive_proof,
            )?)
        }
        (Some(_), None) => {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "previous_recursive_proof",
            });
        }
        (None, Some(_)) => {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "previous_recursive_proof",
            });
        }
        (None, None) => None,
    };
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendProofChainDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_PROOF_CHAIN_DIGEST_DOMAIN.to_owned(),
        previous_recursive_proof_chain_digest: previous
            .map(|accumulator| accumulator.recursive_proof_chain_digest),
        previous_recursive_proof_artifact_digest,
        previous_recursive_proof_public_inputs_hash: previous_recursive_proof
            .map(|recursive_proof| recursive_proof.public_inputs_hash),
        hop_index,
        current_hop_proof_hash: step.proof_hash,
        current_hop_proof_public_inputs_digest: step.proof_public_inputs_digest,
        current_hop_verifier_witness_batch_digest: hop_verifier_witness_batch_digest,
    })
}

/// Return the canonical Poseidon2 digest of a recursive Kagemusha spend accumulator.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the accumulator is malformed or cannot be
/// encoded with Norito.
pub fn kagemusha_recursive_spend_accumulator_digest(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    accumulator.validate_context()?;
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendAccumulatorDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DIGEST_DOMAIN.to_owned(),
        accumulator: accumulator.clone(),
    })
}

/// Derive recursive proof public inputs from a spend accumulator.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the accumulator or derived public-input
/// layout is invalid.
pub fn kagemusha_recursive_spend_public_inputs_from_accumulator(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
) -> Result<KagemushaRecursiveAggregationProofPublicInputs, KagemushaFoldError> {
    accumulator.validate_context()?;
    let public_inputs = KagemushaRecursiveAggregationProofPublicInputs {
        domain: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_DOMAIN.to_owned(),
        evidence_digest: kagemusha_recursive_spend_accumulator_digest(accumulator)?,
        aggregation_transcript_digest: accumulator.aggregation_transcript_digest,
        verifier_params_fingerprint: accumulator.verifier_params_fingerprint,
        fixed_window_table_schedule_digest: accumulator.fixed_window_table_schedule_digest,
        fixed_window_shared_table_manifest_digest: accumulator
            .fixed_window_shared_table_manifest_digest,
        fixed_window_table_base_digest: accumulator.fixed_window_table_base_digest,
        verifier_witness_batch_digest: accumulator.verifier_witness_batch_digest,
        recursive_proof_chain_digest: accumulator.recursive_proof_chain_digest,
        recursive_verifier_scalar_projection_digest: [0u8; Hash::LENGTH],
        verifier_opening_len: accumulator.verifier_opening_len,
        verifier_witness_count: accumulator.hop_count,
        hop_count: accumulator.hop_count,
    };
    public_inputs.validate_context()?;
    Ok(public_inputs)
}

fn ensure_recursive_spend_verifier_context_matches(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    evidence: &KagemushaRecursiveAggregationEvidence,
) -> Result<(), KagemushaFoldError> {
    macro_rules! ensure_field {
        ($field:ident) => {
            if previous.$field != evidence.$field {
                return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                    field: stringify!($field),
                });
            }
        };
    }
    ensure_field!(verifier_opening_len);
    ensure_field!(verifier_params_fingerprint);
    ensure_field!(fixed_window_table_schedule_digest);
    ensure_field!(fixed_window_shared_table_manifest_digest);
    Ok(())
}

fn kagemusha_recursive_spend_accumulator_from_parts(
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    previous_recursive_proof: Option<&KagemushaRecursiveAggregationProof>,
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendAccumulatorV1, KagemushaFoldError> {
    validate_kagemusha_recursive_aggregation_evidence(evidence)?;
    validate_kagemusha_recursive_spend_note(current_note)?;
    if evidence.aggregation_statement.steps.len() != 1 {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: 1,
            actual: evidence.aggregation_statement.hop_count,
        });
    }
    let step_statement = evidence
        .aggregation_statement
        .steps
        .first()
        .expect("validated one-hop evidence");
    let step = KagemushaFoldStep {
        root_before: step_statement.root_before,
        input_nullifiers: step_statement.input_nullifiers.clone(),
        output_commitments: step_statement.output_commitments.clone(),
        root_after: step_statement.root_after,
        proof_hash: step_statement.proof_hash,
        proof_public_inputs_digest: step_statement.proof_public_inputs_digest,
        verifier_key_id: step_statement.verifier_key_id.clone(),
        verifier_key_commitment: step_statement.verifier_key_commitment,
        verifier_key_poseidon_digest: step_statement.verifier_key_poseidon_digest,
    };
    if !step
        .output_commitments
        .iter()
        .any(|commitment| commitment == &current_note.note_commitment)
    {
        return Err(KagemushaFoldError::RecursiveSpendMissingCurrentNoteCommitment);
    }
    if step
        .input_nullifiers
        .iter()
        .any(|nullifier| nullifier == &current_note.spend_nullifier)
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "spend_nullifier",
        });
    }
    if step
        .output_commitments
        .iter()
        .any(|commitment| commitment == &current_note.spend_nullifier)
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "spend_nullifier",
        });
    }

    let chain_id = &evidence.aggregation_statement.chain_id;
    let asset = &evidence.aggregation_statement.asset;
    let hop_index = previous.map_or(0, |accumulator| accumulator.hop_count);
    if let Some(previous) = previous {
        previous.validate_context()?;
        ensure_recursive_spend_verifier_context_matches(previous, evidence)?;
        if previous.chain_id != *chain_id {
            return Err(KagemushaFoldError::RecursiveSpendChainMismatch);
        }
        if previous.asset != *asset {
            return Err(KagemushaFoldError::RecursiveSpendAssetMismatch);
        }
        if current_note.amount != previous.current_note.amount {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" });
        }
        if previous.final_root != step.root_before {
            return Err(KagemushaFoldError::RecursiveSpendRootMismatch);
        }
        if !step
            .input_nullifiers
            .iter()
            .any(|nullifier| nullifier == &previous.current_note.spend_nullifier)
        {
            return Err(KagemushaFoldError::RecursiveSpendMissingPreviousNullifier);
        }
        if step.input_nullifiers.len() != 1 {
            return Err(KagemushaFoldError::RecursiveSpendUnexpectedAppendInput);
        }
        if current_note.spend_nullifier == previous.current_note.note_commitment {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier",
            });
        }
        if step
            .output_commitments
            .iter()
            .any(|commitment| commitment == &previous.current_note.note_commitment)
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "output_commitments",
            });
        }
        if step.output_commitments.iter().any(|commitment| {
            previous
                .topup_anchor_nullifiers
                .iter()
                .any(|anchor| anchor == commitment)
        }) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "output_commitments",
            });
        }
    } else if step_statement.hop_index != 0 {
        return Err(KagemushaFoldError::HopIndexMismatch {
            expected: 0,
            actual: step_statement.hop_index,
        });
    }

    let lineage_digest = kagemusha_recursive_spend_lineage_digest(
        previous,
        chain_id,
        asset,
        hop_index,
        &step,
        current_note,
    )?;
    let verifier_witness_batch_digest = kagemusha_recursive_spend_verifier_batch_digest(
        previous,
        hop_index,
        evidence.verifier_witness_batch_digest,
    )?;
    let fixed_window_table_base_digest = kagemusha_recursive_spend_fixed_window_table_base_digest(
        previous,
        hop_index,
        evidence.fixed_window_table_base_digest,
    )?;
    let recursive_proof_chain_digest = kagemusha_recursive_spend_proof_chain_digest(
        previous,
        previous_recursive_proof,
        hop_index,
        &step,
        evidence.verifier_witness_batch_digest,
    )?;
    let mut current_input_nullifiers = step.input_nullifiers.clone();
    current_input_nullifiers.sort_unstable();
    validate_kagemusha_recursive_spend_topup_anchor_nullifiers(&current_input_nullifiers)?;
    let topup_anchor_nullifiers = previous
        .map(|accumulator| accumulator.topup_anchor_nullifiers.clone())
        .unwrap_or_else(|| current_input_nullifiers.clone());
    let nullifier_values = previous
        .map(|accumulator| vec![hash_bytes_from_hash(accumulator.nullifier_digest)])
        .unwrap_or_default()
        .into_iter()
        .chain(step.input_nullifiers.iter().copied())
        .collect::<Vec<_>>();
    let output_values = previous
        .map(|accumulator| vec![hash_bytes_from_hash(accumulator.output_commitment_digest)])
        .unwrap_or_default()
        .into_iter()
        .chain(step.output_commitments.iter().copied())
        .collect::<Vec<_>>();
    let nullifier_digest = kagemusha_hash_preimage(&KagemushaFoldListDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_NULLIFIER_DIGEST_DOMAIN.to_owned(),
        values: nullifier_values,
    })?;
    let output_commitment_digest = kagemusha_hash_preimage(&KagemushaFoldListDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_OUTPUT_DIGEST_DOMAIN.to_owned(),
        values: output_values,
    })?;
    let step_digest = kagemusha_hash_preimage(&KagemushaFoldStepDigestPreimage {
        domain: KAGEMUSHA_FOLD_STEP_DIGEST_DOMAIN.to_owned(),
        hop_index,
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
    let mut step_digests = Vec::with_capacity(2);
    if let Some(previous) = previous {
        step_digests.push(previous.fold_digest);
    }
    step_digests.push(step_digest);
    let fold_digest = kagemusha_hash_preimage(&KagemushaFoldTranscriptDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_FOLD_DIGEST_DOMAIN.to_owned(),
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        step_digests,
    })?;
    let hop_count = previous
        .map(|accumulator| accumulator.hop_count.saturating_add(1))
        .unwrap_or(1);
    let hop_count_usize = usize::try_from(hop_count).unwrap_or(usize::MAX);
    if hop_count_usize > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
        return Err(KagemushaFoldError::TooManyHops {
            max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
            actual: hop_count_usize,
        });
    }

    let accumulator = KagemushaRecursiveSpendAccumulatorV1 {
        domain: KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN.to_owned(),
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        initial_root: previous.map_or(step.root_before, |accumulator| accumulator.initial_root),
        final_root: step.root_after,
        topup_anchor_nullifiers,
        hop_count,
        lineage_digest,
        aggregation_transcript_digest: lineage_digest,
        nullifier_digest,
        output_commitment_digest,
        fold_digest,
        recursive_proof_chain_digest,
        verifier_params_fingerprint: evidence.verifier_params_fingerprint,
        fixed_window_table_schedule_digest: evidence.fixed_window_table_schedule_digest,
        fixed_window_shared_table_manifest_digest: evidence
            .fixed_window_shared_table_manifest_digest,
        fixed_window_table_base_digest,
        verifier_witness_batch_digest,
        verifier_opening_len: evidence.verifier_opening_len,
        current_note: current_note.clone(),
    };
    accumulator.validate_context()?;
    Ok(accumulator)
}

/// Build the first recursive Kagemusha spend accumulator from one verified hop.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the one-hop evidence is malformed or the
/// current note does not match the hop output.
pub fn kagemusha_recursive_spend_accumulator_from_initial_evidence(
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendAccumulatorV1, KagemushaFoldError> {
    kagemusha_recursive_spend_accumulator_from_parts(None, None, evidence, current_note)
}

/// Append one verified hop to an existing recursive Kagemusha spend accumulator.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the previous accumulator is malformed,
/// the previous recursive proof is not bound to that accumulator, the new
/// one-hop evidence does not continue the lineage, or verifier context changes
/// across hops.
pub fn kagemusha_recursive_spend_accumulator_append_evidence(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    previous_recursive_proof: &KagemushaRecursiveAggregationProof,
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendAccumulatorV1, KagemushaFoldError> {
    kagemusha_recursive_spend_accumulator_from_parts(
        Some(previous),
        Some(previous_recursive_proof),
        evidence,
        current_note,
    )
}

impl KagemushaRecursiveSpendAccumulatorV1 {
    /// Validate accumulator shape and public verifier corridor.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when any accumulator field is outside the
    /// recursive spend corridor.
    pub fn validate_context(&self) -> Result<(), KagemushaFoldError> {
        if self.domain != KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN {
            return Err(KagemushaFoldError::InvalidRecursiveSpendAccumulatorDomain {
                expected: KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN,
                actual: self.domain.clone(),
            });
        }
        validate_kagemusha_fold_root("initial_root", self.initial_root)?;
        validate_kagemusha_fold_root("final_root", self.final_root)?;
        if self.initial_root == self.final_root {
            return Err(KagemushaFoldError::UnchangedFoldedPublicRoots);
        }
        validate_kagemusha_recursive_spend_topup_anchor_nullifiers(&self.topup_anchor_nullifiers)?;
        if self
            .topup_anchor_nullifiers
            .contains(&self.current_note.spend_nullifier)
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers",
            });
        }
        if self
            .topup_anchor_nullifiers
            .contains(&self.current_note.note_commitment)
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers",
            });
        }
        if self.hop_count == 0 {
            return Err(KagemushaFoldError::Empty);
        }
        let hop_count = usize::try_from(self.hop_count).unwrap_or(usize::MAX);
        if hop_count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
            return Err(KagemushaFoldError::TooManyHops {
                max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
                actual: hop_count,
            });
        }
        macro_rules! ensure_non_zero_bytes {
            ($field:ident) => {
                if self.$field == [0u8; Hash::LENGTH] {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: stringify!($field),
                    });
                }
            };
        }
        ensure_non_zero_bytes!(lineage_digest);
        ensure_non_zero_bytes!(aggregation_transcript_digest);
        ensure_non_zero_bytes!(verifier_params_fingerprint);
        ensure_non_zero_bytes!(fixed_window_table_schedule_digest);
        ensure_non_zero_bytes!(fixed_window_shared_table_manifest_digest);
        ensure_non_zero_bytes!(fixed_window_table_base_digest);
        ensure_non_zero_bytes!(verifier_witness_batch_digest);
        ensure_non_zero_bytes!(recursive_proof_chain_digest);
        if self.aggregation_transcript_digest != self.lineage_digest {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "aggregation_transcript_digest",
            });
        }
        for (field, digest) in [
            ("nullifier_digest", self.nullifier_digest),
            ("output_commitment_digest", self.output_commitment_digest),
            ("fold_digest", self.fold_digest),
        ] {
            if hash_bytes_from_hash(digest) == [0u8; Hash::LENGTH] {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch { field });
            }
        }
        validate_kagemusha_recursive_verifier_opening_len(self.verifier_opening_len)?;
        validate_kagemusha_recursive_spend_note(&self.current_note)?;
        Ok(())
    }

    /// Return the chain-visible nullifiers that must be consumed on final redemption.
    ///
    /// This includes the first-hop top-up anchor nullifiers plus the current
    /// spendable note nullifier. Consuming both closes hidden-branch replays:
    /// two recursive branches from the same online-to-offline top-up collide on
    /// the top-up anchor even when their final note nullifiers differ.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the accumulator is malformed.
    pub fn redeem_nullifiers(&self) -> Result<Vec<[u8; Hash::LENGTH]>, KagemushaFoldError> {
        self.validate_context()?;
        let mut nullifiers = self.topup_anchor_nullifiers.clone();
        nullifiers.push(self.current_note.spend_nullifier);
        Ok(nullifiers)
    }

    /// Return the recursive proof public inputs bound to this accumulator.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the accumulator cannot be converted
    /// to canonical recursive proof public inputs.
    pub fn recursive_public_inputs(
        &self,
    ) -> Result<KagemushaRecursiveAggregationProofPublicInputs, KagemushaFoldError> {
        kagemusha_recursive_spend_public_inputs_from_accumulator(self)
    }

    /// Return the Norito-encoded size of this accumulator.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito encoding fails.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

impl KagemushaRecursiveSpendBundleV1 {
    /// Validate that the recursive proof public inputs are derived from the accumulator.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the accumulator, proof envelope, or
    /// public-input parity is invalid.
    pub fn validate_public_input_binding(&self) -> Result<(), KagemushaFoldError> {
        let circuit =
            validate_kagemusha_recursive_spend_proof_public_input_binding(&self.recursive_proof)?;
        let expected = expected_kagemusha_recursive_spend_public_inputs_for_proof(
            &self.accumulator,
            &self.recursive_proof,
            circuit,
        )?;
        macro_rules! ensure_field {
            ($field:ident) => {
                if self.recursive_proof.public_inputs.$field != expected.$field {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: stringify!($field),
                    });
                }
            };
        }
        ensure_field!(domain);
        ensure_field!(evidence_digest);
        ensure_field!(aggregation_transcript_digest);
        ensure_field!(verifier_params_fingerprint);
        ensure_field!(fixed_window_table_schedule_digest);
        ensure_field!(fixed_window_shared_table_manifest_digest);
        ensure_field!(fixed_window_table_base_digest);
        ensure_field!(verifier_witness_batch_digest);
        ensure_field!(recursive_proof_chain_digest);
        ensure_field!(recursive_verifier_scalar_projection_digest);
        ensure_field!(verifier_opening_len);
        ensure_field!(verifier_witness_count);
        ensure_field!(hop_count);
        Ok(())
    }

    /// Return the Norito-encoded size of this spendable D2D payload.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito encoding fails.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

/// Build the record-backed lineage witness that corresponds to the first recursive spend bundle.
///
/// This witness is not part of the constant-size D2D bundle. Wallets keep it as
/// redeem-side audit material until the production in-circuit lineage proof
/// replaces the record-backed admission path.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the request is not a single-hop lineage
/// fragment, when its Pallas envelope archive is malformed, or when the result
/// does not bind to `bundle`.
pub fn kagemusha_recursive_spend_lineage_witness_from_init_result(
    request: &KagemushaRecursiveSpendInitRequestV1,
    bundle: &KagemushaRecursiveSpendBundleV1,
) -> Result<KagemushaRecursiveSpendLineageWitnessV1, KagemushaFoldError> {
    validate_kagemusha_recursive_lineage_record_fragment(
        &request.record_bundle,
        &request.pallas_open_envelopes_archive,
        1,
    )?;
    let witness = KagemushaRecursiveSpendLineageWitnessV1 {
        record_bundle: request.record_bundle.clone(),
        pallas_open_envelopes_archive: request.pallas_open_envelopes_archive.clone(),
        current_notes: vec![request.current_note.clone()],
        previous_recursive_proofs: Vec::new(),
    };
    validate_kagemusha_recursive_spend_lineage_witness(bundle, &witness)?;
    Ok(witness)
}

/// Append one hop of record-backed redeem witness material alongside a recursive spend append.
///
/// `append_request.previous_bundle` must be the bundle that `previous_witness`
/// already describes, and `appended_bundle` must be the newly proved recursive
/// spend bundle. The returned witness can be stored separately from the D2D
/// bundle and later attached to [`KagemushaRecursiveSpendRedeemRequestV1`].
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the previous witness is not bound to the
/// previous bundle, when the new hop is not a single-hop lineage fragment, when
/// verifier records conflict, when Pallas envelope archives cannot be merged, or
/// when the appended witness does not bind to `appended_bundle`.
pub fn kagemusha_recursive_spend_lineage_witness_append_result(
    previous_witness: &KagemushaRecursiveSpendLineageWitnessV1,
    append_request: &KagemushaRecursiveSpendAppendRequestV1,
    appended_bundle: &KagemushaRecursiveSpendBundleV1,
) -> Result<KagemushaRecursiveSpendLineageWitnessV1, KagemushaFoldError> {
    validate_kagemusha_recursive_spend_lineage_witness(
        &append_request.previous_bundle,
        previous_witness,
    )?;
    validate_kagemusha_recursive_lineage_record_fragment(
        &append_request.record_bundle,
        &append_request.pallas_open_envelopes_archive,
        1,
    )?;

    let previous_hops = previous_witness.record_bundle.bundle.steps.len();
    let mut envelopes = decode_kagemusha_recursive_lineage_open_envelopes(
        &previous_witness.pallas_open_envelopes_archive,
        previous_hops,
    )?;
    let mut appended_envelopes = decode_kagemusha_recursive_lineage_open_envelopes(
        &append_request.pallas_open_envelopes_archive,
        1,
    )?;
    envelopes.append(&mut appended_envelopes);
    let pallas_open_envelopes_archive =
        to_bytes(&envelopes).map_err(|_| KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.pallas_open_envelopes_archive",
        })?;

    let mut record_bundle = previous_witness.record_bundle.clone();
    if record_bundle.bundle.chain_id != append_request.record_bundle.bundle.chain_id {
        return Err(KagemushaFoldError::RecursiveSpendChainMismatch);
    }
    if record_bundle.bundle.asset != append_request.record_bundle.bundle.asset {
        return Err(KagemushaFoldError::RecursiveSpendAssetMismatch);
    }
    record_bundle
        .bundle
        .steps
        .extend(append_request.record_bundle.bundle.steps.clone());
    for entry in &append_request.record_bundle.verifier_records {
        match record_bundle
            .verifier_records
            .iter()
            .find(|existing| existing.id == entry.id)
        {
            Some(existing) if existing == entry => {}
            Some(_) => {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "lineage_witness.record_bundle.verifier_records.conflict",
                });
            }
            None => record_bundle.verifier_records.push(entry.clone()),
        }
    }

    let mut current_notes = previous_witness.current_notes.clone();
    current_notes.push(append_request.current_note.clone());
    let mut previous_recursive_proofs = previous_witness.previous_recursive_proofs.clone();
    previous_recursive_proofs.push(append_request.previous_bundle.recursive_proof.clone());
    let witness = KagemushaRecursiveSpendLineageWitnessV1 {
        record_bundle,
        pallas_open_envelopes_archive,
        current_notes,
        previous_recursive_proofs,
    };
    validate_kagemusha_recursive_spend_lineage_witness(appended_bundle, &witness)?;
    Ok(witness)
}

fn validate_kagemusha_recursive_lineage_record_fragment(
    record_bundle: &KagemushaVerifiedFoldRecordBundle,
    pallas_open_envelopes_archive: &[u8],
    expected_hops: usize,
) -> Result<(), KagemushaFoldError> {
    if record_bundle.bundle.steps.len() != expected_hops {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: expected_hops,
            actual: u32::try_from(record_bundle.bundle.steps.len()).unwrap_or(u32::MAX),
        });
    }
    validate_kagemusha_verified_fold_record_bundle_exact_records(record_bundle)?;
    decode_kagemusha_recursive_lineage_open_envelopes(
        pallas_open_envelopes_archive,
        expected_hops,
    )?;
    Ok(())
}

fn validate_kagemusha_verified_fold_record_bundle_exact_records(
    record_bundle: &KagemushaVerifiedFoldRecordBundle,
) -> Result<(), KagemushaFoldError> {
    let required_records = record_bundle
        .bundle
        .steps
        .iter()
        .map(|step| step.attachment.vk_ref.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut supplied_records = std::collections::BTreeSet::new();
    for entry in &record_bundle.verifier_records {
        if !supplied_records.insert(entry.id.clone()) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.duplicate",
            });
        }
        if !required_records.contains(&entry.id) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.unreferenced",
            });
        }
    }
    if supplied_records != required_records {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.missing",
        });
    }
    Ok(())
}

fn decode_kagemusha_recursive_lineage_open_envelopes(
    pallas_open_envelopes_archive: &[u8],
    expected_hops: usize,
) -> Result<Vec<iroha_zkp_halo2::OpenVerifyEnvelope>, KagemushaFoldError> {
    if pallas_open_envelopes_archive.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.pallas_open_envelopes_archive",
        });
    }
    let envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
        norito::decode_from_bytes(pallas_open_envelopes_archive).map_err(|_| {
            KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.pallas_open_envelopes_archive",
            }
        })?;
    if envelopes.len() != expected_hops {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: expected_hops,
            actual: u32::try_from(envelopes.len()).unwrap_or(u32::MAX),
        });
    }
    Ok(envelopes)
}

const KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND: &str = KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND;
const KAGEMUSHA_RECURSIVE_SPEND_REDEEM_PROOF_BACKEND: &str =
    KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND;

fn validate_kagemusha_recursive_spend_bundle_production_proof_attachment(
    bundle: &KagemushaRecursiveSpendBundleV1,
) -> Result<(), KagemushaFoldError> {
    validate_kagemusha_recursive_spend_proof_attachment(&bundle.recursive_proof)
}

fn validate_kagemusha_recursive_spend_proof_attachment(
    recursive_proof: &KagemushaRecursiveAggregationProof,
) -> Result<(), KagemushaFoldError> {
    if recursive_proof.proof.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "proof.backend",
        });
    }
    if recursive_proof.verifier_key_id.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "verifier_key_id.backend",
        });
    }
    kagemusha_recursive_spend_proof_circuit(&recursive_proof.verifier_key_id)?;
    if recursive_proof.proof.bytes.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "proof.bytes",
        });
    }
    Ok(())
}

fn validate_kagemusha_recursive_spend_lineage_witness(
    bundle: &KagemushaRecursiveSpendBundleV1,
    witness: &KagemushaRecursiveSpendLineageWitnessV1,
) -> Result<(), KagemushaFoldError> {
    bundle.validate_public_input_binding()?;
    let hop_count = witness.record_bundle.bundle.steps.len();
    if hop_count == 0 {
        return Err(KagemushaFoldError::Empty);
    }
    if hop_count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
        return Err(KagemushaFoldError::TooManyHops {
            max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
            actual: hop_count,
        });
    }
    if witness.current_notes.len() != hop_count {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: hop_count,
            actual: u32::try_from(witness.current_notes.len()).unwrap_or(u32::MAX),
        });
    }
    if witness.previous_recursive_proofs.len().saturating_add(1) != hop_count {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: hop_count.saturating_sub(1),
            actual: u32::try_from(witness.previous_recursive_proofs.len()).unwrap_or(u32::MAX),
        });
    }
    decode_kagemusha_recursive_lineage_open_envelopes(
        &witness.pallas_open_envelopes_archive,
        hop_count,
    )?;
    if witness.record_bundle.bundle.chain_id != bundle.accumulator.chain_id {
        return Err(KagemushaFoldError::RecursiveSpendChainMismatch);
    }
    if witness.record_bundle.bundle.asset != bundle.accumulator.asset {
        return Err(KagemushaFoldError::RecursiveSpendAssetMismatch);
    }
    if usize::try_from(bundle.accumulator.hop_count).ok() != Some(hop_count) {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: hop_count,
            actual: bundle.accumulator.hop_count,
        });
    }
    let mut required_records = std::collections::BTreeSet::new();
    for step in &witness.record_bundle.bundle.steps {
        required_records.insert(step.attachment.vk_ref.clone());
    }
    let mut supplied_records = std::collections::BTreeSet::new();
    for entry in &witness.record_bundle.verifier_records {
        if !supplied_records.insert(entry.id.clone()) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.duplicate",
            });
        }
        if !required_records.contains(&entry.id) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.unreferenced",
            });
        }
    }
    if supplied_records != required_records {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.missing",
        });
    }

    for (proof_index, previous_proof) in witness.previous_recursive_proofs.iter().enumerate() {
        let circuit =
            validate_kagemusha_recursive_spend_proof_public_input_binding(previous_proof)?;
        if circuit != KagemushaRecursiveSpendProofCircuit::SemanticAggregation {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.previous_recursive_proofs.verifier_key_id.name",
            });
        }
        if previous_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest
            != [0u8; Hash::LENGTH]
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.previous_recursive_proofs.recursive_verifier_scalar_projection_digest",
            });
        }
        let expected_hop_count = proof_index.saturating_add(1);
        if previous_proof.public_inputs.hop_count != expected_hop_count as u32 {
            return Err(KagemushaFoldError::HopCountMismatch {
                expected: expected_hop_count,
                actual: previous_proof.public_inputs.hop_count,
            });
        }
    }

    for (hop_index, (step, note)) in witness
        .record_bundle
        .bundle
        .steps
        .iter()
        .zip(witness.current_notes.iter())
        .enumerate()
    {
        validate_kagemusha_recursive_spend_note(note)?;
        if !step
            .output_commitments
            .iter()
            .any(|commitment| commitment == &note.note_commitment)
        {
            return Err(KagemushaFoldError::RecursiveSpendMissingCurrentNoteCommitment);
        }
        if step
            .input_nullifiers
            .iter()
            .any(|nullifier| nullifier == &note.spend_nullifier)
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.spend_nullifier",
            });
        }
        if hop_index == 0 {
            continue;
        }
        let previous_note = &witness.current_notes[hop_index - 1];
        if note.amount != previous_note.amount {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" });
        }
        if step.input_nullifiers.len() != 1 {
            return Err(KagemushaFoldError::RecursiveSpendUnexpectedAppendInput);
        }
        if !step
            .input_nullifiers
            .iter()
            .any(|nullifier| nullifier == &previous_note.spend_nullifier)
        {
            return Err(KagemushaFoldError::RecursiveSpendMissingPreviousNullifier);
        }
    }

    if witness.current_notes.last() != Some(&bundle.accumulator.current_note) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "lineage_witness.current_notes.final",
        });
    }
    Ok(())
}

fn validate_kagemusha_recursive_spend_redeem_proof_attachment(
    redeem_proof: &ProofAttachment,
) -> Result<(), KagemushaFoldError> {
    if redeem_proof.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_REDEEM_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof { field: "backend" });
    }
    if redeem_proof.proof.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_REDEEM_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
            field: "proof.backend",
        });
    }
    if redeem_proof.vk_ref.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_REDEEM_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
            field: "vk_ref.backend",
        });
    }
    if redeem_proof.vk_ref.name.trim().is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
            field: "vk_ref.name",
        });
    }
    if redeem_proof.proof.bytes.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
            field: "proof.bytes",
        });
    }
    let Some(vk_commitment) = redeem_proof.vk_commitment else {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
            field: "vk_commitment",
        });
    };
    if vk_commitment == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
            field: "vk_commitment",
        });
    }
    if let Some(envelope_hash) = redeem_proof.envelope_hash {
        let expected_hash: [u8; Hash::LENGTH] = Hash::new(&redeem_proof.proof.bytes).into();
        if envelope_hash != expected_hash {
            return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "envelope_hash",
            });
        }
    }
    Ok(())
}

impl KagemushaRecursiveSpendRedeemRequestV1 {
    /// Validate wallet-side public bindings before producing a redeem instruction.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the recursive bundle is malformed,
    /// the requested public amount is not exactly the current spendable note
    /// amount, or the final redeem proof attachment is not in the transparent
    /// production corridor.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaFoldError> {
        validate_kagemusha_recursive_spend_bundle_production_proof_attachment(&self.bundle)?;
        self.bundle.validate_public_input_binding()?;
        validate_kagemusha_recursive_spend_redeem_proof_attachment(&self.redeem_proof)?;
        let current_note = &self.bundle.accumulator.current_note;
        if self.public_amount == 0
            || current_note.amount.scale() != 0
            || current_note.amount.try_mantissa_u128() != Some(self.public_amount)
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "public_amount",
            });
        }
        if let Some(witness) = &self.lineage_witness {
            validate_kagemusha_recursive_spend_lineage_witness(&self.bundle, witness)?;
        }
        Ok(())
    }
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
            if seen_outputs.contains(input) {
                return Err(KagemushaFoldError::InputOutputOverlap { hop_index });
            }
            if !seen_inputs.insert(*input) {
                return Err(KagemushaFoldError::DuplicateInputNullifier { hop_index });
            }
        }

        let mut output_commitments = step.output_commitments.clone();
        output_commitments.sort_unstable();
        for output in &output_commitments {
            if seen_inputs.contains(output) {
                return Err(KagemushaFoldError::InputOutputOverlap { hop_index });
            }
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

/// Build reserved-mode recursive aggregation evidence from checked folded-hop material.
///
/// The builder canonicalizes the folded-hop transcript exactly like
/// [`kagemusha_folded_public_inputs`], then changes only the aggregation mode to
/// reserved mode `2` and binds the canonical no-trusted-setup verifier-witness
/// profile plus the caller-supplied native verifier-witness batch preflight
/// fields, including the verifier opening length and fixed-window table
/// schedule, shared-table manifest, and base digests that the batch preflight
/// and recursive table manifest bind.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the folded-hop witness is non-canonical,
/// the verifier parameter fingerprint is all-zero, the fixed-window table
/// schedule, shared-table manifest, or table-base digest is all-zero, or the
/// verifier-witness batch digest is all-zero.
pub fn kagemusha_recursive_aggregation_evidence_from_steps(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    steps: &[KagemushaFoldStep],
    verifier_opening_len: u32,
    verifier_params_fingerprint: [u8; Hash::LENGTH],
    fixed_window_table_schedule_digest: [u8; Hash::LENGTH],
    fixed_window_shared_table_manifest_digest: [u8; Hash::LENGTH],
    fixed_window_table_base_digest: [u8; Hash::LENGTH],
    verifier_witness_batch_digest: [u8; Hash::LENGTH],
) -> Result<KagemushaRecursiveAggregationEvidence, KagemushaFoldError> {
    let mut aggregation_statement =
        kagemusha_canonical_fold_parts(chain_id, asset, steps)?.aggregation_statement;
    aggregation_statement.aggregation_mode = KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1;
    let evidence = KagemushaRecursiveAggregationEvidence {
        verifier_witness_count: aggregation_statement.hop_count,
        verifier_witness_profile: KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1.to_owned(),
        verifier_opening_len,
        aggregation_statement,
        verifier_params_fingerprint,
        fixed_window_table_schedule_digest,
        fixed_window_shared_table_manifest_digest,
        fixed_window_table_base_digest,
        verifier_witness_batch_digest,
    };
    validate_kagemusha_recursive_aggregation_evidence(&evidence)?;
    Ok(evidence)
}

struct KagemushaFoldDigestParts {
    nullifier_digest: Hash,
    output_commitment_digest: Hash,
    fold_digest: Hash,
}

fn kagemusha_fold_digest_parts_from_aggregation_statement(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<KagemushaFoldDigestParts, KagemushaFoldError> {
    validate_kagemusha_hashable_aggregation_transcript_statement(statement)?;

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

    fn sample_kagemusha_recursive_aggregation_evidence() -> KagemushaRecursiveAggregationEvidence {
        let chain_id: ChainId = "kagemusha-recursive-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive");
        let root0 = fixed_hash(b"kagemusha-recursive-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-root-2");
        let steps = vec![
            kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-proof-hop-0"),
            kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-proof-hop-1"),
        ];
        kagemusha_recursive_aggregation_evidence_from_steps(
            &chain_id,
            &asset,
            &steps,
            4,
            fixed_hash(b"recursive-pallas-params"),
            fixed_hash(b"recursive-fixed-window-schedule"),
            fixed_hash(b"recursive-fixed-window-shared-manifest"),
            fixed_hash(b"recursive-fixed-window-bases"),
            fixed_hash(b"recursive-pallas-witness-batch"),
        )
        .expect("recursive aggregation evidence")
    }

    fn kagemusha_recursive_spend_note(
        note_label: &[u8],
        nullifier_label: &[u8],
        amount: u128,
    ) -> KagemushaSpendableNoteDescriptorV1 {
        KagemushaSpendableNoteDescriptorV1 {
            note_commitment: fixed_hash(note_label),
            spend_nullifier: fixed_hash(nullifier_label),
            amount: Numeric::new(amount, 0),
        }
    }

    fn kagemusha_recursive_spend_one_hop_evidence(
        chain_id: &ChainId,
        asset: &AssetDefinitionId,
        step: KagemushaFoldStep,
        witness_label: &[u8],
    ) -> KagemushaRecursiveAggregationEvidence {
        kagemusha_recursive_aggregation_evidence_from_steps(
            chain_id,
            asset,
            &[step],
            4,
            fixed_hash(b"recursive-spend-pallas-params"),
            fixed_hash(b"recursive-spend-fixed-window-schedule"),
            fixed_hash(b"recursive-spend-fixed-window-shared-manifest"),
            fixed_hash(b"recursive-spend-fixed-window-bases"),
            fixed_hash(witness_label),
        )
        .expect("one-hop recursive spend evidence")
    }

    fn kagemusha_recursive_spend_proof(
        accumulator: &KagemushaRecursiveSpendAccumulatorV1,
    ) -> KagemushaRecursiveAggregationProof {
        let public_inputs = accumulator
            .recursive_public_inputs()
            .expect("recursive spend public inputs");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("recursive spend public-input hash");
        KagemushaRecursiveAggregationProof {
            verifier_key_id: VerifyingKeyId::new(
                "halo2/ipa",
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
            public_inputs,
            public_inputs_hash,
            proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 256]),
        }
    }

    fn kagemusha_recursive_spend_lineage_proof(
        accumulator: &KagemushaRecursiveSpendAccumulatorV1,
        scalar_projection_label: &[u8],
    ) -> KagemushaRecursiveAggregationProof {
        let mut proof = kagemusha_recursive_spend_proof(accumulator);
        proof.verifier_key_id.name = KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.into();
        proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest = fixed_hash(scalar_projection_label);
        proof.public_inputs_hash = proof
            .public_inputs
            .public_inputs_hash()
            .expect("recursive spend lineage public-input hash");
        proof
    }

    fn kagemusha_recursive_spend_bundle(
        accumulator: KagemushaRecursiveSpendAccumulatorV1,
    ) -> KagemushaRecursiveSpendBundleV1 {
        let recursive_proof = kagemusha_recursive_spend_proof(&accumulator);
        KagemushaRecursiveSpendBundleV1 {
            accumulator,
            recursive_proof,
        }
    }

    fn kagemusha_recursive_spend_record_bundle_for_step(
        chain_id: ChainId,
        asset: AssetDefinitionId,
        step: &KagemushaFoldStep,
        vk_name: &'static str,
        proof_label: &'static [u8],
    ) -> KagemushaVerifiedFoldRecordBundle {
        let vk_id = VerifyingKeyId::new("halo2/ipa", vk_name);
        let proof = ProofBox::new("halo2/ipa".into(), proof_label.to_vec());
        let mut attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof, vk_id.clone());
        let vk_commitment = fixed_hash(vk_name.as_bytes());
        attachment.vk_commitment = Some(vk_commitment);
        let verifier_key = VerifyingKeyBox::new("halo2/ipa".into(), vec![0x42; 48]);
        let lineage_step = KagemushaVerifiedFoldStep {
            root_before: step.root_before,
            input_nullifiers: step.input_nullifiers.clone(),
            output_commitments: step.output_commitments.clone(),
            root_after: step.root_after,
            attachment,
            verifier_key: verifier_key.clone(),
        };
        let mut record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:kagemusha-hop-fixture",
            BackendTag::Halo2IpaPasta,
            "pasta",
            fixed_hash(b"kagemusha-recursive-lineage-schema"),
            vk_commitment,
        );
        record.status = ConfidentialStatus::Active;
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.max_proof_bytes = 4096;
        record.key = Some(verifier_key);
        KagemushaVerifiedFoldRecordBundle {
            bundle: KagemushaVerifiedFoldBundle {
                chain_id,
                asset,
                steps: vec![lineage_step],
            },
            verifier_records: vec![KagemushaVerifiedFoldVerifierRecord { id: vk_id, record }],
        }
    }

    fn kagemusha_recursive_spend_pallas_open_envelope_archive(label: u8) -> Vec<u8> {
        let envelope = iroha_zkp_halo2::OpenVerifyEnvelope {
            params: iroha_zkp_halo2::IpaParams {
                version: 1,
                curve_id: 1,
                n: 2,
                g: vec![[label; Hash::LENGTH], [label.wrapping_add(1); Hash::LENGTH]],
                h: vec![
                    [label.wrapping_add(2); Hash::LENGTH],
                    [label.wrapping_add(3); Hash::LENGTH],
                ],
                u: [label.wrapping_add(4); Hash::LENGTH],
            },
            public: iroha_zkp_halo2::PolyOpenPublic {
                version: 1,
                curve_id: 1,
                n: 2,
                z: [label.wrapping_add(5); Hash::LENGTH],
                t: [label.wrapping_add(6); Hash::LENGTH],
                p_g: [label.wrapping_add(7); Hash::LENGTH],
            },
            proof: iroha_zkp_halo2::IpaProofData {
                version: 1,
                l: vec![[label.wrapping_add(8); Hash::LENGTH]],
                r: vec![[label.wrapping_add(9); Hash::LENGTH]],
                a_final: [label.wrapping_add(10); Hash::LENGTH],
                b_final: [label.wrapping_add(11); Hash::LENGTH],
            },
            transcript_label: format!("kagemusha-recursive-lineage-{label}"),
            vk_commitment: Some([label.wrapping_add(12); Hash::LENGTH]),
            public_inputs_schema_hash: Some([label.wrapping_add(13); Hash::LENGTH]),
            domain_tag: Some([label.wrapping_add(14); Hash::LENGTH]),
        };
        to_bytes(&vec![envelope]).expect("encode Pallas envelope archive")
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
        assert!(
            kagemusha_verifier_key_poseidon_digest(
                "stark/fri/sha256_goldilocks.v1",
                b"kagemusha-hop-vk"
            )
            .is_ok()
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
            "stark/fri/prod;kzg",
            "stark/fri/prod,kzg",
            "stark/fri/prod+kzg",
            "stark/fri/prod.kzg",
            "stark/fri/prod-k-z-g",
            "stark/fri/prod(kzg)",
            "stark/fri/bn254",
            "stark/fri/prod;bn254",
            "stark/fri/prod-bn-254",
            "stark/fri/prod+bn256",
            "stark/fri/prod-bn-256",
            "stark/fri/bls12_381",
            "stark/fri/prod-bls12-381",
            "stark/fri/prod.bls12_381",
            "stark/fri/prod-b.l.s.12.381",
            "srs",
            "SRS",
            "crs",
            "ptau",
            "powersoftau",
            "powers-of-tau",
            "trusted-setup",
            "structured-reference-string",
            "universal-srs",
            "halo2/ipa:universal-srs",
            "stark/fri/prod-srs",
            "stark/fri/prod-s-r-s",
            "stark/fri/prod.crs",
            "stark/fri/prod-ptau",
            "stark/fri/prod-powers-of-tau",
            "stark/fri/prod-ceremony",
            "stark/fri/structured-reference-string",
            "halo2/ipa;groth16",
            "halo2/ipa:groth-16",
        ] {
            assert!(matches!(
                kagemusha_verifier_key_poseidon_digest(backend, b"kagemusha-hop-vk"),
                Err(KagemushaFoldError::UnsupportedProofBackend { backend: rejected })
                    if rejected == backend
            ));
        }
        for backend in [
            "stark/fri/d-e-b-u-g",
            "stark/fri/m-o-c-k",
            "halo2/ipa:d-e-b-u-g",
            "halo2/ipa:m-o-c-k",
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
        assert!(matches!(
            kagemusha_verifier_key_poseidon_digest("stark/fri/ ", b"kagemusha-hop-vk"),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/ "
        ));
        assert!(matches!(
            kagemusha_verifier_key_poseidon_digest("stark/fri/\t\n", b"kagemusha-hop-vk"),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/\t\n"
        ));
        for backend in [
            "stark/fri/ sha256-goldilocks",
            "stark/fri/sha256-goldilocks ",
            "stark/fri/sha256 goldilocks",
            "stark/fri/prod;foo",
            "stark/fri/prod,foo",
            "stark/fri/prod+foo",
            "stark/fri/prod/foo",
            "stark/fri/prod(foo)",
            "stark/fri/Δ",
        ] {
            assert!(matches!(
                kagemusha_verifier_key_poseidon_digest(backend, b"kagemusha-hop-vk"),
                Err(KagemushaFoldError::UnsupportedProofBackend { backend: rejected })
                    if rejected == backend
            ));
        }
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
            unsupported_kagemusha_aggregation_mode_reason(
                KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
            )
            .contains("private-hop verifier is not yet composed into compact-token admission")
        );
        assert!(
            !unsupported_kagemusha_aggregation_mode_reason(
                KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
            )
            .contains("no recursive verifier")
        );
        assert!(
            unsupported_kagemusha_aggregation_mode_reason(0xFFFF)
                .contains("unsupported or unknown")
        );
        assert_eq!(
            preferred_kagemusha_offline_spend_mode(true),
            KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
        );
        assert_eq!(
            preferred_kagemusha_offline_spend_mode(false),
            KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
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
        assert_ne!(
            digest,
            kagemusha_poseidon_aggregation_transcript_digest(&changed_mode)
                .expect("reserved recursive mode still has a separated transcript digest")
        );
        let mut reserved_public_inputs =
            kagemusha_folded_public_inputs_from_aggregation_statement(&changed_mode)
                .expect("reserved recursive transcript projection");
        reserved_public_inputs.aggregation_mode =
            KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1;
        assert!(matches!(
            reserved_public_inputs.validate_supported_context(),
            Err(KagemushaFoldError::UnsupportedAggregationMode { actual, .. })
                if actual == KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        ));
        let mut unknown_mode = changed_mode.clone();
        unknown_mode.aggregation_mode = 0xFFFF;
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&unknown_mode),
            Err(KagemushaFoldError::UnsupportedAggregationMode { actual: 0xFFFF, .. })
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
    fn kagemusha_recursive_aggregation_evidence_binds_batch_preflight_digest() {
        let chain_id: ChainId = "kagemusha-recursive-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive");
        let root0 = fixed_hash(b"kagemusha-recursive-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-root-2");
        let steps = vec![
            kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-proof-hop-0"),
            kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-proof-hop-1"),
        ];
        let opening_len = 4;
        let params_fingerprint = fixed_hash(b"recursive-pallas-params");
        let schedule_digest = fixed_hash(b"recursive-fixed-window-schedule");
        let manifest_digest = fixed_hash(b"recursive-fixed-window-manifest");
        let base_digest = fixed_hash(b"recursive-fixed-window-bases");
        let batch_digest = fixed_hash(b"recursive-pallas-witness-batch");
        let evidence = kagemusha_recursive_aggregation_evidence_from_steps(
            &chain_id,
            &asset,
            &steps,
            opening_len,
            params_fingerprint,
            schedule_digest,
            manifest_digest,
            base_digest,
            batch_digest,
        )
        .expect("recursive aggregation evidence");
        assert_eq!(
            evidence.aggregation_statement.aggregation_mode,
            KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        );
        assert_eq!(evidence.verifier_witness_count, 2);
        assert_eq!(
            evidence.verifier_witness_profile,
            KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1
        );
        assert_eq!(evidence.verifier_opening_len, opening_len);
        assert_eq!(evidence.verifier_params_fingerprint, params_fingerprint);
        assert_eq!(evidence.fixed_window_table_schedule_digest, schedule_digest);
        assert_eq!(
            evidence.fixed_window_shared_table_manifest_digest,
            manifest_digest
        );
        assert_eq!(evidence.fixed_window_table_base_digest, base_digest);
        assert_eq!(evidence.verifier_witness_batch_digest, batch_digest);
        let transcript_digest =
            kagemusha_poseidon_aggregation_transcript_digest(&evidence.aggregation_statement)
                .expect("reserved recursive aggregation transcript digest");
        assert_ne!(transcript_digest, [0u8; Hash::LENGTH]);

        let digest = kagemusha_recursive_aggregation_evidence_digest(&evidence)
            .expect("recursive aggregation evidence digest");
        assert_ne!(digest, [0u8; Hash::LENGTH]);
        assert_eq!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&evidence)
                .expect("repeat recursive aggregation evidence digest")
        );
        let bytes = to_bytes(&evidence).expect("encode recursive aggregation evidence");
        let decoded: KagemushaRecursiveAggregationEvidence =
            norito::decode_from_bytes(&bytes).expect("decode recursive aggregation evidence");
        assert_eq!(decoded, evidence);
        assert_eq!(
            decoded.verifier_witness_profile,
            KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1
        );
        assert_eq!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&decoded)
                .expect("decoded recursive aggregation evidence digest")
        );

        let mut changed_batch = evidence.clone();
        changed_batch.verifier_witness_batch_digest =
            fixed_hash(b"recursive-pallas-witness-batch-other");
        assert_ne!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&changed_batch)
                .expect("changed batch digest evidence")
        );

        let mut changed_opening_len = evidence.clone();
        changed_opening_len.verifier_opening_len = 8;
        assert_ne!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&changed_opening_len)
                .expect("changed opening length evidence")
        );

        let mut changed_schedule = evidence.clone();
        changed_schedule.fixed_window_table_schedule_digest =
            fixed_hash(b"recursive-fixed-window-schedule-other");
        assert_ne!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&changed_schedule)
                .expect("changed schedule digest evidence")
        );

        let mut changed_manifest = evidence.clone();
        changed_manifest.fixed_window_shared_table_manifest_digest =
            fixed_hash(b"recursive-fixed-window-manifest-other");
        assert_ne!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&changed_manifest)
                .expect("changed shared-table manifest digest evidence")
        );

        let mut changed_base = evidence.clone();
        changed_base.fixed_window_table_base_digest =
            fixed_hash(b"recursive-fixed-window-bases-other");
        assert_ne!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&changed_base)
                .expect("changed table-base digest evidence")
        );

        let mut changed_profile = evidence.clone();
        changed_profile.verifier_witness_profile =
            "pallas-ipa-transparent-v1/vesta-recursive-fixed-window-unsafe".to_owned();
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_digest(&changed_profile),
            Err(
                KagemushaFoldError::UnsupportedRecursiveVerifierWitnessProfile { actual, .. }
            ) if actual == "pallas-ipa-transparent-v1/vesta-recursive-fixed-window-unsafe"
        ));

        let mut changed_params = evidence;
        changed_params.verifier_params_fingerprint = fixed_hash(b"recursive-pallas-params-other");
        assert_ne!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&changed_params)
                .expect("changed params evidence")
        );
    }

    #[test]
    fn kagemusha_recursive_aggregation_evidence_rejects_noncanonical_fields() {
        let chain_id: ChainId = "kagemusha-recursive-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive");
        let root0 = fixed_hash(b"kagemusha-recursive-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-root-2");
        let steps = vec![
            kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-bad-hop-0"),
            kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-bad-hop-1"),
        ];
        let evidence = kagemusha_recursive_aggregation_evidence_from_steps(
            &chain_id,
            &asset,
            &steps,
            4,
            fixed_hash(b"recursive-bad-params"),
            fixed_hash(b"recursive-bad-schedule"),
            fixed_hash(b"recursive-bad-manifest"),
            fixed_hash(b"recursive-bad-bases"),
            fixed_hash(b"recursive-bad-batch"),
        )
        .expect("recursive aggregation evidence");

        let mut checked_mode = evidence.clone();
        checked_mode.aggregation_statement.aggregation_mode =
            KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1;
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&checked_mode),
            Err(KagemushaFoldError::RecursiveAggregationEvidenceModeMismatch { actual, .. })
                if actual == KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
        ));

        let mut bad_count = evidence.clone();
        bad_count.verifier_witness_count = 1;
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&bad_count),
            Err(
                KagemushaFoldError::RecursiveAggregationWitnessCountMismatch {
                    expected: 2,
                    actual: 1
                }
            )
        ));

        let mut empty_statement = evidence.clone();
        empty_statement.aggregation_statement.steps.clear();
        empty_statement.aggregation_statement.hop_count = 0;
        empty_statement.verifier_witness_count = 0;
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&empty_statement),
            Err(KagemushaFoldError::Empty)
        ));

        let mut too_many_hops = evidence.clone();
        too_many_hops.aggregation_statement.steps =
            vec![
                too_many_hops.aggregation_statement.steps[0].clone();
                KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
            ];
        too_many_hops.aggregation_statement.hop_count =
            u32::try_from(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1).expect("hop count fits");
        too_many_hops.verifier_witness_count = too_many_hops.aggregation_statement.hop_count;
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&too_many_hops),
            Err(KagemushaFoldError::TooManyHops { actual, .. })
                if actual == KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
        ));

        let mut bad_profile = evidence.clone();
        bad_profile.verifier_witness_profile = "pallas-ipa-transparent-v1/mock".to_owned();
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&bad_profile),
            Err(
                KagemushaFoldError::UnsupportedRecursiveVerifierWitnessProfile { actual, .. }
            ) if actual == "pallas-ipa-transparent-v1/mock"
        ));

        let mut unsupported_opening_len = evidence.clone();
        unsupported_opening_len.verifier_opening_len = 1;
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&unsupported_opening_len),
            Err(KagemushaFoldError::UnsupportedRecursiveVerifierOpeningLength { actual: 1, .. })
        ));

        let mut non_power_opening_len = evidence.clone();
        non_power_opening_len.verifier_opening_len = 3;
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&non_power_opening_len),
            Err(KagemushaFoldError::NonPowerOfTwoRecursiveVerifierOpeningLength { actual: 3 })
        ));

        let bad_profile_bytes =
            to_bytes(&bad_profile).expect("encode unsupported-profile recursive evidence");
        let bad_profile_decoded: KagemushaRecursiveAggregationEvidence =
            norito::decode_from_bytes(&bad_profile_bytes)
                .expect("decode unsupported-profile recursive evidence");
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&bad_profile_decoded),
            Err(
                KagemushaFoldError::UnsupportedRecursiveVerifierWitnessProfile { actual, .. }
            ) if actual == "pallas-ipa-transparent-v1/mock"
        ));

        let mut zero_params = evidence.clone();
        zero_params.verifier_params_fingerprint = [0u8; Hash::LENGTH];
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&zero_params),
            Err(KagemushaFoldError::ZeroRecursiveVerifierParamsFingerprint)
        ));

        let mut zero_schedule = evidence.clone();
        zero_schedule.fixed_window_table_schedule_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&zero_schedule),
            Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableScheduleDigest)
        ));

        let mut zero_bases = evidence.clone();
        zero_bases.fixed_window_table_base_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&zero_bases),
            Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableBaseDigest)
        ));

        let mut zero_batch = evidence.clone();
        zero_batch.verifier_witness_batch_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&zero_batch),
            Err(KagemushaFoldError::ZeroRecursiveVerifierWitnessBatchDigest)
        ));

        let mut discontinuous = evidence.clone();
        discontinuous.aggregation_statement.steps[1].root_before =
            fixed_hash(b"kagemusha-recursive-wrong-root");
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&discontinuous),
            Err(KagemushaFoldError::RootDiscontinuity { hop_index: 1, .. })
        ));

        let mut duplicate_input = evidence.clone();
        duplicate_input.aggregation_statement.steps[1].input_nullifiers[0] =
            duplicate_input.aggregation_statement.steps[0].input_nullifiers[0];
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&duplicate_input),
            Err(KagemushaFoldError::DuplicateInputNullifier { hop_index: 1 })
        ));

        let mut duplicate_output = evidence.clone();
        duplicate_output.aggregation_statement.steps[1].output_commitments[0] =
            duplicate_output.aggregation_statement.steps[0].output_commitments[0];
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&duplicate_output),
            Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index: 1 })
        ));

        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                1,
                fixed_hash(b"recursive-bad-params"),
                fixed_hash(b"recursive-bad-schedule"),
                fixed_hash(b"recursive-bad-manifest"),
                fixed_hash(b"recursive-bad-bases"),
                fixed_hash(b"recursive-bad-batch"),
            ),
            Err(KagemushaFoldError::UnsupportedRecursiveVerifierOpeningLength { actual: 1, .. })
        ));
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                3,
                fixed_hash(b"recursive-bad-params"),
                fixed_hash(b"recursive-bad-schedule"),
                fixed_hash(b"recursive-bad-manifest"),
                fixed_hash(b"recursive-bad-bases"),
                fixed_hash(b"recursive-bad-batch"),
            ),
            Err(KagemushaFoldError::NonPowerOfTwoRecursiveVerifierOpeningLength { actual: 3 })
        ));
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                4,
                [0u8; Hash::LENGTH],
                fixed_hash(b"recursive-bad-schedule"),
                fixed_hash(b"recursive-bad-manifest"),
                fixed_hash(b"recursive-bad-bases"),
                fixed_hash(b"recursive-bad-batch"),
            ),
            Err(KagemushaFoldError::ZeroRecursiveVerifierParamsFingerprint)
        ));
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                4,
                fixed_hash(b"recursive-bad-params"),
                [0u8; Hash::LENGTH],
                fixed_hash(b"recursive-bad-manifest"),
                fixed_hash(b"recursive-bad-bases"),
                fixed_hash(b"recursive-bad-batch"),
            ),
            Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableScheduleDigest)
        ));
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                4,
                fixed_hash(b"recursive-bad-params"),
                fixed_hash(b"recursive-bad-schedule"),
                [0u8; Hash::LENGTH],
                fixed_hash(b"recursive-bad-bases"),
                fixed_hash(b"recursive-bad-batch"),
            ),
            Err(KagemushaFoldError::ZeroRecursiveFixedWindowSharedTableManifestDigest)
        ));
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                4,
                fixed_hash(b"recursive-bad-params"),
                fixed_hash(b"recursive-bad-schedule"),
                fixed_hash(b"recursive-bad-manifest"),
                [0u8; Hash::LENGTH],
                fixed_hash(b"recursive-bad-batch"),
            ),
            Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableBaseDigest)
        ));
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                4,
                fixed_hash(b"recursive-bad-params"),
                fixed_hash(b"recursive-bad-schedule"),
                fixed_hash(b"recursive-bad-manifest"),
                fixed_hash(b"recursive-bad-bases"),
                [0u8; Hash::LENGTH],
            ),
            Err(KagemushaFoldError::ZeroRecursiveVerifierWitnessBatchDigest)
        ));

        let bytes = to_bytes(&evidence).expect("encode canonical recursive evidence");
        for len in 0..bytes.len().min(8) {
            assert!(
                norito::decode_from_bytes::<KagemushaRecursiveAggregationEvidence>(&bytes[..len])
                    .is_err(),
                "truncated recursive evidence archive at length {len} must reject"
            );
        }
    }

    #[test]
    fn kagemusha_recursive_aggregation_proof_bundle_binds_evidence_and_roundtrips() {
        let evidence = sample_kagemusha_recursive_aggregation_evidence();
        let public_inputs =
            kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(&evidence)
                .expect("recursive proof public inputs");
        assert_eq!(
            public_inputs.domain,
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_DOMAIN
        );
        assert_eq!(
            public_inputs.evidence_digest,
            kagemusha_recursive_aggregation_evidence_digest(&evidence)
                .expect("recursive evidence digest")
        );
        assert_ne!(public_inputs.evidence_digest, [0u8; Hash::LENGTH]);
        assert_ne!(
            public_inputs.aggregation_transcript_digest,
            [0u8; Hash::LENGTH]
        );
        assert_eq!(
            public_inputs.verifier_params_fingerprint,
            evidence.verifier_params_fingerprint
        );
        assert_eq!(
            public_inputs.fixed_window_table_schedule_digest,
            evidence.fixed_window_table_schedule_digest
        );
        assert_eq!(
            public_inputs.fixed_window_shared_table_manifest_digest,
            evidence.fixed_window_shared_table_manifest_digest
        );
        assert_eq!(
            public_inputs.fixed_window_table_base_digest,
            evidence.fixed_window_table_base_digest
        );
        assert_eq!(
            public_inputs.verifier_witness_batch_digest,
            evidence.verifier_witness_batch_digest
        );
        assert_eq!(
            public_inputs.recursive_proof_chain_digest,
            [0u8; Hash::LENGTH],
            "plain recursive aggregation proofs do not carry spend proof-chain state"
        );
        assert_eq!(
            public_inputs.recursive_verifier_scalar_projection_digest,
            [0u8; Hash::LENGTH],
            "plain recursive aggregation proofs do not carry verifier-slice scalar projection state"
        );
        assert_eq!(
            public_inputs.verifier_opening_len,
            evidence.verifier_opening_len
        );
        assert_eq!(
            public_inputs.verifier_witness_count,
            evidence.verifier_witness_count
        );
        assert_eq!(
            public_inputs.hop_count,
            evidence.aggregation_statement.hop_count
        );
        assert_ne!(
            kagemusha_recursive_aggregation_proof_public_inputs_schema_hash(),
            [0u8; Hash::LENGTH]
        );

        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("recursive proof public-input hash");
        let recursive_proof = KagemushaRecursiveAggregationProof {
            verifier_key_id: VerifyingKeyId::new(
                "halo2/ipa",
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
            public_inputs,
            public_inputs_hash,
            proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 64]),
        };
        recursive_proof
            .validate_public_input_binding()
            .expect("recursive proof public inputs bind to proof metadata");
        let bundle = KagemushaRecursiveAggregationProofBundle {
            evidence,
            recursive_proof,
        };
        bundle
            .validate_evidence_binding()
            .expect("recursive proof bundle binds evidence");

        let bytes = to_bytes(&bundle).expect("encode recursive proof bundle");
        let decoded: KagemushaRecursiveAggregationProofBundle =
            norito::decode_from_bytes(&bytes).expect("decode recursive proof bundle");
        assert_eq!(decoded, bundle);
        decoded
            .validate_evidence_binding()
            .expect("decoded recursive proof bundle remains canonical");
    }

    #[test]
    fn kagemusha_recursive_aggregation_proof_bundle_rejects_public_input_substitution() {
        let evidence = sample_kagemusha_recursive_aggregation_evidence();
        let public_inputs =
            kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(&evidence)
                .expect("recursive proof public inputs");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("recursive proof public-input hash");
        let recursive_proof = KagemushaRecursiveAggregationProof {
            verifier_key_id: VerifyingKeyId::new(
                "halo2/ipa",
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
            public_inputs,
            public_inputs_hash,
            proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 64]),
        };
        let bundle = KagemushaRecursiveAggregationProofBundle {
            evidence,
            recursive_proof,
        };

        let mut changed_batch = bundle.clone();
        changed_batch
            .recursive_proof
            .public_inputs
            .verifier_witness_batch_digest = fixed_hash(b"substituted-recursive-batch-digest");
        changed_batch.recursive_proof.public_inputs_hash = changed_batch
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed public-input hash");
        assert!(matches!(
            changed_batch.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "verifier_witness_batch_digest"
                }
            )
        ));

        let mut changed_params = bundle.clone();
        changed_params
            .recursive_proof
            .public_inputs
            .verifier_params_fingerprint = fixed_hash(b"substituted-recursive-params");
        changed_params.recursive_proof.public_inputs_hash = changed_params
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed public-input hash");
        assert!(matches!(
            changed_params.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "verifier_params_fingerprint"
                }
            )
        ));

        let mut changed_manifest = bundle.clone();
        changed_manifest
            .recursive_proof
            .public_inputs
            .fixed_window_shared_table_manifest_digest =
            fixed_hash(b"substituted-recursive-shared-manifest");
        changed_manifest.recursive_proof.public_inputs_hash = changed_manifest
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed public-input hash");
        assert!(matches!(
            changed_manifest.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "fixed_window_shared_table_manifest_digest"
                }
            )
        ));

        let mut changed_proof_chain = bundle.clone();
        changed_proof_chain
            .recursive_proof
            .public_inputs
            .recursive_proof_chain_digest = fixed_hash(b"substituted-recursive-proof-chain");
        changed_proof_chain.recursive_proof.public_inputs_hash = changed_proof_chain
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed proof-chain public-input hash");
        assert!(matches!(
            changed_proof_chain.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "recursive_proof_chain_digest"
                }
            )
        ));

        let mut changed_scalar_projection = bundle.clone();
        changed_scalar_projection
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            fixed_hash(b"substituted-recursive-scalar-projection");
        changed_scalar_projection.recursive_proof.public_inputs_hash = changed_scalar_projection
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed scalar projection public-input hash");
        assert!(matches!(
            changed_scalar_projection.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "recursive_verifier_scalar_projection_digest"
                }
            )
        ));

        let mut changed_hash = bundle.clone();
        changed_hash.recursive_proof.public_inputs_hash =
            Hash::new(b"wrong-recursive-proof-public-input-hash");
        assert!(matches!(
            changed_hash.validate_evidence_binding(),
            Err(KagemushaFoldError::RecursiveAggregationProofPublicInputHashMismatch { .. })
        ));

        let mut changed_domain = bundle.clone();
        changed_domain.recursive_proof.public_inputs.domain =
            "iroha:kagemusha:v1:recursive-proof-alias".to_owned();
        changed_domain.recursive_proof.public_inputs_hash = changed_domain
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed domain public-input hash");
        assert!(matches!(
            changed_domain.validate_evidence_binding(),
            Err(KagemushaFoldError::InvalidRecursiveAggregationProofPublicInputDomain { .. })
        ));
    }

    #[test]
    fn kagemusha_recursive_aggregation_proof_bundle_rejects_backend_and_circuit_substitution() {
        let evidence = sample_kagemusha_recursive_aggregation_evidence();
        let public_inputs =
            kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(&evidence)
                .expect("recursive proof public inputs");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("recursive proof public-input hash");
        let recursive_proof = KagemushaRecursiveAggregationProof {
            verifier_key_id: VerifyingKeyId::new(
                "halo2/ipa",
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
            public_inputs,
            public_inputs_hash,
            proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 64]),
        };
        let bundle = KagemushaRecursiveAggregationProofBundle {
            evidence,
            recursive_proof,
        };

        let mut backend_mismatch = bundle.clone();
        backend_mismatch.recursive_proof.verifier_key_id = VerifyingKeyId::new(
            "stark/fri",
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        );
        assert!(matches!(
            backend_mismatch.validate_evidence_binding(),
            Err(KagemushaFoldError::RecursiveAggregationProofBackendMismatch { .. })
        ));

        let mut trusted_setup_backend = bundle.clone();
        trusted_setup_backend.recursive_proof.proof =
            ProofBox::new("halo2/kzg".into(), vec![0xA5; 64]);
        assert!(matches!(
            trusted_setup_backend.validate_evidence_binding(),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));

        let mut transparent_wrong_family = bundle.clone();
        transparent_wrong_family.recursive_proof.proof =
            ProofBox::new("stark/fri/transparent-v1".into(), vec![0xA5; 64]);
        transparent_wrong_family.recursive_proof.verifier_key_id = VerifyingKeyId::new(
            "stark/fri/transparent-v1",
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        );
        assert!(matches!(
            transparent_wrong_family.validate_evidence_binding(),
            Err(KagemushaFoldError::InvalidRecursiveAggregationProof {
                field: "proof.backend"
            })
        ));

        let mut empty_proof_payload = bundle.clone();
        empty_proof_payload.recursive_proof.proof = ProofBox::new("halo2/ipa".into(), Vec::new());
        assert!(matches!(
            empty_proof_payload.validate_evidence_binding(),
            Err(KagemushaFoldError::InvalidRecursiveAggregationProof {
                field: "proof.bytes"
            })
        ));

        let mut wrong_circuit = bundle;
        wrong_circuit.recursive_proof.verifier_key_id =
            VerifyingKeyId::new("halo2/ipa", "kagemusha-recursive-aggregation-alias");
        assert!(matches!(
            wrong_circuit.validate_evidence_binding(),
            Err(KagemushaFoldError::RecursiveAggregationProofCircuitIdMismatch {
                actual,
                ..
            }) if actual == "kagemusha-recursive-aggregation-alias"
        ));
    }

    #[test]
    fn kagemusha_recursive_spend_bundle_roundtrips_and_appends_without_prior_hops() {
        let chain_id: ChainId = "kagemusha-recursive-spend-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend");
        let root0 = fixed_hash(b"kagemusha-recursive-spend-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-spend-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-spend-root-2");

        let step0 = kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-spend-hop-0");
        let mut expected_topup_anchors = step0.input_nullifiers.clone();
        expected_topup_anchors.sort_unstable();
        let note0 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step0.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-spend-note-0-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence0 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step0.clone(),
            b"recursive-spend-witness-hop-0",
        );
        let accumulator0 =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence0, &note0)
                .expect("initial recursive spend accumulator");
        assert_eq!(accumulator0.hop_count, 1);
        assert_eq!(accumulator0.initial_root, root0);
        assert_eq!(accumulator0.final_root, root1);
        assert_eq!(accumulator0.topup_anchor_nullifiers, expected_topup_anchors);
        let folded_nullifier_digest = kagemusha_list_digest(
            KAGEMUSHA_FOLD_NULLIFIER_DIGEST_DOMAIN,
            evidence0.aggregation_statement.steps[0]
                .input_nullifiers
                .clone(),
        )
        .expect("folded nullifier digest");
        let folded_output_digest = kagemusha_list_digest(
            KAGEMUSHA_FOLD_OUTPUT_DIGEST_DOMAIN,
            evidence0.aggregation_statement.steps[0]
                .output_commitments
                .clone(),
        )
        .expect("folded output digest");
        let mut checked_statement = evidence0.aggregation_statement.clone();
        checked_statement.aggregation_mode = KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1;
        let folded_parts =
            kagemusha_fold_digest_parts_from_aggregation_statement(&checked_statement)
                .expect("checked folded digest parts");
        assert_ne!(
            accumulator0.nullifier_digest, folded_nullifier_digest,
            "recursive spend nullifier stream must not reuse the folded-list digest domain"
        );
        assert_ne!(
            accumulator0.output_commitment_digest, folded_output_digest,
            "recursive spend output stream must not reuse the folded-list digest domain"
        );
        assert_ne!(
            accumulator0.fold_digest, folded_parts.fold_digest,
            "recursive spend fold stream must not reuse the checked folded-token transcript domain"
        );
        assert_ne!(
            accumulator0.recursive_proof_chain_digest,
            [0u8; Hash::LENGTH],
            "recursive spend proof-chain stream must be initialized at the first hop"
        );
        let previous_proof0 = kagemusha_recursive_spend_proof(&accumulator0);
        let lineage_previous_proof0 = kagemusha_recursive_spend_lineage_proof(
            &accumulator0,
            b"recursive-spend-lineage-previous-proof-0",
        );

        let mut step1 = kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-spend-hop-1");
        step1.input_nullifiers = vec![note0.spend_nullifier];
        let note1 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step1.output_commitments[1],
            spend_nullifier: fixed_hash(b"recursive-spend-note-1-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence1 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step1,
            b"recursive-spend-witness-hop-1",
        );
        let accumulator1 = kagemusha_recursive_spend_accumulator_append_evidence(
            &accumulator0,
            &previous_proof0,
            &evidence1,
            &note1,
        )
        .expect("append recursive spend accumulator");
        assert_eq!(accumulator1.hop_count, 2);
        assert_eq!(accumulator1.initial_root, root0);
        assert_eq!(accumulator1.final_root, root2);
        assert_eq!(accumulator1.current_note, note1);
        assert_eq!(accumulator1.topup_anchor_nullifiers, expected_topup_anchors);
        assert_ne!(accumulator1.lineage_digest, accumulator0.lineage_digest);
        assert_ne!(accumulator1.nullifier_digest, accumulator0.nullifier_digest);
        assert_ne!(
            accumulator1.output_commitment_digest,
            accumulator0.output_commitment_digest
        );
        assert_ne!(
            accumulator1.verifier_witness_batch_digest,
            accumulator0.verifier_witness_batch_digest
        );
        assert_ne!(
            accumulator1.recursive_proof_chain_digest,
            accumulator0.recursive_proof_chain_digest
        );
        let accumulator1_from_lineage = kagemusha_recursive_spend_accumulator_append_evidence(
            &accumulator0,
            &lineage_previous_proof0,
            &evidence1,
            &note1,
        )
        .expect("append recursive spend accumulator from lineage proof");
        assert_eq!(accumulator1_from_lineage.hop_count, 2);
        assert_eq!(accumulator1_from_lineage.current_note, note1);
        assert_eq!(
            accumulator1_from_lineage.topup_anchor_nullifiers,
            expected_topup_anchors
        );
        assert_ne!(
            accumulator1_from_lineage.recursive_proof_chain_digest,
            accumulator1.recursive_proof_chain_digest,
            "lineage proof artifact must be distinguished from semantic v1 proof artifact"
        );

        let bundle = kagemusha_recursive_spend_bundle(accumulator1);
        bundle
            .validate_public_input_binding()
            .expect("recursive spend bundle binding");
        let bytes = to_bytes(&bundle).expect("encode recursive spend bundle");
        let decoded: KagemushaRecursiveSpendBundleV1 =
            norito::decode_from_bytes(&bytes).expect("decode recursive spend bundle");
        assert_eq!(decoded, bundle);
        decoded
            .validate_public_input_binding()
            .expect("decoded recursive spend bundle binding");
    }

    #[test]
    fn kagemusha_recursive_spend_lineage_witness_helpers_append_record_backed_material() {
        let chain_id: ChainId = "kagemusha-recursive-spend-lineage-chain"
            .parse()
            .expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend-lineage");
        let root0 = fixed_hash(b"kagemusha-recursive-spend-lineage-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-spend-lineage-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-spend-lineage-root-2");

        let step0 = kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-lineage-hop-0");
        let note0 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step0.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-lineage-note-0-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence0 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step0.clone(),
            b"recursive-lineage-witness-hop-0",
        );
        let accumulator0 =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence0, &note0)
                .expect("initial recursive spend accumulator");
        let bundle0 = kagemusha_recursive_spend_bundle(accumulator0);
        let init_request = KagemushaRecursiveSpendInitRequestV1 {
            record_bundle: kagemusha_recursive_spend_record_bundle_for_step(
                chain_id.clone(),
                asset.clone(),
                &step0,
                "kagemusha-recursive-lineage-hop-0",
                b"recursive-lineage-proof-hop-0",
            ),
            pallas_open_envelopes_archive: kagemusha_recursive_spend_pallas_open_envelope_archive(
                0x41,
            ),
            current_note: note0.clone(),
        };
        let witness0 =
            kagemusha_recursive_spend_lineage_witness_from_init_result(&init_request, &bundle0)
                .expect("initial lineage witness");
        assert_eq!(witness0.current_notes, vec![note0.clone()]);
        assert!(witness0.previous_recursive_proofs.is_empty());

        let mut stale_init_bundle = bundle0.clone();
        stale_init_bundle.recursive_proof.public_inputs.hop_count = 2;
        stale_init_bundle
            .recursive_proof
            .public_inputs
            .verifier_witness_count = 2;
        stale_init_bundle.recursive_proof.public_inputs_hash = stale_init_bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("stale init bundle public-input hash");
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_from_init_result(
                &init_request,
                &stale_init_bundle
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch { field: "hop_count" })
        ));

        let mut step1 = kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-lineage-hop-1");
        step1.input_nullifiers = vec![note0.spend_nullifier];
        let note1 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step1.output_commitments[1],
            spend_nullifier: fixed_hash(b"recursive-lineage-note-1-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence1 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step1.clone(),
            b"recursive-lineage-witness-hop-1",
        );
        let accumulator1 = kagemusha_recursive_spend_accumulator_append_evidence(
            &bundle0.accumulator,
            &bundle0.recursive_proof,
            &evidence1,
            &note1,
        )
        .expect("append recursive spend accumulator");
        let bundle1 = kagemusha_recursive_spend_bundle(accumulator1);
        let append_request = KagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle: bundle0.clone(),
            record_bundle: kagemusha_recursive_spend_record_bundle_for_step(
                chain_id,
                asset,
                &step1,
                "kagemusha-recursive-lineage-hop-1",
                b"recursive-lineage-proof-hop-1",
            ),
            pallas_open_envelopes_archive: kagemusha_recursive_spend_pallas_open_envelope_archive(
                0x51,
            ),
            current_note: note1.clone(),
        };
        let witness1 = kagemusha_recursive_spend_lineage_witness_append_result(
            &witness0,
            &append_request,
            &bundle1,
        )
        .expect("appended lineage witness");
        assert_eq!(witness1.current_notes, vec![note0, note1]);
        assert_eq!(
            witness1.previous_recursive_proofs,
            vec![bundle0.recursive_proof.clone()]
        );
        let envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&witness1.pallas_open_envelopes_archive)
                .expect("decode merged Pallas envelope archive");
        assert_eq!(envelopes.len(), 2);

        let mut redeem_proof = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0xA5; 64]),
            VerifyingKeyId::new("halo2/ipa", "kagemusha-unshield-fixture"),
        );
        redeem_proof.vk_commitment = Some(fixed_hash(b"recursive-lineage-unshield-vk"));
        let valid_redeem_request = KagemushaRecursiveSpendRedeemRequestV1 {
            bundle: bundle1.clone(),
            recipient: sample_account(0xB3, "offline"),
            public_amount: 42,
            redeem_proof,
            lineage_witness: Some(witness1.clone()),
        };
        valid_redeem_request
            .validate_public_binding()
            .expect("redeem request accepts assembled lineage witness");

        let mut reserved_previous_proof = valid_redeem_request.clone();
        let lineage_previous_proof = kagemusha_recursive_spend_lineage_proof(
            &bundle0.accumulator,
            b"recursive-lineage-previous-proof-scalar",
        );
        reserved_previous_proof
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .previous_recursive_proofs[0] = lineage_previous_proof;
        assert!(matches!(
            reserved_previous_proof.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.previous_recursive_proofs.verifier_key_id.name"
            })
        ));

        let mut scalar_spliced_previous_proof = valid_redeem_request.clone();
        let previous_proof = &mut scalar_spliced_previous_proof
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .previous_recursive_proofs[0];
        previous_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            fixed_hash(b"recursive-lineage-semantic-previous-proof-scalar-splice");
        previous_proof.public_inputs_hash = previous_proof
            .public_inputs
            .public_inputs_hash()
            .expect("scalar-spliced previous proof public-input hash");
        assert!(matches!(
            scalar_spliced_previous_proof.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.previous_recursive_proofs.recursive_verifier_scalar_projection_digest"
            })
        ));

        let mut out_of_order_previous_proof = valid_redeem_request;
        let previous_proof = &mut out_of_order_previous_proof
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .previous_recursive_proofs[0];
        previous_proof.public_inputs.hop_count = 2;
        previous_proof.public_inputs.verifier_witness_count = 2;
        previous_proof.public_inputs_hash = previous_proof
            .public_inputs
            .public_inputs_hash()
            .expect("out-of-order previous proof public-input hash");
        assert!(matches!(
            out_of_order_previous_proof.validate_public_binding(),
            Err(KagemushaFoldError::HopCountMismatch {
                expected: 1,
                actual: 2
            })
        ));

        let mut bad_init = init_request;
        bad_init.pallas_open_envelopes_archive = vec![0x01, 0x02];
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_from_init_result(&bad_init, &bundle0),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.pallas_open_envelopes_archive"
            })
        ));

        let mut conflicting_append = append_request;
        let previous_entry = &witness0.record_bundle.verifier_records[0];
        conflicting_append.record_bundle.bundle.steps[0]
            .attachment
            .vk_ref = previous_entry.id.clone();
        conflicting_append.record_bundle.verifier_records[0].id = previous_entry.id.clone();
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_append_result(
                &witness0,
                &conflicting_append,
                &bundle0
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.conflict"
            })
        ));
    }

    #[test]
    fn kagemusha_recursive_spend_rejects_malformed_notes_and_lineage() {
        let chain_id: ChainId = "kagemusha-recursive-spend-chain".parse().expect("chain id");
        let other_chain_id: ChainId = "kagemusha-recursive-spend-other-chain"
            .parse()
            .expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend");
        let other_asset = kagemusha_asset("kgm-recursive-spend-other");
        let root0 = fixed_hash(b"kagemusha-recursive-spend-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-spend-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-spend-root-2");

        let step0 = kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-spend-hop-0");
        let note0 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step0.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-spend-note-0-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence0 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step0.clone(),
            b"recursive-spend-witness-hop-0",
        );
        let accumulator0 =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence0, &note0)
                .expect("initial recursive spend accumulator");
        let previous_proof0 = kagemusha_recursive_spend_proof(&accumulator0);
        let mut missing_topup_anchor = accumulator0.clone();
        missing_topup_anchor.topup_anchor_nullifiers.clear();
        assert!(matches!(
            missing_topup_anchor.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers"
            })
        ));

        let mut zero_topup_anchor = accumulator0.clone();
        zero_topup_anchor.topup_anchor_nullifiers[0] = [0u8; Hash::LENGTH];
        assert!(matches!(
            zero_topup_anchor.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers"
            })
        ));

        let mut duplicate_topup_anchor = accumulator0.clone();
        duplicate_topup_anchor.topup_anchor_nullifiers[1] =
            duplicate_topup_anchor.topup_anchor_nullifiers[0];
        assert!(matches!(
            duplicate_topup_anchor.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers"
            })
        ));

        let mut unsorted_topup_anchor = accumulator0.clone();
        unsorted_topup_anchor.topup_anchor_nullifiers.swap(0, 1);
        assert!(matches!(
            unsorted_topup_anchor.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers"
            })
        ));

        let mut reused_final_nullifier_anchor = accumulator0.clone();
        reused_final_nullifier_anchor.topup_anchor_nullifiers[0] = note0.spend_nullifier;
        reused_final_nullifier_anchor
            .topup_anchor_nullifiers
            .sort_unstable();
        assert!(matches!(
            reused_final_nullifier_anchor.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers"
            })
        ));

        let mut reused_final_commitment_anchor = accumulator0.clone();
        reused_final_commitment_anchor.topup_anchor_nullifiers[0] = note0.note_commitment;
        reused_final_commitment_anchor
            .topup_anchor_nullifiers
            .sort_unstable();
        assert!(matches!(
            reused_final_commitment_anchor.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers"
            })
        ));

        let mut detached_aggregation_transcript = accumulator0.clone();
        detached_aggregation_transcript.aggregation_transcript_digest[0] ^= 0x01;
        assert!(matches!(
            detached_aggregation_transcript.validate_context(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "aggregation_transcript_digest"
            })
        ));

        let mut zero_commitment_note = note0.clone();
        zero_commitment_note.note_commitment = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &zero_commitment_note
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "note_commitment"
            })
        ));

        let mut zero_nullifier_note = note0.clone();
        zero_nullifier_note.spend_nullifier = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &zero_nullifier_note
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier"
            })
        ));

        let mut duplicate_note_fields = note0.clone();
        duplicate_note_fields.spend_nullifier = duplicate_note_fields.note_commitment;
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &duplicate_note_fields
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier"
            })
        ));

        let mut sibling_output_nullifier_note = note0.clone();
        sibling_output_nullifier_note.spend_nullifier = step0.output_commitments[1];
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &sibling_output_nullifier_note
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier"
            })
        ));

        let mut zero_amount_note = note0.clone();
        zero_amount_note.amount = Numeric::new(0, 0);
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &zero_amount_note
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" })
        ));

        let mut fractional_amount_note = note0.clone();
        fractional_amount_note.amount = Numeric::new(425, 1);
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &fractional_amount_note
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" })
        ));

        let mut negative_amount_note = note0.clone();
        negative_amount_note.amount = Numeric::new(-42, 0);
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &negative_amount_note
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" })
        ));

        let missing_output_note = kagemusha_recursive_spend_note(
            b"recursive-spend-forged-output",
            b"recursive-spend-forged-nullifier",
            42,
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &missing_output_note
            ),
            Err(KagemushaFoldError::RecursiveSpendMissingCurrentNoteCommitment)
        ));

        let mut step1 = kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-spend-hop-1");
        let note1 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step1.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-spend-note-1-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let missing_previous_evidence = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step1.clone(),
            b"recursive-spend-witness-hop-1",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &missing_previous_evidence,
                &note1
            ),
            Err(KagemushaFoldError::RecursiveSpendMissingPreviousNullifier)
        ));

        let mut merged_external_input_step = step1.clone();
        merged_external_input_step.input_nullifiers[0] = note0.spend_nullifier;
        let merged_external_input = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            merged_external_input_step,
            b"recursive-spend-witness-hop-1",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &merged_external_input,
                &note1
            ),
            Err(KagemushaFoldError::RecursiveSpendUnexpectedAppendInput)
        ));

        step1.input_nullifiers = vec![note0.spend_nullifier];
        let append_evidence = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step1.clone(),
            b"recursive-spend-witness-hop-1",
        );
        let valid_append_accumulator = kagemusha_recursive_spend_accumulator_append_evidence(
            &accumulator0,
            &previous_proof0,
            &append_evidence,
            &note1,
        )
        .expect("valid append evidence");
        let mut previous_public_input_splice = previous_proof0.clone();
        previous_public_input_splice.public_inputs.evidence_digest[0] ^= 0x01;
        previous_public_input_splice.public_inputs_hash = previous_public_input_splice
            .public_inputs
            .public_inputs_hash()
            .expect("spliced previous proof public-input hash");
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_public_input_splice,
                &append_evidence,
                &note1,
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "previous_recursive_proof.evidence_digest"
            })
        ));
        let mut previous_proof_byte_splice = previous_proof0.clone();
        previous_proof_byte_splice.proof.bytes[0] ^= 0x01;
        let byte_splice_accumulator = kagemusha_recursive_spend_accumulator_append_evidence(
            &accumulator0,
            &previous_proof_byte_splice,
            &append_evidence,
            &note1,
        )
        .expect("proof-byte splice is bound into accumulator state");
        assert_ne!(
            byte_splice_accumulator.recursive_proof_chain_digest,
            valid_append_accumulator.recursive_proof_chain_digest
        );
        let mut table_base_rotation = append_evidence.clone();
        table_base_rotation.fixed_window_table_base_digest =
            fixed_hash(b"recursive-spend-rotated-table-base");
        let rotated_table_base_accumulator = kagemusha_recursive_spend_accumulator_append_evidence(
            &accumulator0,
            &previous_proof0,
            &table_base_rotation,
            &note1,
        )
        .expect("per-hop fixed-window table-base digest must stream across append");
        assert_ne!(
            rotated_table_base_accumulator.fixed_window_table_base_digest,
            accumulator0.fixed_window_table_base_digest
        );
        assert_ne!(
            rotated_table_base_accumulator.fixed_window_table_base_digest,
            table_base_rotation.fixed_window_table_base_digest
        );
        let mut amount_drift_note = note1.clone();
        amount_drift_note.amount = Numeric::new(43, 0);
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &append_evidence,
                &amount_drift_note,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" })
        ));
        let mut reused_input_nullifier_note = note1.clone();
        reused_input_nullifier_note.spend_nullifier = note0.spend_nullifier;
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &kagemusha_recursive_spend_one_hop_evidence(
                    &chain_id,
                    &asset,
                    step1.clone(),
                    b"recursive-spend-witness-hop-1-reused-nullifier",
                ),
                &reused_input_nullifier_note,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier"
            })
        ));
        let mut reused_previous_commitment_nullifier_note = note1.clone();
        reused_previous_commitment_nullifier_note.spend_nullifier = note0.note_commitment;
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &append_evidence,
                &reused_previous_commitment_nullifier_note,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier"
            })
        ));

        let mut reused_previous_commitment_output_step = step1.clone();
        reused_previous_commitment_output_step.output_commitments[0] = note0.note_commitment;
        let reused_previous_commitment_output_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: reused_previous_commitment_output_step.output_commitments[1],
            spend_nullifier: fixed_hash(b"recursive-spend-note-1-output-commitment-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let reused_previous_commitment_output = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            reused_previous_commitment_output_step,
            b"recursive-spend-witness-hop-1-output-reuses-previous-commitment",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &reused_previous_commitment_output,
                &reused_previous_commitment_output_note,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "output_commitments"
            })
        ));

        let mut reused_topup_anchor_output_step = step1.clone();
        reused_topup_anchor_output_step.output_commitments[0] =
            accumulator0.topup_anchor_nullifiers[0];
        let reused_topup_anchor_output_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: reused_topup_anchor_output_step.output_commitments[1],
            spend_nullifier: fixed_hash(b"recursive-spend-note-1-topup-anchor-output-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let reused_topup_anchor_output = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            reused_topup_anchor_output_step,
            b"recursive-spend-witness-hop-1-output-reuses-topup-anchor",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &reused_topup_anchor_output,
                &reused_topup_anchor_output_note,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "output_commitments"
            })
        ));

        let chain_mismatch = kagemusha_recursive_spend_one_hop_evidence(
            &other_chain_id,
            &asset,
            step1.clone(),
            b"recursive-spend-witness-hop-1",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &chain_mismatch,
                &note1
            ),
            Err(KagemushaFoldError::RecursiveSpendChainMismatch)
        ));

        let asset_mismatch = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &other_asset,
            step1.clone(),
            b"recursive-spend-witness-hop-1",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &asset_mismatch,
                &note1
            ),
            Err(KagemushaFoldError::RecursiveSpendAssetMismatch)
        ));

        let mut root_mismatch_step = step1.clone();
        root_mismatch_step.root_before = fixed_hash(b"recursive-spend-forged-root-before");
        let root_mismatch = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            root_mismatch_step,
            b"recursive-spend-witness-hop-1",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &root_mismatch,
                &note1
            ),
            Err(KagemushaFoldError::RecursiveSpendRootMismatch)
        ));

        let mut verifier_context_mismatch = append_evidence;
        verifier_context_mismatch.fixed_window_table_schedule_digest =
            fixed_hash(b"recursive-spend-forged-schedule");
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &verifier_context_mismatch,
                &note1
            ),
            Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                field: "fixed_window_table_schedule_digest"
            })
        ));
    }

    #[test]
    fn kagemusha_recursive_spend_bundle_rejects_public_input_tampering() {
        let chain_id: ChainId = "kagemusha-recursive-spend-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend");
        let root0 = fixed_hash(b"kagemusha-recursive-spend-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-spend-root-1");
        let step0 = kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-spend-hop-0");
        let note0 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step0.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-spend-note-0-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence0 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step0.clone(),
            b"recursive-spend-witness-hop-0",
        );
        let accumulator =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence0, &note0)
                .expect("initial recursive spend accumulator");
        let bundle = kagemusha_recursive_spend_bundle(accumulator);
        bundle
            .validate_public_input_binding()
            .expect("valid recursive spend bundle");
        assert_eq!(
            bundle
                .recursive_proof
                .public_inputs
                .recursive_proof_chain_digest,
            bundle.accumulator.recursive_proof_chain_digest
        );
        assert_eq!(
            bundle
                .recursive_proof
                .public_inputs
                .recursive_verifier_scalar_projection_digest,
            [0u8; Hash::LENGTH]
        );

        let mut wrong_domain = bundle.clone();
        wrong_domain.accumulator.domain = "iroha:kagemusha:v1:recursive-spend-forged".into();
        assert!(matches!(
            wrong_domain.validate_public_input_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendAccumulatorDomain { .. })
        ));

        let mut forged_evidence_digest = bundle.clone();
        forged_evidence_digest
            .recursive_proof
            .public_inputs
            .evidence_digest = fixed_hash(b"recursive-spend-forged-evidence-digest");
        forged_evidence_digest.recursive_proof.public_inputs_hash = forged_evidence_digest
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged recursive spend public-input hash");
        assert!(matches!(
            forged_evidence_digest.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "evidence_digest"
            })
        ));

        let mut forged_hash = bundle.clone();
        forged_hash.recursive_proof.public_inputs_hash =
            Hash::new(b"recursive-spend-forged-public-input-hash");
        assert!(matches!(
            forged_hash.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveAggregationProofPublicInputHashMismatch { .. })
        ));

        let mut forged_topup_anchor = bundle.clone();
        forged_topup_anchor.accumulator.topup_anchor_nullifiers[0][0] ^= 0x01;
        forged_topup_anchor
            .accumulator
            .topup_anchor_nullifiers
            .sort_unstable();
        assert!(matches!(
            forged_topup_anchor.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "evidence_digest"
            })
        ));

        let mut forged_proof_chain_public_input = bundle.clone();
        forged_proof_chain_public_input
            .recursive_proof
            .public_inputs
            .recursive_proof_chain_digest =
            fixed_hash(b"recursive-spend-forged-proof-chain-public-input");
        forged_proof_chain_public_input
            .recursive_proof
            .public_inputs_hash = forged_proof_chain_public_input
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged proof-chain public-input hash");
        assert!(matches!(
            forged_proof_chain_public_input.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "recursive_proof_chain_digest"
            })
        ));

        let mut forged_scalar_projection_public_input = bundle.clone();
        forged_scalar_projection_public_input
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            fixed_hash(b"recursive-spend-forged-scalar-projection-public-input");
        forged_scalar_projection_public_input
            .recursive_proof
            .public_inputs_hash = forged_scalar_projection_public_input
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged scalar projection public-input hash");
        assert!(matches!(
            forged_scalar_projection_public_input.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "recursive_verifier_scalar_projection_digest"
            })
        ));

        let mut lineage_bundle = bundle.clone();
        lineage_bundle.recursive_proof = kagemusha_recursive_spend_lineage_proof(
            &lineage_bundle.accumulator,
            b"recursive-spend-lineage-scalar-projection",
        );
        lineage_bundle
            .validate_public_input_binding()
            .expect("reserved lineage recursive spend proof binding");

        let mut zero_lineage_scalar = lineage_bundle.clone();
        zero_lineage_scalar
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest = [0u8; Hash::LENGTH];
        zero_lineage_scalar.recursive_proof.public_inputs_hash = zero_lineage_scalar
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("zero lineage scalar public-input hash");
        assert!(matches!(
            zero_lineage_scalar.validate_public_input_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "recursive_verifier_scalar_projection_digest"
            })
        ));

        let mut lineage_forged_evidence = lineage_bundle.clone();
        lineage_forged_evidence
            .recursive_proof
            .public_inputs
            .evidence_digest = fixed_hash(b"recursive-spend-lineage-forged-evidence");
        lineage_forged_evidence.recursive_proof.public_inputs_hash = lineage_forged_evidence
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged lineage evidence public-input hash");
        assert!(matches!(
            lineage_forged_evidence.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "evidence_digest"
            })
        ));

        let mut unknown_lineage_circuit = lineage_bundle;
        unknown_lineage_circuit.recursive_proof.verifier_key_id.name =
            "kagemusha-recursive-spend-lineage-dev".to_owned();
        assert!(matches!(
            unknown_lineage_circuit.validate_public_input_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "verifier_key_id.name"
            })
        ));

        let mut forged_proof_chain = bundle.clone();
        forged_proof_chain.accumulator.recursive_proof_chain_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            forged_proof_chain.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "recursive_proof_chain_digest"
            })
        ));

        let mut forged_proof_backend = bundle;
        forged_proof_backend.recursive_proof.proof =
            ProofBox::new("halo2/kzg".into(), vec![0xA5; 256]);
        assert!(matches!(
            forged_proof_backend.validate_public_input_binding(),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_request_binds_public_amount() {
        let chain_id: ChainId = "kagemusha-recursive-spend-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend");
        let root0 = fixed_hash(b"kagemusha-recursive-spend-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-spend-root-1");
        let step0 = kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-spend-hop-0");
        let note0 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step0.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-spend-note-0-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence0 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step0.clone(),
            b"recursive-spend-witness-hop-0",
        );
        let accumulator =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence0, &note0)
                .expect("initial recursive spend accumulator");
        let bundle = kagemusha_recursive_spend_bundle(accumulator);
        let mut redeem_proof = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0xA5; 64]),
            VerifyingKeyId::new("halo2/ipa", "kagemusha-unshield-fixture"),
        );
        redeem_proof.vk_commitment = Some(fixed_hash(b"recursive-spend-unshield-vk"));
        let recipient = sample_account(0xB2, "offline");
        let valid = KagemushaRecursiveSpendRedeemRequestV1 {
            bundle: bundle.clone(),
            recipient: recipient.clone(),
            public_amount: 42,
            redeem_proof: redeem_proof.clone(),
            lineage_witness: None,
        };
        valid
            .validate_public_binding()
            .expect("redeem request amount binding");

        let vk_id = VerifyingKeyId::new("halo2/ipa", "kagemusha-recursive-hop-fixture");
        let proof = ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE, 0x02]);
        let mut attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof, vk_id.clone());
        let vk_commitment = fixed_hash(b"kagemusha-recursive-hop-vk");
        attachment.vk_commitment = Some(vk_commitment);
        let verifier_key = VerifyingKeyBox::new("halo2/ipa".into(), vec![0x42; 48]);
        let lineage_step = KagemushaVerifiedFoldStep {
            root_before: step0.root_before,
            input_nullifiers: step0.input_nullifiers.clone(),
            output_commitments: step0.output_commitments.clone(),
            root_after: step0.root_after,
            attachment,
            verifier_key: verifier_key.clone(),
        };
        let mut record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:kagemusha-hop-fixture",
            BackendTag::Halo2IpaPasta,
            "pasta",
            fixed_hash(b"kagemusha-recursive-hop-schema"),
            vk_commitment,
        );
        record.status = ConfidentialStatus::Active;
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.max_proof_bytes = 4096;
        record.key = Some(verifier_key);
        let lineage_witness = KagemushaRecursiveSpendLineageWitnessV1 {
            record_bundle: KagemushaVerifiedFoldRecordBundle {
                bundle: KagemushaVerifiedFoldBundle {
                    chain_id: chain_id.clone(),
                    asset: asset.clone(),
                    steps: vec![lineage_step],
                },
                verifier_records: vec![KagemushaVerifiedFoldVerifierRecord { id: vk_id, record }],
            },
            pallas_open_envelopes_archive: kagemusha_recursive_spend_pallas_open_envelope_archive(
                0x61,
            ),
            current_notes: vec![note0.clone()],
            previous_recursive_proofs: Vec::new(),
        };
        let mut valid_with_lineage = valid.clone();
        valid_with_lineage.lineage_witness = Some(lineage_witness.clone());
        valid_with_lineage
            .validate_public_binding()
            .expect("redeem request accepts well-shaped lineage witness");

        let mut malformed_archive = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.pallas_open_envelopes_archive = vec![0xE1, 0xE2];
        malformed_archive.lineage_witness = Some(bad_witness);
        assert!(matches!(
            malformed_archive.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.pallas_open_envelopes_archive"
            })
        ));

        let mut envelope_count_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        let mut envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&bad_witness.pallas_open_envelopes_archive)
                .expect("decode one-hop Pallas envelope archive");
        let mut extra_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&kagemusha_recursive_spend_pallas_open_envelope_archive(
                0x62,
            ))
            .expect("decode extra Pallas envelope archive");
        envelopes.append(&mut extra_envelopes);
        bad_witness.pallas_open_envelopes_archive =
            to_bytes(&envelopes).expect("encode two-hop Pallas envelope archive");
        envelope_count_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            envelope_count_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::HopCountMismatch {
                expected: 1,
                actual: 2
            })
        ));

        let mut note_count_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.current_notes.clear();
        note_count_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            note_count_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::HopCountMismatch {
                expected: 1,
                actual: 0
            })
        ));

        let mut final_note_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.current_notes[0].spend_nullifier =
            fixed_hash(b"recursive-spend-wrong-final-nullifier");
        final_note_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            final_note_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.final"
            })
        ));

        let mut missing_record = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.verifier_records.clear();
        missing_record.lineage_witness = Some(bad_witness);
        assert!(matches!(
            missing_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.missing"
            })
        ));

        let mut duplicate_record = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        let duplicate = bad_witness.record_bundle.verifier_records[0].clone();
        bad_witness.record_bundle.verifier_records.push(duplicate);
        duplicate_record.lineage_witness = Some(bad_witness);
        assert!(matches!(
            duplicate_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.duplicate"
            })
        ));

        let mut unreferenced_record = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        let mut extra = bad_witness.record_bundle.verifier_records[0].clone();
        extra.id = VerifyingKeyId::new("halo2/ipa", "unused-kagemusha-hop-fixture");
        bad_witness.record_bundle.verifier_records.push(extra);
        unreferenced_record.lineage_witness = Some(bad_witness);
        assert!(matches!(
            unreferenced_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.unreferenced"
            })
        ));

        let mut previous_proof_count_mismatch = valid.clone();
        let mut bad_witness = lineage_witness;
        bad_witness
            .previous_recursive_proofs
            .push(bundle.recursive_proof.clone());
        previous_proof_count_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            previous_proof_count_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::HopCountMismatch {
                expected: 0,
                actual: 1
            })
        ));

        let mut valid_with_envelope_hash = valid.clone();
        valid_with_envelope_hash.redeem_proof.envelope_hash =
            Some(Hash::new(&valid_with_envelope_hash.redeem_proof.proof.bytes).into());
        valid_with_envelope_hash
            .validate_public_binding()
            .expect("redeem request accepts matching envelope hash");

        let mut lineage_valid = valid.clone();
        lineage_valid.bundle.recursive_proof = kagemusha_recursive_spend_lineage_proof(
            &lineage_valid.bundle.accumulator,
            b"recursive-spend-redeem-lineage-scalar-projection",
        );
        lineage_valid
            .validate_public_binding()
            .expect("redeem request accepts reserved lineage proof profile");

        let mut stark_recursive_bundle = valid.clone();
        stark_recursive_bundle.bundle.recursive_proof.proof =
            ProofBox::new("stark/fri/production".into(), vec![0xA5; 64]);
        stark_recursive_bundle
            .bundle
            .recursive_proof
            .verifier_key_id = VerifyingKeyId::new(
            "stark/fri/production",
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        );
        assert!(matches!(
            stark_recursive_bundle.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "proof.backend"
            })
        ));

        let mut empty_recursive_proof = valid.clone();
        empty_recursive_proof.bundle.recursive_proof.proof =
            ProofBox::new("halo2/ipa".into(), Vec::new());
        assert!(matches!(
            empty_recursive_proof.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "proof.bytes"
            })
        ));

        let wrong_amount = KagemushaRecursiveSpendRedeemRequestV1 {
            bundle: bundle.clone(),
            recipient: recipient.clone(),
            public_amount: 41,
            redeem_proof: redeem_proof.clone(),
            lineage_witness: None,
        };
        assert!(matches!(
            wrong_amount.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "public_amount"
            })
        ));

        let mut bad_redeem_backend = valid.clone();
        bad_redeem_backend.redeem_proof = ProofAttachment::new_ref(
            "groth16".into(),
            ProofBox::new("groth16".into(), vec![0xA5; 64]),
            VerifyingKeyId::new("groth16", "kagemusha-unshield-fixture"),
        );
        assert!(matches!(
            bad_redeem_backend.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof { field: "backend" })
        ));

        let mut bad_proof_backend = valid.clone();
        bad_proof_backend.redeem_proof.proof = ProofBox::new("halo2/kzg".into(), vec![0xA5; 64]);
        assert!(matches!(
            bad_proof_backend.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "proof.backend"
            })
        ));

        let mut bad_vk_backend = valid.clone();
        bad_vk_backend.redeem_proof.vk_ref =
            VerifyingKeyId::new("halo2/kzg", "kagemusha-unshield-fixture");
        assert!(matches!(
            bad_vk_backend.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "vk_ref.backend"
            })
        ));

        let mut empty_vk_name = valid.clone();
        empty_vk_name.redeem_proof.vk_ref = VerifyingKeyId::new("halo2/ipa", "   ");
        assert!(matches!(
            empty_vk_name.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "vk_ref.name"
            })
        ));

        let mut empty_proof = valid.clone();
        empty_proof.redeem_proof.proof = ProofBox::new("halo2/ipa".into(), Vec::new());
        assert!(matches!(
            empty_proof.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "proof.bytes"
            })
        ));

        let mut missing_vk_commitment = valid.clone();
        missing_vk_commitment.redeem_proof.vk_commitment = None;
        assert!(matches!(
            missing_vk_commitment.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "vk_commitment"
            })
        ));

        let mut zero_vk_commitment = valid.clone();
        zero_vk_commitment.redeem_proof.vk_commitment = Some([0u8; Hash::LENGTH]);
        assert!(matches!(
            zero_vk_commitment.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "vk_commitment"
            })
        ));

        let mut bad_envelope_hash = valid.clone();
        bad_envelope_hash.redeem_proof.envelope_hash =
            Some(fixed_hash(b"recursive-spend-bad-envelope-hash"));
        assert!(matches!(
            bad_envelope_hash.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "envelope_hash"
            })
        ));

        let zero_amount = KagemushaRecursiveSpendRedeemRequestV1 {
            bundle,
            recipient,
            public_amount: 0,
            redeem_proof,
            lineage_witness: None,
        };
        assert!(matches!(
            zero_amount.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "public_amount"
            })
        ));
    }

    #[test]
    fn kagemusha_recursive_spend_payload_size_is_hop_count_independent() {
        const FIXED_PROOF_PAYLOAD_BUNDLE_LEN: usize = 1_553;
        const FIXED_PROOF_PAYLOAD_MATERIAL_GROWTH_CEILING: usize = 1_600;

        let chain_id: ChainId = "kagemusha-recursive-spend-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend");
        let target_hops = [1usize, 2, 3, 5, 8, 13, 21, 34, 55, 64];
        let mut observed = Vec::new();
        let mut previous = None::<KagemushaRecursiveSpendAccumulatorV1>;
        let mut previous_proof = None::<KagemushaRecursiveAggregationProof>;

        for hop_index in 0..KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
            let root_before =
                fixed_hash(format!("recursive-spend-size-root-{hop_index}").as_bytes());
            let root_after =
                fixed_hash(format!("recursive-spend-size-root-{}", hop_index + 1).as_bytes());
            let input_seed = u8::try_from(0x20 + hop_index).expect("input seed fits");
            let proof_label = format!("recursive-spend-size-hop-{hop_index}");
            let mut step = kagemusha_step(
                previous.as_ref().map_or(root_before, |acc| acc.final_root),
                root_after,
                input_seed,
                0x80,
                b"recursive-spend-size-hop",
            );
            step.output_commitments = vec![
                fixed_hash(format!("recursive-spend-size-output-{hop_index}-0").as_bytes()),
                fixed_hash(format!("recursive-spend-size-output-{hop_index}-1").as_bytes()),
            ];
            step.proof_hash = Hash::new(proof_label.as_bytes());
            step.proof_public_inputs_digest =
                fixed_hash(format!("{proof_label}:public-inputs").as_bytes());
            step.verifier_key_commitment = fixed_hash(format!("{proof_label}:vk").as_bytes());
            step.verifier_key_poseidon_digest =
                kagemusha_verifier_key_poseidon_digest("halo2/ipa", proof_label.as_bytes())
                    .expect("size verifier-key digest");
            if let Some(previous) = previous.as_ref() {
                step.input_nullifiers = vec![previous.current_note.spend_nullifier];
            }
            let note = KagemushaSpendableNoteDescriptorV1 {
                note_commitment: step.output_commitments[0],
                spend_nullifier: fixed_hash(
                    format!("recursive-spend-size-nullifier-{hop_index}").as_bytes(),
                ),
                amount: Numeric::new(42, 0),
            };
            let evidence = kagemusha_recursive_spend_one_hop_evidence(
                &chain_id,
                &asset,
                step,
                format!("recursive-spend-size-witness-{hop_index}").as_bytes(),
            );
            let accumulator = match previous.as_ref() {
                Some(previous) => kagemusha_recursive_spend_accumulator_append_evidence(
                    previous,
                    previous_proof.as_ref().expect("previous recursive proof"),
                    &evidence,
                    &note,
                )
                .expect("append size accumulator"),
                None => {
                    kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence, &note)
                        .expect("initial size accumulator")
                }
            };
            let hop_count = usize::try_from(accumulator.hop_count).expect("hop count fits");
            if target_hops.contains(&hop_count) {
                let bundle = kagemusha_recursive_spend_bundle(accumulator.clone());
                bundle
                    .validate_public_input_binding()
                    .expect("size bundle binding");
                observed.push((
                    hop_count,
                    bundle
                        .norito_encoded_len()
                        .expect("recursive spend bundle encoded length"),
                ));
            }
            previous_proof = Some(kagemusha_recursive_spend_proof(&accumulator));
            previous = Some(accumulator);
        }

        assert_eq!(observed.len(), target_hops.len());
        let first_len = observed[0].1;
        assert_eq!(
            first_len, FIXED_PROOF_PAYLOAD_BUNDLE_LEN,
            "recursive Kagemusha fixed-proof fixture archive length changed"
        );
        assert!(
            first_len <= FIXED_PROOF_PAYLOAD_MATERIAL_GROWTH_CEILING,
            "recursive Kagemusha fixed-proof fixture archive exceeded the material-growth ceiling: {first_len} > {FIXED_PROOF_PAYLOAD_MATERIAL_GROWTH_CEILING}"
        );
        for (hop_count, len) in observed {
            assert_eq!(
                len, first_len,
                "recursive Kagemusha D2D payload grew at hop {hop_count}: {len} != {first_len}"
            );
        }
    }

    #[test]
    fn kagemusha_recursive_spend_append_rejects_hop_count_above_cap() {
        let chain_id: ChainId = "kagemusha-recursive-spend-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend");
        let mut previous = None::<KagemushaRecursiveSpendAccumulatorV1>;
        let mut previous_proof = None::<KagemushaRecursiveAggregationProof>;

        for hop_index in 0..KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
            let root_after =
                fixed_hash(format!("recursive-spend-cap-root-{}", hop_index + 1).as_bytes());
            let mut step = kagemusha_step(
                previous.as_ref().map_or_else(
                    || fixed_hash(b"recursive-spend-cap-root-0"),
                    |accumulator| accumulator.final_root,
                ),
                root_after,
                u8::try_from(0x20 + hop_index).expect("input seed fits"),
                0x80,
                b"recursive-spend-cap-hop",
            );
            step.output_commitments = vec![
                fixed_hash(format!("recursive-spend-cap-output-{hop_index}-0").as_bytes()),
                fixed_hash(format!("recursive-spend-cap-output-{hop_index}-1").as_bytes()),
            ];
            let proof_label = format!("recursive-spend-cap-hop-{hop_index}");
            step.proof_hash = Hash::new(proof_label.as_bytes());
            step.proof_public_inputs_digest =
                fixed_hash(format!("{proof_label}:public-inputs").as_bytes());
            step.verifier_key_commitment = fixed_hash(format!("{proof_label}:vk").as_bytes());
            step.verifier_key_poseidon_digest =
                kagemusha_verifier_key_poseidon_digest("halo2/ipa", proof_label.as_bytes())
                    .expect("cap verifier-key digest");
            if let Some(previous) = previous.as_ref() {
                step.input_nullifiers = vec![previous.current_note.spend_nullifier];
            }
            let note = KagemushaSpendableNoteDescriptorV1 {
                note_commitment: step.output_commitments[0],
                spend_nullifier: fixed_hash(
                    format!("recursive-spend-cap-nullifier-{hop_index}").as_bytes(),
                ),
                amount: Numeric::new(42, 0),
            };
            let evidence = kagemusha_recursive_spend_one_hop_evidence(
                &chain_id,
                &asset,
                step,
                format!("recursive-spend-cap-witness-{hop_index}").as_bytes(),
            );
            let accumulator = match previous.as_ref() {
                Some(previous) => kagemusha_recursive_spend_accumulator_append_evidence(
                    previous,
                    previous_proof.as_ref().expect("previous recursive proof"),
                    &evidence,
                    &note,
                )
                .expect("append capped accumulator"),
                None => {
                    kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence, &note)
                        .expect("initial capped accumulator")
                }
            };
            previous_proof = Some(kagemusha_recursive_spend_proof(&accumulator));
            previous = Some(accumulator);
        }

        let previous = previous.expect("64-hop accumulator");
        let previous_proof = previous_proof.expect("64-hop recursive proof");
        assert_eq!(
            previous.hop_count,
            u32::try_from(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS).expect("hop cap fits u32")
        );
        let mut overflow_step = kagemusha_step(
            previous.final_root,
            fixed_hash(b"recursive-spend-cap-root-65"),
            0x70,
            0xC0,
            b"recursive-spend-cap-overflow-hop",
        );
        overflow_step.input_nullifiers = vec![previous.current_note.spend_nullifier];
        let overflow_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: overflow_step.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-spend-cap-nullifier-64"),
            amount: Numeric::new(42, 0),
        };
        let overflow_evidence = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            overflow_step,
            b"recursive-spend-cap-witness-64",
        );

        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &previous,
                &previous_proof,
                &overflow_evidence,
                &overflow_note,
            ),
            Err(KagemushaFoldError::TooManyHops { actual, .. })
                if actual == KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
        ));
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

        let mut same_hop_overlap = statement.clone();
        same_hop_overlap.steps[0].output_commitments[0] =
            same_hop_overlap.steps[0].input_nullifiers[0];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&same_hop_overlap),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 0 })
        ));

        let mut cross_hop_overlap = statement.clone();
        cross_hop_overlap.steps[1].input_nullifiers[0] =
            cross_hop_overlap.steps[0].output_commitments[0];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&cross_hop_overlap),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 1 })
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

    fn kagemusha_verified_fold_record_bundle_fixture() -> KagemushaVerifiedFoldRecordBundle {
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
        record_bundle
    }

    #[test]
    fn kagemusha_verified_fold_record_bundle_roundtrips() {
        let record_bundle = kagemusha_verified_fold_record_bundle_fixture();

        let bytes = to_bytes(&record_bundle).expect("encode record-backed bundle");
        let decoded: KagemushaVerifiedFoldRecordBundle =
            norito::decode_from_bytes(&bytes).expect("decode record-backed bundle");
        assert_eq!(decoded, record_bundle);
    }

    #[test]
    fn kagemusha_recursive_spend_bridge_abi_archives_roundtrip() {
        let record_bundle = kagemusha_verified_fold_record_bundle_fixture();
        let chain_id: ChainId = "kagemusha-recursive-spend-abi-chain"
            .parse()
            .expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-abi");
        let root0 = fixed_hash(b"kagemusha-recursive-spend-abi-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-spend-abi-root-1");
        let step = kagemusha_step(root0, root1, 0x32, 0x74, b"recursive-spend-abi-hop");
        let current_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step.output_commitments[0],
            spend_nullifier: fixed_hash(b"kagemusha-recursive-spend-abi-nullifier"),
            amount: Numeric::new(7, 0),
        };
        let evidence = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step,
            b"recursive-spend-abi-witness-hop",
        );
        let accumulator =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence, &current_note)
                .expect("recursive spend ABI accumulator");
        let bundle = kagemusha_recursive_spend_bundle(accumulator);
        let pallas_open_envelopes_archive = vec![0xE1, 0xE2, 0xE3];

        let init = KagemushaRecursiveSpendInitRequestV1 {
            record_bundle: record_bundle.clone(),
            pallas_open_envelopes_archive: pallas_open_envelopes_archive.clone(),
            current_note: current_note.clone(),
        };
        let init_bytes = to_bytes(&init).expect("encode recursive spend init request");
        let decoded_init: KagemushaRecursiveSpendInitRequestV1 =
            norito::decode_from_bytes(&init_bytes).expect("decode recursive spend init request");
        assert_eq!(decoded_init, init);

        let append = KagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle: bundle.clone(),
            record_bundle: record_bundle.clone(),
            pallas_open_envelopes_archive,
            current_note: current_note.clone(),
        };
        let append_bytes = to_bytes(&append).expect("encode recursive spend append request");
        let decoded_append: KagemushaRecursiveSpendAppendRequestV1 =
            norito::decode_from_bytes(&append_bytes)
                .expect("decode recursive spend append request");
        assert_eq!(decoded_append, append);

        let verify = KagemushaRecursiveSpendVerifyRequestV1 {
            bundle: bundle.clone(),
        };
        let verify_bytes = to_bytes(&verify).expect("encode recursive spend verify request");
        let decoded_verify: KagemushaRecursiveSpendVerifyRequestV1 =
            norito::decode_from_bytes(&verify_bytes)
                .expect("decode recursive spend verify request");
        assert_eq!(decoded_verify, verify);

        let verify_result = KagemushaRecursiveSpendVerifyResultV1 {
            valid: false,
            hop_count: bundle.accumulator.hop_count,
            encoded_bytes: u32::try_from(
                bundle
                    .norito_encoded_len()
                    .expect("recursive spend bundle encoded length"),
            )
            .expect("encoded length fits u32"),
            reason: "fixture recursive proof is not a production proof".to_owned(),
            chain_admissible: false,
            chain_admission_reason: "offline verification failed".to_owned(),
        };
        let verify_result_bytes =
            to_bytes(&verify_result).expect("encode recursive spend verify result");
        let decoded_verify_result: KagemushaRecursiveSpendVerifyResultV1 =
            norito::decode_from_bytes(&verify_result_bytes)
                .expect("decode recursive spend verify result");
        assert_eq!(decoded_verify_result, verify_result);

        let mut redeem_proof = ProofAttachment::new_ref(
            "halo2/ipa".to_owned(),
            ProofBox::new("halo2/ipa".to_owned(), vec![0xA7; 64]),
            VerifyingKeyId::new("halo2/ipa", "kagemusha-recursive-spend-abi-redeem"),
        );
        redeem_proof.vk_commitment = Some(fixed_hash(b"kagemusha-recursive-spend-abi-redeem-vk"));
        let redeem = KagemushaRecursiveSpendRedeemRequestV1 {
            bundle,
            recipient: sample_account(0xAB, "offline"),
            public_amount: 7,
            redeem_proof,
            lineage_witness: None,
        };
        let redeem_bytes = to_bytes(&redeem).expect("encode recursive spend redeem request");
        let decoded_redeem: KagemushaRecursiveSpendRedeemRequestV1 =
            norito::decode_from_bytes(&redeem_bytes)
                .expect("decode recursive spend redeem request");
        assert_eq!(decoded_redeem, redeem);
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

        let mut same_hop_overlap = step0.clone();
        same_hop_overlap.output_commitments[0] = same_hop_overlap.input_nullifiers[0];
        let same_hop_overlap_steps = [same_hop_overlap];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &same_hop_overlap_steps),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &same_hop_overlap_steps
            ),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 0 })
        ));

        let mut cross_hop_overlap = step1.clone();
        cross_hop_overlap.input_nullifiers[0] = step0.output_commitments[0];
        let cross_hop_overlap_steps = [step0.clone(), cross_hop_overlap];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &cross_hop_overlap_steps),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 1 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &cross_hop_overlap_steps
            ),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 1 })
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
