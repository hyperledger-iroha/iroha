//! `Transaction`-related functionality of Iroha.
//!
//! Admission derives the Nexus lane/dataspace assignment for every transaction
//! using the configured routing policy (see `docs/source/nexus_transition_notes.md`)
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
    sync::LazyLock,
    time::{Duration, SystemTime, SystemTimeError},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use eyre::Result;
use hex;
pub use iroha_data_model::prelude::*;
use iroha_data_model::{
    account::address::AccountAddress as IrohaAccountAddress,
    fraud::types::FraudAssessment,
    isi::error::Mismatch,
    isi::{
        runtime_upgrade::{ActivateRuntimeUpgrade, CancelRuntimeUpgrade, ProposeRuntimeUpgrade},
        smart_contract_code::{
            ActivateContractInstance, DeactivateContractInstance, RegisterSmartContractBytes,
            RegisterSmartContractCode, RemoveSmartContractBytes,
        },
    },
    nexus::UniversalAccountId,
    query::error::FindError,
    smart_contract::manifest::{ContractManifest, MANIFEST_METADATA_KEY},
    transaction::{error::TransactionLimitError, signed::TransactionSignatureError},
};
use iroha_logger::{debug, error, warn};
use iroha_macro::FromVariant;
use iroha_primitives::time::TimeSource;
use mv::storage::StorageReadOnly;

use crate::{
    compliance::{LaneComplianceContext, LaneComplianceEvaluation},
    governance::manifest::{GovernanceRules, LaneManifestRegistryHandle},
    interlane::verify_lane_privacy_proofs,
    queue::evaluate_policy_with_catalog,
    smartcontracts::ivm::cache::IvmCache,
    state::{StateBlock, StateReadOnlyWithTransactions, StateTransaction, WorldReadOnly},
};

#[cfg(feature = "telemetry")]
type StateTelemetry = crate::telemetry::StateTelemetry;
#[cfg(not(feature = "telemetry"))]
type StateTelemetry = ();
type NexusDataSpaceId = iroha_data_model::nexus::DataSpaceId;
type NexusLaneId = iroha_data_model::nexus::LaneId;

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
static GOV_NAMESPACE_METADATA_KEY: LazyLock<iroha_data_model::name::Name> = LazyLock::new(|| {
    iroha_data_model::name::Name::from_str("gov_namespace").expect("static governance metadata key")
});
static GOV_CONTRACT_ID_METADATA_KEY: LazyLock<iroha_data_model::name::Name> = LazyLock::new(|| {
    iroha_data_model::name::Name::from_str("gov_contract_id")
        .expect("static governance metadata key")
});
static GOV_APPROVERS_METADATA_KEY: LazyLock<iroha_data_model::name::Name> = LazyLock::new(|| {
    iroha_data_model::name::Name::from_str("gov_manifest_approvers")
        .expect("static governance metadata key")
});
static CONTRACT_NAMESPACE_METADATA_KEY: LazyLock<iroha_data_model::name::Name> =
    LazyLock::new(|| {
        iroha_data_model::name::Name::from_str("contract_namespace")
            .expect("static contract namespace metadata key")
    });
static CONTRACT_ID_METADATA_KEY: LazyLock<iroha_data_model::name::Name> = LazyLock::new(|| {
    iroha_data_model::name::Name::from_str("contract_id").expect("static contract id metadata key")
});
static HEARTBEAT_METADATA_NAME: LazyLock<iroha_data_model::name::Name> = LazyLock::new(|| {
    iroha_data_model::name::Name::from_str("sumeragi_heartbeat")
        .expect("static heartbeat metadata key")
});
#[cfg(test)]
static HEARTBEAT_DOMAIN_ID: LazyLock<DomainId> =
    LazyLock::new(|| DomainId::from_str("sumeragi_heartbeat").expect("static heartbeat domain"));
#[cfg(test)]
static HEARTBEAT_EXPIRES_AT_HEIGHT_NAME: LazyLock<iroha_data_model::name::Name> =
    LazyLock::new(|| {
        iroha_data_model::name::Name::from_str("expires_at_height")
            .expect("static heartbeat expires_at_height metadata key")
    });
#[cfg(test)]
static HEARTBEAT_TX_SEQUENCE_NAME: LazyLock<iroha_data_model::name::Name> = LazyLock::new(|| {
    iroha_data_model::name::Name::from_str("tx_sequence")
        .expect("static heartbeat tx_sequence metadata key")
});
const ED25519_SIGNATURE_LENGTH: usize = 64;
const MULTISIG_DIRECT_SIGN_REJECTION: &str =
    "multisig accounts must use the multisig propose/approve flow; direct signatures are rejected";
/// Prefix used in transaction-limit rejection reasons when the signature cap is exceeded.
pub const SIGNATURE_LIMIT_REASON_PREFIX: &str = "Too many signatures in payload";
#[cfg(feature = "telemetry")]
#[allow(clippy::module_name_repetitions)]
use iroha_data_model::{metadata::Metadata as TelemetryMetadata, name::Name as TelemetryName};
/// `AcceptedTransaction` — a transaction accepted by Iroha peer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct AcceptedTransaction<'tx>(Cow<'tx, SignedTransaction>);

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
        if state.has_transaction(tx.as_ref().hash()) {
            return Err((tx, TransactionAlreadyCommitted));
        }
        Ok(Self(tx))
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
    for (key, value) in metadata.iter() {
        let parsed =
            norito::json::parse_value(value.get()).map_err(|err| TransactionLimitError {
                reason: format!("Metadata `{key}` is not valid JSON: {err}"),
            })?;
        let depth = json_value_depth(&parsed);
        if depth > max_depth {
            return Err(TransactionLimitError {
                reason: format!("Metadata `{key}` nesting depth {depth} exceeds limit {max_depth}"),
            });
        }
    }
    Ok(())
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

fn heartbeat_marker_value(tx: &SignedTransaction) -> Result<Option<bool>, TransactionLimitError> {
    let Some(value) = tx.metadata().get(&*HEARTBEAT_METADATA_NAME) else {
        return Ok(None);
    };

    if let Ok(flag) = value.clone().try_into_any_norito::<bool>() {
        return if flag {
            Ok(Some(true))
        } else {
            Err(TransactionLimitError {
                reason: "Heartbeat metadata `sumeragi_heartbeat` must be true".into(),
            })
        };
    }

    if let Ok(text) = value.clone().try_into_any_norito::<String>() {
        let trimmed = text.trim();
        if trimmed.eq_ignore_ascii_case("true") {
            return Ok(Some(true));
        }
        if trimmed.eq_ignore_ascii_case("false") {
            return Err(TransactionLimitError {
                reason: "Heartbeat metadata `sumeragi_heartbeat` must be true".into(),
            });
        }
    }

    Err(TransactionLimitError {
        reason: "Heartbeat metadata `sumeragi_heartbeat` must be a boolean".into(),
    })
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
    any.is::<iroha_data_model::isi::offline::RegisterOfflineAllowance>()
        || any.is::<iroha_data_model::isi::offline::SubmitOfflineToOnlineTransfer>()
        || any.is::<iroha_data_model::isi::offline::RegisterOfflineVerdictRevocation>()
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
        || any.is::<iroha_data_model::isi::governance::CastZkBallot>()
        || any.is::<iroha_data_model::isi::governance::CastPlainBallot>()
        || any.is::<iroha_data_model::isi::governance::ApproveGovernanceProposal>()
        || any.is::<iroha_data_model::isi::governance::EnactReferendum>()
        || any.is::<iroha_data_model::isi::governance::FinalizeReferendum>()
}

fn is_time_sensitive_executable(executable: &Executable) -> bool {
    match executable {
        Executable::Instructions(instructions) => {
            instructions.iter().any(is_time_sensitive_instruction)
        }
        Executable::Ivm(_) => true,
    }
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

/// Returns `true` if the transaction is a Sumeragi heartbeat (marker, empty instructions, no attachments).
pub(crate) fn is_heartbeat_transaction(tx: &SignedTransaction) -> bool {
    let marker = matches!(heartbeat_marker_value(tx).ok().flatten(), Some(true));
    let empty_instructions = matches!(
        tx.instructions(),
        Executable::Instructions(instructions) if instructions.is_empty()
    );
    let no_attachments = tx.attachments().is_none();
    marker && empty_instructions && no_attachments
}

/// Build a Sumeragi heartbeat transaction using the provided time source.
#[cfg(test)]
pub(crate) fn build_heartbeat_transaction_with_time_source(
    chain_id: ChainId,
    signer: &KeyPair,
    tx_params: &TransactionParameters,
    proposal_height: u64,
    time_source: &TimeSource,
) -> SignedTransaction {
    let authority = AccountId::new(HEARTBEAT_DOMAIN_ID.clone(), signer.public_key().clone());
    let mut metadata = Metadata::default();
    metadata.insert(HEARTBEAT_METADATA_NAME.clone(), Json::new(true));
    if tx_params.require_height_ttl {
        metadata.insert(
            HEARTBEAT_EXPIRES_AT_HEIGHT_NAME.clone(),
            Json::new(proposal_height.saturating_add(1)),
        );
    }
    if tx_params.require_sequence {
        metadata.insert(
            HEARTBEAT_TX_SEQUENCE_NAME.clone(),
            Json::new(proposal_height),
        );
    }
    TransactionBuilder::new_with_time_source(chain_id, authority, time_source)
        .with_metadata(metadata)
        .sign(signer.private_key())
}

impl<'tx> AcceptedTransaction<'tx> {
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

        Ok(())
    }

    fn ensure_signing_allowed(
        tx: &SignedTransaction,
        crypto: &iroha_config::parameters::actual::Crypto,
    ) -> Result<(), AcceptTransactionFail> {
        let signature = tx.signature().clone();
        match tx.authority().controller() {
            iroha_data_model::account::AccountController::Single(signatory) => {
                let algo = signatory.algorithm();
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
                    let algo = member.public_key().algorithm();
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
                        let algo = entry.signer.algorithm();
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

    fn signature_rejection_code(err: &TransactionSignatureError) -> SignatureRejectionCode {
        match err {
            TransactionSignatureError::UnsupportedMultisigAuthority => {
                SignatureRejectionCode::UnsupportedAuthority
            }
            TransactionSignatureError::AlgorithmNotPermitted(_) => {
                SignatureRejectionCode::AlgorithmNotPermitted
            }
            TransactionSignatureError::CryptoError(_) => SignatureRejectionCode::InvalidSignature,
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
        Self::validate_with_now_with_signature_result(
            tx,
            expected_chain_id,
            max_clock_drift,
            limits,
            crypto,
            now,
            None,
        )
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate_with_now_with_signature_result(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
        now: Duration,
        signature_override: Option<Result<(), SignatureVerificationFail>>,
    ) -> Result<(), AcceptTransactionFail> {
        let heartbeat_marker =
            heartbeat_marker_value(tx).map_err(AcceptTransactionFail::TransactionLimit)?;
        if heartbeat_marker == Some(true) {
            return Self::validate_heartbeat_with_now_with_signature_result(
                tx,
                expected_chain_id,
                max_clock_drift,
                limits,
                crypto,
                now,
                signature_override,
            );
        }
        Self::validate_common(tx, expected_chain_id, max_clock_drift, now)?;

        if let Some(ttl) = tx.time_to_live()
            && let Some(expires_at) = tx.creation_time().checked_add(ttl)
            && now > expires_at
        {
            return Err(AcceptTransactionFail::TransactionExpired {
                expires_at_ms: expires_at.as_millis(),
                now_ms: now.as_millis(),
            });
        }

        if *iroha_genesis::GENESIS_DOMAIN_ID == *tx.authority().domain() {
            return Err(AcceptTransactionFail::UnexpectedGenesisAccountSignature);
        }

        Self::ensure_signing_allowed(tx, crypto)?;

        match signature_override {
            Some(Ok(())) => {}
            Some(Err(fail)) => {
                return Err(AcceptTransactionFail::SignatureVerification(fail));
            }
            None => {
                if let Err(err) = tx.verify_signature() {
                    return Err(AcceptTransactionFail::SignatureVerification(
                        Self::signature_fail_from_error(tx, err),
                    ));
                }
            }
        }

        let signature_count = tx.signature_count();
        Self::ensure_signature_limit(signature_count, &limits)?;

        let tx_encoded_len = norito::to_bytes(tx)
            .map(|bytes| bytes.len())
            .map_err(|err| {
                AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                    reason: format!("Failed to encode transaction for size check: {err}"),
                })
            })?;
        let tx_encoded_len = u64::try_from(tx_encoded_len).unwrap_or(u64::MAX);
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

        let decompressed_len = tx.attachments().map_or(0usize, |attachments| {
            attachments.0.iter().fold(0usize, |acc, attachment| {
                let mut subtotal = attachment.proof.bytes.len();
                if let Some(vk) = &attachment.vk_inline {
                    subtotal = subtotal.saturating_add(vk.bytes.len());
                }
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
        ensure_metadata_depth(tx.metadata(), max_metadata_depth)
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
            Executable::Ivm(smart_contract) => {
                let gas_limit_key = iroha_data_model::name::Name::from_str("gas_limit")
                    .expect("static gas_limit key");
                let Some(raw_gas_limit) = tx.metadata().get(&gas_limit_key) else {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: "missing gas_limit in transaction metadata".into(),
                        },
                    ));
                };
                let gas_limit = raw_gas_limit.try_into_any_norito::<u64>().map_err(|err| {
                    AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                        reason: format!("invalid gas_limit metadata: {err}"),
                    })
                })?;
                if gas_limit == 0 {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: "gas_limit must be positive".into(),
                        },
                    ));
                }

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
                let decoded = ivm::ivm_cache::IvmCache::decode_stream(code).map_err(|err| {
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

    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate_heartbeat_with_now(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
        now: Duration,
    ) -> Result<(), AcceptTransactionFail> {
        Self::validate_heartbeat_with_now_with_signature_result(
            tx,
            expected_chain_id,
            max_clock_drift,
            limits,
            crypto,
            now,
            None,
        )
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate_heartbeat_with_now_with_signature_result(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
        crypto: &iroha_config::parameters::actual::Crypto,
        now: Duration,
        signature_override: Option<Result<(), SignatureVerificationFail>>,
    ) -> Result<(), AcceptTransactionFail> {
        let _ = crypto;
        Self::validate_common(tx, expected_chain_id, max_clock_drift, now)?;

        if let Some(ttl) = tx.time_to_live()
            && let Some(expires_at) = tx.creation_time().checked_add(ttl)
            && now > expires_at
        {
            return Err(AcceptTransactionFail::TransactionExpired {
                expires_at_ms: expires_at.as_millis(),
                now_ms: now.as_millis(),
            });
        }

        if *iroha_genesis::GENESIS_DOMAIN_ID == *tx.authority().domain() {
            return Err(AcceptTransactionFail::UnexpectedGenesisAccountSignature);
        }

        if !is_heartbeat_transaction(tx) {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: "Heartbeat transaction must include sumeragi_heartbeat metadata and zero instructions".into(),
                },
            ));
        }

        if tx.attachments().is_some() {
            return Err(AcceptTransactionFail::TransactionLimit(
                TransactionLimitError {
                    reason: "Heartbeat transaction must not include proof attachments".into(),
                },
            ));
        }

        match signature_override {
            Some(Ok(())) => {}
            Some(Err(fail)) => {
                return Err(AcceptTransactionFail::SignatureVerification(fail));
            }
            None => {
                if let Err(err) = tx.verify_signature() {
                    return Err(AcceptTransactionFail::SignatureVerification(
                        Self::signature_fail_from_error(tx, err),
                    ));
                }
            }
        }

        let signature_count = tx.signature_count();
        Self::ensure_signature_limit(signature_count, &limits)?;

        let tx_encoded_len = norito::to_bytes(tx)
            .map(|bytes| bytes.len())
            .map_err(|err| {
                AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                    reason: format!("Failed to encode transaction for size check: {err}"),
                })
            })?;
        let tx_encoded_len = u64::try_from(tx_encoded_len).unwrap_or(u64::MAX);
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

        let decompressed_len = tx.attachments().map_or(0usize, |attachments| {
            attachments.0.iter().fold(0usize, |acc, attachment| {
                let mut subtotal = attachment.proof.bytes.len();
                if let Some(vk) = &attachment.vk_inline {
                    subtotal = subtotal.saturating_add(vk.bytes.len());
                }
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
        ensure_metadata_depth(tx.metadata(), max_metadata_depth)
            .map_err(AcceptTransactionFail::TransactionLimit)?;

        match &tx.instructions() {
            Executable::Instructions(instructions) => {
                if !instructions.is_empty() {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: "Heartbeat transaction must not include instructions".into(),
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
            Executable::Ivm(_) => {
                return Err(AcceptTransactionFail::TransactionLimit(
                    TransactionLimitError {
                        reason: "Heartbeat transaction must not include IVM bytecode".into(),
                    },
                ));
            }
        }

        Ok(())
    }

    /// Create [`Self`] assuming the transaction is acceptable.
    pub fn new_unchecked(tx: impl Into<Cow<'tx, SignedTransaction>>) -> Self {
        Self(tx.into())
    }

    /// Return the canonical hash of the wrapped transaction.
    #[must_use]
    pub fn hash(&self) -> HashOf<SignedTransaction> {
        self.as_ref().hash()
    }

    /// Borrow the transaction authority account identifier.
    #[must_use]
    pub fn authority(&self) -> &AccountId {
        self.as_ref().authority()
    }
}

impl AcceptedTransaction<'static> {
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
        .map(|()| Self(Cow::Owned(tx)))
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
            .map(|()| Self(Cow::Owned(tx)))
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
        Ok(Self(Cow::Owned(tx)))
    }
}

impl<'tx> From<AcceptedTransaction<'tx>> for SignedTransaction {
    fn from(source: AcceptedTransaction<'tx>) -> Self {
        source.0.into_owned()
    }
}

impl<'tx> From<AcceptedTransaction<'tx>> for (AccountId, Executable) {
    fn from(source: AcceptedTransaction<'tx>) -> Self {
        source.0.into_owned().into()
    }
}

impl AsRef<SignedTransaction> for AcceptedTransaction<'_> {
    fn as_ref(&self) -> &SignedTransaction {
        self.0.as_ref()
    }
}

impl StateBlock<'_> {
    /// Validate and apply the transaction to the state if validation succeeds; leave the state unchanged on failure.
    ///
    /// Returns the hash and the result of the transaction -- the trigger sequence on success, or the rejection reason on failure.
    pub fn validate_transaction(
        &mut self,
        tx: AcceptedTransaction<'_>,
        ivm_cache: &mut IvmCache,
    ) -> (HashOf<TransactionEntrypoint>, TransactionResultInner) {
        // Capture gas accounting inputs up front to avoid borrowing conflicts
        let gas_total_before = self.gas_used_in_block;
        let gas_limit = self.gas_limit_per_block;
        let ops_total_before = self.zk_confidential_ops_in_block;
        let verify_calls_before = self.zk_verify_calls_in_block;
        let proof_bytes_before = self.zk_proof_bytes_in_block;
        let conf_gas_before = self.confidential_gas_used_in_block;
        let mut state_transaction = self.transaction();
        let hash = tx.as_ref().hash_as_entrypoint();
        let result = Self::validate_transaction_internal(tx, &mut state_transaction, ivm_cache);
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
    ) -> TransactionResultInner {
        let authority = tx.as_ref().authority().clone();
        let is_heartbeat = is_heartbeat_transaction(tx.as_ref());

        // Heartbeat transactions may be signed by ephemeral identities; other transactions
        // must originate from an existing account.
        if state_transaction.world.accounts.get(&authority).is_none() && !is_heartbeat {
            return Err(TransactionRejectionReason::AccountDoesNotExist(
                FindError::Account(authority.clone()),
            ));
        }

        if !is_heartbeat {
            if let Executable::Instructions(instructions) = tx.as_ref().instructions()
                && instructions.is_empty()
            {
                return Err(TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted(
                        "Transaction must contain at least one instruction".to_owned(),
                    ),
                ));
            }
        }

        let (require_height_ttl, require_sequence) = {
            let params = state_transaction.world.parameters();
            (
                params.transaction.require_height_ttl,
                params.transaction.require_sequence,
            )
        };

        let expires_at_height = tx.as_ref().expires_at_height().map_err(|err| {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
                "Transaction metadata `expires_at_height` must be an unsigned integer: {err}"
            )))
        })?;

        let tx_sequence_value = tx.as_ref().tx_sequence().map_err(|err| {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
                "Transaction metadata `tx_sequence` must be an unsigned integer: {err}"
            )))
        })?;

        let mut sequence_to_commit: Option<u64> = None;

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

        if let Some(account) = state_transaction.world.accounts.get(&authority) {
            let multisig_spec_key = crate::smartcontracts::isi::multisig::spec_key();
            let has_multisig_role = state_transaction
                .world
                .account_roles_iter(&authority)
                .any(|role| role.name().as_ref().starts_with("MULTISIG_SIGNATORY/"));
            if has_multisig_role || account.metadata().get(&multisig_spec_key).is_some() {
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

        let routing_decision = evaluate_policy_with_catalog(
            &state_transaction.nexus.routing_policy,
            &state_transaction.nexus.lane_catalog,
            &state_transaction.nexus.dataspace_catalog,
            &tx,
        );
        let lane_assignment = LaneAssignment {
            lane_id: routing_decision.lane_id,
            dataspace_id: routing_decision.dataspace_id,
            dataspace_catalog: &state_transaction.nexus.dataspace_catalog,
        };

        enforce_lane_policies(tx.as_ref(), state_transaction, &lane_assignment)?;

        if !is_heartbeat {
            enforce_fraud_policy(
                &state_transaction.fraud_monitoring,
                tx.as_ref().metadata(),
                telemetry_handle,
                &lane_assignment,
            )?;
        }

        let manifest_metadata = tx
            .as_ref()
            .metadata()
            .get(&*CONTRACT_MANIFEST_METADATA_NAME)
            .and_then(|json| json.clone().try_into_any_norito::<ContractManifest>().ok());

        // Extract optional governance deployment metadata for protected-namespaces gating
        let (ns_meta, cid_meta) = {
            use core::str::FromStr as _;
            let md = tx.as_ref().metadata();
            let ns_key = iroha_data_model::name::Name::from_str("gov_namespace");
            let cid_key = iroha_data_model::name::Name::from_str("gov_contract_id");
            let ns = ns_key
                .ok()
                .and_then(|k| md.get(&k))
                .map(|j| j.get().clone());
            let cid = cid_key
                .ok()
                .and_then(|k| md.get(&k))
                .map(|j| j.get().clone());
            (ns, cid)
        };

        if let Executable::Ivm(bytes) = tx.as_ref().instructions() {
            let gas_limit = crate::executor::parse_gas_limit(tx.as_ref().metadata())
                .map_err(TransactionRejectionReason::Validation)?;
            if gas_limit.is_none() {
                return Err(TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted(
                        "missing gas_limit in transaction metadata".to_owned(),
                    ),
                ));
            }
            Self::validate_ivm(
                authority.clone(),
                state_transaction,
                bytes.clone(),
                manifest_metadata.clone(),
                ns_meta.clone().zip(cid_meta.clone()),
                ivm_cache,
            )?;
        }

        debug!(tx=%tx.as_ref().hash(), "Validating transaction");
        let trigger_sequence = if is_heartbeat {
            DataTriggerSequence::default()
        } else {
            Self::validate_transaction_with_runtime_executor(
                tx.clone(),
                state_transaction,
                ivm_cache,
            )?;
            debug!("Transaction validated successfully; processing data triggers");
            let trigger_sequence = state_transaction.execute_data_triggers_dfs(&authority)?;
            debug!("Data triggers executed successfully");
            trigger_sequence
        };

        if let Some(seq) = sequence_to_commit {
            state_transaction
                .world
                .tx_sequences
                .insert(authority.clone(), seq);
        }

        Ok(trigger_sequence)
    }

    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    fn validate_ivm(
        authority: AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
        contract: IvmBytecode,
        manifest_metadata: Option<ContractManifest>,
        deploy_target: Option<(String, String)>,
        ivm_cache: &mut IvmCache,
    ) -> Result<(), TransactionRejectionReason> {
        use ivm::ivm_mode as mode;

        // Parse and cache metadata + derived hashes.
        let bytes = contract.as_ref();
        let summary = ivm_cache.summarize_program(bytes).map_err(|e| {
            TransactionRejectionReason::Validation(ValidationFail::InternalError(e.to_string()))
        })?;
        let meta = summary.metadata.clone();
        let offset = summary.code_offset;

        // Version gate: accept known major versions only (1.x for now).
        if meta.version_major != 1 {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::IvmAdmission(
                    iroha_data_model::executor::IvmAdmissionError::UnsupportedVersion(
                        iroha_data_model::executor::UnsupportedVersionInfo {
                            major: meta.version_major,
                            minor: meta.version_minor,
                        },
                    ),
                ),
            ));
        }

        // Feature bits: reject unknown flags to keep behaviour deterministic.
        let known = mode::ZK | mode::VECTOR | mode::HTM;
        if meta.mode & !known != 0 {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::IvmAdmission(
                    iroha_data_model::executor::IvmAdmissionError::UnsupportedFeatureBits(
                        meta.mode & !known,
                    ),
                ),
            ));
        }

        // ABI validation: first release accepts only ABI v1.
        if meta.abi_version != 1 {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::IvmAdmission(
                    iroha_data_model::executor::IvmAdmissionError::UnsupportedAbiVersion(
                        meta.abi_version,
                    ),
                ),
            ));
        }

        // Vector length: 0 means "auto"; otherwise require a sane bound.
        if meta.vector_length != 0 && meta.vector_length > 64 {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::IvmAdmission(
                    iroha_data_model::executor::IvmAdmissionError::VectorLengthTooLarge(
                        iroha_data_model::executor::VectorLengthTooLargeInfo {
                            vector_length: meta.vector_length,
                            max_allowed: 64,
                        },
                    ),
                ),
            ));
        }

        // Fuel (`max_cycles`) must be explicitly provided and bounded.
        if meta.max_cycles == 0 {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::IvmAdmission(
                    iroha_data_model::executor::IvmAdmissionError::MissingMaxCycles,
                ),
            ));
        }

        // Start from configured upper bound and enforce custom overrides.
        let mut upper_bound: u64 = state_transaction.pipeline.ivm_max_cycles_upper_bound;
        let params = state_transaction.world.parameters.get();

        let fuel_limit = params.smart_contract().fuel().get();
        if meta.max_cycles > fuel_limit {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::IvmAdmission(
                    iroha_data_model::executor::IvmAdmissionError::MaxCyclesExceedsFuel(
                        iroha_data_model::executor::MaxCyclesExceedsFuelInfo {
                            max_cycles: meta.max_cycles,
                            fuel_limit,
                        },
                    ),
                ),
            ));
        }

        if let Ok(name) = core::str::FromStr::from_str("max_ivm_cycles_upper_bound") {
            let id = iroha_data_model::parameter::CustomParameterId(name);
            if let Some(custom) = params.custom().get(&id)
                && let Ok(v) = custom.payload().try_into_any_norito::<u64>()
            {
                upper_bound = v;
            }
        }

        if upper_bound != 0 && meta.max_cycles > upper_bound {
            return Err(TransactionRejectionReason::Validation(
                ValidationFail::IvmAdmission(
                    iroha_data_model::executor::IvmAdmissionError::MaxCyclesExceedsUpperBound(
                        iroha_data_model::executor::MaxCyclesExceedsUpperBoundInfo {
                            max_cycles: meta.max_cycles,
                            upper_bound,
                        },
                    ),
                ),
            ));
        }

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
                if ivm::instruction::wide::opcode(op.inst) == ivm::instruction::wide::system::SCALL
                {
                    // SCALL immediate is an unsigned byte; reinterpret negative imm8 as its
                    // 8-bit two's complement value to mirror VM execution semantics.
                    let number = u32::from(ivm::instruction::wide::imm8(op.inst).to_ne_bytes()[0]);
                    if !ivm::syscalls::is_syscall_allowed(policy, number) {
                        return Err(TransactionRejectionReason::Validation(
                            ValidationFail::NotPermitted(format!(
                                "unknown syscall number 0x{number:02x} for abi_version {}",
                                meta.abi_version
                            )),
                        ));
                    }
                }
            }
        }

        // Optional manifest validation (lookup by code_hash).
        // Compute code_hash over the program body (bytes after header) and ABI hash for the policy.
        let code_hash = summary.code_hash;
        let abi_hash = summary.abi_hash;
        let validate_manifest =
            |manifest: &ContractManifest| -> Result<(), TransactionRejectionReason> {
                if let Some(mh) = manifest.code_hash
                    && mh != code_hash
                {
                    return Err(TransactionRejectionReason::Validation(
                        ValidationFail::IvmAdmission(
                            iroha_data_model::executor::IvmAdmissionError::ManifestCodeHashMismatch(
                                iroha_data_model::executor::ManifestCodeHashMismatchInfo {
                                    expected: mh,
                                    actual: code_hash,
                                },
                            ),
                        ),
                    ));
                }
                if let Some(ah) = manifest.abi_hash
                    && ah != abi_hash
                {
                    return Err(TransactionRejectionReason::Validation(
                        ValidationFail::IvmAdmission(
                            iroha_data_model::executor::IvmAdmissionError::ManifestAbiHashMismatch(
                                iroha_data_model::executor::ManifestAbiHashMismatchInfo {
                                    expected: ah,
                                    actual: abi_hash,
                                },
                            ),
                        ),
                    ));
                }
                Ok(())
            };

        if let Some(manifest) = manifest_metadata.as_ref() {
            validate_manifest(manifest)?;
        }
        if let Some(manifest) = state_transaction.world.contract_manifests.get(&code_hash) {
            validate_manifest(manifest)?;
        }

        // Protected namespaces admission (governance gating)
        if let Some((ns, cid)) = deploy_target {
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
            if protected.iter().any(|p| p == &ns) {
                // Require an enacted proposal matching (ns, cid, code_hash, abi_hash)
                let want_code = hex::encode(<[u8; 32]>::from(code_hash));
                let want_abi = hex::encode(<[u8; 32]>::from(abi_hash));
                let mut ok = false;
                for (_pid, rec) in state_transaction.world.governance_proposals.iter() {
                    let Some(payload) = rec.as_deploy_contract() else {
                        continue;
                    };
                    if payload.namespace == ns
                        && payload.contract_id == cid
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
                            "deployment into protected namespace requires enacted governance proposal"
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
        .flat_map(|list| list.0.iter())
        .filter_map(|attachment| attachment.lane_privacy.clone())
        .collect()
}

fn enforce_manifest_quorum(
    alias: &str,
    rules: &GovernanceRules,
    tx: &SignedTransaction,
) -> Result<(), TransactionRejectionReason> {
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
    let authority_ih58 = authority.canonical_ih58().map_err(|err| {
        reject_lane_policy(
            alias,
            format!("failed to encode authority `{authority}` as ih58: {err}"),
        )
    })?;
    approvals.insert(authority_ih58);

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
        let canonical = if let Ok(account) = AccountId::from_str(trimmed) {
            account.canonical_ih58().map_err(|err| {
                reject_lane_policy(
                    alias,
                    format!("invalid account id `{trimmed}` in `gov_manifest_approvers`: {err}"),
                )
            })?
        } else {
            let prefix = iroha_data_model::account::address::chain_discriminant();
            let (address, _) =
                IrohaAccountAddress::parse_any(trimmed, Some(prefix)).map_err(|err| {
                    reject_lane_policy(
                        alias,
                        format!(
                            "invalid account id `{trimmed}` in `gov_manifest_approvers`: {err}"
                        ),
                    )
                })?;
            address.to_ih58(prefix).map_err(|err| {
                reject_lane_policy(
                    alias,
                    format!("invalid account id `{trimmed}` in `gov_manifest_approvers`: {err}"),
                )
            })?
        };
        approvals.insert(canonical);
    }
    Ok(approvals)
}

fn canonical_manifest_validators(
    alias: &str,
    rules: &GovernanceRules,
) -> Result<BTreeSet<String>, TransactionRejectionReason> {
    let mut validators = BTreeSet::new();
    for validator in &rules.validators {
        let ih58 = validator.canonical_ih58().map_err(|err| {
            reject_lane_policy(
                alias,
                format!("failed to encode validator `{validator}` as ih58: {err}"),
            )
        })?;
        validators.insert(ih58);
    }
    Ok(validators)
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
    let metadata_namespace = metadata
        .get(&*GOV_NAMESPACE_METADATA_KEY)
        .map(|value| {
            let raw = value.try_into_any_norito::<String>().map_err(|_| {
                reject_lane_policy(alias, "`gov_namespace` metadata must be a string value")
            })?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(reject_lane_policy(
                    alias,
                    "`gov_namespace` metadata must not be blank",
                ));
            }
            Name::from_str(trimmed).map_err(|err| {
                reject_lane_policy(
                    alias,
                    format!("`gov_namespace` metadata `{trimmed}` is not a valid Name: {err}"),
                )
            })
        })
        .transpose()?;

    let metadata_contract_id = metadata
        .get(&*GOV_CONTRACT_ID_METADATA_KEY)
        .map(|value| {
            let raw = value.try_into_any_norito::<String>().map_err(|_| {
                reject_lane_policy(alias, "`gov_contract_id` metadata must be a string value")
            })?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(reject_lane_policy(
                    alias,
                    "`gov_contract_id` metadata must not be blank",
                ));
            }
            Ok(trimmed.to_string())
        })
        .transpose()?;

    let metadata_contract_namespace = metadata
        .get(&*CONTRACT_NAMESPACE_METADATA_KEY)
        .map(|value| {
            let raw = value.try_into_any_norito::<String>().map_err(|_| {
                reject_lane_policy(
                    alias,
                    "`contract_namespace` metadata must be a string value",
                )
            })?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(reject_lane_policy(
                    alias,
                    "`contract_namespace` metadata must not be blank",
                ));
            }
            Name::from_str(trimmed).map_err(|err| {
                reject_lane_policy(
                    alias,
                    format!("`contract_namespace` metadata `{trimmed}` is not a valid Name: {err}"),
                )
            })
        })
        .transpose()?;

    let metadata_contract_id_hint = metadata
        .get(&*CONTRACT_ID_METADATA_KEY)
        .map(|value| {
            let raw = value.try_into_any_norito::<String>().map_err(|_| {
                reject_lane_policy(alias, "`contract_id` metadata must be a string value")
            })?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(reject_lane_policy(
                    alias,
                    "`contract_id` metadata must not be blank",
                ));
            }
            Ok(trimmed.to_string())
        })
        .transpose()?;

    let mut namespaces_from_instructions = BTreeSet::new();
    let mut contract_bindings = BTreeSet::new();
    let mut register_code_seen = false;
    if let Executable::Instructions(instructions) = tx.instructions() {
        for instruction in instructions {
            if let Some(activate) = instruction
                .as_any()
                .downcast_ref::<ActivateContractInstance>()
            {
                let ns = Name::from_str(activate.namespace.trim()).map_err(|err| {
                    reject_lane_policy(
                        alias,
                        format!(
                            "instruction namespace `{}` is not valid: {err}",
                            activate.namespace
                        ),
                    )
                })?;
                namespaces_from_instructions.insert(ns.clone());
                let contract_id = activate.contract_id.trim();
                if contract_id.is_empty() {
                    return Err(reject_lane_policy(
                        alias,
                        "contract_id in ActivateContractInstance must not be blank",
                    ));
                }
                contract_bindings.insert((ns, contract_id.to_string()));
            } else if let Some(deactivate) = instruction
                .as_any()
                .downcast_ref::<DeactivateContractInstance>()
            {
                let ns = Name::from_str(deactivate.namespace.trim()).map_err(|err| {
                    reject_lane_policy(
                        alias,
                        format!(
                            "instruction namespace `{}` is not valid: {err}",
                            deactivate.namespace
                        ),
                    )
                })?;
                namespaces_from_instructions.insert(ns.clone());
                let contract_id = deactivate.contract_id.trim();
                if contract_id.is_empty() {
                    return Err(reject_lane_policy(
                        alias,
                        "contract_id in DeactivateContractInstance must not be blank",
                    ));
                }
                contract_bindings.insert((ns, contract_id.to_string()));
            } else {
                let modifies_contract_code = {
                    let any = instruction.as_any();
                    any.is::<RegisterSmartContractCode>()
                        || any.is::<RegisterSmartContractBytes>()
                        || any.is::<RemoveSmartContractBytes>()
                };
                if modifies_contract_code {
                    register_code_seen = true;
                }
            }
        }
    }

    if let Some(ns) = metadata_namespace.clone() {
        if let Some(cid) = metadata_contract_id
            .clone()
            .or_else(|| metadata_contract_id_hint.clone())
        {
            namespaces_from_instructions.insert(ns.clone());
            contract_bindings.insert((ns, cid));
        }
    } else if let Some(ns) = metadata_contract_namespace.clone() {
        if let Some(cid) = metadata_contract_id_hint.clone() {
            namespaces_from_instructions.insert(ns.clone());
            contract_bindings.insert((ns, cid));
        }
    }

    let ivm_with_contract_metadata = matches!(tx.instructions(), Executable::Ivm(_))
        && (metadata_namespace.is_some()
            || metadata_contract_namespace.is_some()
            || metadata_contract_id_hint.is_some());

    let contract_instr_seen =
        register_code_seen || !contract_bindings.is_empty() || ivm_with_contract_metadata;

    if contract_instr_seen && metadata_namespace.is_none() {
        return Err(reject_lane_policy(
            alias,
            "transactions with contract namespace operations must set `gov_namespace` metadata when lane governance protects namespaces",
        ));
    }

    if (contract_instr_seen || metadata_namespace.is_some()) && metadata_contract_id.is_none() {
        return Err(reject_lane_policy(
            alias,
            "metadata key `gov_contract_id` is required when `gov_namespace` is provided",
        ));
    }

    if let (Some(ns_hint), Some(ns_meta)) = (
        metadata_contract_namespace.as_ref(),
        metadata_namespace.as_ref(),
    ) {
        if ns_hint != ns_meta {
            return Err(reject_lane_policy(
                alias,
                "`contract_namespace` metadata must match `gov_namespace` for protected operations",
            ));
        }
    }

    if let (Some(cid_hint), Some(cid_meta)) = (
        metadata_contract_id_hint.as_ref(),
        metadata_contract_id.as_ref(),
    ) {
        if cid_hint != cid_meta {
            return Err(reject_lane_policy(
                alias,
                "`contract_id` metadata must match `gov_contract_id` for protected operations",
            ));
        }
    }

    if let Some(meta_cid) = metadata_contract_id.as_ref()
        && let Some(target_ns) = metadata_namespace
            .clone()
            .or_else(|| namespaces_from_instructions.iter().next().cloned())
    {
        let cross_namespace = world
            .contract_instances()
            .iter()
            .filter(|((_ns, cid), _)| cid == meta_cid)
            .filter_map(|((ns, _), _)| Name::from_str(ns).ok())
            .any(|existing_ns| existing_ns != target_ns);
        if cross_namespace {
            return Err(reject_lane_policy(
                alias,
                format!(
                    "contract `{meta_cid}` is already bound to a different namespace; cross-namespace rebinding is not allowed"
                ),
            ));
        }
    }

    let mut namespaces_to_check = namespaces_from_instructions.clone();
    if let Some(ns) = metadata_namespace.clone() {
        namespaces_to_check.insert(ns);
    }
    if let Some(ns) = metadata_contract_namespace.clone() {
        namespaces_to_check.insert(ns);
    }

    for namespace in &namespaces_to_check {
        if !rules.protected_namespaces.contains(namespace) {
            return Err(reject_lane_policy(
                alias,
                format!("namespace `{namespace}` is not declared in lane governance protected set"),
            ));
        }
    }

    if let Some(ns) = metadata_namespace
        && !namespaces_from_instructions.is_empty()
        && namespaces_from_instructions
            .iter()
            .any(|other| other != &ns)
    {
        return Err(reject_lane_policy(
            alias,
            "`gov_namespace` metadata does not match namespaces referenced by contract instructions",
        ));
    }

    if let Some(meta_contract_id) = metadata_contract_id
        && !contract_bindings.is_empty()
        && contract_bindings
            .iter()
            .any(|(_, cid)| cid != &meta_contract_id)
    {
        return Err(reject_lane_policy(
            alias,
            "`gov_contract_id` metadata does not match contract ids referenced by contract instructions",
        ));
    }

    Ok(())
}

fn enforce_runtime_upgrade_hook(
    alias: &str,
    rules: &GovernanceRules,
    tx: &SignedTransaction,
) -> Result<bool, TransactionRejectionReason> {
    let mut contains_runtime_upgrade = false;
    if let Executable::Instructions(instructions) = tx.instructions() {
        for instruction in instructions {
            if instruction
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
            {
                contains_runtime_upgrade = true;
                break;
            }
        }
    }
    if !contains_runtime_upgrade {
        return Ok(false);
    }

    let Some(hook) = rules.hooks.runtime_upgrade.as_ref() else {
        return Ok(false);
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

    Ok(true)
}

fn extract_lane_identity_metadata(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    dataspace_id: NexusDataSpaceId,
    lane_alias: &str,
) -> Result<(Option<UniversalAccountId>, Vec<String>), TransactionRejectionReason> {
    let account_entry = match world.account(authority) {
        Ok(entry) => entry,
        Err(_) => return Ok((None, Vec::new())),
    };
    let Some(uaid) = account_entry.value().uaid().copied() else {
        return Ok((None, Vec::new()));
    };

    let bindings = world.uaid_dataspaces().get(&uaid).ok_or_else(|| {
        reject_lane_policy(
            lane_alias,
            format!(
                "account {authority} carries UAID {uaid} but has no Space Directory bindings for dataspace {}",
                dataspace_id.as_u64()
            ),
        )
    })?;

    let is_bound = bindings
        .iter()
        .any(|(dataspace, accounts)| *dataspace == dataspace_id && accounts.contains(authority));
    if !is_bound {
        return Err(reject_lane_policy(
            lane_alias,
            format!(
                "account {authority} is not bound to dataspace {} for UAID {uaid}",
                dataspace_id.as_u64()
            ),
        ));
    }

    let manifest_set = world
        .space_directory_manifests()
        .get(&uaid)
        .ok_or_else(|| {
            reject_lane_policy(
                lane_alias,
                format!(
                    "UAID {uaid} has no manifest registry for dataspace {}",
                    dataspace_id.as_u64()
                ),
            )
        })?;
    let record = manifest_set.get(&dataspace_id).ok_or_else(|| {
        reject_lane_policy(
            lane_alias,
            format!(
                "UAID {uaid} is missing a manifest for dataspace {}",
                dataspace_id.as_u64()
            ),
        )
    })?;

    if !record.is_active() {
        return Err(reject_lane_policy(
            lane_alias,
            format!(
                "UAID {uaid} manifest for dataspace {} is not active",
                dataspace_id.as_u64()
            ),
        ));
    }

    let mut tags = BTreeSet::new();
    for entry in &record.manifest.entries {
        if let Some(note) = &entry.notes {
            let trimmed = note.trim();
            if !trimmed.is_empty() {
                tags.insert(trimmed.to_string());
            }
        }
    }

    Ok((Some(uaid), tags.into_iter().collect()))
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

    if let Some(status) = manifest_status.as_ref() {
        if let Some(rules) = status.rules() {
            if !rules.validators.is_empty()
                && !rules
                    .validators
                    .iter()
                    .any(|validator| validator == tx.authority())
            {
                return Err(reject_lane_policy(
                    &lane_alias,
                    "authority not part of lane validator set".to_string(),
                ));
            }

            let quorum_required =
                rules.quorum.unwrap_or(0).saturating_sub(1) > 0 && !rules.validators.is_empty();
            let quorum_result = enforce_manifest_quorum(&lane_alias, rules, tx);
            if quorum_required {
                quorum_result?;
            } else if let Err(err) = quorum_result {
                return Err(err);
            }

            enforce_manifest_protected_namespaces(
                &lane_alias,
                rules,
                tx,
                &state_transaction.world,
            )?;

            let _ = enforce_runtime_upgrade_hook(&lane_alias, rules, tx)?;
        }
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

    let lane_identity = extract_lane_identity_metadata(
        &state_transaction.world,
        tx.authority(),
        dataspace_id,
        &lane_alias,
    )?;

    if let Some(engine) = state_transaction.lane_compliance.as_ref() {
        let (uaid_value, capability_tags) = lane_identity;
        let ctx = LaneComplianceContext {
            lane_id,
            dataspace_id,
            authority: tx.authority(),
            uaid: uaid_value.as_ref(),
            capability_tags: capability_tags.as_slice(),
            lane_privacy_registry,
            verified_privacy_commitments: &verified_privacy_commitments,
        };
        let evaluation = engine.evaluate(&ctx);
        match evaluation {
            LaneComplianceEvaluation::NotConfigured => {}
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
        if attester.public_key.algorithm() == iroha_crypto::Algorithm::Ed25519
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
        let signature = iroha_crypto::Signature::from_bytes(signature_bytes);
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
        num::{NonZeroU16, NonZeroU64},
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
        block::{BlockHeader, SignedBlock},
        domain::{Domain, DomainId},
        events::{
            EventBox,
            data::{
                self,
                prelude::{AccountEvent, AssetChanged, AssetEvent, DomainEvent},
            },
            trigger_completed::{TriggerCompletedEvent, TriggerCompletedOutcome},
        },
        isi::{InstructionBox, Log},
        metadata::Metadata,
        name::Name,
        nexus::{
            AuditControls, DataSpaceCatalog, DataSpaceId as TestDataSpaceId, JurisdictionSet,
            LaneCompliancePolicy, LaneCompliancePolicyId, LaneComplianceRule, LaneId as TestLaneId,
            LanePrivacyMerkleWitness, LanePrivacyProof, LanePrivacyWitness, LaneStorageProfile,
            LaneVisibility, ParticipantSelector,
        },
        permission::Permissions,
        proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyBox},
        role::{Role, RoleId},
        transaction::{
            TransactionBuilder,
            signed::{MultisigSignature, MultisigSignatures},
        },
    };
    use iroha_executor_data_model::isi::multisig::{DEFAULT_MULTISIG_TTL_MS, MultisigSpec};
    use iroha_genesis::GENESIS_DOMAIN_ID;
    use iroha_logger::Level;
    use iroha_primitives::{json::Json, numeric::Numeric, time::TimeSource};
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
        query::store::LiveQueryStore,
        smartcontracts::ivm::cache::IvmCache,
        state::{State, StateBlock, StateReadOnly, World},
    };

    fn single_lane_assignment(catalog: &DataSpaceCatalog) -> super::LaneAssignment<'_> {
        super::LaneAssignment {
            lane_id: TestLaneId::SINGLE,
            dataspace_id: TestDataSpaceId::GLOBAL,
            dataspace_catalog: catalog,
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
    fn multisig_authority_rejected_with_stable_code() {
        let chain: ChainId = "multisig-accept".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let mut builder = TransactionBuilder::new(chain.clone(), authority.clone());
        builder = builder.with_instructions([Log::new(Level::INFO, "multisig".into())]);
        let tx = builder.sign(keypair.private_key());

        let member = MultisigMember::new(keypair.public_key().clone(), 1).expect("member is valid");
        let policy = MultisigPolicy::new(1, vec![member]).expect("policy is valid");
        let multisig_authority = AccountId::new_multisig(authority.domain().clone(), policy);
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
        let domain = iroha_data_model::domain::DomainId::from_str("wonderland").unwrap();
        let member_ed = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let member_secp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::Secp256k1);

        let members = vec![
            MultisigMember::new(member_ed.public_key().clone(), 1).expect("member ed"),
            MultisigMember::new(member_secp.public_key().clone(), 1).expect("member secp"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let authority = AccountId::new_multisig(domain.clone(), policy.clone());

        let mut builder = TransactionBuilder::new(chain.clone(), authority.clone());
        builder = builder.with_instructions([Log::new(Level::INFO, "multisig ok".into())]);
        let tx = builder.sign(member_ed.private_key());

        let payload = tx.payload().clone();
        let signatures = vec![
            iroha_data_model::transaction::signed::MultisigSignature::new(
                member_ed.public_key().clone(),
                SignatureOf::new(member_ed.private_key(), &payload),
            ),
            iroha_data_model::transaction::signed::MultisigSignature::new(
                member_secp.public_key().clone(),
                SignatureOf::new(member_secp.private_key(), &payload),
            ),
        ];
        let mut tx = tx;
        tx.set_multisig_signatures(
            iroha_data_model::transaction::signed::MultisigSignatures::new(signatures),
        );

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
        let mut builder = TransactionBuilder::new(chain.clone(), authority.clone());
        builder = builder.with_instructions([Log::new(Level::INFO, "multisig".into())]);
        let tx = builder.sign(keypair.private_key());

        let member = MultisigMember::new(keypair.public_key().clone(), 2).expect("member is valid");
        let policy = MultisigPolicy::new(2, vec![member]).expect("policy is valid");
        let multisig_authority = AccountId::new_multisig(authority.domain().clone(), policy);
        let mut tx = tx.with_authority(multisig_authority);

        // Attach a signature from an unknown signer.
        let payload = tx.payload().clone();
        let rogue = iroha_crypto::KeyPair::random();
        let rogue_sig = SignatureOf::new(rogue.private_key(), &payload);
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
        let domain: iroha_data_model::domain::DomainId = "wonderland".parse().unwrap();
        let signer = iroha_crypto::KeyPair::random();
        let other = iroha_crypto::KeyPair::random();

        let members = vec![
            MultisigMember::new(signer.public_key().clone(), 1).expect("member"),
            MultisigMember::new(other.public_key().clone(), 1).expect("member"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let authority = AccountId::new_multisig(domain.clone(), policy);

        let mut builder = TransactionBuilder::new(chain.clone(), authority.clone());
        builder = builder
            .with_instructions([Log::new(Level::INFO, "insufficient multisig weight".into())]);
        let mut tx = builder.sign(signer.private_key());

        let payload = tx.payload().clone();
        tx.set_multisig_signatures(MultisigSignatures::new(vec![MultisigSignature::new(
            signer.public_key().clone(),
            SignatureOf::new(signer.private_key(), &payload),
        )]));

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
        let domain_id: DomainId = "multisig".parse().unwrap();
        let signer1 = KeyPair::random();
        let signer2 = KeyPair::random();
        let signer1_id = AccountId::new(domain_id.clone(), signer1.public_key().clone());
        let signer2_id = AccountId::new(domain_id.clone(), signer2.public_key().clone());

        let spec = MultisigSpec {
            signatories: BTreeMap::from([(signer1_id.clone(), 1), (signer2_id.clone(), 1)]),
            quorum: NonZeroU16::new(2).expect("nonzero quorum"),
            transaction_ttl_ms: NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS)
                .expect("nonzero multisig ttl"),
        };

        let multisig_key = KeyPair::random();
        let multisig_id = AccountId::new(domain_id.clone(), multisig_key.public_key().clone());

        let mut multisig_metadata = Metadata::default();
        multisig_metadata.insert(
            crate::smartcontracts::isi::multisig::spec_key(),
            Json::new(spec),
        );

        let domain = Domain::new(domain_id.clone()).build(&signer1_id);
        let accounts = [
            Account::new(signer1_id.clone()).build(&signer1_id),
            Account::new(signer2_id.clone()).build(&signer2_id),
            Account::new(multisig_id.clone())
                .with_metadata(multisig_metadata)
                .build(&multisig_id),
        ];
        let world = World::with([domain], accounts, []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let tx = TransactionBuilder::new(chain.clone(), multisig_id.clone())
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
    fn multisig_account_direct_signing_rejected_when_only_role_present() {
        use iroha_data_model::domain::DomainId;

        let chain: ChainId = "multisig-role-only".parse().unwrap();
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let (authority_id, keypair) = gen_account_in("wonderland");

        let domain = Domain::new(domain_id.clone()).build(&authority_id);
        let account = Account::new(authority_id.clone()).build(&authority_id);
        let mut world = World::with([domain], [account], []);

        let role_id: RoleId = format!(
            "MULTISIG_SIGNATORY/{}/{}",
            authority_id.domain(),
            authority_id.signatory()
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

        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
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
    fn multisig_authority_rejects_disallowed_algorithm() {
        let chain: ChainId = "multisig-disallowed".parse().unwrap();
        let domain = iroha_data_model::domain::DomainId::from_str("wonderland").unwrap();
        let member = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::Secp256k1);

        let members = vec![MultisigMember::new(member.public_key().clone(), 1).expect("member")];
        let policy = MultisigPolicy::new(1, members).expect("policy");
        let authority = AccountId::new_multisig(domain.clone(), policy);

        let mut builder = TransactionBuilder::new(chain.clone(), authority);
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
        let domain = iroha_data_model::domain::DomainId::from_str("wonderland").unwrap();
        let member_a = iroha_crypto::KeyPair::random();
        let member_b = iroha_crypto::KeyPair::random();

        let members = vec![
            MultisigMember::new(member_a.public_key().clone(), 1).expect("member a"),
            MultisigMember::new(member_b.public_key().clone(), 1).expect("member b"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let authority = AccountId::new_multisig(domain.clone(), policy);

        let mut builder = TransactionBuilder::new(chain.clone(), authority);
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
        let domain = iroha_data_model::domain::DomainId::from_str("wonderland").unwrap();
        let signer = iroha_crypto::KeyPair::random();

        let members = vec![MultisigMember::new(signer.public_key().clone(), 1).expect("member")];
        let policy = MultisigPolicy::new(1, members).expect("policy");
        let authority = AccountId::new_multisig(domain.clone(), policy);

        let mut tx = TransactionBuilder::new(chain.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, "multisig too many signatures".into())])
            .sign_multisig(vec![signer.private_key()]);

        let payload = tx.payload().clone();
        let member_signature = SignatureOf::new(signer.private_key(), &payload);
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
        let signed = TransactionBuilder::new(chain.clone(), authority.clone())
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
    fn accepted_transaction_into_checked_detects_committed() {
        let chain: ChainId = "checked-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let instruction = Log::new(Level::INFO, "commit".into());
        let signed = TransactionBuilder::new(chain.clone(), authority.clone())
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
    #[allow(clippy::too_many_lines)]
    fn time_sensitive_instruction_detects_governance_and_non_sensitive() {
        let (authority, _keypair) = gen_account_in("wonderland");
        let (counterparty, _keypair) = gen_account_in("wonderland");
        let ballot = iroha_data_model::isi::governance::CastPlainBallot {
            referendum_id: "ref-1".into(),
            owner: authority.clone(),
            amount: 1,
            duration_blocks: 1,
            direction: 0,
        };
        let ballot_box = InstructionBox::from(ballot);
        assert!(super::is_time_sensitive_instruction(&ballot_box));

        let agreement_id: iroha_data_model::repo::RepoAgreementId =
            "repo-1".parse().expect("repo id");
        let cash_leg = iroha_data_model::repo::RepoCashLeg {
            asset_definition_id: "usd#wonderland".parse().expect("asset id"),
            quantity: 1u32.into(),
        };
        let collateral_leg = iroha_data_model::repo::RepoCollateralLeg::new(
            "bond#wonderland".parse().expect("asset id"),
            1u32.into(),
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
        let reverse = iroha_data_model::isi::repo::ReverseRepoIsi::new(
            agreement_id.clone(),
            counterparty.clone(),
            authority.clone(),
            cash_leg.clone(),
            collateral_leg.clone(),
            1_700_000_100_000,
        );
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
                "bond#wonderland".parse().expect("asset id"),
                1u32.into(),
                counterparty.clone(),
                authority.clone(),
            ),
            iroha_data_model::isi::settlement::SettlementLeg::new(
                "usd#wonderland".parse().expect("asset id"),
                1u32.into(),
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
                "eur#wonderland".parse().expect("asset id"),
                1u32.into(),
                counterparty.clone(),
                authority.clone(),
            ),
            iroha_data_model::isi::settlement::SettlementLeg::new(
                "usd#wonderland".parse().expect("asset id"),
                1u32.into(),
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
            amount: 1,
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
            amount: 1,
            duration_blocks: 1,
            direction: 0,
        };
        let tx = TransactionBuilder::new(chain, authority)
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
        let tx = TransactionBuilder::new(chain, authority)
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

    #[test]
    fn tx_rejected_when_gas_asset_required_but_missing() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        // Minimal state with one domain/account as authority
        let (authority_id, kp) = gen_account_in("domain");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("domain".parse().unwrap()).build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let mut state = State::new_with_chain(world, kura, query_handle, chain.clone());

        // Configure pipeline gas allowlist
        let mut pipeline = state.pipeline.clone();
        pipeline.gas.accepted_assets = vec!["xor#domain".to_string()];
        state.set_pipeline(pipeline);

        // Build minimal IVM program (HALT) without gas_asset_id metadata
        let program = minimal_ivm_program_with_max_cycles(1, 1_000);
        let chain: ChainId = "chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
            .sign(kp.private_key());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
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

    const IVM_METADATA_HEADER_LEN: usize = 17;
    const LITERAL_SECTION_MAGIC: [u8; 4] = *b"LTLB";

    /// Build a minimal valid IVM program: header (1.0, vector=4, `max_cycles=0`, abi=1) + HALT.
    fn minimal_ivm_program(abi_version: u8) -> Vec<u8> {
        let mut code = Vec::new();
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let mut program = Vec::new();
        program.extend_from_slice(b"IVM\0");
        program.extend_from_slice(&[1, 0, 0, 4]);
        program.extend_from_slice(&1_000u64.to_le_bytes());
        program.push(abi_version);
        program.extend_from_slice(&code);
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
        let mut program = Vec::new();
        program.extend_from_slice(b"IVM\0");
        program.extend_from_slice(&[1, 0, 0, 4]);
        program.extend_from_slice(&max_cycles.to_le_bytes());
        program.push(abi_version);
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
        let post_pad = pad_len - 16;
        let post_pad_u32 =
            u32::try_from(post_pad).expect("pad length exceeds literal section encoding");

        let mut padded = program;
        padded.extend_from_slice(&LITERAL_SECTION_MAGIC);
        padded.extend_from_slice(&0u32.to_le_bytes()); // literal count
        padded.extend_from_slice(&post_pad_u32.to_le_bytes());
        padded.extend_from_slice(&0u32.to_le_bytes()); // literal data bytes
        padded.resize(padded.len() + post_pad, 0);
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

        let mut program = Vec::new();
        program.extend_from_slice(b"IVM\0");
        program.extend_from_slice(&[1, 0, 0, 4]);
        program.extend_from_slice(&1_000u64.to_le_bytes());
        program.push(abi_version);
        program.extend_from_slice(&code);
        program
    }

    const TEST_GAS_LIMIT: u64 = 1_000_000;

    fn metadata_with_gas_limit(limit: u64) -> Metadata {
        let mut metadata = Metadata::default();
        insert_gas_limit(&mut metadata, limit);
        metadata
    }

    fn insert_gas_limit(metadata: &mut Metadata, limit: u64) {
        use iroha_data_model::name::Name;
        use iroha_primitives::json::Json;

        let key = Name::from_str("gas_limit").expect("static gas_limit key");
        metadata.insert(key, Json::new(limit));
    }

    #[test]
    fn validate_ivm_header_accepts_supported_versions() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        // World with a single domain and account as authority
        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_program(1);
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_program(3); // unsupported abi_version
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
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
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
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
    fn validate_ivm_manifest_metadata_conflict_rejected_even_if_state_matches() {
        use iroha_data_model::{
            smart_contract::manifest::ContractManifest,
            transaction::{Executable, TransactionBuilder},
        };
        use nonzero_ext::nonzero;

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        // Seed block 1 with a correct manifest for the program.
        let header1 =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block1 = state.block(header1);
        let mut tx1 = block1.transaction();
        let prog = minimal_ivm_program(1);
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);
        let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        tx1.world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                code_hash: Some(code_hash),
                abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                kotoba: None,
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
            code_hash: Some(code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(wrong_abi)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&kp);
        let mut md = metadata_with_gas_limit(TEST_GAS_LIMIT);
        md.insert(
            "contract_manifest".parse::<Name>().unwrap(),
            Json::new(manifest),
        );
        let tx = TransactionBuilder::new(chain, authority_id.clone())
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
        use iroha_data_model::{
            smart_contract::manifest::ContractManifest,
            transaction::{Executable, TransactionBuilder},
        };
        use nonzero_ext::nonzero;

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        // Build minimal program with abi_version=1 (V1)
        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_program(1);
        // Compute code hash over bytes after header
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);
        // Compute abi hash for the policy
        let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        // Attach manifest in metadata
        let manifest = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&kp);
        let mut md = metadata_with_gas_limit(TEST_GAS_LIMIT);
        md.insert(
            "contract_manifest".parse::<Name>().unwrap(),
            Json::new(manifest),
        );
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(md)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        assert!(result.is_ok(), "valid manifest should pass: {result:?}");
    }

    #[test]
    fn validate_ivm_manifest_rejects_mismatched_hashes() {
        use iroha_data_model::{
            smart_contract::manifest::ContractManifest,
            transaction::{Executable, TransactionBuilder},
        };
        use nonzero_ext::nonzero;

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_program(1);
        // Compute real code hash; then corrupt expected
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);
        // Compute abi hash then flip
        let mut wrong_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        wrong_abi[0] ^= 0xAA;
        let manifest = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(wrong_abi)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&kp);
        let mut md = metadata_with_gas_limit(TEST_GAS_LIMIT);
        md.insert(
            "contract_manifest".parse::<Name>().unwrap(),
            Json::new(manifest),
        );
        let tx = TransactionBuilder::new(chain, authority_id.clone())
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_program(1);
        let mut wrong_bytes = [0u8; 32];
        wrong_bytes[0] = 0xFF;
        wrong_bytes[31] = 1; // set LSB as per Hash invariant
        let wrong_code_hash = iroha_crypto::Hash::prehashed(wrong_bytes);
        let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let manifest = ContractManifest {
            code_hash: Some(wrong_code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&kp);
        let mut md = metadata_with_gas_limit(TEST_GAS_LIMIT);
        md.insert(
            "contract_manifest".parse::<Name>().unwrap(),
            Json::new(manifest),
        );
        let tx = TransactionBuilder::new(chain, authority_id.clone())
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        // Seed block 1 with a manifest that has the right code_hash but wrong abi_hash.
        let header1 =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block1 = state.block(header1);
        let mut tx1 = block1.transaction();
        let prog = minimal_ivm_program(1);
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);
        let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let mut wrong_abi = abi_hash;
        wrong_abi[0] ^= 0x5A;
        tx1.world.contract_manifests.insert(
            code_hash,
            ContractManifest {
                code_hash: Some(code_hash),
                abi_hash: Some(iroha_crypto::Hash::prehashed(wrong_abi)),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                kotoba: None,
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
            code_hash: Some(code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&kp);
        let mut md = metadata_with_gas_limit(TEST_GAS_LIMIT);
        md.insert(
            "contract_manifest".parse::<Name>().unwrap(),
            Json::new(manifest),
        );
        let tx = TransactionBuilder::new(chain, authority_id.clone())
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
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
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let prog = minimal_ivm_program_with_max_cycles(1, 0);
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let mut state = State::new_with_chain(world, kura, query_handle, chain.clone());

        // Raise pipeline upper bound above fuel limit so the fuel check triggers first.
        let mut pipeline = state.pipeline.clone();
        let fuel_limit = state.world.parameters.view().smart_contract().fuel().get();
        pipeline.ivm_max_cycles_upper_bound = fuel_limit + 10;
        state.set_pipeline(pipeline);

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let prog = minimal_ivm_program_with_max_cycles(1, fuel_limit + 1);
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
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
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
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
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
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
        use iroha_data_model::{
            smart_contract::manifest::ContractManifest,
            transaction::{Executable, TransactionBuilder},
        };
        use nonzero_ext::nonzero;

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
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
        let prog = minimal_ivm_program(1);
        let parsed = ivm::ProgramMetadata::parse(&prog).expect("header parse");
        let code_hash = iroha_crypto::Hash::new(&prog[parsed.header_len..]);
        let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let manifest = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
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
        let chain: ChainId = "chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());
        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block2.validate_transaction(accepted, &mut ivm_cache);
        assert!(result.is_ok(), "lookup manifest should allow validation");
    }

    #[test]
    fn validate_ivm_unknown_syscall_rejected_at_admission() {
        use iroha_data_model::transaction::{Executable, TransactionBuilder};
        use nonzero_ext::nonzero;

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        // Program issues SCALL 0xAB (not mapped by the ABI policy) then HALT; admission should
        // reject before the VM runs.
        let prog = minimal_ivm_program_with_syscall(1, 0xAB);
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
                assert!(
                    msg.contains("unknown syscall number 0xab") && msg.contains("abi_version 1"),
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
        let tx = TransactionBuilder::new(chain_id.clone(), authority_id.clone())
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
        let forged_signature = iroha_crypto::Signature::from_bytes(&signature_payload);
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
        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
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

        // Create a blob exactly at the allowed size (1024 bytes)
        let at_limit_blob = minimal_ivm_program_with_literal_padding(1, 1024);
        assert_eq!(at_limit_blob.len(), 1024);
        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
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
    fn ivm_missing_gas_limit_rejected_at_admission() {
        use std::time::Duration;

        use iroha_data_model::transaction::{Executable, TransactionBuilder};

        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let prog = minimal_ivm_program_with_max_cycles(1, 1_000);
        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let limits = TransactionParameters::default();
        let err =
            AcceptedTransaction::validate(&tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect_err("missing gas_limit should be rejected");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit.reason.contains("missing gas_limit"),
                    "unexpected reason: {}",
                    limit.reason
                );
            }
            other => panic!("Expected TransactionLimit failure, got {other:?}"),
        }
    }

    #[test]
    fn ivm_zero_gas_limit_rejected_at_admission() {
        use std::time::Duration;

        use iroha_data_model::transaction::{Executable, TransactionBuilder};

        let chain: ChainId = "chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let prog = minimal_ivm_program_with_max_cycles(1, 1_000);
        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
            .with_metadata(metadata_with_gas_limit(0))
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let limits = TransactionParameters::default();
        let err =
            AcceptedTransaction::validate(&tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect_err("zero gas_limit should be rejected");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit.reason.contains("gas_limit must be positive"),
                    "unexpected reason: {}",
                    limit.reason
                );
            }
            other => panic!("Expected TransactionLimit failure, got {other:?}"),
        }
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

        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
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

        let proof = ProofBox::new("halo2/ipa".into(), vec![0u8; 96]);
        let vk = VerifyingKeyBox::new("halo2/ipa".into(), vec![0u8; 96]);
        let attachment = ProofAttachment::new_inline("halo2/ipa".into(), proof, vk);
        let attachments = ProofAttachmentList(vec![attachment]);

        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
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
    fn accept_transaction_requires_expires_at_height_when_configured() {
        use std::time::Duration;

        use iroha_data_model::isi::Log;
        use iroha_logger::Level;

        let chain: ChainId = "ttl-config-chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");

        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
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

        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
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
    fn heartbeat_transaction_detected_and_validated() {
        use std::time::Duration;

        let chain: ChainId = "heartbeat-chain".parse().unwrap();
        let signer = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let tx_params = TransactionParameters::default();
        let tx = build_heartbeat_transaction_with_time_source(
            chain.clone(),
            &signer,
            &tx_params,
            1,
            &time_source,
        );

        assert!(is_heartbeat_transaction(&tx));

        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let result = AcceptedTransaction::validate_heartbeat_with_now(
            &tx,
            &chain,
            Duration::ZERO,
            tx_params,
            &crypto_cfg,
            time_source.get_unix_time(),
        );
        assert!(
            result.is_ok(),
            "heartbeat should validate via heartbeat path"
        );
    }

    #[test]
    fn signature_verification_result_reports_invalid_signature() {
        use std::time::Duration;

        let chain: ChainId = "sig-check".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let (other_id, _other_kp) = gen_account_in("underland");

        let tx = TransactionBuilder::new(chain, authority_id)
            .with_instructions([Log::new(Level::INFO, "sig".into())])
            .sign(kp.private_key());
        let tampered = tx.with_authority(other_id);

        let err =
            AcceptedTransaction::signature_verification_result(&tampered).expect_err("must fail");
        assert_eq!(err.code, SignatureRejectionCode::InvalidSignature);

        let now = tampered.creation_time();
        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let override_err = SignatureVerificationFail::new(
            tampered.signature().clone(),
            SignatureRejectionCode::InvalidSignature,
            "override".to_string(),
        );

        let chain_id = tampered.chain().clone();
        let result = AcceptedTransaction::validate_with_now_with_signature_result(
            &tampered,
            &chain_id,
            Duration::ZERO,
            limits,
            &crypto_cfg,
            now,
            Some(Err(override_err.clone())),
        );
        assert!(matches!(
            result,
            Err(AcceptTransactionFail::SignatureVerification(err)) if err == override_err
        ));
    }

    #[test]
    fn heartbeat_marker_string_true_is_accepted() {
        use std::time::Duration;

        let chain: ChainId = "heartbeat-marker-true".parse().unwrap();
        let signer = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let authority = AccountId::new(HEARTBEAT_DOMAIN_ID.clone(), signer.public_key().clone());
        let mut metadata = Metadata::default();
        metadata.insert(HEARTBEAT_METADATA_NAME.clone(), Json::new("true"));

        let tx = TransactionBuilder::new_with_time_source(chain.clone(), authority, &time_source)
            .with_metadata(metadata)
            .sign(signer.private_key());
        let tx_params = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();

        let result = AcceptedTransaction::accept_with_time_source(
            tx,
            &chain,
            Duration::ZERO,
            tx_params,
            &crypto_cfg,
            &time_source,
        );
        assert!(result.is_ok(), "string heartbeat marker should be accepted");
    }

    #[test]
    fn heartbeat_marker_false_is_rejected() {
        use std::time::Duration;

        let chain: ChainId = "heartbeat-marker-false".parse().unwrap();
        let signer = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let authority = AccountId::new(HEARTBEAT_DOMAIN_ID.clone(), signer.public_key().clone());
        let mut metadata = Metadata::default();
        metadata.insert(HEARTBEAT_METADATA_NAME.clone(), Json::new(false));

        let tx = TransactionBuilder::new_with_time_source(chain.clone(), authority, &time_source)
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
    fn heartbeat_marker_invalid_value_is_rejected() {
        use std::time::Duration;

        let chain: ChainId = "heartbeat-marker-invalid".parse().unwrap();
        let signer = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let authority = AccountId::new(HEARTBEAT_DOMAIN_ID.clone(), signer.public_key().clone());
        let mut metadata = Metadata::default();
        metadata.insert(HEARTBEAT_METADATA_NAME.clone(), Json::new("nope"));

        let tx = TransactionBuilder::new_with_time_source(chain.clone(), authority, &time_source)
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
    fn heartbeat_metadata_rejects_non_empty_instructions() {
        use std::time::Duration;

        use iroha_data_model::isi::Log;
        use iroha_logger::Level;

        let chain: ChainId = "heartbeat-metadata-chain".parse().unwrap();
        let (authority_id, kp) = gen_account_in("wonderland");
        let mut metadata = Metadata::default();
        metadata.insert(HEARTBEAT_METADATA_NAME.clone(), Json::new(true));

        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
            .with_instructions([Log::new(Level::INFO, "noop".into())])
            .with_metadata(metadata)
            .sign(kp.private_key());

        let limits = TransactionParameters::default();
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();

        let err =
            AcceptedTransaction::accept(tx, &chain, Duration::from_secs(0), limits, &crypto_cfg)
                .expect_err("heartbeat marker should force heartbeat validation");

        match err {
            AcceptTransactionFail::TransactionLimit(limit) => {
                assert!(
                    limit.reason.contains("Heartbeat transaction"),
                    "expected heartbeat rejection, got {limit:?}"
                );
            }
            other => panic!("expected heartbeat validation failure, got {other:?}"),
        }
    }

    #[test]
    fn heartbeat_execution_allows_missing_authority_account() {
        use std::time::Duration;

        let chain: ChainId = "heartbeat-exec-chain".parse().unwrap();
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());
        let tx_params = state.view().world().parameters().transaction();
        let signer = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let tx = build_heartbeat_transaction_with_time_source(
            chain.clone(),
            &signer,
            &tx_params,
            1,
            &time_source,
        );
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);

        assert!(
            result.is_ok(),
            "heartbeat should succeed without authority account"
        );
    }

    #[test]
    fn transaction_expired_at_height_is_rejected_by_state() {
        use std::time::Duration;

        use iroha_data_model::{isi::Log, metadata::Metadata, transaction::TransactionBuilder};
        use iroha_logger::Level;
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let mut world = World::with([domain], [account], []);
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

        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let mut world = World::with([domain], [account], []);
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

        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let mut world = World::with([domain], [account], []);
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

        let tx = TransactionBuilder::new(chain.clone(), authority_id.clone())
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
    fn ivm_max_cycles_exceeds_upper_bound_is_rejected() {
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        // Set custom parameter: upper bound = 1_000 cycles
        let id = CustomParameterId::new(Name::from_str("max_ivm_cycles_upper_bound").unwrap());
        let custom = CustomParameter::new(id, Json::new(1_000u64));
        block
            .world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Custom(custom));

        // Build program with max_cycles = 2000
        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_program_with_max_cycles(1, 2_000);
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(prog)))
            .sign(kp.private_key());

        let mut ivm_cache = IvmCache::new();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        assert!(result.is_err(), "max_cycles above bound must be rejected");
    }

    #[test]
    fn ivm_max_cycles_within_upper_bound_is_accepted() {
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

        let (authority_id, kp) = gen_account_in("wonderland");
        let domain: iroha_data_model::domain::Domain =
            iroha_data_model::domain::Domain::new("wonderland".parse().unwrap())
                .build(&authority_id);
        let account =
            iroha_data_model::account::Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [account], []);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);

        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        // Set custom parameter: upper bound = 4_000 cycles
        let id = CustomParameterId::new(Name::from_str("max_ivm_cycles_upper_bound").unwrap());
        let custom = CustomParameter::new(id, Json::new(4_000u64));
        block
            .world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Custom(custom));

        // Build program with max_cycles = 2000, below the bound
        let chain: ChainId = "chain".parse().unwrap();
        let prog = minimal_ivm_program_with_max_cycles(1, 2_000);
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata_with_gas_limit(TEST_GAS_LIMIT))
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

        /// Data trigger chains must roll back when a transfer uses a negative amount.
        #[tokio::test]
        async fn atomically_aborts_on_negative_amount_from_transaction() {
            let sandbox = || {
                let mut res = Sandbox::default();
                res.request_transfer("alice", 50, "bob");
                res
            };

            aborts_on_negative_amount(sandbox(), "txn");
        }

        /// Negative transfer amounts should abort chains initiated by time triggers as well.
        #[tokio::test]
        async fn atomically_aborts_on_negative_amount_from_time_trigger() {
            let sandbox = || Sandbox::default().with_time_trigger_transfer("alice", 50, "bob");

            aborts_on_negative_amount(sandbox(), "time");
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

        fn aborts_on_negative_amount(sandbox: Sandbox, snapshot_suffix: &str) {
            let negative = Numeric::try_new(-1_i128, 0).expect("negative numeric amount");
            let mut sandbox = sandbox
                .with_data_trigger_transfer("bob", 10, "carol")
                .with_data_trigger_transfer("bob", 10, "dave")
                .with_data_trigger_transfer_numeric("dave", negative, "eve");
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
                format!("data_trigger/aborts_on_negative_amount-{snapshot_suffix}"),
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

    #[test]
    fn state_rejects_empty_instructions_non_heartbeat() {
        let chain: ChainId = "empty-instructions-chain".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let domain = Domain::new("wonderland".parse().unwrap()).build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let world = World::with([domain], [account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let tx = TransactionBuilder::new(chain, authority)
            .with_instructions(std::iter::empty::<InstructionBox>())
            .sign(keypair.private_key());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);

        match result {
            Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
                assert!(
                    msg.contains("at least one instruction"),
                    "expected empty-instruction rejection, got {msg}"
                );
            }
            other => panic!("expected empty-instruction rejection, got {other:?}"),
        }
    }

    #[test]
    fn lane_privacy_proofs_collected_from_attachments() {
        let chain: ChainId = "lane-privacy-collect".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let backend = Ident::from_str("halo2/ipa").expect("ident");

        let proof1 = LanePrivacyProof {
            commitment_id: LaneCommitmentId::new(1),
            witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
                leaf: [0x11; 32],
                proof: MerkleProof::from_audit_path_bytes(0, Vec::new()),
            }),
        };
        let proof2 = LanePrivacyProof {
            commitment_id: LaneCommitmentId::new(2),
            witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
                leaf: [0x22; 32],
                proof: MerkleProof::from_audit_path_bytes(0, Vec::new()),
            }),
        };

        let mut attachment1 = ProofAttachment::new_inline(
            backend.clone(),
            ProofBox::new(backend.clone(), vec![0xAA]),
            VerifyingKeyBox::new(backend.clone(), vec![0xBB]),
        );
        attachment1.lane_privacy = Some(proof1.clone());
        let mut attachment2 = ProofAttachment::new_inline(
            backend.clone(),
            ProofBox::new(backend.clone(), vec![0xCC]),
            VerifyingKeyBox::new(backend, vec![0xDD]),
        );
        attachment2.lane_privacy = Some(proof2.clone());

        let tx = TransactionBuilder::new(chain, authority)
            .with_instructions([Log::new(Level::INFO, "noop".into())])
            .with_attachments(ProofAttachmentList(vec![attachment1, attachment2]))
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
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let primary_keypair = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
        let secondary_keypair = KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519);
        let primary_id = AccountId::new(domain_id.clone(), primary_keypair.public_key().clone());
        let secondary_id =
            AccountId::new(domain_id.clone(), secondary_keypair.public_key().clone());

        let rules = GovernanceRules {
            validators: vec![primary_id.clone(), secondary_id.clone()],
            quorum: Some(2),
            ..GovernanceRules::default()
        };
        let lane_alias = "gov";

        let tx = TransactionBuilder::new(chain.clone(), primary_id.clone())
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
        let tx = TransactionBuilder::new(chain, primary_id)
            .with_instructions([Log::new(Level::INFO, "noop".into())])
            .with_metadata(metadata)
            .sign(primary_keypair.private_key());
        let result = enforce_manifest_quorum(lane_alias, &rules, &tx);
        assert!(result.is_ok(), "quorum satisfied should pass: {result:?}");
    }

    #[test]
    fn manifest_protected_namespaces_require_metadata() {
        let chain: ChainId = "lane-protected-ns".parse().unwrap();
        let (authority, keypair) = gen_account_in("wonderland");
        let mut rules = GovernanceRules::default();
        rules
            .protected_namespaces
            .insert(Name::from_str("apps").expect("namespace"));

        let instruction = iroha_data_model::isi::smart_contract_code::ActivateContractInstance {
            namespace: "apps".to_string(),
            contract_id: "calc".to_string(),
            code_hash: Hash::prehashed([0_u8; 32]),
        };
        let tx = TransactionBuilder::new(chain, authority)
            .with_instructions([instruction])
            .sign(keypair.private_key());

        let world = World::default();
        let world_view = world.view();
        let err = super::enforce_manifest_protected_namespaces("lane-0", &rules, &tx, &world_view)
            .expect_err("missing governance metadata should reject");
        match err {
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
                assert!(
                    msg.contains("gov_namespace"),
                    "expected gov_namespace rejection, got {msg}"
                );
            }
            other => panic!("expected NotPermitted rejection, got {other:?}"),
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
        let tx = TransactionBuilder::new(chain.clone(), authority.clone())
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
        let tx = TransactionBuilder::new(chain, authority)
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
        let (authority, keypair) = gen_account_in("wonderland");
        let domain = Domain::new("wonderland".parse().unwrap()).build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let world = World::with([domain], [account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());

        let policy = LaneCompliancePolicy {
            id: LaneCompliancePolicyId::new(Hash::prehashed([0xAA; 32])),
            version: 1,
            lane_id: TestLaneId::SINGLE,
            dataspace_id: TestDataSpaceId::GLOBAL,
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

        let tx = TransactionBuilder::new(chain, authority.clone())
            .with_instructions([Log::new(Level::INFO, "noop".into())])
            .sign(keypair.private_key());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let stx = block.transaction();
        let assignment = super::LaneAssignment {
            lane_id: TestLaneId::SINGLE,
            dataspace_id: TestDataSpaceId::GLOBAL,
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
            dataspace: TestDataSpaceId::GLOBAL,
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
    pub static DOMAIN: LazyLock<DomainId> = LazyLock::new(|| DOMAIN_STR.parse().unwrap());
    /// Pre-parsed asset definition identifier for the sandbox asset.
    pub static ASSET: LazyLock<AssetDefinitionId> =
        LazyLock::new(|| format!("{ASSET_STR}#{DOMAIN_STR}").parse().unwrap());
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
                let id = AccountId::new(DOMAIN.clone(), signatory);
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
            assert_eq!(ACCOUNT[name].id.signatory().to_string(), *public);
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
            Transfer::asset_numeric(
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
            if collapse_to_unix_line_endings(text) == rendered {
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
            || format!("{}@{}", account.signatory(), account.domain()),
            |alias| format!("{alias}@{}", account.domain()),
        );
        if asset_id.definition().domain() == account.domain() {
            format!("{}##{}", asset_id.definition().name(), account_str)
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
                let asset_def = AssetDefinition::new(ASSET.clone(), NumericSpec::default())
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
            let chain: ChainId = "chain".parse().unwrap();
            let mut state = State::new_for_testing(world, kura, query_handle);
            state.chain_id = chain;
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
            self.with_data_trigger_transfer_numeric_internal(
                src,
                Numeric::from(quantity),
                dest,
                Repeats::Indefinitely,
                0,
            )
        }

        /// Add a single-use data trigger that fires at most once.
        #[must_use]
        pub fn with_data_trigger_transfer_once(self, src: &str, quantity: u32, dest: &str) -> Self {
            self.with_data_trigger_transfer_numeric_internal(
                src,
                Numeric::from(quantity),
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
            self.with_data_trigger_transfer_numeric_internal(
                src,
                Numeric::from(quantity),
                dest,
                Repeats::Indefinitely,
                label,
            )
        }

        /// Add a data trigger with an explicit [`Numeric`] amount.
        #[must_use]
        pub fn with_data_trigger_transfer_numeric(
            self,
            src: &str,
            amount: Numeric,
            dest: &str,
        ) -> Self {
            self.with_data_trigger_transfer_numeric_internal(
                src,
                amount,
                dest,
                Repeats::Indefinitely,
                0,
            )
        }

        fn with_data_trigger_transfer_numeric_internal(
            self,
            src: &str,
            amount: Numeric,
            dest: &str,
            repeats: Repeats,
            label: u32,
        ) -> Self {
            let mut block = self.state.world.triggers.block();
            let mut transaction = block.transaction();
            let trigger = Trigger::new(
                format!("data-{src}-{dest}-{label}").parse().unwrap(),
                Action::new(
                    [InstructionBox::from(Transfer::asset_numeric(
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
                TransactionBuilder::new(CHAIN_ID.clone(), GENESIS_ACCOUNT.id.clone())
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
            let prev_fifo = crate::pipeline::set_force_fifo_scheduler(true);
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
            crate::pipeline::set_force_fifo_scheduler(prev_fifo);

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
                    let balance = numeric_to_u64(&balance_num).unwrap_or_else(|error| {
                        panic!("account {name} has non-integer balance {balance_num}: {error:?}");
                    });
                    (*name, balance)
                })
                .collect();

            assert_eq!(actual, expected);
        }
    }
}

#[cfg(test)]
fn numeric_to_u64(n: &Numeric) -> Result<u64, iroha_primitives::TryFromNumericError> {
    let mantissa = n
        .try_mantissa_u128()
        .ok_or(iroha_primitives::TryFromNumericError)?;
    if n.scale() == 0 {
        return mantissa
            .try_into()
            .map_err(|_| iroha_primitives::TryFromNumericError);
    }

    let scale = 10u128
        .checked_pow(n.scale())
        .ok_or(iroha_primitives::TryFromNumericError)?;
    if mantissa % scale != 0 {
        return Err(iroha_primitives::TryFromNumericError);
    }
    mantissa
        .checked_div(scale)
        .ok_or(iroha_primitives::TryFromNumericError)?
        .try_into()
        .map_err(|_| iroha_primitives::TryFromNumericError)
}

#[cfg(test)]
mod numeric_to_u64_tests {
    use iroha_primitives::numeric::Numeric;

    use super::numeric_to_u64;

    #[test]
    fn accepts_scaled_whole_numbers() {
        let scaled = Numeric::try_new(120_i32, 1).expect("numeric");
        assert_eq!(numeric_to_u64(&scaled).unwrap(), 12);
    }

    #[test]
    fn rejects_fractional_balances() {
        let fractional = Numeric::try_new(1_i32, 1).expect("numeric");
        assert!(numeric_to_u64(&fractional).is_err());
    }

    #[test]
    fn rejects_values_outside_u64_range() {
        // Any value that cannot be represented as u64 should error.
        let large = Numeric::try_new(i128::MAX, 0).expect("numeric");
        assert!(numeric_to_u64(&large).is_err());
        let overflowing = Numeric::try_new(i128::MIN, 0).expect("numeric");
        assert!(numeric_to_u64(&overflowing).is_err());
    }
}
