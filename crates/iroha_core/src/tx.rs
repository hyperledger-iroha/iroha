//! `Transaction`-related functionality of Iroha.
//!
//! Admission derives the Nexus lane/dataspace assignment for every transaction
//! using the configured routing policy (see `specs/nexus_transition_notes.md`)
//! so telemetry, fraud monitoring, and queue accounting observe the real topology.
//!
//! Types represent various stages of a `Transaction`'s lifecycle. For
//! example, `Transaction` is the start, when a transaction had been
//! received by Torii.
//!
//! This is also where the actual execution of instructions, as well
//! as various forms of validation are performed.

use core::{fmt, str::FromStr as _};
use std::{
    borrow::Cow,
    collections::BTreeSet,
    sync::{Arc, LazyLock, OnceLock},
    time::{Duration, SystemTime, SystemTimeError},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use eyre::Result;
use hex;
pub use iroha_data_model::prelude::*;
use iroha_data_model::{
    asset::definition::ConfidentialPolicyMode,
    fraud::types::FraudAssessment,
    isi::error::Mismatch,
    isi::{
        runtime_upgrade::{ActivateRuntimeUpgrade, CancelRuntimeUpgrade, ProposeRuntimeUpgrade},
        smart_contract_code::{
            ActivateContractInstance, CommitContractDeployment, DeactivateContractInstance,
            FinalizeSmartContractCodeUpload, RegisterSmartContractBytes, RegisterSmartContractCode,
            RemoveSmartContractBytes, UploadSmartContractCodeChunk,
        },
        zk,
    },
    nexus::UniversalAccountId,
    proof::{ProofAttachment, ProofBox},
    query::error::FindError,
    smart_contract::manifest::{ContractManifest, MANIFEST_METADATA_KEY},
    transaction::signed::{
        SealedTransactionCommitmentPayload, SealedTransactionReveal,
        SignedSealedTransactionCommitment, compute_sealed_transaction_commitment,
    },
    transaction::{error::TransactionLimitError, signed::TransactionSignatureError},
    zk::OpenVerifyEnvelope,
};
use iroha_executor_data_model::isi::multisig::MultisigInstructionBox;
use iroha_logger::{debug, error, warn};
use iroha_macro::FromVariant;
use iroha_primitives::time::TimeSource;
use iroha_schema::Ident;
use mv::storage::StorageReadOnly;

use crate::{
    compliance::{LaneComplianceContext, LaneComplianceEvaluation},
    gas as isi_gas,
    governance::manifest::{GovernanceRules, LaneManifestRegistryHandle},
    interlane::verify_lane_privacy_proofs,
    nexus::space_directory::{
        LaneIdentityMetadataError,
        extract_authority_domains as extract_directory_authority_domains,
        extract_lane_identity_metadata as extract_directory_lane_identity_metadata,
    },
    queue::evaluate_policy_plan_with_nexus_and_world_at_block_height,
    smartcontracts::{Execute, code, ivm::cache::IvmCache},
    state::{StateBlock, StateReadOnlyWithTransactions, StateTransaction, WorldReadOnly},
};

#[cfg(feature = "telemetry")]
type StateTelemetry = crate::telemetry::StateTelemetry;
#[cfg(not(feature = "telemetry"))]
type StateTelemetry = ();
type NexusDataSpaceId = iroha_data_model::nexus::DataSpaceId;
type NexusLaneId = iroha_data_model::nexus::LaneId;

/// Decode one canonical Norito-framed [`TransactionEntrypoint`] and return its identity.
///
/// The identity is derived from the decoded signed intent rather than the transport frame. This
/// prevents alternate framing, authorization-proof changes, or zero-filled logical tails from
/// creating replay-distinct transaction identifiers.
pub(crate) fn entrypoint_hash_from_framed_bytes(
    framed: &[u8],
) -> Result<HashOf<TransactionEntrypoint>, norito::core::Error> {
    let entrypoint: TransactionEntrypoint = norito::decode_canonical(framed)?;
    Ok(entrypoint.hash())
}

/// Stateful admission facts that must be committed only if transaction execution succeeds.
#[derive(Debug, Clone)]
pub(crate) struct StatefulAdmission {
    /// Transaction authority.
    pub(crate) authority: AccountId,
    /// Whether this transaction may run before its authority account is materialized.
    pub(crate) allow_unregistered_authority: bool,
    /// Monotonic sequence value to store after successful execution.
    pub(crate) sequence_to_commit: Option<u64>,
    /// Exact signed validation-fee value to credit only after complete execution succeeds.
    pub(crate) validation_fee_credit: Option<crate::validation_fee::ValidationFeeCredit>,
}

#[derive(Debug, Clone, norito::codec::Decode, norito::codec::Encode)]
struct PendingSealedTransactionCommitment {
    payload: SealedTransactionCommitmentPayload,
    commit_height: u64,
    commit_index: u64,
}

const SEALED_TRANSACTION_STATE_PREFIX: &str = "sealed_tx_commitment_";

fn sealed_commitment_state_key(commitment: &iroha_crypto::Hash) -> StatePath {
    StatePath::from_str(&format!(
        "{SEALED_TRANSACTION_STATE_PREFIX}{}",
        hex::encode(commitment.as_ref())
    ))
    .expect("sealed transaction commitment key is a valid state path")
}

fn sealed_state_decode_error(error: impl fmt::Display) -> TransactionRejectionReason {
    TransactionRejectionReason::Validation(ValidationFail::InternalError(format!(
        "sealed transaction commitment state decode failed: {error}"
    )))
}

fn sealed_state_encode_error(error: impl fmt::Display) -> TransactionRejectionReason {
    TransactionRejectionReason::Validation(ValidationFail::InternalError(format!(
        "sealed transaction commitment state encode failed: {error}"
    )))
}

pub(crate) fn validate_sealed_commitment_stateless(
    commitment: &SignedSealedTransactionCommitment,
    expected_chain_id: &ChainId,
    _limits: TransactionParameters,
) -> Result<(), AcceptTransactionFail> {
    let payload = commitment.payload();
    if &payload.chain_id != expected_chain_id {
        return Err(AcceptTransactionFail::ChainIdMismatch(Mismatch {
            expected: expected_chain_id.clone(),
            actual: payload.chain_id.clone(),
        }));
    }
    if payload.reveal_deadline_height < payload.reveal_after_height {
        return Err(AcceptTransactionFail::TransactionLimit(
            TransactionLimitError {
                reason: "sealed transaction reveal deadline precedes reveal start".into(),
            },
        ));
    }
    commitment.verify_signature().map_err(|err| {
        AcceptTransactionFail::TransactionLimit(TransactionLimitError {
            reason: format!("sealed transaction commitment signature verification failed: {err}"),
        })
    })
}

pub(crate) fn sealed_reveal_execution_key(
    state_block: &StateBlock<'_>,
    reveal: &SealedTransactionReveal,
) -> (u64, iroha_crypto::Hash, u64) {
    let key = sealed_commitment_state_key(&reveal.commitment);
    let Some(bytes) = state_block.world.smart_contract_state.get(&key) else {
        return (u64::MAX, reveal.commitment, u64::MAX);
    };
    let Ok(record) = norito::decode_from_bytes::<PendingSealedTransactionCommitment>(bytes) else {
        return (u64::MAX, reveal.commitment, u64::MAX);
    };
    (record.commit_height, reveal.commitment, record.commit_index)
}

pub(crate) fn prune_expired_sealed_commitments(state_block: &mut StateBlock<'_>) -> usize {
    let height = state_block._curr_block.height().get();
    let expired_keys: Vec<_> = state_block
        .world
        .smart_contract_state
        .iter()
        .filter_map(|(key, bytes)| {
            if !key.as_ref().starts_with(SEALED_TRANSACTION_STATE_PREFIX) {
                return None;
            }
            let record: PendingSealedTransactionCommitment = match norito::decode_from_bytes(bytes)
            {
                Ok(record) => record,
                Err(error) => {
                    warn!(
                        %key,
                        %error,
                        "skipping invalid sealed transaction commitment state during expiry pruning"
                    );
                    return None;
                }
            };
            (height > record.payload.reveal_deadline_height).then(|| key.clone())
        })
        .collect();
    for key in &expired_keys {
        state_block.world.smart_contract_state.remove(key.clone());
    }
    expired_keys.len()
}

static FRAUD_ASSESSMENT_BAND_NAME: LazyLock<iroha_data_model::name::Name> = LazyLock::new(|| {
    iroha_data_model::name::Name::from_str("fraud_assessment_band")
        .expect("static band metadata name")
});
static FRAUD_ASSESSMENT_SCORE_NAME: LazyLock<iroha_data_model::name::Name> = LazyLock::new(|| {
    iroha_data_model::name::Name::from_str("fraud_assessment_score_bps")
        .expect("static score metadata name")
});
static FRAUD_ASSESSMENT_TENANT_NAME: LazyLock<iroha_data_model::name::Name> = LazyLock::new(|| {
    iroha_data_model::name::Name::from_str("fraud_assessment_tenant")
        .expect("static tenant metadata name")
});
static FRAUD_ASSESSMENT_LATENCY_NAME: LazyLock<iroha_data_model::name::Name> =
    LazyLock::new(|| {
        iroha_data_model::name::Name::from_str("fraud_assessment_latency_ms")
            .expect("static latency metadata name")
    });
static FRAUD_ASSESSMENT_ENVELOPE_NAME: LazyLock<iroha_data_model::name::Name> =
    LazyLock::new(|| {
        iroha_data_model::name::Name::from_str("fraud_assessment_envelope")
            .expect("static attestation envelope metadata name")
    });
static FRAUD_ASSESSMENT_DIGEST_NAME: LazyLock<iroha_data_model::name::Name> = LazyLock::new(|| {
    iroha_data_model::name::Name::from_str("fraud_assessment_digest")
        .expect("static attestation digest metadata name")
});
static CONTRACT_MANIFEST_METADATA_NAME: LazyLock<iroha_data_model::name::Name> =
    LazyLock::new(|| {
        iroha_data_model::name::Name::from_str(MANIFEST_METADATA_KEY)
            .expect("static contract manifest metadata key")
    });
static GOV_CONTRACT_ADDRESS_METADATA_KEY: LazyLock<iroha_data_model::name::Name> =
    LazyLock::new(|| {
        iroha_data_model::name::Name::from_str("gov_contract_address")
            .expect("static governance metadata key")
    });
static GOV_APPROVERS_METADATA_KEY: LazyLock<iroha_data_model::name::Name> = LazyLock::new(|| {
    iroha_data_model::name::Name::from_str("gov_manifest_approvers")
        .expect("static governance metadata key")
});
static CONTRACT_ADDRESS_METADATA_KEY: LazyLock<iroha_data_model::name::Name> =
    LazyLock::new(|| {
        iroha_data_model::name::Name::from_str("contract_address")
            .expect("static contract address metadata key")
    });
static HEARTBEAT_METADATA_NAME: LazyLock<iroha_data_model::name::Name> = LazyLock::new(|| {
    iroha_data_model::name::Name::from_str("sumeragi_heartbeat")
        .expect("static heartbeat metadata key")
});
pub(crate) const ED25519_SIGNATURE_LENGTH: usize = 64;
const MULTISIG_DIRECT_SIGN_REJECTION: &str =
    "multisig accounts must use the multisig propose/approve flow; direct signatures are rejected";
const CONTRACT_SUBJECT_DIRECT_SIGN_REJECTION: &str =
    "deployed contract subjects cannot originate signed transactions directly";
/// Prefix used in transaction-limit rejection reasons when the signature cap is exceeded.
pub const SIGNATURE_LIMIT_REASON_PREFIX: &str = "Too many signatures in payload";

#[derive(Clone, Debug)]
enum SignatureCheck {
    Verify,
    PrecheckedSingleEd25519,
    Override(Result<(), SignatureVerificationFail>),
}
#[cfg(feature = "telemetry")]
#[allow(clippy::module_name_repetitions)]
use iroha_data_model::{metadata::Metadata as TelemetryMetadata, name::Name as TelemetryName};
/// `AcceptedTransaction` — a transaction accepted by Iroha peer.
#[derive(Debug)]
pub struct AcceptedTransaction<'tx> {
    entrypoint: Cow<'tx, TransactionEntrypoint>,
    entrypoint_hash: OnceLock<HashOf<TransactionEntrypoint>>,
    signed_hash: OnceLock<HashOf<SignedTransaction>>,
    encoded_len: OnceLock<usize>,
    entrypoint_bytes: OnceLock<Arc<Vec<u8>>>,
    signed_bytes: OnceLock<Option<Arc<Vec<u8>>>>,
    payload_hash:
        OnceLock<Option<HashOf<iroha_data_model::transaction::signed::TransactionPayload>>>,
    single_ed25519_key: OnceLock<Option<iroha_crypto::Ed25519ParsedPublicKey>>,
}

impl Clone for AcceptedTransaction<'_> {
    fn clone(&self) -> Self {
        Self {
            entrypoint: Cow::Owned(self.entrypoint().clone()),
            entrypoint_hash: clone_once_lock(&self.entrypoint_hash),
            signed_hash: clone_once_lock(&self.signed_hash),
            encoded_len: clone_once_lock(&self.encoded_len),
            entrypoint_bytes: clone_once_lock(&self.entrypoint_bytes),
            signed_bytes: clone_once_lock(&self.signed_bytes),
            payload_hash: clone_once_lock(&self.payload_hash),
            single_ed25519_key: clone_once_lock(&self.single_ed25519_key),
        }
    }
}

impl PartialEq for AcceptedTransaction<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.entrypoint() == other.entrypoint()
    }
}

impl Eq for AcceptedTransaction<'_> {}

fn clone_once_lock<T: Clone>(source: &OnceLock<T>) -> OnceLock<T> {
    let cloned = OnceLock::new();
    if let Some(value) = source.get() {
        let _ = cloned.set(value.clone());
    }
    cloned
}

/// Accepted transaction that has been verified to be absent from the blockchain.
///
/// This wrapper is constructed by checking an [`AcceptedTransaction`] against a state view and
/// guarantees that the transaction hash was not present in the ledger at the time of creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTransaction<'tx>(AcceptedTransaction<'tx>);

/// Error returned when trying to mark an already committed transaction as pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransactionAlreadyCommitted;

impl fmt::Display for TransactionAlreadyCommitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("transaction already committed")
    }
}

impl std::error::Error for TransactionAlreadyCommitted {}

/// Reusable stateless transaction metadata derived once for block validation.
#[derive(Debug, Clone)]
pub(crate) struct PreparedTransactionMetadata {
    /// Canonical signed transaction hash.
    pub(crate) signed_hash: HashOf<SignedTransaction>,
    /// Canonical entrypoint hash used by block Merkle roots.
    pub(crate) entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Hash of the signed payload, used as the Ed25519 verification message.
    pub(crate) payload_hash: HashOf<iroha_data_model::transaction::signed::TransactionPayload>,
    /// Exact framed Norito length of the signed transaction.
    pub(crate) encoded_len: usize,
    /// Canonical signed transaction bytes when a caller has already materialized them.
    pub(crate) signed_bytes: Option<Arc<Vec<u8>>>,
    /// Canonical external entrypoint bytes derived from the same signed payload.
    pub(crate) entrypoint_bytes: Option<Arc<Vec<u8>>>,
    /// Parsed Ed25519 key when the transaction has a single Ed25519 authority.
    pub(crate) single_ed25519_key: Option<iroha_crypto::Ed25519ParsedPublicKey>,
    /// Parsed transaction metadata nesting depths, in canonical metadata iteration order.
    metadata_depths: Result<Vec<(String, usize)>, TransactionLimitError>,
}

/// Signed transaction decoded from a versioned Torii payload with reusable admission metadata.
///
/// This is public only so Torii can pass the decoded transaction across crate boundaries without
/// redoing stateless preparation. The metadata is derived internally and cannot be supplied by
/// callers.
#[doc(hidden)]
#[derive(Debug)]
pub struct DecodedVersionedSignedTransaction {
    tx: SignedTransaction,
    prepared: PreparedTransactionMetadata,
}

impl<'tx> CheckedTransaction<'tx> {
    /// Attempt to construct a [`CheckedTransaction`] by verifying the transaction hash against the provided state.
    ///
    /// # Errors
    ///
    /// Returns the original transaction and [`TransactionAlreadyCommitted`] when the hash is already present.
    #[allow(clippy::result_large_err)]
    pub fn new(
        tx: AcceptedTransaction<'tx>,
        state: &impl StateReadOnlyWithTransactions,
    ) -> Result<Self, (AcceptedTransaction<'tx>, TransactionAlreadyCommitted)> {
        if state.has_transaction(tx.hash()) {
            return Err((tx, TransactionAlreadyCommitted));
        }
        Ok(Self(tx))
    }

    /// Construct a checked transaction after the caller performed committed-hash validation.
    ///
    /// This is intended for hot requeue paths that already validated
    /// `InBlockchain` membership via a narrow state accessor.
    #[must_use]
    pub(crate) fn new_unchecked(tx: AcceptedTransaction<'tx>) -> Self {
        Self(tx)
    }

    /// Borrow the underlying [`AcceptedTransaction`].
    #[must_use]
    pub fn as_accepted(&self) -> &AcceptedTransaction<'tx> {
        &self.0
    }

    /// Consume the wrapper and return the inner [`AcceptedTransaction`].
    #[must_use]
    pub fn into_accepted(self) -> AcceptedTransaction<'tx> {
        self.0
    }

    /// Check whether the transaction is now recorded in the blockchain.
    #[must_use]
    pub fn is_in_blockchain(&self, state: &impl StateReadOnlyWithTransactions) -> bool {
        state.has_transaction(self.hash())
    }
}

impl<'tx> core::ops::Deref for CheckedTransaction<'tx> {
    type Target = AcceptedTransaction<'tx>;

    fn deref(&self) -> &Self::Target {
        self.as_accepted()
    }
}

impl<'tx> AsRef<AcceptedTransaction<'tx>> for CheckedTransaction<'tx> {
    fn as_ref(&self) -> &AcceptedTransaction<'tx> {
        self.as_accepted()
    }
}

impl<'tx> From<CheckedTransaction<'tx>> for AcceptedTransaction<'tx> {
    fn from(value: CheckedTransaction<'tx>) -> Self {
        value.into_accepted()
    }
}

fn json_value_depth(value: &norito::json::Value) -> usize {
    match value {
        norito::json::Value::Array(items) => {
            1 + items.iter().map(json_value_depth).max().unwrap_or(0)
        }
        norito::json::Value::Object(map) => {
            1 + map.values().map(json_value_depth).max().unwrap_or(0)
        }
        _ => 1,
    }
}

fn ensure_metadata_depth(
    metadata: &Metadata,
    max_depth: usize,
) -> Result<(), TransactionLimitError> {
    for (key, depth) in prepare_metadata_depths(metadata)? {
        if depth > max_depth {
            return Err(TransactionLimitError {
                reason: format!("Metadata `{key}` nesting depth {depth} exceeds limit {max_depth}"),
            });
        }
    }
    Ok(())
}

fn prepare_metadata_depths(
    metadata: &Metadata,
) -> Result<Vec<(String, usize)>, TransactionLimitError> {
    let entries = metadata.iter();
    let mut depths = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        let parsed =
            norito::json::parse_value(value.get()).map_err(|err| TransactionLimitError {
                reason: format!("Metadata `{key}` is not valid JSON: {err}"),
            })?;
        depths.push((key.to_string(), json_value_depth(&parsed)));
    }
    Ok(depths)
}

fn ensure_metadata_depth_with_prepared(
    metadata: &Metadata,
    max_depth: usize,
    prepared: Option<&PreparedTransactionMetadata>,
) -> Result<(), TransactionLimitError> {
    if let Some(prepared) = prepared {
        let depths = prepared.metadata_depths.as_ref().map_err(Clone::clone)?;
        for (key, depth) in depths {
            if *depth > max_depth {
                return Err(TransactionLimitError {
                    reason: format!(
                        "Metadata `{key}` nesting depth {depth} exceeds limit {max_depth}"
                    ),
                });
            }
        }
        Ok(())
    } else {
        ensure_metadata_depth(metadata, max_depth)
    }
}

#[derive(Debug, Clone)]
struct PrivateKaigiFeeBinding {
    action_hash_hex: String,
    chain_id: String,
    asset_definition_id: String,
    fee_amount: Quantity,
}

fn json_object_string(
    map: &norito::json::Map,
    key: &str,
    context: &str,
) -> Result<String, TransactionRejectionReason> {
    map.get(key)
        .and_then(norito::json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
                "{context} must include non-empty `{key}`"
            )))
        })
}

fn decode_private_kaigi_fee_binding(
    proof_bytes: &[u8],
) -> Result<PrivateKaigiFeeBinding, TransactionRejectionReason> {
    let envelope: OpenVerifyEnvelope = norito::decode_canonical(proof_bytes).map_err(|_| {
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "private Kaigi fee spend proof must use OpenVerifyEnvelope payload".into(),
        ))
    })?;
    if envelope.aux.is_empty() {
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(
                "private Kaigi fee spend proof is missing binding metadata".into(),
            ),
        ));
    }
    let aux = std::str::from_utf8(&envelope.aux).map_err(|_| {
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "private Kaigi fee spend aux payload must be valid UTF-8 JSON".into(),
        ))
    })?;
    let aux_value: norito::json::Value = norito::json::from_str(aux).map_err(|_| {
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "private Kaigi fee spend aux payload must be valid JSON".into(),
        ))
    })?;
    let norito::json::Value::Object(map) = aux_value else {
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(
                "private Kaigi fee spend aux payload must be a JSON object".into(),
            ),
        ));
    };
    let schema = json_object_string(&map, "schema", "private Kaigi fee spend aux payload")?;
    if schema != "iroha.private_kaigi.fee.v1" {
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(
                "private Kaigi fee spend aux payload has unsupported schema".into(),
            ),
        ));
    }
    let fee_amount_text = map
        .get("fee_amount")
        .and_then(norito::json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                "private Kaigi fee spend aux payload must include non-empty `fee_amount`".into(),
            ))
        })?;
    let fee_amount = Quantity::from_str(fee_amount_text).map_err(|err| {
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
            "private Kaigi fee amount is invalid: {err}"
        )))
    })?;
    if fee_amount_text != fee_amount.to_string() {
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(format!(
                "private Kaigi fee amount must use canonical form `{fee_amount}`"
            )),
        ));
    }

    Ok(PrivateKaigiFeeBinding {
        action_hash_hex: json_object_string(
            &map,
            "action_hash_hex",
            "private Kaigi fee spend aux payload",
        )?,
        chain_id: json_object_string(&map, "chain_id", "private Kaigi fee spend aux payload")?,
        asset_definition_id: json_object_string(
            &map,
            "asset_definition_id",
            "private Kaigi fee spend aux payload",
        )?,
        fee_amount,
    })
}

fn canonical_private_kaigi_fee_transfer_proof(
    proof_bytes: &[u8],
) -> Result<Vec<u8>, TransactionRejectionReason> {
    let mut envelope: OpenVerifyEnvelope = norito::decode_canonical(proof_bytes).map_err(|_| {
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "private Kaigi fee spend proof must use OpenVerifyEnvelope payload".into(),
        ))
    })?;
    envelope.aux.clear();
    norito::encode_canonical(&envelope).map_err(|err| {
        TransactionRejectionReason::Validation(ValidationFail::InternalError(format!(
            "failed to canonicalize private Kaigi fee spend proof: {err}"
        )))
    })
}

/// Verification failed of some signature due to following reason
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureVerificationFail {
    /// Signature which verification has failed
    pub signature: TransactionSignature,
    /// Stable rejection code associated with the failure.
    pub code: SignatureRejectionCode,
    /// Error which happened during verification
    pub detail: String,
}

impl core::fmt::Display for SignatureVerificationFail {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Failed to verify signatures ({}): {}",
            self.code.as_str(),
            self.detail,
        )
    }
}

impl std::error::Error for SignatureVerificationFail {}

/// Stable codes describing why signature verification was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureRejectionCode {
    /// Multisig controllers are not supported for transaction signatures yet.
    UnsupportedAuthority,
    /// Signature algorithm is disabled by configuration.
    AlgorithmNotPermitted,
    /// Signature failed to verify against the payload.
    InvalidSignature,
    /// Signature bytes are malformed or incomplete.
    MalformedSignature,
    /// Multisig signature bundle is missing.
    MissingSignatures,
    /// Multisig bundle references a signer outside the policy.
    UnknownSigner,
    /// Multisig bundle does not reach the configured threshold.
    InsufficientWeight,
}

impl SignatureRejectionCode {
    /// Stable machine-readable code string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedAuthority => "PRTRY:TX_UNSUPPORTED_AUTHORITY",
            Self::AlgorithmNotPermitted => "PRTRY:TX_SIGNATURE_ALGO_DENIED",
            Self::InvalidSignature => "PRTRY:TX_SIGNATURE_INVALID",
            Self::MalformedSignature => "PRTRY:TX_SIGNATURE_MALFORMED",
            Self::MissingSignatures => "PRTRY:TX_SIGNATURE_MISSING",
            Self::UnknownSigner => "PRTRY:TX_SIGNATURE_UNKNOWN_SIGNER",
            Self::InsufficientWeight => "PRTRY:TX_SIGNATURE_INSUFFICIENT",
        }
    }

    /// Human-readable summary for logs and envelopes.
    pub const fn summary(self) -> &'static str {
        match self {
            Self::UnsupportedAuthority => "authority type is not supported for transaction signing",
            Self::AlgorithmNotPermitted => "signing algorithm is not permitted by configuration",
            Self::InvalidSignature => "signature verification failed",
            Self::MalformedSignature => "signature or key encoding is malformed",
            Self::MissingSignatures => "multisig signatures are missing",
            Self::UnknownSigner => "multisig contains a signature from an unknown member",
            Self::InsufficientWeight => "multisig signatures do not satisfy the threshold",
        }
    }
}

impl SignatureVerificationFail {
    /// Construct a new failure with the given code and detail string.
    pub fn new(
        signature: TransactionSignature,
        code: SignatureRejectionCode,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            signature,
            code,
            detail: detail.into(),
        }
    }

    /// Accessor for the rejection code.
    pub const fn code(&self) -> SignatureRejectionCode {
        self.code
    }
}

/// Error type for transaction from [`SignedTransaction`] to [`AcceptedTransaction`]
#[derive(Debug, displaydoc::Display, PartialEq, Eq, FromVariant, thiserror::Error)]
pub enum AcceptTransactionFail {
    /// Failure during limits check
    TransactionLimit(#[source] TransactionLimitError),
    /// Failure during signature verification
    SignatureVerification(#[source] SignatureVerificationFail),
    /// Transaction expired at `{expires_at_ms}` ms since Unix epoch (current time `{now_ms}` ms)
    TransactionExpired {
        /// Millisecond Unix timestamp at which the transaction's TTL elapsed.
        expires_at_ms: u128,
        /// Millisecond Unix timestamp observed during admission.
        now_ms: u128,
    },
    /// Network time service is unhealthy: {reason}
    NetworkTimeUnhealthy {
        /// Health snapshot summary for diagnostics.
        reason: String,
    },
    /// The genesis account can only sign transactions in the genesis block
    UnexpectedGenesisAccountSignature,
    /// Chain id doesn't correspond to the id of current blockchain: {0}
    ChainIdMismatch(Mismatch<ChainId>),
    /// Transaction creation time is in the future
    TransactionInTheFuture,
}

fn duration_since_epoch_with_fallback(result: Result<Duration, SystemTimeError>) -> Duration {
    match result {
        Ok(duration) => duration,
        Err(error) => {
            let skew = error.duration();
            warn!(
                clock_skew_ms = u64::try_from(skew.as_millis()).unwrap_or(u64::MAX),
                "local clock is before the Unix epoch; falling back to Duration::ZERO for admission"
            );
            Duration::ZERO
        }
    }
}

fn current_unix_time() -> Duration {
    duration_since_epoch_with_fallback(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH))
}

impl DecodedVersionedSignedTransaction {
    /// Decode a versioned signed transaction and prepare admission metadata once.
    ///
    /// # Errors
    ///
    /// Returns the same versioning and Norito decode errors as the signed transaction
    /// versioned decoder.
    pub fn decode_versioned(input: &[u8]) -> iroha_version::error::Result<Self> {
        let tx =
            <SignedTransaction as iroha_version::codec::DecodeVersioned>::decode_all_versioned(
                input,
            )?;
        let prepared =
            AcceptedTransaction::prepare_signed_metadata_from_versioned_payload(&tx, input);
        Ok(Self { tx, prepared })
    }

    /// Decode an owned versioned signed transaction and prepare admission metadata once.
    ///
    /// This normalizes adaptive versioned transport payloads to canonical signed
    /// transaction metadata before stateless validation.
    ///
    /// # Errors
    ///
    /// Returns the same versioning and Norito decode errors as the signed transaction
    /// versioned decoder.
    pub fn decode_versioned_owned(input: Vec<u8>) -> iroha_version::error::Result<Self> {
        Self::decode_versioned(input.as_slice())
    }

    /// Borrow the decoded signed transaction.
    #[must_use]
    pub fn signed(&self) -> &SignedTransaction {
        &self.tx
    }

    /// Borrow the decoded authority.
    #[must_use]
    pub fn authority(&self) -> &AccountId {
        self.tx.authority()
    }

    /// Return the prepared canonical transaction hash.
    #[must_use]
    pub fn hash(&self) -> HashOf<SignedTransaction> {
        self.prepared.signed_hash
    }

    /// Return the prepared canonical entrypoint hash.
    #[must_use]
    pub fn hash_as_entrypoint(&self) -> HashOf<TransactionEntrypoint> {
        self.prepared.entrypoint_hash
    }

    /// Return the prepared exact framed signed-transaction length.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        self.prepared.encoded_len
    }

    /// Return the message/signature/public-key tuple used for deterministic Ed25519 batch precheck.
    #[must_use]
    pub fn single_ed25519_precheck_parts(
        &self,
    ) -> Option<(&[u8], &[u8], iroha_crypto::Ed25519ParsedPublicKey)> {
        let public_key = self.prepared.single_ed25519_key?;
        Some((
            self.prepared.payload_hash.as_ref().as_slice(),
            self.tx.signature().payload().payload(),
            public_key,
        ))
    }

    /// Validate and accept the decoded signed transaction using prepared metadata.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`].
    pub fn into_accepted(
        self,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
    ) -> Result<AcceptedTransaction<'static>, AcceptTransactionFail> {
        let now = current_unix_time();
        AcceptedTransaction::validate_with_now_and_prepared_metadata(
            &self.tx,
            expected_chain_id,
            max_clock_drift,
            limits,
            crypto,
            now,
            &self.prepared,
        )?;
        enforce_nts_health_for_time_sensitive(&self.tx)?;
        Ok(AcceptedTransaction::from_external_with_prepared_metadata(
            self.tx,
            self.prepared,
        ))
    }

    /// Validate and accept the decoded signed transaction after deterministic Ed25519 precheck.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`].
    pub fn into_accepted_after_single_ed25519_precheck(
        self,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
    ) -> Result<AcceptedTransaction<'static>, AcceptTransactionFail> {
        let now = current_unix_time();
        AcceptedTransaction::validate_with_now_after_single_ed25519_precheck_and_prepared_metadata(
            &self.tx,
            expected_chain_id,
            max_clock_drift,
            limits,
            crypto,
            now,
            &self.prepared,
        )?;
        enforce_nts_health_for_time_sensitive(&self.tx)?;
        Ok(AcceptedTransaction::from_external_with_prepared_metadata(
            self.tx,
            self.prepared,
        ))
    }
}

fn reject_retired_heartbeat_metadata(tx: &SignedTransaction) -> Result<(), TransactionLimitError> {
    if tx.metadata().get(&*HEARTBEAT_METADATA_NAME).is_some() {
        return Err(TransactionLimitError {
            reason:
                "Transaction metadata `sumeragi_heartbeat` is retired in the first-release protocol"
                    .into(),
        });
    }
    Ok(())
}

fn is_time_sensitive_instruction(instruction: &InstructionBox) -> bool {
    let any = instruction.as_any();
    if let Some(iroha_data_model::isi::register::RegisterBox::Trigger(register)) =
        any.downcast_ref::<iroha_data_model::isi::register::RegisterBox>()
    {
        let trigger = &register.object;
        return is_time_sensitive_executable(trigger.action().executable());
    }
    if let Some(register) = any.downcast_ref::<
        iroha_data_model::isi::register::Register<iroha_data_model::trigger::Trigger>,
    >() {
        let trigger = &register.object;
        return is_time_sensitive_executable(trigger.action().executable());
    }
    any.is::<iroha_data_model::isi::offline::TopUpKagemushaRecursiveV4>()
        || any.is::<iroha_data_model::isi::offline::RedeemKagemushaRecursiveV4>()
        || any.is::<iroha_data_model::isi::oracle::RecordTwitterBinding>()
        || any.is::<iroha_data_model::isi::social::ClaimTwitterFollowReward>()
        || any.is::<iroha_data_model::isi::social::SendToTwitter>()
        || any.is::<iroha_data_model::isi::repo::RepoInstructionBox>()
        || any.is::<iroha_data_model::isi::settlement::SettlementInstructionBox>()
        || any.is::<iroha_data_model::isi::staking::ExitPublicLaneValidator>()
        || any.is::<iroha_data_model::isi::staking::SchedulePublicLaneUnbond>()
        || any.is::<iroha_data_model::isi::staking::FinalizePublicLaneUnbond>()
        || any.is::<iroha_data_model::isi::ExecuteTrigger>()
        || any.is::<iroha_data_model::isi::CustomInstruction>()
        || any.is::<iroha_data_model::isi::governance::ProposeDeployContract>()
        || any.is::<iroha_data_model::isi::governance::ProposeSccpRouteGovernance>()
        || any.is::<iroha_data_model::isi::governance::CastZkBallot>()
        || any.is::<iroha_data_model::isi::governance::CastPlainBallot>()
        || any.is::<iroha_data_model::isi::governance::ApproveGovernanceProposal>()
        || any.is::<iroha_data_model::isi::governance::EnactReferendum>()
        || any.is::<iroha_data_model::isi::governance::FinalizeReferendum>()
        || any.is::<iroha_data_model::isi::ministry::SubmitAgendaProposal>()
}

fn is_time_sensitive_executable(executable: &Executable) -> bool {
    match executable {
        Executable::Instructions(instructions) => {
            instructions.iter().any(is_time_sensitive_instruction)
        }
        Executable::ContractCall(_) => true,
        Executable::Batch(items) => items.iter().any(|item| match item {
            ExecutableBatchItem::Instruction(instruction) => {
                is_time_sensitive_instruction(instruction)
            }
            ExecutableBatchItem::ContractCall(_) => true,
        }),
        Executable::IvmProved(proved) => proved.overlay.iter().any(is_time_sensitive_instruction),
        Executable::Ivm(_) => true,
    }
}

fn instruction_self_registers_authority(
    instruction: &InstructionBox,
    authority: &AccountId,
) -> bool {
    let maybe_registration = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::Register<Account>>()
        .map(|register| register.object())
        .or_else(|| {
            instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::RegisterBox>()
                .and_then(|register| match register {
                    iroha_data_model::isi::RegisterBox::Account(register) => {
                        Some(register.object())
                    }
                    _ => None,
                })
        });

    let Some(registration) = maybe_registration else {
        return false;
    };

    registration.clone().build(authority).id == *authority
}

/// Return whether the executable's first instruction registers its exact authority.
///
/// Self-registering transactions are the only single-signature transactions that may enter
/// admission before their authority exists in world state. Keeping this recognition in Core lets
/// pre-admission services, such as fee quoting, apply the same instruction-shape rule.
#[must_use]
pub fn executable_self_registers_authority(executable: &Executable, authority: &AccountId) -> bool {
    match executable {
        Executable::Instructions(instructions) => {
            let Some((first, _rest)) = instructions.split_first() else {
                return false;
            };

            instruction_self_registers_authority(first, authority)
        }
        Executable::ContractCall(_)
        | Executable::Batch(_)
        | Executable::IvmProved(_)
        | Executable::Ivm(_) => false,
    }
}

/// Return whether admission may accept an authority that is absent from world state.
///
/// This includes exact first-instruction account self-registration and the existing multisig
/// proposal envelope path, whose authorisation is established from multisig membership rather
/// than a materialised authority account.
#[must_use]
pub fn allows_unregistered_authority(executable: &Executable, authority: &AccountId) -> bool {
    executable_self_registers_authority(executable, authority)
        || matches!(
            executable,
            Executable::Instructions(instructions)
                if instructions_allow_multisig_envelope_authority(instructions)
        )
}

pub(crate) fn instructions_allow_multisig_envelope_authority(
    instructions: &[InstructionBox],
) -> bool {
    instructions.iter().all(|instruction| {
        matches!(
            MultisigInstructionBox::try_from(instruction),
            Ok(MultisigInstructionBox::Propose(_))
                | Ok(MultisigInstructionBox::Approve(_))
                | Ok(MultisigInstructionBox::Cancel(_))
        )
    })
}

#[derive(Clone, Copy)]
enum ConfidentialPolicyAdmissionAction {
    Shield,
    Transfer,
    Unshield,
}

impl ConfidentialPolicyAdmissionAction {
    const fn label(self) -> &'static str {
        match self {
            Self::Shield => "shield",
            Self::Transfer => "transfer",
            Self::Unshield => "unshield",
        }
    }
}

fn confidential_policy_admission_rejection(
    action: ConfidentialPolicyAdmissionAction,
) -> TransactionRejectionReason {
    TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
        "{} not permitted by policy",
        action.label()
    )))
}

fn effective_confidential_policy_mode_for_admission(
    world: &impl WorldReadOnly,
    asset_def_id: &AssetDefinitionId,
    block_height: u64,
) -> Result<ConfidentialPolicyMode, TransactionRejectionReason> {
    let asset_definition = world
        .asset_definition(asset_def_id)
        .map_err(|err| TransactionRejectionReason::Validation(ValidationFail::from(err)))?;
    let policy = *asset_definition.confidential_policy();
    let Some(transition) = policy.pending_transition() else {
        return Ok(policy.mode());
    };

    if transition.new_mode() == ConfidentialPolicyMode::ShieldedOnly
        && block_height >= transition.effective_height()
    {
        let transparent_total = world
            .asset_total_amount(asset_def_id)
            .map_err(|err| TransactionRejectionReason::Validation(ValidationFail::from(err)))?;
        if transparent_total > Quantity::zero() {
            return Ok(transition.previous_mode());
        }
    }

    Ok(policy.effective_mode(block_height))
}

fn validate_confidential_policy_for_action(
    world: &impl WorldReadOnly,
    asset_def_id: &AssetDefinitionId,
    block_height: u64,
    action: ConfidentialPolicyAdmissionAction,
) -> Result<(), TransactionRejectionReason> {
    let policy_mode =
        effective_confidential_policy_mode_for_admission(world, asset_def_id, block_height)?;
    match action {
        ConfidentialPolicyAdmissionAction::Shield => match policy_mode {
            ConfidentialPolicyMode::TransparentOnly => {
                Err(confidential_policy_admission_rejection(action))
            }
            ConfidentialPolicyMode::Convertible => {
                let allowed = world
                    .zk_assets()
                    .get(asset_def_id)
                    .is_some_and(|st| st.allow_shield);
                if allowed {
                    Ok(())
                } else {
                    Err(confidential_policy_admission_rejection(action))
                }
            }
            ConfidentialPolicyMode::ShieldedOnly => Ok(()),
        },
        ConfidentialPolicyAdmissionAction::Transfer => {
            if matches!(policy_mode, ConfidentialPolicyMode::TransparentOnly) {
                Err(confidential_policy_admission_rejection(action))
            } else {
                Ok(())
            }
        }
        ConfidentialPolicyAdmissionAction::Unshield => match policy_mode {
            ConfidentialPolicyMode::TransparentOnly | ConfidentialPolicyMode::ShieldedOnly => {
                Err(confidential_policy_admission_rejection(action))
            }
            ConfidentialPolicyMode::Convertible => {
                let allowed = world
                    .zk_assets()
                    .get(asset_def_id)
                    .is_some_and(|st| st.allow_unshield);
                if allowed {
                    Ok(())
                } else {
                    Err(confidential_policy_admission_rejection(action))
                }
            }
        },
    }
}

pub(crate) fn validate_confidential_policy_admission_for_world(
    executable: &Executable,
    world: &impl WorldReadOnly,
    block_height: u64,
) -> Result<(), TransactionRejectionReason> {
    for instruction in executable.explicit_instructions() {
        let any = instruction.as_any();
        if let Some(shield) = any.downcast_ref::<zk::Shield>() {
            validate_confidential_policy_for_action(
                world,
                shield.asset(),
                block_height,
                ConfidentialPolicyAdmissionAction::Shield,
            )?;
        } else if let Some(transfer) = any.downcast_ref::<zk::ZkTransfer>() {
            validate_confidential_policy_for_action(
                world,
                transfer.asset(),
                block_height,
                ConfidentialPolicyAdmissionAction::Transfer,
            )?;
        } else if let Some(topup) =
            any.downcast_ref::<iroha_data_model::isi::offline::TopUpKagemushaRecursiveV4>()
        {
            validate_confidential_policy_for_action(
                world,
                topup.request.asset.definition(),
                block_height,
                ConfidentialPolicyAdmissionAction::Transfer,
            )?;
        } else if let Some(redeem) =
            any.downcast_ref::<iroha_data_model::isi::offline::RedeemKagemushaRecursiveV4>()
        {
            validate_confidential_policy_for_action(
                world,
                &redeem.request.bundle.statement.current_note.asset,
                block_height,
                ConfidentialPolicyAdmissionAction::Unshield,
            )?;
        } else if let Some(unshield) = any.downcast_ref::<zk::Unshield>() {
            validate_confidential_policy_for_action(
                world,
                unshield.asset(),
                block_height,
                ConfidentialPolicyAdmissionAction::Unshield,
            )?;
        }
    }

    Ok(())
}

fn format_nts_health_reason(status: &crate::time::NetworkTimeStatus) -> String {
    format!(
        "fallback={} samples_used={} peers_seen={} offset_ms={} confidence_ms={} min_samples_ok={} offset_ok={} confidence_ok={}",
        status.fallback,
        status.sample_count,
        status.peer_count,
        status.offset_ms,
        status.confidence_ms,
        status.health.min_samples_ok,
        status.health.offset_ok,
        status.health.confidence_ok
    )
}

fn enforce_time_sensitive_with_nts(
    tx: &SignedTransaction,
    status: crate::time::NetworkTimeStatus,
    mode: iroha_config::parameters::actual::NtsEnforcementMode,
) -> Result<(), AcceptTransactionFail> {
    if status.health.healthy {
        return Ok(());
    }
    match mode {
        iroha_config::parameters::actual::NtsEnforcementMode::Warn => {
            warn!(
                tx_hash = %tx.hash(),
                fallback = status.fallback,
                sample_count = status.sample_count,
                peer_count = status.peer_count,
                offset_ms = status.offset_ms,
                confidence_ms = status.confidence_ms,
                min_samples_ok = status.health.min_samples_ok,
                offset_ok = status.health.offset_ok,
                confidence_ok = status.health.confidence_ok,
                "NTS unhealthy during time-sensitive admission; allowing transaction"
            );
            Ok(())
        }
        iroha_config::parameters::actual::NtsEnforcementMode::Reject => {
            Err(AcceptTransactionFail::NetworkTimeUnhealthy {
                reason: format_nts_health_reason(&status),
            })
        }
    }
}

fn enforce_nts_health_for_time_sensitive(
    tx: &SignedTransaction,
) -> Result<(), AcceptTransactionFail> {
    if !is_time_sensitive_executable(tx.instructions()) {
        return Ok(());
    }
    let status = crate::time::now();
    let mode = crate::time::enforcement_mode();
    enforce_time_sensitive_with_nts(tx, status, mode)
}

fn validate_proof_attachment_shapes(tx: &SignedTransaction) -> Result<(), AcceptTransactionFail> {
    let Some(attachments) = tx.attachments() else {
        return Ok(());
    };
    if attachments.is_empty() {
        return Err(AcceptTransactionFail::TransactionLimit(
            TransactionLimitError {
                reason: "Proof attachment list must not be empty".into(),
            },
        ));
    }
    for (index, attachment) in attachments.as_slice().iter().enumerate() {
        if let Some((field, message)) = attachment.structural_error() {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: format!("Proof attachment {index} `{field}` {message}"),
                },
            ));
        }
    }
    Ok(())
}

impl<'tx> AcceptedTransaction<'tx> {
    fn from_entrypoint(entrypoint: Cow<'tx, TransactionEntrypoint>) -> Self {
        Self {
            entrypoint,
            entrypoint_hash: OnceLock::new(),
            signed_hash: OnceLock::new(),
            encoded_len: OnceLock::new(),
            entrypoint_bytes: OnceLock::new(),
            signed_bytes: OnceLock::new(),
            payload_hash: OnceLock::new(),
            single_ed25519_key: OnceLock::new(),
        }
    }

    fn from_external_with_cached_bytes(
        tx: SignedTransaction,
        signed_bytes: Option<Arc<Vec<u8>>>,
    ) -> Self {
        let canonical_signed_bytes =
            Arc::new(norito::encode_canonical(&tx).expect("encode accepted signed transaction"));
        let signed_bytes = signed_bytes
            .filter(|bytes| bytes.as_slice() == canonical_signed_bytes.as_slice())
            .unwrap_or(canonical_signed_bytes);
        let entrypoint_hash = tx.hash_as_entrypoint();
        let encoded_len = signed_bytes.len();
        let payload_hash = HashOf::new(tx.payload());
        let single_ed25519_key = Self::parsed_single_ed25519_key(&tx);
        let entrypoint = TransactionEntrypoint::External(tx);
        let signed_hash = Self::compat_signed_hash(entrypoint_hash);
        let accepted = Self::from_entrypoint(Cow::Owned(entrypoint));
        let _ = accepted.signed_bytes.set(Some(signed_bytes));
        let _ = accepted.encoded_len.set(encoded_len);
        let _ = accepted.payload_hash.set(Some(payload_hash));
        let _ = accepted.single_ed25519_key.set(single_ed25519_key);
        let _ = accepted.entrypoint_hash.set(entrypoint_hash);
        let _ = accepted.signed_hash.set(signed_hash);
        accepted
    }

    fn from_entrypoint_with_cached_entrypoint_bytes(
        tx: TransactionEntrypoint,
        entrypoint_bytes: Arc<Vec<u8>>,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
    ) -> Self {
        let accepted = Self::from_entrypoint(Cow::Owned(tx));
        let _ = accepted.entrypoint_bytes.set(entrypoint_bytes);
        let _ = accepted.entrypoint_hash.set(entrypoint_hash);
        let _ = accepted
            .signed_hash
            .set(Self::compat_signed_hash(entrypoint_hash));
        accepted
    }

    fn from_external_with_hot_cache(tx: SignedTransaction) -> Self {
        Self::from_external_with_cached_bytes(tx, None)
    }

    fn compat_signed_hash(
        entrypoint_hash: HashOf<TransactionEntrypoint>,
    ) -> HashOf<SignedTransaction> {
        HashOf::from_untyped_unchecked(iroha_crypto::Hash::from(entrypoint_hash))
    }

    fn canonical_signed_payload_with_flags(tx: &SignedTransaction) -> (Vec<u8>, u8) {
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        norito::codec::encode_with_header_flags(tx)
    }

    #[cfg(test)]
    fn external_entrypoint_hash_from_signed(
        tx: &SignedTransaction,
    ) -> HashOf<TransactionEntrypoint> {
        tx.hash_as_entrypoint()
    }

    #[cfg(test)]
    fn external_entrypoint_hash_from_signed_frame(
        signed_frame: &[u8],
    ) -> Result<HashOf<TransactionEntrypoint>, norito::core::Error> {
        let transaction: SignedTransaction = norito::decode_canonical(signed_frame)?;
        Ok(transaction.hash_as_entrypoint())
    }

    fn framed_padding_for<T>() -> usize {
        let align = norito::core::archived_payload_align::<T>();
        if align <= 1 {
            return 0;
        }
        let remainder = norito::core::Header::SIZE % align;
        if remainder == 0 { 0 } else { align - remainder }
    }

    fn bare_encoded_len<T: norito::NoritoSerialize>(value: &T) -> usize {
        norito::codec::Encode::encode(value).len()
    }

    fn framed_encoded_len<T: norito::NoritoSerialize>(value: &T) -> usize {
        norito::core::Header::SIZE
            .saturating_add(Self::framed_padding_for::<T>())
            .saturating_add(Self::bare_encoded_len(value))
    }

    fn framed_encoded_payload_len<T>(payload_len: usize) -> usize {
        norito::core::Header::SIZE
            .saturating_add(Self::framed_padding_for::<T>())
            .saturating_add(payload_len)
    }

    fn signed_encoded_len(tx: &SignedTransaction) -> usize {
        Self::framed_encoded_len(tx)
    }

    fn entrypoint_encoded_len(tx: &TransactionEntrypoint) -> usize {
        match tx {
            TransactionEntrypoint::External(signed) => Self::signed_encoded_len(signed),
            TransactionEntrypoint::SealedCommitment(entrypoint) => {
                Self::framed_encoded_len(entrypoint)
            }
            TransactionEntrypoint::SealedReveal(entrypoint) => Self::framed_encoded_len(entrypoint),
            TransactionEntrypoint::PrivateKaigi(entrypoint) => Self::framed_encoded_len(entrypoint),
            TransactionEntrypoint::Time(entrypoint) => Self::framed_encoded_len(entrypoint),
        }
    }

    fn signed_encoded_len_for_limit(tx: &SignedTransaction) -> u64 {
        u64::try_from(Self::signed_encoded_len(tx)).unwrap_or(u64::MAX)
    }

    fn signed_encoded_len_for_limit_with_prepared(
        tx: &SignedTransaction,
        prepared: Option<&PreparedTransactionMetadata>,
    ) -> u64 {
        prepared.map_or_else(
            || Self::signed_encoded_len_for_limit(tx),
            |metadata| {
                let encoded_len = metadata
                    .signed_bytes
                    .as_ref()
                    .map_or(metadata.encoded_len, |bytes| bytes.len());
                u64::try_from(encoded_len).unwrap_or(u64::MAX)
            },
        )
    }

    fn parsed_single_ed25519_key(
        tx: &SignedTransaction,
    ) -> Option<iroha_crypto::Ed25519ParsedPublicKey> {
        let iroha_data_model::account::AccountController::Single(signatory) =
            tx.authority().controller()
        else {
            return None;
        };
        let Ok((iroha_crypto::Algorithm::Ed25519, public_key)) = signatory.try_to_bytes() else {
            return None;
        };
        if tx.signature().payload().payload().len() != ED25519_SIGNATURE_LENGTH {
            return None;
        }
        iroha_crypto::ed25519_parse_public_key(public_key).ok()
    }

    /// Build reusable stateless metadata for a signed transaction.
    #[must_use]
    pub(crate) fn prepare_signed_metadata(tx: &SignedTransaction) -> PreparedTransactionMetadata {
        let (signed_payload, _signed_payload_flags) = Self::canonical_signed_payload_with_flags(tx);
        let signed_payload_len = signed_payload.len();
        let encoded_len = Self::framed_encoded_payload_len::<SignedTransaction>(signed_payload_len);
        let entrypoint_hash = tx.hash_as_entrypoint();
        Self::prepare_signed_metadata_with_entrypoint_hash_encoded_len_and_caches(
            tx,
            entrypoint_hash,
            encoded_len,
            None,
            None,
        )
    }

    fn prepare_signed_metadata_from_versioned_payload(
        tx: &SignedTransaction,
        versioned_payload: &[u8],
    ) -> PreparedTransactionMetadata {
        let (signed_payload, signed_payload_flags) = Self::canonical_signed_payload_with_flags(tx);
        let signed_payload_len = signed_payload.len();
        let encoded_len = Self::framed_encoded_payload_len::<SignedTransaction>(signed_payload_len);
        let entrypoint_hash = tx.hash_as_entrypoint();
        let signed_bytes = versioned_payload
            .get(1..)
            .filter(|payload| *payload == signed_payload.as_slice())
            .and_then(|payload| {
                norito::core::frame_bare_with_header_flags::<SignedTransaction>(
                    payload,
                    signed_payload_flags,
                )
                .ok()
            })
            .map(Arc::new);
        let entrypoint_bytes = Some(Arc::new(
            norito::encode_canonical(&TransactionEntrypoint::External(tx.clone()))
                .expect("encode canonical external transaction entrypoint"),
        ));
        let encoded_len = signed_bytes
            .as_ref()
            .map_or(encoded_len, |bytes| bytes.len());
        Self::prepare_signed_metadata_with_entrypoint_hash_encoded_len_and_caches(
            tx,
            entrypoint_hash,
            encoded_len,
            signed_bytes,
            entrypoint_bytes,
        )
    }

    fn prepare_signed_metadata_with_entrypoint_hash_encoded_len_and_caches(
        tx: &SignedTransaction,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        encoded_len: usize,
        signed_bytes: Option<Arc<Vec<u8>>>,
        entrypoint_bytes: Option<Arc<Vec<u8>>>,
    ) -> PreparedTransactionMetadata {
        let signed_hash = HashOf::from_untyped_unchecked(iroha_crypto::Hash::from(entrypoint_hash));
        PreparedTransactionMetadata {
            signed_hash,
            entrypoint_hash,
            payload_hash: HashOf::new(tx.payload()),
            encoded_len,
            signed_bytes,
            entrypoint_bytes,
            single_ed25519_key: Self::parsed_single_ed25519_key(tx),
            metadata_depths: prepare_metadata_depths(tx.metadata()),
        }
    }

    fn signed_encoded_len_from_external_entrypoint_frame(
        framed: &[u8],
    ) -> Result<usize, norito::core::Error> {
        const EXTERNAL_ENTRYPOINT_TAG: u32 = 0;

        let view = norito::core::from_bytes_view(framed)?;
        if view.schema() != <TransactionEntrypoint as norito::core::NoritoSerialize>::schema_hash()
        {
            return Err(norito::core::Error::SchemaMismatch);
        }
        let payload = view.as_bytes();
        let tag_bytes = payload
            .get(..4)
            .ok_or(norito::core::Error::LengthMismatch)?;
        let tag = u32::from_le_bytes(
            tag_bytes
                .try_into()
                .expect("slice length checked for u32 tag"),
        );
        if tag != EXTERNAL_ENTRYPOINT_TAG {
            return Err(norito::core::Error::Message(
                "gossip entrypoint frame does not contain an external signed transaction".into(),
            ));
        }
        let (signed_payload_len, len_prefix_len) =
            norito::core::read_len_from_slice_with_flags(&payload[4..], view.flags())?;
        let signed_payload_start = 4usize
            .checked_add(len_prefix_len)
            .ok_or(norito::core::Error::LengthMismatch)?;
        let signed_payload_end = signed_payload_start
            .checked_add(signed_payload_len)
            .ok_or(norito::core::Error::LengthMismatch)?;
        if signed_payload_end > payload.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        Ok(Self::framed_encoded_payload_len::<SignedTransaction>(
            signed_payload_len,
        ))
    }

    /// Build stateless metadata for a signed transaction decoded from a canonical gossip frame.
    #[must_use]
    pub(crate) fn prepare_gossip_signed_metadata(
        tx: &SignedTransaction,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        entrypoint_bytes: Arc<Vec<u8>>,
    ) -> PreparedTransactionMetadata {
        let encoded_len =
            Self::signed_encoded_len_from_external_entrypoint_frame(entrypoint_bytes.as_slice())
                .unwrap_or_else(|_| Self::signed_encoded_len(tx));
        Self::prepare_signed_metadata_with_entrypoint_hash_encoded_len_and_caches(
            tx,
            entrypoint_hash,
            encoded_len,
            None,
            Some(entrypoint_bytes),
        )
    }

    fn from_external_with_prepared_metadata(
        tx: SignedTransaction,
        prepared: PreparedTransactionMetadata,
    ) -> Self {
        let accepted = Self::from_entrypoint(Cow::Owned(TransactionEntrypoint::External(tx)));
        let _ = accepted.encoded_len.set(prepared.encoded_len);
        let _ = accepted.payload_hash.set(Some(prepared.payload_hash));
        let _ = accepted.single_ed25519_key.set(prepared.single_ed25519_key);
        if let Some(signed_bytes) = prepared.signed_bytes {
            let _ = accepted.signed_bytes.set(Some(signed_bytes));
        }
        if let Some(entrypoint_bytes) = prepared.entrypoint_bytes {
            let _ = accepted.entrypoint_bytes.set(entrypoint_bytes);
        }
        let _ = accepted.signed_hash.set(prepared.signed_hash);
        let _ = accepted.entrypoint_hash.set(prepared.entrypoint_hash);
        accepted
    }

    fn validate_common(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        now: Duration,
    ) -> Result<(), AcceptTransactionFail> {
        let actual_chain_id = tx.chain();

        if expected_chain_id != actual_chain_id {
            return Err(AcceptTransactionFail::ChainIdMismatch(Mismatch {
                expected: expected_chain_id.clone(),
                actual: actual_chain_id.clone(),
            }));
        }

        if tx.creation_time().saturating_sub(now) > max_clock_drift {
            return Err(AcceptTransactionFail::TransactionInTheFuture);
        }

        tx.payload().validate_fee_payment_intent().map_err(|err| {
            AcceptTransactionFail::SignatureVerification(Self::signature_fail_from_error(
                tx,
                TransactionSignatureError::InvalidFeePaymentIntent(err.to_string()),
            ))
        })?;

        Ok(())
    }

    fn has_single_ed25519_signature(tx: &SignedTransaction) -> bool {
        matches!(
            tx.authority().controller(),
            iroha_data_model::account::AccountController::Single(signatory)
                if signatory
                    .try_to_bytes()
                    .is_ok_and(|(algorithm, _)| algorithm == iroha_crypto::Algorithm::Ed25519)
                    && tx.signature().payload().payload().len() == ED25519_SIGNATURE_LENGTH
        )
    }

    fn verify_signature_for_check(
        tx: &SignedTransaction,
        signature_check: SignatureCheck,
        prepared: Option<&PreparedTransactionMetadata>,
    ) -> Result<(), AcceptTransactionFail> {
        match signature_check {
            SignatureCheck::Override(result) => {
                return result.map_err(AcceptTransactionFail::SignatureVerification);
            }
            SignatureCheck::PrecheckedSingleEd25519 if Self::has_single_ed25519_signature(tx) => {
                return Ok(());
            }
            SignatureCheck::Verify | SignatureCheck::PrecheckedSingleEd25519 => {}
        }

        if let Some(prepared) = prepared
            && let Some(public_key) = prepared.single_ed25519_key
            && Self::has_single_ed25519_signature(tx)
        {
            let message = prepared.payload_hash.as_ref().as_slice();
            let signature = tx.signature().payload().payload();
            let messages = [message];
            let signatures = [signature];
            let public_keys = [public_key];

            return iroha_crypto::ed25519_verify_batch_preparsed_deterministic(
                &messages,
                &signatures,
                &public_keys,
                [0; 32],
            )
            .map_err(|err| {
                AcceptTransactionFail::SignatureVerification(Self::signature_fail_from_error(
                    tx,
                    TransactionSignatureError::CryptoError(err.to_string()),
                ))
            });
        }

        tx.verify_signature().map_err(|err| {
            AcceptTransactionFail::SignatureVerification(Self::signature_fail_from_error(tx, err))
        })
    }

    fn ensure_signing_allowed(
        tx: &SignedTransaction,
        crypto: &iroha_config::parameters::actual::Crypto,
    ) -> Result<(), AcceptTransactionFail> {
        let signature = tx.signature().clone();
        match tx.authority().controller() {
            iroha_data_model::account::AccountController::Single(signatory) => {
                let algo =
                    Self::signature_public_key_algorithm(&signature, signatory, "signatory")?;
                if !crypto.allowed_signing.contains(&algo) {
                    return Err(AcceptTransactionFail::SignatureVerification(
                        SignatureVerificationFail::new(
                            signature,
                            SignatureRejectionCode::AlgorithmNotPermitted,
                            format!("signing algorithm {algo} is not permitted by configuration"),
                        ),
                    ));
                }
                Ok(())
            }
            iroha_data_model::account::AccountController::Multisig(policy) => {
                for member in policy.members() {
                    let algo = Self::signature_public_key_algorithm(
                        &signature,
                        member.public_key(),
                        "multisig member",
                    )?;
                    if !crypto.allowed_signing.contains(&algo) {
                        return Err(AcceptTransactionFail::SignatureVerification(
                            SignatureVerificationFail::new(
                                signature.clone(),
                                SignatureRejectionCode::AlgorithmNotPermitted,
                                format!(
                                    "multisig member algorithm {algo} is not permitted by configuration"
                                ),
                            ),
                        ));
                    }
                }
                if let Some(bundle) = tx.multisig_signatures() {
                    for entry in &bundle.signatures {
                        let algo = Self::signature_public_key_algorithm(
                            &signature,
                            &entry.signer,
                            "multisig signer",
                        )?;
                        if !crypto.allowed_signing.contains(&algo) {
                            return Err(AcceptTransactionFail::SignatureVerification(
                                SignatureVerificationFail::new(
                                    signature.clone(),
                                    SignatureRejectionCode::AlgorithmNotPermitted,
                                    format!(
                                        "multisig signer {} uses disallowed algorithm {algo}",
                                        entry.signer
                                    ),
                                ),
                            ));
                        }
                    }
                }
                Ok(())
            }
        }
    }

    fn signature_public_key_algorithm(
        signature: &TransactionSignature,
        public_key: &iroha_crypto::PublicKey,
        context: &str,
    ) -> Result<iroha_crypto::Algorithm, AcceptTransactionFail> {
        public_key.try_algorithm().map_err(|err| {
            AcceptTransactionFail::SignatureVerification(SignatureVerificationFail::new(
                signature.clone(),
                SignatureRejectionCode::MalformedSignature,
                format!("{context} public key is malformed: {err}"),
            ))
        })
    }

    fn signature_rejection_code(err: &TransactionSignatureError) -> SignatureRejectionCode {
        match err {
            TransactionSignatureError::UnsupportedMultisigAuthority => {
                SignatureRejectionCode::UnsupportedAuthority
            }
            TransactionSignatureError::AlgorithmNotPermitted(_) => {
                SignatureRejectionCode::AlgorithmNotPermitted
            }
            TransactionSignatureError::AuthorityKeyMismatch
            | TransactionSignatureError::CryptoError(_) => SignatureRejectionCode::InvalidSignature,
            TransactionSignatureError::InvalidFeePaymentIntent(_)
            | TransactionSignatureError::MissingTimeToLive => {
                SignatureRejectionCode::MalformedSignature
            }
            TransactionSignatureError::UnexpectedMultisigSignatures
            | TransactionSignatureError::NonCanonicalMultisigSignatures => {
                SignatureRejectionCode::MalformedSignature
            }
            TransactionSignatureError::NoSignatures
            | TransactionSignatureError::MissingMultisigSignatures => {
                SignatureRejectionCode::MissingSignatures
            }
            TransactionSignatureError::UnknownMultisigSigner => {
                SignatureRejectionCode::UnknownSigner
            }
            TransactionSignatureError::InsufficientMultisigWeight { .. } => {
                SignatureRejectionCode::InsufficientWeight
            }
        }
    }

    fn signature_fail_from_error(
        tx: &SignedTransaction,
        err: TransactionSignatureError,
    ) -> SignatureVerificationFail {
        SignatureVerificationFail::new(
            tx.signature().clone(),
            Self::signature_rejection_code(&err),
            err.to_string(),
        )
    }

    pub(crate) fn signature_verification_result(
        tx: &SignedTransaction,
    ) -> Result<(), SignatureVerificationFail> {
        tx.verify_signature()
            .map_err(|err| Self::signature_fail_from_error(tx, err))
    }

    fn ensure_signature_limit(
        signature_count: usize,
        limits: &TransactionParameters,
    ) -> Result<(), AcceptTransactionFail> {
        let signature_limit = limits.max_signatures().get();
        let signature_count_u64 = u64::try_from(signature_count).unwrap_or(u64::MAX);
        if signature_count_u64 > signature_limit {
            warn!(
                signature_count,
                signature_limit, "rejecting transaction: signature count exceeds configured limit"
            );
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: format!(
                        "{SIGNATURE_LIMIT_REASON_PREFIX}, max number is {}, but got {}",
                        limits.max_signatures(),
                        signature_count
                    ),
                },
            ));
        }

        Ok(())
    }

    /// Verify that the transaction is not yet committed and wrap it in a [`CheckedTransaction`].
    ///
    /// # Errors
    ///
    /// Returns the original transaction and [`TransactionAlreadyCommitted`] when the hash is already present in the ledger.
    #[allow(clippy::result_large_err)]
    pub fn into_checked(
        self,
        state: &impl StateReadOnlyWithTransactions,
    ) -> Result<CheckedTransaction<'tx>, (AcceptedTransaction<'tx>, TransactionAlreadyCommitted)>
    {
        CheckedTransaction::new(self, state)
    }

    fn validate_private_kaigi_with_now(
        tx: &PrivateKaigiTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        now: Duration,
    ) -> Result<(), AcceptTransactionFail> {
        if tx.chain != *expected_chain_id {
            return Err(AcceptTransactionFail::ChainIdMismatch(Mismatch {
                expected: expected_chain_id.clone(),
                actual: tx.chain.clone(),
            }));
        }

        let creation_time = tx.creation_time();
        if creation_time.saturating_sub(now) > max_clock_drift {
            return Err(AcceptTransactionFail::TransactionInTheFuture);
        }

        let entrypoint = TransactionEntrypoint::PrivateKaigi(tx.clone());
        let tx_encoded_len =
            u64::try_from(Self::entrypoint_encoded_len(&entrypoint)).unwrap_or(u64::MAX);
        let max_tx_bytes = limits.max_tx_bytes().get();
        if tx_encoded_len > max_tx_bytes {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: format!(
                        "Transaction size {tx_encoded_len} bytes exceeds limit {max_tx_bytes} bytes"
                    ),
                },
            ));
        }

        let decompressed_len = tx
            .artifacts
            .proof
            .len()
            .saturating_add(tx.fee_spend.proof.len())
            .saturating_add(
                tx.fee_spend
                    .encrypted_change_payloads
                    .iter()
                    .map(Vec::len)
                    .sum::<usize>(),
            );
        let decompressed_len = u64::try_from(decompressed_len).unwrap_or(u64::MAX);
        let max_decompressed_bytes = limits.max_decompressed_bytes().get();
        if decompressed_len > max_decompressed_bytes {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: format!(
                        "Private Kaigi artifacts expand to {decompressed_len} bytes which exceeds limit {max_decompressed_bytes} bytes"
                    ),
                },
            ));
        }

        let max_metadata_depth = usize::from(limits.max_metadata_depth().get());
        ensure_metadata_depth(&tx.metadata, max_metadata_depth)
            .map_err(AcceptTransactionFail::TransactionLimit)?;

        if tx.artifacts.proof.is_empty() {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: "private Kaigi proof payload must be non-empty".into(),
                },
            ));
        }
        if tx.fee_spend.proof.is_empty() {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: "private Kaigi fee spend proof must be non-empty".into(),
                },
            ));
        }
        if tx.fee_spend.nullifiers.is_empty() {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: "private Kaigi fee spend must consume at least one nullifier".into(),
                },
            ));
        }
        if tx.fee_spend.output_commitments.len() != tx.fee_spend.encrypted_change_payloads.len() {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason:
                        "private Kaigi fee spend outputs must match encrypted change payload count"
                            .into(),
                },
            ));
        }
        if tx.fee_spend.asset_definition_id.to_string()
            != iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
        {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason:
                        "private Kaigi fee spend asset must be the canonical xor#universal asset"
                            .into(),
                },
            ));
        }

        match &tx.action {
            PrivateKaigiAction::Create(create) => {
                if create.call.privacy_mode != iroha_data_model::kaigi::KaigiPrivacyMode::ZkRosterV1
                {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: "private Kaigi create must use ZkRosterV1 privacy mode".into(),
                        },
                    ));
                }
            }
            PrivateKaigiAction::Join(_) | PrivateKaigiAction::End(_) => {}
        }

        Ok(())
    }

    fn private_kaigi_instruction_gas(
        tx: &PrivateKaigiTransaction,
    ) -> Result<u64, TransactionRejectionReason> {
        let instruction =
            crate::smartcontracts::isi::kaigi::private_instruction_box(tx).map_err(|error| {
                TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(error))
            })?;
        Ok(isi_gas::meter_instruction(&instruction))
    }

    fn compute_private_kaigi_fee_amount(
        tx: &PrivateKaigiTransaction,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<Quantity, TransactionRejectionReason> {
        if !state_transaction.nexus.enabled {
            return Ok(Quantity::zero());
        }

        let cfg = state_transaction.nexus.fees.clone();
        let entrypoint = TransactionEntrypoint::PrivateKaigi(tx.clone());
        let tx_bytes_len = norito::to_bytes(&entrypoint)
            .map(|bytes| bytes.len())
            .map_err(|err| {
                TransactionRejectionReason::Validation(ValidationFail::InternalError(format!(
                    "failed to encode private Kaigi transaction for fee metering: {err}"
                )))
            })?;
        let gas_used = Self::private_kaigi_instruction_gas(tx)?;
        crate::executor::compute_nexus_fee_amount(&cfg, tx_bytes_len, 1, gas_used)
            .map_err(TransactionRejectionReason::Validation)
    }

    fn execute_private_kaigi_fee_spend(
        tx: &PrivateKaigiTransaction,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), TransactionRejectionReason> {
        let binding = decode_private_kaigi_fee_binding(&tx.fee_spend.proof)?;
        let expected_action_hash = hex::encode(tx.action_hash().as_ref());
        if binding.action_hash_hex != expected_action_hash {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "private Kaigi fee spend proof is not bound to this action hash".into(),
                ),
            ));
        }
        if binding.chain_id != tx.chain.to_string() {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "private Kaigi fee spend proof is not bound to this chain id".into(),
                ),
            ));
        }
        if binding.asset_definition_id != tx.fee_spend.asset_definition_id.to_string() {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "private Kaigi fee spend proof is not bound to the canonical xor#universal asset"
                        .into(),
                ),
            ));
        }

        let expected_fee = Self::compute_private_kaigi_fee_amount(tx, state_transaction)?;
        if binding.fee_amount != expected_fee {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(format!(
                    "private Kaigi fee spend amount mismatch: expected {expected_fee}, observed {}",
                    binding.fee_amount
                )),
            ));
        }
        let canonical_fee_proof = canonical_private_kaigi_fee_transfer_proof(&tx.fee_spend.proof)?;

        let zk_asset = state_transaction
            .world
            .zk_assets
            .get(&tx.fee_spend.asset_definition_id)
            .cloned()
            .ok_or_else(|| {
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                    "private Kaigi fee asset is not configured for confidential transfers".into(),
                ))
            })?;
        let Some(vk_binding) = zk_asset.vk_transfer.clone() else {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "private Kaigi fee asset is missing a confidential transfer verifier".into(),
                ),
            ));
        };
        let backend_ident = Ident::from_str(vk_binding.id.backend.as_str()).map_err(|_| {
            TransactionRejectionReason::Validation(ValidationFail::InternalError(
                "invalid transfer verifier backend identifier".into(),
            ))
        })?;
        let mut attachment = ProofAttachment::new_ref(
            backend_ident.clone(),
            ProofBox::new(backend_ident, canonical_fee_proof),
            vk_binding.id,
        );
        attachment.vk_commitment = Some(vk_binding.commitment);

        let transfer = zk::ZkTransfer::new(
            tx.fee_spend.asset_definition_id.clone(),
            tx.fee_spend.nullifiers.clone(),
            tx.fee_spend.output_commitments.clone(),
            attachment,
            Some(tx.fee_spend.anchor_root.into()),
        );

        let fee_payer = Self::private_kaigi_fee_payer_account(tx)?;

        transfer
            .execute(&fee_payer, state_transaction)
            .map_err(|error| {
                TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(error))
            })
    }

    fn private_kaigi_fee_payer_account(
        tx: &PrivateKaigiTransaction,
    ) -> Result<AccountId, TransactionRejectionReason> {
        let fee_payer_seed = iroha_crypto::Hash::new(tx.action_hash().as_ref());
        let fee_payer_keypair = iroha_crypto::KeyPair::try_from_seed(
            fee_payer_seed.as_ref().to_vec(),
            Algorithm::Ed25519,
        )
        .map_err(|err| {
            TransactionRejectionReason::Validation(ValidationFail::InternalError(format!(
                "failed to derive private Kaigi fee payer account: {err}"
            )))
        })?;
        Ok(AccountId::new(fee_payer_keypair.public_key().clone()))
    }

    /// Like [`Self::accept_genesis`], but without wrapping.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`]
    pub fn validate_genesis(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        genesis_account: &AccountId,
        crypto: &iroha_config::parameters::actual::Crypto,
    ) -> Result<(), AcceptTransactionFail> {
        let now = current_unix_time();
        Self::validate_genesis_with_now(
            tx,
            expected_chain_id,
            max_clock_drift,
            genesis_account,
            crypto,
            now,
        )
    }

    /// Like [`Self::validate_genesis`], but with a caller-provided "now" timestamp.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`]
    pub fn validate_genesis_with_now(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        genesis_account: &AccountId,
        crypto: &iroha_config::parameters::actual::Crypto,
        now: Duration,
    ) -> Result<(), AcceptTransactionFail> {
        Self::validate_common(tx, expected_chain_id, max_clock_drift, now)?;

        if genesis_account != tx.authority() {
            return Err(AcceptTransactionFail::UnexpectedGenesisAccountSignature);
        }

        Self::ensure_signing_allowed(tx, crypto)?;

        Ok(())
    }

    /// Like [`Self::accept`], but without wrapping.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`]
    #[allow(clippy::too_many_lines)]
    pub fn validate(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
    ) -> Result<(), AcceptTransactionFail> {
        let now = current_unix_time();
        Self::validate_with_now(tx, expected_chain_id, max_clock_drift, limits, crypto, now)?;
        enforce_nts_health_for_time_sensitive(tx)?;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate_with_now(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
        now: Duration,
    ) -> Result<(), AcceptTransactionFail> {
        Self::validate_with_now_and_signature_check(
            tx,
            expected_chain_id,
            max_clock_drift,
            limits,
            crypto,
            now,
            SignatureCheck::Verify,
            None,
        )
    }

    /// Validate a transaction with metadata prepared once by the caller.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`].
    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate_with_now_and_prepared_metadata(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
        now: Duration,
        prepared: &PreparedTransactionMetadata,
    ) -> Result<(), AcceptTransactionFail> {
        Self::validate_with_now_and_signature_check(
            tx,
            expected_chain_id,
            max_clock_drift,
            limits,
            crypto,
            now,
            SignatureCheck::Verify,
            Some(prepared),
        )
    }

    /// Validate a transaction with metadata prepared once by the caller and a precomputed signature result.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`].
    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate_with_now_with_signature_result_and_prepared_metadata(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
        now: Duration,
        prechecked_signature_result: Option<Result<(), SignatureVerificationFail>>,
        prepared: &PreparedTransactionMetadata,
    ) -> Result<(), AcceptTransactionFail> {
        let signature_check =
            prechecked_signature_result.map_or(SignatureCheck::Verify, SignatureCheck::Override);
        Self::validate_with_now_and_signature_check(
            tx,
            expected_chain_id,
            max_clock_drift,
            limits,
            crypto,
            now,
            signature_check,
            Some(prepared),
        )
    }

    /// Validate a transaction after a successful single-Ed25519 precheck with prepared metadata.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`].
    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate_with_now_after_single_ed25519_precheck_and_prepared_metadata(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
        now: Duration,
        prepared: &PreparedTransactionMetadata,
    ) -> Result<(), AcceptTransactionFail> {
        Self::validate_with_now_and_signature_check(
            tx,
            expected_chain_id,
            max_clock_drift,
            limits,
            crypto,
            now,
            SignatureCheck::PrecheckedSingleEd25519,
            Some(prepared),
        )
    }

    #[allow(clippy::too_many_lines)]
    fn validate_with_now_and_signature_check(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
        now: Duration,
        signature_check: SignatureCheck,
        prepared: Option<&PreparedTransactionMetadata>,
    ) -> Result<(), AcceptTransactionFail> {
        reject_retired_heartbeat_metadata(tx).map_err(AcceptTransactionFail::TransactionLimit)?;
        Self::validate_common(tx, expected_chain_id, max_clock_drift, now)?;

        let ttl = tx.time_to_live().ok_or_else(|| {
            AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                reason: "Transaction `time_to_live_ms` is required and must be signature-bound"
                    .into(),
            })
        })?;
        let ttl_ms = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX);
        let max_ttl_ms = limits.max_time_to_live_ms().get();
        if ttl_ms > max_ttl_ms {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: format!(
                        "Transaction time-to-live {ttl_ms} ms exceeds the governed limit \
                         {max_ttl_ms} ms"
                    ),
                },
            ));
        }
        let expires_at = tx.creation_time().checked_add(ttl).ok_or_else(|| {
            AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                reason: "Transaction creation time plus time-to-live exceeds the timestamp range"
                    .into(),
            })
        })?;
        if now > expires_at {
            return Err(AcceptTransactionFail::TransactionExpired {
                expires_at_ms: expires_at.as_millis(),
                now_ms: now.as_millis(),
            });
        }

        Self::ensure_signing_allowed(tx, crypto)?;

        Self::verify_signature_for_check(tx, signature_check, prepared)?;

        let signature_count = tx.signature_count();
        Self::ensure_signature_limit(signature_count, &limits)?;

        let tx_encoded_len = Self::signed_encoded_len_for_limit_with_prepared(tx, prepared);
        let max_tx_bytes = limits.max_tx_bytes().get();
        if tx_encoded_len > max_tx_bytes {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: format!(
                        "Transaction size {tx_encoded_len} bytes exceeds limit {max_tx_bytes} bytes"
                    ),
                },
            ));
        }

        validate_proof_attachment_shapes(tx)?;

        let decompressed_len = tx.attachments().map_or(0usize, |attachments| {
            attachments.as_slice().iter().fold(0usize, |acc, attachment| {
                let mut subtotal = attachment.proof.bytes.len();
                if attachment.vk_commitment.is_some() {
                    subtotal = subtotal.saturating_add(32);
                }
                if attachment.envelope_hash.is_some() {
                    subtotal = subtotal.saturating_add(32);
                }
                if let Some(privacy) = &attachment.lane_privacy {
                    subtotal = subtotal.saturating_add(privacy.encoded_len());
                }
                acc.saturating_add(subtotal)
            })
        });
        let decompressed_len = u64::try_from(decompressed_len).unwrap_or(u64::MAX);
        let max_decompressed_bytes = limits.max_decompressed_bytes().get();
        if decompressed_len > max_decompressed_bytes {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: format!(
                        "Transaction attachments expand to {decompressed_len} bytes which exceeds limit {max_decompressed_bytes} bytes"
                    ),
                },
            ));
        }

        let expires_at_height_meta = tx.expires_at_height().map_err(|err| {
            AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                reason: format!(
                    "Transaction metadata `expires_at_height` must be an unsigned integer: {err}"
                ),
            })
        })?;
        if limits.require_height_ttl && expires_at_height_meta.is_none() {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: "Transaction metadata `expires_at_height` is required by configuration"
                        .into(),
                },
            ));
        }

        let tx_sequence_meta = tx.tx_sequence().map_err(|err| {
            AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                reason: format!(
                    "Transaction metadata `tx_sequence` must be an unsigned integer: {err}"
                ),
            })
        })?;
        if limits.require_sequence && tx_sequence_meta.is_none() {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: "Transaction metadata `tx_sequence` is required by configuration"
                        .into(),
                },
            ));
        }

        let max_metadata_depth = usize::from(limits.max_metadata_depth().get());
        ensure_metadata_depth_with_prepared(tx.metadata(), max_metadata_depth, prepared)
            .map_err(AcceptTransactionFail::TransactionLimit)?;

        // Attachment payloads currently carry flat structures; no additional nesting cap required.
        match &tx.instructions() {
            Executable::Instructions(instructions) => {
                if instructions.is_empty() {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: "Transaction must contain at least one instruction".into(),
                        },
                    ));
                }

                let instruction_limit = limits.max_instructions().get();
                let instruction_count = u64::try_from(instructions.len()).unwrap_or(u64::MAX);
                if instruction_count > instruction_limit {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: format!(
                                "Too many instructions in payload, max number is {}, but got {}",
                                limits.max_instructions(),
                                instructions.len()
                            ),
                        },
                    ));
                }
            }
            Executable::ContractCall(_) => {
                iroha_data_model::transaction::require_transaction_gas_limit(
                    tx.fee_payment_intent(),
                )
                .map_err(|err| {
                    AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                        reason: err.to_string(),
                    })
                })?;
            }
            Executable::Batch(items) => {
                if items.is_empty() {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: "Transaction executable batch must not be empty".into(),
                        },
                    ));
                }

                let item_limit = limits.max_instructions().get();
                let item_count = u64::try_from(items.len()).unwrap_or(u64::MAX);
                if item_count > item_limit {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: format!(
                                "Too many executable batch items, max number is {}, but got {}",
                                limits.max_instructions(),
                                items.len()
                            ),
                        },
                    ));
                }

                if items
                    .iter()
                    .any(|item| matches!(item, ExecutableBatchItem::ContractCall(_)))
                {
                    iroha_data_model::transaction::require_transaction_gas_limit(
                        tx.fee_payment_intent(),
                    )
                    .map_err(|err| {
                        AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                            reason: err.to_string(),
                        })
                    })?;
                }
            }
            Executable::IvmProved(proved) => {
                iroha_data_model::transaction::require_transaction_gas_limit(
                    tx.fee_payment_intent(),
                )
                .map_err(|err| {
                    AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                        reason: err.to_string(),
                    })
                })?;

                let instruction_limit = limits.max_instructions().get();
                let instruction_count = u64::try_from(proved.overlay.len()).unwrap_or(u64::MAX);
                if instruction_count > instruction_limit {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: format!(
                                "Too many instructions in proved overlay, max number is {}, but got {}",
                                limits.max_instructions(),
                                proved.overlay.len()
                            ),
                        },
                    ));
                }

                let ivm_bytecode_size_limit = limits.ivm_bytecode_size().get();
                let bytecode_size = u64::try_from(proved.bytecode.size_bytes()).unwrap_or(u64::MAX);
                if bytecode_size > ivm_bytecode_size_limit {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: format!(
                                "IVM bytecode size is too large: max {}, got {} \
                                (configured by \"Parameter::SmartContractLimits\")",
                                limits.ivm_bytecode_size(),
                                proved.bytecode.size_bytes()
                            ),
                        },
                    ));
                }

                // Decode the program header to obtain the code section and enforce the global
                // instruction count limit published via `TransactionParameters`.
                let parsed =
                    ivm::ProgramMetadata::parse(proved.bytecode.as_ref()).map_err(|err| {
                        AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                            reason: format!("Failed to parse IVM metadata: {err}"),
                        })
                    })?;
                let code = &proved.bytecode.as_ref()[parsed.code_offset..];
                let decoded = ivm::ivm_cache::global_get_with_meta(code, &parsed.metadata)
                    .map_err(|err| {
                        AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                            reason: format!("Failed to decode IVM instructions: {err}"),
                        })
                    })?;

                let decoded_bytes = decoded
                    .iter()
                    .try_fold(0u64, |acc, op| acc.checked_add(u64::from(op.len)))
                    .unwrap_or(u64::MAX);
                if decoded_bytes > ivm_bytecode_size_limit {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: format!(
                                "Decoded IVM instruction stream exceeds byte limit {} with {} bytes",
                                limits.ivm_bytecode_size(),
                                decoded_bytes
                            ),
                        },
                    ));
                }

                let decoded_len = u64::try_from(decoded.len()).unwrap_or(u64::MAX);
                if decoded_len > instruction_limit {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: format!(
                                "Too many IVM instructions in payload, max number is {}, but decoded {}",
                                limits.max_instructions(),
                                decoded.len()
                            ),
                        },
                    ));
                }
            }
            Executable::Ivm(smart_contract) => {
                iroha_data_model::transaction::require_transaction_gas_limit(
                    tx.fee_payment_intent(),
                )
                .map_err(|err| {
                    AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                        reason: err.to_string(),
                    })
                })?;

                let ivm_bytecode_size_limit = limits.ivm_bytecode_size().get();
                let bytecode_size = u64::try_from(smart_contract.size_bytes()).unwrap_or(u64::MAX);
                if bytecode_size > ivm_bytecode_size_limit {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: format!(
                                "IVM bytecode size is too large: max {}, got {} \
                                (configured by \"Parameter::SmartContractLimits\")",
                                limits.ivm_bytecode_size(),
                                smart_contract.size_bytes()
                            ),
                        },
                    ));
                }

                // Decode the program header to obtain the code section and enforce the global
                // instruction count limit published via `TransactionParameters`.
                let parsed =
                    ivm::ProgramMetadata::parse(smart_contract.as_ref()).map_err(|err| {
                        AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                            reason: format!("Failed to parse IVM metadata: {err}"),
                        })
                    })?;
                let code = &smart_contract.as_ref()[parsed.code_offset..];
                let decoded = ivm::ivm_cache::global_get_with_meta(code, &parsed.metadata)
                    .map_err(|err| {
                        AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                            reason: format!("Failed to decode IVM instructions: {err}"),
                        })
                    })?;

                let decoded_bytes = decoded
                    .iter()
                    .try_fold(0u64, |acc, op| acc.checked_add(u64::from(op.len)))
                    .unwrap_or(u64::MAX);
                if decoded_bytes > ivm_bytecode_size_limit {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: format!(
                                "Decoded IVM instruction stream exceeds byte limit {} with {} bytes",
                                limits.ivm_bytecode_size(),
                                decoded_bytes
                            ),
                        },
                    ));
                }

                let instruction_limit = limits.max_instructions().get();
                let decoded_len = u64::try_from(decoded.len()).unwrap_or(u64::MAX);
                if decoded_len > instruction_limit {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: format!(
                                "Too many IVM instructions in payload, max number is {}, but decoded {}",
                                limits.max_instructions(),
                                decoded.len()
                            ),
                        },
                    ));
                }
            }
        }

        Ok(())
    }

    /// Create [`Self`] assuming the signed transaction is acceptable.
    pub fn new_unchecked(tx: impl Into<Cow<'tx, SignedTransaction>>) -> Self {
        let entrypoint = match tx.into() {
            Cow::Borrowed(signed) => Cow::Owned(TransactionEntrypoint::External(signed.clone())),
            Cow::Owned(signed) => return Self::from_external_with_hot_cache(signed),
        };
        Self::from_entrypoint(entrypoint)
    }

    /// Create [`Self`] assuming the entrypoint is acceptable.
    pub fn new_unchecked_entrypoint(tx: impl Into<Cow<'tx, TransactionEntrypoint>>) -> Self {
        match tx.into() {
            Cow::Owned(TransactionEntrypoint::External(signed)) => {
                Self::from_external_with_hot_cache(signed)
            }
            entrypoint => Self::from_entrypoint(entrypoint),
        }
    }

    /// Borrow the underlying entrypoint.
    #[must_use]
    pub fn entrypoint(&self) -> &TransactionEntrypoint {
        self.entrypoint.as_ref()
    }

    /// Consume the accepted transaction and return its wrapped entrypoint.
    #[must_use]
    pub(crate) fn into_entrypoint(self) -> TransactionEntrypoint {
        self.entrypoint.into_owned()
    }

    /// Borrow the wrapped signed transaction when present.
    #[must_use]
    pub fn external(&self) -> Option<&SignedTransaction> {
        match self.entrypoint() {
            TransactionEntrypoint::External(entrypoint) => Some(entrypoint),
            TransactionEntrypoint::SealedReveal(entrypoint) => {
                Some(entrypoint.signed_transaction())
            }
            TransactionEntrypoint::SealedCommitment(_)
            | TransactionEntrypoint::PrivateKaigi(_)
            | TransactionEntrypoint::Time(_) => None,
        }
    }

    /// Return the canonical hash of the wrapped transaction.
    #[must_use]
    pub fn hash(&self) -> HashOf<SignedTransaction> {
        *self
            .signed_hash
            .get_or_init(|| Self::compat_signed_hash(self.hash_as_entrypoint()))
    }

    /// Return the canonical entrypoint hash of the wrapped transaction.
    #[must_use]
    pub fn hash_as_entrypoint(&self) -> HashOf<TransactionEntrypoint> {
        *self
            .entrypoint_hash
            .get_or_init(|| self.entrypoint().hash())
    }

    /// Return the exact encoded size used by queue and transaction-size budgeting.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        *self
            .encoded_len
            .get_or_init(|| Self::entrypoint_encoded_len(self.entrypoint()))
    }

    /// Return cached canonical signed-transaction bytes for queue gossip when this is external.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn signed_bytes(&self) -> Option<Arc<Vec<u8>>> {
        self.signed_bytes
            .get_or_init(|| {
                self.external().map(|signed| {
                    Arc::new(norito::to_bytes(signed).expect("encode signed transaction"))
                })
            })
            .as_ref()
            .map(Arc::clone)
    }

    /// Return cached canonical transaction-entrypoint bytes for queue gossip.
    #[must_use]
    pub(crate) fn entrypoint_bytes(&self) -> Arc<Vec<u8>> {
        Arc::clone(self.entrypoint_bytes.get_or_init(|| {
            Arc::new(
                norito::encode_canonical(self.entrypoint())
                    .expect("encode canonical transaction entrypoint"),
            )
        }))
    }

    /// Return cached payload hash when this is an external transaction.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn payload_hash(
        &self,
    ) -> Option<HashOf<iroha_data_model::transaction::signed::TransactionPayload>> {
        *self
            .payload_hash
            .get_or_init(|| self.external().map(|signed| HashOf::new(signed.payload())))
    }

    /// Return cached parsed Ed25519 key for single-key Ed25519 authorities.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn single_ed25519_key(&self) -> Option<iroha_crypto::Ed25519ParsedPublicKey> {
        *self
            .single_ed25519_key
            .get_or_init(|| self.external().and_then(Self::parsed_single_ed25519_key))
    }

    /// Return prepared metadata for an external transaction.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn prepared_metadata(&self) -> Option<PreparedTransactionMetadata> {
        self.external()?;
        let payload_hash = *self
            .payload_hash
            .get_or_init(|| self.external().map(|signed| HashOf::new(signed.payload())))
            .as_ref()?;
        let single_ed25519_key = *self
            .single_ed25519_key
            .get_or_init(|| self.external().and_then(Self::parsed_single_ed25519_key));
        Some(PreparedTransactionMetadata {
            signed_hash: self.hash(),
            entrypoint_hash: self.hash_as_entrypoint(),
            payload_hash,
            encoded_len: self.encoded_len(),
            signed_bytes: self.signed_bytes(),
            entrypoint_bytes: Some(self.entrypoint_bytes()),
            single_ed25519_key,
            metadata_depths: prepare_metadata_depths(self.metadata()?),
        })
    }

    pub(crate) fn stateless_cache_metadata(&self) -> Option<PreparedTransactionMetadata> {
        let signed = self.external()?;
        let payload_hash = *self
            .payload_hash
            .get_or_init(|| Some(HashOf::new(signed.payload())))
            .as_ref()?;
        let single_ed25519_key = *self
            .single_ed25519_key
            .get_or_init(|| Self::parsed_single_ed25519_key(signed));
        Some(PreparedTransactionMetadata {
            signed_hash: self.hash(),
            entrypoint_hash: self.hash_as_entrypoint(),
            payload_hash,
            encoded_len: self.encoded_len(),
            signed_bytes: None,
            entrypoint_bytes: None,
            single_ed25519_key,
            metadata_depths: prepare_metadata_depths(self.metadata()?),
        })
    }

    /// Borrow the transaction authority account identifier when present.
    #[must_use]
    pub fn authority_opt(&self) -> Option<&AccountId> {
        self.entrypoint().authority_opt()
    }

    /// Borrow the transaction authority account identifier.
    #[must_use]
    pub fn authority(&self) -> &AccountId {
        self.entrypoint().authority()
    }

    /// Entry-point metadata when present.
    #[must_use]
    pub fn metadata(&self) -> Option<&Metadata> {
        self.entrypoint().metadata()
    }

    /// Creation timestamp for queue expiry and projections.
    #[must_use]
    pub fn creation_time(&self) -> Duration {
        match self.entrypoint() {
            TransactionEntrypoint::External(entrypoint) => entrypoint.creation_time(),
            TransactionEntrypoint::SealedCommitment(_) => Duration::ZERO,
            TransactionEntrypoint::SealedReveal(entrypoint) => {
                entrypoint.signed_transaction().creation_time()
            }
            TransactionEntrypoint::PrivateKaigi(entrypoint) => entrypoint.creation_time(),
            TransactionEntrypoint::Time(_) => Duration::ZERO,
        }
    }

    /// Entry-point TTL when one exists.
    #[must_use]
    pub fn time_to_live(&self) -> Option<Duration> {
        self.external().and_then(SignedTransaction::time_to_live)
    }
}

impl AcceptedTransaction<'static> {
    fn validate_entrypoint_with_now(
        tx: &TransactionEntrypoint,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
        now: Duration,
    ) -> Result<(), AcceptTransactionFail> {
        match tx {
            TransactionEntrypoint::External(signed) => {
                Self::validate_with_now(
                    signed,
                    expected_chain_id,
                    max_clock_drift,
                    limits,
                    crypto,
                    now,
                )?;
                enforce_nts_health_for_time_sensitive(signed)?;
            }
            TransactionEntrypoint::SealedCommitment(commitment) => {
                validate_sealed_commitment_stateless(commitment, expected_chain_id, limits)?;
            }
            TransactionEntrypoint::SealedReveal(reveal) => {
                let signed = reveal.signed_transaction();
                Self::validate_with_now(
                    signed,
                    expected_chain_id,
                    max_clock_drift,
                    limits,
                    crypto,
                    now,
                )?;
                enforce_nts_health_for_time_sensitive(signed)?;
            }
            TransactionEntrypoint::PrivateKaigi(private) => {
                Self::validate_private_kaigi_with_now(
                    private,
                    expected_chain_id,
                    max_clock_drift,
                    limits,
                    now,
                )?;
            }
            TransactionEntrypoint::Time(_) => {
                return Err(AcceptTransactionFail::TransactionLimit(
                    TransactionLimitError {
                        reason: "direct time entrypoints are not accepted on ingress".into(),
                    },
                ));
            }
        }
        Ok(())
    }

    /// Accept genesis transaction. Transition from [`SignedTransaction`] to [`AcceptedTransaction`].
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`]
    pub fn accept_genesis(
        tx: SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        genesis_account: &AccountId,
        crypto: &iroha_config::parameters::actual::Crypto,
    ) -> Result<Self, AcceptTransactionFail> {
        Self::validate_genesis(
            &tx,
            expected_chain_id,
            max_clock_drift,
            genesis_account,
            crypto,
        )
        .map(|()| Self::from_external_with_hot_cache(tx))
    }

    /// Accept transaction. Transition from [`SignedTransaction`] to [`AcceptedTransaction`].
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`]
    pub fn accept(
        tx: SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
    ) -> Result<Self, AcceptTransactionFail> {
        Self::validate(&tx, expected_chain_id, max_clock_drift, limits, crypto)
            .map(|()| Self::from_external_with_hot_cache(tx))
    }

    /// Accept transaction using caller-provided canonical signed-transaction bytes.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`].
    #[cfg(test)]
    pub(crate) fn accept_with_canonical_signed_bytes(
        tx: SignedTransaction,
        signed_bytes: Arc<Vec<u8>>,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
    ) -> Result<Self, AcceptTransactionFail> {
        Self::validate(&tx, expected_chain_id, max_clock_drift, limits, crypto)
            .map(|()| Self::from_external_with_cached_bytes(tx, Some(signed_bytes)))
    }

    /// Accept transaction using a caller-provided [`TimeSource`] for admission-time checks.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`]
    pub fn accept_with_time_source(
        tx: SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
        time_source: &TimeSource,
    ) -> Result<Self, AcceptTransactionFail> {
        let now = time_source.get_unix_time();
        Self::validate_with_now(&tx, expected_chain_id, max_clock_drift, limits, crypto, now)?;
        enforce_nts_health_for_time_sensitive(&tx)?;
        Ok(Self::from_external_with_hot_cache(tx))
    }

    /// Accept any directly submitted transaction entrypoint.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`].
    pub fn accept_entrypoint(
        tx: TransactionEntrypoint,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
    ) -> Result<Self, AcceptTransactionFail> {
        let now = current_unix_time();
        Self::validate_entrypoint_with_now(
            &tx,
            expected_chain_id,
            max_clock_drift,
            limits,
            crypto,
            now,
        )?;
        Ok(match tx {
            TransactionEntrypoint::External(signed) => Self::from_external_with_hot_cache(signed),
            other => Self::from_entrypoint(Cow::Owned(other)),
        })
    }

    /// Accept an entrypoint at one explicit validation instant.
    ///
    /// Durable recovery uses the persisted enqueue instant to prove that an acknowledged
    /// entrypoint passed the same chain, signature, crypto-policy, clock, and limit checks as
    /// ingress before classifying its current terminal state.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`].
    pub(crate) fn accept_entrypoint_at_time(
        tx: TransactionEntrypoint,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
        validation_time: Duration,
    ) -> Result<Self, AcceptTransactionFail> {
        Self::validate_entrypoint_with_now(
            &tx,
            expected_chain_id,
            max_clock_drift,
            limits,
            crypto,
            validation_time,
        )?;
        Ok(match tx {
            TransactionEntrypoint::External(signed) => Self::from_external_with_hot_cache(signed),
            other => Self::from_entrypoint(Cow::Owned(other)),
        })
    }

    /// Accept an already-decoded gossip entrypoint and seed its canonical entrypoint frame bytes.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`].
    pub(crate) fn accept_gossip_entrypoint_with_payload(
        tx: TransactionEntrypoint,
        entrypoint_bytes: Arc<Vec<u8>>,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
    ) -> Result<Self, AcceptTransactionFail> {
        Self::accept_gossip_entrypoint_with_payload_and_prepared_metadata(
            tx,
            entrypoint_bytes,
            entrypoint_hash,
            expected_chain_id,
            max_clock_drift,
            limits,
            crypto,
            None,
            false,
        )
    }

    /// Accept an already-decoded gossip entrypoint using caller-prepared metadata.
    ///
    /// This path lets gossip admission reuse deterministic single-Ed25519 batch
    /// verification while still running every non-signature admission check.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`].
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn accept_gossip_entrypoint_with_payload_and_prepared_metadata(
        tx: TransactionEntrypoint,
        entrypoint_bytes: Arc<Vec<u8>>,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
        prepared: Option<&PreparedTransactionMetadata>,
        single_ed25519_prechecked: bool,
    ) -> Result<Self, AcceptTransactionFail> {
        let now = current_unix_time();
        match &tx {
            TransactionEntrypoint::External(signed) => {
                if let Some(prepared) = prepared {
                    if single_ed25519_prechecked {
                        Self::validate_with_now_after_single_ed25519_precheck_and_prepared_metadata(
                            signed,
                            expected_chain_id,
                            max_clock_drift,
                            limits,
                            crypto,
                            now,
                            prepared,
                        )?;
                    } else {
                        Self::validate_with_now_and_prepared_metadata(
                            signed,
                            expected_chain_id,
                            max_clock_drift,
                            limits,
                            crypto,
                            now,
                            prepared,
                        )?;
                    }
                    enforce_nts_health_for_time_sensitive(signed)?;
                } else {
                    Self::validate_entrypoint_with_now(
                        &tx,
                        expected_chain_id,
                        max_clock_drift,
                        limits,
                        crypto,
                        now,
                    )?;
                }
            }
            _ => {
                Self::validate_entrypoint_with_now(
                    &tx,
                    expected_chain_id,
                    max_clock_drift,
                    limits,
                    crypto,
                    now,
                )?;
            }
        }

        let accepted = Self::from_entrypoint_with_cached_entrypoint_bytes(
            tx,
            entrypoint_bytes,
            entrypoint_hash,
        );
        if let Some(prepared) = prepared {
            let _ = accepted.encoded_len.set(prepared.encoded_len);
            let _ = accepted.payload_hash.set(Some(prepared.payload_hash));
            let _ = accepted.single_ed25519_key.set(prepared.single_ed25519_key);
            if let Some(signed_bytes) = prepared.signed_bytes.as_ref() {
                let _ = accepted.signed_bytes.set(Some(Arc::clone(signed_bytes)));
            }
            if let Some(entrypoint_bytes) = prepared.entrypoint_bytes.as_ref() {
                let _ = accepted.entrypoint_bytes.set(Arc::clone(entrypoint_bytes));
            }
            let _ = accepted.signed_hash.set(prepared.signed_hash);
            let _ = accepted.entrypoint_hash.set(prepared.entrypoint_hash);
        }
        Ok(accepted)
    }
}

impl<'tx> From<AcceptedTransaction<'tx>> for SignedTransaction {
    fn from(source: AcceptedTransaction<'tx>) -> Self {
        match source.entrypoint.into_owned() {
            TransactionEntrypoint::External(entrypoint) => entrypoint,
            TransactionEntrypoint::SealedReveal(entrypoint) => entrypoint.signed_transaction,
            TransactionEntrypoint::SealedCommitment(_) => {
                panic!("sealed commitment entrypoints are not signed transactions")
            }
            TransactionEntrypoint::PrivateKaigi(_) => {
                panic!("private Kaigi entrypoints are not signed transactions")
            }
            TransactionEntrypoint::Time(_) => {
                panic!("time entrypoints are not signed transactions")
            }
        }
    }
}

impl<'tx> From<AcceptedTransaction<'tx>> for (AccountId, Executable) {
    fn from(source: AcceptedTransaction<'tx>) -> Self {
        SignedTransaction::from(source).into()
    }
}

impl AsRef<SignedTransaction> for AcceptedTransaction<'_> {
    fn as_ref(&self) -> &SignedTransaction {
        self.external()
            .expect("private Kaigi entrypoints do not expose SignedTransaction access")
    }
}

impl StateBlock<'_> {
    /// Validate stateful admission rules that must hold before transaction execution.
    ///
    /// This helper intentionally does not execute instructions or apply state. Callers must only
    /// commit the returned sequence after the transaction itself succeeds.
    pub(crate) fn validate_stateful_admission(
        tx: &SignedTransaction,
        state_transaction: &mut StateTransaction<'_, '_>,
        routing_decision: Option<crate::queue::RoutingDecision>,
    ) -> Result<StatefulAdmission, TransactionRejectionReason> {
        let authority = tx.authority().clone();
        if code::is_historical_contract_subject(&state_transaction.world, &authority) {
            warn!(
                authority = %authority,
                "deployed contract subjects cannot sign transactions directly"
            );
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(CONTRACT_SUBJECT_DIRECT_SIGN_REJECTION.into()),
            ));
        }

        let authority_exists = state_transaction.world.accounts.get(&authority).is_some();
        let allow_unregistered_authority =
            !authority_exists && allows_unregistered_authority(tx.instructions(), &authority);

        // Multisig propose/approve envelopes may use an unmaterialized authority because
        // authorization is derived from multisig membership rather than account storage. All
        // other transactions must originate from an existing account.
        if !authority_exists && !allow_unregistered_authority {
            return Err(TransactionRejectionReason::AccountDoesNotExist(
                FindError::Account(authority.clone()),
            ));
        }

        if let Executable::Instructions(instructions) = tx.instructions()
            && instructions.is_empty()
        {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "Transaction must contain at least one instruction".to_owned(),
                ),
            ));
        }

        let (require_height_ttl, require_sequence) = {
            let params = state_transaction.world.parameters();
            (
                params.transaction.require_height_ttl,
                params.transaction.require_sequence,
            )
        };

        let expires_at_height = tx.expires_at_height().map_err(|err| {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
                "Transaction metadata `expires_at_height` must be an unsigned integer: {err}"
            )))
        })?;

        let tx_sequence_value = tx.tx_sequence().map_err(|err| {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
                "Transaction metadata `tx_sequence` must be an unsigned integer: {err}"
            )))
        })?;

        if require_height_ttl {
            let expiry = expires_at_height.ok_or_else(|| {
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                    "Transaction metadata `expires_at_height` is required by configuration".into(),
                ))
            })?;
            let current_height = state_transaction.block_height();
            if current_height >= expiry {
                return Err(TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted(format!(
                        "Transaction expired at height {expiry}; current height is {current_height}"
                    )),
                ));
            }
        }

        let mut sequence_to_commit = None;
        if let Some(seq) = tx_sequence_value {
            let previous = state_transaction
                .world
                .tx_sequences
                .get(&authority)
                .copied();
            if let Some(prev) = previous {
                if seq <= prev {
                    if require_sequence {
                        return Err(TransactionRejectionReason::Validation(
                            ValidationFail::NotPermitted(format!(
                                "Transaction sequence {seq} for {authority} must exceed previous {prev}"
                            )),
                        ));
                    }
                } else {
                    sequence_to_commit = Some(seq);
                }
            } else {
                sequence_to_commit = Some(seq);
            }
        } else if require_sequence {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "Transaction metadata `tx_sequence` is required by configuration".into(),
                ),
            ));
        }

        #[cfg(feature = "telemetry")]
        let telemetry_handle: Option<&StateTelemetry> = Some(state_transaction.telemetry);
        #[cfg(not(feature = "telemetry"))]
        let telemetry_handle: Option<&StateTelemetry> = None;

        if let Ok(account) = state_transaction.world.account(&authority) {
            let has_multisig_state = state_transaction
                .world
                .smart_contract_state
                .get(&crate::smartcontracts::isi::multisig::multisig_account_state_key(&authority))
                .is_some();
            let has_multisig_metadata = account
                .metadata()
                .get(&crate::smartcontracts::isi::multisig::spec_key())
                .is_some();
            let has_multisig_controller = authority.multisig_policy().is_some();
            let allows_multisig_envelope_authority = match tx.instructions() {
                Executable::Instructions(instructions) => {
                    instructions_allow_multisig_envelope_authority(instructions)
                }
                Executable::ContractCall(_)
                | Executable::Batch(_)
                | Executable::IvmProved(_)
                | Executable::Ivm(_) => false,
            };
            if (has_multisig_state || has_multisig_metadata || has_multisig_controller)
                && !allows_multisig_envelope_authority
            {
                warn!(
                    authority = %authority,
                    "multisig accounts cannot sign transactions directly"
                );
                #[cfg(feature = "telemetry")]
                if let Some(telemetry) = telemetry_handle {
                    crate::telemetry::record_social_rejection(telemetry, "multisig_direct_sign");
                }
                return Err(TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted(MULTISIG_DIRECT_SIGN_REJECTION.into()),
                ));
            }
        }

        let routing_decision = match routing_decision {
            Some(decision) => decision,
            None => {
                let accepted = AcceptedTransaction::new_unchecked(Cow::Borrowed(tx));
                evaluate_policy_plan_with_nexus_and_world_at_block_height(
                    &state_transaction.nexus,
                    &accepted,
                    &state_transaction.world,
                    state_transaction.block_unix_timestamp_ms(),
                    state_transaction.block_height(),
                )
                .map(|plan| plan.coordinator_route())
                .map_err(|err| {
                    TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
                        "transaction routing could not be resolved: {err}"
                    )))
                })?
            }
        };
        state_transaction.current_lane_id = Some(routing_decision.lane_id);
        state_transaction.current_dataspace_id = Some(routing_decision.dataspace_id);
        state_transaction.world.current_dataspace_id = Some(routing_decision.dataspace_id);
        crate::executor::validate_transaction_fee_admission(state_transaction, tx)
            .map_err(TransactionRejectionReason::Validation)?;
        let lane_assignment = LaneAssignment {
            lane_id: routing_decision.lane_id,
            dataspace_id: routing_decision.dataspace_id,
            dataspace_catalog: &state_transaction.nexus.dataspace_catalog,
        };

        enforce_lane_policies(tx, state_transaction, &lane_assignment)?;
        validate_confidential_policy_admission_for_world(
            tx.instructions(),
            &state_transaction.world,
            state_transaction.block_height(),
        )?;
        let validation_fee_credit =
            crate::validation_fee::enforce_validation_fee_admission(tx, state_transaction)?;

        enforce_fraud_policy(
            &state_transaction.fraud_monitoring,
            tx.metadata(),
            telemetry_handle,
            &lane_assignment,
        )?;

        Ok(StatefulAdmission {
            authority,
            allow_unregistered_authority,
            sequence_to_commit,
            validation_fee_credit,
        })
    }

    /// Validate and apply the transaction to the state if validation succeeds; leave the state unchanged on failure.
    ///
    /// Returns the hash and the result of the transaction -- the trigger sequence on success, or the rejection reason on failure.
    pub fn validate_transaction(
        &mut self,
        tx: AcceptedTransaction<'_>,
        ivm_cache: &mut IvmCache,
    ) -> (HashOf<TransactionEntrypoint>, TransactionResultInner) {
        self.validate_transaction_at_entrypoint_index_and_routing(tx, ivm_cache, None, None)
    }

    /// Validate and apply a transaction with both its original block entrypoint index and routing context.
    ///
    /// Returns the hash and the result of the transaction.
    pub(crate) fn validate_transaction_with_entrypoint_index_and_routing_context(
        &mut self,
        tx: AcceptedTransaction<'_>,
        ivm_cache: &mut IvmCache,
        entrypoint_index: usize,
        routing: crate::queue::RoutingDecision,
    ) -> (HashOf<TransactionEntrypoint>, TransactionResultInner) {
        self.validate_transaction_at_entrypoint_index_and_routing(
            tx,
            ivm_cache,
            Some(u64::try_from(entrypoint_index).unwrap_or(u64::MAX)),
            Some(routing),
        )
    }

    /// Validate recovered standalone lane-block execution input in descriptor order.
    ///
    /// Successful transactions stage their state effects in this [`StateBlock`]
    /// using the lane/dataspace routing context and the original fetched-batch
    /// entrypoint indices from the lane descriptor. The caller owns the commit
    /// boundary: dropping the block reverts the staged effects, while committing
    /// the block must use a real consensus-approved block context.
    pub(crate) fn validate_lane_block_execution_input_with_routing_context(
        &mut self,
        artifact: &crate::kura::LaneBlockExecutionInputArtifact,
        ivm_cache: &mut IvmCache,
    ) -> core::result::Result<
        Vec<(u64, HashOf<TransactionEntrypoint>, TransactionResultInner)>,
        &'static str,
    > {
        Self::validate_lane_block_execution_input_unique_entrypoints(artifact)?;
        crate::kura::Kura::validate_lane_block_execution_input_artifact(artifact)?;
        let descriptor = &artifact.proposal.descriptor;
        let routing =
            crate::queue::RoutingDecision::new(descriptor.lane_id, descriptor.dataspace_id);
        let mut results = Vec::with_capacity(artifact.entrypoints.len());
        for (position, (raw_entrypoint_index, entrypoint)) in descriptor
            .accepted_candidate_indices
            .iter()
            .copied()
            .zip(artifact.entrypoints.iter())
            .enumerate()
        {
            let accepted = AcceptedTransaction::new_unchecked_entrypoint(Cow::Borrowed(entrypoint));
            let plan = if let Some(bound) = artifact.routing_plans.get(position) {
                // Autonomous payloads carry a producer-authenticated plan bound
                // to the proposal-height incarnation. Recomputing against the
                // current catalog would make valid delayed merges depend on
                // unrelated scale-out or policy drift.
                bound.clone()
            } else {
                evaluate_policy_plan_with_nexus_and_world_at_block_height(
                    &self.nexus,
                    &accepted,
                    &self.world,
                    u64::try_from(self._curr_block.creation_time().as_millis()).unwrap_or(u64::MAX),
                    descriptor.proposal_height,
                )
                .map_err(|_| "execution input routing cannot be resolved")?
            };
            if plan.coordinator_route() != routing {
                return Err("execution input route does not match recomputed coordinator route");
            }
            let (entrypoint_hash, result) = self
                .validate_transaction_at_entrypoint_index_and_routing(
                    accepted,
                    ivm_cache,
                    Some(raw_entrypoint_index),
                    Some(routing),
                );
            results.push((raw_entrypoint_index, entrypoint_hash, result));
        }
        Ok(results)
    }

    fn validate_lane_block_execution_input_unique_entrypoints(
        artifact: &crate::kura::LaneBlockExecutionInputArtifact,
    ) -> core::result::Result<(), &'static str> {
        let mut seen_entrypoint_hashes = BTreeSet::new();
        let mut seen_signed_hashes = BTreeSet::new();
        let mut seen_sealed_commitments = BTreeSet::new();
        for entrypoint in &artifact.entrypoints {
            if !seen_entrypoint_hashes.insert(entrypoint.hash()) {
                return Err("execution input contains duplicate entrypoints");
            }
            match entrypoint {
                TransactionEntrypoint::External(signed) => {
                    let signed_hash =
                        AcceptedTransaction::prepare_signed_metadata(signed).signed_hash;
                    if !seen_signed_hashes.insert(signed_hash) {
                        return Err("execution input contains duplicate signed transactions");
                    }
                }
                TransactionEntrypoint::SealedReveal(reveal) => {
                    let signed_hash =
                        AcceptedTransaction::prepare_signed_metadata(reveal.signed_transaction())
                            .signed_hash;
                    if !seen_signed_hashes.insert(signed_hash) {
                        return Err("execution input contains duplicate signed transactions");
                    }
                }
                TransactionEntrypoint::SealedCommitment(commitment) => {
                    if !seen_sealed_commitments.insert(*commitment.commitment()) {
                        return Err("execution input contains duplicate sealed commitments");
                    }
                }
                TransactionEntrypoint::PrivateKaigi(_) | TransactionEntrypoint::Time(_) => {}
            }
        }
        Ok(())
    }

    fn validate_transaction_at_entrypoint_index_and_routing(
        &mut self,
        tx: AcceptedTransaction<'_>,
        ivm_cache: &mut IvmCache,
        entrypoint_index: Option<u64>,
        routing_decision: Option<crate::queue::RoutingDecision>,
    ) -> (HashOf<TransactionEntrypoint>, TransactionResultInner) {
        // Capture gas accounting inputs up front to avoid borrowing conflicts
        let gas_total_before = self.gas_used_in_block;
        let gas_limit = self.gas_limit_per_block;
        let ops_total_before = self.zk_confidential_ops_in_block;
        let verify_calls_before = self.zk_verify_calls_in_block;
        let proof_bytes_before = self.zk_proof_bytes_in_block;
        let conf_gas_before = self.confidential_gas_used_in_block;
        let mut state_transaction = self.transaction();
        state_transaction.current_entrypoint_index = entrypoint_index;
        if let Some(routing) = routing_decision {
            state_transaction.current_lane_id = Some(routing.lane_id);
            state_transaction.current_dataspace_id = Some(routing.dataspace_id);
            state_transaction.world.current_dataspace_id = Some(routing.dataspace_id);
        }
        let hash = tx.hash_as_entrypoint();
        let result = Self::validate_transaction_internal(
            tx,
            &mut state_transaction,
            ivm_cache,
            routing_decision,
        );
        if result.is_ok() {
            // Enforce block gas limit if configured; accumulate gas used by last tx (IVM path)
            let used = state_transaction.last_tx_gas_used;
            // Compute new total without touching `self` while `state_transaction` borrows it
            let new_total = gas_total_before.saturating_add(used);
            if used > 0 && new_total > gas_limit {
                return (
                    hash,
                    Err(TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted(format!(
                            "block gas limit exceeded: {new_total} > {gas_limit}"
                        )),
                    )),
                );
            }
            let tx_ops = state_transaction.zk_confidential_ops_in_tx;
            let tx_verify_calls = state_transaction.zk_verify_calls_in_tx;
            let tx_proof_bytes = state_transaction.zk_proof_bytes_in_tx;
            let tx_conf_gas = state_transaction.confidential_gas_used_in_tx;
            let new_ops_total = ops_total_before.saturating_add(tx_ops);
            let new_verify_total = verify_calls_before.saturating_add(tx_verify_calls);
            let new_proof_bytes_total = proof_bytes_before.saturating_add(tx_proof_bytes);
            let new_conf_total = conf_gas_before.saturating_add(tx_conf_gas);
            // Apply staged changes first, then update gas accounting after borrow ends
            state_transaction.apply();
            if used > 0 {
                self.gas_used_in_block = new_total;
            }
            if tx_ops > 0 {
                self.zk_confidential_ops_in_block = new_ops_total;
            }
            if tx_conf_gas > 0 {
                self.confidential_gas_used_in_block = new_conf_total;
            }
            if tx_verify_calls > 0 {
                self.zk_verify_calls_in_block = new_verify_total;
            }
            if tx_proof_bytes > 0 {
                self.zk_proof_bytes_in_block = new_proof_bytes_total;
            }
        }

        (hash, result)
    }

    /// Validate the transaction, staging its state changes.
    ///
    /// Returns the trigger sequence on success, or the rejection reason on failure.
    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    fn validate_transaction_internal(
        tx: AcceptedTransaction<'_>,
        state_transaction: &mut StateTransaction<'_, '_>,
        ivm_cache: &mut IvmCache,
        routing_decision: Option<crate::queue::RoutingDecision>,
    ) -> TransactionResultInner {
        if let TransactionEntrypoint::PrivateKaigi(private_tx) = tx.entrypoint() {
            return Self::validate_private_kaigi_transaction(private_tx, state_transaction);
        }
        if let TransactionEntrypoint::SealedCommitment(commitment) = tx.entrypoint() {
            return Self::validate_sealed_transaction_commitment(commitment, state_transaction);
        }
        if let TransactionEntrypoint::SealedReveal(reveal) = tx.entrypoint() {
            return Self::validate_sealed_transaction_reveal(reveal, state_transaction, ivm_cache);
        }
        if matches!(tx.entrypoint(), TransactionEntrypoint::Time(_)) {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "time entrypoints cannot be executed via transaction admission".into(),
                ),
            ));
        }

        let admission =
            Self::validate_stateful_admission(tx.as_ref(), state_transaction, routing_decision)?;
        let authority = admission.authority;
        let allow_unregistered_authority = admission.allow_unregistered_authority;

        match tx.as_ref().instructions() {
            Executable::ContractCall(call) => {
                if crate::executor::transaction_gas_limit(tx.as_ref()).is_none() {
                    return Err(TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted(
                            "missing gas limit in fee payment intent".to_owned(),
                        ),
                    ));
                }
                let record =
                    code::fetch_bound_contract_record(state_transaction, &call.contract_address)
                        .ok_or_else(|| {
                            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                                format!(
                                    "contract instance `{}` not found in WSV",
                                    call.contract_address
                                ),
                            ))
                        })?;
                crate::executor::ensure_contract_invocation_code_hash(call, record.code_hash)
                    .map_err(TransactionRejectionReason::Validation)?;
                let contract_address = Some(call.contract_address.clone());
                Self::validate_ivm(
                    authority.clone(),
                    state_transaction,
                    IvmBytecode::from_compiled(record.code_bytes),
                    None,
                    contract_address,
                    ivm_cache,
                )?;
            }
            Executable::Ivm(bytes) => {
                if crate::executor::transaction_gas_limit(tx.as_ref()).is_none() {
                    return Err(TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted(
                            "missing gas limit in fee payment intent".to_owned(),
                        ),
                    ));
                }
                Self::validate_ivm(
                    authority.clone(),
                    state_transaction,
                    bytes.clone(),
                    Some(tx.as_ref().metadata()),
                    None,
                    ivm_cache,
                )?;
            }
            _ => {}
        }

        debug!(tx=%tx.hash(), "Validating transaction");
        Self::validate_transaction_with_runtime_executor(tx.clone(), state_transaction, ivm_cache)?;
        let trigger_sequence = if allow_unregistered_authority {
            debug!(
                authority = %authority,
                "transaction authority is not materialized; skipping data trigger dispatch"
            );
            DataTriggerSequence::default()
        } else {
            debug!("Transaction validated successfully; processing data triggers");
            let trigger_sequence = state_transaction.execute_data_triggers_dfs(&authority)?;
            debug!("Data triggers executed successfully");
            trigger_sequence
        };

        crate::validation_fee::commit_validation_fee_credit(
            state_transaction,
            admission.validation_fee_credit.as_ref(),
        )?;

        if let Some(seq) = admission.sequence_to_commit {
            state_transaction
                .world
                .tx_sequences
                .insert(authority.clone(), seq);
        }

        Ok(trigger_sequence)
    }

    fn validate_private_kaigi_transaction(
        tx: &PrivateKaigiTransaction,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> TransactionResultInner {
        state_transaction.tx_call_hash = Some(tx.action_hash());
        AcceptedTransaction::execute_private_kaigi_fee_spend(tx, state_transaction)?;
        state_transaction.last_tx_gas_used =
            AcceptedTransaction::private_kaigi_instruction_gas(tx)?;
        crate::smartcontracts::isi::kaigi::execute_private_transaction(tx, state_transaction)
            .map_err(|error| {
                TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(error))
            })?;
        Ok(DataTriggerSequence::default())
    }

    fn validate_sealed_transaction_commitment(
        commitment: &SignedSealedTransactionCommitment,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> TransactionResultInner {
        validate_sealed_commitment_stateless(
            commitment,
            &state_transaction.chain_id,
            state_transaction.world.parameters().transaction(),
        )
        .map_err(|err| match err {
            AcceptTransactionFail::ChainIdMismatch(mismatch) => {
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
                    "chain id mismatch: expected {} got {}",
                    mismatch.expected, mismatch.actual
                )))
            }
            AcceptTransactionFail::TransactionLimit(limit) => {
                TransactionRejectionReason::LimitCheck(limit)
            }
            other => TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                other.to_string(),
            )),
        })?;

        let payload = commitment.payload();
        let height = state_transaction._curr_block.height().get();
        if payload.reveal_after_height <= height {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(format!(
                    "sealed transaction reveal_after_height {} must be greater than commit height {height}",
                    payload.reveal_after_height
                )),
            ));
        }
        let key = sealed_commitment_state_key(&payload.commitment);
        if state_transaction
            .world
            .smart_contract_state
            .get(&key)
            .is_some()
        {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("sealed transaction commitment already exists".into()),
            ));
        }
        let record = PendingSealedTransactionCommitment {
            payload: payload.clone(),
            commit_height: height,
            commit_index: state_transaction.current_entrypoint_index.unwrap_or(0),
        };
        let bytes = norito::to_bytes(&record).map_err(sealed_state_encode_error)?;
        state_transaction
            .world
            .smart_contract_state
            .insert(key, bytes);
        Ok(DataTriggerSequence::default())
    }

    fn validate_sealed_transaction_reveal(
        reveal: &SealedTransactionReveal,
        state_transaction: &mut StateTransaction<'_, '_>,
        ivm_cache: &mut IvmCache,
    ) -> TransactionResultInner {
        let key = sealed_commitment_state_key(&reveal.commitment);
        let Some(bytes) = state_transaction.world.smart_contract_state.get(&key) else {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("sealed transaction commitment is not pending".into()),
            ));
        };
        let record: PendingSealedTransactionCommitment =
            norito::decode_from_bytes(bytes).map_err(sealed_state_decode_error)?;
        let height = state_transaction._curr_block.height().get();
        if height < record.payload.reveal_after_height {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(format!(
                    "sealed transaction reveal is too early: height {height}, reveal_after_height {}",
                    record.payload.reveal_after_height
                )),
            ));
        }
        if height > record.payload.reveal_deadline_height {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(format!(
                    "sealed transaction reveal deadline {} passed at height {height}",
                    record.payload.reveal_deadline_height
                )),
            ));
        }
        let signed = reveal.signed_transaction();
        if signed.chain() != &record.payload.chain_id {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(format!(
                    "sealed transaction chain mismatch: commitment chain {} reveal chain {}",
                    record.payload.chain_id,
                    signed.chain()
                )),
            ));
        }
        if signed.authority() != &record.payload.authority {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "sealed transaction reveal authority does not match commitment authority"
                        .into(),
                ),
            ));
        }
        let expected = compute_sealed_transaction_commitment(
            &record.payload.chain_id,
            signed,
            reveal.salt,
            record.payload.reveal_deadline_height,
        );
        if expected != record.payload.commitment || expected != reveal.commitment {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "sealed transaction reveal does not match commitment".into(),
                ),
            ));
        }

        state_transaction.world.smart_contract_state.remove(key);
        let accepted = AcceptedTransaction::new_unchecked(Cow::Borrowed(signed));
        Self::validate_transaction_internal(accepted, state_transaction, ivm_cache, None)
    }

    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    fn validate_ivm(
        authority: AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
        contract: IvmBytecode,
        transaction_metadata: Option<&Metadata>,
        deploy_target: Option<iroha_data_model::smart_contract::ContractAddress>,
        ivm_cache: &mut IvmCache,
    ) -> Result<(), TransactionRejectionReason> {
        // Parse and cache metadata + derived hashes.
        let bytes = contract.as_ref();
        let summary = ivm_cache.summarize_executable(bytes).map_err(|error| {
            let failure = match error {
                ivm::VMError::UnknownSyscall(number) => ValidationFail::NotPermitted(format!(
                    "unknown syscall number 0x{number:02x} for abi_version 1"
                )),
                error => ValidationFail::IvmAdmission(
                    crate::smartcontracts::ivm::admission_reason_from_vm_error(error),
                ),
            };
            TransactionRejectionReason::Validation(failure)
        })?;
        let is_contract = matches!(
            &summary,
            crate::smartcontracts::ivm::cache::ExecutableProgramSummary::Contract(_)
        );
        if !is_contract {
            if let Some(metadata) = transaction_metadata {
                crate::smartcontracts::ivm::validate_generic_execution_metadata(metadata)
                    .map_err(TransactionRejectionReason::Validation)?;
            }
        }
        let manifest_metadata = transaction_metadata
            .and_then(|metadata| metadata.get(&*CONTRACT_MANIFEST_METADATA_NAME))
            .map(|json| json.clone().try_into_any_norito::<ContractManifest>())
            .transpose()
            .map_err(|_| {
                TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                    iroha_data_model::executor::IvmAdmissionError::ManifestMalformed,
                ))
            })?;
        let metadata_deploy_target = transaction_metadata
            .and_then(|metadata| metadata.get(&*GOV_CONTRACT_ADDRESS_METADATA_KEY))
            .map(|json| {
                json.clone()
                    .try_into_any_norito::<String>()
                    .map_err(|_| ())
                    .and_then(|raw| raw.parse().map_err(|_| ()))
            })
            .transpose()
            .map_err(|()| {
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                    "invalid gov_contract_address metadata".to_owned(),
                ))
            })?;
        let deploy_target = deploy_target.or(metadata_deploy_target);
        let meta = summary.metadata().clone();
        let offset = summary.code_offset();
        // Use the domain-separated full-artifact hash and canonical ABI hash.
        let code_hash = summary.code_hash();
        let abi_hash = summary.abi_hash();

        crate::pipeline::overlay::validate_header_policy(&meta).map_err(|error| {
            TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(error))
        })?;

        // Runtime upgrade admission: if there is an activated runtime upgrade record for this ABI
        // version, require that the computed ABI hash matches the active manifest.
        //
        // This is redundant under the v1-only policy (all valid manifests must match ABI v1),
        // but guards against tampered WSV and keeps admission deterministic across nodes.
        {
            let current_height = state_transaction._curr_block.height().get();
            if let Some(expected) = crate::smartcontracts::ivm::active_runtime_abi_hash(
                &state_transaction.world,
                current_height,
            )
            .map_err(|error| {
                TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(error))
            })? {
                if expected != abi_hash {
                    return Err(TransactionRejectionReason::Validation(
                        ValidationFail::IvmAdmission(
                            iroha_data_model::executor::IvmAdmissionError::ManifestAbiHashMismatch(
                                iroha_data_model::executor::ManifestAbiHashMismatchInfo {
                                    expected,
                                    actual: abi_hash,
                                },
                            ),
                        ),
                    ));
                }
            }
        }

        // Fuel (`max_cycles`) must be explicitly provided and bounded by both
        // governance and the local deterministic execution ceiling.
        let upper_bound = state_transaction.pipeline.ivm_max_cycles_upper_bound;
        let params = state_transaction.world.parameters.get();
        crate::smartcontracts::ivm::validate_cycle_limits(
            &meta,
            upper_bound,
            params.smart_contract().fuel(),
        )
        .map_err(|error| {
            TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(error))
        })?;

        let code = &bytes[offset..];
        let decoded = if code.is_empty() {
            None
        } else {
            Some(
                ivm::ivm_cache::global_get_with_meta(code, &meta).map_err(|err| {
                    TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                        iroha_data_model::executor::IvmAdmissionError::BytecodeDecodingFailed(
                            err.to_string(),
                        ),
                    ))
                })?,
            )
        };

        let inst_cap = state_transaction.pipeline.ivm_max_decoded_instructions;
        let bytes_cap = state_transaction.pipeline.ivm_max_decoded_bytes;
        if let Some(decoded) = decoded.as_ref() {
            if inst_cap != 0 {
                let decoded_instr = u64::try_from(decoded.len()).unwrap_or(u64::MAX);
                if decoded_instr > inst_cap {
                    return Err(TransactionRejectionReason::Validation(
                        ValidationFail::IvmAdmission(
                            iroha_data_model::executor::IvmAdmissionError::DecodedInstructionCountExceeded(
                                iroha_data_model::executor::DecodedInstructionLimitInfo {
                                    decoded_instructions: decoded_instr,
                                    limit: inst_cap,
                                },
                            ),
                        ),
                    ));
                }
            }

            if bytes_cap != 0 {
                let decoded_bytes = decoded
                    .iter()
                    .try_fold(0u64, |acc, op| acc.checked_add(u64::from(op.len)))
                    .unwrap_or(u64::MAX);
                if decoded_bytes > bytes_cap {
                    return Err(TransactionRejectionReason::Validation(
                        ValidationFail::IvmAdmission(
                            iroha_data_model::executor::IvmAdmissionError::DecodedCodeSizeExceeded(
                                iroha_data_model::executor::DecodedCodeSizeLimitInfo {
                                    decoded_bytes,
                                    limit: bytes_cap,
                                },
                            ),
                        ),
                    ));
                }
            }
        }

        // Admission guard: reject bytecode that invokes syscalls outside the ABI surface.
        if let Some(decoded) = decoded.as_ref() {
            debug_assert_eq!(meta.abi_version, 1, "only ABI v1 is supported");
            let policy = ivm::SyscallPolicy::AbiV1;
            for op in decoded.iter() {
                let opcode = ivm::instruction::wide::opcode(op.inst);
                let number = if opcode == ivm::instruction::wide::system::SCALL {
                    // SCALL immediate is an unsigned byte; reinterpret negative imm8 as its
                    // 8-bit two's complement value to mirror VM execution semantics.
                    Some(u32::from(
                        ivm::instruction::wide::imm8(op.inst).to_ne_bytes()[0],
                    ))
                } else if opcode == ivm::instruction::wide::system::SYSTEM {
                    Some(ivm::encoding::wide::decode_syscallx(op.inst))
                } else {
                    None
                };
                if let Some(number) = number
                    && !ivm::syscalls::is_syscall_allowed(policy, number)
                {
                    return Err(TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted(format!(
                            "unknown syscall number 0x{number:02x} for abi_version {}",
                            meta.abi_version
                        )),
                    ));
                }
            }
        }

        // Validate every supplied or stored manifest as a complete V1
        // consensus binding. A present manifest may not omit either hash.
        let validate_manifest =
            |manifest: &ContractManifest| -> Result<(), TransactionRejectionReason> {
                crate::smartcontracts::ivm::validate_manifest_hashes(manifest, code_hash, abi_hash)
                    .map_err(|error| {
                        TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(error))
                    })
            };

        if is_contract {
            if let Some(manifest) = manifest_metadata.as_ref() {
                validate_manifest(manifest)?;
            }
            if let Some(manifest) = state_transaction.world.contract_manifests.get(&code_hash) {
                validate_manifest(manifest)?;
            }
        } else if manifest_metadata.is_some()
            || state_transaction
                .world
                .contract_manifests
                .get(&code_hash)
                .is_some()
            || deploy_target.is_some()
        {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::IvmAdmission(
                    iroha_data_model::executor::IvmAdmissionError::ManifestMalformed,
                ),
            ));
        }

        // Protected namespaces admission (governance gating)
        if let Some(contract_address) = deploy_target {
            // Read protected namespaces from on-chain custom parameter `gov_protected_namespaces`
            let mut protected: Vec<String> = Vec::new();
            if let Ok(name) = core::str::FromStr::from_str("gov_protected_namespaces") {
                let id = iroha_data_model::parameter::CustomParameterId(name);
                let params = state_transaction.world.parameters.get();
                if let Some(custom) = params.custom().get(&id)
                    && let Ok(v) = custom.payload().try_into_any_norito::<Vec<String>>()
                {
                    protected = v;
                }
            }
            if !protected.is_empty() {
                // Require an enacted proposal matching the governed contract address and hashes.
                let want_code = hex::encode(<[u8; 32]>::from(code_hash));
                let want_abi = hex::encode(<[u8; 32]>::from(abi_hash));
                let mut ok = false;
                for (_pid, rec) in state_transaction.world.governance_proposals.iter() {
                    let Some(payload) = rec.as_deploy_contract() else {
                        continue;
                    };
                    if payload.contract_address == contract_address
                        && payload.code_hash_hex.to_hex() == want_code
                        && payload.abi_hash_hex.to_hex() == want_abi
                        && matches!(rec.status, crate::state::GovernanceProposalStatus::Enacted)
                    {
                        ok = true;
                        break;
                    }
                }
                if !ok {
                    #[cfg(feature = "telemetry")]
                    state_transaction
                        .telemetry
                        .record_protected_namespace_enforcement("rejected");
                    return Err(TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted(
                            "deployment into governed contract address requires enacted governance proposal"
                                .to_owned(),
                        ),
                    ));
                }
                #[cfg(feature = "telemetry")]
                state_transaction
                    .telemetry
                    .record_protected_namespace_enforcement("allowed");
            }
        }

        let _ = authority; // reserved for future context-dependent checks

        Ok(())
    }

    /// Validate transaction with runtime executors.
    ///
    /// Note: transaction instructions will be executed on the given `state_transaction`.
    fn validate_transaction_with_runtime_executor(
        tx: AcceptedTransaction<'_>,
        state_transaction: &mut StateTransaction<'_, '_>,
        ivm_cache: &mut IvmCache,
    ) -> Result<(), TransactionRejectionReason> {
        let tx: SignedTransaction = tx.into();
        let authority = tx.authority().clone();

        state_transaction
            .world
            .executor
            .clone()
            .execute_transaction(state_transaction, &authority, tx, ivm_cache)
            .map_err(|error| {
                if let ValidationFail::InternalError(msg) = &error {
                    error!(
                        error = msg,
                        "Internal error occurred during transaction validation, \
                         is Runtime Executor correct?"
                    )
                }
                error.into()
            })
    }
}

#[cfg(feature = "telemetry")]
static FRAUD_ASSESSMENT_TENANT_KEY: LazyLock<TelemetryName> = LazyLock::new(|| {
    TelemetryName::from_str("fraud_assessment_tenant").expect("static tenant metadata key")
});
#[cfg(feature = "telemetry")]
static FRAUD_ASSESSMENT_SCORE_KEY: LazyLock<TelemetryName> = LazyLock::new(|| {
    TelemetryName::from_str("fraud_assessment_score_bps").expect("static score metadata key")
});
#[cfg(feature = "telemetry")]
static FRAUD_ASSESSMENT_LATENCY_KEY: LazyLock<TelemetryName> = LazyLock::new(|| {
    TelemetryName::from_str("fraud_assessment_latency_ms").expect("static latency metadata key")
});
#[cfg(feature = "telemetry")]
static FRAUD_ASSESSMENT_DISPOSITION_KEY: LazyLock<TelemetryName> = LazyLock::new(|| {
    TelemetryName::from_str("fraud_assessment_disposition")
        .expect("static disposition metadata key")
});

#[cfg(feature = "telemetry")]
#[derive(Clone, Copy)]
enum FraudDisposition {
    Fraud,
    Clean,
}

/// Dataspace routing details needed when enforcing the fraud policy.
pub(crate) struct LaneAssignment<'cfg> {
    /// Lane identifier selected by routing policy.
    pub(crate) lane_id: NexusLaneId,
    /// Dataspace identifier associated with the lane.
    pub(crate) dataspace_id: NexusDataSpaceId,
    /// Catalog used to resolve dataspace metadata.
    pub(crate) dataspace_catalog: &'cfg DataSpaceCatalog,
}

impl LaneAssignment<'_> {
    fn dataspace_label(&self) -> String {
        dataspace_label_from_catalog(self.dataspace_catalog, self.dataspace_id)
    }
}

fn dataspace_label_from_catalog(catalog: &DataSpaceCatalog, id: NexusDataSpaceId) -> String {
    catalog
        .entries()
        .iter()
        .find(|entry| entry.id == id)
        .map_or_else(|| id.as_u64().to_string(), |entry| entry.alias.clone())
}

fn reject_not_permitted(reason: impl Into<String>) -> TransactionRejectionReason {
    TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason.into()))
}

fn reject_lane_policy(alias: &str, reason: impl Into<String>) -> TransactionRejectionReason {
    reject_not_permitted(format!("lane {alias}: {}", reason.into()))
}

fn collect_lane_privacy_proofs(
    tx: &SignedTransaction,
) -> Vec<iroha_data_model::nexus::LanePrivacyProof> {
    tx.attachments()
        .into_iter()
        .flat_map(|list| list.as_slice().iter())
        .filter_map(|attachment| attachment.lane_privacy.clone())
        .collect()
}

fn enforce_manifest_quorum(
    alias: &str,
    rules: &GovernanceRules,
    tx: &SignedTransaction,
) -> Result<(), TransactionRejectionReason> {
    if let Executable::Instructions(instructions) = tx.instructions()
        && instructions_allow_multisig_envelope_authority(instructions)
    {
        return Ok(());
    }
    let Some(quorum) = rules.quorum else {
        return Ok(());
    };
    if quorum <= 1 {
        return Ok(());
    }
    if rules.validators.is_empty() {
        return Ok(());
    }

    let approvals = collect_manifest_approvals(alias, tx)?;
    let validators = canonical_manifest_validators(alias, rules)?;
    let approved = approvals
        .iter()
        .filter(|account| validators.contains(*account))
        .count();
    let required = usize::try_from(quorum).unwrap_or(usize::MAX);
    if approved < required {
        return Err(reject_lane_policy(
            alias,
            format!(
                "lane manifest quorum requires {quorum} validator approvals but {approved} were provided"
            ),
        ));
    }
    Ok(())
}

fn collect_manifest_approvals(
    alias: &str,
    tx: &SignedTransaction,
) -> Result<BTreeSet<String>, TransactionRejectionReason> {
    let mut approvals = BTreeSet::new();
    let authority = tx.authority();
    let authority_i105 = authority.canonical_i105().map_err(|err| {
        reject_lane_policy(
            alias,
            format!("failed to encode authority `{authority}` as i105: {err}"),
        )
    })?;
    approvals.insert(authority_i105);

    let metadata = tx.metadata();
    let Some(raw) = metadata.get(&*GOV_APPROVERS_METADATA_KEY) else {
        return Ok(approvals);
    };
    let entries = raw.try_into_any_norito::<Vec<String>>().map_err(|_| {
        reject_lane_policy(
            alias,
            "`gov_manifest_approvers` metadata must be an array of account identifiers",
        )
    })?;
    for entry in entries {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return Err(reject_lane_policy(
                alias,
                "`gov_manifest_approvers` metadata entries must not be blank",
            ));
        }
        let canonical = AccountId::canonicalize(trimmed).map_err(|err| {
            reject_lane_policy(
                alias,
                format!("invalid account id `{trimmed}` in `gov_manifest_approvers`: {err}"),
            )
        })?;
        if !approvals.insert(canonical) {
            return Err(reject_lane_policy(
                alias,
                "`gov_manifest_approvers` metadata must not duplicate approvers",
            ));
        }
    }
    Ok(approvals)
}

fn canonical_manifest_validators(
    alias: &str,
    rules: &GovernanceRules,
) -> Result<BTreeSet<String>, TransactionRejectionReason> {
    let mut validators = BTreeSet::new();
    for validator in &rules.validators {
        let i105 = validator.canonical_i105().map_err(|err| {
            reject_lane_policy(
                alias,
                format!("failed to encode validator `{validator}` as i105: {err}"),
            )
        })?;
        if !validators.insert(i105) {
            return Err(reject_lane_policy(
                alias,
                "lane manifest validator set contains duplicate validators",
            ));
        }
    }
    Ok(validators)
}

fn tx_contains_runtime_upgrade_instruction(tx: &SignedTransaction) -> bool {
    let contains_runtime_upgrade = |instruction: &InstructionBox| {
        instruction
            .as_any()
            .downcast_ref::<ProposeRuntimeUpgrade>()
            .is_some()
            || instruction
                .as_any()
                .downcast_ref::<ActivateRuntimeUpgrade>()
                .is_some()
            || instruction
                .as_any()
                .downcast_ref::<CancelRuntimeUpgrade>()
                .is_some()
    };
    match tx.instructions() {
        Executable::Instructions(instructions) => instructions.iter().any(contains_runtime_upgrade),
        Executable::Batch(items) => items.iter().any(|item| match item {
            ExecutableBatchItem::Instruction(instruction) => contains_runtime_upgrade(instruction),
            ExecutableBatchItem::ContractCall(_) => false,
        }),
        Executable::ContractCall(_) | Executable::Ivm(_) | Executable::IvmProved(_) => false,
    }
}

fn tx_touches_manifest_protected_namespace_surface(tx: &SignedTransaction) -> bool {
    let metadata = tx.metadata();
    let has_governance_contract_address =
        metadata.get(&*GOV_CONTRACT_ADDRESS_METADATA_KEY).is_some();
    let has_contract_address_hint = metadata.get(&*CONTRACT_ADDRESS_METADATA_KEY).is_some();

    let mut contract_targets_seen = false;
    let mut register_code_seen = false;
    let inspect_instruction = |instruction: &InstructionBox| {
        let is_contract_target = instruction
            .as_any()
            .downcast_ref::<ActivateContractInstance>()
            .is_some()
            || instruction
                .as_any()
                .downcast_ref::<CommitContractDeployment>()
                .is_some()
            || instruction
                .as_any()
                .downcast_ref::<DeactivateContractInstance>()
                .is_some();
        let is_code_registration = if is_contract_target {
            false
        } else {
            let any = instruction.as_any();
            any.is::<RegisterSmartContractCode>()
                || any.is::<RegisterSmartContractBytes>()
                || any.is::<UploadSmartContractCodeChunk>()
                || any.is::<FinalizeSmartContractCodeUpload>()
                || any.is::<RemoveSmartContractBytes>()
        };
        (is_contract_target, is_code_registration)
    };
    match tx.instructions() {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                let (target, registration) = inspect_instruction(instruction);
                contract_targets_seen |= target;
                register_code_seen |= registration;
            }
        }
        Executable::ContractCall(_) => {
            contract_targets_seen = true;
        }
        Executable::Batch(items) => {
            for item in items {
                match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        let (target, registration) = inspect_instruction(instruction);
                        contract_targets_seen |= target;
                        register_code_seen |= registration;
                    }
                    ExecutableBatchItem::ContractCall(_) => contract_targets_seen = true,
                }
            }
        }
        Executable::Ivm(_) | Executable::IvmProved(_) => {}
    }

    let ivm_with_contract_metadata = matches!(tx.instructions(), Executable::Ivm(_))
        && (has_governance_contract_address || has_contract_address_hint);

    register_code_seen || contract_targets_seen || ivm_with_contract_metadata
}

fn tx_requires_manifest_validator_gating(rules: &GovernanceRules, tx: &SignedTransaction) -> bool {
    tx_contains_runtime_upgrade_instruction(tx)
        || (!rules.protected_namespaces.is_empty()
            && tx_touches_manifest_protected_namespace_surface(tx))
}

#[allow(clippy::too_many_lines)]
fn enforce_manifest_protected_namespaces(
    alias: &str,
    rules: &GovernanceRules,
    tx: &SignedTransaction,
    world: &impl WorldReadOnly,
) -> Result<(), TransactionRejectionReason> {
    if rules.protected_namespaces.is_empty() {
        return Ok(());
    }

    let metadata = tx.metadata();
    let metadata_governance_contract_address = metadata
        .get(&*GOV_CONTRACT_ADDRESS_METADATA_KEY)
        .map(|value| {
            let raw = value.try_into_any_norito::<String>().map_err(|_| {
                reject_lane_policy(
                    alias,
                    "`gov_contract_address` metadata must be a string value",
                )
            })?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(reject_lane_policy(
                    alias,
                    "`gov_contract_address` metadata must not be blank",
                ));
            }
            trimmed.parse::<iroha_data_model::smart_contract::ContractAddress>().map_err(|err| {
                reject_lane_policy(
                    alias,
                    format!(
                        "`gov_contract_address` metadata `{trimmed}` is not a valid ContractAddress: {err}"
                    ),
                )
            })
        })
        .transpose()?;

    let metadata_contract_address_hint = metadata
        .get(&*CONTRACT_ADDRESS_METADATA_KEY)
        .map(|value| {
            let raw = value.try_into_any_norito::<String>().map_err(|_| {
                reject_lane_policy(alias, "`contract_address` metadata must be a string value")
            })?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(reject_lane_policy(
                    alias,
                    "`contract_address` metadata must not be blank",
                ));
            }
            trimmed.parse::<iroha_data_model::smart_contract::ContractAddress>().map_err(|err| {
                reject_lane_policy(
                    alias,
                    format!(
                        "`contract_address` metadata `{trimmed}` is not a valid ContractAddress: {err}"
                    ),
                )
            })
        })
        .transpose()?;

    let mut contract_targets = BTreeSet::new();
    let mut register_code_seen = false;
    let mut commit_deployment_count = 0_usize;
    let mut activate_seen = false;
    let mut deactivate_seen = false;
    match tx.instructions() {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                if let Some(commit) = instruction
                    .as_any()
                    .downcast_ref::<CommitContractDeployment>()
                {
                    commit_deployment_count += 1;
                    contract_targets.insert(commit.contract_address().clone());
                } else if let Some(activate) = instruction
                    .as_any()
                    .downcast_ref::<ActivateContractInstance>()
                {
                    activate_seen = true;
                    contract_targets.insert(activate.contract_address().clone());
                } else if let Some(deactivate) = instruction
                    .as_any()
                    .downcast_ref::<DeactivateContractInstance>()
                {
                    deactivate_seen = true;
                    contract_targets.insert(deactivate.contract_address().clone());
                } else {
                    let modifies_contract_code = {
                        let any = instruction.as_any();
                        any.is::<RegisterSmartContractCode>()
                            || any.is::<RegisterSmartContractBytes>()
                            || any.is::<UploadSmartContractCodeChunk>()
                            || any.is::<FinalizeSmartContractCodeUpload>()
                            || any.is::<RemoveSmartContractBytes>()
                    };
                    if modifies_contract_code {
                        register_code_seen = true;
                    }
                }
            }
        }
        Executable::ContractCall(call) => {
            contract_targets.insert(call.contract_address.clone());
        }
        Executable::Batch(items) => {
            for item in items {
                match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        if let Some(commit) = instruction
                            .as_any()
                            .downcast_ref::<CommitContractDeployment>()
                        {
                            commit_deployment_count += 1;
                            contract_targets.insert(commit.contract_address().clone());
                        } else if let Some(activate) = instruction
                            .as_any()
                            .downcast_ref::<ActivateContractInstance>()
                        {
                            activate_seen = true;
                            contract_targets.insert(activate.contract_address().clone());
                        } else if let Some(deactivate) = instruction
                            .as_any()
                            .downcast_ref::<DeactivateContractInstance>(
                        ) {
                            deactivate_seen = true;
                            contract_targets.insert(deactivate.contract_address().clone());
                        } else {
                            let any = instruction.as_any();
                            if any.is::<RegisterSmartContractCode>()
                                || any.is::<RegisterSmartContractBytes>()
                                || any.is::<UploadSmartContractCodeChunk>()
                                || any.is::<FinalizeSmartContractCodeUpload>()
                                || any.is::<RemoveSmartContractBytes>()
                            {
                                register_code_seen = true;
                            }
                        }
                    }
                    ExecutableBatchItem::ContractCall(call) => {
                        contract_targets.insert(call.contract_address.clone());
                    }
                }
            }
        }
        Executable::Ivm(_) | Executable::IvmProved(_) => {}
    }

    if commit_deployment_count > 1
        || (commit_deployment_count == 1 && (activate_seen || deactivate_seen))
        || (activate_seen && deactivate_seen)
    {
        return Err(reject_lane_policy(
            alias,
            "protected contract rotations must use exactly one `CommitContractDeployment` instruction and no legacy activate/deactivate pair",
        ));
    }

    if let Some(contract_address) = metadata_governance_contract_address.clone() {
        contract_targets.insert(contract_address);
    }

    let ivm_with_contract_metadata = matches!(tx.instructions(), Executable::Ivm(_))
        && (metadata_governance_contract_address.is_some()
            || metadata_contract_address_hint.is_some());

    let contract_instr_seen =
        register_code_seen || !contract_targets.is_empty() || ivm_with_contract_metadata;
    let explicit_contract_instruction_seen =
        register_code_seen || commit_deployment_count > 0 || activate_seen || deactivate_seen;
    let has_directly_addressed_call = match tx.instructions() {
        Executable::ContractCall(_) => true,
        Executable::Batch(items) => items
            .iter()
            .any(|item| matches!(item, ExecutableBatchItem::ContractCall(_))),
        Executable::Instructions(_) | Executable::Ivm(_) | Executable::IvmProved(_) => false,
    };

    if contract_instr_seen
        && metadata_governance_contract_address.is_none()
        && (!has_directly_addressed_call || explicit_contract_instruction_seen)
    {
        return Err(reject_lane_policy(
            alias,
            "transactions with contract operations must set `gov_contract_address` metadata when lane governance protects namespaces",
        ));
    }

    if let (Some(hint), Some(meta)) = (
        metadata_contract_address_hint.as_ref(),
        metadata_governance_contract_address.as_ref(),
    ) {
        if hint != meta {
            return Err(reject_lane_policy(
                alias,
                "`contract_address` metadata must match `gov_contract_address` for protected operations",
            ));
        }
    }

    if let Some(meta_contract_address) = metadata_governance_contract_address.as_ref()
        && !contract_targets.is_empty()
        && contract_targets
            .iter()
            .any(|contract_address| contract_address != meta_contract_address)
    {
        return Err(reject_lane_policy(
            alias,
            "`gov_contract_address` metadata does not match contract addresses referenced by contract instructions",
        ));
    }

    let _ = world;

    Ok(())
}

fn enforce_runtime_upgrade_hook(
    alias: &str,
    rules: &GovernanceRules,
    tx: &SignedTransaction,
) -> Result<bool, TransactionRejectionReason> {
    let contains_runtime_upgrade = contains_runtime_upgrade_instruction(tx);
    if !contains_runtime_upgrade {
        return Ok(false);
    }

    let Some(hook) = rules.hooks.runtime_upgrade.as_ref() else {
        return Ok(true);
    };

    if !hook.allow {
        return Err(reject_lane_policy(
            alias,
            "runtime upgrade hook prohibits runtime upgrade instructions".to_string(),
        ));
    }

    if hook.require_metadata || hook.allowed_ids.is_some() {
        let Some(key) = hook.metadata_key.as_ref() else {
            return Err(reject_lane_policy(
                alias,
                "runtime upgrade hook missing metadata_key despite requiring metadata".to_string(),
            ));
        };
        let metadata = tx.metadata();
        let Some(raw_value) = metadata.get(key) else {
            return Err(reject_lane_policy(
                alias,
                format!("runtime upgrade hook requires metadata `{}`", key.as_ref()),
            ));
        };
        let value = raw_value.try_into_any_norito::<String>().map_err(|_| {
            reject_lane_policy(
                alias,
                format!(
                    "runtime upgrade metadata `{}` must be a string",
                    key.as_ref()
                ),
            )
        })?;
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(reject_lane_policy(
                alias,
                format!(
                    "runtime upgrade metadata `{}` must not be blank",
                    key.as_ref()
                ),
            ));
        }
        if let Some(ids) = hook.allowed_ids.as_ref()
            && !ids.contains(trimmed)
        {
            return Err(reject_lane_policy(
                alias,
                format!(
                    "runtime upgrade metadata `{}` value `{trimmed}` not permitted by lane manifest",
                    key.as_ref()
                ),
            ));
        }
    }

    Ok(contains_runtime_upgrade)
}

fn contains_runtime_upgrade_instruction(tx: &SignedTransaction) -> bool {
    tx_contains_runtime_upgrade_instruction(tx)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeUpgradeModuleKind {
    Parliament,
    LocalAdmins,
}

fn resolve_runtime_upgrade_module_kind(
    module_name: Option<&str>,
    catalog: &iroha_config::parameters::actual::GovernanceCatalog,
) -> Option<RuntimeUpgradeModuleKind> {
    let configured = module_name.or(catalog.default_module.as_deref())?;
    let module_type = catalog
        .modules
        .get(configured)
        .and_then(|module| module.module_type.as_deref())
        .unwrap_or(configured);
    let normalized = module_type.trim().to_ascii_lowercase().replace('-', "_");
    if normalized.contains("parliament") {
        Some(RuntimeUpgradeModuleKind::Parliament)
    } else {
        Some(RuntimeUpgradeModuleKind::LocalAdmins)
    }
}

fn enforce_runtime_upgrade_dataspace_policy(
    alias: &str,
    dataspace_id: NexusDataSpaceId,
    module_name: Option<&str>,
    catalog: &iroha_config::parameters::actual::GovernanceCatalog,
) -> Result<(), TransactionRejectionReason> {
    let Some(module_kind) = resolve_runtime_upgrade_module_kind(module_name, catalog) else {
        return Err(reject_lane_policy(
            alias,
            "runtime upgrade policy requires a governance module".to_string(),
        ));
    };
    if dataspace_id == NexusDataSpaceId::UNIVERSAL
        && !matches!(module_kind, RuntimeUpgradeModuleKind::Parliament)
    {
        return Err(reject_lane_policy(
            alias,
            "runtime upgrades in universal dataspace require a parliament governance module"
                .to_string(),
        ));
    }
    Ok(())
}

fn extract_lane_identity_metadata(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    dataspace_id: NexusDataSpaceId,
    lane_alias: &str,
) -> Result<(Option<UniversalAccountId>, Vec<String>), TransactionRejectionReason> {
    extract_directory_lane_identity_metadata(world, authority, dataspace_id).map_err(
        |err| match err {
            LaneIdentityMetadataError::MissingDataspaceBinding { uaid, dataspace } => {
                reject_lane_policy(
                    lane_alias,
                    format!(
                        "UAID {uaid} is not bound to dataspace {}",
                        dataspace.as_u64()
                    ),
                )
            }
            LaneIdentityMetadataError::InactiveManifest { uaid, dataspace } => reject_lane_policy(
                lane_alias,
                format!(
                    "UAID {uaid} manifest for dataspace {} is not active",
                    dataspace.as_u64()
                ),
            ),
        },
    )
}

fn extract_lane_authority_domains(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    lane_alias: &str,
    now_ms: u64,
) -> Result<Vec<iroha_data_model::domain::DomainId>, TransactionRejectionReason> {
    extract_directory_authority_domains(world, authority, now_ms).map_err(|err| {
        reject_lane_policy(
            lane_alias,
            format!("authority alias domain resolution failed: {err}"),
        )
    })
}

fn publishes_only_space_directory_manifests(tx: &SignedTransaction) -> bool {
    let is_publish = |instruction: &InstructionBox| {
        instruction
            .as_any()
            .downcast_ref::<iroha_data_model::isi::space_directory::PublishSpaceDirectoryManifest>()
            .is_some()
    };
    match tx.instructions() {
        Executable::Instructions(instructions) if !instructions.is_empty() => {
            instructions.iter().all(is_publish)
        }
        Executable::Batch(items) if !items.is_empty() => items.iter().all(|item| match item {
            ExecutableBatchItem::Instruction(instruction) => is_publish(instruction),
            ExecutableBatchItem::ContractCall(_) => false,
        }),
        _ => false,
    }
}

fn enforce_lane_policies(
    tx: &SignedTransaction,
    state_transaction: &StateTransaction<'_, '_>,
    lane_assignment: &LaneAssignment<'_>,
) -> Result<(), TransactionRejectionReason> {
    let lane_id = lane_assignment.lane_id;
    let dataspace_id = lane_assignment.dataspace_id;
    let manifest_registry: &LaneManifestRegistryHandle = &state_transaction.lane_manifests;

    if let Err(err) = manifest_registry.ensure_lane_ready(lane_id) {
        return Err(reject_not_permitted(err.message()));
    }

    let manifest_status = manifest_registry.status(lane_id).cloned();
    let lane_alias = manifest_status.as_ref().map_or_else(
        || format!("lane-{}", lane_id.as_u32()),
        |status| status.alias.clone(),
    );
    let allows_multisig_envelope_authority = match tx.instructions() {
        Executable::Instructions(instructions) => {
            instructions_allow_multisig_envelope_authority(instructions)
        }
        Executable::ContractCall(_)
        | Executable::Batch(_)
        | Executable::IvmProved(_)
        | Executable::Ivm(_) => false,
    };

    let mut runtime_upgrade_present = false;
    if let Some(status) = manifest_status.as_ref() {
        if let Some(rules) = status.rules() {
            let governance_manifest = status.governance.is_some();
            let governance_sensitive =
                governance_manifest && tx_requires_manifest_validator_gating(rules, tx);
            if governance_sensitive {
                let manifest_validators = if rules.validators.is_empty() {
                    None
                } else {
                    Some(canonical_manifest_validators(&lane_alias, rules)?)
                };
                if let Some(validators) = manifest_validators.as_ref()
                    && !allows_multisig_envelope_authority
                {
                    let authority = tx.authority();
                    let authority_i105 = authority.canonical_i105().map_err(|err| {
                        reject_lane_policy(
                            &lane_alias,
                            format!("failed to encode authority `{authority}` as i105: {err}"),
                        )
                    })?;
                    if !validators.contains(&authority_i105) {
                        return Err(reject_lane_policy(
                            &lane_alias,
                            "authority not part of lane validator set".to_string(),
                        ));
                    }
                }

                let quorum_required = !allows_multisig_envelope_authority
                    && rules.quorum.unwrap_or(0).saturating_sub(1) > 0
                    && !rules.validators.is_empty();
                if quorum_required {
                    enforce_manifest_quorum(&lane_alias, rules, tx)?;
                }
            }

            if governance_manifest {
                enforce_manifest_protected_namespaces(
                    &lane_alias,
                    rules,
                    tx,
                    &state_transaction.world,
                )?;

                runtime_upgrade_present = enforce_runtime_upgrade_hook(&lane_alias, rules, tx)?;
            }
        }
    }

    if !runtime_upgrade_present {
        runtime_upgrade_present = contains_runtime_upgrade_instruction(tx);
    }
    if runtime_upgrade_present && state_transaction.nexus.enabled {
        let module_name = manifest_status
            .as_ref()
            .and_then(|status| status.governance.as_deref());
        enforce_runtime_upgrade_dataspace_policy(
            &lane_alias,
            dataspace_id,
            module_name,
            &state_transaction.nexus.governance,
        )?;
    }

    let privacy_proofs = collect_lane_privacy_proofs(tx);
    let verified_privacy_commitments = if privacy_proofs.is_empty() {
        BTreeSet::new()
    } else {
        verify_lane_privacy_proofs(
            state_transaction.lane_privacy_registry.as_ref(),
            lane_id,
            &privacy_proofs,
        )
        .map_err(|err| {
            reject_lane_policy(&lane_alias, format!("lane privacy proof rejected: {err}"))
        })?
    };

    let lane_privacy_registry = if state_transaction.lane_privacy_registry.is_empty() {
        None
    } else {
        Some(state_transaction.lane_privacy_registry.clone())
    };

    let publishes_space_directory_manifest = publishes_only_space_directory_manifests(tx);
    let lane_identity = if publishes_space_directory_manifest {
        (None, Vec::new())
    } else {
        extract_lane_identity_metadata(
            &state_transaction.world,
            tx.authority(),
            dataspace_id,
            &lane_alias,
        )?
    };

    if !publishes_space_directory_manifest
        && let Some(engine) = state_transaction.lane_compliance.as_ref()
    {
        let (uaid_value, capability_tags) = lane_identity;
        let authority_domains = extract_lane_authority_domains(
            &state_transaction.world,
            tx.authority(),
            &lane_alias,
            state_transaction.block_unix_timestamp_ms(),
        )?;
        let ctx = LaneComplianceContext {
            lane_id,
            dataspace_id,
            authority: tx.authority(),
            authority_domains: authority_domains.as_slice(),
            uaid: uaid_value.as_ref(),
            capability_tags: capability_tags.as_slice(),
            lane_privacy_registry,
            verified_privacy_commitments: &verified_privacy_commitments,
        };
        let evaluation = engine.evaluate(&ctx);
        match evaluation {
            LaneComplianceEvaluation::NotConfigured => {
                if !engine.audit_only() {
                    return Err(reject_lane_policy(
                        &lane_alias,
                        "no exact lane compliance policy is configured".to_string(),
                    ));
                }
            }
            LaneComplianceEvaluation::Allowed(record) => {
                record.log(engine.audit_only());
            }
            LaneComplianceEvaluation::Denied(record) => {
                record.log(engine.audit_only());
                if !engine.audit_only() {
                    let reason = record
                        .reason
                        .clone()
                        .unwrap_or_else(|| "lane compliance policy denied".to_string());
                    return Err(reject_lane_policy(&lane_alias, reason));
                }
            }
        }
    }

    Ok(())
}

#[cfg(feature = "telemetry")]
fn tenant_label_from(raw: &str) -> &str {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "unknown"
    } else {
        trimmed
    }
}

#[cfg(feature = "telemetry")]
struct FraudTelemetryContext<'a> {
    telemetry: &'a StateTelemetry,
    lane_id: NexusLaneId,
    dataspace_id: NexusDataSpaceId,
    dataspace_label: String,
    tenant: String,
    score_bps: Option<u16>,
    latency_ms: Option<u64>,
    disposition: Option<FraudDisposition>,
}

#[cfg(feature = "telemetry")]
impl<'a> FraudTelemetryContext<'a> {
    fn prepare(
        telemetry: Option<&'a StateTelemetry>,
        routing: &LaneAssignment<'_>,
        metadata: &TelemetryMetadata,
    ) -> Option<Self> {
        let telemetry = telemetry?;
        if !telemetry.is_enabled() {
            return None;
        }
        Some(Self::new(telemetry, routing, metadata))
    }

    #[allow(clippy::too_many_lines)]
    fn new(
        telemetry: &'a StateTelemetry,
        routing: &LaneAssignment<'_>,
        metadata: &TelemetryMetadata,
    ) -> Self {
        let dataspace_label = routing.dataspace_label();
        let lane_id = routing.lane_id;
        let dataspace_id = routing.dataspace_id;
        let tenant = metadata
            .get(FRAUD_ASSESSMENT_TENANT_KEY.as_ref())
            .map_or_else(
                || "unknown".to_string(),
                |value| {
                    value.try_into_any_norito::<String>().map_or_else(
                        |_| {
                            telemetry.record_fraud_invalid_metadata(
                                lane_id,
                                dataspace_id,
                                dataspace_label.as_str(),
                                "unknown",
                                "tenant",
                            );
                            "unknown".to_string()
                        },
                        |raw| {
                            let trimmed = raw.trim();
                            if trimmed.is_empty() {
                                telemetry.record_fraud_invalid_metadata(
                                    lane_id,
                                    dataspace_id,
                                    dataspace_label.as_str(),
                                    "unknown",
                                    "tenant",
                                );
                                "unknown".to_string()
                            } else if trimmed.len() != raw.len() {
                                trimmed.to_owned()
                            } else {
                                raw
                            }
                        },
                    )
                },
            );

        let tenant_label = tenant_label_from(&tenant);

        let latency_ms: Option<u64> = metadata
            .get(FRAUD_ASSESSMENT_LATENCY_KEY.as_ref())
            .and_then(|value| {
                value.try_into_any_norito::<u64>().map_or_else(
                    |_| {
                        telemetry.record_fraud_invalid_metadata(
                            lane_id,
                            dataspace_id,
                            dataspace_label.as_str(),
                            tenant_label,
                            "latency_ms",
                        );
                        None
                    },
                    Some,
                )
            });

        let score_bps: Option<u16> =
            metadata
                .get(FRAUD_ASSESSMENT_SCORE_KEY.as_ref())
                .and_then(|value| {
                    value.try_into_any_norito::<u64>().map_or_else(
                        |_| {
                            telemetry.record_fraud_invalid_metadata(
                                lane_id,
                                dataspace_id,
                                dataspace_label.as_str(),
                                tenant_label,
                                "score_bps",
                            );
                            None
                        },
                        |raw| {
                            u16::try_from(raw).map_or_else(
                                |_| {
                                    telemetry.record_fraud_invalid_metadata(
                                        lane_id,
                                        dataspace_id,
                                        dataspace_label.as_str(),
                                        tenant_label,
                                        "score_bps",
                                    );
                                    None
                                },
                                Some,
                            )
                        },
                    )
                });

        let disposition = metadata
            .get(FRAUD_ASSESSMENT_DISPOSITION_KEY.as_ref())
            .and_then(|value| {
                value.try_into_any_norito::<String>().map_or_else(
                    |_| {
                        telemetry.record_fraud_invalid_metadata(
                            lane_id,
                            dataspace_id,
                            dataspace_label.as_str(),
                            tenant_label,
                            "disposition",
                        );
                        None
                    },
                    |raw| {
                        FraudDisposition::from_metadata(&raw).unwrap_or_else(|()| {
                            telemetry.record_fraud_invalid_metadata(
                                lane_id,
                                dataspace_id,
                                dataspace_label.as_str(),
                                tenant_label,
                                "disposition",
                            );
                            None
                        })
                    },
                )
            });

        Self {
            telemetry,
            lane_id,
            dataspace_id,
            dataspace_label,
            tenant,
            score_bps,
            latency_ms,
            disposition,
        }
    }

    fn tenant_label(&self) -> &str {
        tenant_label_from(&self.tenant)
    }

    fn dataspace_label(&self) -> &str {
        self.dataspace_label.as_str()
    }

    fn record_missing(&self, cause: &'static str) {
        self.telemetry.record_fraud_missing_assessment(
            self.lane_id,
            self.dataspace_id,
            self.dataspace_label(),
            self.tenant_label(),
            cause,
        );
    }

    fn record_invalid(&self, field: &'static str) {
        self.telemetry.record_fraud_invalid_metadata(
            self.lane_id,
            self.dataspace_id,
            self.dataspace_label(),
            self.tenant_label(),
            field,
        );
    }

    fn record_assessment(&self, band: iroha_config::parameters::actual::FraudRiskBand) {
        self.telemetry.record_fraud_assessment(
            self.lane_id,
            self.dataspace_id,
            self.dataspace_label(),
            self.tenant_label(),
            band.as_str(),
            self.score_bps,
            self.latency_ms,
        );
        if let Some(direction) = self.outcome_mismatch_direction(band) {
            self.telemetry.record_fraud_outcome_mismatch(
                self.lane_id,
                self.dataspace_id,
                self.dataspace_label(),
                self.tenant_label(),
                direction,
            );
        }
    }

    fn record_attestation(&self, engine_id: &str, status: &'static str) {
        self.telemetry.record_fraud_attestation(
            self.lane_id,
            self.dataspace_id,
            self.dataspace_label(),
            self.tenant_label(),
            engine_id,
            status,
        );
    }

    fn outcome_mismatch_direction(
        &self,
        band: iroha_config::parameters::actual::FraudRiskBand,
    ) -> Option<&'static str> {
        let band_level = match band {
            iroha_config::parameters::actual::FraudRiskBand::Low => 0,
            iroha_config::parameters::actual::FraudRiskBand::Medium => 1,
            iroha_config::parameters::actual::FraudRiskBand::High => 2,
            iroha_config::parameters::actual::FraudRiskBand::Critical => 3,
        };
        match self.disposition {
            Some(FraudDisposition::Fraud) if band_level < 2 => Some("missed_fraud"),
            Some(FraudDisposition::Clean) if band_level >= 2 => Some("false_positive"),
            _ => None,
        }
    }
}

#[cfg(feature = "telemetry")]
impl FraudDisposition {
    fn from_metadata(raw: &str) -> Result<Option<Self>, ()> {
        let normalized = raw.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Err(());
        }
        match normalized.as_str() {
            "confirmed_fraud" | "chargeback" | "fraud" | "loss" | "write_off" => {
                Ok(Some(Self::Fraud))
            }
            "approved" | "cleared" | "authorized" | "settled" | "false_positive" | "refunded" => {
                Ok(Some(Self::Clean))
            }
            "declined" | "manual_review" | "review" | "pending" | "blocked" => Ok(None),
            _ => Err(()),
        }
    }
}

#[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
#[allow(clippy::too_many_lines)]
/// Enforce the configured fraud monitoring policy against the transaction metadata.
pub(crate) fn enforce_fraud_policy(
    config: &iroha_config::parameters::actual::FraudMonitoring,
    metadata: &iroha_data_model::metadata::Metadata,
    telemetry: Option<&StateTelemetry>,
    routing: &LaneAssignment<'_>,
) -> Result<(), TransactionRejectionReason> {
    if !config.enabled {
        return Ok(());
    }

    let lane_id = routing.lane_id;
    let dataspace_id = routing.dataspace_id;
    let dataspace_label = routing.dataspace_label();

    #[cfg(feature = "telemetry")]
    let fraud_ctx = FraudTelemetryContext::prepare(telemetry, routing, metadata);

    let Some(required) = config.required_minimum_band else {
        if config.enabled {
            warn!(
                "Fraud monitoring enabled but required_minimum_band not set; skipping enforcement"
            );
        }
        return Ok(());
    };

    let Some(value) = metadata.get(FRAUD_ASSESSMENT_BAND_NAME.as_ref()) else {
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            let cause = if config.missing_assessment_grace.is_zero() {
                "missing"
            } else {
                "grace"
            };
            ctx.record_missing(cause);
        }
        if config.missing_assessment_grace.is_zero() {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "fraud monitoring requires an attached assessment".into(),
                ),
            ));
        }
        warn!(
            missing_grace_seconds = config.missing_assessment_grace.as_secs(),
            endpoints = ?config.service_endpoints,
            lane = ?lane_id,
            dataspace = ?dataspace_id,
            dataspace_label = %dataspace_label,
            "Transaction missing fraud assessment; permitted by grace window"
        );
        return Ok(());
    };

    let band_str = if let Ok(s) = value.try_into_any_norito::<String>() {
        s
    } else {
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            ctx.record_invalid("band_type");
        }
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted("fraud assessment band must be a string".into()),
        ));
    };

    let band = if let Ok(band) = band_str.parse::<iroha_config::parameters::actual::FraudRiskBand>()
    {
        band
    } else {
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            ctx.record_invalid("band_value");
        }
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(format!("fraud assessment band '{band_str}' is invalid")),
        ));
    };

    let tenant_value = if let Some(value) = metadata.get(FRAUD_ASSESSMENT_TENANT_NAME.as_ref()) {
        value
    } else {
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            ctx.record_invalid("tenant");
        }
        warn!(
            lane = ?lane_id,
            dataspace = ?dataspace_id,
            dataspace_label = %dataspace_label,
            "fraud assessment missing tenant metadata"
        );
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(
                "fraud monitoring requires metadata field 'fraud_assessment_tenant'".into(),
            ),
        ));
    };
    let tenant_raw = tenant_value.try_into_any_norito::<String>().map_err(|_| {
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            ctx.record_invalid("tenant");
        }
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "fraud assessment tenant must be a string".into(),
        ))
    })?;
    if tenant_raw.trim().is_empty() {
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            ctx.record_invalid("tenant");
        }
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted("fraud assessment tenant must not be empty".into()),
        ));
    }
    let tenant = tenant_raw;

    if let Some(latency_value) = metadata.get(FRAUD_ASSESSMENT_LATENCY_NAME.as_ref())
        && latency_value
            .try_into_any_norito::<u64>()
            .map(|_| ())
            .is_err()
    {
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            ctx.record_invalid("latency_ms");
        }
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(
                "fraud assessment latency must be an unsigned integer".into(),
            ),
        ));
    }

    let score_value = if let Some(value) = metadata.get(FRAUD_ASSESSMENT_SCORE_NAME.as_ref()) {
        value
    } else {
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            ctx.record_invalid("score_bps");
        }
        warn!(
            endpoints = ?config.service_endpoints,
            lane = ?lane_id,
            dataspace = ?dataspace_id,
            dataspace_label = %dataspace_label,
            "fraud assessment missing score_bps metadata"
        );
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(
                "fraud monitoring requires metadata field 'fraud_assessment_score_bps'".into(),
            ),
        ));
    };
    let score_raw = score_value.try_into_any_norito::<u64>().map_err(|_| {
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            ctx.record_invalid("score_bps");
        }
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "fraud assessment score must be an integer basis-point value".into(),
        ))
    })?;
    let score_bps = u16::try_from(score_raw).map_err(|_| {
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            ctx.record_invalid("score_bps");
        }
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
            "fraud assessment score {score_raw} exceeds supported range (0-10000 basis points)"
        )))
    })?;
    if score_bps > 10_000 {
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            ctx.record_invalid("score_bps");
        }
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(format!(
                "fraud assessment score {score_bps} exceeds supported range (0-10000 basis points)"
            )),
        ));
    }

    let expected_band = expected_band_from_score(score_bps);
    if expected_band != band {
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            ctx.record_invalid("band");
        }
        warn!(
            lane = ?lane_id,
            dataspace = ?dataspace_id,
            dataspace_label = %dataspace_label,
            actual = %band,
            expected = %expected_band,
            score_bps,
            "fraud assessment band does not match reported score"
        );
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(format!(
                "fraud assessment band {band} inconsistent with score {score_bps} bps (expected {expected_band})"
            )),
        ));
    }

    #[cfg(feature = "telemetry")]
    if let Some(ctx) = fraud_ctx.as_ref() {
        ctx.record_assessment(band);
    }

    if band < required {
        return Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(format!(
                "fraud assessment band {band} below required minimum {required}"
            )),
        ));
    }

    if !config.attesters.is_empty() {
        let tenant_label = {
            let trimmed = tenant.trim();
            if trimmed.is_empty() {
                "unknown"
            } else {
                trimmed
            }
        };
        let Some(envelope_value) = metadata.get(FRAUD_ASSESSMENT_ENVELOPE_NAME.as_ref()) else {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_invalid("attestation_envelope");
                ctx.record_attestation("unknown", "missing_envelope");
            }
            warn!(
                lane = ?lane_id,
                dataspace = ?dataspace_id,
                dataspace_label = %dataspace_label,
                tenant = %tenant_label,
                "fraud assessment missing attestation envelope metadata"
            );
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "fraud monitoring requires metadata field 'fraud_assessment_envelope'".into(),
                ),
            ));
        };
        let envelope_raw = envelope_value
            .try_into_any_norito::<String>()
            .map_err(|_| {
                #[cfg(feature = "telemetry")]
                if let Some(ctx) = fraud_ctx.as_ref() {
                    ctx.record_invalid("attestation_envelope");
                    ctx.record_attestation("unknown", "envelope_type");
                }
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                    "fraud assessment envelope must be a base64 string".into(),
                ))
            })?;
        let envelope_trimmed = envelope_raw.trim();
        if envelope_trimmed.is_empty() {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_invalid("attestation_envelope");
                ctx.record_attestation("unknown", "missing_envelope");
            }
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("fraud assessment envelope must not be blank".into()),
            ));
        }
        let envelope_bytes = BASE64_STANDARD
            .decode(envelope_trimmed.as_bytes())
            .map_err(|err| {
                #[cfg(feature = "telemetry")]
                if let Some(ctx) = fraud_ctx.as_ref() {
                    ctx.record_invalid("attestation_envelope");
                    ctx.record_attestation("unknown", "envelope_decode");
                }
                warn!(
                    lane = ?lane_id,
                    dataspace = ?dataspace_id,
                    dataspace_label = %dataspace_label,
                    tenant = %tenant_label,
                    error = %err,
                    "fraud assessment envelope failed base64 decoding"
                );
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                    "fraud assessment envelope must be base64-encoded".into(),
                ))
            })?;
        let mut cursor = envelope_bytes.as_slice();
        let assessment: FraudAssessment =
            norito::codec::Decode::decode(&mut cursor).map_err(|err| {
                #[cfg(feature = "telemetry")]
                if let Some(ctx) = fraud_ctx.as_ref() {
                    ctx.record_invalid("attestation_envelope");
                    ctx.record_attestation("unknown", "envelope_decode");
                }
                warn!(
                    lane = ?lane_id,
                    dataspace = ?dataspace_id,
                    dataspace_label = %dataspace_label,
                    tenant = %tenant_label,
                    error = %err,
                    "fraud assessment envelope failed Norito decode"
                );
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                    "fraud assessment envelope could not be decoded".into(),
                ))
            })?;
        if !cursor.is_empty() {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_invalid("attestation_envelope");
                ctx.record_attestation("unknown", "envelope_decode");
            }
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "fraud assessment envelope contains trailing bytes".into(),
                ),
            ));
        }
        let signature_bytes = assessment.signature.as_ref().ok_or_else(|| {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_invalid("attestation_signature");
                ctx.record_attestation("unknown", "unsigned");
            }
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                "fraud assessment envelope missing signature".into(),
            ))
        })?;
        let engine_id = assessment.engine_id.trim();
        let attester = config.attester(engine_id).ok_or_else(|| {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_attestation(engine_id, "unknown_engine");
            }
            warn!(
                lane = ?lane_id,
                dataspace = ?dataspace_id,
                dataspace_label = %dataspace_label,
                tenant = %tenant_label,
                engine = %assessment.engine_id,
                "fraud assessment engine id not registered for attestation"
            );
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
                "fraud assessment engine id '{}' is not registered with this host",
                assessment.engine_id
            )))
        })?;
        let attester_label = attester.engine_label();
        let mut unsigned = assessment.clone();
        unsigned.signature = None;
        let attester_algorithm = attester.public_key.try_algorithm().map_err(|_| {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_invalid("attestation_signature");
                ctx.record_attestation(attester_label, "public_key");
            }
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                "fraud assessment attester public key is malformed".into(),
            ))
        })?;
        if attester_algorithm == iroha_crypto::Algorithm::Ed25519
            && signature_bytes.len() != ED25519_SIGNATURE_LENGTH
        {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_invalid("attestation_signature");
                ctx.record_attestation(attester_label, "signature_parse");
            }
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("fraud assessment signature must be 64 bytes".into()),
            ));
        }
        let signature_parse_rejection = |err: String| {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_invalid("attestation_signature");
                ctx.record_attestation(attester_label, "signature_parse");
            }
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
                "fraud assessment signature is malformed: {err}"
            )))
        };
        let signature = match attester_algorithm {
            iroha_crypto::Algorithm::Ed25519 => {
                iroha_crypto::ed25519_parse_signature(signature_bytes)
                    .map_err(|err| signature_parse_rejection(err.to_string()))?
            }
            iroha_crypto::Algorithm::MlDsa => {
                iroha_crypto::mldsa65_parse_signature(signature_bytes)
                    .map_err(|err| signature_parse_rejection(err.to_string()))?
            }
            _ => iroha_crypto::Signature::try_from_bytes(signature_bytes).map_err(|err| {
                #[cfg(feature = "telemetry")]
                if let Some(ctx) = fraud_ctx.as_ref() {
                    ctx.record_invalid("attestation_signature");
                    ctx.record_attestation(attester_label, "signature_parse");
                }
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
                    "fraud assessment signature is malformed: {err}"
                )))
            })?,
        };
        let typed = iroha_crypto::SignatureOf::<FraudAssessment>::from_signature(signature);
        typed.verify(&attester.public_key, &unsigned).map_err(|_| {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_attestation(attester_label, "signature_verify");
            }
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                "fraud assessment signature failed verification".into(),
            ))
        })?;
        if assessment.risk_score_bps != score_bps {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_attestation(attester_label, "score_mismatch");
            }
            warn!(
                lane = ?lane_id,
                dataspace = ?dataspace_id,
                dataspace_label = %dataspace_label,
                tenant = %tenant_label,
                engine = %assessment.engine_id,
                observed = assessment.risk_score_bps,
                metadata = score_bps,
                "fraud assessment risk_score_bps mismatch with metadata"
            );
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "fraud assessment score does not match attested envelope".into(),
                ),
            ));
        }
        let unsigned_bytes = norito::codec::Encode::encode(&unsigned);
        let digest_bytes: [u8; 32] = iroha_crypto::Hash::new(&unsigned_bytes).into();
        let expected_digest = hex::encode_upper(digest_bytes);
        let Some(digest_value) = metadata.get(FRAUD_ASSESSMENT_DIGEST_NAME.as_ref()) else {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_invalid("attestation_digest");
                ctx.record_attestation(attester_label, "digest_missing");
            }
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "fraud monitoring requires metadata field 'fraud_assessment_digest'".into(),
                ),
            ));
        };
        let digest_str = digest_value.try_into_any_norito::<String>().map_err(|_| {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_invalid("attestation_digest");
                ctx.record_attestation(attester_label, "digest_type");
            }
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                "fraud assessment digest must be a hex string".into(),
            ))
        })?;
        let digest_trimmed = digest_str.trim();
        if digest_trimmed.len() != expected_digest.len()
            || !digest_trimmed.eq_ignore_ascii_case(&expected_digest)
        {
            #[cfg(feature = "telemetry")]
            if let Some(ctx) = fraud_ctx.as_ref() {
                ctx.record_attestation(attester_label, "digest_mismatch");
            }
            warn!(
                lane = ?lane_id,
                dataspace = ?dataspace_id,
                dataspace_label = %dataspace_label,
                tenant = %tenant_label,
                engine = %assessment.engine_id,
                expected = %expected_digest,
                provided = %digest_trimmed,
                "fraud assessment digest mismatch"
            );
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(
                    "fraud assessment digest does not match attested payload".into(),
                ),
            ));
        }
        #[cfg(feature = "telemetry")]
        if let Some(ctx) = fraud_ctx.as_ref() {
            ctx.record_attestation(attester_label, "verified");
        }
    }

    Ok(())
}

fn expected_band_from_score(score_bps: u16) -> iroha_config::parameters::actual::FraudRiskBand {
    use iroha_config::parameters::actual::FraudRiskBand;

    if score_bps > 1_000 {
        match score_bps {
            ..=2_499 => FraudRiskBand::Low,
            2_500..=5_499 => FraudRiskBand::Medium,
            5_500..=7_499 => FraudRiskBand::High,
            _ => FraudRiskBand::Critical,
        }
    } else {
        match score_bps {
            ..=249 => FraudRiskBand::Low,
            250..=549 => FraudRiskBand::Medium,
            550..=749 => FraudRiskBand::High,
            _ => FraudRiskBand::Critical,
        }
    }
}

#[cfg(test)]
/// Tests for transaction acceptance and validation.
pub mod tests {
    use core::panic;
    use std::sync::LazyLock; // for Name::from_str in tests
    use std::{
        borrow::Cow,
        collections::{BTreeMap, BTreeSet},
        num::{NonZeroU16, NonZeroU32, NonZeroU64},
        path::PathBuf,
        str::FromStr,
        sync::Arc,
    };

    use iroha_crypto::{
        Algorithm, Hash, KeyPair, MerkleProof,
        privacy::{LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment},
    };
    use iroha_data_model::{
        account::{Account, AccountId, MultisigMember, MultisigPolicy},
        block::{
            BlockHeader, SignedBlock,
            consensus::{LaneBlockDescriptorV1, LaneBlockProposalV1, SumeragiLanePayloadOwnership},
        },
        domain::{Domain, DomainId},
        events::{
            EventBox,
            data::{
                self,
                prelude::{AccountEvent, AssetChanged, AssetEvent, DomainEvent},
            },
            trigger_completed::{TriggerCompletedEvent, TriggerCompletedOutcome},
        },
        isi::{
            InstructionBox, Log,
            governance::{ProposeRuntimeUpgradeProposal, VotingMode},
        },
        metadata::Metadata,
        name::Name,
        nexus::{
            AUTOSCALE_META_CREATED_HEIGHT, AUTOSCALE_META_MANAGED, AssetPermissionManifest,
            AuditControls, DataSpaceCatalog, DataSpaceId as TestDataSpaceId, JurisdictionSet,
            LaneCatalog, LaneCompliancePolicy, LaneCompliancePolicyId, LaneComplianceRule,
            LaneConfig, LaneId as TestLaneId, LanePrivacyMerkleWitness, LanePrivacyProof,
            LanePrivacyWitness, LaneStorageProfile, LaneVisibility, ManifestVersion,
            ParticipantSelector,
        },
        peer::PeerId,
        permission::Permissions,
        proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyId},
        role::{Role, RoleId},
        runtime::RuntimeUpgradeManifest,
        transaction::{
            TransactionBuilder,
            executable::ContractInvocation,
            signed::{MultisigSignature, MultisigSignatures},
        },
    };
    use iroha_executor_data_model::isi::multisig::{
        DEFAULT_MULTISIG_TTL_MS, MultisigApprove, MultisigRegister, MultisigSpec,
    };
    use iroha_genesis::GENESIS_DOMAIN_ID;
    use iroha_logger::Level;
    use iroha_primitives::{
        const_vec::ConstVec,
        json::Json,
        numeric::{Numeric, Quantity},
        time::TimeSource,
    };
    use iroha_schema::Ident;
    use iroha_test_samples::gen_account_in;
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        block::{BlockBuilder, CommittedBlock, ValidBlock},
        compliance::LaneComplianceEngine,
        governance::manifest::{
            GovernanceRules, LaneManifestRegistry, LaneManifestStatus, RuntimeUpgradeHook,
        },
        kura::Kura,
        nexus::space_directory::{
            SpaceDirectoryManifestRecord, SpaceDirectoryManifestSet, UaidDataspaceBindings,
        },
        query::store::LiveQueryStore,
        smartcontracts::ivm::cache::IvmCache,
        state::{State, StateBlock, StateReadOnly, World},
    };

    fn checked_signature_of<T: norito::codec::Encode>(
        private_key: &PrivateKey,
        payload: &T,
    ) -> SignatureOf<T> {
        SignatureOf::try_new(private_key, payload).expect("test fixture signing should succeed")
    }

    fn checked_fixture_keypair(seed: Vec<u8>, algorithm: Algorithm) -> KeyPair {
        KeyPair::try_from_seed(seed, algorithm).expect("test fixture key derivation should succeed")
    }

    fn checked_random_tx_keypair() -> KeyPair {
        KeyPair::try_random().expect("transaction fixture key generation should succeed")
    }

    fn checked_random_tx_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm)
            .expect("transaction fixture key generation should succeed")
    }

    #[test]
    fn tx_fixture_key_generation_preserves_algorithms() {
        assert_eq!(
            checked_random_tx_keypair().public_key().algorithm(),
            Algorithm::default()
        );
        for algorithm in [Algorithm::Ed25519, Algorithm::Secp256k1] {
            assert_eq!(
                checked_random_tx_keypair_with_algorithm(algorithm)
                    .public_key()
                    .algorithm(),
                algorithm
            );
        }
    }

    fn single_lane_assignment(catalog: &DataSpaceCatalog) -> super::LaneAssignment<'_> {
        super::LaneAssignment {
            lane_id: TestLaneId::SINGLE,
            dataspace_id: TestDataSpaceId::UNIVERSAL,
            dataspace_catalog: catalog,
        }
    }

    fn new_account_in_domain(
        account_id: &AccountId,
        _domain_id: &DomainId,
    ) -> iroha_data_model::account::NewAccount {
        Account::new(account_id.clone())
    }

    fn world_with_authority(domain: &str) -> (World, AccountId, KeyPair) {
        let (authority_id, key_pair) = gen_account_in(domain);
        let domain_id = DomainId::try_new(domain, "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(&authority_id);
        let account = new_account_in_domain(&authority_id, &domain_id).build(&authority_id);
        (World::with([domain], [account], []), authority_id, key_pair)
    }

    fn world_with_convertible_zk_asset(
        allow_shield: bool,
        allow_unshield: bool,
    ) -> (World, AccountId, AssetDefinitionId) {
        let (mut world, authority_id, _) = world_with_authority("wonderland");
        let asset_def_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "zkpolicy".parse().expect("asset name"),
        );
        let asset_definition = AssetDefinition::numeric(asset_def_id.clone())
            .with_name(asset_def_id.name().to_string())
            .confidential_policy(
                iroha_data_model::asset::definition::AssetConfidentialPolicy::convertible(),
            )
            .build(&authority_id);
        world
            .asset_definitions
            .insert(asset_def_id.clone(), asset_definition);
        let mut zk_state = crate::state::ZkAssetState::default();
        zk_state.mode = iroha_data_model::isi::zk::ZkAssetMode::Hybrid;
        zk_state.allow_shield = allow_shield;
        zk_state.allow_unshield = allow_unshield;
        world.zk_assets.insert(asset_def_id.clone(), zk_state);
        (world, authority_id, asset_def_id)
    }

    #[test]
    fn confidential_policy_admission_rejects_disabled_shield() {
        let (world, authority_id, asset_def_id) = world_with_convertible_zk_asset(false, true);
        let executable =
            Executable::Instructions(ConstVec::from(vec![InstructionBox::from(zk::Shield::new(
                asset_def_id,
                authority_id,
                10_u128,
                [7; 32],
                iroha_data_model::confidential::ConfidentialEncryptedPayload::default(),
            ))]));

        let err = validate_confidential_policy_admission_for_world(&executable, &world.view(), 1)
            .expect_err("disabled shield must be rejected during admission");

        match err {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason)) => {
                assert_eq!(reason, "shield not permitted by policy");
            }
            other => panic!("expected policy NotPermitted rejection, got {other:?}"),
        }
    }

    #[test]
    fn confidential_policy_admission_allows_enabled_shield() {
        let (world, authority_id, asset_def_id) = world_with_convertible_zk_asset(true, true);
        let executable =
            Executable::Instructions(ConstVec::from(vec![InstructionBox::from(zk::Shield::new(
                asset_def_id,
                authority_id,
                10_u128,
                [9; 32],
                iroha_data_model::confidential::ConfidentialEncryptedPayload::default(),
            ))]));

        validate_confidential_policy_admission_for_world(&executable, &world.view(), 1)
            .expect("enabled shield should pass confidential policy admission");
    }

    fn world_with_uaid_account(
        uaid: UniversalAccountId,
        dataspace: TestDataSpaceId,
        with_manifest: bool,
        manifest_active: bool,
    ) -> (World, AccountId) {
        let (authority, _) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(&authority);
        let account = new_account_in_domain(&authority, &domain_id)
            .with_uaid(Some(uaid))
            .build(&authority);
        let mut world = World::with([domain], [account], []);

        if with_manifest {
            let manifest = AssetPermissionManifest {
                version: ManifestVersion::default(),
                uaid,
                dataspace,
                issued_ms: 1,
                activation_epoch: 1,
                expiry_epoch: None,
                entries: Vec::new(),
            };
            let mut record = SpaceDirectoryManifestRecord::new(manifest);
            record.lifecycle.mark_activated(1);
            if !manifest_active {
                record.lifecycle.mark_expired(2);
            }

            let mut set = SpaceDirectoryManifestSet::default();
            set.upsert(record);
            world.space_directory_manifests.insert(uaid, set);
            if manifest_active {
                let mut bindings = UaidDataspaceBindings::default();
                bindings.bind_account(dataspace, authority.clone());
                world.uaid_dataspaces.insert(uaid, bindings);
            }
        }

        (world, authority)
    }

    #[test]
    fn lane_identity_rejects_uaid_without_dataspace_binding() {
        let uaid = UniversalAccountId::from_hash(Hash::new(b"tx::uaid-no-manifest"));
        let dataspace = TestDataSpaceId::new(7);
        let (world, authority) = world_with_uaid_account(uaid, dataspace, false, true);
        let world_view = world.view();

        let err =
            super::extract_lane_identity_metadata(&world_view, &authority, dataspace, "lane-x")
                .expect_err("UAID routing must require a dataspace binding");
        match err {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
                assert!(
                    msg.contains("not bound to dataspace"),
                    "expected missing binding rejection message, got {msg}"
                );
            }
            other => panic!("expected NotPermitted rejection, got {other:?}"),
        }
    }

    #[test]
    fn lane_policy_allows_space_directory_manifest_publish_before_uaid_binding_exists() {
        let chain: ChainId = "space-directory-publish".parse().unwrap();
        let uaid = UniversalAccountId::from_hash(Hash::new(b"tx::publish-manifest"));
        let dataspace = TestDataSpaceId::new(10);
        let (authority, keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(&authority);
        let account = new_account_in_domain(&authority, &domain_id)
            .with_uaid(Some(uaid))
            .build(&authority);
        let world = World::with([domain], [account], []);
        let state = State::new_with_chain(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            chain.clone(),
        );
        let policy = LaneCompliancePolicy {
            id: LaneCompliancePolicyId::new(Hash::prehashed([0xBC; 32])),
            version: 1,
            lane_id: TestLaneId::SINGLE,
            dataspace_id: dataspace,
            jurisdiction: JurisdictionSet::default(),
            deny: vec![LaneComplianceRule {
                selector: ParticipantSelector {
                    account: Some(authority.clone()),
                    ..ParticipantSelector::default()
                },
                reason_code: Some("unbound uaid".to_string()),
                jurisdiction_override: JurisdictionSet::default(),
            }],
            allow: Vec::new(),
            transfer_limits: Vec::new(),
            audit_controls: AuditControls::default(),
            metadata: Metadata::default(),
        };
        let engine = LaneComplianceEngine::from_policies(vec![policy], false).expect("engine");
        state.install_lane_compliance_engine(Some(Arc::new(engine)));

        let manifest = AssetPermissionManifest {
            version: ManifestVersion::default(),
            uaid,
            dataspace,
            issued_ms: 1,
            activation_epoch: 0,
            expiry_epoch: None,
            entries: Vec::new(),
        };
        let tx = TransactionBuilder::new(
            chain,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([PublishSpaceDirectoryManifest { manifest }])
        .sign(keypair.private_key());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let stx = block.transaction();
        let assignment = super::LaneAssignment {
            lane_id: TestLaneId::SINGLE,
            dataspace_id: TestDataSpaceId::UNIVERSAL,
            dataspace_catalog: &stx.nexus.dataspace_catalog,
        };

        super::enforce_lane_policies(&tx, &stx, &assignment)
            .expect("manifest publication creates the UAID dataspace binding");
    }

    #[test]
    fn lane_identity_rejects_inactive_target_manifest() {
        let uaid = UniversalAccountId::from_hash(Hash::new(b"tx::uaid-inactive-manifest"));
        let dataspace = TestDataSpaceId::new(9);
        let (world, authority) = world_with_uaid_account(uaid, dataspace, true, false);
        let world_view = world.view();

        let err =
            super::extract_lane_identity_metadata(&world_view, &authority, dataspace, "lane-x")
                .expect_err("inactive target manifest must be rejected");
        match err {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
                assert!(
                    msg.contains("not active"),
                    "expected inactive-manifest rejection message, got {msg}"
                );
            }
            other => panic!("expected NotPermitted rejection, got {other:?}"),
        }
    }

    #[test]
    fn dataspace_label_helper_uses_alias_and_fallback() {
        use iroha_data_model::nexus::DataSpaceMetadata;

        let catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: TestDataSpaceId::new(7),
                alias: "alpha".to_string(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid catalog");

        let label = super::dataspace_label_from_catalog(&catalog, TestDataSpaceId::new(7));
        assert_eq!(label, "alpha");

        let fallback = super::dataspace_label_from_catalog(&catalog, TestDataSpaceId::new(9));
        assert_eq!(fallback, "9");
    }

    #[test]
    fn duration_since_epoch_with_ok_result_passes_through() {
        let expected = Duration::from_secs(42);
        let actual = super::duration_since_epoch_with_fallback(Ok(expected));
        assert_eq!(actual, expected);
    }

    #[test]
    fn duration_since_epoch_with_err_falls_back_to_zero() {
        let skew_error = SystemTime::UNIX_EPOCH
            .duration_since(SystemTime::UNIX_EPOCH + Duration::from_secs(5))
            .unwrap_err();
        let actual = super::duration_since_epoch_with_fallback(Err(skew_error));
        assert_eq!(actual, Duration::ZERO);
    }

    #[test]
    fn validate_genesis_with_now_uses_supplied_timestamp() {
        let far_future = Duration::from_secs(10_000_000_000);
        let (_handle, time_source) = TimeSource::new_mock(far_future);
        let tx = TransactionBuilder::new_with_time_source(
            CHAIN_ID.clone(),
            GENESIS_ACCOUNT.id.clone(),
            &time_source,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::DEBUG,
            "genesis timestamp check".to_string(),
        )])
        .sign(&GENESIS_ACCOUNT.key);

        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        AcceptedTransaction::validate_genesis_with_now(
            &tx,
            &CHAIN_ID,
            Duration::from_secs(1),
            &GENESIS_ACCOUNT.id,
            &crypto_cfg,
            far_future,
        )
        .expect("genesis validation should use provided timestamp");
    }

    #[test]
    fn signature_limit_allows_count_at_cap() {
        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            nonzero!(3_u64),
            nonzero!(16_u64),
            nonzero!(2048_u64),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );

        super::AcceptedTransaction::ensure_signature_limit(3, &limits)
            .expect("signature count at cap should be accepted");
    }

    #[test]
    fn signature_limit_rejects_counts_above_cap() {
        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            nonzero!(3_u64),
            nonzero!(16_u64),
            nonzero!(2048_u64),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );

        let err = super::AcceptedTransaction::ensure_signature_limit(4, &limits)
            .expect_err("signature count above cap must be rejected");

        match err {
            super::AcceptTransactionFail::TransactionLimit(fail) => {
                assert!(
                    fail.reason.contains("Too many signatures"),
                    "error message should explain the signature cap: {:?}",
                    fail.reason
                );
            }
            other => panic!("expected TransactionLimit failure, got {other:?}"),
        }
    }

    #[test]
    fn malformed_multisig_bundle_shapes_have_stable_rejection_code() {
        for error in [
            TransactionSignatureError::UnexpectedMultisigSignatures,
            TransactionSignatureError::NonCanonicalMultisigSignatures,
        ] {
            assert_eq!(
                AcceptedTransaction::signature_rejection_code(&error),
                SignatureRejectionCode::MalformedSignature,
            );
        }
    }

    #[test]
    fn multisig_authority_rejected_with_stable_code() {
        let chain: ChainId = "multisig-accept".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let mut builder = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder = builder.with_instructions([Log::new(Level::INFO, "multisig".into())]);
        let tx = builder.sign(keypair.private_key());

        let member = MultisigMember::new(keypair.public_key().clone(), 1).expect("member is valid");
        let policy = MultisigPolicy::new(1, vec![member]).expect("policy is valid");
        let multisig_authority = AccountId::new_multisig(policy);
        let tx = tx.with_authority(multisig_authority);

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        match AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg) {
            Err(AcceptTransactionFail::SignatureVerification(fail)) => {
                assert_eq!(
                    fail.code(),
                    SignatureRejectionCode::MissingSignatures,
                    "expected multisig missing-signatures code"
                );
                assert_eq!(
                    fail.detail,
                    "missing multisig signatures for multisig authority"
                );
            }
            other => panic!("expected SignatureVerification failure, got {other:?}"),
        }
    }

    #[test]
    fn multisig_authority_accepts_mixed_curves_with_quorum() {
        let chain: ChainId = "multisig-accept".parse().unwrap();
        let member_ed = checked_random_tx_keypair_with_algorithm(Algorithm::Ed25519);
        let member_secp = checked_random_tx_keypair_with_algorithm(Algorithm::Secp256k1);

        let members = vec![
            MultisigMember::new(member_ed.public_key().clone(), 1).expect("member ed"),
            MultisigMember::new(member_secp.public_key().clone(), 1).expect("member secp"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let authority = AccountId::new_multisig(policy.clone());

        let mut builder = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder = builder.with_instructions([Log::new(Level::INFO, "multisig ok".into())]);
        let tx = builder.sign_multisig([member_ed.private_key(), member_secp.private_key()]);

        let limits = TransactionParameters::default();
        let mut crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        if !crypto_cfg.allowed_signing.contains(&Algorithm::Secp256k1) {
            crypto_cfg.allowed_signing.push(Algorithm::Secp256k1);
        }
        crypto_cfg.allowed_signing.sort();
        crypto_cfg.allowed_signing.dedup();

        AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg)
            .expect("multisig with quorum should be accepted");
    }

    #[test]
    fn multisig_authority_rejects_unknown_signer() {
        let chain: ChainId = "multisig-unknown".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let mut builder = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder = builder.with_instructions([Log::new(Level::INFO, "multisig".into())]);
        let tx = builder.sign(keypair.private_key());

        let member = MultisigMember::new(keypair.public_key().clone(), 2).expect("member is valid");
        let policy = MultisigPolicy::new(2, vec![member]).expect("policy is valid");
        let multisig_authority = AccountId::new_multisig(policy);
        let mut tx = tx.with_authority(multisig_authority);

        // Attach a signature from an unknown signer.
        let payload = tx.payload().clone();
        let rogue = checked_random_tx_keypair();
        let rogue_sig = checked_signature_of(rogue.private_key(), &payload);
        tx.set_multisig_signatures(
            iroha_data_model::transaction::signed::MultisigSignatures::new(vec![
                iroha_data_model::transaction::signed::MultisigSignature::new(
                    rogue.public_key().clone(),
                    rogue_sig,
                ),
            ]),
        );

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        match AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg) {
            Err(AcceptTransactionFail::SignatureVerification(fail)) => {
                assert_eq!(fail.code(), SignatureRejectionCode::UnknownSigner);
            }
            other => panic!("expected UnknownSigner rejection, got {other:?}"),
        }
    }

    #[test]
    fn multisig_authority_rejects_insufficient_weight_bundle() {
        let chain: ChainId = "multisig-insufficient-weight".parse().unwrap();
        let signer = checked_random_tx_keypair();
        let other = checked_random_tx_keypair();

        let members = vec![
            MultisigMember::new(signer.public_key().clone(), 1).expect("member"),
            MultisigMember::new(other.public_key().clone(), 1).expect("member"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let authority = AccountId::new_multisig(policy);

        let mut builder = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder = builder
            .with_instructions([Log::new(Level::INFO, "insufficient multisig weight".into())]);
        let tx = builder.sign_multisig([signer.private_key()]);

        let limits = TransactionParameters::default();
        let mut crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        if !crypto_cfg.allowed_signing.contains(&Algorithm::Ed25519) {
            crypto_cfg.allowed_signing.push(Algorithm::Ed25519);
        }
        crypto_cfg.allowed_signing.sort();
        crypto_cfg.allowed_signing.dedup();

        match AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg) {
            Err(AcceptTransactionFail::SignatureVerification(fail)) => {
                assert_eq!(fail.code(), SignatureRejectionCode::InsufficientWeight);
            }
            other => panic!("expected InsufficientWeight rejection, got {other:?}"),
        }
    }

    #[test]
    fn multisig_account_direct_signing_rejected_in_validation() {
        use iroha_data_model::domain::DomainId;

        let chain: ChainId = "multisig-direct".parse().unwrap();
        let domain_id: DomainId = DomainId::try_new("multisig", "universal").unwrap();
        let signer1 = checked_random_tx_keypair();
        let signer2 = checked_random_tx_keypair();
        let signer1_id = AccountId::new(signer1.public_key().clone());
        let signer2_id = AccountId::new(signer2.public_key().clone());

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).expect("nonzero quorum"),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS)
                .expect("nonzero multisig ttl"),
        };

        let multisig_key = checked_random_tx_keypair();
        let multisig_id = AccountId::new(multisig_key.public_key().clone());

        let mut multisig_metadata = Metadata::default();
        multisig_metadata.insert(
            crate::smartcontracts::isi::multisig::spec_key(),
            Json::new(spec),
        );

        let domain = Domain::new(domain_id.clone()).build(&signer1_id);
        let accounts = [
            new_account_in_domain(&signer1_id, &domain_id).build(&signer1_id),
            new_account_in_domain(&signer2_id, &domain_id).build(&signer2_id),
            new_account_in_domain(&multisig_id, &domain_id)
                .with_metadata(multisig_metadata)
                .build(&multisig_id),
        ];
        let world = World::with([domain], accounts, []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let tx = TransactionBuilder::new(
            chain.clone(),
            multisig_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "direct multisig signer bypass".into(),
        )])
        .sign(multisig_key.private_key());

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted = AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg)
            .expect("admission must accept the signature shape");

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);

        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason))) => {
                assert!(
                    reason.contains("multisig"),
                    "unexpected reject reason: {reason}"
                );
            }
            other => panic!("expected multisig direct-sign reject, got {other:?}"),
        }
    }

    #[test]
    fn deactivated_contract_subject_remains_in_the_non_signing_index() {
        use iroha_data_model::{
            domain::DomainId, isi::smart_contract_code::DeactivateContractInstance,
            smart_contract::ContractAddress,
        };

        let chain: ChainId = "contract-subject-direct-sign".parse().unwrap();
        let domain_id: DomainId = DomainId::try_new("contracts", "universal").unwrap();
        let deployer_keypair = checked_random_tx_keypair();
        let deployer = AccountId::new(deployer_keypair.public_key().clone());
        let contract_address = ContractAddress::derive(
            0,
            &deployer,
            1,
            iroha_data_model::nexus::DataSpaceId::new(0),
        )
        .expect("derive contract address");

        let contract_subject = contract_address.subject_id();

        let domain = Domain::new(domain_id.clone()).build(&deployer);
        let deployer_account = new_account_in_domain(&deployer, &domain_id).build(&deployer);
        let mut world = World::with([domain], [deployer_account], []);
        world.contract_instances.insert(
            contract_address.clone(),
            iroha_crypto::Hash::new(b"contract-code"),
        );
        let lifecycle_permission: Permission =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode
                .into();
        world
            .account_permissions
            .insert(deployer.clone(), Permissions::from([lifecycle_permission]));
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut state_tx = block.transaction();
        DeactivateContractInstance {
            contract_address: contract_address.clone(),
            reason: Some("retired for signer-history regression".to_owned()),
        }
        .execute(&deployer, &mut state_tx)
        .expect("deactivate contract instance");
        let retained_binding = state_tx
            .world
            .contract_subject_bindings
            .get(&contract_address)
            .expect("deactivation must retain typed subject history");
        assert_eq!(retained_binding.subject, contract_subject);
        state_tx.apply();
        block.commit().expect("commit contract deactivation");

        assert!(
            code::is_historical_contract_subject(state.view().world(), &contract_subject),
            "deactivation must retain the canonical subject in the admission-denial index"
        );
    }

    #[test]
    fn multisig_signatory_role_does_not_block_direct_signing() {
        use iroha_data_model::domain::DomainId;

        let chain: ChainId = "multisig-role-only".parse().unwrap();
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let (authority_id, keypair) = gen_account_in("wonderland");

        let domain = Domain::new(domain_id.clone()).build(&authority_id);
        let account = new_account_in_domain(&authority_id, &domain_id).build(&authority_id);
        let mut world = World::with([domain], [account], []);

        let role_id: RoleId = format!(
            "MULTISIG_SIGNATORY/{}/{}",
            domain_id,
            authority_id.expect_single_signatory()
        )
        .parse()
        .expect("static multisig role must parse");
        let role = Role {
            id: role_id.clone(),
            permissions: Permissions::new(),
            permission_epochs: BTreeMap::new(),
        };
        world.roles.insert(role_id.clone(), role);
        world.account_roles.insert(
            crate::role::RoleIdWithOwner::new(authority_id.clone(), role_id),
            (),
        );

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "multisig direct sign role fallback".into(),
        )])
        .sign(keypair.private_key());

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted = AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg)
            .expect("admission must accept the signature shape");

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);

        assert!(
            result.is_ok(),
            "single-key signatories with multisig roles should keep ordinary direct-signing rights: {result:?}"
        );
    }

    #[test]
    fn multisig_signatory_role_can_submit_multisig_propose_envelope() {
        use iroha_data_model::domain::DomainId;
        use iroha_executor_data_model::isi::multisig::MultisigPropose;

        let chain: ChainId = "multisig-propose-role-allowed".parse().unwrap();
        let home_domain: DomainId = DomainId::try_new("banka", "universal").unwrap();
        let target_domain: DomainId = DomainId::try_new("centralbank", "universal").unwrap();

        let signer1 = checked_random_tx_keypair();
        let signer2 = checked_random_tx_keypair();
        let signer1_id = AccountId::new(signer1.public_key().clone());
        let signer2_id = AccountId::new(signer2.public_key().clone());
        let retail_key = checked_random_tx_keypair();
        let retail_id = AccountId::new(retail_key.public_key().clone());

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).expect("nonzero quorum"),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS)
                .expect("nonzero multisig ttl"),
        };
        let multisig_id = AccountId::new_multisig(
            MultisigPolicy::new(
                2,
                vec![
                    MultisigMember::new(signer1.public_key().clone(), 1).expect("signer1 member"),
                    MultisigMember::new(signer2.public_key().clone(), 1).expect("signer2 member"),
                ],
            )
            .expect("multisig policy"),
        );

        let home = Domain::new(home_domain.clone()).build(&signer1_id);
        let target = Domain::new(target_domain.clone()).build(&signer1_id);
        let signer1_account = new_account_in_domain(&signer1_id, &home_domain).build(&signer1_id);
        let signer2_account = new_account_in_domain(&signer2_id, &home_domain).build(&signer2_id);
        let world = World::with([home, target], [signer1_account, signer2_account], []);

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());
        let setup_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut setup_block = state.block(setup_header);
        let mut setup_tx = setup_block.transaction();
        crate::executor::Executor::Initial
            .execute_instruction(
                &mut setup_tx,
                &signer1_id,
                InstructionBox::from(MultisigRegister::with_account(
                    AccountId::new(checked_random_tx_keypair().public_key().clone()),
                    home_domain.clone(),
                    spec,
                )),
            )
            .expect("register canonical multisig account");
        setup_tx.apply();
        setup_block.commit().expect("commit multisig setup");

        let registration = Register::account(new_account_in_domain(&retail_id, &target_domain));
        let tx = TransactionBuilder::new(
            chain.clone(),
            signer1_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(MultisigPropose::new(
            multisig_id,
            vec![registration.into()],
            None,
        ))])
        .sign(signer1.private_key());

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted = AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg)
            .expect("admission must accept the signature shape");

        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);

        assert!(
            result.is_ok(),
            "multisig propose envelope should bypass direct-sign rejection for signatory roles: {result:?}"
        );
    }

    #[test]
    fn lane_validator_gating_allows_multisig_propose_envelope_from_live_signer() {
        use iroha_data_model::domain::DomainId;
        use iroha_executor_data_model::isi::multisig::MultisigPropose;

        let chain: ChainId = "multisig-propose-lane-validator-bypass".parse().unwrap();
        let home_domain: DomainId = DomainId::try_new("banka", "universal").unwrap();
        let target_domain: DomainId = DomainId::try_new("centralbank", "universal").unwrap();

        let signer1 = checked_random_tx_keypair();
        let signer2 = checked_random_tx_keypair();
        let validator = checked_random_tx_keypair();
        let signer1_id = AccountId::new(signer1.public_key().clone());
        let signer2_id = AccountId::new(signer2.public_key().clone());
        let validator_id = AccountId::new(validator.public_key().clone());
        let retail_key = checked_random_tx_keypair();
        let retail_id = AccountId::new(retail_key.public_key().clone());

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).expect("nonzero quorum"),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS)
                .expect("nonzero multisig ttl"),
        };
        let multisig_id = AccountId::new_multisig(
            MultisigPolicy::new(
                2,
                vec![
                    MultisigMember::new(signer1.public_key().clone(), 1).expect("signer1 member"),
                    MultisigMember::new(signer2.public_key().clone(), 1).expect("signer2 member"),
                ],
            )
            .expect("multisig policy"),
        );

        let home = Domain::new(home_domain.clone()).build(&signer1_id);
        let target = Domain::new(target_domain.clone()).build(&signer1_id);
        let signer1_account = new_account_in_domain(&signer1_id, &home_domain).build(&signer1_id);
        let signer2_account = new_account_in_domain(&signer2_id, &home_domain).build(&signer2_id);
        let validator_account =
            new_account_in_domain(&validator_id, &home_domain).build(&validator_id);
        let world = World::with(
            [home, target],
            [signer1_account, signer2_account, validator_account],
            [],
        );

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());
        let setup_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut setup_block = state.block(setup_header);
        let mut setup_tx = setup_block.transaction();
        crate::executor::Executor::Initial
            .execute_instruction(
                &mut setup_tx,
                &signer1_id,
                InstructionBox::from(MultisigRegister::with_account(
                    AccountId::new(checked_random_tx_keypair().public_key().clone()),
                    home_domain.clone(),
                    spec,
                )),
            )
            .expect("register canonical multisig account");
        setup_tx.apply();
        setup_block.commit().expect("commit multisig setup");

        let mut statuses = BTreeMap::new();
        statuses.insert(
            TestLaneId::SINGLE,
            LaneManifestStatus {
                lane: TestLaneId::SINGLE,
                alias: "centralbank".to_string(),
                dataspace: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: Some("parliament".to_string()),
                manifest_path: Some(std::path::PathBuf::from("/tmp/centralbank.manifest.json")),
                governance_rules: Some(GovernanceRules {
                    validators: vec![validator_id.clone()],
                    ..GovernanceRules::default()
                }),
                privacy_commitments: Vec::new(),
            },
        );
        let registry = std::sync::Arc::new(LaneManifestRegistry::from_statuses(statuses));
        state.install_lane_manifests(&registry);

        let registration = Register::account(new_account_in_domain(&retail_id, &target_domain));
        let tx = TransactionBuilder::new(
            chain.clone(),
            signer1_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(MultisigPropose::new(
            multisig_id,
            vec![registration.into()],
            None,
        ))])
        .sign(signer1.private_key());

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted = AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg)
            .expect("admission must accept the signature shape");

        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);

        assert!(
            result.is_ok(),
            "lane validator gating should not reject multisig propose envelopes from live signers: {result:?}"
        );
    }

    #[test]
    fn lane_validator_gating_ignores_non_governance_transactions() {
        let chain: ChainId = "lane-validator-gating-plain-transfer".parse().unwrap();
        let (authority, authority_keypair) = gen_account_in("wonderland");
        let (validator_a, _) = gen_account_in("wonderland");
        let (validator_b, _) = gen_account_in("wonderland");
        let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(&authority);
        let authority_account = new_account_in_domain(&authority, &domain_id).build(&authority);
        let validator_a_account =
            new_account_in_domain(&validator_a, &domain_id).build(&validator_a);
        let validator_b_account =
            new_account_in_domain(&validator_b, &domain_id).build(&validator_b);
        let world = World::with(
            [domain],
            [authority_account, validator_a_account, validator_b_account],
            [],
        );

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());
        let mut statuses = BTreeMap::new();
        statuses.insert(
            TestLaneId::SINGLE,
            LaneManifestStatus {
                lane: TestLaneId::SINGLE,
                alias: "paynet".to_string(),
                dataspace: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: Some("parliament".to_string()),
                manifest_path: Some(std::path::PathBuf::from("/tmp/paynet.manifest.json")),
                governance_rules: Some(GovernanceRules {
                    validators: vec![validator_a.clone(), validator_b.clone()],
                    quorum: Some(2),
                    ..GovernanceRules::default()
                }),
                privacy_commitments: Vec::new(),
            },
        );
        let registry = std::sync::Arc::new(LaneManifestRegistry::from_statuses(statuses));
        state.install_lane_manifests(&registry);

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "retail transfer".into())])
        .sign(authority_keypair.private_key());

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted = AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg)
            .expect("admission should accept transaction shape");

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);

        assert!(
            result.is_ok(),
            "lane validator gating should ignore plain transactions that do not touch governance surfaces: {result:?}"
        );
    }

    #[test]
    fn missing_authority_rejected_for_non_multisig_transaction() {
        let chain: ChainId = "missing-authority-regular".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "regular".into())])
        .sign(keypair.private_key());

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted = AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg)
            .expect("admission should accept transaction shape");

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);

        match result {
            Err(TransactionRejectionReason::AccountDoesNotExist(FindError::Account(id))) => {
                assert_eq!(id, authority, "unexpected missing-account id");
            }
            other => panic!("expected AccountDoesNotExist rejection, got {other:?}"),
        }
    }

    #[test]
    fn unregistered_authority_predicates_require_exact_first_self_registration() {
        let (authority, _) = gen_account_in("wonderland");
        let (other, _) = gen_account_in("wonderland");
        let exact = Executable::Instructions(
            vec![
                InstructionBox::from(Register::account(Account::new(authority.clone()))),
                InstructionBox::from(Log::new(Level::INFO, "after registration".into())),
            ]
            .into(),
        );
        assert!(executable_self_registers_authority(&exact, &authority));
        assert!(allows_unregistered_authority(&exact, &authority));

        let registers_other = Executable::Instructions(
            vec![InstructionBox::from(Register::account(Account::new(other)))].into(),
        );
        assert!(!executable_self_registers_authority(
            &registers_other,
            &authority
        ));
        assert!(!allows_unregistered_authority(&registers_other, &authority));

        let registration_is_not_first = Executable::Instructions(
            vec![
                InstructionBox::from(Log::new(Level::INFO, "before registration".into())),
                InstructionBox::from(Register::account(Account::new(authority.clone()))),
            ]
            .into(),
        );
        assert!(!executable_self_registers_authority(
            &registration_is_not_first,
            &authority
        ));
        assert!(!allows_unregistered_authority(
            &registration_is_not_first,
            &authority
        ));
    }

    #[test]
    fn missing_authority_self_register_allows_transaction() {
        let chain: ChainId = "missing-authority-self-register".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([
            InstructionBox::from(Register::account(Account::new(authority.clone()))),
            InstructionBox::from(Log::new(Level::INFO, "self-register".into())),
        ])
        .sign(keypair.private_key());

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted = AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg)
            .expect("admission should accept transaction shape");

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);

        assert!(result.is_ok(), "self-register flow should pass: {result:?}");
        assert!(
            block.world.accounts.get(&authority).is_some(),
            "authority account should be created by the first transaction"
        );
    }

    #[test]
    fn existing_authority_self_register_is_idempotent() {
        let chain: ChainId = "existing-authority-self-register".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let existing = Account::new(authority.clone()).build(&authority);
        let world = World::with([], [existing], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([
            InstructionBox::from(Register::account(Account::new(authority.clone()))),
            InstructionBox::from(Log::new(Level::INFO, "self-register-again".into())),
        ])
        .sign(keypair.private_key());

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted = AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg)
            .expect("admission should accept transaction shape");

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);

        assert!(
            result.is_ok(),
            "duplicate self-register should remain a no-op: {result:?}"
        );
    }

    #[test]
    fn missing_authority_multisig_approve_reaches_instruction_validation() {
        let chain: ChainId = "missing-authority-multisig-approve".parse().unwrap();
        let (missing_authority, keypair) = gen_account_in("wonderland");
        let multisig_account = AccountId::new(checked_random_tx_keypair().public_key().clone());
        let instructions_hash = HashOf::new(&Vec::<InstructionBox>::new());

        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let tx = TransactionBuilder::new(
            chain.clone(),
            missing_authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([MultisigApprove::new(
            multisig_account.clone(),
            instructions_hash,
        )])
        .sign(keypair.private_key());

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted = AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg)
            .expect("admission should accept transaction shape");

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);

        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
                iroha_data_model::isi::error::InstructionExecutionError::Find(FindError::Account(
                    _,
                )),
            ))) => {}
            other => panic!("expected instruction-level account lookup failure, got {other:?}"),
        }
    }

    #[test]
    fn single_authority_rejects_disallowed_algorithm() {
        let chain: ChainId = "single-disallowed".parse().unwrap();
        let keypair = checked_random_tx_keypair_with_algorithm(Algorithm::Secp256k1);
        let authority = AccountId::new(keypair.public_key().clone());

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "single disallowed algorithm".into())])
        .sign(keypair.private_key());

        let limits = TransactionParameters::default();
        let mut crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        crypto_cfg
            .allowed_signing
            .retain(|algo| *algo == Algorithm::Ed25519);

        match AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg) {
            Err(AcceptTransactionFail::SignatureVerification(fail)) => {
                assert_eq!(fail.code(), SignatureRejectionCode::AlgorithmNotPermitted);
            }
            other => panic!("expected AlgorithmNotPermitted rejection, got {other:?}"),
        }
    }

    #[test]
    fn multisig_authority_rejects_disallowed_algorithm() {
        let chain: ChainId = "multisig-disallowed".parse().unwrap();
        let member = checked_random_tx_keypair_with_algorithm(Algorithm::Secp256k1);

        let members = vec![MultisigMember::new(member.public_key().clone(), 1).expect("member")];
        let policy = MultisigPolicy::new(1, members).expect("policy");
        let authority = AccountId::new_multisig(policy);

        let mut builder = TransactionBuilder::new(
            chain.clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder = builder.with_instructions([Log::new(
            Level::INFO,
            "multisig disallowed algorithm".into(),
        )]);
        let tx = builder.sign_multisig(vec![member.private_key()]);

        let limits = TransactionParameters::default();
        let mut crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        crypto_cfg
            .allowed_signing
            .retain(|algo| *algo == Algorithm::Ed25519);

        match AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg) {
            Err(AcceptTransactionFail::SignatureVerification(fail)) => {
                assert_eq!(fail.code(), SignatureRejectionCode::AlgorithmNotPermitted);
            }
            other => panic!("expected AlgorithmNotPermitted rejection, got {other:?}"),
        }
    }

    #[test]
    fn multisig_authority_rejects_insufficient_weight() {
        let chain: ChainId = "multisig-insufficient".parse().unwrap();
        let member_a = checked_random_tx_keypair();
        let member_b = checked_random_tx_keypair();

        let members = vec![
            MultisigMember::new(member_a.public_key().clone(), 1).expect("member a"),
            MultisigMember::new(member_b.public_key().clone(), 1).expect("member b"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let authority = AccountId::new_multisig(policy);

        let mut builder = TransactionBuilder::new(
            chain.clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder = builder
            .with_instructions([Log::new(Level::INFO, "multisig insufficient weight".into())]);
        let tx = builder.sign_multisig(vec![member_a.private_key()]);

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        match AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg) {
            Err(AcceptTransactionFail::SignatureVerification(fail)) => {
                assert_eq!(fail.code(), SignatureRejectionCode::InsufficientWeight);
            }
            other => panic!("expected InsufficientWeight rejection, got {other:?}"),
        }
    }

    #[test]
    fn multisig_signature_limit_counts_bundle_entries() {
        let chain: ChainId = "multisig-signature-limit".parse().unwrap();
        let signer = checked_random_tx_keypair();

        let members = vec![MultisigMember::new(signer.public_key().clone(), 1).expect("member")];
        let policy = MultisigPolicy::new(1, members).expect("policy");
        let authority = AccountId::new_multisig(policy);

        let mut tx = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "multisig too many signatures".into())])
        .sign_multisig(vec![signer.private_key()]);

        let payload = tx.payload().clone();
        let member_signature = checked_signature_of(signer.private_key(), &payload);
        tx.set_multisig_signatures(MultisigSignatures::new(vec![
            MultisigSignature::new(signer.public_key().clone(), member_signature.clone()),
            MultisigSignature::new(signer.public_key().clone(), member_signature.clone()),
            MultisigSignature::new(signer.public_key().clone(), member_signature),
        ]));

        let defaults = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            nonzero!(2_u64),
            defaults.max_instructions(),
            defaults.ivm_bytecode_size(),
            defaults.max_tx_bytes(),
            defaults.max_decompressed_bytes(),
            defaults.max_metadata_depth(),
        );
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();

        match AcceptedTransaction::accept(tx, &chain, Duration::ZERO, limits, &crypto_cfg) {
            Err(AcceptTransactionFail::TransactionLimit(fail)) => {
                assert!(
                    fail.reason.contains("Too many signatures"),
                    "expected signature limit failure, got {:?}",
                    fail.reason
                );
            }
            other => panic!("expected signature limit rejection, got {other:?}"),
        }
    }

    #[test]
    fn accepted_transaction_into_checked_allows_pending() {
        let chain: ChainId = "checked-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let instruction = Log::new(Level::INFO, "noop".into());
        let signed = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(keypair.private_key());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(signed));

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let view = state.view();
        let checked = accepted
            .clone()
            .into_checked(&view)
            .expect("transaction should not be committed");
        assert_eq!(checked.as_ref().hash(), accepted.as_ref().hash());
    }

    #[test]
    fn accepted_transaction_into_entrypoint_consumes_wrapped_entrypoint() {
        let chain: ChainId = "accepted-into-entrypoint-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let signed = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "into-entrypoint".into())])
        .sign(keypair.private_key());
        let expected = TransactionEntrypoint::External(signed.clone());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(signed));

        assert_eq!(accepted.into_entrypoint(), expected);
    }

    #[test]
    fn accepted_transaction_into_checked_detects_committed() {
        let chain: ChainId = "checked-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let instruction = Log::new(Level::INFO, "commit".into());
        let signed = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(keypair.private_key());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(signed.clone()));

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut state_block = state.block(header);
        state_block
            .transactions
            .insert_block_with_single_tx(accepted.as_ref().hash(), nonzero!(1_usize));
        state_block.commit().expect("block commit");

        let view = state.view();
        let result = accepted.into_checked(&view);
        assert!(matches!(result, Err((_, TransactionAlreadyCommitted))));
    }

    #[test]
    fn accepted_private_entrypoint_into_checked_uses_entrypoint_hash() {
        let chain: ChainId = "checked-private-entrypoint-chain".parse().unwrap();
        let private = sample_private_kaigi_transaction(chain);
        let accepted = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(
            TransactionEntrypoint::PrivateKaigi(private),
        ));

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let view = state.view();
        let checked = accepted
            .clone()
            .into_checked(&view)
            .expect("private entrypoint should not require signed transaction access");
        assert_eq!(checked.hash(), accepted.hash());
        drop(view);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut state_block = state.block(header);
        state_block
            .transactions
            .insert_block_with_single_tx(accepted.hash(), nonzero!(1_usize));
        state_block.commit().expect("block commit");

        let view = state.view();
        let result = accepted.into_checked(&view);
        assert!(matches!(result, Err((_, TransactionAlreadyCommitted))));
    }

    #[test]
    fn accepted_transaction_caches_hashes_and_encoded_length() {
        let chain: ChainId = "accepted-cache-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let instruction = Log::new(Level::INFO, "cache".into());
        let signed = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(keypair.private_key());
        let expected_len = norito::to_bytes(&signed)
            .expect("signed transaction encodes")
            .len();

        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(signed.clone()));

        assert_eq!(accepted.hash(), signed.hash());
        assert_eq!(accepted.hash_as_entrypoint(), signed.hash_as_entrypoint());
        assert_eq!(accepted.encoded_len(), expected_len);
        let signed_bytes = accepted
            .signed_bytes()
            .expect("external transaction should cache signed bytes");
        assert_eq!(signed_bytes.as_slice(), norito::to_bytes(&signed).unwrap());
        assert_eq!(accepted.payload_hash(), Some(HashOf::new(signed.payload())));
        assert!(accepted.single_ed25519_key().is_some());
        let prepared = accepted
            .prepared_metadata()
            .expect("external transaction has prepared metadata");
        assert_eq!(prepared.signed_hash, signed.hash());
        assert_eq!(prepared.entrypoint_hash, signed.hash_as_entrypoint());
        assert_eq!(prepared.encoded_len, expected_len);
        assert!(
            prepared
                .signed_bytes
                .as_ref()
                .is_some_and(|bytes| Arc::ptr_eq(bytes, &signed_bytes))
        );
        assert_eq!(accepted.clone().encoded_len(), expected_len);
        let cloned_bytes = accepted
            .clone()
            .signed_bytes()
            .expect("clone should preserve signed bytes");
        assert!(Arc::ptr_eq(&signed_bytes, &cloned_bytes));
    }

    #[test]
    fn single_ed25519_fast_path_requires_ed25519_key_and_signature_shape() {
        let chain: ChainId = "single-ed25519-fast-path-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let signed = TransactionBuilder::new(
            chain.clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "ed25519-fast-path".into())])
        .sign(keypair.private_key());

        assert!(AcceptedTransaction::has_single_ed25519_signature(&signed));
        assert!(AcceptedTransaction::parsed_single_ed25519_key(&signed).is_some());

        let secp_keypair = checked_random_tx_keypair_with_algorithm(Algorithm::Secp256k1);
        let secp_authority = AccountId::new(secp_keypair.public_key().clone());
        let secp_signed = TransactionBuilder::new(
            chain.clone(),
            secp_authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "secp256k1-fast-path".into())])
        .sign(secp_keypair.private_key());

        assert!(!AcceptedTransaction::has_single_ed25519_signature(
            &secp_signed
        ));
        assert!(AcceptedTransaction::parsed_single_ed25519_key(&secp_signed).is_none());

        let mut short_signature_tx = signed.clone();
        let short_signature = iroha_crypto::Signature::from_bytes(&[0_u8; 1]);
        short_signature_tx.set_signature(TransactionSignature(
            iroha_crypto::SignatureOf::from_signature(short_signature),
        ));

        assert!(!AcceptedTransaction::has_single_ed25519_signature(
            &short_signature_tx
        ));
        assert!(AcceptedTransaction::parsed_single_ed25519_key(&short_signature_tx).is_none());
    }

    #[test]
    fn borrowed_external_entrypoint_hash_matches_canonical_hash() {
        let chain: ChainId = "accepted-borrowed-entrypoint-hash-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let signed = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "borrowed-hash".into())])
        .sign(keypair.private_key());

        assert_eq!(
            AcceptedTransaction::external_entrypoint_hash_from_signed(&signed),
            signed.hash_as_entrypoint()
        );
    }

    #[test]
    fn signed_frame_entrypoint_hash_matches_canonical_hash() {
        let chain: ChainId = "accepted-signed-frame-entrypoint-hash-chain"
            .parse()
            .unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let signed = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "signed-frame-hash".into())])
        .sign(keypair.private_key());
        let signed_bytes =
            norito::encode_canonical(&signed).expect("signed transaction encodes canonically");

        assert_eq!(
            AcceptedTransaction::external_entrypoint_hash_from_signed_frame(&signed_bytes)
                .expect("signed frame hashes"),
            signed.hash_as_entrypoint()
        );

        let mut corrupted = signed_bytes.clone();
        let payload_start = norito::core::Header::SIZE
            + AcceptedTransaction::framed_padding_for::<SignedTransaction>();
        corrupted[payload_start] ^= 0x01;
        assert!(matches!(
            AcceptedTransaction::external_entrypoint_hash_from_signed_frame(&corrupted),
            Err(norito::core::Error::ChecksumMismatch)
        ));

        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&signed).expect("encode alternate-layout signed transaction")
        };
        assert_ne!(alternate, signed_bytes);
        norito::decode_from_bytes::<SignedTransaction>(&alternate)
            .expect("ordinary Norito accepts its advertised layout");
        assert!(matches!(
            AcceptedTransaction::external_entrypoint_hash_from_signed_frame(&alternate),
            Err(norito::core::Error::NonCanonicalEncoding)
        ));
    }

    #[test]
    fn prepared_governance_transaction_hash_matches_canonical_entrypoint_hash() {
        let chain: ChainId = "accepted-governance-entrypoint-hash-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let manifest = RuntimeUpgradeManifest {
            name: "runtime.upgrade.hash.test".into(),
            description: "runtime upgrade hash fixture".into(),
            abi_version: 1,
            abi_hash: [7; 32],
            added_syscalls: Vec::new(),
            added_pointer_types: Vec::new(),
            start_height: 42,
            end_height: 84,
            sbom_digests: Vec::new(),
            slsa_attestation: Vec::new(),
            provenance: Vec::new(),
        };
        let signed = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([ProposeRuntimeUpgradeProposal {
            manifest,
            window: None,
            mode: Some(VotingMode::Plain),
        }])
        .sign(keypair.private_key());
        let prepared = AcceptedTransaction::prepare_signed_metadata(&signed);

        assert_eq!(prepared.entrypoint_hash, signed.hash_as_entrypoint());
        assert_eq!(prepared.signed_hash, signed.hash());
    }

    #[test]
    fn accept_with_canonical_signed_bytes_reuses_payload_cache() {
        let chain: ChainId = "accepted-canonical-cache-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let signed = TransactionBuilder::new(
            chain.clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "canonical-cache".into())])
        .sign(keypair.private_key());
        let (signed_payload, signed_payload_flags) =
            AcceptedTransaction::canonical_signed_payload_with_flags(&signed);
        let signed_bytes = Arc::new(
            norito::core::frame_bare_with_header_flags::<SignedTransaction>(
                &signed_payload,
                signed_payload_flags,
            )
            .expect("signed transaction encodes"),
        );
        let expected_entrypoint_bytes = Arc::new(
            norito::encode_canonical(&TransactionEntrypoint::External(signed.clone()))
                .expect("external entrypoint encodes canonically"),
        );
        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();

        let accepted = AcceptedTransaction::accept_with_canonical_signed_bytes(
            signed,
            Arc::clone(&signed_bytes),
            &chain,
            Duration::ZERO,
            limits,
            &crypto_cfg,
        )
        .expect("accepted transaction");

        let cached = accepted.signed_bytes().expect("canonical bytes cached");
        assert!(Arc::ptr_eq(&cached, &signed_bytes));
        assert_eq!(
            accepted.entrypoint_bytes().as_slice(),
            expected_entrypoint_bytes.as_slice()
        );
    }

    #[test]
    fn decoded_versioned_signed_transaction_prepares_exact_length_from_payload() {
        let chain: ChainId = "decoded-versioned-cache-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let signed = TransactionBuilder::new(
            chain.clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "versioned-cache".into())])
        .sign(keypair.private_key());
        let expected_len = norito::to_bytes(&signed)
            .expect("signed transaction encodes")
            .len();
        let expected_entrypoint_bytes =
            norito::encode_canonical(&TransactionEntrypoint::External(signed.clone()))
                .expect("entrypoint transaction encodes canonically");
        let versioned =
            <SignedTransaction as iroha_version::codec::EncodeVersioned>::encode_versioned(&signed);

        let decoded = DecodedVersionedSignedTransaction::decode_versioned(&versioned)
            .expect("versioned signed transaction decodes");

        assert_eq!(decoded.hash(), signed.hash());
        assert_eq!(decoded.hash_as_entrypoint(), signed.hash_as_entrypoint());
        assert_eq!(decoded.encoded_len(), expected_len);
        assert_eq!(decoded.prepared.encoded_len, expected_len);
        assert_eq!(
            decoded
                .prepared
                .signed_bytes
                .as_ref()
                .expect("canonical signed bytes are seeded from ingress")
                .as_slice(),
            norito::encode_canonical(&signed).unwrap().as_slice()
        );
        assert_eq!(
            decoded
                .prepared
                .entrypoint_bytes
                .as_ref()
                .expect("canonical entrypoint bytes are seeded from ingress")
                .as_slice(),
            expected_entrypoint_bytes.as_slice()
        );
        assert_eq!(decoded.prepared.payload_hash, HashOf::new(signed.payload()));
        assert!(decoded.prepared.single_ed25519_key.is_some());

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted = decoded
            .into_accepted(&chain, Duration::ZERO, limits, &crypto_cfg)
            .expect("decoded transaction accepts");

        assert_eq!(accepted.hash(), signed.hash());
        assert_eq!(accepted.hash_as_entrypoint(), signed.hash_as_entrypoint());
        assert_eq!(accepted.encoded_len(), expected_len);
        assert_eq!(
            accepted.entrypoint_bytes().as_slice(),
            expected_entrypoint_bytes.as_slice()
        );
        assert_eq!(accepted.payload_hash(), Some(HashOf::new(signed.payload())));
        assert!(accepted.single_ed25519_key().is_some());
    }

    #[test]
    fn decoded_versioned_signed_transaction_normalizes_adaptive_payload_metadata() {
        let chain: ChainId = "decoded-versioned-adaptive-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let asset_def_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "adaptivezk".parse().expect("asset name"),
        );
        let signed = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(
            iroha_data_model::isi::zk::Shield::new(
                asset_def_id,
                authority,
                200_u128,
                [7; 32],
                iroha_data_model::confidential::ConfidentialEncryptedPayload::default(),
            ),
        )])
        .sign(keypair.private_key());
        let actual_payload_len = norito::codec::Encode::encode(&signed).len();
        assert!(
            norito::core::NoritoSerialize::encoded_len_exact(&signed).is_none(),
            "adaptive confidential payload must not advertise an exact encoded length"
        );
        let canonical_len = norito::to_bytes(&signed)
            .expect("signed transaction encodes")
            .len();
        let versioned =
            <SignedTransaction as iroha_version::codec::EncodeVersioned>::encode_versioned(&signed);

        assert_eq!(versioned.len().saturating_sub(1), actual_payload_len);
        assert_eq!(
            AcceptedTransaction::signed_encoded_len(&signed),
            canonical_len
        );

        let decoded = DecodedVersionedSignedTransaction::decode_versioned(&versioned)
            .expect("versioned signed transaction decodes");

        assert_eq!(decoded.signed(), &signed);
        assert_eq!(
            AcceptedTransaction::external_entrypoint_hash_from_signed(&signed),
            signed.hash_as_entrypoint()
        );
        assert_eq!(decoded.hash(), signed.hash());
        assert_eq!(decoded.hash_as_entrypoint(), signed.hash_as_entrypoint());
        assert_eq!(decoded.encoded_len(), canonical_len);
        assert_eq!(decoded.prepared.encoded_len, canonical_len);
        assert!(
            decoded.prepared.signed_bytes.is_some(),
            "canonical adaptive ingress payload should seed signed bytes"
        );
        assert!(
            decoded.prepared.entrypoint_bytes.is_some(),
            "canonical adaptive ingress payload should seed entrypoint bytes"
        );
    }

    #[test]
    fn decoded_versioned_signed_transaction_owned_supports_ed25519_prechecked_accept() {
        let chain: ChainId = "decoded-versioned-owned-precheck-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let signed = TransactionBuilder::new(
            chain.clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "versioned-owned-precheck".into())])
        .sign(keypair.private_key());
        let expected_len = norito::to_bytes(&signed)
            .expect("signed transaction encodes")
            .len();
        let versioned =
            <SignedTransaction as iroha_version::codec::EncodeVersioned>::encode_versioned(&signed);

        let decoded = DecodedVersionedSignedTransaction::decode_versioned_owned(versioned)
            .expect("owned versioned signed transaction decodes");
        let (message, signature, _public_key) = decoded
            .single_ed25519_precheck_parts()
            .expect("single Ed25519 transaction exposes precheck parts");

        assert_eq!(message, HashOf::new(signed.payload()).as_ref().as_slice());
        assert_eq!(signature, signed.signature().payload().payload());
        assert_eq!(decoded.encoded_len(), expected_len);

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted = decoded
            .into_accepted_after_single_ed25519_precheck(
                &chain,
                Duration::ZERO,
                limits,
                &crypto_cfg,
            )
            .expect("prechecked decoded transaction accepts");

        assert_eq!(accepted.hash(), signed.hash());
        assert_eq!(accepted.encoded_len(), expected_len);
    }

    #[test]
    fn every_prechecked_signature_path_rejects_invalid_fee_intent() {
        let chain: ChainId = "prechecked-fee-intent-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let mut metadata = Metadata::default();
        metadata.insert(
            "fee_sponsor".parse().expect("valid metadata key"),
            Json::new("retired".to_owned()),
        );
        let builder = TransactionBuilder::new(
            chain.clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "invalid fee intent".into())])
        .with_metadata(metadata);
        let signature = Signature::try_new(keypair.private_key(), &builder.payload_hash_bytes())
            .expect("fixture signature");
        let signed = builder.build_with_signature(signature);
        let prepared = AcceptedTransaction::prepare_signed_metadata(&signed);
        let crypto = iroha_config::parameters::actual::Crypto::default();
        let now = signed.creation_time();

        let assert_rejected = |result: Result<(), AcceptTransactionFail>, path: &str| {
            let error = result.expect_err(path);
            match error {
                AcceptTransactionFail::SignatureVerification(failure) => {
                    assert_eq!(failure.code(), SignatureRejectionCode::MalformedSignature);
                    assert!(
                        failure.detail.contains("fee_sponsor"),
                        "{path} returned an unrelated rejection: {failure}"
                    );
                }
                other => panic!("{path} returned an unrelated rejection: {other:?}"),
            }
        };

        assert_rejected(
            AcceptedTransaction::validate_with_now_with_signature_result_and_prepared_metadata(
                &signed,
                &chain,
                Duration::ZERO,
                TransactionParameters::default(),
                &crypto,
                now,
                Some(Ok(())),
                &prepared,
            ),
            "batch signature override",
        );
        assert_rejected(
            AcceptedTransaction::validate_with_now_after_single_ed25519_precheck_and_prepared_metadata(
                &signed,
                &chain,
                Duration::ZERO,
                TransactionParameters::default(),
                &crypto,
                now,
                &prepared,
            ),
            "single Ed25519 precheck",
        );
        assert_rejected(
            AcceptedTransaction::validate_with_now_and_prepared_metadata(
                &signed,
                &chain,
                Duration::ZERO,
                TransactionParameters::default(),
                &crypto,
                now,
                &prepared,
            ),
            "prepared deterministic verification",
        );
    }

    #[test]
    fn decoded_versioned_signed_transaction_rejects_malformed_payloads() {
        let chain: ChainId = "decoded-versioned-reject-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let signed = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "versioned-reject".into())])
        .sign(keypair.private_key());
        let mut versioned =
            <SignedTransaction as iroha_version::codec::EncodeVersioned>::encode_versioned(&signed);

        assert!(DecodedVersionedSignedTransaction::decode_versioned(&[]).is_err());

        let mut unsupported = versioned.clone();
        unsupported[0] = 0x7f;
        assert!(DecodedVersionedSignedTransaction::decode_versioned(&unsupported).is_err());

        versioned.push(0);
        assert!(DecodedVersionedSignedTransaction::decode_versioned(&versioned).is_err());
    }

    #[test]
    fn accepted_transaction_entrypoint_encoded_lengths_match_norito_frames() {
        let chain: ChainId = "accepted-entrypoint-len-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let signed = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "entrypoint-len".into())])
        .sign(keypair.private_key());
        let signed_expected_len = norito::to_bytes(&signed)
            .expect("signed transaction encodes")
            .len();

        assert_eq!(
            AcceptedTransaction::framed_encoded_len(&signed),
            signed_expected_len
        );
        assert_eq!(
            AcceptedTransaction::new_unchecked(Cow::Owned(signed.clone())).encoded_len(),
            signed_expected_len
        );
        assert_eq!(
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(
                TransactionEntrypoint::External(signed)
            ))
            .encoded_len(),
            signed_expected_len
        );

        let time_entrypoint = TimeTriggerEntrypoint {
            id: "accepted-entrypoint-len-trigger".parse().unwrap(),
            instructions: ExecutionStep(ConstVec::from(Vec::<InstructionBox>::new())),
            authority,
        };
        let time_expected_len = norito::to_bytes(&time_entrypoint)
            .expect("time entrypoint encodes")
            .len();

        assert_eq!(
            AcceptedTransaction::framed_encoded_len(&time_entrypoint),
            time_expected_len
        );
        assert_eq!(
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(TransactionEntrypoint::Time(
                time_entrypoint
            )))
            .encoded_len(),
            time_expected_len
        );

        let private_entrypoint = sample_private_kaigi_transaction(chain);
        let private_expected_len = norito::to_bytes(&private_entrypoint)
            .expect("private Kaigi entrypoint encodes")
            .len();

        assert_eq!(
            AcceptedTransaction::framed_encoded_len(&private_entrypoint),
            private_expected_len
        );
        assert_eq!(
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(
                TransactionEntrypoint::PrivateKaigi(private_entrypoint)
            ))
            .encoded_len(),
            private_expected_len
        );
    }

    #[test]
    fn signed_encoded_len_matches_norito_for_optional_metadata_shapes() {
        let chain: ChainId = "signed-len-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("signed-len-tag").expect("name"),
            Json::from("cached"),
        );

        let mut builder = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "signed-len".into())])
        .with_metadata(metadata);
        builder.set_nonce(NonZeroU32::new(7).expect("nonce"));
        builder.set_ttl(Duration::from_millis(5_000));
        let signed = builder.sign(keypair.private_key());
        let expected_len = norito::to_bytes(&signed)
            .expect("signed transaction encodes")
            .len();

        assert!(
            norito::NoritoSerialize::encoded_len_exact(&signed).is_some(),
            "representative signed transaction should have an exact encoded length"
        );
        assert_eq!(
            AcceptedTransaction::signed_encoded_len(&signed),
            expected_len
        );

        let prepared = AcceptedTransaction::prepare_signed_metadata(&signed);
        assert_eq!(prepared.encoded_len, expected_len);
    }

    #[test]
    fn prepared_metadata_depth_matches_direct_depth_check() {
        let chain: ChainId = "prepared-depth-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("depth").expect("name"),
            Json::from_str_norito("[[[1]]]").expect("valid nested json"),
        );
        let signed = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "prepared-depth".into())])
        .with_metadata(metadata)
        .sign(keypair.private_key());
        let prepared = AcceptedTransaction::prepare_signed_metadata(&signed);

        ensure_metadata_depth_with_prepared(signed.metadata(), 4, Some(&prepared))
            .expect("prepared metadata depth should accept equal max depth");
        assert_eq!(
            ensure_metadata_depth_with_prepared(signed.metadata(), 3, Some(&prepared))
                .expect_err("prepared metadata depth should reject too deep metadata"),
            ensure_metadata_depth(signed.metadata(), 3)
                .expect_err("direct metadata depth should reject too deep metadata")
        );
    }

    #[test]
    fn gossip_signed_metadata_matches_canonical_preparation() {
        let chain: ChainId = "gossip-signed-metadata-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let signed = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "gossip-metadata".into())])
        .sign(keypair.private_key());
        let entrypoint = TransactionEntrypoint::External(signed.clone());
        let entrypoint_bytes =
            norito::encode_canonical(&entrypoint).expect("entrypoint transaction encodes");

        let expected = AcceptedTransaction::prepare_signed_metadata(&signed);
        let actual = AcceptedTransaction::prepare_gossip_signed_metadata(
            &signed,
            entrypoint.hash(),
            Arc::new(entrypoint_bytes.clone()),
        );

        assert_eq!(actual.signed_hash, expected.signed_hash);
        assert_eq!(actual.entrypoint_hash, expected.entrypoint_hash);
        assert_eq!(actual.payload_hash, expected.payload_hash);
        assert_eq!(actual.encoded_len, expected.encoded_len);
        assert!(actual.signed_bytes.is_none());
        assert_eq!(
            actual
                .entrypoint_bytes
                .as_ref()
                .expect("gossip metadata keeps canonical entrypoint bytes")
                .as_slice(),
            entrypoint_bytes.as_slice()
        );
        assert_eq!(
            actual.single_ed25519_key.is_some(),
            expected.single_ed25519_key.is_some()
        );
    }

    #[test]
    fn signed_encoded_len_for_limit_uses_cached_canonical_bytes() {
        let chain: ChainId = "signed-len-cache-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let signed = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "signed-len-cache".into())])
        .sign(keypair.private_key());
        let signed_bytes = Arc::new(norito::to_bytes(&signed).expect("signed transaction encodes"));
        let mut prepared = AcceptedTransaction::prepare_signed_metadata(&signed);
        prepared.encoded_len = usize::MAX;
        prepared.signed_bytes = Some(Arc::clone(&signed_bytes));

        assert_eq!(
            AcceptedTransaction::signed_encoded_len_for_limit_with_prepared(
                &signed,
                Some(&prepared)
            ),
            u64::try_from(signed_bytes.len()).expect("length fits in u64")
        );
    }

    fn sample_private_kaigi_transaction(chain: ChainId) -> PrivateKaigiTransaction {
        PrivateKaigiTransaction {
            chain,
            creation_time_ms: 42,
            nonce: Some(NonZeroU32::new(7).expect("nonce")),
            metadata: Metadata::default(),
            action: PrivateKaigiAction::Create(PrivateCreateKaigi {
                call: PrivateKaigiTemplate {
                    id: KaigiId::new(
                        DomainId::try_new("kaigi", "universal").expect("domain"),
                        Name::from_str("private-room").expect("call"),
                    ),
                    title: Some("Private".to_owned()),
                    description: None,
                    max_participants: Some(2),
                    gas_rate_per_minute: 5,
                    metadata: Metadata::default(),
                    scheduled_start_ms: None,
                    privacy_mode: KaigiPrivacyMode::ZkRosterV1,
                    room_policy: KaigiRoomPolicy::Authenticated,
                    relay_manifest: None,
                },
            }),
            artifacts: PrivateKaigiArtifacts {
                commitment: KaigiParticipantCommitment {
                    commitment: Hash::new(b"host-commitment"),
                    alias_tag: Some("host".to_owned()),
                },
                nullifier: KaigiParticipantNullifier {
                    digest: Hash::new(b"private-kaigi-nullifier"),
                    issued_at_ms: 42,
                },
                roster_root: Hash::new(b"roster-root"),
                proof: vec![0xAA, 0xBB, 0xCC],
            },
            fee_spend: PrivateKaigiFeeSpend {
                asset_definition_id: AssetDefinitionId::new(
                    DomainId::try_new("wonderland", "universal").expect("domain"),
                    Name::from_str("xor").expect("name"),
                ),
                anchor_root: Hash::new(b"anchor-root"),
                nullifiers: vec![[0x11; 32]],
                output_commitments: vec![[0x22; 32]],
                encrypted_change_payloads: vec![vec![0x33, 0x44]],
                proof: vec![0x55, 0x66],
            },
        }
    }

    #[test]
    fn private_kaigi_fee_transfer_proof_canonicalization_strips_only_aux() {
        let aux = br#"{"schema":"iroha.private_kaigi.fee.v1","action_hash_hex":"abcd","chain_id":"private-kaigi-chain","asset_definition_id":"xor#wonderland","fee_amount":"5"}"#;
        let envelope = OpenVerifyEnvelope {
            backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            circuit_id: crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID.to_owned(),
            vk_hash: [0x42; 32],
            public_inputs:
                crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1
                    .to_vec(),
            proof_bytes: vec![0xCA, 0xFE, 0xBA, 0xBE],
            aux: aux.to_vec(),
        };
        let proof_bytes = norito::encode_canonical(&envelope).expect("encode fee envelope");

        let binding =
            super::decode_private_kaigi_fee_binding(&proof_bytes).expect("binding decodes");
        assert_eq!(binding.action_hash_hex, "abcd");
        assert_eq!(binding.chain_id, "private-kaigi-chain");
        assert_eq!(binding.asset_definition_id, "xor#wonderland");
        assert_eq!(binding.fee_amount, Quantity::from(5_u32));

        let canonical = super::canonical_private_kaigi_fee_transfer_proof(&proof_bytes)
            .expect("canonicalize fee proof");
        let decoded: OpenVerifyEnvelope =
            norito::decode_canonical(&canonical).expect("decode canonical envelope");
        assert_eq!(decoded.backend, envelope.backend);
        assert_eq!(decoded.circuit_id, envelope.circuit_id);
        assert_eq!(decoded.vk_hash, envelope.vk_hash);
        assert_eq!(decoded.public_inputs, envelope.public_inputs);
        assert_eq!(decoded.proof_bytes, envelope.proof_bytes);
        assert!(
            decoded.aux.is_empty(),
            "internal ZkTransfer proof must not carry fee-binding aux"
        );

        let err = super::decode_private_kaigi_fee_binding(&canonical)
            .expect_err("canonical internal transfer proof should no longer carry fee binding");
        match err {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => assert!(
                msg.contains("missing binding metadata"),
                "unexpected message: {msg}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn private_kaigi_fee_binding_rejects_alternate_layout_outer() {
        let aux = br#"{"schema":"iroha.private_kaigi.fee.v1","action_hash_hex":"abcd","chain_id":"private-kaigi-chain","asset_definition_id":"xor#wonderland","fee_amount":"5"}"#;
        let envelope = OpenVerifyEnvelope {
            backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            circuit_id: crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID.to_owned(),
            vk_hash: [0x42; 32],
            public_inputs:
                crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1
                    .to_vec(),
            proof_bytes: vec![0xCA, 0xFE, 0xBA, 0xBE],
            aux: aux.to_vec(),
        };
        let canonical = norito::encode_canonical(&envelope).expect("encode canonical fee envelope");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&envelope).expect("encode alternate-layout fee envelope")
        };
        assert_ne!(alternate, canonical);
        norito::decode_from_bytes::<OpenVerifyEnvelope>(&alternate)
            .expect("ordinary Norito accepts the advertised layout");

        for err in [
            super::decode_private_kaigi_fee_binding(&alternate)
                .expect_err("alternate-layout fee binding must fail closed"),
            super::canonical_private_kaigi_fee_transfer_proof(&alternate)
                .expect_err("alternate-layout fee proof must not be normalized"),
        ] {
            match err {
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
                    assert!(
                        msg.contains("must use OpenVerifyEnvelope payload"),
                        "unexpected semantic classification: {msg}"
                    );
                }
                other => panic!("unexpected error classification: {other:?}"),
            }
        }
    }

    #[test]
    fn private_kaigi_fee_binding_rejects_negative_amount() {
        let aux = br#"{"schema":"iroha.private_kaigi.fee.v1","action_hash_hex":"abcd","chain_id":"private-kaigi-chain","asset_definition_id":"xor#wonderland","fee_amount":"-1"}"#;
        let envelope = OpenVerifyEnvelope {
            backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            circuit_id: crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID.to_owned(),
            vk_hash: [0x42; 32],
            public_inputs:
                crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1
                    .to_vec(),
            proof_bytes: vec![0xCA, 0xFE, 0xBA, 0xBE],
            aux: aux.to_vec(),
        };
        let proof_bytes =
            norito::encode_canonical(&envelope).expect("encode negative fee envelope");

        let err = super::decode_private_kaigi_fee_binding(&proof_bytes)
            .expect_err("negative private Kaigi fee amount must fail at the nominal boundary");
        match err {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => assert!(
                msg.contains("private Kaigi fee amount is invalid"),
                "unexpected message: {msg}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn private_kaigi_fee_binding_rejects_noncanonical_amount_text() {
        for amount in ["+1", "01", "1.0", "123.4500", " 1 "] {
            let aux = format!(
                r#"{{"schema":"iroha.private_kaigi.fee.v1","action_hash_hex":"abcd","chain_id":"private-kaigi-chain","asset_definition_id":"xor#wonderland","fee_amount":"{amount}"}}"#
            );
            let envelope = OpenVerifyEnvelope {
                backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
                circuit_id: crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
                    .to_owned(),
                vk_hash: [0x42; 32],
                public_inputs:
                    crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1
                        .to_vec(),
                proof_bytes: vec![0xCA, 0xFE, 0xBA, 0xBE],
                aux: aux.into_bytes(),
            };
            let proof_bytes =
                norito::encode_canonical(&envelope).expect("encode noncanonical fee envelope");

            let err = super::decode_private_kaigi_fee_binding(&proof_bytes)
                .expect_err("noncanonical private Kaigi fee text must fail closed");
            match err {
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
                    assert!(
                        msg.contains("private Kaigi fee amount must use canonical form"),
                        "unexpected message for `{amount}`: {msg}"
                    );
                }
                other => panic!("unexpected error for `{amount}`: {other:?}"),
            }
        }
    }

    #[test]
    fn private_kaigi_fee_payer_account_uses_checked_ed25519_derivation() {
        let tx = sample_private_kaigi_transaction("private-kaigi-chain".parse().expect("chain id"));
        let fee_payer = AcceptedTransaction::private_kaigi_fee_payer_account(&tx)
            .expect("checked private Kaigi fee payer derivation");
        let seed = Hash::new(tx.action_hash().as_ref());
        let expected_keypair = KeyPair::try_from_seed(seed.as_ref().to_vec(), Algorithm::Ed25519)
            .expect("expected checked fee payer seed derivation");

        assert_eq!(
            fee_payer,
            AccountId::new(expected_keypair.public_key().clone())
        );
    }

    #[test]
    fn fraud_policy_allows_when_disabled() {
        let cfg = iroha_config::parameters::actual::FraudMonitoring::default();
        let metadata = Metadata::default();
        let catalog = DataSpaceCatalog::default();
        let assignment = single_lane_assignment(&catalog);
        assert!(super::enforce_fraud_policy(&cfg, &metadata, None, &assignment).is_ok());
    }

    #[test]
    fn fraud_policy_rejects_missing_assessment() {
        let cfg = iroha_config::parameters::actual::FraudMonitoring {
            enabled: true,
            required_minimum_band: Some(iroha_config::parameters::actual::FraudRiskBand::High),
            ..Default::default()
        };
        let metadata = iroha_data_model::metadata::Metadata::default();
        let catalog = DataSpaceCatalog::default();
        let assignment = single_lane_assignment(&catalog);
        let result = super::enforce_fraud_policy(&cfg, &metadata, None, &assignment);
        assert!(matches!(
            result,
            Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(_)
            ))
        ));
    }

    #[test]
    fn accept_with_time_source_uses_mock_clock() {
        let (authority, keypair) = gen_account_in("wonderland");
        let chain: ChainId = "mock-clock-chain".parse().expect("chain id");
        let (handle, time_source) = TimeSource::new_mock(Duration::from_secs(5));
        let mut builder = TransactionBuilder::new_with_time_source(
            chain.clone(),
            authority.clone(),
            &time_source,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "ttl ok".to_owned())])
        .with_metadata(Metadata::default());
        builder.set_ttl(Duration::from_secs(10));
        let signed = builder.sign(keypair.private_key());
        let default_limits = TransactionParameters::default();
        let tx_limits = TransactionParameters::with_max_signatures(
            nonzero!(1_u64),
            nonzero!(16_u64),
            nonzero!(2048_u64),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        handle.advance(Duration::from_secs(1));
        AcceptedTransaction::accept_with_time_source(
            signed.clone(),
            &chain,
            Duration::from_secs(0),
            tx_limits,
            &crypto_cfg,
            &time_source,
        )
        .expect("transaction should be accepted with mock clock");
        let err = AcceptedTransaction::accept(
            signed,
            &chain,
            Duration::from_secs(0),
            tx_limits,
            &crypto_cfg,
        )
        .expect_err("system clock should see TTL expired relative to mock timestamp");
        assert!(matches!(
            err,
            AcceptTransactionFail::TransactionExpired { .. }
        ));
    }

    #[test]
    fn stateless_admission_rejects_missing_signature_bound_ttl() {
        let (authority, keypair) = gen_account_in("wonderland");
        let chain: ChainId = "required-ttl-chain".parse().expect("chain id");
        let signed = TransactionBuilder::new(
            chain.clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "required ttl".to_owned())])
        .sign(keypair.private_key());
        let encoded = norito::json::to_json(&signed).expect("serialize transaction");
        let ttl_field = format!(
            "\"time_to_live_ms\":{}",
            iroha_data_model::transaction::DEFAULT_TRANSACTION_TIME_TO_LIVE.as_millis()
        );
        assert!(
            encoded.contains(&ttl_field),
            "fixture must carry the builder's signature-bound TTL: {encoded}"
        );
        let malformed_json = encoded.replacen(&ttl_field, "\"time_to_live_ms\":null", 1);
        let malformed: SignedTransaction =
            norito::json::from_str(&malformed_json).expect("decode malformed wire fixture");

        let error = AcceptedTransaction::validate_with_now(
            &malformed,
            &chain,
            Duration::ZERO,
            TransactionParameters::default(),
            &iroha_config::parameters::actual::Crypto::default(),
            malformed.creation_time(),
        )
        .expect_err("missing TTL must never be stateless-valid");
        match error {
            AcceptTransactionFail::TransactionLimit(limit) => assert!(
                limit.reason.contains("time_to_live_ms") && limit.reason.contains("required"),
                "unexpected missing-TTL reason: {limit:?}"
            ),
            other => panic!("expected TransactionLimit for missing TTL, got {other:?}"),
        }
    }

    #[test]
    fn stateless_admission_enforces_governed_maximum_ttl() {
        let (authority, keypair) = gen_account_in("wonderland");
        let chain: ChainId = "bounded-ttl-chain".parse().expect("chain id");
        let mut builder = TransactionBuilder::new(
            chain.clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "bounded ttl".to_owned())]);
        builder.set_ttl(Duration::from_millis(5_001));
        let signed = builder.sign(keypair.private_key());
        let limits = TransactionParameters::default().with_max_time_to_live_ms(nonzero!(5_000_u64));

        let error = AcceptedTransaction::validate_with_now(
            &signed,
            &chain,
            Duration::ZERO,
            limits,
            &iroha_config::parameters::actual::Crypto::default(),
            signed.creation_time(),
        )
        .expect_err("TTL above the governed maximum must be rejected");
        match error {
            AcceptTransactionFail::TransactionLimit(limit) => assert!(
                limit.reason.contains("5001") && limit.reason.contains("5000"),
                "unexpected maximum-TTL reason: {limit:?}"
            ),
            other => panic!("expected TransactionLimit for excessive TTL, got {other:?}"),
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn time_sensitive_instruction_detects_governance_and_non_sensitive() {
        let (authority, _keypair) = gen_account_in("wonderland");
        let (counterparty, _keypair) = gen_account_in("wonderland");
        let ballot = iroha_data_model::isi::governance::CastPlainBallot {
            referendum_id: "ref-1".into(),
            owner: authority.clone(),
            amount: 1_u64.into(),
            duration_blocks: 1,
            direction: 0,
        };
        let ballot_box = InstructionBox::from(ballot);
        assert!(super::is_time_sensitive_instruction(&ballot_box));

        let agreement_id: iroha_data_model::repo::RepoAgreementId =
            "repo-1".parse().expect("repo id");
        let cash_leg = iroha_data_model::repo::RepoCashLeg {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "usd".parse().unwrap(),
            ),
            quantity: 1u32.into(),
        };
        let collateral_leg = iroha_data_model::repo::RepoCollateralLeg::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "bond".parse().unwrap(),
            ),
            1u32,
        );
        let governance = iroha_data_model::repo::RepoGovernance::with_defaults(100, 3600);
        let repo = iroha_data_model::isi::repo::RepoIsi::new(
            agreement_id.clone(),
            counterparty.clone(),
            authority.clone(),
            None,
            cash_leg.clone(),
            collateral_leg.clone(),
            250,
            1_700_000_000_000,
            governance,
        );
        assert!(super::is_time_sensitive_instruction(&InstructionBox::from(
            repo
        )));
        let reverse = iroha_data_model::isi::repo::ReverseRepoIsi::new(agreement_id.clone());
        assert!(super::is_time_sensitive_instruction(&InstructionBox::from(
            reverse
        )));
        let margin_call = iroha_data_model::isi::repo::RepoMarginCallIsi::new(agreement_id.clone());
        assert!(super::is_time_sensitive_instruction(&InstructionBox::from(
            margin_call
        )));

        let exit = iroha_data_model::isi::staking::ExitPublicLaneValidator {
            lane_id: TestLaneId::SINGLE,
            validator: counterparty.clone(),
            release_at_ms: 1_700_000_000_000,
        };
        assert!(super::is_time_sensitive_instruction(&InstructionBox::from(
            exit
        )));
        let request_id = iroha_crypto::Hash::new("unbond");
        let unbond = iroha_data_model::isi::staking::SchedulePublicLaneUnbond {
            lane_id: TestLaneId::SINGLE,
            validator: counterparty.clone(),
            staker: counterparty.clone(),
            request_id,
            amount: 1u32.into(),
            release_at_ms: 1_700_000_000_000,
        };
        assert!(super::is_time_sensitive_instruction(&InstructionBox::from(
            unbond
        )));
        let finalize = iroha_data_model::isi::staking::FinalizePublicLaneUnbond {
            lane_id: TestLaneId::SINGLE,
            validator: counterparty.clone(),
            staker: counterparty.clone(),
            request_id,
        };
        assert!(super::is_time_sensitive_instruction(&InstructionBox::from(
            finalize
        )));

        let settlement_id: iroha_data_model::isi::settlement::SettlementId =
            "settlement-1".parse().expect("settlement id");
        let dvp = iroha_data_model::isi::settlement::DvpIsi::new(
            settlement_id.clone(),
            iroha_data_model::isi::settlement::SettlementLeg::new(
                iroha_data_model::asset::AssetDefinitionId::new(
                    DomainId::try_new("wonderland", "universal").unwrap(),
                    "bond".parse().unwrap(),
                ),
                1u32,
                counterparty.clone(),
                authority.clone(),
            ),
            iroha_data_model::isi::settlement::SettlementLeg::new(
                iroha_data_model::asset::AssetDefinitionId::new(
                    DomainId::try_new("wonderland", "universal").unwrap(),
                    "usd".parse().unwrap(),
                ),
                1u32,
                authority.clone(),
                counterparty.clone(),
            ),
            iroha_data_model::isi::settlement::SettlementPlan::default(),
        );
        assert!(super::is_time_sensitive_instruction(&InstructionBox::from(
            dvp
        )));
        let pvp = iroha_data_model::isi::settlement::PvpIsi::new(
            settlement_id,
            iroha_data_model::isi::settlement::SettlementLeg::new(
                iroha_data_model::asset::AssetDefinitionId::new(
                    DomainId::try_new("wonderland", "universal").unwrap(),
                    "eur".parse().unwrap(),
                ),
                1u32,
                counterparty.clone(),
                authority.clone(),
            ),
            iroha_data_model::isi::settlement::SettlementLeg::new(
                iroha_data_model::asset::AssetDefinitionId::new(
                    DomainId::try_new("wonderland", "universal").unwrap(),
                    "usd".parse().unwrap(),
                ),
                1u32,
                authority.clone(),
                counterparty.clone(),
            ),
            iroha_data_model::isi::settlement::SettlementPlan::default(),
        );
        assert!(super::is_time_sensitive_instruction(&InstructionBox::from(
            pvp
        )));

        let trigger_id: iroha_data_model::trigger::TriggerId =
            "nts-trigger".parse().expect("trigger id");
        let execute_trigger = iroha_data_model::isi::ExecuteTrigger::new(trigger_id);
        assert!(super::is_time_sensitive_instruction(&InstructionBox::from(
            execute_trigger
        )));

        let log_box = InstructionBox::from(Log::new(Level::INFO, "ok".into()));
        assert!(!super::is_time_sensitive_instruction(&log_box));
    }

    #[test]
    fn time_sensitive_instruction_detects_trigger_registration() {
        let (authority, _keypair) = gen_account_in("wonderland");
        let trigger_id: iroha_data_model::trigger::TriggerId =
            "nts-trigger-reg".parse().expect("trigger id");
        let exec_trigger_id: iroha_data_model::trigger::TriggerId =
            "nts-trigger-exec".parse().expect("trigger id");
        let action = iroha_data_model::trigger::action::Action::new(
            vec![InstructionBox::from(
                iroha_data_model::isi::ExecuteTrigger::new(exec_trigger_id),
            )],
            iroha_data_model::trigger::action::Repeats::Indefinitely,
            authority.clone(),
            iroha_data_model::events::EventFilterBox::ExecuteTrigger(
                iroha_data_model::events::execute_trigger::ExecuteTriggerEventFilter::new(),
            ),
        );
        let trigger = iroha_data_model::trigger::Trigger::new(trigger_id, action);
        let register = iroha_data_model::isi::register::Register::trigger(trigger);
        let boxed = InstructionBox::from(register);
        assert!(super::is_time_sensitive_instruction(&boxed));
    }

    #[test]
    fn time_sensitive_instruction_marks_custom_instruction() {
        let custom = iroha_data_model::isi::CustomInstruction::new(Json::new("payload"));
        let boxed = InstructionBox::from(custom);
        assert!(super::is_time_sensitive_instruction(&boxed));
    }

    #[test]
    fn time_sensitive_executable_detects_sensitive_and_safe() {
        let (authority, _keypair) = gen_account_in("wonderland");
        let ballot = iroha_data_model::isi::governance::CastPlainBallot {
            referendum_id: "ref-2".into(),
            owner: authority,
            amount: 1_u64.into(),
            duration_blocks: 1,
            direction: 0,
        };
        let sensitive = Executable::from(vec![InstructionBox::from(ballot)]);
        assert!(super::is_time_sensitive_executable(&sensitive));

        let safe = Executable::from(vec![InstructionBox::from(Log::new(
            Level::INFO,
            "ok".into(),
        ))]);
        assert!(!super::is_time_sensitive_executable(&safe));

        let ivm = Executable::Ivm(
            iroha_data_model::transaction::executable::IvmBytecode::from_compiled(vec![0xCA]),
        );
        assert!(super::is_time_sensitive_executable(&ivm));
    }

    #[test]
    fn nts_enforcement_rejects_time_sensitive_when_unhealthy() {
        let (authority, keypair) = gen_account_in("wonderland");
        let chain: ChainId = "nts-reject".parse().expect("chain id");
        let ballot = iroha_data_model::isi::governance::CastPlainBallot {
            referendum_id: "ref-3".into(),
            owner: authority.clone(),
            amount: 1_u64.into(),
            duration_blocks: 1,
            direction: 0,
        };
        let tx = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([ballot])
        .sign(keypair.private_key());
        let status = crate::time::NetworkTimeStatus {
            now: std::time::SystemTime::UNIX_EPOCH,
            offset_ms: 0,
            confidence_ms: 0,
            sample_count: 0,
            peer_count: 0,
            fallback: true,
            health: crate::time::NtsHealth {
                min_samples_ok: false,
                offset_ok: true,
                confidence_ok: true,
                healthy: false,
            },
        };
        let err = super::enforce_time_sensitive_with_nts(
            &tx,
            status,
            iroha_config::parameters::actual::NtsEnforcementMode::Reject,
        )
        .expect_err("unhealthy NTS should reject in reject mode");
        match err {
            AcceptTransactionFail::NetworkTimeUnhealthy { reason } => {
                assert!(reason.contains("fallback=true"));
                assert!(reason.contains("samples_used=0"));
            }
            other => panic!("expected NetworkTimeUnhealthy, got {other:?}"),
        }
        assert!(
            super::enforce_time_sensitive_with_nts(
                &tx,
                status,
                iroha_config::parameters::actual::NtsEnforcementMode::Warn,
            )
            .is_ok()
        );
    }

    #[test]
    fn nts_enforcement_skips_non_sensitive_transactions() {
        let (authority, keypair) = gen_account_in("wonderland");
        let chain: ChainId = "nts-skip".parse().expect("chain id");
        let tx = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "ok".into())])
        .sign(keypair.private_key());
        assert!(super::enforce_nts_health_for_time_sensitive(&tx).is_ok());
    }

    #[test]
    fn fraud_policy_rejects_insufficient_band() {
        use iroha_primitives::json::Json;
        let cfg = iroha_config::parameters::actual::FraudMonitoring {
            enabled: true,
            required_minimum_band: Some(iroha_config::parameters::actual::FraudRiskBand::High),
            ..Default::default()
        };
        let mut metadata = Metadata::default();
        let band_key = Name::from_str("fraud_assessment_band").expect("static name");
        metadata.insert(band_key, Json::new("medium"));
        let score_key = Name::from_str("fraud_assessment_score_bps").expect("static name");
        metadata.insert(score_key, Json::new(450_u64));
        let tenant_key = Name::from_str("fraud_assessment_tenant").expect("static name");
        metadata.insert(tenant_key, Json::new("tenant-eu"));
        let latency_key = Name::from_str("fraud_assessment_latency_ms").expect("static name");
        metadata.insert(latency_key, Json::new(120_u64));
        let catalog = DataSpaceCatalog::default();
        let assignment = single_lane_assignment(&catalog);
        let result = super::enforce_fraud_policy(&cfg, &metadata, None, &assignment);
        assert!(matches!(
            result,
            Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(_)
            ))
        ));
    }

    #[test]
    fn fraud_policy_accepts_sufficient_band() {
        use iroha_primitives::json::Json;
        let cfg = iroha_config::parameters::actual::FraudMonitoring {
            enabled: true,
            required_minimum_band: Some(iroha_config::parameters::actual::FraudRiskBand::Medium),
            ..Default::default()
        };
        let mut metadata = Metadata::default();
        let band_key = Name::from_str("fraud_assessment_band").expect("static name");
        metadata.insert(band_key, Json::new("high"));
        let score_key = Name::from_str("fraud_assessment_score_bps").expect("static name");
        metadata.insert(score_key, Json::new(650_u64));
        let tenant_key = Name::from_str("fraud_assessment_tenant").expect("static name");
        metadata.insert(tenant_key, Json::new("tenant-eu"));
        let latency_key = Name::from_str("fraud_assessment_latency_ms").expect("static name");
        metadata.insert(latency_key, Json::new(95_u64));
        let catalog = DataSpaceCatalog::default();
        let assignment = single_lane_assignment(&catalog);
        assert!(super::enforce_fraud_policy(&cfg, &metadata, None, &assignment).is_ok());
    }

    #[test]
    fn fraud_policy_rejects_inconsistent_band() {
        use iroha_primitives::json::Json;
        let cfg = iroha_config::parameters::actual::FraudMonitoring {
            enabled: true,
            required_minimum_band: Some(iroha_config::parameters::actual::FraudRiskBand::Low),
            ..Default::default()
        };
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("fraud_assessment_band").expect("static name"),
            Json::new("low"),
        );
        metadata.insert(
            Name::from_str("fraud_assessment_score_bps").expect("static name"),
            Json::new(8_000_u64),
        );
        metadata.insert(
            Name::from_str("fraud_assessment_tenant").expect("static name"),
            Json::new("tenant-eu"),
        );
        metadata.insert(
            Name::from_str("fraud_assessment_latency_ms").expect("static name"),
            Json::new(110_u64),
        );
        let catalog = DataSpaceCatalog::default();
        let assignment = single_lane_assignment(&catalog);
        let result = super::enforce_fraud_policy(&cfg, &metadata, None, &assignment)
            .expect_err("inconsistent band must be rejected");
        match result {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason)) => {
                assert!(
                    reason.contains("inconsistent"),
                    "unexpected error message: {reason}"
                );
            }
            other => panic!("expected Validation::NotPermitted, got {other:?}"),
        }
    }

    fn fraud_metadata_with_assessment(assessment: &FraudAssessment) -> Metadata {
        use iroha_primitives::json::Json;

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("fraud_assessment_band").expect("static name"),
            Json::new("high"),
        );
        metadata.insert(
            Name::from_str("fraud_assessment_score_bps").expect("static name"),
            Json::new(u64::from(assessment.risk_score_bps)),
        );
        metadata.insert(
            Name::from_str("fraud_assessment_tenant").expect("static name"),
            Json::new("tenant-eu"),
        );
        metadata.insert(
            Name::from_str("fraud_assessment_latency_ms").expect("static name"),
            Json::new(95_u64),
        );

        let mut unsigned = assessment.clone();
        unsigned.signature = None;
        let unsigned_bytes = norito::codec::Encode::encode(&unsigned);
        let digest_bytes: [u8; 32] = iroha_crypto::Hash::new(&unsigned_bytes).into();
        metadata.insert(
            Name::from_str("fraud_assessment_digest").expect("static name"),
            Json::new(hex::encode_upper(digest_bytes)),
        );
        metadata.insert(
            Name::from_str("fraud_assessment_envelope").expect("static name"),
            Json::new(BASE64_STANDARD.encode(norito::codec::Encode::encode(assessment))),
        );

        metadata
    }

    #[test]
    fn fraud_policy_attester_signature_precheck_uses_checked_public_key_algorithm() {
        let attester =
            checked_fixture_keypair(b"fraud-attester-ed25519".to_vec(), Algorithm::Ed25519);
        let assessment = FraudAssessment::new(
            Vec::new(),
            iroha_data_model::fraud::types::FraudAssessmentParts {
                query_id: [0xFA; 32],
                engine_id: "risk-engine-eu".to_owned(),
                risk_score_bps: 650,
                confidence_bps: 9_000,
                decision: iroha_data_model::fraud::types::AssessmentDecision::Allow,
                generated_at_ms: 1,
                signature: Some(vec![0xAA, 0xBB, 0xCC]),
            },
        );
        let metadata = fraud_metadata_with_assessment(&assessment);
        let cfg = iroha_config::parameters::actual::FraudMonitoring {
            enabled: true,
            required_minimum_band: Some(iroha_config::parameters::actual::FraudRiskBand::Medium),
            attesters: vec![iroha_config::parameters::actual::FraudAttester {
                engine_id: "risk-engine-eu".to_owned(),
                public_key: attester.public_key().clone(),
            }],
            ..Default::default()
        };
        let catalog = DataSpaceCatalog::default();
        let assignment = single_lane_assignment(&catalog);

        let err = super::enforce_fraud_policy(&cfg, &metadata, None, &assignment)
            .expect_err("short Ed25519 attestation signature must be rejected");

        match err {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason)) => {
                assert!(
                    reason.contains("64 bytes"),
                    "unexpected rejection reason: {reason}"
                );
            }
            other => panic!("expected Validation::NotPermitted, got {other:?}"),
        }
    }

    #[test]
    fn fraud_policy_rejects_all_zero_attestation_signature_before_backend() {
        let attester =
            checked_fixture_keypair(b"fraud-attester-ed25519".to_vec(), Algorithm::Ed25519);
        let assessment = FraudAssessment::new(
            Vec::new(),
            iroha_data_model::fraud::types::FraudAssessmentParts {
                query_id: [0xFA; 32],
                engine_id: "risk-engine-eu".to_owned(),
                risk_score_bps: 650,
                confidence_bps: 9_000,
                decision: iroha_data_model::fraud::types::AssessmentDecision::Allow,
                generated_at_ms: 1,
                signature: Some(vec![0_u8; ED25519_SIGNATURE_LENGTH]),
            },
        );
        let metadata = fraud_metadata_with_assessment(&assessment);
        let cfg = iroha_config::parameters::actual::FraudMonitoring {
            enabled: true,
            required_minimum_band: Some(iroha_config::parameters::actual::FraudRiskBand::Medium),
            attesters: vec![iroha_config::parameters::actual::FraudAttester {
                engine_id: "risk-engine-eu".to_owned(),
                public_key: attester.public_key().clone(),
            }],
            ..Default::default()
        };
        let catalog = DataSpaceCatalog::default();
        let assignment = single_lane_assignment(&catalog);

        let err = super::enforce_fraud_policy(&cfg, &metadata, None, &assignment)
            .expect_err("all-zero Ed25519 attestation signature must be rejected");

        match err {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason)) => {
                assert!(
                    reason.contains("signature payload must not be all zero"),
                    "unexpected rejection reason: {reason}"
                );
            }
            other => panic!("expected Validation::NotPermitted, got {other:?}"),
        }
    }

    #[test]
    fn fraud_policy_rejects_malformed_ed25519_attestation_signature_r_before_backend() {
        const SMALL_ORDER_R: [u8; 32] = [
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];
        const NONCANONICAL_R: [u8; 32] = [
            0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0x7f,
        ];

        let attester =
            checked_fixture_keypair(b"fraud-attester-ed25519".to_vec(), Algorithm::Ed25519);
        let unsigned = FraudAssessment::new(
            Vec::new(),
            iroha_data_model::fraud::types::FraudAssessmentParts {
                query_id: [0xFA; 32],
                engine_id: "risk-engine-eu".to_owned(),
                risk_score_bps: 650,
                confidence_bps: 9_000,
                decision: iroha_data_model::fraud::types::AssessmentDecision::Allow,
                generated_at_ms: 1,
                signature: None,
            },
        );
        let valid_signature = checked_signature_of(attester.private_key(), &unsigned);
        let cfg = iroha_config::parameters::actual::FraudMonitoring {
            enabled: true,
            required_minimum_band: Some(iroha_config::parameters::actual::FraudRiskBand::Medium),
            attesters: vec![iroha_config::parameters::actual::FraudAttester {
                engine_id: "risk-engine-eu".to_owned(),
                public_key: attester.public_key().clone(),
            }],
            ..Default::default()
        };
        let catalog = DataSpaceCatalog::default();
        let assignment = single_lane_assignment(&catalog);

        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_R),
            ("noncanonical", NONCANONICAL_R),
        ] {
            let mut signature_bytes = valid_signature.payload().to_vec();
            signature_bytes[..replacement_r.len()].copy_from_slice(&replacement_r);
            let mut assessment = unsigned.clone();
            assessment.signature = Some(signature_bytes);
            let metadata = fraud_metadata_with_assessment(&assessment);

            let err = super::enforce_fraud_policy(&cfg, &metadata, None, &assignment)
                .expect_err("malformed Ed25519 attestation signature R must be rejected");

            match err {
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason)) => {
                    assert!(
                        reason.contains("signature is malformed"),
                        "{label} R produced unexpected rejection reason: {reason}"
                    );
                }
                other => panic!("expected Validation::NotPermitted for {label} R, got {other:?}"),
            }
        }
    }

    #[test]
    fn fraud_policy_rejects_malformed_mldsa_attestation_signature_lengths_before_backend() {
        let attester = checked_fixture_keypair(b"fraud-attester-mldsa".to_vec(), Algorithm::MlDsa);
        let unsigned = FraudAssessment::new(
            Vec::new(),
            iroha_data_model::fraud::types::FraudAssessmentParts {
                query_id: [0xFB; 32],
                engine_id: "risk-engine-eu".to_owned(),
                risk_score_bps: 650,
                confidence_bps: 9_000,
                decision: iroha_data_model::fraud::types::AssessmentDecision::Allow,
                generated_at_ms: 1,
                signature: None,
            },
        );
        let valid_signature = checked_signature_of(attester.private_key(), &unsigned);
        let cfg = iroha_config::parameters::actual::FraudMonitoring {
            enabled: true,
            required_minimum_band: Some(iroha_config::parameters::actual::FraudRiskBand::Medium),
            attesters: vec![iroha_config::parameters::actual::FraudAttester {
                engine_id: "risk-engine-eu".to_owned(),
                public_key: attester.public_key().clone(),
            }],
            ..Default::default()
        };
        let catalog = DataSpaceCatalog::default();
        let assignment = single_lane_assignment(&catalog);

        for label in ["short", "overlong"] {
            let mut signature_bytes = valid_signature.payload().to_vec();
            match label {
                "short" => {
                    signature_bytes
                        .pop()
                        .expect("ML-DSA fraud attestation signature is non-empty");
                }
                "overlong" => signature_bytes.push(0xA5),
                _ => unreachable!("covered labels"),
            }
            let mut assessment = unsigned.clone();
            assessment.signature = Some(signature_bytes);
            let metadata = fraud_metadata_with_assessment(&assessment);

            let err = super::enforce_fraud_policy(&cfg, &metadata, None, &assignment)
                .expect_err("malformed ML-DSA attestation signature length must be rejected");

            match err {
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason)) => {
                    assert!(
                        reason.contains("signature is malformed"),
                        "{label} ML-DSA signature length produced unexpected rejection reason: {reason}"
                    );
                }
                other => {
                    panic!(
                        "expected Validation::NotPermitted for {label} ML-DSA length, got {other:?}"
                    )
                }
            }
        }
    }

    #[test]
    fn tx_rejected_when_pipeline_gas_charge_limit_is_required_but_missing() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        // Minimal state with one domain/account as authority
        let (world, authority_id, kp) = world_with_authority("domain");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let mut state = State::new_with_chain(world, kura, query_handle, chain.clone());

        // Configure pipeline gas allowlist
        let mut pipeline = state.pipeline.clone();
        pipeline.gas.accepted_assets = vec!["xor#domain".to_string()];
        state.set_pipeline(pipeline);

        // Bind an executable gas limit but omit the required PipelineGas charge limit.
        let program = minimal_ivm_program_with_max_cycles(1, 1_000);
        let chain: ChainId = "chain".parse().unwrap();
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
        .sign(kp.private_key());

        // Height one is the fee-exempt genesis bootstrap boundary. Exercise
        // ordinary admission so the signed PipelineGas limit is mandatory.
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let accepted = super::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, res) = block.validate_transaction(accepted, &mut ivm_cache);
        assert!(matches!(
            res,
            Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(_)
            ))
        ));
    }

    #[test]
    fn signature_limit_rejects_above_bound() {
        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            nonzero!(1_u64),
            nonzero!(4096_u64),
            nonzero!(4096_u64),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );
        let err = AcceptedTransaction::ensure_signature_limit(2, &limits)
            .expect_err("limit should reject excessive signatures");
        assert!(matches!(err, AcceptTransactionFail::TransactionLimit(_)));
    }

    #[test]
    fn signature_limit_allows_at_bound() {
        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            nonzero!(2_u64),
            nonzero!(4096_u64),
            nonzero!(4096_u64),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );
        assert!(AcceptedTransaction::ensure_signature_limit(2, &limits).is_ok());
    }

    const IVM_METADATA_HEADER_LEN: usize = ivm::HEADER_SIZE;
    const LITERAL_SECTION_MAGIC: [u8; 4] = *b"LTLB";

    /// Build a minimal valid IVM program: header (1.0, vector=4, `max_cycles=0`, abi=1) + HALT.
    fn minimal_ivm_program(abi_version: u8) -> Vec<u8> {
        let mut code = Vec::new();
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let mut program = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 4,
            max_cycles: 1_000,
            abi_version,
        }
        .encode();
        program.extend_from_slice(&code);
        program
    }

    /// Build a minimal self-describing contract containing only a view entrypoint and HALT.
    fn minimal_ivm_contract_program() -> Vec<u8> {
        let mut program = ivm::ProgramMetadata {
            max_cycles: 1_000,
            ..ivm::ProgramMetadata::default()
        }
        .encode();
        let interface = ivm::EmbeddedContractInterfaceV1 {
            seiyaku_name: "TxManifestFixture".to_owned(),
            compiler_fingerprint: "iroha-core-tx-tests".to_owned(),
            abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
                name: "inspect".to_owned(),
                kind: iroha_data_model::smart_contract::manifest::EntryPointKind::View,
                params: Vec::new(),
                argument_schema: None,
                return_type: None,
                return_schema: None,
                permission: None,
                read_keys: Vec::new(),
                write_keys: Vec::new(),
                access_hints_complete: Some(true),
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
                entry_pc: 0,
            }],
            error_codes: Vec::new(),
            states: Vec::new(),
        };
        program.extend_from_slice(&interface.encode_section());
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        program
    }

    /// Build a minimal program and override `max_cycles` in the header.
    fn minimal_ivm_program_with_max_cycles(abi_version: u8, max_cycles: u64) -> Vec<u8> {
        let mut prog = minimal_ivm_program(abi_version);
        // Overwrite bytes [8..16] with the desired max_cycles value
        prog[8..16].copy_from_slice(&max_cycles.to_le_bytes());
        prog
    }

    #[track_caller]
    fn minimal_ivm_program_with_instruction_count(
        abi_version: u8,
        max_cycles: u64,
        instruction_count: usize,
    ) -> Vec<u8> {
        assert!(instruction_count > 0, "instruction_count must be non-zero");
        assert!(max_cycles > 0, "max_cycles must be non-zero");
        let mut code = Vec::with_capacity(instruction_count * core::mem::size_of::<u32>());
        for _ in 0..instruction_count {
            code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        }
        let mut program = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 4,
            max_cycles,
            abi_version,
        }
        .encode();
        program.extend_from_slice(&code);
        program
    }

    /// Build a minimal program and insert a literal-section padding block so the artifact
    /// reaches `total_len` bytes.
    fn minimal_ivm_program_with_literal_padding(abi_version: u8, total_len: usize) -> Vec<u8> {
        assert!(
            total_len >= IVM_METADATA_HEADER_LEN + 4 + 16,
            "literal padding requires at least one opcode and metadata block"
        );
        let mut program = minimal_ivm_program(abi_version);
        let mut code = program.split_off(IVM_METADATA_HEADER_LEN);
        debug_assert_eq!(
            code.len(),
            4,
            "minimal program should contain a single opcode"
        );

        let pad_len = total_len
            .checked_sub(IVM_METADATA_HEADER_LEN + code.len())
            .expect("total_len smaller than header + code");
        assert!(
            pad_len >= 16,
            "literal table header consumes 16 bytes; remaining pad must fit that"
        );
        let data_and_pad = pad_len - 16;
        let literal_data_len = data_and_pad
            .checked_sub(data_and_pad % 4)
            .expect("literal padding underflow");
        let post_pad = data_and_pad - literal_data_len;
        assert!(
            post_pad == (4 - (literal_data_len % 4)) % 4,
            "requested total_len cannot be represented by a valid literal section"
        );
        let literal_data_len_u32 = u32::try_from(literal_data_len)
            .expect("literal data length exceeds literal section encoding");
        let post_pad_u32 =
            u32::try_from(post_pad).expect("pad length exceeds literal section encoding");

        let mut padded = program;
        padded.extend_from_slice(&LITERAL_SECTION_MAGIC);
        padded.extend_from_slice(&0u32.to_le_bytes()); // literal count
        padded.extend_from_slice(&post_pad_u32.to_le_bytes());
        padded.extend_from_slice(&literal_data_len_u32.to_le_bytes());
        padded.resize(padded.len() + literal_data_len + post_pad, 0);
        padded.append(&mut code);
        assert_eq!(
            padded.len(),
            total_len,
            "padding must reach exact target length"
        );
        padded
    }

    /// Build a minimal program that issues a single syscall followed by HALT.
    fn minimal_ivm_program_with_syscall(abi_version: u8, syscall: u8) -> Vec<u8> {
        let mut code = Vec::new();
        code.extend_from_slice(
            &ivm::encoding::wide::encode_sys(ivm::instruction::wide::system::SCALL, syscall)
                .to_le_bytes(),
        );
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());

        let mut program = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 4,
            max_cycles: 1_000,
            abi_version,
        }
        .encode();
        program.extend_from_slice(&code);
        program
    }

    /// Build a minimal program that issues a single extended syscall followed by HALT.
    fn minimal_ivm_program_with_syscallx(abi_version: u8, syscall: u32) -> Vec<u8> {
        let mut code = Vec::new();
        code.extend_from_slice(&ivm::encoding::wide::encode_syscallx(syscall).to_le_bytes());
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());

        let mut program = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 4,
            max_cycles: 1_000,
            abi_version,
        }
        .encode();
        program.extend_from_slice(&code);
        program
    }

    const TEST_GAS_LIMIT: u64 = 1_000_000;

    fn fee_payment_with_gas_limit(limit: u64) -> FeePaymentIntent {
        FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(limit))
    }

    #[test]
    fn validate_ivm_header_accepts_supported_versions() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        // World with a single domain and account as authority
        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_program(1);
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        assert!(result.is_ok(), "valid header should pass: {result:?}");
    }

    #[test]
    fn validate_ivm_header_rejects_unknown_abi() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_program(3); // unsupported abi_version
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::UnsupportedAbiVersion(3),
            ))) => {}
            other => panic!("Expected UnsupportedAbiVersion(3) error, got {other:?}"),
        }
    }

    #[test]
    fn validate_ivm_header_rejects_abi_zero() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let chain: ChainId = "chain".parse().unwrap();
        // abi_version=0 must be rejected in v1-only release
        let prog = minimal_ivm_program(0);
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::UnsupportedAbiVersion(0),
            ))) => {}
            other => panic!("Expected UnsupportedAbiVersion(0) error, got {other:?}"),
        }
    }

    #[test]
    fn validate_generic_ivm_rejects_reserved_manifest_metadata_before_decode() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let mut metadata = Metadata::default();
        metadata.insert(
            (*CONTRACT_MANIFEST_METADATA_NAME).clone(),
            Json::from("not-a-contract-manifest"),
        );
        let tx = TransactionBuilder::new(
            chain,
            authority_id,
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_metadata(metadata)
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(
            minimal_ivm_program(1),
        )))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        assert!(matches!(
            result,
            Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                message
            ))) if message.contains("reserved `contract_manifest`")
        ));
    }

    #[test]
    fn validate_ivm_rejects_stale_authenticated_cntr_abi_hash() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let (artifact, _) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(
                "seiyaku StaleAbi { view fn inspect() -> int { return 1; } }",
            )
            .expect("compile self-describing contract");
        let parsed = ivm::ProgramMetadata::parse(&artifact).expect("parse compiled contract");
        let mut interface = parsed
            .contract_interface
            .expect("compiled contract carries CNTR");
        let original_section_len = interface.encode_section().len();
        let expected = interface.abi_hash;
        interface.abi_hash[0] ^= 0x80;
        let actual = interface.abi_hash;
        let mut stale = parsed.metadata.encode();
        stale.extend_from_slice(&interface.encode_section());
        stale.extend_from_slice(
            artifact
                .get(parsed.header_len + original_section_len..)
                .expect("post-CNTR artifact suffix is in bounds"),
        );

        let tx = TransactionBuilder::new(
            chain,
            authority_id,
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(stale)))
        .sign(kp.private_key());
        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        assert!(matches!(
            result,
            Err(TransactionRejectionReason::Validation(
                ValidationFail::IvmAdmission(
                    iroha_data_model::executor::IvmAdmissionError::ArtifactAbiHashMismatch(info)
                )
            )) if info.expected == iroha_crypto::Hash::prehashed(expected)
                && info.actual == iroha_crypto::Hash::prehashed(actual)
        ));
    }

    #[test]
    fn validate_ivm_manifest_metadata_conflict_rejected_even_if_state_matches() {
        use iroha_data_model::{
            smart_contract::manifest::ContractManifest,
            transaction::{Executable, TransactionBuilder},
        };
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        // Seed block 1 with a correct manifest for the program.
        let header1 =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block1 = state.block(header1);
        let mut tx1 = block1.transaction();
        let prog = minimal_ivm_contract_program();
        let code_hash = ivm::contract_code_hash(&prog);
        let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        tx1.world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                seiyaku_name: None,
                code_hash: Some(code_hash),
                abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                states: None,
                kotoba: None,
                error_codes: None,
                provenance: None,
            }
            .signed(&kp),
        );
        tx1.apply();
        let _ = block1.commit();

        // Block 2: metadata manifest advertises the wrong abi_hash; admission must reject even
        // though the stored manifest matches.
        let header2 =
            iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block2 = state.block(header2);
        let mut wrong_abi = abi_hash;
        wrong_abi[0] ^= 0x55;
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(wrong_abi)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(&kp);
        let mut md = Metadata::default();
        md.insert(
            "contract_manifest".parse::<Name>().unwrap(),
            Json::new(manifest),
        );
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_metadata(md)
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block2.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::ManifestAbiHashMismatch(info),
            ))) => {
                assert_eq!(info.expected, iroha_crypto::Hash::prehashed(wrong_abi));
                assert_eq!(info.actual, iroha_crypto::Hash::prehashed(abi_hash));
            }
            other => panic!(
                "Expected ManifestAbiHashMismatch from metadata manifest conflict, got {other:?}"
            ),
        }
    }

    #[test]
    fn validate_ivm_manifest_abi_and_code_hash_match() {
        use iroha_data_model::smart_contract::manifest::ContractManifest;
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut state_tx = block.transaction();

        // Build minimal program with abi_version=1 (current baseline)
        let prog = minimal_ivm_contract_program();
        // Compute the canonical full-artifact contract hash.
        let code_hash = ivm::contract_code_hash(&prog);
        // Compute abi hash for the policy
        let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        // Attach manifest in metadata
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(&kp);
        let mut md = Metadata::default();
        md.insert(
            "contract_manifest".parse::<Name>().unwrap(),
            Json::new(manifest),
        );
        let mut ivm_cache = IvmCache::new();
        let result = StateBlock::validate_ivm(
            authority_id,
            &mut state_tx,
            IvmBytecode::from_compiled(prog),
            Some(&md),
            None,
            &mut ivm_cache,
        );
        assert!(result.is_ok(), "valid manifest should pass: {result:?}");
    }

    #[test]
    fn validate_ivm_manifest_rejects_mismatched_hashes() {
        use iroha_data_model::{
            smart_contract::manifest::ContractManifest,
            transaction::{Executable, TransactionBuilder},
        };
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_contract_program();
        // Compute real code hash; then corrupt expected
        let code_hash = ivm::contract_code_hash(&prog);
        // Compute abi hash then flip
        let mut wrong_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        wrong_abi[0] ^= 0xAA;
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(wrong_abi)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(&kp);
        let mut md = Metadata::default();
        md.insert(
            "contract_manifest".parse::<Name>().unwrap(),
            Json::new(manifest),
        );
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_metadata(md)
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::ManifestAbiHashMismatch(..),
            ))) => {}
            other => panic!(
                "Expected ManifestAbiHashMismatch structured error for mismatched manifest, got {other:?}"
            ),
        }
    }

    #[test]
    fn validate_ivm_manifest_rejects_mismatched_code_hash() {
        use iroha_data_model::{
            smart_contract::manifest::ContractManifest,
            transaction::{Executable, TransactionBuilder},
        };
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_contract_program();
        let mut wrong_bytes = [0u8; 32];
        wrong_bytes[0] = 0xFF;
        wrong_bytes[31] = 1; // set LSB as per Hash invariant
        let wrong_code_hash = iroha_crypto::Hash::prehashed(wrong_bytes);
        let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(wrong_code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(&kp);
        let mut md = Metadata::default();
        md.insert(
            "contract_manifest".parse::<Name>().unwrap(),
            Json::new(manifest),
        );
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_metadata(md)
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::ManifestCodeHashMismatch(..),
            ))) => {}
            other => panic!(
                "Expected ManifestCodeHashMismatch structured error for mismatched manifest, got {other:?}"
            ),
        }
    }

    #[test]
    fn validate_ivm_manifest_state_conflict_rejected_even_if_metadata_matches() {
        use iroha_data_model::{
            smart_contract::manifest::ContractManifest,
            transaction::{Executable, TransactionBuilder},
        };
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        // Seed block 1 with a manifest that has the right code_hash but wrong abi_hash.
        let header1 =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block1 = state.block(header1);
        let mut tx1 = block1.transaction();
        let prog = minimal_ivm_contract_program();
        let code_hash = ivm::contract_code_hash(&prog);
        let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let mut wrong_abi = abi_hash;
        wrong_abi[0] ^= 0x5A;
        tx1.world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                seiyaku_name: None,
                code_hash: Some(code_hash),
                abi_hash: Some(iroha_crypto::Hash::prehashed(wrong_abi)),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                states: None,
                kotoba: None,
                error_codes: None,
                provenance: None,
            }
            .signed(&kp),
        );
        tx1.apply();
        let _ = block1.commit();

        // Block 2: attach a correct manifest in metadata; validation should still reject
        // because the stored manifest ABI hash mismatches the computed one.
        let header2 =
            iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block2 = state.block(header2);
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(&kp);
        let mut md = Metadata::default();
        md.insert(
            "contract_manifest".parse::<Name>().unwrap(),
            Json::new(manifest),
        );
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_metadata(md)
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block2.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::ManifestAbiHashMismatch(info),
            ))) => {
                assert_eq!(info.expected, iroha_crypto::Hash::prehashed(wrong_abi));
                assert_eq!(info.actual, iroha_crypto::Hash::prehashed(abi_hash));
            }
            other => panic!(
                "Expected ManifestAbiHashMismatch structured error despite metadata manifest, got {other:?}"
            ),
        }
    }

    #[test]
    fn validate_ivm_max_cycles_structured_error() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        // Program with max_cycles above default config bound; expect structured error
        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_program_with_max_cycles(1, 9_999_999);
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::MaxCyclesExceedsUpperBound(..),
            ))) => {}
            other => panic!("Expected MaxCyclesExceedsUpperBound structured error, got {other:?}"),
        }
    }

    #[test]
    fn validate_ivm_missing_max_cycles_rejected() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let prog = minimal_ivm_program_with_max_cycles(1, 0);
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::MissingMaxCycles,
            ))) => {}
            other => panic!("Expected MissingMaxCycles error, got {other:?}"),
        }
    }

    #[test]
    fn validate_ivm_max_cycles_exceeds_fuel_rejected() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let mut state = State::new_with_chain(world, kura, query_handle, chain.clone());

        // Raise pipeline upper bound above fuel limit so the fuel check triggers first.
        let mut pipeline = state.pipeline.clone();
        let fuel_limit = state.world.parameters.view().smart_contract().fuel().get();
        pipeline.ivm_max_cycles_upper_bound =
            std::num::NonZeroU64::new(fuel_limit + 10).expect("fuel limit plus ten is non-zero");
        state.set_pipeline(pipeline);

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let prog = minimal_ivm_program_with_max_cycles(1, fuel_limit + 1);
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::MaxCyclesExceedsFuel(info),
            ))) => {
                assert_eq!(info.fuel_limit, fuel_limit);
                assert_eq!(info.max_cycles, fuel_limit + 1);
            }
            other => panic!("Expected MaxCyclesExceedsFuel error, got {other:?}"),
        }
    }

    #[test]
    fn validate_ivm_instruction_limit_enforced() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let mut state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let mut pipeline = state.pipeline.clone();
        pipeline.ivm_max_decoded_instructions = 2;
        pipeline.ivm_max_decoded_bytes =
            iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_BYTES;
        state.set_pipeline(pipeline);

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let prog = minimal_ivm_program_with_instruction_count(1, 1_000, 4);
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::DecodedInstructionCountExceeded(
                    info,
                ),
            ))) => {
                assert_eq!(info.limit, 2);
                assert_eq!(info.decoded_instructions, 4);
            }
            other => panic!("Expected DecodedInstructionCountExceeded error, got {other:?}"),
        }
    }

    #[test]
    fn validate_ivm_decoded_byte_limit_enforced() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let mut state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let mut pipeline = state.pipeline.clone();
        pipeline.ivm_max_decoded_instructions = 0;
        pipeline.ivm_max_decoded_bytes = 8; // allow only two 4-byte instructions
        state.set_pipeline(pipeline);

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let prog = minimal_ivm_program_with_instruction_count(1, 1_000, 4);
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::DecodedCodeSizeExceeded(info),
            ))) => {
                assert_eq!(info.limit, 8);
                assert_eq!(info.decoded_bytes, 16);
            }
            other => panic!("Expected DecodedCodeSizeExceeded error, got {other:?}"),
        }
    }

    #[test]
    fn validate_ivm_manifest_lookup_in_state() {
        use iroha_data_model::smart_contract::manifest::ContractManifest;
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        // Seed block 1: insert a manifest into WSV directly via state tx
        let header1 =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block1 = state.block(header1);
        let mut tx1 = block1.transaction();
        // Build a minimal program to compute its code_hash/abi_hash
        let prog = minimal_ivm_contract_program();
        let code_hash = ivm::contract_code_hash(&prog);
        let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(&kp);
        tx1.world
            .contract_manifests
            .insert(code_hash, manifest.clone());
        tx1.apply();
        let _ = block1.commit();

        // Block 2: submit the IVM program; validation should find the manifest in WSV and accept
        let header2 =
            iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block2 = state.block(header2);
        let mut state_tx = block2.transaction();
        let mut ivm_cache = IvmCache::new();
        let result = StateBlock::validate_ivm(
            authority_id,
            &mut state_tx,
            IvmBytecode::from_compiled(prog),
            None,
            None,
            &mut ivm_cache,
        );
        assert!(result.is_ok(), "lookup manifest should allow validation");
    }

    #[test]
    fn validate_ivm_unknown_syscall_rejected_at_admission() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let syscall = (0u8..=u8::MAX)
            .find(|number| {
                !ivm::syscalls::is_syscall_allowed(ivm::SyscallPolicy::AbiV1, u32::from(*number))
            })
            .expect("ABI v1 should leave at least one u8 syscall number unmapped");

        // Program issues an unmapped SCALL then HALT; admission should reject before the VM runs.
        let prog = minimal_ivm_program_with_syscall(1, syscall);
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
                let expected = format!("unknown syscall number 0x{syscall:02x}");
                assert!(
                    msg.contains(&expected) && msg.contains("abi_version 1"),
                    "expected UnknownSyscall rejection to surface via NotPermitted, got {msg}"
                );
            }
            other => panic!("Expected UnknownSyscall rejection, got {other:?}"),
        }
    }

    #[test]
    fn validate_ivm_unknown_scallx_rejected_at_admission() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let syscall = 0x02_0000;
        assert!(!ivm::syscalls::is_syscall_allowed(
            ivm::SyscallPolicy::AbiV1,
            syscall
        ));

        let prog = minimal_ivm_program_with_syscallx(1, syscall);
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
                let expected = format!("unknown syscall number 0x{syscall:02x}");
                assert!(
                    msg.contains(&expected) && msg.contains("abi_version 1"),
                    "expected UnknownSyscall rejection to surface via NotPermitted, got {msg}"
                );
            }
            other => panic!("Expected UnknownSyscall rejection, got {other:?}"),
        }
    }

    #[test]
    fn invalid_signature_is_rejected() {
        use std::time::Duration;

        use iroha_data_model::prelude::*;

        let chain_id = ChainId::from("chain");
        let (authority_id, keypair) = gen_account_in("wonderland");
        let instruction = SetKeyValue::account(authority_id.clone(), "k".parse().unwrap(), "v");
        let tx = TransactionBuilder::new(
            chain_id.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(keypair.private_key());
        let mut invalid_tx = tx.clone();
        let mut signature_payload = invalid_tx.signature().payload().payload().to_vec();
        assert!(
            !signature_payload.is_empty(),
            "transaction signature payload should never be empty"
        );
        let flip_index = signature_payload.len() - 1;
        signature_payload[flip_index] ^= 0xFF;
        let forged_signature = iroha_crypto::Signature::try_from_bytes(&signature_payload)
            .expect("tampered transaction signature remains structurally admissible");
        invalid_tx.set_signature(TransactionSignature(
            iroha_crypto::SignatureOf::from_signature(forged_signature),
        ));
        assert_ne!(invalid_tx.signature(), tx.signature());
        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            NonZeroU64::new(1).unwrap(),
            NonZeroU64::new(10).unwrap(),
            NonZeroU64::new(1024).unwrap(),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        match AcceptedTransaction::validate(
            &invalid_tx,
            &chain_id,
            Duration::from_secs(0),
            limits,
            &crypto_cfg,
        ) {
            Err(AcceptTransactionFail::SignatureVerification(fail)) => {
                assert_eq!(fail.signature, invalid_tx.signature().clone());
            }
            other => panic!("Expected signature verification error, got {other:?}"),
        }
    }

    #[test]
    fn ivm_bytecode_oversize_is_rejected_at_admission() {
        use std::time::Duration;

        use iroha_data_model::transaction::{Executable, TransactionBuilder};

        // Build a valid signed transaction with an oversized IVM bytecode blob
        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");

        // Limit bytecode size to 1024 bytes for this test
        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            NonZeroU64::new(1).unwrap(),
            NonZeroU64::new(10).unwrap(),
            NonZeroU64::new(1024).unwrap(),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );

        // Create a blob twice the allowed size (2 KiB) — content need not be a valid IVM header
        let oversize_blob = vec![0u8; 2048];
        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(oversize_blob)))
        .sign(kp.private_key());

        // Admission must reject with a TransactionLimit error
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        match AcceptedTransaction::validate(
            &tx,
            &chain,
            Duration::from_secs(0),
            limits,
            &crypto_cfg,
        ) {
            Err(AcceptTransactionFail::TransactionLimit(_)) => {}
            other => {
                panic!("Expected TransactionLimit error for oversize IVM bytecode, got {other:?}")
            }
        }
    }

    #[test]
    fn ivm_bytecode_at_limit_is_accepted_at_admission() {
        use std::time::Duration;

        use iroha_data_model::transaction::{Executable, TransactionBuilder};

        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");

        // Use an exact bytecode-size limit that can be represented by the 17-byte
        // IVM metadata header plus a valid literal-prefix alignment.
        const BYTECODE_LIMIT: u64 = 1021;
        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            NonZeroU64::new(1).unwrap(),
            NonZeroU64::new(10).unwrap(),
            NonZeroU64::new(BYTECODE_LIMIT).unwrap(),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );

        // Create a blob exactly at the allowed bytecode size.
        let at_limit_blob = minimal_ivm_program_with_literal_padding(1, BYTECODE_LIMIT as usize);
        assert_eq!(at_limit_blob.len(), BYTECODE_LIMIT as usize);
        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(at_limit_blob)))
        .sign(kp.private_key());

        // Admission should accept this transaction
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        match AcceptedTransaction::validate(
            &tx,
            &chain,
            Duration::from_secs(0),
            limits,
            &crypto_cfg,
        ) {
            Ok(()) => {}
            other => panic!("Expected Ok for at-limit IVM bytecode, got {other:?}"),
        }
    }

    #[test]
    fn ivm_missing_gas_bound_rejected_at_admission() {
        use std::time::Duration;

        use iroha_data_model::transaction::{Executable, TransactionBuilder};

        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let prog = minimal_ivm_program_with_max_cycles(1, 1_000);
        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let limits = TransactionParameters::default();
        let err =
            AcceptedTransaction::validate(&tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect_err("missing gas limit in fee payment intent should be rejected");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit
                        .reason
                        .contains("missing gas limit in fee payment intent"),
                    "unexpected reason: {}",
                    limit.reason
                );
            }
            other => panic!("Expected TransactionLimit failure, got {other:?}"),
        }
    }

    #[test]
    fn legacy_gas_limit_metadata_is_rejected_before_admission() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};

        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let prog = minimal_ivm_program_with_max_cycles(1, 1_000);
        let mut metadata = Metadata::default();
        metadata.insert("gas_limit".parse().unwrap(), Json::new(0_u64));
        let error = TransactionBuilder::new(
            chain,
            authority_id,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_metadata(metadata)
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .try_sign(kp.private_key())
        .expect_err("retired gas-limit metadata must fail before admission");

        assert!(
            error
                .to_string()
                .contains("legacy transaction metadata key `gas_limit`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn ivm_proved_missing_gas_bound_rejected_at_admission() {
        use std::time::Duration;

        use iroha_data_model::transaction::{Executable, IvmProved, TransactionBuilder};

        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let prog = minimal_ivm_program_with_max_cycles(1, 1_000);
        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(prog),
            overlay: Vec::<InstructionBox>::new().into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        }))
        .sign(kp.private_key());

        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let limits = TransactionParameters::default();
        let err =
            AcceptedTransaction::validate(&tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect_err("missing gas limit in fee payment intent should be rejected");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit
                        .reason
                        .contains("missing gas limit in fee payment intent"),
                    "unexpected reason: {}",
                    limit.reason
                );
            }
            other => panic!("Expected TransactionLimit failure, got {other:?}"),
        }
    }

    #[test]
    fn contract_call_missing_gas_bound_rejected_at_admission() {
        use std::time::Duration;

        use iroha_data_model::transaction::{
            Executable, TransactionBuilder, executable::ContractInvocation,
        };

        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::ContractCall(ContractInvocation {
            contract_address: "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
                .parse()
                .expect("contract address"),
            expected_code_hash: Hash::new(b"admission-contract-code"),
            entrypoint: "call".to_owned(),
            arguments: None,
        }))
        .sign(kp.private_key());

        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let limits = TransactionParameters::default();
        let err =
            AcceptedTransaction::validate(&tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect_err("missing gas limit in fee payment intent should be rejected");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit
                        .reason
                        .contains("missing gas limit in fee payment intent"),
                    "unexpected reason: {}",
                    limit.reason
                );
            }
            other => panic!("Expected TransactionLimit failure, got {other:?}"),
        }
    }

    #[test]
    fn mixed_batch_missing_gas_bound_rejected_at_admission() {
        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let call = ContractInvocation {
            contract_address: "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
                .parse()
                .expect("contract address"),
            expected_code_hash: Hash::new(b"batch-admission-contract-code"),
            entrypoint: "call".to_owned(),
            arguments: None,
        };
        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Batch(
            vec![
                ExecutableBatchItem::Instruction(InstructionBox::from(Log::new(
                    Level::INFO,
                    "before call".to_owned(),
                ))),
                ExecutableBatchItem::ContractCall(call),
            ]
            .into(),
        ))
        .sign(kp.private_key());

        let err = AcceptedTransaction::validate(
            &tx,
            &chain,
            Duration::ZERO,
            TransactionParameters::default(),
            &iroha_config::parameters::actual::Crypto::default(),
        )
        .expect_err("mixed batch without a signed gas limit must be rejected");

        assert!(matches!(
            err,
            AcceptTransactionFail::TransactionLimit(ref limit)
                if limit.reason.contains("missing gas limit in fee payment intent")
        ));
    }

    #[test]
    fn empty_executable_batch_rejected_at_admission() {
        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Batch(ConstVec::new_empty()))
        .sign(kp.private_key());

        let err = AcceptedTransaction::validate(
            &tx,
            &chain,
            Duration::ZERO,
            TransactionParameters::default(),
            &iroha_config::parameters::actual::Crypto::default(),
        )
        .expect_err("empty executable batch must be rejected");

        assert!(matches!(
            err,
            AcceptTransactionFail::TransactionLimit(ref limit)
                if limit.reason.contains("must not be empty")
        ));
    }

    #[test]
    fn transaction_size_limit_enforced() {
        use std::time::Duration;

        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");

        let mut metadata = Metadata::default();
        metadata.insert(
            "blob".parse().expect("metadata key"),
            Json::new("x".repeat(1024)),
        );

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "sized".to_string())])
        .with_metadata(metadata)
        .sign(kp.private_key());

        let limits = TransactionParameters::with_max_signatures(
            NonZeroU64::new(1).unwrap(),
            NonZeroU64::new(10).unwrap(),
            NonZeroU64::new(4096).unwrap(),
            NonZeroU64::new(256).unwrap(),
            NonZeroU64::new(4096).unwrap(),
            NonZeroU16::new(8).unwrap(),
        );
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();

        let err =
            AcceptedTransaction::validate(&tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect_err("transaction exceeding max_tx_bytes must be rejected");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit.reason.contains("Transaction size"),
                    "expected max_tx_bytes rejection, got {limit:?}"
                );
            }
            other => panic!("expected TransactionLimit failure, got {other:?}"),
        }
    }

    #[test]
    fn attachments_decompressed_limit_enforced() {
        use std::time::Duration;

        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");

        let proof = ProofBox::new("halo2/ipa".into(), vec![0u8; 192]);
        let vk_id = VerifyingKeyId::new("halo2/ipa", "vk_limit");
        let attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof, vk_id);
        let attachments = ProofAttachmentList::try_from(vec![attachment])
            .expect("one attachment is a valid bounded proof list");

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "proof".to_string())])
        .with_attachments(attachments)
        .sign(kp.private_key());

        let limits = TransactionParameters::with_max_signatures(
            NonZeroU64::new(1).unwrap(),
            NonZeroU64::new(10).unwrap(),
            NonZeroU64::new(4096).unwrap(),
            NonZeroU64::new(1_048_576).unwrap(),
            NonZeroU64::new(128).unwrap(),
            NonZeroU16::new(8).unwrap(),
        );
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();

        let err =
            AcceptedTransaction::validate(&tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect_err("attachments exceeding max_decompressed_bytes must be rejected");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit.reason.contains("attachments expand"),
                    "expected max_decompressed_bytes rejection, got {limit:?}"
                );
            }
            other => panic!("expected TransactionLimit failure, got {other:?}"),
        }
    }

    #[test]
    fn malformed_proof_attachments_rejected_at_transaction_admission() {
        use std::time::Duration;

        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let limits = TransactionParameters::default();

        let mut zero_vk_commitment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_admission"),
        );
        zero_vk_commitment.vk_commitment = Some([0u8; 32]);

        let mut zero_envelope_hash = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_admission"),
        );
        zero_envelope_hash.envelope_hash = Some([0u8; 32]);

        let mut forged_envelope_hash = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_admission"),
        );
        let mut forged_hash: [u8; 32] =
            iroha_crypto::Hash::new(&forged_envelope_hash.proof.bytes).into();
        forged_hash[0] ^= 0x80;
        forged_envelope_hash.envelope_hash = Some(forged_hash);

        assert!(matches!(
            ProofAttachmentList::try_from(Vec::new()),
            Err(iroha_data_model::proof::ProofAttachmentListError::Empty)
        ));

        let cases = [
            (
                "proof-backend-mismatch",
                ProofAttachmentList::try_from(vec![ProofAttachment::new_ref(
                    "halo2/ipa".into(),
                    ProofBox::new("stark/fri".into(), vec![1, 2, 3]),
                    VerifyingKeyId::new("halo2/ipa", "vk_admission"),
                )])
                .expect("one attachment is a valid bounded proof list"),
                "proof.backend",
            ),
            (
                "nonportable-vk-ref-name",
                ProofAttachmentList::try_from(vec![ProofAttachment::new_ref(
                    "halo2/ipa".into(),
                    ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
                    VerifyingKeyId::new("halo2/ipa", "VkAdmission"),
                )])
                .expect("one attachment is a valid bounded proof list"),
                "vk_ref",
            ),
            (
                "empty-proof-bytes",
                ProofAttachmentList::try_from(vec![ProofAttachment::new_ref(
                    "halo2/ipa".into(),
                    ProofBox::new("halo2/ipa".into(), Vec::new()),
                    VerifyingKeyId::new("halo2/ipa", "vk_admission"),
                )])
                .expect("one attachment is a valid bounded proof list"),
                "proof.bytes",
            ),
            (
                "zero-vk-commitment",
                ProofAttachmentList::try_from(vec![zero_vk_commitment])
                    .expect("one attachment is a valid bounded proof list"),
                "vk_commitment",
            ),
            (
                "zero-envelope-hash",
                ProofAttachmentList::try_from(vec![zero_envelope_hash])
                    .expect("one attachment is a valid bounded proof list"),
                "envelope_hash",
            ),
            (
                "forged-envelope-hash",
                ProofAttachmentList::try_from(vec![forged_envelope_hash])
                    .expect("one attachment is a valid bounded proof list"),
                "envelope_hash",
            ),
        ];

        for (label, attachments, expected_reason) in cases {
            let tx = TransactionBuilder::new(
                chain.clone(),
                authority_id.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, format!("proof-{label}"))])
            .with_attachments(attachments)
            .sign(kp.private_key());

            let err = AcceptedTransaction::validate(
                &tx,
                &chain,
                Duration::from_secs(0),
                limits,
                &crypto_cfg,
            )
            .expect_err("malformed proof attachment must be rejected at admission");

            match err {
                AcceptTransactionFail::TransactionLimit(limit) => {
                    assert!(
                        limit.reason.contains(expected_reason),
                        "case {label}: expected reason to contain {expected_reason}, got {}",
                        limit.reason
                    );
                }
                other => panic!("case {label}: expected TransactionLimit failure, got {other:?}"),
            }
        }
    }

    #[test]
    fn accept_transaction_requires_expires_at_height_when_configured() {
        use std::time::Duration;

        use iroha_data_model::isi::Log;
        use iroha_logger::Level;

        let chain: ChainId = "ttl-config-chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "ttl".into())])
        .sign(kp.private_key());

        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            default_limits.max_signatures(),
            default_limits.max_instructions(),
            default_limits.ivm_bytecode_size(),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        )
        .with_ingress_enforcement(true, false);
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();

        let err =
            AcceptedTransaction::accept(tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect_err("transactions must provide expires_at_height when required");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit.reason.contains("expires_at_height"),
                    "expected expires_at_height failure, got {limit:?}"
                );
            }
            other => panic!("expected TransactionLimit failure, got {other:?}"),
        }
    }

    #[test]
    fn accept_transaction_requires_tx_sequence_when_configured() {
        use std::time::Duration;

        use iroha_data_model::isi::Log;
        use iroha_logger::Level;

        let chain: ChainId = "sequence-config-chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "seq".into())])
        .sign(kp.private_key());

        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            default_limits.max_signatures(),
            default_limits.max_instructions(),
            default_limits.ivm_bytecode_size(),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        )
        .with_ingress_enforcement(false, true);
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();

        let err =
            AcceptedTransaction::accept(tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect_err("transactions must provide tx_sequence when required");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit.reason.contains("tx_sequence"),
                    "expected tx_sequence failure, got {limit:?}"
                );
            }
            other => panic!("expected TransactionLimit failure, got {other:?}"),
        }
    }

    #[test]
    fn signature_verification_result_reports_invalid_signature() {
        use std::time::Duration;

        let chain: ChainId = "sig-check".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let (other_id, _other_kp) = gen_account_in("underland");

        let tx = TransactionBuilder::new(
            chain,
            authority_id,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "sig".into())])
        .sign(kp.private_key());
        let tampered = tx.with_authority(other_id);

        let err =
            AcceptedTransaction::signature_verification_result(&tampered).expect_err("must fail");
        assert_eq!(err.code, SignatureRejectionCode::InvalidSignature);

        let now = tampered.creation_time();
        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let chain_id = tampered.chain().clone();
        let result = AcceptedTransaction::validate_with_now(
            &tampered,
            &chain_id,
            Duration::ZERO,
            limits,
            &crypto_cfg,
            now,
        );
        assert!(matches!(
            result,
            Err(AcceptTransactionFail::SignatureVerification(err))
                if err.code == SignatureRejectionCode::InvalidSignature
        ));
    }

    #[test]
    fn retired_heartbeat_string_marker_is_rejected() {
        use std::time::Duration;

        let chain: ChainId = "heartbeat-marker-true".parse().unwrap();
        let signer = checked_random_tx_keypair_with_algorithm(Algorithm::Ed25519);
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let authority = AccountId::new(signer.public_key().clone());
        let mut metadata = Metadata::default();
        metadata.insert(HEARTBEAT_METADATA_NAME.clone(), Json::new("true"));

        let tx = TransactionBuilder::new_with_time_source(
            chain.clone(),
            authority,
            &time_source,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_metadata(metadata)
        .sign(signer.private_key());
        let tx_params = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();

        let error = AcceptedTransaction::accept_with_time_source(
            tx,
            &chain,
            Duration::ZERO,
            tx_params,
            &crypto_cfg,
            &time_source,
        )
        .expect_err("retired string heartbeat marker must be rejected");
        assert!(matches!(
            error,
            AcceptTransactionFail::TransactionLimit(limit)
                if limit.reason.contains("sumeragi_heartbeat")
                    && limit.reason.contains("retired")
        ));
    }

    #[test]
    fn retired_heartbeat_false_marker_is_rejected() {
        use std::time::Duration;

        let chain: ChainId = "heartbeat-marker-false".parse().unwrap();
        let signer = checked_random_tx_keypair_with_algorithm(Algorithm::Ed25519);
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let authority = AccountId::new(signer.public_key().clone());
        let mut metadata = Metadata::default();
        metadata.insert(HEARTBEAT_METADATA_NAME.clone(), Json::new(false));

        let tx = TransactionBuilder::new_with_time_source(
            chain.clone(),
            authority,
            &time_source,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_metadata(metadata)
        .sign(signer.private_key());
        let tx_params = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();

        let err = AcceptedTransaction::accept_with_time_source(
            tx,
            &chain,
            Duration::ZERO,
            tx_params,
            &crypto_cfg,
            &time_source,
        )
        .expect_err("false heartbeat marker should be rejected");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit.reason.contains("sumeragi_heartbeat"),
                    "expected heartbeat marker rejection, got {limit:?}"
                );
            }
            other => panic!("expected TransactionLimit failure, got {other:?}"),
        }
    }

    #[test]
    fn retired_heartbeat_arbitrary_marker_is_rejected() {
        use std::time::Duration;

        let chain: ChainId = "heartbeat-marker-invalid".parse().unwrap();
        let signer = checked_random_tx_keypair_with_algorithm(Algorithm::Ed25519);
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let authority = AccountId::new(signer.public_key().clone());
        let mut metadata = Metadata::default();
        metadata.insert(HEARTBEAT_METADATA_NAME.clone(), Json::new("nope"));

        let tx = TransactionBuilder::new_with_time_source(
            chain.clone(),
            authority,
            &time_source,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_metadata(metadata)
        .sign(signer.private_key());
        let tx_params = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();

        let err = AcceptedTransaction::accept_with_time_source(
            tx,
            &chain,
            Duration::ZERO,
            tx_params,
            &crypto_cfg,
            &time_source,
        )
        .expect_err("invalid heartbeat marker should be rejected");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit.reason.contains("sumeragi_heartbeat"),
                    "expected heartbeat marker rejection, got {limit:?}"
                );
            }
            other => panic!("expected TransactionLimit failure, got {other:?}"),
        }
    }

    #[test]
    fn retired_heartbeat_marker_rejects_non_empty_transactions() {
        use std::time::Duration;

        use iroha_data_model::isi::Log;
        use iroha_logger::Level;

        let chain: ChainId = "heartbeat-metadata-chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let mut metadata = Metadata::default();
        metadata.insert(HEARTBEAT_METADATA_NAME.clone(), Json::new(true));

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "noop".into())])
        .with_metadata(metadata)
        .sign(kp.private_key());

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();

        let err =
            AcceptedTransaction::accept(tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect_err("retired heartbeat marker must reject a non-empty transaction");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit.reason.contains("sumeragi_heartbeat") && limit.reason.contains("retired"),
                    "expected heartbeat rejection, got {limit:?}"
                );
            }
            other => panic!("expected retired heartbeat rejection, got {other:?}"),
        }
    }

    #[test]
    fn transaction_expired_at_height_is_rejected_by_state() {
        use std::time::Duration;

        use iroha_data_model::{isi::Log, metadata::Metadata, transaction::TransactionBuilder};
        use iroha_logger::Level;
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        let (mut world, authority_id, kp) = world_with_authority("wonderland");
        let mut params = iroha_data_model::parameter::system::Parameters::default();
        params.transaction = params.transaction.with_ingress_enforcement(true, false);
        world.parameters = mv::cell::Cell::new(params);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "ttl-check-chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let mut metadata = Metadata::default();
        metadata.insert(
            iroha_data_model::name::Name::from_str("expires_at_height").unwrap(),
            Json::from(0_u64),
        );

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "ttl".into())])
        .with_metadata(metadata)
        .sign(kp.private_key());

        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            default_limits.max_signatures(),
            default_limits.max_instructions(),
            default_limits.ivm_bytecode_size(),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        )
        .with_ingress_enforcement(true, false);
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted =
            AcceptedTransaction::accept(tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect("stateless TTL checks should pass when metadata present");

        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
                assert!(
                    msg.contains("expired"),
                    "expected expiry rejection, got {msg}"
                );
            }
            other => {
                panic!("expected Validation::NotPermitted for expired transaction, got {other:?}")
            }
        }
    }

    #[test]
    fn sequence_not_increasing_is_rejected_by_state() {
        use std::time::Duration;

        use iroha_data_model::{isi::Log, metadata::Metadata, transaction::TransactionBuilder};
        use iroha_logger::Level;
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        let (mut world, authority_id, kp) = world_with_authority("wonderland");
        world.tx_sequences.insert(authority_id.clone(), 5);
        let mut params = iroha_data_model::parameter::system::Parameters::default();
        params.transaction = params.transaction.with_ingress_enforcement(false, true);
        world.parameters = mv::cell::Cell::new(params);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "seq-check-chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let mut metadata = Metadata::default();
        metadata.insert(
            iroha_data_model::name::Name::from_str("tx_sequence").unwrap(),
            Json::from(5_u64),
        );

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "seq".into())])
        .with_metadata(metadata)
        .sign(kp.private_key());

        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            default_limits.max_signatures(),
            default_limits.max_instructions(),
            default_limits.ivm_bytecode_size(),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        )
        .with_ingress_enforcement(false, true);
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted =
            AcceptedTransaction::accept(tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect("stateless sequence checks should pass when metadata present");

        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
                assert!(
                    msg.contains("sequence"),
                    "expected sequence rejection, got {msg}"
                );
            }
            other => panic!(
                "expected Validation::NotPermitted for non-increasing sequence, got {other:?}"
            ),
        }
    }

    #[test]
    fn sequence_increasing_is_accepted_by_state() {
        use std::time::Duration;

        use iroha_data_model::{isi::Log, metadata::Metadata, transaction::TransactionBuilder};
        use iroha_logger::Level;
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        let (mut world, authority_id, kp) = world_with_authority("wonderland");
        world.tx_sequences.insert(authority_id.clone(), 5);
        let mut params = iroha_data_model::parameter::system::Parameters::default();
        params.transaction = params.transaction.with_ingress_enforcement(false, true);
        world.parameters = mv::cell::Cell::new(params);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "seq-accept-chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let mut metadata = Metadata::default();
        metadata.insert(
            iroha_data_model::name::Name::from_str("tx_sequence").unwrap(),
            Json::from(6_u64),
        );

        let tx = TransactionBuilder::new(
            chain.clone(),
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "seq".into())])
        .with_metadata(metadata)
        .sign(kp.private_key());

        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            default_limits.max_signatures(),
            default_limits.max_instructions(),
            default_limits.ivm_bytecode_size(),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        )
        .with_ingress_enforcement(false, true);
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted =
            AcceptedTransaction::accept(tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect("stateless sequence checks should pass when metadata present");

        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        result.expect("sequence should be accepted");

        let updated = block
            .world
            .tx_sequences
            .get(&authority_id)
            .copied()
            .expect("sequence entry must exist");
        assert_eq!(updated, 6);
    }

    #[test]
    fn custom_parameter_cannot_disable_configured_ivm_cycle_ceiling() {
        use iroha_data_model::{
            parameter::{
                Parameter,
                custom::{CustomParameter, CustomParameterId},
            },
            prelude::Name,
            transaction::{Executable, TransactionBuilder},
        };
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let mut state = State::new_with_chain(world, kura, query_handle, chain.clone());
        let mut pipeline = state.pipeline.clone();
        pipeline.ivm_max_cycles_upper_bound =
            std::num::NonZeroU64::new(1_000).expect("test ceiling is non-zero");
        state.set_pipeline(pipeline);

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        // A consensus custom parameter with the retired name must not disable
        // or override the node's configured production admission policy.
        let id = CustomParameterId::new(Name::from_str("max_ivm_cycles_upper_bound").unwrap());
        let custom = CustomParameter::new(id, Json::new(0_u64));
        block
            .world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Custom(custom));

        // Build program with max_cycles = 2000
        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_program_with_max_cycles(1, 2_000);
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::MaxCyclesExceedsUpperBound(info),
            ))) => {
                assert_eq!(info.max_cycles, 2_000);
                assert_eq!(info.upper_bound, 1_000);
            }
            other => panic!("configured cycle ceiling must reject the program, got {other:?}"),
        }
    }

    #[test]
    fn configured_ivm_cycle_ceiling_accepts_within_bound_despite_custom_parameter() {
        use iroha_data_model::{
            parameter::{
                Parameter,
                custom::{CustomParameter, CustomParameterId},
            },
            prelude::Name,
            transaction::{Executable, TransactionBuilder},
        };
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        let (world, authority_id, kp) = world_with_authority("wonderland");
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);
        let mut pipeline = state.pipeline.clone();
        pipeline.ivm_max_cycles_upper_bound =
            std::num::NonZeroU64::new(4_000).expect("test ceiling is non-zero");
        state.set_pipeline(pipeline);

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        // The retired custom parameter cannot lower the configured ceiling.
        let id = CustomParameterId::new(Name::from_str("max_ivm_cycles_upper_bound").unwrap());
        let custom = CustomParameter::new(id, Json::new(1_u64));
        block
            .world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Custom(custom));

        // Build program with max_cycles = 2000, below the bound
        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_program_with_max_cycles(1, 2_000);
        let tx = TransactionBuilder::new(
            chain,
            authority_id.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
        .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        assert!(result.is_ok(), "max_cycles within bound should pass");
    }

    mod time_trigger {
        use super::*;

        /// # Scenario
        ///
        /// 1. Transaction: Alice sends a large donation to Bob.
        /// 2. Data trigger: Bob forwards the donation to Carol.
        /// 3. Time trigger: Carol attempts to send the donation to Dave; this should fail if step 2 did not occur.
        /// 4. Data trigger: Dave forwards the donation to Eve.
        #[tokio::test]
        async fn fires_after_external_transactions() {
            let mut sandbox = Sandbox::default()
                .with_data_trigger_transfer("bob", 50, "carol")
                .with_time_trigger_transfer("carol", 50, "dave")
                .with_data_trigger_transfer("dave", 50, "eve");
            sandbox.request_transfer("alice", 50, "bob");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _) = block.apply();
            assert_events(&events, "time_trigger/fires_after_external_transactions");
            block.assert_balances([
                ("alice", 10),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 60),
            ]);
        }
    }

    mod data_trigger {
        use super::*;

        /// # Scenario
        ///
        /// 1. Transaction: Alice sends a large donation to Bob.
        /// 2. Data trigger: Bob forwards the donation to Carol.
        /// 3. Transaction: Carol attempts to send the donation to Dave; this should fail if step 2 did not occur.
        #[tokio::test]
        async fn fires_for_each_transaction() {
            let mut sandbox = Sandbox::default().with_data_trigger_transfer("bob", 50, "carol");
            sandbox.request_transfer("alice", 50, "bob");
            sandbox.request_transfer("carol", 50, "dave");
            let mut block = sandbox.block();
            block.assert_balances([("alice", 60), ("bob", 10), ("carol", 10), ("dave", 10)]);
            let (events, _committed_block) = block.apply();
            assert_events(&events, "data_trigger/fires_for_each_transaction");
            block.assert_balances([("alice", 10), ("bob", 10), ("carol", 10), ("dave", 60)]);
        }

        /// # Scenario
        ///
        /// 1. Transaction: Alice sends the asset to Bob in two separate packages, emitting two events.
        /// 2. Data trigger: Bob forwards each package to Carol; it fires once per matching instruction,
        ///    even when the events are emitted within the same transaction.
        #[tokio::test]
        async fn fires_for_each_matching_instruction() {
            let mut sandbox = Sandbox::default().with_data_trigger_transfer("bob", 10, "carol");
            sandbox.request_transfers_batched::<2>("alice", 10, "bob");
            let mut block = sandbox.block();
            block.assert_balances([("alice", 60), ("bob", 10), ("carol", 10)]);
            let (events, _committed_block) = block.apply();
            assert_events(&events, "data_trigger/fires_for_each_matching_instruction");
            block.assert_balances([("alice", 40), ("bob", 10), ("carol", 30)]);
        }

        /// # Scenario
        ///
        /// 1. Transaction: Alice sends a large donation to Bob.
        /// 2. Data triggers: Bob forwards the donation to Carol, Carol forwards it to Dave, and Dave forwards it back to Bob.
        /// 3. Data trigger: Bob forwards the donation to Eve; this should fail if step 2 has not completed.
        #[tokio::test]
        async fn chains_in_depth_first_order() {
            let mut sandbox = Sandbox::default()
                // Carol receives it before Eve because triggers matching the same event are processed in lexicographical order of their IDs.
                .with_data_trigger_transfer_once("bob", 50, "carol")
                // Sibling trigger waits for depth-first resolution.
                .with_data_trigger_transfer_once("bob", 50, "eve")
                .with_data_trigger_transfer("carol", 50, "dave")
                .with_data_trigger_transfer("dave", 50, "bob");
            sandbox.request_transfer("alice", 50, "bob");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _committed_block) = block.apply();
            assert_events(&events, "data_trigger/chains_in_depth_first_order");
            block.assert_balances([
                ("alice", 10),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 60),
            ]);
        }

        /// # Scenario
        ///
        /// 1. Transaction: Alice sends 50 units to Bob.
        /// 2. Data triggers: each branch (Bob -> Carol -> Dave -> Eve) runs independently to a max depth of 3, forwarding 1 unit per step.
        #[tokio::test]
        async fn each_branch_is_assigned_depth() {
            let mut sandbox = Sandbox::default()
                .with_max_execution_depth(3)
                // Branches: Bob -> Carol
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 0)
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 1)
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 2)
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 3)
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 4)
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 5)
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 6)
                // Common path: Carol -> Dave -> Eve
                .with_data_trigger_transfer("carol", 1, "dave")
                .with_data_trigger_transfer("dave", 1, "eve");
            sandbox.request_transfer("alice", 50, "bob");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _committed_block) = block.apply();
            assert_events(&events, "data_trigger/each_branch_is_assigned_depth");
            block.assert_balances([
                ("alice", 10),
                ("bob", 53),
                ("carol", 10),
                ("dave", 10),
                ("eve", 17),
            ]);
        }

        /// All or none of the initial transaction and subsequent data triggers should take effect.
        #[tokio::test]
        async fn atomically_chains_from_transaction() {
            let sandbox = || {
                let mut res = Sandbox::default();
                res.request_transfer("alice", 50, "bob");
                res
            };

            aborts_on_execution_error(sandbox(), "txn");
            aborts_on_exceeding_depth(sandbox(), "txn");
            commits_on_depleting_lives(sandbox(), "txn");
            commits_on_regular_success(sandbox(), "txn");
        }

        /// All or none of the initial time trigger and subsequent data triggers should take effect.
        #[tokio::test]
        async fn atomically_chains_from_time_trigger() {
            let sandbox = || Sandbox::default().with_time_trigger_transfer("alice", 50, "bob");

            aborts_on_execution_error(sandbox(), "time");
            aborts_on_exceeding_depth(sandbox(), "time");
            commits_on_depleting_lives(sandbox(), "time");
            commits_on_regular_success(sandbox(), "time");
        }

        /// Negative transfer amounts cannot cross the nominal quantity boundary.
        #[test]
        fn negative_transfer_amount_cannot_be_constructed() {
            let negative = Numeric::try_new(-1_i128, 0).expect("negative numeric amount");
            assert!(Quantity::try_from_numeric(negative).is_err());
        }

        /// Data trigger chains must roll back when a transfer uses a zero amount.
        #[tokio::test]
        async fn atomically_aborts_on_zero_amount_from_transaction() {
            let sandbox = || {
                let mut res = Sandbox::default();
                res.request_transfer("alice", 50, "bob");
                res
            };

            aborts_on_zero_amount(sandbox(), "txn");
        }

        /// Zero transfer amounts should abort chains initiated by time triggers as well.
        #[tokio::test]
        async fn atomically_aborts_on_zero_amount_from_time_trigger() {
            let sandbox = || Sandbox::default().with_time_trigger_transfer("alice", 50, "bob");

            aborts_on_zero_amount(sandbox(), "time");
        }

        fn aborts_on_execution_error(sandbox: Sandbox, snapshot_suffix: &str) {
            let mut sandbox = sandbox
                .with_data_trigger_transfer("bob", 10, "carol")
                .with_data_trigger_transfer("bob", 10, "dave")
                // This trigger execution fails.
                .with_data_trigger_transfer("dave", 500, "eve");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _committed_block) = block.apply();
            let data_events = events
                .iter()
                .filter(|event| matches!(event, EventBox::Data(_)))
                .count();
            assert_eq!(
                data_events, 0,
                "failing data trigger must not emit persisted data events"
            );
            assert_events(
                &events,
                format!("data_trigger/aborts_on_execution_error-{snapshot_suffix}"),
            );
            // Everything should be rolled back.
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
        }

        fn aborts_on_zero_amount(sandbox: Sandbox, snapshot_suffix: &str) {
            let mut sandbox = sandbox
                .with_data_trigger_transfer("bob", 10, "carol")
                .with_data_trigger_transfer("bob", 10, "dave")
                .with_data_trigger_transfer_quantity("dave", Quantity::zero(), "eve");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _committed_block) = block.apply();
            let data_events = events
                .iter()
                .filter(|event| matches!(event, EventBox::Data(_)))
                .count();
            assert_eq!(
                data_events, 0,
                "failing data trigger must not emit persisted data events"
            );
            assert_events(
                &events,
                format!("data_trigger/aborts_on_zero_amount-{snapshot_suffix}"),
            );
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
        }

        fn aborts_on_exceeding_depth(sandbox: Sandbox, snapshot_suffix: &str) {
            let mut sandbox = sandbox
                .with_max_execution_depth(2)
                .with_data_trigger_transfer("bob", 50, "carol")
                .with_data_trigger_transfer("carol", 50, "dave")
                // The execution sequence exceeds the depth limit.
                .with_data_trigger_transfer("dave", 50, "eve");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _committed_block) = block.apply();
            assert_events(
                &events,
                format!("data_trigger/aborts_on_exceeding_depth-{snapshot_suffix}"),
            );
            // Everything should be rolled back.
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
        }

        fn commits_on_depleting_lives(sandbox: Sandbox, snapshot_suffix: &str) {
            let mut sandbox = sandbox
                .with_data_trigger_transfer("bob", 50, "carol")
                // This trigger depletes after an execution.
                .with_data_trigger_transfer_once("carol", 50, "bob");
            let mut block = sandbox.block();
            block.assert_balances([("alice", 60), ("bob", 10), ("carol", 10)]);
            let (events, _committed_block) = block.apply();
            assert_events(
                &events,
                format!("data_trigger/commits_on_depleting_lives-{snapshot_suffix}"),
            );
            // The execution sequence should take effect.
            block.assert_balances([("alice", 10), ("bob", 10), ("carol", 60)]);
        }

        fn commits_on_regular_success(sandbox: Sandbox, snapshot_suffix: &str) {
            let mut sandbox = sandbox
                .with_max_execution_depth(3)
                .with_data_trigger_transfer("bob", 50, "carol")
                .with_data_trigger_transfer("carol", 50, "dave")
                .with_data_trigger_transfer("dave", 50, "eve");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _committed_block) = block.apply();
            assert_events(
                &events,
                format!("data_trigger/commits_on_regular_success-{snapshot_suffix}"),
            );
            // The execution sequence should take effect.
            block.assert_balances([
                ("alice", 10),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 60),
            ]);
        }
    }

    include!("tx/empty_instruction_test.rs");

    #[test]
    fn lane_privacy_proofs_collected_from_attachments() {
        let chain: ChainId = "lane-privacy-collect".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let backend = Ident::from_str("halo2/ipa").expect("ident");

        let proof1 = LanePrivacyProof {
            commitment_id: LaneCommitmentId::new(1),
            witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
                leaf: [0x11; 32],
                proof: MerkleProof::from_audit_path_bytes(0, vec![[0x33; 32]]),
            }),
        };
        let proof2 = LanePrivacyProof {
            commitment_id: LaneCommitmentId::new(2),
            witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
                leaf: [0x22; 32],
                proof: MerkleProof::from_audit_path_bytes(0, vec![[0x44; 32]]),
            }),
        };

        let mut attachment1 = ProofAttachment::new_ref(
            backend.clone(),
            ProofBox::new(backend.clone(), vec![0xAA]),
            VerifyingKeyId::new(backend.clone(), "vk_lane_1"),
        );
        attachment1.lane_privacy = Some(proof1.clone());
        let mut attachment2 = ProofAttachment::new_ref(
            backend.clone(),
            ProofBox::new(backend.clone(), vec![0xCC]),
            VerifyingKeyId::new(backend, "vk_lane_2"),
        );
        attachment2.lane_privacy = Some(proof2.clone());

        let tx = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "noop".into())])
        .with_attachments(
            ProofAttachmentList::try_from(vec![attachment1, attachment2])
                .expect("two attachments are a valid bounded proof list"),
        )
        .sign(keypair.private_key());

        let collected_proofs = super::collect_lane_privacy_proofs(&tx);
        let ids: BTreeSet<_> = collected_proofs
            .iter()
            .map(|proof| proof.commitment_id())
            .collect();
        assert_eq!(collected_proofs.len(), 2);
        assert!(ids.contains(&LaneCommitmentId::new(1)));
        assert!(ids.contains(&LaneCommitmentId::new(2)));
    }

    #[test]
    fn state_manifest_quorum_requires_approvers() {
        let chain: ChainId = "lane-manifest-quorum".parse().unwrap();
        let primary_keypair = checked_fixture_keypair(vec![0x11; 32], Algorithm::Ed25519);
        let secondary_keypair = checked_fixture_keypair(vec![0x22; 32], Algorithm::Ed25519);
        let primary_id = AccountId::new(primary_keypair.public_key().clone());
        let secondary_id = AccountId::new(secondary_keypair.public_key().clone());

        let rules = GovernanceRules {
            validators: vec![primary_id.clone(), secondary_id.clone()],
            quorum: Some(2),
            ..GovernanceRules::default()
        };
        let lane_alias = "gov";

        let tx = TransactionBuilder::new(
            chain.clone(),
            primary_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "noop".into())])
        .sign(primary_keypair.private_key());
        match enforce_manifest_quorum(lane_alias, &rules, &tx) {
            Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
                assert!(
                    msg.contains("quorum"),
                    "expected quorum rejection, got {msg}"
                );
            }
            other => panic!("expected quorum rejection, got {other:?}"),
        }

        let mut metadata = Metadata::default();
        metadata.insert(
            (*super::GOV_APPROVERS_METADATA_KEY).clone(),
            Json::new(vec![secondary_id.to_string()]),
        );
        let tx = TransactionBuilder::new(
            chain,
            primary_id,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "noop".into())])
        .with_metadata(metadata)
        .sign(primary_keypair.private_key());
        let result = enforce_manifest_quorum(lane_alias, &rules, &tx);
        assert!(result.is_ok(), "quorum satisfied should pass: {result:?}");
    }

    #[test]
    fn state_manifest_quorum_rejects_duplicate_validators() {
        let primary_keypair = checked_fixture_keypair(vec![0x11; 32], Algorithm::Ed25519);
        let primary_id = AccountId::new(primary_keypair.public_key().clone());
        let rules = GovernanceRules {
            validators: vec![primary_id.clone(), primary_id],
            ..GovernanceRules::default()
        };

        match super::canonical_manifest_validators("gov", &rules) {
            Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
                assert!(
                    msg.contains("duplicate validators"),
                    "expected duplicate validator rejection, got {msg}"
                );
            }
            other => panic!("expected duplicate validator rejection, got {other:?}"),
        }
    }

    #[test]
    fn state_manifest_quorum_rejects_duplicate_approvers() {
        let chain: ChainId = "lane-manifest-duplicate-approvers".parse().unwrap();
        let primary_keypair = checked_fixture_keypair(vec![0x11; 32], Algorithm::Ed25519);
        let secondary_keypair = checked_fixture_keypair(vec![0x22; 32], Algorithm::Ed25519);
        let primary_id = AccountId::new(primary_keypair.public_key().clone());
        let secondary_id = AccountId::new(secondary_keypair.public_key().clone());
        let rules = GovernanceRules {
            validators: vec![primary_id.clone(), secondary_id.clone()],
            quorum: Some(2),
            ..GovernanceRules::default()
        };

        let mut metadata = Metadata::default();
        metadata.insert(
            (*super::GOV_APPROVERS_METADATA_KEY).clone(),
            Json::new(vec![secondary_id.to_string(), secondary_id.to_string()]),
        );
        let tx = TransactionBuilder::new(
            chain,
            primary_id,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "noop".into())])
        .with_metadata(metadata)
        .sign(primary_keypair.private_key());

        match enforce_manifest_quorum("gov", &rules, &tx) {
            Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
                assert!(
                    msg.contains("duplicate approvers"),
                    "expected duplicate approver rejection, got {msg}"
                );
            }
            other => panic!("expected duplicate approver rejection, got {other:?}"),
        }
    }

    #[test]
    fn manifest_protected_namespaces_require_metadata() {
        let chain: ChainId = "lane-protected-ns".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let mut rules = GovernanceRules::default();
        rules
            .protected_namespaces
            .insert(Name::from_str("apps").expect("namespace"));

        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let instruction = iroha_data_model::isi::smart_contract_code::ActivateContractInstance {
            contract_address,
            code_hash: Hash::prehashed([0_u8; 32]),
        };
        let tx = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(keypair.private_key());

        let world = World::default();
        let world_view = world.view();
        let err = super::enforce_manifest_protected_namespaces("lane-0", &rules, &tx, &world_view)
            .expect_err("missing governance metadata should reject");
        match err {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
                assert!(
                    msg.contains("gov_contract_address"),
                    "expected gov_contract_address rejection, got {msg}"
                );
            }
            other => panic!("expected NotPermitted rejection, got {other:?}"),
        }
    }

    #[test]
    fn manifest_protected_rotation_requires_atomic_commit_instruction() {
        let chain: ChainId = "lane-protected-atomic-rotation".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let mut rules = GovernanceRules::default();
        rules
            .protected_namespaces
            .insert(Name::from_str("apps").expect("namespace"));

        let old_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("old contract address");
        let new_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            1,
            DataSpaceId::UNIVERSAL,
        )
        .expect("new contract address");
        let code_hash = Hash::new(b"atomic protected rotation");
        let mut metadata = Metadata::default();
        metadata.insert(
            (*super::GOV_CONTRACT_ADDRESS_METADATA_KEY).clone(),
            Json::new(new_address.to_string()),
        );
        let commit = CommitContractDeployment {
            expected_deploy_nonce: 1,
            contract_address: new_address.clone(),
            code_hash,
            contract_alias: "payments::universal".parse().expect("contract alias"),
            lease_expiry_ms: None,
            expected_previous_contract_address: Some(old_address.clone()),
        };
        let tx = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([commit])
        .with_metadata(metadata.clone())
        .sign(keypair.private_key());
        let world = World::default();
        let world_view = world.view();
        super::enforce_manifest_protected_namespaces("lane-0", &rules, &tx, &world_view)
            .expect("single atomic deployment commit should pass protected address validation");

        let legacy = vec![
            InstructionBox::from(DeactivateContractInstance {
                contract_address: old_address,
                reason: Some("legacy rotation".to_owned()),
            }),
            InstructionBox::from(ActivateContractInstance {
                contract_address: new_address,
                code_hash,
            }),
        ];
        let tx = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(legacy)
        .with_metadata(metadata)
        .sign(keypair.private_key());
        let error =
            super::enforce_manifest_protected_namespaces("lane-0", &rules, &tx, &world_view)
                .expect_err("legacy multi-instruction rotation must be rejected");
        match error {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(message)) => {
                assert!(
                    message.contains("CommitContractDeployment"),
                    "unexpected protected rotation rejection: {message}"
                );
            }
            other => panic!("expected NotPermitted rejection, got {other:?}"),
        }
    }

    #[test]
    fn state_block_manifest_protects_native_contract_upload_lifecycle() {
        let chain: ChainId = "lane-native-upload-protected-ns".parse().unwrap();
        let (mut world, authority, keypair) = world_with_authority("wonderland");
        let lifecycle_permission: Permission =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode
                .into();
        world
            .account_permissions
            .insert(authority.clone(), Permissions::from([lifecycle_permission]));

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());
        let mut protected_namespaces = BTreeSet::new();
        protected_namespaces.insert(Name::from_str("apps").expect("protected namespace"));
        let rules = GovernanceRules {
            validators: vec![authority.clone()],
            protected_namespaces,
            ..GovernanceRules::default()
        };
        let mut statuses = BTreeMap::new();
        statuses.insert(
            TestLaneId::SINGLE,
            LaneManifestStatus {
                lane: TestLaneId::SINGLE,
                alias: "apps".to_owned(),
                dataspace: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: Some("parliament".to_owned()),
                manifest_path: Some(PathBuf::from("/tmp/apps.manifest.json")),
                governance_rules: Some(rules),
                privacy_commitments: Vec::new(),
            },
        );
        state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(statuses)));

        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let mut governance_metadata = Metadata::default();
        governance_metadata.insert(
            (*super::GOV_CONTRACT_ADDRESS_METADATA_KEY).clone(),
            Json::new(contract_address.to_string()),
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        macro_rules! validate_instruction {
            ($instruction:expr, $metadata:expr) => {{
                let tx = TransactionBuilder::new(
                    chain.clone(),
                    authority.clone(),
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([$instruction])
                .with_metadata($metadata)
                .sign(keypair.private_key());
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
                let mut ivm_cache = IvmCache::new();
                block.validate_transaction(accepted, &mut ivm_cache).1
            }};
        }

        let (code, _) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(
                "seiyaku NativeUploadGovernance { view fn inspect() -> int { return 1; } }",
            )
            .expect("compile self-describing contract fixture");
        assert!(
            code.len()
                <= iroha_data_model::isi::smart_contract_code::SMART_CONTRACT_CODE_CHUNK_BYTES,
            "governance fixture must fit the declared one-chunk upload"
        );
        let code_hash = ivm::contract_code_hash(&code);
        let total_size = u64::try_from(code.len()).expect("contract fixture size fits u64");
        let descriptor = crate::state::SmartContractCodeUploadDescriptor {
            total_size,
            chunk_count: 1,
        };

        let missing_upload_metadata = validate_instruction!(
            UploadSmartContractCodeChunk {
                code_hash,
                total_size,
                chunk_index: 0,
                chunk_count: 1,
                chunk: code.clone(),
            },
            Metadata::default()
        );
        assert!(matches!(
            missing_upload_metadata,
            Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(message)
            )) if message.contains("gov_contract_address")
        ));
        assert_eq!(
            block
                .world
                .contract_code_upload_progress(&authority, &code_hash),
            None,
            "rejected upload must not create resumable staging"
        );

        let accepted_upload = validate_instruction!(
            UploadSmartContractCodeChunk {
                code_hash,
                total_size,
                chunk_index: 0,
                chunk_count: 1,
                chunk: code.clone(),
            },
            governance_metadata.clone()
        );
        assert!(
            accepted_upload.is_ok(),
            "governance metadata should admit upload: {accepted_upload:?}"
        );
        assert_eq!(
            block
                .world
                .contract_code_upload_progress(&authority, &code_hash),
            Some(crate::state::SmartContractCodeUploadProgress {
                descriptor,
                received_chunks: 1,
            })
        );

        let missing_finalize_metadata = validate_instruction!(
            FinalizeSmartContractCodeUpload {
                code_hash,
                total_size,
                chunk_count: 1,
            },
            Metadata::default()
        );
        assert!(matches!(
            missing_finalize_metadata,
            Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(message)
            )) if message.contains("gov_contract_address")
        ));
        assert_eq!(
            block
                .world
                .contract_code_upload_progress(&authority, &code_hash),
            Some(crate::state::SmartContractCodeUploadProgress {
                descriptor,
                received_chunks: 1,
            }),
            "rejected finalization must preserve resumable staging"
        );
        assert!(block.world.contract_code().get(&code_hash).is_none());

        let accepted_finalize = validate_instruction!(
            FinalizeSmartContractCodeUpload {
                code_hash,
                total_size,
                chunk_count: 1,
            },
            governance_metadata.clone()
        );
        assert!(
            accepted_finalize.is_ok(),
            "governance metadata should admit finalization: {accepted_finalize:?}"
        );
        assert_eq!(
            block
                .world
                .contract_code()
                .get(&code_hash)
                .map(Vec::as_slice),
            Some(code.as_slice())
        );
        assert_eq!(
            block
                .world
                .contract_code_upload_progress(&authority, &code_hash),
            None,
            "successful finalization must clear staging"
        );

        let cancelled_hash = Hash::new(b"owner-scoped cleanup");
        let accepted_cancel_stage = validate_instruction!(
            UploadSmartContractCodeChunk {
                code_hash: cancelled_hash,
                total_size: 1,
                chunk_index: 0,
                chunk_count: 1,
                chunk: vec![0xCA],
            },
            governance_metadata
        );
        assert!(
            accepted_cancel_stage.is_ok(),
            "cleanup fixture upload should be staged: {accepted_cancel_stage:?}"
        );
        let accepted_cancel = validate_instruction!(
            iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload {
                code_hash: cancelled_hash,
            },
            Metadata::default()
        );
        assert!(
            accepted_cancel.is_ok(),
            "owner cleanup must remain outside protected deployment governance: {accepted_cancel:?}"
        );
        assert_eq!(
            block
                .world
                .contract_code_upload_progress(&authority, &cancelled_hash),
            None
        );
    }

    #[test]
    fn generic_ivm_cannot_hide_contract_admin_syscalls_from_governance() {
        use iroha_data_model::transaction::{Executable, executable::IvmBytecode};

        let chain: ChainId = "generic-ivm-contract-admin-governance".parse().unwrap();
        let (mut world, authority, keypair) = world_with_authority("wonderland");
        let (second_validator, _) = gen_account_in("wonderland");
        let lifecycle_permission: Permission =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode
                .into();
        world
            .account_permissions
            .insert(authority.clone(), Permissions::from([lifecycle_permission]));

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());
        let mut protected_namespaces = BTreeSet::new();
        protected_namespaces.insert(Name::from_str("apps").expect("protected namespace"));
        let rules = GovernanceRules {
            validators: vec![authority.clone(), second_validator],
            quorum: Some(2),
            protected_namespaces,
            ..GovernanceRules::default()
        };
        let mut statuses = BTreeMap::new();
        statuses.insert(
            TestLaneId::SINGLE,
            LaneManifestStatus {
                lane: TestLaneId::SINGLE,
                alias: "apps".to_owned(),
                dataspace: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: Some("parliament".to_owned()),
                manifest_path: Some(PathBuf::from("/tmp/apps.manifest.json")),
                governance_rules: Some(rules),
                privacy_commitments: Vec::new(),
            },
        );
        state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(statuses)));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        for syscall in [
            ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
            ivm::syscalls::SYSCALL_ACTIVATE_CONTRACT_INSTANCE,
        ] {
            let syscall_u8 = u8::try_from(syscall).expect("contract-admin syscall fits SCALL");
            let program = minimal_ivm_program_with_syscall(1, syscall_u8);
            let tx = TransactionBuilder::new(
                chain.clone(),
                authority.clone(),
                fee_payment_with_gas_limit(TEST_GAS_LIMIT),
            )
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
            .sign(keypair.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
            let mut ivm_cache = IvmCache::new();
            let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);

            assert!(matches!(
                result,
                Err(TransactionRejectionReason::Validation(
                    ValidationFail::IvmAdmission(
                        iroha_data_model::executor::IvmAdmissionError::GenericSyscallNotAllowed(
                            rejected
                        )
                    )
                )) if rejected == syscall
            ));
            assert!(
                block.world.contract_code().iter().next().is_none(),
                "rejected generic syscall must not register contract bytes"
            );
            assert!(
                block.world.contract_instances().iter().next().is_none(),
                "rejected generic syscall must not activate a contract instance"
            );
        }
    }

    #[test]
    fn runtime_upgrade_hook_requires_metadata() {
        let chain: ChainId = "lane-runtime-hook".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");

        let mut rules = GovernanceRules::default();
        rules.hooks.runtime_upgrade = Some(RuntimeUpgradeHook {
            allow: true,
            require_metadata: true,
            metadata_key: Some(Name::from_str("upgrade_id").expect("key")),
            allowed_ids: Some(BTreeSet::from(["v1".to_string()])),
        });

        let instruction = iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade {
            manifest_bytes: vec![0x01, 0x02],
        };
        let tx = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction.clone()])
        .sign(keypair.private_key());

        let err = super::enforce_runtime_upgrade_hook("lane-0", &rules, &tx)
            .expect_err("missing metadata should reject");
        match err {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
                assert!(
                    msg.contains("requires metadata"),
                    "expected metadata rejection, got {msg}"
                );
            }
            other => panic!("expected NotPermitted rejection, got {other:?}"),
        }

        let mut metadata = Metadata::default();
        metadata.insert(Name::from_str("upgrade_id").expect("key"), Json::new("v1"));
        let tx = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .with_metadata(metadata)
        .sign(keypair.private_key());
        let ok = super::enforce_runtime_upgrade_hook("lane-0", &rules, &tx)
            .expect("runtime upgrade hook should allow");
        assert!(ok, "runtime upgrade hook should be applied");
    }

    #[test]
    fn state_enforces_lane_compliance_engine() {
        let chain: ChainId = "lane-compliance".parse().unwrap();
        let (world, authority, keypair) = world_with_authority("wonderland");
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let policy = LaneCompliancePolicy {
            id: LaneCompliancePolicyId::new(Hash::prehashed([0xAA; 32])),
            version: 1,
            lane_id: TestLaneId::SINGLE,
            dataspace_id: TestDataSpaceId::UNIVERSAL,
            jurisdiction: JurisdictionSet::default(),
            deny: vec![LaneComplianceRule {
                selector: ParticipantSelector {
                    account: Some(authority.clone()),
                    ..ParticipantSelector::default()
                },
                reason_code: Some("denied account".to_string()),
                jurisdiction_override: JurisdictionSet::default(),
            }],
            allow: Vec::new(),
            transfer_limits: Vec::new(),
            audit_controls: AuditControls::default(),
            metadata: Metadata::default(),
        };
        let engine = LaneComplianceEngine::from_policies(vec![policy], false).expect("engine");
        state.install_lane_compliance_engine(Some(Arc::new(engine)));
        assert!(
            state.lane_compliance_engine().is_some(),
            "lane compliance engine should be installed"
        );

        let tx = TransactionBuilder::new(
            chain,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "noop".into())])
        .sign(keypair.private_key());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let stx = block.transaction();
        let assignment = super::LaneAssignment {
            lane_id: TestLaneId::SINGLE,
            dataspace_id: TestDataSpaceId::UNIVERSAL,
            dataspace_catalog: &stx.nexus.dataspace_catalog,
        };

        let err = super::enforce_lane_policies(&tx, &stx, &assignment)
            .expect_err("compliance denial should reject");
        match err {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
                assert!(
                    msg.contains("denied account") || msg.contains("lane compliance"),
                    "expected compliance rejection, got {msg}"
                );
            }
            other => panic!("expected compliance rejection, got {other:?}"),
        }
    }

    #[test]
    fn non_governed_manifest_validators_do_not_gate_state_policy_for_ivm_contract_metadata() {
        use iroha_data_model::transaction::{Executable, executable::IvmBytecode};

        let chain: ChainId = "non-governed-manifest-ivm-contract".parse().unwrap();
        let (world, authority, keypair) = world_with_authority("wonderland");
        let (validator, _) = gen_account_in("wonderland");
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let mut rules = GovernanceRules {
            validators: vec![validator],
            quorum: Some(1),
            ..GovernanceRules::default()
        };
        rules
            .protected_namespaces
            .insert(Name::from_str("is").expect("namespace"));

        let mut statuses = BTreeMap::new();
        statuses.insert(
            TestLaneId::SINGLE,
            LaneManifestStatus {
                lane: TestLaneId::SINGLE,
                alias: "is".to_string(),
                dataspace: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: None,
                manifest_path: Some(PathBuf::from("/tmp/is.manifest.json")),
                governance_rules: Some(rules),
                privacy_commitments: Vec::new(),
            },
        );
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        state.install_lane_manifests(&manifests);

        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            0,
            TestDataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let mut metadata = Metadata::default();
        metadata.insert(
            (*super::CONTRACT_ADDRESS_METADATA_KEY).clone(),
            Json::new(contract_address.to_string()),
        );
        let tx = TransactionBuilder::new(
            chain,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_metadata(metadata)
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(vec![0xCA])))
        .sign(keypair.private_key());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let stx = block.transaction();
        let assignment = single_lane_assignment(&stx.nexus.dataspace_catalog);

        let result = super::enforce_lane_policies(&tx, &stx, &assignment);
        assert!(
            result.is_ok(),
            "non-governed manifest validators must not reject contract metadata: {result:?}"
        );
    }

    #[test]
    fn validate_transaction_without_context_uses_live_autoscale_route() {
        use iroha_data_model::transaction::{Executable, executable::IvmBytecode};

        let chain: ChainId = "tx-live-autoscale-route".parse().unwrap();
        let (world, authority, keypair) = world_with_authority("wonderland");
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        {
            let mut elastic_lane = LaneConfig {
                id: TestLaneId::new(1),
                alias: "elastic-lane-1".to_string(),
                dataspace_id: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                ..LaneConfig::default()
            };
            elastic_lane
                .metadata
                .insert(AUTOSCALE_META_MANAGED.to_string(), "true".to_string());
            elastic_lane
                .metadata
                .insert(AUTOSCALE_META_CREATED_HEIGHT.to_string(), "1".to_string());
            crate::state::attach_synthetic_autoscale_committee_for_test(&mut elastic_lane);

            let mut nexus = state.nexus.write();
            nexus.enabled = true;
            nexus.autoscale.enabled = true;
            nexus.autoscale.min_lanes = nonzero!(1_u32);
            nexus.autoscale.max_lanes = nonzero!(8_u32);
            nexus.lane_catalog =
                LaneCatalog::new(nonzero!(2_u32), vec![LaneConfig::default(), elastic_lane])
                    .expect("autoscale lane catalog");
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        }

        let mut statuses = BTreeMap::new();
        statuses.insert(
            TestLaneId::SINGLE,
            LaneManifestStatus {
                lane: TestLaneId::SINGLE,
                alias: "base-lane".to_string(),
                dataspace: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: Some("base-governance".to_string()),
                manifest_path: None,
                governance_rules: None,
                privacy_commitments: Vec::new(),
            },
        );
        statuses.insert(
            TestLaneId::new(1),
            LaneManifestStatus {
                lane: TestLaneId::new(1),
                alias: "elastic-lane-1".to_string(),
                dataspace: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: None,
                manifest_path: None,
                governance_rules: None,
                privacy_commitments: Vec::new(),
            },
        );
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        state.install_lane_manifests(&manifests);

        let mut selected = None;
        for attempt in 0_u64..256 {
            let mut metadata = Metadata::default();
            metadata.insert(
                Name::from_str("route_attempt").expect("static metadata key"),
                Json::new(attempt),
            );
            let tx = TransactionBuilder::new(
                chain.clone(),
                authority.clone(),
                fee_payment_with_gas_limit(TEST_GAS_LIMIT),
            )
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(
                minimal_ivm_program(1),
            )))
            .sign(keypair.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
            let (catalog_only, live_plan) = {
                let view = state.view();
                let ledger_time_ms = 0;
                let catalog_only = crate::queue::evaluate_policy_with_catalog_and_world_at(
                    &view.nexus.routing_policy,
                    &view.nexus.lane_catalog,
                    &view.nexus.dataspace_catalog,
                    &accepted,
                    view.world(),
                    ledger_time_ms,
                )
                .expect("catalog-only route resolves");
                let live_plan =
                    crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                        &view.nexus,
                        &accepted,
                        view.world(),
                        ledger_time_ms,
                        1,
                    )
                    .expect("live autoscale route resolves");
                (catalog_only, live_plan)
            };
            if catalog_only.lane_id == TestLaneId::SINGLE
                && live_plan.coordinator_route().lane_id == TestLaneId::new(1)
            {
                selected = Some(tx);
                break;
            }
        }
        let tx = selected.expect("fixture should find a tx routed to elastic lane");
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        result.expect("live autoscale-routed transaction should bypass blocked base lane");
    }

    #[test]
    fn validate_transaction_without_context_ignores_autoscale_when_nexus_disabled() {
        use iroha_data_model::transaction::{Executable, executable::IvmBytecode};

        let chain: ChainId = "tx-disabled-nexus-autoscale-route".parse().unwrap();
        let (world, authority, keypair) = world_with_authority("wonderland");
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        {
            let mut elastic_lane = LaneConfig {
                id: TestLaneId::new(1),
                alias: "elastic-lane-1".to_string(),
                dataspace_id: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                ..LaneConfig::default()
            };
            elastic_lane
                .metadata
                .insert(AUTOSCALE_META_MANAGED.to_string(), "true".to_string());
            elastic_lane
                .metadata
                .insert(AUTOSCALE_META_CREATED_HEIGHT.to_string(), "1".to_string());
            crate::state::attach_synthetic_autoscale_committee_for_test(&mut elastic_lane);

            let mut nexus = state.nexus.write();
            nexus.enabled = true;
            nexus.autoscale.enabled = true;
            nexus.autoscale.min_lanes = nonzero!(1_u32);
            nexus.autoscale.max_lanes = nonzero!(8_u32);
            nexus.lane_catalog =
                LaneCatalog::new(nonzero!(2_u32), vec![LaneConfig::default(), elastic_lane])
                    .expect("autoscale lane catalog");
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        }

        let mut statuses = BTreeMap::new();
        statuses.insert(
            TestLaneId::SINGLE,
            LaneManifestStatus {
                lane: TestLaneId::SINGLE,
                alias: "base-lane".to_string(),
                dataspace: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: Some("base-governance".to_string()),
                manifest_path: None,
                governance_rules: None,
                privacy_commitments: Vec::new(),
            },
        );
        statuses.insert(
            TestLaneId::new(1),
            LaneManifestStatus {
                lane: TestLaneId::new(1),
                alias: "elastic-lane-1".to_string(),
                dataspace: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: None,
                manifest_path: None,
                governance_rules: None,
                privacy_commitments: Vec::new(),
            },
        );
        let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        state.install_lane_manifests(&manifests);

        let mut selected = None;
        for attempt in 0_u64..256 {
            let mut metadata = Metadata::default();
            metadata.insert(
                Name::from_str("route_attempt").expect("static metadata key"),
                Json::new(attempt),
            );
            let tx = TransactionBuilder::new(
                chain.clone(),
                authority.clone(),
                fee_payment_with_gas_limit(TEST_GAS_LIMIT),
            )
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(
                minimal_ivm_program(1),
            )))
            .sign(keypair.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
            let (enabled_plan, disabled_plan) = {
                let mut nexus = state.nexus.write();
                nexus.enabled = true;
                drop(nexus);
                let enabled_plan = {
                    let view = state.view();
                    crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                        &view.nexus,
                        &accepted,
                        view.world(),
                        0,
                        1,
                    )
                    .expect("enabled Nexus autoscale route resolves")
                };
                let mut nexus = state.nexus.write();
                nexus.enabled = false;
                drop(nexus);
                let disabled_plan = {
                    let view = state.view();
                    crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                        &view.nexus,
                        &accepted,
                        view.world(),
                        0,
                        1,
                    )
                    .expect("disabled Nexus default route resolves")
                };
                (enabled_plan, disabled_plan)
            };
            if enabled_plan.coordinator_route().lane_id == TestLaneId::new(1)
                && disabled_plan.coordinator_route().lane_id == TestLaneId::SINGLE
            {
                selected = Some(tx);
                break;
            }
        }
        let tx =
            selected.expect("fixture should find a tx that would route to elastic when enabled");
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        state.nexus.write().enabled = false;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result.expect_err("disabled Nexus must keep autoscale traffic on blocked base lane") {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(message)) => {
                assert!(
                    message.contains("governance")
                        || message.contains("base-governance")
                        || message.contains("lane"),
                    "expected blocked base-lane policy rejection, got {message}"
                );
            }
            other => panic!("expected base-lane NotPermitted rejection, got {other:?}"),
        }
    }

    fn lane_execution_input_artifact(
        lane_id: TestLaneId,
        dataspace_id: TestDataSpaceId,
        candidate_indices: Vec<u64>,
        entrypoints: Vec<TransactionEntrypoint>,
        validator: PeerId,
    ) -> crate::kura::LaneBlockExecutionInputArtifact {
        let accepted_transaction_hashes = entrypoints
            .iter()
            .map(|entrypoint| Hash::from(entrypoint.hash()))
            .collect::<Vec<_>>();
        let validator_set = vec![validator];
        let lane_incarnation = Hash::new(b"tx-test-lane-incarnation");
        let subject_hash = SumeragiLanePayloadOwnership::compute_replay_subject_hash(
            lane_id,
            dataspace_id,
            lane_incarnation,
            1,
            0,
            &candidate_indices,
            &accepted_transaction_hashes,
            "tx-test-lane-execution",
        )
        .expect("synthetic lane subject should hash");
        let payload_ownership_hash =
            SumeragiLanePayloadOwnership::compute_replay_payload_ownership_hash(
                lane_id,
                dataspace_id,
                lane_incarnation,
                1,
                0,
                subject_hash,
                &candidate_indices,
                &accepted_transaction_hashes,
                "tx-test-lane-execution",
            )
            .expect("synthetic lane ownership should hash");
        let rbc_instance_hash = SumeragiLanePayloadOwnership::compute_replay_rbc_instance_hash(
            lane_id,
            dataspace_id,
            lane_incarnation,
            1,
            0,
            subject_hash,
            payload_ownership_hash,
        )
        .expect("synthetic lane RBC instance should hash");

        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id,
            dataspace_id,
            lane_incarnation,
            proposal_height: 1,
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_height: 1,
            lane_block_view: 0,
            subject_hash,
            payload_ownership_hash,
            rbc_instance_hash,
            accepted_candidate_indices: candidate_indices.clone(),
            accepted_transaction_hashes: accepted_transaction_hashes.clone(),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: iroha_crypto::HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            validator_count: 1,
            min_quorum: 1,
            qc_mode_tag: "tx-test-lane-execution".to_string(),
            descriptor_hash: Hash::new(b"lane execution descriptor placeholder"),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();

        let ownership = SumeragiLanePayloadOwnership {
            proposal_height: 1,
            proposal_view: 0,
            lane_id,
            dataspace_id,
            lane_incarnation,
            lane_block_height: 1,
            lane_block_view: 0,
            subject_hash,
            qc_mode_tag: "tx-test-lane-execution".to_string(),
            accepted_candidate_indices: candidate_indices.clone(),
            accepted_transaction_hashes: accepted_transaction_hashes.clone(),
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_descriptor_hash: Some(descriptor.descriptor_hash),
            lane_block_descriptor_validator_set: validator_set.clone(),
            lane_block_descriptor_validator_count: 1,
            lane_block_descriptor_min_quorum: 1,
            payload_ownership_hash,
            rbc_instance_hash,
        };
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::new(b"lane execution proposal placeholder"),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        crate::kura::LaneBlockExecutionInputArtifact::new(crate::kura::RecoveredLaneBlockPayload {
            proposal,
            artifact: crate::kura::LaneBlockArtifact::new(
                iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"lane execution proposal block",
                )),
                ownership,
            ),
            autonomous_chain_id_hash: None,
            autonomous_epoch: None,
            autonomous_payload_hash: None,
            entrypoints,
            reservation_keys: Vec::new(),
            routing_plans: Vec::new(),
            native_amx_receipts: Vec::new(),
        })
    }

    fn state_with_guarded_base_and_open_elastic_lane(chain: &ChainId, world: World) -> State {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let elastic_lane_id = TestLaneId::new(1);
        let mut elastic_lane = LaneConfig {
            id: elastic_lane_id,
            alias: "elastic-lane-1".to_string(),
            dataspace_id: TestDataSpaceId::UNIVERSAL,
            visibility: LaneVisibility::Public,
            ..LaneConfig::default()
        };
        elastic_lane
            .metadata
            .insert(AUTOSCALE_META_MANAGED.to_string(), "true".to_string());
        elastic_lane
            .metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.to_string(), "1".to_string());
        crate::state::attach_synthetic_autoscale_committee_for_test(&mut elastic_lane);

        {
            let mut nexus = state.nexus.write();
            nexus.enabled = true;
            nexus.autoscale.enabled = true;
            nexus.autoscale.min_lanes = nonzero!(1_u32);
            nexus.autoscale.max_lanes = nonzero!(8_u32);
            nexus.lane_catalog =
                LaneCatalog::new(nonzero!(2_u32), vec![LaneConfig::default(), elastic_lane])
                    .expect("autoscale lane catalog");
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        }

        let mut statuses = BTreeMap::new();
        statuses.insert(
            TestLaneId::SINGLE,
            LaneManifestStatus {
                lane: TestLaneId::SINGLE,
                alias: "base-lane".to_string(),
                dataspace: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: Some("base-governance".to_string()),
                manifest_path: None,
                governance_rules: None,
                privacy_commitments: Vec::new(),
            },
        );
        statuses.insert(
            elastic_lane_id,
            LaneManifestStatus {
                lane: elastic_lane_id,
                alias: "elastic-lane-1".to_string(),
                dataspace: TestDataSpaceId::UNIVERSAL,
                visibility: LaneVisibility::Public,
                storage: LaneStorageProfile::FullReplica,
                governance: None,
                manifest_path: None,
                governance_rules: None,
                privacy_commitments: Vec::new(),
            },
        );
        state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(statuses)));
        state
    }

    #[test]
    fn lane_block_execution_input_uses_descriptor_indices_and_routing_context() {
        use iroha_data_model::transaction::{
            Executable, TransactionBuilder, executable::IvmBytecode,
        };

        let chain: ChainId = "lane-execution-input-route".parse().unwrap();
        let (world, authority, keypair) = world_with_authority("wonderland");
        let validator = PeerId {
            public_key: keypair.public_key().clone(),
        };
        let state = state_with_guarded_base_and_open_elastic_lane(&chain, world);
        let entrypoints = (0_u64..256)
            .filter_map(|attempt| {
                let mut metadata = Metadata::default();
                metadata.insert(
                    Name::from_str("lane_execution_attempt").expect("static metadata key"),
                    Json::new(attempt),
                );
                let tx = TransactionBuilder::new(
                    chain.clone(),
                    authority.clone(),
                    fee_payment_with_gas_limit(TEST_GAS_LIMIT),
                )
                .with_metadata(metadata)
                .with_executable(Executable::Ivm(IvmBytecode::from_compiled(
                    minimal_ivm_program(1),
                )))
                .sign(keypair.private_key());
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
                let plan = {
                    let view = state.view();
                    evaluate_policy_plan_with_nexus_and_world_at_block_height(
                        &view.nexus,
                        &accepted,
                        view.world(),
                        0,
                        1,
                    )
                    .expect("elastic route resolves")
                };
                (plan.coordinator_route().lane_id == TestLaneId::new(1))
                    .then_some(TransactionEntrypoint::External(tx))
            })
            .take(2)
            .collect::<Vec<_>>();
        assert_eq!(
            entrypoints.len(),
            2,
            "fixture should find two transactions routed to the elastic lane"
        );
        let expected_hashes = entrypoints
            .iter()
            .map(TransactionEntrypoint::hash)
            .collect::<Vec<_>>();
        let artifact = lane_execution_input_artifact(
            TestLaneId::new(1),
            TestDataSpaceId::UNIVERSAL,
            vec![2, 0],
            entrypoints,
            validator,
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let results = block
            .validate_lane_block_execution_input_with_routing_context(&artifact, &mut ivm_cache)
            .expect("valid lane execution input should execute");

        assert_eq!(results.len(), 2);
        assert_eq!(
            results
                .iter()
                .map(|(index, _, _)| *index)
                .collect::<Vec<_>>(),
            vec![2, 0]
        );
        assert_eq!(
            results.iter().map(|(_, hash, _)| *hash).collect::<Vec<_>>(),
            expected_hashes
        );
        for (_, _, result) in results {
            result.expect("descriptor-routed lane transaction should pass");
        }
    }

    #[test]
    fn lane_block_execution_input_rejects_forged_hashes_before_state_execution() {
        use iroha_data_model::transaction::{
            Executable, TransactionBuilder, executable::IvmBytecode,
        };

        let chain: ChainId = "lane-execution-input-forged".parse().unwrap();
        let (world, authority, keypair) = world_with_authority("wonderland");
        let validator = PeerId {
            public_key: keypair.public_key().clone(),
        };
        let state = state_with_guarded_base_and_open_elastic_lane(&chain, world);
        let tx = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(
            minimal_ivm_program(1),
        )))
        .sign(keypair.private_key());
        let mut artifact = lane_execution_input_artifact(
            TestLaneId::new(1),
            TestDataSpaceId::UNIVERSAL,
            vec![0],
            vec![TransactionEntrypoint::External(tx)],
            validator,
        );
        artifact.entrypoint_hashes[0] = Hash::new(b"forged lane execution entrypoint hash");

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let err = block
            .validate_lane_block_execution_input_with_routing_context(&artifact, &mut ivm_cache)
            .expect_err("forged execution input hashes must be rejected");

        assert_eq!(
            err,
            "execution input entrypoint hashes do not match proposal descriptor"
        );
    }

    #[test]
    fn lane_block_execution_input_rejects_duplicate_signed_entrypoints_before_state_execution() {
        use iroha_data_model::transaction::{
            Executable, TransactionBuilder, executable::IvmBytecode,
        };

        let chain: ChainId = "lane-execution-input-duplicate".parse().unwrap();
        let (world, authority, keypair) = world_with_authority("wonderland");
        let validator = PeerId {
            public_key: keypair.public_key().clone(),
        };
        let state = state_with_guarded_base_and_open_elastic_lane(&chain, world);
        let tx = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            fee_payment_with_gas_limit(TEST_GAS_LIMIT),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(
            minimal_ivm_program(1),
        )))
        .sign(keypair.private_key());
        let artifact = lane_execution_input_artifact(
            TestLaneId::new(1),
            TestDataSpaceId::UNIVERSAL,
            vec![0, 1],
            vec![
                TransactionEntrypoint::External(tx.clone()),
                TransactionEntrypoint::External(tx),
            ],
            validator,
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let err = block
            .validate_lane_block_execution_input_with_routing_context(&artifact, &mut ivm_cache)
            .expect_err("duplicate lane execution entrypoints must be rejected");

        assert_eq!(err, "execution input contains duplicate entrypoints");
    }

    #[test]
    fn lane_block_execution_input_preserves_full_width_entrypoint_indices() {
        use iroha_data_model::transaction::{
            Executable, TransactionBuilder, executable::IvmBytecode,
        };

        let chain: ChainId = "lane-execution-input-full-width-index".parse().unwrap();
        let (world, authority, keypair) = world_with_authority("wonderland");
        let validator = PeerId {
            public_key: keypair.public_key().clone(),
        };
        let state = state_with_guarded_base_and_open_elastic_lane(&chain, world);
        let tx = (0_u64..256)
            .find_map(|attempt| {
                let mut metadata = Metadata::default();
                metadata.insert(
                    Name::from_str("lane_execution_attempt").expect("static metadata key"),
                    Json::new(attempt),
                );
                let tx = TransactionBuilder::new(
                    chain.clone(),
                    authority.clone(),
                    fee_payment_with_gas_limit(TEST_GAS_LIMIT),
                )
                .with_metadata(metadata)
                .with_executable(Executable::Ivm(IvmBytecode::from_compiled(
                    minimal_ivm_program(1),
                )))
                .sign(keypair.private_key());
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
                let plan = {
                    let view = state.view();
                    evaluate_policy_plan_with_nexus_and_world_at_block_height(
                        &view.nexus,
                        &accepted,
                        view.world(),
                        0,
                        1,
                    )
                    .expect("elastic route resolves")
                };
                (plan.coordinator_route().lane_id == TestLaneId::new(1)).then_some(tx)
            })
            .expect("fixture should find a transaction routed to the elastic lane");
        let artifact = lane_execution_input_artifact(
            TestLaneId::new(1),
            TestDataSpaceId::UNIVERSAL,
            vec![u64::MAX],
            vec![TransactionEntrypoint::External(tx)],
            validator,
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let results = block
            .validate_lane_block_execution_input_with_routing_context(&artifact, &mut ivm_cache)
            .expect("full-width entrypoint index should be preserved");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, u64::MAX);
        results[0]
            .2
            .clone()
            .expect("descriptor-routed lane transaction should pass");
    }

    #[test]
    fn install_lane_manifests_updates_privacy_registry() {
        let chain: ChainId = "lane-privacy-registry".parse().unwrap();
        let world = World::default();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain);

        let commitment = LanePrivacyCommitment::merkle(
            LaneCommitmentId::new(9),
            MerkleCommitment::from_root_bytes([0x11; 32], 8),
        );
        let status = LaneManifestStatus {
            lane: TestLaneId::SINGLE,
            alias: "private".to_string(),
            dataspace: TestDataSpaceId::UNIVERSAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::CommitmentOnly,
            governance: None,
            manifest_path: Some(PathBuf::from("/tmp/privacy.json")),
            governance_rules: None,
            privacy_commitments: vec![commitment],
        };
        let mut statuses = BTreeMap::new();
        statuses.insert(TestLaneId::SINGLE, status);
        let registry = Arc::new(LaneManifestRegistry::from_statuses(statuses));
        state.install_lane_manifests(&registry);

        let snapshot = state.lane_privacy_registry.read().clone();
        assert!(!snapshot.is_empty(), "privacy registry should not be empty");
        assert!(
            snapshot.lane(TestLaneId::SINGLE).is_some(),
            "privacy registry should contain lane entry"
        );
    }

    /// Lightweight end-to-end harness for exercising transaction, trigger, and block flow in tests.
    pub struct Sandbox {
        /// In-memory state under test.
        pub state: State,
        /// Buffered transactions that will be packed into the next block.
        pub transactions: Vec<SignedTransaction>,
    }

    /// Handle returned by [`Sandbox::block`] for asserting and committing a prepared block.
    pub struct SandboxBlock<'state> {
        /// View into the mutable world state for this block.
        pub state: StateBlock<'state>,
        /// The signed block prepared from queued transactions.
        pub block: Option<SignedBlock>,
    }

    /// Short names of pre-created test accounts used by the sandbox.
    pub const ACCOUNTS_STR: [&str; 5] = ["alice", "bob", "carol", "dave", "eve"];
    /// Initial balances for the sandbox asset, keyed by account short name.
    pub static INIT_BALANCE: LazyLock<AccountBalance> =
        LazyLock::new(|| ACCOUNTS_STR.into_iter().zip([60, 10, 10, 10, 10]).collect());
    /// Default maximum smart contract execution depth used by the sandbox.
    pub const INIT_EXECUTION_DEPTH: u8 = u8::MAX;

    /// Mapping from account short name to its numeric asset balance.
    pub type AccountBalance = std::collections::BTreeMap<&'static str, u64>;
    /// Mapping from account short name to its credentials (ID and key).
    pub type AccountMap = std::collections::BTreeMap<&'static str, Credential>;
    /// Mapping from account identifier to its short alias.
    pub type AccountAliasMap = std::collections::BTreeMap<AccountId, &'static str>;

    /// Domain used for all sandbox entities.
    pub const DOMAIN_STR: &str = "wonderland";
    /// Asset definition name used by the sandbox.
    pub const ASSET_STR: &str = "rose";
    /// Pre-parsed domain identifier for the sandbox domain.
    pub static DOMAIN: LazyLock<DomainId> =
        LazyLock::new(|| DomainId::try_new(DOMAIN_STR, "universal").unwrap());
    /// Pre-parsed asset definition identifier for the sandbox asset.
    pub static ASSET: LazyLock<AssetDefinitionId> = LazyLock::new(|| {
        AssetDefinitionId::new(
            DOMAIN.clone(),
            ASSET_STR.parse().expect("sandbox asset name is valid"),
        )
    });
    static FIFO_SCHEDULER_LOCK: LazyLock<std::sync::Mutex<()>> =
        LazyLock::new(|| std::sync::Mutex::new(()));
    const SANDBOX_ACCOUNT_KEYS: [(&str, &str, &str); 5] = [
        (
            "alice",
            "ed0120FF365BDAA7CB998DBE6505CA8911C8C85C9ADBBF3A9CD4DF4FFAE5A09723590F",
            "5309423ca944339f556bdbaa648e6f962ca680c7a07ca4bfdaeb92c2e84c0631",
        ),
        (
            "bob",
            "ed01200B25F5244DCFA449F1A00758C1652F3BD912FE5ADF3244B084D84BC11548C640",
            "79a36c14bca68bd098e2968d03fa0ec1dc44f863aeb2cf775892352696f27943",
        ),
        (
            "carol",
            "ed0120D3E0032F42620A333DC05AF7B72C5D5613286505AD6590356046FA23C3231EDD",
            "64942ccc247311f9265ef5144962da8a462e788174d7a3d586fbd31633a6d7ef",
        ),
        (
            "dave",
            "ed01206AA7B199B45261F2A9C71B7644F9346EF5B1A8DCD59F90B0C6B954DD5DF320DC",
            "608673cd53310dbec45a8cca4716968712d1b4986b4dc7294d75028bcf7ac34e",
        ),
        (
            "eve",
            "ed012040C2A4B02CCAD1EFEBDF9BDB77AACCECC8A7BDEA2C6E543719FDD3B6DD21DA74",
            "bc4ff9e3d5cc415426f864c513f974ccd5ab2f86cda19fc20c4e1fec86585fa1",
        ),
    ];

    /// Pre-derived credentials for sandbox accounts (IDs and private keys).
    pub static ACCOUNT: LazyLock<AccountMap> = LazyLock::new(|| {
        SANDBOX_ACCOUNT_KEYS
            .iter()
            .map(|(name, public, private_hex)| {
                let signatory: iroha_crypto::PublicKey = public.parse().unwrap();
                let id = AccountId::new(signatory);
                let key = iroha_crypto::PrivateKey::from_hex(
                    iroha_crypto::Algorithm::Ed25519,
                    private_hex,
                )
                .unwrap();
                (*name, Credential { id, key })
            })
            .collect()
    });
    /// Reverse lookup from account identifier to its sandbox alias.
    pub static ACCOUNT_ALIAS_BY_ID: LazyLock<AccountAliasMap> = LazyLock::new(|| {
        ACCOUNT
            .iter()
            .map(|(alias, cred)| (cred.id.clone(), *alias))
            .collect()
    });

    #[test]
    fn sandbox_accounts_are_deterministic() {
        for (name, public, _) in SANDBOX_ACCOUNT_KEYS {
            assert_eq!(
                ACCOUNT[name].id.expect_single_signatory().to_string(),
                *public
            );
        }
    }

    /// Account credentials used by the sandbox (ID and signing key).
    #[derive(Debug, Clone)]
    pub struct Credential {
        /// Fully-qualified account identifier.
        pub id: AccountId,
        /// Private key used to sign transactions for the account.
        pub key: iroha_crypto::PrivateKey,
    }

    /// Credentials of the special genesis account used to bootstrap state.
    pub static GENESIS_ACCOUNT: LazyLock<Credential> = LazyLock::new(|| {
        let (id, key_pair) = gen_account_in(GENESIS_DOMAIN_ID.clone());
        Credential {
            id,
            key: key_pair.into_parts().1,
        }
    });
    /// Chain identifier used in sandbox transactions.
    pub static CHAIN_ID: LazyLock<ChainId> =
        LazyLock::new(|| ChainId::from("00000000-0000-0000-0000-000000000000"));

    /// Build the [`AssetId`] for the sandbox test asset owned by a named account.
    pub fn asset(account_name: &str) -> AssetId {
        AssetId::new(ASSET.clone(), ACCOUNT[account_name].id.clone())
    }

    /// Convenience builder that yields a single transfer instruction iterator.
    ///
    /// Transfers `quantity` units of the sandbox asset from `src` to `dest`.
    pub fn transfer<'a>(
        src: &'a str,
        quantity: u32,
        dest: &'a str,
    ) -> impl IntoIterator<Item = InstructionBox> + 'a {
        transfers_batched::<1>(src, quantity, dest)
    }

    /// Produce an iterator over `N_INSTRUCTIONS` transfer instructions.
    ///
    /// Each instruction transfers `quantity_per_instruction` units of the sandbox
    /// asset from `src` to `dest`.
    pub fn transfers_batched<'a, const N_INSTRUCTIONS: usize>(
        src: &'a str,
        quantity_per_instruction: u32,
        dest: &'a str,
    ) -> impl IntoIterator<Item = InstructionBox> + 'a {
        (0..N_INSTRUCTIONS).map(move |_| {
            Transfer::asset_quantity(
                asset(src),
                quantity_per_instruction,
                ACCOUNT[dest].id.clone(),
            )
            .into()
        })
    }

    /// Assert that the emitted events match a stored JSON snapshot.
    pub fn assert_events(actual: &[EventBox], snapshot_path: impl AsRef<std::path::Path>) {
        let snapshot_path_buf = {
            let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(snapshot_path.as_ref());
            path.set_extension("json");
            path
        };
        let (snapshot_text, line_endings) = load_snapshot(&snapshot_path_buf);
        let expected = expect_test::expect_file![snapshot_path_buf.clone()];
        let actual = actual
            .iter()
            .filter(|e| {
                !matches!(
                    e,
                    EventBox::Time(_) | EventBox::Pipeline(_) | EventBox::PipelineBatch(_)
                )
            })
            .map(EventSnapshot::from_event)
            .collect::<Vec<_>>();
        let rendered = if actual.is_empty() {
            "[]".to_owned()
        } else {
            norito::json::to_json_pretty(&actual).unwrap()
        };
        if let Some(text) = snapshot_text.as_deref() {
            let collapsed = collapse_to_unix_line_endings(text);
            let collapsed = collapsed.strip_suffix('\n').unwrap_or(collapsed.as_ref());
            if collapsed == rendered {
                return;
            }
        }
        let normalised = normalise_line_endings(&rendered, line_endings);
        expected.assert_eq(normalised.as_ref());
    }

    enum EventSnapshot<'a> {
        Asset(AssetEventSnapshot<'a>),
        TriggerCompleted(TriggerCompletedSnapshot<'a>),
        Raw(String),
    }

    impl<'a> EventSnapshot<'a> {
        fn from_event(event: &'a EventBox) -> Self {
            match event {
                EventBox::Data(data) => AssetEventSnapshot::from_data_event(data.as_ref())
                    .map_or_else(|| Self::Raw(format!("{event:?}")), Self::Asset),
                EventBox::TriggerCompleted(event) => {
                    Self::TriggerCompleted(TriggerCompletedSnapshot(event))
                }
                other => Self::Raw(format!("{other:?}")),
            }
        }
    }

    impl norito::json::JsonSerialize for EventSnapshot<'_> {
        fn json_serialize(&self, out: &mut String) {
            match self {
                Self::Asset(asset) => asset.json_serialize(out),
                Self::TriggerCompleted(event) => event.json_serialize(out),
                Self::Raw(raw) => norito::json::write_json_string(raw, out),
            }
        }
    }

    enum AssetEventSnapshot<'a> {
        Added(&'a AssetChanged),
        Removed(&'a AssetChanged),
    }

    impl<'a> AssetEventSnapshot<'a> {
        fn from_data_event(event: &'a data::DataEvent) -> Option<Self> {
            match event {
                data::DataEvent::Domain(domain_event) => Self::from_domain_event(domain_event),
                _ => None,
            }
        }

        fn from_domain_event(event: &'a DomainEvent) -> Option<Self> {
            match event {
                DomainEvent::Account(account_event) => Self::from_account_event(account_event),
                _ => None,
            }
        }

        fn from_account_event(event: &'a AccountEvent) -> Option<Self> {
            match event {
                AccountEvent::Asset(asset_event) => Self::from_asset_event(asset_event),
                _ => None,
            }
        }

        fn from_asset_event(event: &'a AssetEvent) -> Option<Self> {
            match event {
                AssetEvent::Added(change) => Some(Self::Added(change)),
                AssetEvent::Removed(change) => Some(Self::Removed(change)),
                _ => None,
            }
        }

        fn variant_label(&self) -> &'static str {
            match self {
                Self::Added(_) => "Added",
                Self::Removed(_) => "Removed",
            }
        }

        fn change(&self) -> &'a AssetChanged {
            match self {
                Self::Added(change) | Self::Removed(change) => change,
            }
        }
    }

    fn format_asset_id_for_snapshot(asset_id: &AssetId) -> String {
        let account = asset_id.account();
        let account_str = ACCOUNT_ALIAS_BY_ID.get(account).map_or_else(
            || format!("{}@{}", account.expect_single_signatory(), DOMAIN_STR),
            |alias| format!("{alias}@{DOMAIN_STR}"),
        );
        if asset_id.definition().try_domain() == Some(&*DOMAIN) {
            let name = asset_id
                .definition()
                .try_name()
                .expect("matching domain projection must include a name");
            format!("{name}##{account_str}")
        } else {
            format!("{}#{}", asset_id.definition(), account_str)
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum SnapshotLineEndings {
        Lf,
        Crlf,
    }

    fn load_snapshot(path: &std::path::Path) -> (Option<String>, SnapshotLineEndings) {
        std::fs::read_to_string(path).map_or((None, SnapshotLineEndings::Lf), |text| {
            let endings = detect_line_endings_from_text(&text);
            (Some(text), endings)
        })
    }

    fn normalise_line_endings(
        input: &str,
        endings: SnapshotLineEndings,
    ) -> std::borrow::Cow<'_, str> {
        match endings {
            SnapshotLineEndings::Lf => std::borrow::Cow::Borrowed(input),
            SnapshotLineEndings::Crlf => {
                if input.contains('\r') {
                    std::borrow::Cow::Borrowed(input)
                } else {
                    std::borrow::Cow::Owned(input.replace('\n', "\r\n"))
                }
            }
        }
    }

    fn detect_line_endings_from_text(text: &str) -> SnapshotLineEndings {
        if text.contains('\r') {
            SnapshotLineEndings::Crlf
        } else {
            SnapshotLineEndings::Lf
        }
    }

    fn collapse_to_unix_line_endings(text: &str) -> std::borrow::Cow<'_, str> {
        if text.contains('\r') {
            let collapsed = text.replace("\r\n", "\n").replace('\r', "\n");
            std::borrow::Cow::Owned(collapsed)
        } else {
            std::borrow::Cow::Borrowed(text)
        }
    }

    impl norito::json::JsonSerialize for AssetEventSnapshot<'_> {
        fn json_serialize(&self, out: &mut String) {
            out.push('{');
            norito::json::write_json_string("Data", out);
            out.push(':');
            out.push('{');
            norito::json::write_json_string("Domain", out);
            out.push(':');
            out.push('{');
            norito::json::write_json_string("Account", out);
            out.push(':');
            out.push('{');
            norito::json::write_json_string("Asset", out);
            out.push(':');
            out.push('{');
            norito::json::write_json_string(self.variant_label(), out);
            out.push(':');
            out.push('{');
            norito::json::write_json_string("asset", out);
            out.push(':');
            let asset_id = format_asset_id_for_snapshot(self.change().asset());
            norito::json::write_json_string(&asset_id, out);
            out.push(',');
            norito::json::write_json_string("amount", out);
            out.push(':');
            let amount = self.change().amount().to_string();
            norito::json::write_json_string(&amount, out);
            out.push('}');
            out.push('}');
            out.push('}');
            out.push('}');
            out.push('}');
            out.push('}');
        }
    }

    struct TriggerCompletedSnapshot<'a>(&'a TriggerCompletedEvent);

    impl norito::json::JsonSerialize for TriggerCompletedSnapshot<'_> {
        fn json_serialize(&self, out: &mut String) {
            out.push('{');
            norito::json::write_json_string("TriggerCompleted", out);
            out.push(':');
            out.push('{');
            norito::json::write_json_string("trigger_id", out);
            out.push(':');
            let trigger_id = self.0.trigger_id().to_string();
            norito::json::write_json_string(&trigger_id, out);
            out.push(',');
            norito::json::write_json_string("outcome", out);
            out.push(':');
            match self.0.outcome() {
                TriggerCompletedOutcome::Success => {
                    norito::json::write_json_string("Success", out);
                }
                TriggerCompletedOutcome::Failure(message) => {
                    out.push('{');
                    norito::json::write_json_string("Failure", out);
                    out.push(':');
                    norito::json::write_json_string(message, out);
                    out.push('}');
                }
            }
            out.push('}');
            out.push('}');
        }
    }

    impl Default for Sandbox {
        fn default() -> Self {
            let world = {
                let domain = Domain::new(DOMAIN.clone()).build(&GENESIS_ACCOUNT.id);
                let asset_def = {
                    let __asset_definition_id = ASSET.clone();
                    AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::default())
                        .with_name(__asset_definition_id.name().to_string())
                }
                .build(&GENESIS_ACCOUNT.id);
                let accounts = ACCOUNT
                    .clone()
                    .into_iter()
                    .chain([("genesis", GENESIS_ACCOUNT.clone())])
                    .map(|(_name, cred)| Account::new(cred.id.clone()).build(&GENESIS_ACCOUNT.id));
                let assets = INIT_BALANCE
                    .iter()
                    .map(|(name, num)| Asset::new(asset(name), *num));

                World::with_assets([domain], accounts, [asset_def], assets, [])
            };
            let kura = crate::kura::Kura::blank_kura_for_testing();
            let query_handle = crate::query::store::LiveQueryStore::start_test();
            let state =
                State::new_with_chain_for_testing(world, kura, query_handle, CHAIN_ID.clone());
            let mut sandbox = Self {
                state,
                transactions: vec![],
            };
            // Force deterministic single-threaded pipeline evaluation in tests to avoid
            // parallel scheduling reordering transactions that rely on chained data triggers.
            sandbox.state.pipeline.dynamic_prepass = false;
            sandbox.state.pipeline.parallel_overlay = false;
            sandbox.state.pipeline.parallel_apply = false;
            sandbox.state.pipeline.workers = 1;

            sandbox.with_max_execution_depth(INIT_EXECUTION_DEPTH)
        }
    }

    impl Sandbox {
        fn trigger_registration_metadata(&self) -> Metadata {
            let height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
            let registered_ms = self
                .state
                .view()
                .latest_block()
                .map(|block| block.header().creation_time().as_millis())
                .and_then(|ms| u64::try_from(ms).ok())
                .unwrap_or(0);
            let mut metadata = Metadata::default();
            let key_height = "__registered_block_height"
                .parse::<Name>()
                .expect("registered block height metadata key");
            let key_time = "__registered_at_ms"
                .parse::<Name>()
                .expect("registered timestamp metadata key");
            metadata.insert(key_height, Json::new(height));
            metadata.insert(key_time, Json::new(registered_ms));
            metadata
        }

        /// Add a time trigger that transfers the test asset after a timer fires.
        ///
        /// Enqueues a time-based trigger which moves `quantity` units from `src`
        /// to `dest` on each firing. The trigger is configured for infinite repeats
        /// in the sandbox unless otherwise specified by a labeled variant.
        #[must_use]
        pub fn with_time_trigger_transfer(self, src: &str, quantity: u32, dest: &str) -> Self {
            self.with_time_trigger_transfer_internal(src, quantity, dest, Repeats::Indefinitely, 0)
        }

        /// Add a labeled time trigger variant for test disambiguation.
        #[must_use]
        pub fn with_time_trigger_transfer_labeled(
            self,
            src: &str,
            quantity: u32,
            dest: &str,
            label: u32,
        ) -> Self {
            self.with_time_trigger_transfer_internal(
                src,
                quantity,
                dest,
                Repeats::Indefinitely,
                label,
            )
        }

        fn with_time_trigger_transfer_internal(
            self,
            src: &str,
            quantity: u32,
            dest: &str,
            repeats: Repeats,
            label: u32,
        ) -> Self {
            let mut block = self.state.world.triggers.block();
            let mut transaction = block.transaction();
            let trigger = Trigger::new(
                format!("time-{src}-{dest}-{label}").parse().unwrap(),
                Action::new(
                    transfer(src, quantity, dest),
                    repeats,
                    GENESIS_ACCOUNT.id.clone(),
                    TimeEventFilter::new(ExecutionTime::PreCommit),
                )
                .with_metadata(self.trigger_registration_metadata()),
            )
            .try_into()
            .unwrap();

            transaction.add_time_trigger(trigger).unwrap();
            transaction.apply();
            block.commit();
            self
        }

        /// Add a data trigger that reacts to asset-added events and forwards funds.
        #[must_use]
        pub fn with_data_trigger_transfer(self, src: &str, quantity: u32, dest: &str) -> Self {
            self.with_data_trigger_transfer_quantity_internal(
                src,
                Quantity::from(quantity),
                dest,
                Repeats::Indefinitely,
                0,
            )
        }

        /// Add a single-use data trigger that fires at most once.
        #[must_use]
        pub fn with_data_trigger_transfer_once(self, src: &str, quantity: u32, dest: &str) -> Self {
            self.with_data_trigger_transfer_quantity_internal(
                src,
                Quantity::from(quantity),
                dest,
                Repeats::Exactly(1),
                0,
            )
        }

        /// Add a labeled data trigger for disambiguation between similar triggers in tests.
        #[must_use]
        pub fn with_data_trigger_transfer_labeled(
            self,
            src: &str,
            quantity: u32,
            dest: &str,
            label: u32,
        ) -> Self {
            self.with_data_trigger_transfer_quantity_internal(
                src,
                Quantity::from(quantity),
                dest,
                Repeats::Indefinitely,
                label,
            )
        }

        /// Add a data trigger with an explicit [`Quantity`] amount.
        #[must_use]
        pub fn with_data_trigger_transfer_quantity(
            self,
            src: &str,
            amount: Quantity,
            dest: &str,
        ) -> Self {
            self.with_data_trigger_transfer_quantity_internal(
                src,
                amount,
                dest,
                Repeats::Indefinitely,
                0,
            )
        }

        fn with_data_trigger_transfer_quantity_internal(
            self,
            src: &str,
            amount: Quantity,
            dest: &str,
            repeats: Repeats,
            label: u32,
        ) -> Self {
            let mut block = self.state.world.triggers.block();
            let mut transaction = block.transaction();
            let trigger = Trigger::new(
                format!("data-{src}-{dest}-{label}").parse().unwrap(),
                Action::new(
                    [InstructionBox::from(Transfer::asset_quantity(
                        asset(src),
                        amount,
                        ACCOUNT[dest].id.clone(),
                    ))],
                    repeats,
                    GENESIS_ACCOUNT.id.clone(),
                    AssetEventFilter::new()
                        .for_events(AssetEventSet::Added)
                        .for_asset(asset(src)),
                )
                .with_metadata(self.trigger_registration_metadata()),
            )
            .try_into()
            .unwrap();

            transaction.add_data_trigger(trigger).unwrap();
            transaction.apply();
            block.commit();
            self
        }

        /// Limit the maximum smart contract execution depth in the sandbox state.
        #[must_use]
        pub fn with_max_execution_depth(self, depth: u8) -> Self {
            let mut world = self.state.world.block();
            world.parameters.set_parameter(Parameter::SmartContract(
                iroha_data_model::parameter::SmartContractParameter::ExecutionDepth(depth),
            ));
            world.commit();
            self
        }

        /// Queue a single transfer transaction from `src` to `dest`.
        ///
        /// This is a convenience wrapper over [`Self::request_transfers_batched`] with
        /// `N_INSTRUCTIONS = 1`.
        pub fn request_transfer(&mut self, src: &str, quantity: u32, dest: &str) {
            self.request_transfers_batched::<1>(src, quantity, dest);
        }

        /// Queue a transaction consisting of repeated Transfer instructions.
        ///
        /// Builds and buffers a signed transaction that contains `N_INSTRUCTIONS`
        /// transfer instructions, each moving `quantity_per_instruction` units of
        /// the test asset from `src` to `dest`. The buffered transaction is
        /// included the next time a sandbox block is constructed via [`Self::block`].
        ///
        /// - `N_INSTRUCTIONS`: number of identical transfer instructions to include
        /// - `src`: source account name (e.g., "alice")
        /// - `quantity_per_instruction`: amount transferred by each instruction
        /// - `dest`: destination account name (e.g., "bob")
        pub fn request_transfers_batched<const N_INSTRUCTIONS: usize>(
            &mut self,
            src: &str,
            quantity_per_instruction: u32,
            dest: &str,
        ) {
            let transaction = {
                let instructions =
                    transfers_batched::<N_INSTRUCTIONS>(src, quantity_per_instruction, dest);
                TransactionBuilder::new(
                    CHAIN_ID.clone(),
                    GENESIS_ACCOUNT.id.clone(),
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions(instructions)
                .sign(&GENESIS_ACCOUNT.key)
            };
            self.transactions.push(transaction);
        }

        /// Build a signed block from all queued transactions and open it for assertions.
        ///
        /// Consumes the currently queued transactions, packs them into a signed
        /// block and returns a [`SandboxBlock`] handle which allows asserting
        /// balances and applying the block to the in-memory test state.
        pub fn block(&mut self) -> SandboxBlock<'_> {
            let block: SignedBlock = {
                let transactions = {
                    let signed = core::mem::take(&mut self.transactions);
                    // Skip static analysis (AcceptedTransaction::accept)
                    signed
                        .into_iter()
                        .map(|tx| AcceptedTransaction::new_unchecked(Cow::Owned(tx)))
                        .collect::<Vec<_>>()
                };
                BlockBuilder::new_preserve_order(transactions)
                    .chain(0, self.state.view().latest_block().as_deref())
                    .sign(&GENESIS_ACCOUNT.key)
                    .unpack(|_| {})
                    .into()
            };

            SandboxBlock {
                state: self.state.block(block.header()),
                block: Some(block),
            }
        }
    }

    impl SandboxBlock<'_> {
        /// Validate and commit the prepared block to the sandbox state.
        ///
        /// Returns the list of emitted events together with the committed
        /// block for further inspection in tests.
        pub fn apply(&mut self) -> (Vec<EventBox>, CommittedBlock) {
            let _fifo_lock = FIFO_SCHEDULER_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            struct RestoreFifoScheduler(bool);
            impl Drop for RestoreFifoScheduler {
                fn drop(&mut self) {
                    crate::pipeline::set_force_fifo_scheduler(self.0);
                }
            }
            let _restore_fifo =
                RestoreFifoScheduler(crate::pipeline::set_force_fifo_scheduler(true));
            let valid = ValidBlock::validate_unchecked(
                core::mem::take(&mut self.block).unwrap(),
                &mut self.state,
            )
            .unpack(|_| {});
            let committed = valid.commit_unchecked().unpack(|_| {});
            let events = self.state.apply_without_execution(
                &committed,
                // topology in state is only used by sumeragi
                vec![],
            );

            (events, committed)
        }

        /// Assert that selected accounts have the expected balances.
        ///
        /// The `expected` map specifies accounts (by short name like "alice")
        /// and their expected balances of the sandbox test asset. Only the
        /// accounts present in `expected` are checked.
        pub fn assert_balances(&self, expected: impl Into<AccountBalance>) {
            let expected = expected.into();
            let actual: AccountBalance = ACCOUNTS_STR
                .iter()
                .filter(|name| expected.contains_key(*name))
                .map(|name| {
                    let balance_num = self.state.world.assets.get(&asset(name)).map_or_else(
                        || panic!("{name}'s asset not found"),
                        |asset| asset.0.clone(),
                    );
                    let balance =
                        numeric_to_u64(balance_num.as_numeric()).unwrap_or_else(|error| {
                            panic!(
                                "account {name} has non-integer balance {balance_num}: {error:?}"
                            );
                        });
                    (*name, balance)
                })
                .collect();

            assert_eq!(actual, expected);
        }
    }
}

#[cfg(test)]
include!("tx/numeric_to_u64_tests.rs");
