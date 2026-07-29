//! Authenticated Torii boundary for the SoraFS proof-of-personhood service.
//!
//! This module never accepts plaintext enrollment, credential, issuance-draft,
//! holder-secret, or Merkle-witness material over HTTP. Those values are
//! supplied only by runtime-owned adapters and remain behind the authority and
//! durability checks in [`sorafs_node::pop_credentials`].

use std::{fmt, path::PathBuf, sync::Arc, time::Duration};

use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroha_config::parameters::{ProductionRuntimeHandleError, validate_production_runtime_handle};
use iroha_crypto::HybridSecretKey;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use rand::rngs::OsRng;
use sorafs_manifest::pop_credentials::{
    POP_IDENTITY_TEXT_MAX_BYTES_V1, POP_MEMBERSHIP_CONTEXT_MAX_BYTES_V1,
    POP_MEMBERSHIP_PROOF_MAX_BYTES_V1, PopMembershipProofV1, PopMembershipWitnessV1,
    PopRevocationListV1,
};
use sorafs_node::pop_credentials::{
    POP_API_AUTHENTICATION_MAX_BYTES_V1, POP_CREDENTIAL_SERVICE_POLICY_VERSION_V1,
    POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1, POP_WALLET_DELIVERY_MAX_BYTES_V1, PopApprovalSignerV1,
    PopApprovalV1, PopCredentialApiActionV1, PopCredentialApiAuthenticator, PopCredentialApiV1,
    PopCredentialService, PopCredentialServiceError, PopCredentialServicePolicyV1,
    PopEnrollmentStateV1, PopEnrollmentStatusV1, PopFinalizedRegistryProjectionV1,
    PopFinalizedRegistryReader, PopIssuanceDraftV1, PopIssuerHsm, PopOutboxSubmitOutcomeV1,
    PopRegistrySubmitter, PopWalletKeyWrapper, PopWalletVault,
};
use tokio::sync::Mutex;

use crate::{JsonBody, SharedAppState, utils::extractors::NoritoJson};

/// Dedicated credential header. The value is `PopV1 <base64url-no-pad>`.
pub const POP_AUTHORIZATION_HEADER_V1: &str = "sora-pop-authorization";
/// Maximum canonical approval or other small signed control payload.
pub const POP_CONTROL_PAYLOAD_MAX_BYTES_V1: usize = 256 * 1024;
/// Maximum JSON/Norito request envelope for small PoP control operations.
pub const POP_CONTROL_REQUEST_MAX_BYTES_V1: usize = 384 * 1024;
/// Maximum request envelope for encrypted enrollment submission.
pub const POP_ENROLLMENT_REQUEST_MAX_BYTES_V1: usize =
    canonical_base64_max_len(POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1) + 8 * 1024;
/// Maximum request envelope for membership proofs.
pub const POP_PROOF_REQUEST_MAX_BYTES_V1: usize =
    canonical_base64_max_len(POP_MEMBERSHIP_PROOF_MAX_BYTES_V1) + 64 * 1024;
/// Maximum response envelope for an encrypted wallet delivery.
pub const POP_WALLET_DELIVERY_RESPONSE_MAX_BYTES_V1: usize =
    canonical_base64_max_len(POP_WALLET_DELIVERY_MAX_BYTES_V1) + 8 * 1024;
const POP_CANONICAL_DECODE_MAX_DEPTH_V1: usize = 64;
const POP_ISSUE_TRIGGER_BINDING_DOMAIN_V1: &[u8] = b"sorafs.pop.issue-trigger.v1";
const POP_WALLET_WITNESS_SYNC_BINDING_DOMAIN_V1: &[u8] = b"sorafs.pop.wallet-witness-sync-api.v1";

const fn canonical_base64_max_len(decoded_len: usize) -> usize {
    decoded_len.div_ceil(3).saturating_mul(4)
}

/// Drop guard for decoded opaque authorization material.
///
/// This type is deliberately neither cloneable nor serializable. Every exit
/// path overwrites the owned credential bytes before releasing the allocation.
struct PopApiCredentialV1 {
    bytes: Vec<u8>,
    #[cfg(test)]
    drop_probe: Option<Arc<std::sync::Mutex<Vec<u8>>>>,
}

impl PopApiCredentialV1 {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            #[cfg(test)]
            drop_probe: None,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for PopApiCredentialV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PopApiCredentialV1([REDACTED])")
    }
}

impl Drop for PopApiCredentialV1 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        std::hint::black_box(self.bytes.as_slice());
        #[cfg(test)]
        if let Some(probe) = &self.drop_probe
            && let Ok(mut observed) = probe.lock()
        {
            observed.clone_from(&self.bytes);
        }
    }
}

/// Non-secret, config-backed runtime settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopCredentialRuntimeConfigV1 {
    /// Exact finalized service policy.
    pub service_policy: PopCredentialServicePolicyV1,
    /// Durable issuer state directory.
    pub issuer_state_dir: PathBuf,
    /// Encrypted wallet-vault directory.
    pub wallet_state_dir: PathBuf,
    /// Bounded submission/reconciliation worker cadence.
    pub worker_interval: Duration,
    /// Maximum absolute skew between finalized and runtime clock time.
    pub max_finalized_time_skew: Duration,
    /// Exact non-secret wallet wrapping-key handle.
    pub wallet_wrapping_key_id: String,
    /// Exact non-secret deployment runtime-provider registry handle.
    pub runtime_provider_registry_handle: String,
    /// Exact non-zero deployment registry policy revision.
    pub runtime_provider_registry_revision: u64,
    /// Exact deployment registry policy digest.
    pub runtime_provider_registry_policy_digest: [u8; 32],
}

impl PopCredentialRuntimeConfigV1 {
    /// Validate configuration before any runtime secret is consumed.
    pub fn validate(&self) -> Result<(), PopCredentialServiceError> {
        self.service_policy.validate()?;
        if self.issuer_state_dir.as_os_str().is_empty()
            || self.wallet_state_dir.as_os_str().is_empty()
            || self.issuer_state_dir == self.wallet_state_dir
            || self.worker_interval.is_zero()
            || self.worker_interval > Duration::from_secs(60 * 60)
            || self.max_finalized_time_skew > Duration::from_secs(5 * 60)
            || self.max_finalized_time_skew.subsec_nanos() != 0
            || self.runtime_provider_registry_revision == 0
            || self.runtime_provider_registry_policy_digest == [0; 32]
        {
            return Err(PopCredentialServiceError::InvalidInput {
                field: "pop_runtime_config",
            });
        }
        validate_pop_runtime_provider_handle(
            &self.runtime_provider_registry_handle,
            "runtime_provider_registry_handle",
        )?;
        validate_pop_runtime_provider_handle(
            &self.wallet_wrapping_key_id,
            "wallet_wrapping_key_id",
        )?;
        Ok(())
    }
}

impl From<&iroha_config::parameters::actual::SorafsPopCredentialService>
    for PopCredentialRuntimeConfigV1
{
    fn from(value: &iroha_config::parameters::actual::SorafsPopCredentialService) -> Self {
        Self {
            service_policy: PopCredentialServicePolicyV1 {
                version: POP_CREDENTIAL_SERVICE_POLICY_VERSION_V1,
                issuer_policy_digest: value.issuer_policy_digest,
                issuer_id: value.issuer_id.clone(),
                issuer_hsm_key_id: value.issuer_hsm_key_id.clone(),
                issuer_public_key: value.issuer_public_key,
                enrollment_recipient_key_id: value.enrollment_recipient_key_id.clone(),
                approval_quorum: value.approval_quorum,
                approval_signers: value
                    .approval_signers
                    .iter()
                    .map(|signer| PopApprovalSignerV1 {
                        signer_id: signer.signer_id.clone(),
                        public_key: signer.public_key,
                        revoked_at_epoch: signer.revoked_at_epoch,
                    })
                    .collect(),
                max_pending_enrollments: value.max_pending_enrollments,
                max_outbox_entries: value.max_outbox_entries,
                max_dead_letters: value.max_dead_letters,
                max_seen_nullifiers: value.max_seen_nullifiers,
                max_submission_attempts: value.max_submission_attempts,
            },
            issuer_state_dir: value.issuer_state_dir.clone(),
            wallet_state_dir: value.wallet_state_dir.clone(),
            worker_interval: value.worker_interval,
            max_finalized_time_skew: value.max_finalized_time_skew,
            wallet_wrapping_key_id: value.wallet_wrapping_key_id.clone(),
            runtime_provider_registry_handle: value.runtime_provider_registry_handle.clone(),
            runtime_provider_registry_revision: value.runtime_provider_registry_revision,
            runtime_provider_registry_policy_digest: value.runtime_provider_registry_policy_digest,
        }
    }
}

fn validate_pop_runtime_provider_handle(
    value: &str,
    field: &'static str,
) -> Result<(), PopCredentialServiceError> {
    validate_production_runtime_handle(value).map_err(|error| match error {
        ProductionRuntimeHandleError::InvalidSyntax | ProductionRuntimeHandleError::TestMarked => {
            PopCredentialServiceError::InvalidInput { field }
        }
    })
}

/// Exact public bindings supplied to a deployment-owned PoP provider registry.
///
/// The request contains stable handles and finalized public policy identity
/// only. Durable paths, credentials, recipient secrets, witnesses,
/// attestations, wallet material, and PII are deliberately excluded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopCredentialRuntimeProviderBindingsV1 {
    issuer_policy_digest: [u8; 32],
    issuer_id: String,
    issuer_hsm_key_id: String,
    issuer_public_key: [u8; 32],
    enrollment_recipient_key_id: String,
    wallet_wrapping_key_id: String,
}

impl PopCredentialRuntimeProviderBindingsV1 {
    /// Construct exact, validated public bindings for a deployment registry.
    ///
    /// Every V1 field is required by this typed constructor. Runtime handles
    /// must use canonical production syntax and may not be test-marked.
    pub fn try_new(
        issuer_policy_digest: [u8; 32],
        issuer_id: String,
        issuer_hsm_key_id: String,
        issuer_public_key: [u8; 32],
        enrollment_recipient_key_id: String,
        wallet_wrapping_key_id: String,
    ) -> Result<Self, PopCredentialServiceError> {
        if issuer_policy_digest == [0; 32] {
            return Err(PopCredentialServiceError::InvalidInput {
                field: "issuer_policy_digest",
            });
        }
        if issuer_id.is_empty()
            || issuer_id != issuer_id.trim()
            || issuer_id.len() > POP_IDENTITY_TEXT_MAX_BYTES_V1
            || issuer_id.chars().any(char::is_control)
        {
            return Err(PopCredentialServiceError::InvalidInput { field: "issuer_id" });
        }
        validate_pop_runtime_provider_handle(&issuer_hsm_key_id, "issuer_hsm_key_id")?;
        validate_pop_runtime_provider_handle(
            &enrollment_recipient_key_id,
            "enrollment_recipient_key_id",
        )?;
        validate_pop_runtime_provider_handle(&wallet_wrapping_key_id, "wallet_wrapping_key_id")?;
        if issuer_public_key == [0; 32]
            || iroha_crypto::ed25519_parse_public_key(&issuer_public_key).is_err()
        {
            return Err(PopCredentialServiceError::InvalidInput {
                field: "issuer_public_key",
            });
        }
        Ok(Self {
            issuer_policy_digest,
            issuer_id,
            issuer_hsm_key_id,
            issuer_public_key,
            enrollment_recipient_key_id,
            wallet_wrapping_key_id,
        })
    }

    fn from_config(
        config: &PopCredentialRuntimeConfigV1,
    ) -> Result<Self, PopCredentialServiceError> {
        Self::try_new(
            config.service_policy.issuer_policy_digest,
            config.service_policy.issuer_id.clone(),
            config.service_policy.issuer_hsm_key_id.clone(),
            config.service_policy.issuer_public_key,
            config.service_policy.enrollment_recipient_key_id.clone(),
            config.wallet_wrapping_key_id.clone(),
        )
    }

    /// Exact active finalized issuer-policy digest.
    #[must_use]
    pub const fn issuer_policy_digest(&self) -> [u8; 32] {
        self.issuer_policy_digest
    }

    /// Exact governed public issuer identity.
    #[must_use]
    pub fn issuer_id(&self) -> &str {
        &self.issuer_id
    }

    /// Exact non-secret HSM key handle.
    #[must_use]
    pub fn issuer_hsm_key_id(&self) -> &str {
        &self.issuer_hsm_key_id
    }

    /// Exact governed issuer public key.
    #[must_use]
    pub const fn issuer_public_key(&self) -> [u8; 32] {
        self.issuer_public_key
    }

    /// Exact non-secret encrypted-enrollment recipient handle.
    #[must_use]
    pub fn enrollment_recipient_key_id(&self) -> &str {
        &self.enrollment_recipient_key_id
    }

    /// Exact non-secret wallet wrapping-key handle.
    #[must_use]
    pub fn wallet_wrapping_key_id(&self) -> &str {
        &self.wallet_wrapping_key_id
    }
}

/// Public identity of one deployment-owned PoP provider-registry policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopCredentialRuntimeProviderQualificationV1 {
    /// Non-zero external provider policy revision.
    pub revision: u64,
    /// Non-zero digest of the exact external provider policy.
    pub policy_digest: [u8; 32],
}

impl PopCredentialRuntimeProviderQualificationV1 {
    /// Construct one public provider qualification.
    #[must_use]
    pub const fn new(revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            revision,
            policy_digest,
        }
    }

    fn is_valid(self) -> bool {
        self.revision != 0 && self.policy_digest != [0; 32]
    }
}

/// Stable redacted failure returned by a deployment provider registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopCredentialRuntimeProviderRegistryErrorV1 {
    /// The provider control plane is unavailable.
    Unavailable,
    /// The provider policy is stale or revoked.
    StaleOrRevoked,
    /// The registry rejected the exact configured public bindings.
    RejectedBindings,
}

/// Deployment-owned factory for all PoP private runtime dependencies.
///
/// Implementations must change `qualification` whenever any HSM, KMS,
/// authentication, enrollment-recipient, wallet, witness, finalized-query, or
/// transaction adapter identity/policy changes. `resolve` receives only public
/// bindings and must never persist or log private material.
pub trait PopCredentialRuntimeProviderRegistryV1: Send + Sync + fmt::Debug {
    /// Exact stable non-secret registry handle.
    fn handle(&self) -> &str;

    /// Current independently administered public policy qualification.
    fn qualification(
        &self,
    ) -> Result<
        PopCredentialRuntimeProviderQualificationV1,
        PopCredentialRuntimeProviderRegistryErrorV1,
    >;

    /// Resolve one coherent runtime provider set for the exact public bindings.
    fn resolve(
        &self,
        bindings: &PopCredentialRuntimeProviderBindingsV1,
    ) -> Result<PopCredentialRuntimeSecretsV1, PopCredentialRuntimeProviderRegistryErrorV1>;
}

#[derive(Clone)]
struct QualifiedPopCredentialRuntimeProviderRegistryV1 {
    handle: String,
    qualification: PopCredentialRuntimeProviderQualificationV1,
    registry: Arc<dyn PopCredentialRuntimeProviderRegistryV1>,
}

impl fmt::Debug for QualifiedPopCredentialRuntimeProviderRegistryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedPopCredentialRuntimeProviderRegistryV1")
            .field("handle", &self.handle)
            .field("qualification", &self.qualification)
            .finish_non_exhaustive()
    }
}

impl QualifiedPopCredentialRuntimeProviderRegistryV1 {
    fn try_new(
        config: &PopCredentialRuntimeConfigV1,
        registry: Option<Arc<dyn PopCredentialRuntimeProviderRegistryV1>>,
    ) -> Result<Self, PopCredentialServiceError> {
        let registry = registry.ok_or(PopCredentialServiceError::RuntimeProviderRegistryMissing)?;
        validate_pop_runtime_provider_handle(registry.handle(), "runtime_provider_registry_handle")
            .map_err(|_| PopCredentialServiceError::RuntimeProviderRegistryMismatch)?;
        if registry.handle() != config.runtime_provider_registry_handle {
            return Err(PopCredentialServiceError::RuntimeProviderRegistryMismatch);
        }
        let qualification = registry
            .qualification()
            .map_err(|_| PopCredentialServiceError::RuntimeProviderRegistryUnavailable)?;
        let expected = PopCredentialRuntimeProviderQualificationV1::new(
            config.runtime_provider_registry_revision,
            config.runtime_provider_registry_policy_digest,
        );
        if !qualification.is_valid() || qualification != expected {
            return Err(PopCredentialServiceError::RuntimeProviderRegistryMismatch);
        }
        let rechecked_qualification = registry
            .qualification()
            .map_err(|_| PopCredentialServiceError::RuntimeProviderRegistryUnavailable)?;
        if registry.handle() != config.runtime_provider_registry_handle
            || rechecked_qualification != qualification
        {
            return Err(PopCredentialServiceError::RuntimeProviderRegistryDrift);
        }
        Ok(Self {
            handle: config.runtime_provider_registry_handle.clone(),
            qualification,
            registry,
        })
    }

    fn assert_qualification(&self) -> Result<(), PopCredentialServiceError> {
        let qualification = self
            .registry
            .qualification()
            .map_err(|_| PopCredentialServiceError::RuntimeProviderRegistryUnavailable)?;
        if self.registry.handle() != self.handle || qualification != self.qualification {
            return Err(PopCredentialServiceError::RuntimeProviderRegistryDrift);
        }
        Ok(())
    }

    fn resolve(
        &self,
        bindings: &PopCredentialRuntimeProviderBindingsV1,
    ) -> Result<PopCredentialRuntimeSecretsV1, PopCredentialServiceError> {
        self.assert_qualification()?;
        let result = self
            .registry
            .resolve(bindings)
            .map_err(|_| PopCredentialServiceError::RuntimeProviderRegistryUnavailable);
        self.assert_qualification()?;
        result
    }

    fn finish<T>(
        &self,
        result: Result<T, PopCredentialServiceError>,
    ) -> Result<T, PopCredentialServiceError> {
        self.assert_qualification()?;
        result
    }
}

/// Stable runtime-provider failure. Provider details are deliberately absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopPrivateMaterialProviderErrorV1 {
    /// The runtime provider could not produce current governed material.
    Unavailable,
}

/// Stable failure returned by the runtime-only finalized-time provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopFinalizedTimeProviderErrorV1 {
    /// No authoritative finalized time sample is currently available.
    Unavailable,
}

/// One authoritative finalized-chain time sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopFinalizedTimeSampleV1 {
    /// Finalized block height that anchors this sample.
    pub finalized_block_height: u64,
    /// Non-zero finalized block hash.
    pub finalized_block_hash: [u8; 32],
    /// Unix epoch seconds derived from the finalized block timestamp.
    pub finalized_epoch: u64,
    /// Independent Unix epoch observation used only for the skew bound.
    pub observed_epoch: u64,
}

/// Runtime-only provider for finalized-chain time and an independent clock
/// observation. Implementations must source the finalized fields from
/// committed state; no file or environment fallback is permitted.
pub trait PopFinalizedTimeProviderV1: Send + Sync + fmt::Debug {
    /// Return the current authoritative bounded sample.
    fn sample(&self) -> Result<PopFinalizedTimeSampleV1, PopFinalizedTimeProviderErrorV1>;
}

/// Runtime-only provider for private issuance material.
pub trait PopIssuanceDraftProviderV1: Send + Sync + fmt::Debug {
    /// Resolve a draft for one approved public request identifier.
    fn resolve(
        &self,
        request_id: [u8; 32],
        now_epoch: u64,
    ) -> Result<PopIssuanceDraftV1, PopPrivateMaterialProviderErrorV1>;
}

/// Runtime-only provider for a wallet's private updated membership witness.
pub trait PopWalletWitnessProviderV1: Send + Sync + fmt::Debug {
    /// Resolve a private witness for the exact current public projection.
    fn resolve(
        &self,
        credential_commitment: [u8; 32],
        projection: &PopFinalizedRegistryProjectionV1,
    ) -> Result<PopMembershipWitnessV1, PopPrivateMaterialProviderErrorV1>;
}

/// Runtime-only dependencies. No constructor reads keys from config, files, or
/// environment variables.
pub struct PopCredentialRuntimeSecretsV1 {
    /// Hybrid recipient secret for encrypted enrollment.
    pub enrollment_recipient_secret: HybridSecretKey,
    /// HSM/PKCS#11 issuer signer.
    pub issuer_hsm: Arc<dyn PopIssuerHsm>,
    /// Action- and request-bound API authenticator.
    pub authenticator: Arc<dyn PopCredentialApiAuthenticator>,
    /// Idempotent ledger transaction submitter.
    pub registry_submitter: Arc<dyn PopRegistrySubmitter>,
    /// Finalized ledger projection reader.
    pub registry_reader: Arc<dyn PopFinalizedRegistryReader>,
    /// Private issuance material provider.
    pub issuance_draft_provider: Arc<dyn PopIssuanceDraftProviderV1>,
    /// Hybrid wallet recipient secret.
    pub wallet_recipient_secret: HybridSecretKey,
    /// KMS/PKCS#11 wallet DEK wrapper.
    pub wallet_key_wrapper: Arc<dyn PopWalletKeyWrapper>,
    /// Private wallet witness provider.
    pub wallet_witness_provider: Arc<dyn PopWalletWitnessProviderV1>,
    /// Finalized-chain time and independent clock provider.
    pub finalized_time_provider: Arc<dyn PopFinalizedTimeProviderV1>,
}

impl fmt::Debug for PopCredentialRuntimeSecretsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PopCredentialRuntimeSecretsV1([REDACTED])")
    }
}

const POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1: &str = "PoP runtime provider unavailable";

#[derive(Clone)]
struct QualifiedPopIssuerHsmV1 {
    inner: Arc<dyn PopIssuerHsm>,
    key_id: String,
    public_key: [u8; 32],
    registry: QualifiedPopCredentialRuntimeProviderRegistryV1,
}

impl fmt::Debug for QualifiedPopIssuerHsmV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedPopIssuerHsmV1")
            .field("key_id", &self.key_id)
            .field("private_provider", &"[REDACTED]")
            .finish()
    }
}

impl QualifiedPopIssuerHsmV1 {
    fn identity_matches(&self) -> bool {
        self.inner.key_id() == self.key_id && self.inner.public_key() == self.public_key
    }
}

impl PopIssuerHsm for QualifiedPopIssuerHsmV1 {
    fn key_id(&self) -> &str {
        &self.key_id
    }

    fn public_key(&self) -> [u8; 32] {
        self.public_key
    }

    fn sign_digest(&self, digest: [u8; 32]) -> Result<[u8; 64], String> {
        self.registry
            .assert_qualification()
            .map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())?;
        if !self.identity_matches() {
            return Err(POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned());
        }
        let result = self.inner.sign_digest(digest);
        if !self.identity_matches() {
            return Err(POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned());
        }
        self.registry
            .assert_qualification()
            .map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())?;
        result.map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())
    }
}

#[derive(Clone)]
struct QualifiedPopWalletKeyWrapperV1 {
    inner: Arc<dyn PopWalletKeyWrapper>,
    active_key_id: String,
    registry: QualifiedPopCredentialRuntimeProviderRegistryV1,
}

impl fmt::Debug for QualifiedPopWalletKeyWrapperV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedPopWalletKeyWrapperV1")
            .field("active_key_id", &self.active_key_id)
            .field("private_provider", &"[REDACTED]")
            .finish()
    }
}

impl QualifiedPopWalletKeyWrapperV1 {
    fn identity_matches(&self) -> bool {
        self.inner.active_key_id() == self.active_key_id
    }

    fn guard<T>(&self, operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        self.registry
            .assert_qualification()
            .map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())?;
        if !self.identity_matches() {
            return Err(POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned());
        }
        let result = operation();
        if !self.identity_matches() {
            return Err(POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned());
        }
        self.registry
            .assert_qualification()
            .map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())?;
        result.map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())
    }
}

impl PopWalletKeyWrapper for QualifiedPopWalletKeyWrapperV1 {
    fn active_key_id(&self) -> &str {
        &self.active_key_id
    }

    fn wrap_dek(&self, context: [u8; 32], dek: &[u8; 32]) -> Result<Vec<u8>, String> {
        self.guard(|| self.inner.wrap_dek(context, dek))
    }

    fn unwrap_dek(
        &self,
        key_id: &str,
        context: [u8; 32],
        wrapped_dek: &[u8],
    ) -> Result<[u8; 32], String> {
        self.guard(|| self.inner.unwrap_dek(key_id, context, wrapped_dek))
    }
}

#[derive(Clone)]
struct QualifiedPopCredentialApiAuthenticatorV1 {
    inner: Arc<dyn PopCredentialApiAuthenticator>,
    registry: QualifiedPopCredentialRuntimeProviderRegistryV1,
}

impl fmt::Debug for QualifiedPopCredentialApiAuthenticatorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QualifiedPopCredentialApiAuthenticatorV1([REDACTED])")
    }
}

impl PopCredentialApiAuthenticator for QualifiedPopCredentialApiAuthenticatorV1 {
    fn authenticate(
        &self,
        opaque_credential: &[u8],
        action: PopCredentialApiActionV1,
        request_binding: [u8; 32],
        now_epoch: u64,
    ) -> Result<sorafs_node::pop_credentials::PopAuthenticatedPrincipalV1, String> {
        self.registry
            .assert_qualification()
            .map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())?;
        let result = self
            .inner
            .authenticate(opaque_credential, action, request_binding, now_epoch);
        self.registry
            .assert_qualification()
            .map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())?;
        result.map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())
    }
}

#[derive(Clone)]
struct QualifiedPopRegistrySubmitterV1 {
    inner: Arc<dyn PopRegistrySubmitter>,
    registry: QualifiedPopCredentialRuntimeProviderRegistryV1,
}

impl fmt::Debug for QualifiedPopRegistrySubmitterV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QualifiedPopRegistrySubmitterV1([REDACTED])")
    }
}

impl PopRegistrySubmitter for QualifiedPopRegistrySubmitterV1 {
    fn submit(
        &self,
        idempotency_key: [u8; 32],
        operation: &sorafs_node::pop_credentials::PopRegistryOperationV1,
    ) -> Result<(), String> {
        self.registry
            .assert_qualification()
            .map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())?;
        let result = self.inner.submit(idempotency_key, operation);
        self.registry
            .assert_qualification()
            .map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())?;
        result.map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())
    }
}

#[derive(Clone)]
struct QualifiedPopFinalizedRegistryReaderV1 {
    inner: Arc<dyn PopFinalizedRegistryReader>,
    registry: QualifiedPopCredentialRuntimeProviderRegistryV1,
}

impl fmt::Debug for QualifiedPopFinalizedRegistryReaderV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QualifiedPopFinalizedRegistryReaderV1([REDACTED])")
    }
}

impl PopFinalizedRegistryReader for QualifiedPopFinalizedRegistryReaderV1 {
    fn next_after(
        &self,
        cursor: Option<sorafs_node::pop_credentials::PopFinalizedCursorV1>,
    ) -> Result<Option<PopFinalizedRegistryProjectionV1>, String> {
        self.registry
            .assert_qualification()
            .map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())?;
        let result = self.inner.next_after(cursor);
        self.registry
            .assert_qualification()
            .map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())?;
        result.map_err(|_| POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())
    }
}

#[derive(Clone)]
struct QualifiedPopIssuanceDraftProviderV1 {
    inner: Arc<dyn PopIssuanceDraftProviderV1>,
    registry: QualifiedPopCredentialRuntimeProviderRegistryV1,
}

impl fmt::Debug for QualifiedPopIssuanceDraftProviderV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QualifiedPopIssuanceDraftProviderV1([REDACTED])")
    }
}

impl PopIssuanceDraftProviderV1 for QualifiedPopIssuanceDraftProviderV1 {
    fn resolve(
        &self,
        request_id: [u8; 32],
        now_epoch: u64,
    ) -> Result<PopIssuanceDraftV1, PopPrivateMaterialProviderErrorV1> {
        self.registry
            .assert_qualification()
            .map_err(|_| PopPrivateMaterialProviderErrorV1::Unavailable)?;
        let result = self.inner.resolve(request_id, now_epoch);
        self.registry
            .assert_qualification()
            .map_err(|_| PopPrivateMaterialProviderErrorV1::Unavailable)?;
        result.map_err(|_| PopPrivateMaterialProviderErrorV1::Unavailable)
    }
}

#[derive(Clone)]
struct QualifiedPopWalletWitnessProviderV1 {
    inner: Arc<dyn PopWalletWitnessProviderV1>,
    registry: QualifiedPopCredentialRuntimeProviderRegistryV1,
}

impl fmt::Debug for QualifiedPopWalletWitnessProviderV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QualifiedPopWalletWitnessProviderV1([REDACTED])")
    }
}

impl PopWalletWitnessProviderV1 for QualifiedPopWalletWitnessProviderV1 {
    fn resolve(
        &self,
        credential_commitment: [u8; 32],
        projection: &PopFinalizedRegistryProjectionV1,
    ) -> Result<PopMembershipWitnessV1, PopPrivateMaterialProviderErrorV1> {
        self.registry
            .assert_qualification()
            .map_err(|_| PopPrivateMaterialProviderErrorV1::Unavailable)?;
        let result = self.inner.resolve(credential_commitment, projection);
        self.registry
            .assert_qualification()
            .map_err(|_| PopPrivateMaterialProviderErrorV1::Unavailable)?;
        result.map_err(|_| PopPrivateMaterialProviderErrorV1::Unavailable)
    }
}

#[derive(Clone)]
struct QualifiedPopFinalizedTimeProviderV1 {
    inner: Arc<dyn PopFinalizedTimeProviderV1>,
    registry: QualifiedPopCredentialRuntimeProviderRegistryV1,
}

impl fmt::Debug for QualifiedPopFinalizedTimeProviderV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QualifiedPopFinalizedTimeProviderV1([REDACTED])")
    }
}

impl PopFinalizedTimeProviderV1 for QualifiedPopFinalizedTimeProviderV1 {
    fn sample(&self) -> Result<PopFinalizedTimeSampleV1, PopFinalizedTimeProviderErrorV1> {
        self.registry
            .assert_qualification()
            .map_err(|_| PopFinalizedTimeProviderErrorV1::Unavailable)?;
        let result = self.inner.sample();
        self.registry
            .assert_qualification()
            .map_err(|_| PopFinalizedTimeProviderErrorV1::Unavailable)?;
        result.map_err(|_| PopFinalizedTimeProviderErrorV1::Unavailable)
    }
}

/// Torii-owned PoP issuer, registry reconciler, wallet, and verifier runtime.
pub struct PopCredentialToriiRuntimeV1 {
    config: PopCredentialRuntimeConfigV1,
    provider_registry: QualifiedPopCredentialRuntimeProviderRegistryV1,
    api: PopCredentialApiV1,
    authenticator: Arc<dyn PopCredentialApiAuthenticator>,
    service: Mutex<PopCredentialService>,
    registry_submitter: Arc<dyn PopRegistrySubmitter>,
    registry_reader: Arc<dyn PopFinalizedRegistryReader>,
    issuance_draft_provider: Arc<dyn PopIssuanceDraftProviderV1>,
    wallet: PopWalletVault,
    wallet_recipient_secret: HybridSecretKey,
    wallet_witness_provider: Arc<dyn PopWalletWitnessProviderV1>,
    finalized_time_provider: Arc<dyn PopFinalizedTimeProviderV1>,
    accepted_finalized_time: std::sync::Mutex<Option<PopFinalizedTimeSampleV1>>,
}

impl fmt::Debug for PopCredentialToriiRuntimeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PopCredentialToriiRuntimeV1")
            .field("issuer_id", &self.config.service_policy.issuer_id)
            .field("issuer_state_dir", &self.config.issuer_state_dir)
            .field("wallet_state_dir", &self.config.wallet_state_dir)
            .field("runtime_secrets", &"[REDACTED]")
            .finish()
    }
}

impl PopCredentialToriiRuntimeV1 {
    /// Construct the runtime through an explicit deployment provider registry.
    ///
    /// Registry identity, revision, policy digest, and resolved public HSM
    /// identity are checked before issuer or wallet state is opened.
    pub fn open(
        config: PopCredentialRuntimeConfigV1,
        registry: Option<Arc<dyn PopCredentialRuntimeProviderRegistryV1>>,
    ) -> Result<Self, PopCredentialServiceError> {
        config.validate()?;
        let provider_registry =
            QualifiedPopCredentialRuntimeProviderRegistryV1::try_new(&config, registry)?;
        let bindings = PopCredentialRuntimeProviderBindingsV1::from_config(&config)?;
        let secrets = provider_registry.resolve(&bindings)?;
        provider_registry.assert_qualification()?;
        validate_pop_runtime_provider_handle(secrets.issuer_hsm.key_id(), "issuer_hsm_key_id")
            .map_err(|_| PopCredentialServiceError::RuntimeProviderRegistryMismatch)?;
        validate_pop_runtime_provider_handle(
            secrets.wallet_key_wrapper.active_key_id(),
            "wallet_wrapping_key_id",
        )
        .map_err(|_| PopCredentialServiceError::RuntimeProviderRegistryMismatch)?;
        if secrets.issuer_hsm.key_id() != config.service_policy.issuer_hsm_key_id
            || secrets.issuer_hsm.public_key() != config.service_policy.issuer_public_key
        {
            return Err(PopCredentialServiceError::HsmPolicyMismatch);
        }
        if secrets.wallet_key_wrapper.active_key_id() != config.wallet_wrapping_key_id {
            return Err(PopCredentialServiceError::RuntimeProviderRegistryMismatch);
        }
        let issuer_hsm: Arc<dyn PopIssuerHsm> = Arc::new(QualifiedPopIssuerHsmV1 {
            inner: secrets.issuer_hsm,
            key_id: config.service_policy.issuer_hsm_key_id.clone(),
            public_key: config.service_policy.issuer_public_key,
            registry: provider_registry.clone(),
        });
        let wallet_key_wrapper: Arc<dyn PopWalletKeyWrapper> =
            Arc::new(QualifiedPopWalletKeyWrapperV1 {
                inner: secrets.wallet_key_wrapper,
                active_key_id: config.wallet_wrapping_key_id.clone(),
                registry: provider_registry.clone(),
            });
        let authenticator: Arc<dyn PopCredentialApiAuthenticator> =
            Arc::new(QualifiedPopCredentialApiAuthenticatorV1 {
                inner: secrets.authenticator,
                registry: provider_registry.clone(),
            });
        let registry_submitter: Arc<dyn PopRegistrySubmitter> =
            Arc::new(QualifiedPopRegistrySubmitterV1 {
                inner: secrets.registry_submitter,
                registry: provider_registry.clone(),
            });
        let registry_reader: Arc<dyn PopFinalizedRegistryReader> =
            Arc::new(QualifiedPopFinalizedRegistryReaderV1 {
                inner: secrets.registry_reader,
                registry: provider_registry.clone(),
            });
        let issuance_draft_provider: Arc<dyn PopIssuanceDraftProviderV1> =
            Arc::new(QualifiedPopIssuanceDraftProviderV1 {
                inner: secrets.issuance_draft_provider,
                registry: provider_registry.clone(),
            });
        let wallet_witness_provider: Arc<dyn PopWalletWitnessProviderV1> =
            Arc::new(QualifiedPopWalletWitnessProviderV1 {
                inner: secrets.wallet_witness_provider,
                registry: provider_registry.clone(),
            });
        let finalized_time_provider: Arc<dyn PopFinalizedTimeProviderV1> =
            Arc::new(QualifiedPopFinalizedTimeProviderV1 {
                inner: secrets.finalized_time_provider,
                registry: provider_registry.clone(),
            });
        provider_registry.assert_qualification()?;
        let service = PopCredentialService::open(
            &config.issuer_state_dir,
            config.service_policy.clone(),
            secrets.enrollment_recipient_secret,
            issuer_hsm,
        )?;
        if service.policy() != &config.service_policy {
            return Err(PopCredentialServiceError::WrongPolicy);
        }
        provider_registry.assert_qualification()?;
        let wallet = PopWalletVault::open(&config.wallet_state_dir, wallet_key_wrapper)?;
        provider_registry.assert_qualification()?;
        Ok(Self {
            config,
            provider_registry,
            api: PopCredentialApiV1::new(Arc::clone(&authenticator)),
            authenticator,
            service: Mutex::new(service),
            registry_submitter,
            registry_reader,
            issuance_draft_provider,
            wallet,
            wallet_recipient_secret: secrets.wallet_recipient_secret,
            wallet_witness_provider,
            finalized_time_provider,
            accepted_finalized_time: std::sync::Mutex::new(None),
        })
    }

    /// Exact non-secret config used to construct this runtime.
    #[must_use]
    pub fn config(&self) -> &PopCredentialRuntimeConfigV1 {
        &self.config
    }

    fn current_epoch(&self) -> Result<u64, PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = (|| {
            let mut accepted = self
                .accepted_finalized_time
                .lock()
                .map_err(|_| PopCredentialServiceError::RuntimeProviderUnavailable)?;
            let sample = self
                .finalized_time_provider
                .sample()
                .map_err(|_| PopCredentialServiceError::RuntimeProviderUnavailable)?;
            validate_finalized_time_sample(
                accepted.as_ref(),
                &sample,
                self.config.max_finalized_time_skew.as_secs(),
            )
            .map_err(|_| PopCredentialServiceError::RuntimeProviderUnavailable)?;
            self.provider_registry.assert_qualification()?;
            *accepted = Some(sample);
            Ok(sample.finalized_epoch)
        })();
        self.provider_registry.finish(result)
    }

    /// Run bounded retry-safe submission and finalized-chain reconciliation.
    pub fn spawn(self: Arc<Self>, shutdown: iroha_futures::supervisor::ShutdownSignal) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(self.config.worker_interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = shutdown.receive() => break,
                    _ = ticker.tick() => {
                        if self.provider_registry.assert_qualification().is_err() {
                            iroha_logger::warn!(
                                "SoraFS PoP runtime provider qualification failed before worker access"
                            );
                            continue;
                        }
                        let mut service = self.service.lock().await;
                        let now_epoch = match self.current_epoch() {
                            Ok(now) => now,
                            Err(_) => continue,
                        };
                        if service
                            .submit_next(self.registry_submitter.as_ref(), now_epoch)
                            .is_err()
                        {
                            iroha_logger::warn!(
                                "SoraFS PoP registry submission step failed; retained durable state"
                            );
                        }
                        if service
                            .reconcile_next(self.registry_reader.as_ref(), now_epoch)
                            .is_err()
                        {
                            iroha_logger::warn!(
                                "SoraFS PoP finalized-registry reconciliation step failed"
                            );
                        }
                        drop(service);
                        if self.provider_registry.assert_qualification().is_err() {
                            iroha_logger::warn!(
                                "SoraFS PoP runtime provider qualification changed during worker access"
                            );
                        }
                    }
                }
            }
        });
    }

    async fn submit_enrollment(
        &self,
        credential: &[u8],
        canonical_enrollment: &[u8],
    ) -> Result<PopEnrollmentStatusV1, PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let mut service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            self.api
                .submit_enrollment(&mut *service, credential, canonical_enrollment, now_epoch)
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn enrollment_status(
        &self,
        credential: &[u8],
        request_id: [u8; 32],
    ) -> Result<PopEnrollmentStatusV1, PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            self.api
                .enrollment_status(&service, credential, request_id, now_epoch)
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn record_approval(
        &self,
        credential: &[u8],
        approval: PopApprovalV1,
    ) -> Result<PopEnrollmentStatusV1, PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let mut service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            self.api
                .record_approval(&mut *service, credential, approval, now_epoch)
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn issue(
        &self,
        credential: &[u8],
        request_id: [u8; 32],
    ) -> Result<[u8; 32], PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let mut service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            authorize_private_provider_access(
                self.authenticator.as_ref(),
                credential,
                PopCredentialApiActionV1::TriggerCredentialIssuance,
                pop_digest_domain(POP_ISSUE_TRIGGER_BINDING_DOMAIN_V1, &request_id),
                now_epoch,
            )?;
            let draft = self
                .issuance_draft_provider
                .resolve(request_id, now_epoch)
                .map_err(|_| PopCredentialServiceError::RuntimeProviderUnavailable)?;
            if draft.request_id != request_id {
                return Err(PopCredentialServiceError::InvalidIssuance);
            }
            service.issue(draft, now_epoch, &mut OsRng)
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn enqueue_revocation(
        &self,
        credential: &[u8],
        revocations: PopRevocationListV1,
    ) -> Result<[u8; 32], PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let mut service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            self.api
                .enqueue_revocation(&mut *service, credential, revocations, now_epoch)
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn submit_next(
        &self,
        credential: &[u8],
    ) -> Result<PopOutboxSubmitOutcomeV1, PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let mut service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            self.api.submit_next(
                &mut *service,
                credential,
                self.registry_submitter.as_ref(),
                now_epoch,
            )
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn reconcile_next(&self, credential: &[u8]) -> Result<bool, PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let mut service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            self.api.reconcile_next(
                &mut *service,
                credential,
                self.registry_reader.as_ref(),
                now_epoch,
            )
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn finalized_projection(
        &self,
        credential: &[u8],
    ) -> Result<Option<PopFinalizedRegistryProjectionV1>, PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            self.api
                .finalized_projection(&service, credential, now_epoch)
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn wallet_delivery(
        &self,
        credential: &[u8],
        request_id: [u8; 32],
    ) -> Result<Vec<u8>, PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            self.api
                .wallet_delivery(&service, credential, request_id, now_epoch)
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn import_wallet_delivery(
        &self,
        credential: &[u8],
        request_id: [u8; 32],
    ) -> Result<[u8; 32], PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            self.api.import_wallet_delivery(
                &service,
                &self.wallet,
                &self.wallet_recipient_secret,
                credential,
                request_id,
                now_epoch,
            )
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn acknowledge_wallet_delivery(
        &self,
        credential: &[u8],
        request_id: [u8; 32],
    ) -> Result<(), PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let mut service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            self.api
                .acknowledge_wallet_delivery(&mut *service, credential, request_id, now_epoch)
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn synchronize_wallet_witness(
        &self,
        credential: &[u8],
        credential_commitment: [u8; 32],
    ) -> Result<(), PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            authorize_private_provider_access(
                self.authenticator.as_ref(),
                credential,
                PopCredentialApiActionV1::SynchronizeWalletWitness,
                pop_digest_domain(
                    POP_WALLET_WITNESS_SYNC_BINDING_DOMAIN_V1,
                    &credential_commitment,
                ),
                now_epoch,
            )?;
            let projection = service
                .finalized_projection()
                .ok_or(PopCredentialServiceError::NotSynchronized)?;
            let witness = self
                .wallet_witness_provider
                .resolve(credential_commitment, projection)
                .map_err(|_| PopCredentialServiceError::RuntimeProviderUnavailable)?;
            self.wallet
                .synchronize_witness(credential_commitment, projection, &witness)
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn prove_membership(
        &self,
        credential: &[u8],
        credential_commitment: [u8; 32],
        challenge_digest: [u8; 32],
        verifier_context: &str,
    ) -> Result<PopMembershipProofV1, PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            self.api.prove_membership(
                &service,
                &self.wallet,
                credential,
                credential_commitment,
                challenge_digest,
                verifier_context,
                now_epoch,
            )
        }
        .await;
        self.provider_registry.finish(result)
    }

    async fn verify_membership(
        &self,
        credential: &[u8],
        proof: &PopMembershipProofV1,
        challenge_digest: [u8; 32],
        verifier_context: &str,
    ) -> Result<(), PopCredentialServiceError> {
        self.provider_registry.assert_qualification()?;
        let result = async {
            let mut service = self.service.lock().await;
            let now_epoch = self.current_epoch()?;
            self.api.verify_membership(
                &mut *service,
                credential,
                proof,
                challenge_digest,
                verifier_context,
                now_epoch,
            )
        }
        .await;
        self.provider_registry.finish(result)
    }
}

#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
/// Request envelope carrying one exact canonical native-Norito payload.
pub struct PopCanonicalPayloadRequestV1 {
    /// Canonical native-Norito bytes encoded as unpadded URL-safe base64.
    pub canonical_payload_base64url: String,
}

#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
/// Request envelope selecting one durable PoP enrollment by identifier.
pub struct PopRequestIdRequestV1 {
    /// Non-zero 32-byte request id as lowercase hex.
    pub request_id_hex: String,
}

#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
/// Request envelope selecting one wallet credential commitment.
pub struct PopCredentialCommitmentRequestV1 {
    /// Non-zero credential commitment as lowercase hex.
    pub credential_commitment_hex: String,
}

#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
/// Request envelope for local membership-proof generation.
pub struct PopMembershipRequestV1 {
    /// Non-zero credential commitment as lowercase hex.
    pub credential_commitment_hex: String,
    /// Non-zero verifier challenge digest as lowercase hex.
    pub challenge_digest_hex: String,
    /// Canonical bounded verifier context.
    pub verifier_context: String,
}

#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
/// Request envelope for verification and exactly-once nullifier consumption.
pub struct PopVerifyMembershipRequestV1 {
    /// Canonical native-Norito proof encoded as unpadded URL-safe base64.
    pub canonical_proof_base64url: String,
    /// Non-zero verifier challenge digest as lowercase hex.
    pub challenge_digest_hex: String,
    /// Canonical bounded verifier context.
    pub verifier_context: String,
}

#[derive(Clone, Copy, Debug, Default, NoritoSerialize, NoritoDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
/// Strict empty-object request used by bounded worker and projection endpoints.
pub struct PopEmptyRequestV1 {}

impl norito::json::JsonDeserialize for PopEmptyRequestV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        parser.expect(b'{')?;
        if !parser.try_consume_char(b'}')? {
            return Err(norito::json::Error::Message(
                "empty PoP request rejects unknown fields".to_owned(),
            ));
        }
        Ok(Self {})
    }
}

#[derive(Clone, Debug, JsonSerialize)]
/// Payload-free durable enrollment lifecycle response.
pub struct PopEnrollmentStatusResponseV1 {
    /// Request identifier.
    pub request_id_hex: String,
    /// Stable lifecycle state.
    pub state: String,
    /// Number of current distinct non-revoked approvals.
    pub active_approval_count: u8,
    /// Registry operation digest, when issued.
    pub registry_operation_digest_hex: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize)]
/// Accepted durable registry-operation response.
pub struct PopOperationResponseV1 {
    /// Domain-separated durable operation digest.
    pub operation_digest_hex: String,
}

#[derive(Clone, Copy, Debug, JsonSerialize)]
/// Result of one bounded reconciliation step.
pub struct PopBooleanOutcomeResponseV1 {
    /// Whether the requested bounded step advanced state.
    pub advanced: bool,
}

#[derive(Clone, Debug, JsonSerialize)]
/// Result of one bounded durable-outbox submission step.
pub struct PopOutboxOutcomeResponseV1 {
    /// Stable outcome label.
    pub outcome: String,
    /// Operation digest, absent only when the outbox was idle.
    pub operation_digest_hex: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize)]
/// Public finalized registry projection response.
pub struct PopProjectionResponseV1 {
    /// Whether a finalized projection is currently available.
    pub available: bool,
    /// Finalized height.
    pub block_height: Option<u64>,
    /// Finalized block hash.
    pub block_hash_hex: Option<String>,
    /// Previous finalized block hash.
    pub previous_block_hash_hex: Option<String>,
    /// Active finalized issuer-policy digest.
    pub issuer_policy_digest_hex: Option<String>,
    /// Canonical signed commitment-root bytes.
    pub canonical_commitment_root_base64url: Option<String>,
    /// Canonical signed revocation-list bytes.
    pub canonical_revocation_list_base64url: Option<String>,
    /// Committed operation digests in canonical order.
    pub committed_operation_digests_hex: Vec<String>,
    /// Rejected operation digests in canonical order.
    pub rejected_operation_digests_hex: Vec<String>,
    /// Revoked issuer public keys in canonical order.
    pub revoked_issuer_public_keys_hex: Vec<String>,
}

#[derive(Clone, Debug, JsonSerialize)]
/// Encrypted finalized wallet-delivery response.
pub struct PopEncryptedDeliveryResponseV1 {
    /// Canonical encrypted delivery bytes; never plaintext credential material.
    pub canonical_delivery_base64url: String,
}

#[derive(Clone, Debug, JsonSerialize)]
/// Result of importing encrypted delivery into local wallet custody.
pub struct PopCredentialCommitmentResponseV1 {
    /// Imported encrypted-vault credential commitment.
    pub credential_commitment_hex: String,
}

#[derive(Clone, Debug, JsonSerialize)]
/// Public zero-knowledge membership-proof response.
pub struct PopMembershipProofResponseV1 {
    /// Canonical public zero-knowledge proof bytes.
    pub canonical_proof_base64url: String,
}

#[derive(Clone, Debug, JsonSerialize)]
struct PopOkResponseV1 {
    ok: bool,
}

#[derive(Clone, Debug, JsonSerialize)]
struct PopErrorResponseV1 {
    code: String,
    message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PopFinalizedTimeSampleErrorV1 {
    Invalid,
    Skew,
    Rollback,
    Fork,
}

fn validate_finalized_time_sample(
    previous: Option<&PopFinalizedTimeSampleV1>,
    sample: &PopFinalizedTimeSampleV1,
    max_skew_secs: u64,
) -> Result<(), PopFinalizedTimeSampleErrorV1> {
    if sample.finalized_block_height == 0
        || sample.finalized_block_hash == [0; 32]
        || sample.finalized_epoch == 0
        || sample.observed_epoch == 0
    {
        return Err(PopFinalizedTimeSampleErrorV1::Invalid);
    }
    if sample.finalized_epoch.abs_diff(sample.observed_epoch) > max_skew_secs {
        return Err(PopFinalizedTimeSampleErrorV1::Skew);
    }
    let Some(previous) = previous else {
        return Ok(());
    };
    if sample.finalized_block_height < previous.finalized_block_height
        || sample.finalized_epoch < previous.finalized_epoch
        || sample.observed_epoch < previous.observed_epoch
    {
        return Err(PopFinalizedTimeSampleErrorV1::Rollback);
    }
    if (sample.finalized_block_height == previous.finalized_block_height
        && (sample.finalized_block_hash != previous.finalized_block_hash
            || sample.finalized_epoch != previous.finalized_epoch))
        || (sample.finalized_block_height > previous.finalized_block_height
            && sample.finalized_block_hash == previous.finalized_block_hash)
    {
        return Err(PopFinalizedTimeSampleErrorV1::Fork);
    }
    Ok(())
}

fn pop_digest_domain(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn authorize_private_provider_access(
    authenticator: &dyn PopCredentialApiAuthenticator,
    opaque_credential: &[u8],
    action: PopCredentialApiActionV1,
    request_binding: [u8; 32],
    now_epoch: u64,
) -> Result<(), PopCredentialServiceError> {
    if opaque_credential.is_empty()
        || opaque_credential.len() > POP_API_AUTHENTICATION_MAX_BYTES_V1
        || now_epoch == 0
    {
        return Err(PopCredentialServiceError::Unauthorized);
    }
    let principal = authenticator
        .authenticate(opaque_credential, action, request_binding, now_epoch)
        .map_err(|_| PopCredentialServiceError::Unauthorized)?;
    if principal.principal_digest == [0; 32] || principal.expires_at_epoch <= now_epoch {
        return Err(PopCredentialServiceError::Unauthorized);
    }
    Ok(())
}

fn decode_hex_32(value: &str, field: &'static str) -> Result<[u8; 32], PopCredentialServiceError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(PopCredentialServiceError::InvalidInput { field });
    }
    let mut digest = [0_u8; 32];
    hex::decode_to_slice(value, &mut digest)
        .map_err(|_| PopCredentialServiceError::InvalidInput { field })?;
    if digest == [0; 32] {
        return Err(PopCredentialServiceError::InvalidInput { field });
    }
    Ok(digest)
}

fn decode_base64url(value: &str, max_bytes: usize) -> Result<Vec<u8>, PopCredentialServiceError> {
    if value.is_empty()
        || value.len() > canonical_base64_max_len(max_bytes)
        || value
            .bytes()
            .any(|byte| byte == b'=' || byte.is_ascii_whitespace())
    {
        return Err(PopCredentialServiceError::Codec);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| PopCredentialServiceError::Codec)?;
    if bytes.is_empty() || bytes.len() > max_bytes || URL_SAFE_NO_PAD.encode(&bytes) != value {
        return Err(PopCredentialServiceError::Codec);
    }
    Ok(bytes)
}

fn canonical_decode_limits(max_bytes: usize) -> norito::DecodeLimits {
    let bounded_bytes = max_bytes.max(1);
    norito::DecodeLimits::new(
        bounded_bytes,
        bounded_bytes,
        bounded_bytes,
        bounded_bytes.saturating_mul(4),
        POP_CANONICAL_DECODE_MAX_DEPTH_V1,
    )
}

fn decode_canonical_bytes_with_limits<T>(
    bytes: &[u8],
    limits: norito::DecodeLimits,
) -> Result<T, PopCredentialServiceError>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    let decoded: T = norito::decode_from_bytes_with_limits(bytes, limits)
        .map_err(|_| PopCredentialServiceError::Codec)?;
    let canonical = norito::to_bytes(&decoded).map_err(|_| PopCredentialServiceError::Codec)?;
    if canonical != bytes {
        return Err(PopCredentialServiceError::Codec);
    }
    Ok(decoded)
}

fn decode_canonical<T>(value: &str, max_bytes: usize) -> Result<T, PopCredentialServiceError>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    let bytes = decode_base64url(value, max_bytes)?;
    decode_canonical_bytes_with_limits(&bytes, canonical_decode_limits(max_bytes))
}

fn canonical_base64url<T: norito::core::NoritoSerialize>(
    value: &T,
) -> Result<String, PopCredentialServiceError> {
    norito::to_bytes(value)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| PopCredentialServiceError::Codec)
}

fn parse_authentication(
    headers: &HeaderMap,
) -> Result<PopApiCredentialV1, PopCredentialServiceError> {
    let mut values = headers.get_all(POP_AUTHORIZATION_HEADER_V1).iter();
    let value = values
        .next()
        .filter(|_| values.next().is_none())
        .and_then(|value| value.to_str().ok())
        .ok_or(PopCredentialServiceError::Unauthorized)?;
    let encoded = value
        .strip_prefix("PopV1 ")
        .ok_or(PopCredentialServiceError::Unauthorized)?;
    if encoded.is_empty()
        || encoded.len() > canonical_base64_max_len(POP_API_AUTHENTICATION_MAX_BYTES_V1)
        || encoded
            .bytes()
            .any(|byte| byte == b'=' || byte.is_ascii_whitespace())
    {
        return Err(PopCredentialServiceError::Unauthorized);
    }
    let credential = PopApiCredentialV1::new(
        URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| PopCredentialServiceError::Unauthorized)?,
    );
    if credential.as_bytes().is_empty()
        || credential.as_bytes().len() > POP_API_AUTHENTICATION_MAX_BYTES_V1
        || URL_SAFE_NO_PAD.encode(credential.as_bytes()) != encoded
    {
        return Err(PopCredentialServiceError::Unauthorized);
    }
    Ok(credential)
}

fn status_response(status: PopEnrollmentStatusV1) -> PopEnrollmentStatusResponseV1 {
    let state = match status.state {
        PopEnrollmentStateV1::AwaitingApproval => "awaiting_approval",
        PopEnrollmentStateV1::Approved => "approved",
        PopEnrollmentStateV1::Rejected => "rejected",
        PopEnrollmentStateV1::PendingRegistry => "pending_registry",
        PopEnrollmentStateV1::DeliveryReady => "delivery_ready",
        PopEnrollmentStateV1::Delivered => "delivered",
    };
    PopEnrollmentStatusResponseV1 {
        request_id_hex: hex::encode(status.request_id),
        state: state.to_owned(),
        active_approval_count: status.active_approval_count,
        registry_operation_digest_hex: status.registry_operation_digest.map(hex::encode),
    }
}

fn outbox_response(outcome: PopOutboxSubmitOutcomeV1) -> PopOutboxOutcomeResponseV1 {
    match outcome {
        PopOutboxSubmitOutcomeV1::Idle => PopOutboxOutcomeResponseV1 {
            outcome: "idle".to_owned(),
            operation_digest_hex: None,
        },
        PopOutboxSubmitOutcomeV1::Submitted { operation_digest } => PopOutboxOutcomeResponseV1 {
            outcome: "submitted".to_owned(),
            operation_digest_hex: Some(hex::encode(operation_digest)),
        },
        PopOutboxSubmitOutcomeV1::RetryScheduled { operation_digest } => {
            PopOutboxOutcomeResponseV1 {
                outcome: "retry_scheduled".to_owned(),
                operation_digest_hex: Some(hex::encode(operation_digest)),
            }
        }
        PopOutboxSubmitOutcomeV1::DeadLettered { operation_digest } => PopOutboxOutcomeResponseV1 {
            outcome: "dead_lettered".to_owned(),
            operation_digest_hex: Some(hex::encode(operation_digest)),
        },
    }
}

fn projection_response(
    projection: Option<PopFinalizedRegistryProjectionV1>,
) -> PopProjectionResponseV1 {
    let Some(projection) = projection else {
        return PopProjectionResponseV1 {
            available: false,
            block_height: None,
            block_hash_hex: None,
            previous_block_hash_hex: None,
            issuer_policy_digest_hex: None,
            canonical_commitment_root_base64url: None,
            canonical_revocation_list_base64url: None,
            committed_operation_digests_hex: Vec::new(),
            rejected_operation_digests_hex: Vec::new(),
            revoked_issuer_public_keys_hex: Vec::new(),
        };
    };
    PopProjectionResponseV1 {
        available: true,
        block_height: Some(projection.cursor.block_height),
        block_hash_hex: Some(hex::encode(projection.cursor.block_hash)),
        previous_block_hash_hex: projection.previous_block_hash.map(hex::encode),
        issuer_policy_digest_hex: Some(hex::encode(projection.issuer_policy_digest)),
        canonical_commitment_root_base64url: Some(
            URL_SAFE_NO_PAD.encode(projection.canonical_commitment_root),
        ),
        canonical_revocation_list_base64url: Some(
            URL_SAFE_NO_PAD.encode(projection.canonical_revocation_list),
        ),
        committed_operation_digests_hex: projection
            .committed_operation_digests
            .into_iter()
            .map(hex::encode)
            .collect(),
        rejected_operation_digests_hex: projection
            .rejected_operation_digests
            .into_iter()
            .map(hex::encode)
            .collect(),
        revoked_issuer_public_keys_hex: projection
            .revoked_issuer_public_keys
            .into_iter()
            .map(hex::encode)
            .collect(),
    }
}

fn no_store(response: impl IntoResponse) -> Response {
    let mut response = response.into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn error_response(error: PopCredentialServiceError) -> Response {
    let (status, code, message) = match error {
        PopCredentialServiceError::Unauthorized => (
            StatusCode::UNAUTHORIZED,
            "pop_unauthorized",
            "PoP request authentication failed.",
        ),
        PopCredentialServiceError::SignerRevoked => (
            StatusCode::FORBIDDEN,
            "pop_signer_revoked",
            "The governed PoP signer is revoked.",
        ),
        PopCredentialServiceError::EnrollmentNotFound
        | PopCredentialServiceError::CredentialNotFound => (
            StatusCode::NOT_FOUND,
            "pop_not_found",
            "The requested PoP record was not found.",
        ),
        PopCredentialServiceError::EnrollmentReplay
        | PopCredentialServiceError::DuplicateApproval
        | PopCredentialServiceError::ReplayedProof
        | PopCredentialServiceError::RootRollback
        | PopCredentialServiceError::InvalidState
        | PopCredentialServiceError::ApprovalQuorum
        | PopCredentialServiceError::NotFinalized
        | PopCredentialServiceError::NotSynchronized => (
            StatusCode::CONFLICT,
            "pop_state_conflict",
            "The PoP transition conflicts with durable finalized state.",
        ),
        PopCredentialServiceError::ResourceExhausted => (
            StatusCode::TOO_MANY_REQUESTS,
            "pop_resource_exhausted",
            "The bounded PoP service resource policy was exhausted.",
        ),
        PopCredentialServiceError::CheckpointIo
        | PopCredentialServiceError::CheckpointDurabilityUncertain
        | PopCredentialServiceError::PoisonedCheckpoint
        | PopCredentialServiceError::RegistryUnavailable
        | PopCredentialServiceError::HsmUnavailable
        | PopCredentialServiceError::HsmPolicyMismatch
        | PopCredentialServiceError::KeyWrapping
        | PopCredentialServiceError::RuntimeProviderUnavailable
        | PopCredentialServiceError::RuntimeProviderRegistryMissing
        | PopCredentialServiceError::RuntimeProviderRegistryMismatch
        | PopCredentialServiceError::RuntimeProviderRegistryUnavailable
        | PopCredentialServiceError::RuntimeProviderRegistryDrift => (
            StatusCode::SERVICE_UNAVAILABLE,
            "pop_runtime_unavailable",
            "The governed PoP runtime is unavailable.",
        ),
        _ => (
            StatusCode::BAD_REQUEST,
            "pop_invalid_request",
            "The PoP request is malformed or violates canonical V1 policy.",
        ),
    };
    no_store((
        status,
        JsonBody(PopErrorResponseV1 {
            code: code.to_owned(),
            message: message.to_owned(),
        }),
    ))
}

fn runtime(app: &SharedAppState) -> Result<Arc<PopCredentialToriiRuntimeV1>, Response> {
    app.sorafs_pop_credentials
        .clone()
        .ok_or_else(|| error_response(PopCredentialServiceError::RuntimeProviderUnavailable))
}

fn request_context(
    app: &SharedAppState,
    headers: &HeaderMap,
) -> Result<(Arc<PopCredentialToriiRuntimeV1>, PopApiCredentialV1), Response> {
    let runtime = runtime(app)?;
    let credential = parse_authentication(headers).map_err(error_response)?;
    Ok((runtime, credential))
}

/// Submit a canonical encrypted enrollment.
pub(crate) async fn handle_post_pop_enrollment(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<PopCanonicalPayloadRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let enrollment = match decode_base64url(
        &request.canonical_payload_base64url,
        POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1,
    ) {
        Ok(enrollment) => enrollment,
        Err(error) => return error_response(error),
    };
    match runtime
        .submit_enrollment(credential.as_bytes(), &enrollment)
        .await
    {
        Ok(status) => no_store((StatusCode::ACCEPTED, JsonBody(status_response(status)))),
        Err(error) => error_response(error),
    }
}

/// Return payload-free enrollment status.
pub(crate) async fn handle_post_pop_enrollment_status(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<PopRequestIdRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let request_id = match decode_hex_32(&request.request_id_hex, "request_id_hex") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match runtime
        .enrollment_status(credential.as_bytes(), request_id)
        .await
    {
        Ok(status) => no_store(JsonBody(status_response(status))),
        Err(error) => error_response(error),
    }
}

/// Record one signed governed approval.
pub(crate) async fn handle_post_pop_approval(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<PopCanonicalPayloadRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let approval = match decode_canonical::<PopApprovalV1>(
        &request.canonical_payload_base64url,
        POP_CONTROL_PAYLOAD_MAX_BYTES_V1,
    ) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match runtime
        .record_approval(credential.as_bytes(), approval)
        .await
    {
        Ok(status) => no_store(JsonBody(status_response(status))),
        Err(error) => error_response(error),
    }
}

/// Trigger server-resolved, HSM-backed issuance.
pub(crate) async fn handle_post_pop_issue(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<PopRequestIdRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let request_id = match decode_hex_32(&request.request_id_hex, "request_id_hex") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match runtime.issue(credential.as_bytes(), request_id).await {
        Ok(digest) => no_store((
            StatusCode::ACCEPTED,
            JsonBody(PopOperationResponseV1 {
                operation_digest_hex: hex::encode(digest),
            }),
        )),
        Err(error) => error_response(error),
    }
}

/// Enqueue a strict signed revocation successor.
pub(crate) async fn handle_post_pop_revocation(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<PopCanonicalPayloadRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let revocations = match decode_canonical::<PopRevocationListV1>(
        &request.canonical_payload_base64url,
        POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1,
    ) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match runtime
        .enqueue_revocation(credential.as_bytes(), revocations)
        .await
    {
        Ok(digest) => no_store((
            StatusCode::ACCEPTED,
            JsonBody(PopOperationResponseV1 {
                operation_digest_hex: hex::encode(digest),
            }),
        )),
        Err(error) => error_response(error),
    }
}

/// Run one authenticated registry submission step.
pub(crate) async fn handle_post_pop_registry_submit(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(_request): NoritoJson<PopEmptyRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    match runtime.submit_next(credential.as_bytes()).await {
        Ok(outcome) => no_store(JsonBody(outbox_response(outcome))),
        Err(error) => error_response(error),
    }
}

/// Reconcile at most one finalized ledger projection.
pub(crate) async fn handle_post_pop_registry_reconcile(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(_request): NoritoJson<PopEmptyRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    match runtime.reconcile_next(credential.as_bytes()).await {
        Ok(advanced) => no_store(JsonBody(PopBooleanOutcomeResponseV1 { advanced })),
        Err(error) => error_response(error),
    }
}

/// Read the authenticated finalized public root projection.
pub(crate) async fn handle_post_pop_registry_projection(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(_request): NoritoJson<PopEmptyRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    match runtime.finalized_projection(credential.as_bytes()).await {
        Ok(projection) => no_store(JsonBody(projection_response(projection))),
        Err(error) => error_response(error),
    }
}

/// Fetch canonical encrypted wallet delivery.
pub(crate) async fn handle_post_pop_wallet_delivery(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<PopRequestIdRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let request_id = match decode_hex_32(&request.request_id_hex, "request_id_hex") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match runtime
        .wallet_delivery(credential.as_bytes(), request_id)
        .await
    {
        Ok(delivery) => no_store(JsonBody(PopEncryptedDeliveryResponseV1 {
            canonical_delivery_base64url: URL_SAFE_NO_PAD.encode(delivery),
        })),
        Err(error) => error_response(error),
    }
}

/// Import a finalized encrypted delivery into runtime wallet custody.
pub(crate) async fn handle_post_pop_wallet_import(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<PopRequestIdRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let request_id = match decode_hex_32(&request.request_id_hex, "request_id_hex") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match runtime
        .import_wallet_delivery(credential.as_bytes(), request_id)
        .await
    {
        Ok(commitment) => no_store(JsonBody(PopCredentialCommitmentResponseV1 {
            credential_commitment_hex: hex::encode(commitment),
        })),
        Err(error) => error_response(error),
    }
}

/// Acknowledge encrypted delivery without deleting recoverable ciphertext.
pub(crate) async fn handle_post_pop_wallet_acknowledge(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<PopRequestIdRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let request_id = match decode_hex_32(&request.request_id_hex, "request_id_hex") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match runtime
        .acknowledge_wallet_delivery(credential.as_bytes(), request_id)
        .await
    {
        Ok(()) => no_store(JsonBody(PopOkResponseV1 { ok: true })),
        Err(error) => error_response(error),
    }
}

/// Synchronize a runtime-only witness to the current finalized roots.
pub(crate) async fn handle_post_pop_wallet_synchronize(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<PopCredentialCommitmentRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let commitment = match decode_hex_32(
        &request.credential_commitment_hex,
        "credential_commitment_hex",
    ) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match runtime
        .synchronize_wallet_witness(credential.as_bytes(), commitment)
        .await
    {
        Ok(()) => no_store(JsonBody(PopOkResponseV1 { ok: true })),
        Err(error) => error_response(error),
    }
}

/// Generate a public zero-knowledge membership proof from local wallet custody.
pub(crate) async fn handle_post_pop_wallet_prove(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<PopMembershipRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let commitment = match decode_hex_32(
        &request.credential_commitment_hex,
        "credential_commitment_hex",
    ) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let challenge = match decode_hex_32(&request.challenge_digest_hex, "challenge_digest_hex") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    if !valid_context(&request.verifier_context) {
        return error_response(PopCredentialServiceError::InvalidInput {
            field: "verifier_context",
        });
    }
    match runtime
        .prove_membership(
            credential.as_bytes(),
            commitment,
            challenge,
            &request.verifier_context,
        )
        .await
    {
        Ok(proof) => match canonical_base64url(&proof) {
            Ok(encoded) => no_store(JsonBody(PopMembershipProofResponseV1 {
                canonical_proof_base64url: encoded,
            })),
            Err(error) => error_response(error),
        },
        Err(error) => error_response(error),
    }
}

/// Verify a membership proof and durably consume its nullifier.
pub(crate) async fn handle_post_pop_verify(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<PopVerifyMembershipRequestV1>,
) -> Response {
    let (runtime, credential) = match request_context(&app, &headers) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let challenge = match decode_hex_32(&request.challenge_digest_hex, "challenge_digest_hex") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    if !valid_context(&request.verifier_context) {
        return error_response(PopCredentialServiceError::InvalidInput {
            field: "verifier_context",
        });
    }
    let proof = match decode_canonical::<PopMembershipProofV1>(
        &request.canonical_proof_base64url,
        POP_MEMBERSHIP_PROOF_MAX_BYTES_V1,
    ) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match runtime
        .verify_membership(
            credential.as_bytes(),
            &proof,
            challenge,
            &request.verifier_context,
        )
        .await
    {
        Ok(()) => no_store(JsonBody(PopOkResponseV1 { ok: true })),
        Err(error) => error_response(error),
    }
}

fn valid_context(value: &str) -> bool {
    !value.is_empty()
        && value == value.trim()
        && value.len() <= POP_MEMBERSHIP_CONTEXT_MAX_BYTES_V1
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

    use iroha_crypto::{Algorithm, HybridKeyPair, KeyPair, Signature};

    #[derive(Debug)]
    struct FixedAuthenticator {
        principal_digest: [u8; 32],
        expires_at_epoch: u64,
        reject: bool,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl PopCredentialApiAuthenticator for FixedAuthenticator {
        fn authenticate(
            &self,
            _opaque_credential: &[u8],
            _action: PopCredentialApiActionV1,
            _request_binding: [u8; 32],
            _now_epoch: u64,
        ) -> Result<sorafs_node::pop_credentials::PopAuthenticatedPrincipalV1, String> {
            self.calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if self.reject {
                return Err("redacted test rejection".to_owned());
            }
            Ok(sorafs_node::pop_credentials::PopAuthenticatedPrincipalV1 {
                principal_digest: self.principal_digest,
                expires_at_epoch: self.expires_at_epoch,
            })
        }
    }

    struct TestRuntimeHsm {
        key_id: String,
        keypair: KeyPair,
        drift_revision: Option<Arc<AtomicU64>>,
    }

    impl fmt::Debug for TestRuntimeHsm {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("TestRuntimeHsm")
                .field("key_id", &self.key_id)
                .field("private_key", &"[REDACTED]")
                .finish()
        }
    }

    impl PopIssuerHsm for TestRuntimeHsm {
        fn key_id(&self) -> &str {
            &self.key_id
        }

        fn public_key(&self) -> [u8; 32] {
            self.keypair
                .public_key()
                .to_bytes()
                .1
                .try_into()
                .expect("Ed25519 public key width")
        }

        fn sign_digest(&self, digest: [u8; 32]) -> Result<[u8; 64], String> {
            let signature: [u8; 64] = Signature::try_new(self.keypair.private_key(), &digest)
                .map_err(|_| "HSM signing failed".to_owned())?
                .payload()
                .try_into()
                .map_err(|_| "HSM signature width changed".to_owned())?;
            if let Some(revision) = &self.drift_revision {
                revision.fetch_add(1, Ordering::SeqCst);
            }
            Ok(signature)
        }
    }

    struct TestWalletKeyWrapper {
        key_id: String,
        wrapping_key: [u8; 32],
        drift_revision: Option<Arc<AtomicU64>>,
    }

    impl fmt::Debug for TestWalletKeyWrapper {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("TestWalletKeyWrapper")
                .field("key_id", &self.key_id)
                .field("wrapping_key", &"[REDACTED]")
                .finish()
        }
    }

    impl PopWalletKeyWrapper for TestWalletKeyWrapper {
        fn active_key_id(&self) -> &str {
            &self.key_id
        }

        fn wrap_dek(&self, context: [u8; 32], dek: &[u8; 32]) -> Result<Vec<u8>, String> {
            let wrapped = dek
                .iter()
                .zip(self.wrapping_key)
                .zip(context)
                .map(|((&byte, key), aad)| byte ^ key ^ aad)
                .collect();
            if let Some(revision) = &self.drift_revision {
                revision.fetch_add(1, Ordering::SeqCst);
            }
            Ok(wrapped)
        }

        fn unwrap_dek(
            &self,
            key_id: &str,
            context: [u8; 32],
            wrapped_dek: &[u8],
        ) -> Result<[u8; 32], String> {
            if key_id != self.key_id || wrapped_dek.len() != 32 {
                return Err("wallet key unavailable".to_owned());
            }
            let mut dek = [0; 32];
            for (index, output) in dek.iter_mut().enumerate() {
                *output = wrapped_dek[index] ^ self.wrapping_key[index] ^ context[index];
            }
            if let Some(revision) = &self.drift_revision {
                revision.fetch_add(1, Ordering::SeqCst);
            }
            Ok(dek)
        }
    }

    #[derive(Debug)]
    struct NoopRegistrySubmitter;

    impl PopRegistrySubmitter for NoopRegistrySubmitter {
        fn submit(
            &self,
            _idempotency_key: [u8; 32],
            _operation: &sorafs_node::pop_credentials::PopRegistryOperationV1,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct EmptyRegistryReader;

    impl PopFinalizedRegistryReader for EmptyRegistryReader {
        fn next_after(
            &self,
            _cursor: Option<sorafs_node::pop_credentials::PopFinalizedCursorV1>,
        ) -> Result<Option<PopFinalizedRegistryProjectionV1>, String> {
            Ok(None)
        }
    }

    #[derive(Debug)]
    struct UnavailableIssuanceDraftProvider;

    impl PopIssuanceDraftProviderV1 for UnavailableIssuanceDraftProvider {
        fn resolve(
            &self,
            _request_id: [u8; 32],
            _now_epoch: u64,
        ) -> Result<PopIssuanceDraftV1, PopPrivateMaterialProviderErrorV1> {
            Err(PopPrivateMaterialProviderErrorV1::Unavailable)
        }
    }

    #[derive(Debug)]
    struct UnavailableWalletWitnessProvider;

    impl PopWalletWitnessProviderV1 for UnavailableWalletWitnessProvider {
        fn resolve(
            &self,
            _credential_commitment: [u8; 32],
            _projection: &PopFinalizedRegistryProjectionV1,
        ) -> Result<PopMembershipWitnessV1, PopPrivateMaterialProviderErrorV1> {
            Err(PopPrivateMaterialProviderErrorV1::Unavailable)
        }
    }

    #[derive(Debug)]
    struct FixedFinalizedTimeProvider {
        calls: AtomicUsize,
        revision: Arc<AtomicU64>,
        drift_on_sample: AtomicBool,
    }

    impl PopFinalizedTimeProviderV1 for FixedFinalizedTimeProvider {
        fn sample(&self) -> Result<PopFinalizedTimeSampleV1, PopFinalizedTimeProviderErrorV1> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.drift_on_sample.load(Ordering::SeqCst) {
                self.revision.fetch_add(1, Ordering::SeqCst);
            }
            Ok(finalized_time_sample(1, 0x31, 100, 100))
        }
    }

    struct TestRuntimeProviderRegistry {
        handle: String,
        revision: Arc<AtomicU64>,
        policy_digest: [u8; 32],
        qualification_refused: AtomicBool,
        drift_on_resolve: AtomicBool,
        secrets: std::sync::Mutex<Option<PopCredentialRuntimeSecretsV1>>,
        observed_bindings: std::sync::Mutex<Option<PopCredentialRuntimeProviderBindingsV1>>,
    }

    impl fmt::Debug for TestRuntimeProviderRegistry {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("TestRuntimeProviderRegistry")
                .field("handle", &self.handle)
                .field("private_providers", &"[REDACTED]")
                .finish()
        }
    }

    impl PopCredentialRuntimeProviderRegistryV1 for TestRuntimeProviderRegistry {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            PopCredentialRuntimeProviderQualificationV1,
            PopCredentialRuntimeProviderRegistryErrorV1,
        > {
            if self.qualification_refused.load(Ordering::SeqCst) {
                return Err(PopCredentialRuntimeProviderRegistryErrorV1::StaleOrRevoked);
            }
            Ok(PopCredentialRuntimeProviderQualificationV1::new(
                self.revision.load(Ordering::SeqCst),
                self.policy_digest,
            ))
        }

        fn resolve(
            &self,
            bindings: &PopCredentialRuntimeProviderBindingsV1,
        ) -> Result<PopCredentialRuntimeSecretsV1, PopCredentialRuntimeProviderRegistryErrorV1>
        {
            *self
                .observed_bindings
                .lock()
                .map_err(|_| PopCredentialRuntimeProviderRegistryErrorV1::Unavailable)? =
                Some(bindings.clone());
            let secrets = self
                .secrets
                .lock()
                .map_err(|_| PopCredentialRuntimeProviderRegistryErrorV1::Unavailable)?
                .take()
                .ok_or(PopCredentialRuntimeProviderRegistryErrorV1::Unavailable)?;
            if self.drift_on_resolve.load(Ordering::SeqCst) {
                self.revision.fetch_add(1, Ordering::SeqCst);
            }
            Ok(secrets)
        }
    }

    fn finalized_time_sample(
        height: u64,
        hash_byte: u8,
        finalized_epoch: u64,
        observed_epoch: u64,
    ) -> PopFinalizedTimeSampleV1 {
        PopFinalizedTimeSampleV1 {
            finalized_block_height: height,
            finalized_block_hash: [hash_byte; 32],
            finalized_epoch,
            observed_epoch,
        }
    }

    fn ed25519(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("derive test Ed25519 key")
    }

    fn public_key_bytes(keypair: &KeyPair) -> [u8; 32] {
        keypair
            .public_key()
            .to_bytes()
            .1
            .try_into()
            .expect("Ed25519 public key width")
    }

    #[test]
    fn runtime_provider_bindings_constructor_is_exact_and_fail_closed() {
        let issuer_public_key = public_key_bytes(&ed25519(0x40));
        let bindings = PopCredentialRuntimeProviderBindingsV1::try_new(
            [0x41; 32],
            "pop-issuer-runtime-primary".to_owned(),
            "pkcs11:pop/issuer:primary".to_owned(),
            issuer_public_key,
            "kms:pop/enrollment:primary".to_owned(),
            "kms:pop/wallet:primary".to_owned(),
        )
        .expect("canonical exact bindings");
        assert_eq!(bindings.issuer_policy_digest(), [0x41; 32]);
        assert_eq!(bindings.issuer_id(), "pop-issuer-runtime-primary");
        assert_eq!(bindings.issuer_hsm_key_id(), "pkcs11:pop/issuer:primary");
        assert_eq!(bindings.issuer_public_key(), issuer_public_key);
        assert_eq!(
            bindings.enrollment_recipient_key_id(),
            "kms:pop/enrollment:primary"
        );
        assert_eq!(bindings.wallet_wrapping_key_id(), "kms:pop/wallet:primary");

        let assert_rejected = |issuer_policy_digest,
                               issuer_id: &str,
                               issuer_hsm_key_id: &str,
                               issuer_public_key,
                               enrollment_recipient_key_id: &str,
                               wallet_wrapping_key_id: &str,
                               field| {
            assert_eq!(
                PopCredentialRuntimeProviderBindingsV1::try_new(
                    issuer_policy_digest,
                    issuer_id.to_owned(),
                    issuer_hsm_key_id.to_owned(),
                    issuer_public_key,
                    enrollment_recipient_key_id.to_owned(),
                    wallet_wrapping_key_id.to_owned(),
                ),
                Err(PopCredentialServiceError::InvalidInput { field })
            );
        };
        assert_rejected(
            [0; 32],
            "pop-issuer-runtime-primary",
            "pkcs11:pop/issuer:primary",
            issuer_public_key,
            "kms:pop/enrollment:primary",
            "kms:pop/wallet:primary",
            "issuer_policy_digest",
        );
        for issuer_id in [
            "",
            " pop-issuer-runtime-primary",
            "pop-issuer-runtime-primary\n",
        ] {
            assert_rejected(
                [0x41; 32],
                issuer_id,
                "pkcs11:pop/issuer:primary",
                issuer_public_key,
                "kms:pop/enrollment:primary",
                "kms:pop/wallet:primary",
                "issuer_id",
            );
        }
        let oversized_issuer = "i".repeat(POP_IDENTITY_TEXT_MAX_BYTES_V1 + 1);
        assert_rejected(
            [0x41; 32],
            &oversized_issuer,
            "pkcs11:pop/issuer:primary",
            issuer_public_key,
            "kms:pop/enrollment:primary",
            "kms:pop/wallet:primary",
            "issuer_id",
        );
        for (issuer_hsm_key_id, enrollment_key_id, wallet_key_id, field) in [
            (
                "pkcs11:pop/test",
                "kms:pop/enrollment:primary",
                "kms:pop/wallet:primary",
                "issuer_hsm_key_id",
            ),
            (
                "pkcs11:pop/issuer:primary",
                "kms://pop/mock/enrollment",
                "kms:pop/wallet:primary",
                "enrollment_recipient_key_id",
            ),
            (
                "pkcs11:pop/issuer:primary",
                "kms:pop/enrollment:primary",
                "kms://pop/fake/wallet",
                "wallet_wrapping_key_id",
            ),
        ] {
            assert_rejected(
                [0x41; 32],
                "pop-issuer-runtime-primary",
                issuer_hsm_key_id,
                issuer_public_key,
                enrollment_key_id,
                wallet_key_id,
                field,
            );
        }
        assert_rejected(
            [0x41; 32],
            "pop-issuer-runtime-primary",
            "pkcs11:pop/issuer:primary",
            [0; 32],
            "kms:pop/enrollment:primary",
            "kms:pop/wallet:primary",
            "issuer_public_key",
        );
    }

    fn runtime_fixture(
        root: &std::path::Path,
        expected_registry_handle: &str,
        provider_registry_handle: &str,
    ) -> (
        PopCredentialRuntimeConfigV1,
        Arc<TestRuntimeProviderRegistry>,
        Arc<FixedFinalizedTimeProvider>,
    ) {
        let hsm = Arc::new(TestRuntimeHsm {
            key_id: "pkcs11:pop/issuer:primary".to_owned(),
            keypair: ed25519(0x41),
            drift_revision: None,
        });
        let approver_a = ed25519(0x42);
        let approver_b = ed25519(0x43);
        let config = PopCredentialRuntimeConfigV1 {
            service_policy: PopCredentialServicePolicyV1 {
                version: POP_CREDENTIAL_SERVICE_POLICY_VERSION_V1,
                issuer_policy_digest: [0x51; 32],
                issuer_id: "pop-issuer-runtime-primary".to_owned(),
                issuer_hsm_key_id: hsm.key_id.clone(),
                issuer_public_key: hsm.public_key(),
                enrollment_recipient_key_id: "kms:pop/enrollment:primary".to_owned(),
                approval_quorum: 2,
                approval_signers: vec![
                    PopApprovalSignerV1 {
                        signer_id: "approver-a".to_owned(),
                        public_key: public_key_bytes(&approver_a),
                        revoked_at_epoch: None,
                    },
                    PopApprovalSignerV1 {
                        signer_id: "approver-b".to_owned(),
                        public_key: public_key_bytes(&approver_b),
                        revoked_at_epoch: None,
                    },
                ],
                max_pending_enrollments: 16,
                max_outbox_entries: 16,
                max_dead_letters: 16,
                max_seen_nullifiers: 16,
                max_submission_attempts: 3,
            },
            issuer_state_dir: root.join("issuer"),
            wallet_state_dir: root.join("wallet"),
            worker_interval: Duration::from_secs(1),
            max_finalized_time_skew: Duration::from_secs(30),
            wallet_wrapping_key_id: "kms:pop/wallet:primary".to_owned(),
            runtime_provider_registry_handle: expected_registry_handle.to_owned(),
            runtime_provider_registry_revision: 7,
            runtime_provider_registry_policy_digest: [0x61; 32],
        };
        let mut rng = OsRng;
        let enrollment_recipient =
            HybridKeyPair::generate(&mut rng).expect("enrollment recipient key");
        let wallet_recipient = HybridKeyPair::generate(&mut rng).expect("wallet recipient key");
        let revision = Arc::new(AtomicU64::new(7));
        let finalized_time_provider = Arc::new(FixedFinalizedTimeProvider {
            calls: AtomicUsize::new(0),
            revision: Arc::clone(&revision),
            drift_on_sample: AtomicBool::new(false),
        });
        let secrets = PopCredentialRuntimeSecretsV1 {
            enrollment_recipient_secret: enrollment_recipient.secret().clone(),
            issuer_hsm: hsm,
            authenticator: Arc::new(FixedAuthenticator {
                principal_digest: [0x71; 32],
                expires_at_epoch: 1_000,
                reject: false,
                calls: AtomicUsize::new(0),
            }),
            registry_submitter: Arc::new(NoopRegistrySubmitter),
            registry_reader: Arc::new(EmptyRegistryReader),
            issuance_draft_provider: Arc::new(UnavailableIssuanceDraftProvider),
            wallet_recipient_secret: wallet_recipient.secret().clone(),
            wallet_key_wrapper: Arc::new(TestWalletKeyWrapper {
                key_id: "kms:pop/wallet:primary".to_owned(),
                wrapping_key: [0x72; 32],
                drift_revision: None,
            }),
            wallet_witness_provider: Arc::new(UnavailableWalletWitnessProvider),
            finalized_time_provider: finalized_time_provider.clone(),
        };
        let registry = Arc::new(TestRuntimeProviderRegistry {
            handle: provider_registry_handle.to_owned(),
            revision,
            policy_digest: [0x61; 32],
            qualification_refused: AtomicBool::new(false),
            drift_on_resolve: AtomicBool::new(false),
            secrets: std::sync::Mutex::new(Some(secrets)),
            observed_bindings: std::sync::Mutex::new(None),
        });
        (config, registry, finalized_time_provider)
    }

    fn as_runtime_registry(
        registry: &Arc<TestRuntimeProviderRegistry>,
    ) -> Arc<dyn PopCredentialRuntimeProviderRegistryV1> {
        registry.clone()
    }

    fn assert_startup_failure_before_state(
        config: PopCredentialRuntimeConfigV1,
        registry: Option<Arc<dyn PopCredentialRuntimeProviderRegistryV1>>,
        expected: PopCredentialServiceError,
    ) {
        let issuer_state_dir = config.issuer_state_dir.clone();
        let wallet_state_dir = config.wallet_state_dir.clone();
        let error = PopCredentialToriiRuntimeV1::open(config, registry)
            .err()
            .expect("runtime startup must fail");
        assert_eq!(error, expected);
        assert!(!issuer_state_dir.exists());
        assert!(!wallet_state_dir.exists());
    }

    #[test]
    fn runtime_registry_pins_public_bindings_and_blocks_preexisting_drift() {
        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (config, registry, finalized_time) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        let runtime =
            PopCredentialToriiRuntimeV1::open(config.clone(), Some(as_runtime_registry(&registry)))
                .expect("exact provider registry must qualify");

        let observed = registry
            .observed_bindings
            .lock()
            .expect("observed binding lock")
            .clone()
            .expect("registry observed public bindings");
        assert_eq!(
            observed.issuer_policy_digest(),
            config.service_policy.issuer_policy_digest
        );
        assert_eq!(observed.issuer_id(), config.service_policy.issuer_id);
        assert_eq!(
            observed.issuer_hsm_key_id(),
            config.service_policy.issuer_hsm_key_id
        );
        assert_eq!(
            observed.issuer_public_key(),
            config.service_policy.issuer_public_key
        );
        assert_eq!(
            observed.enrollment_recipient_key_id(),
            config.service_policy.enrollment_recipient_key_id
        );
        assert_eq!(
            observed.wallet_wrapping_key_id(),
            config.wallet_wrapping_key_id
        );
        assert!(config.issuer_state_dir.exists());
        assert!(config.wallet_state_dir.exists());

        registry.revision.store(8, Ordering::SeqCst);
        assert_eq!(
            runtime.current_epoch(),
            Err(PopCredentialServiceError::RuntimeProviderRegistryDrift)
        );
        assert_eq!(finalized_time.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn runtime_registry_failures_have_one_payload_free_unavailable_surface() {
        for error in [
            PopCredentialServiceError::RuntimeProviderRegistryMissing,
            PopCredentialServiceError::RuntimeProviderRegistryMismatch,
            PopCredentialServiceError::RuntimeProviderRegistryUnavailable,
            PopCredentialServiceError::RuntimeProviderRegistryDrift,
        ] {
            assert_eq!(
                error_response(error).status(),
                StatusCode::SERVICE_UNAVAILABLE
            );
        }
    }

    #[test]
    fn runtime_registry_discards_a_provider_result_when_policy_drifts_during_call() {
        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (config, registry, finalized_time) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        let runtime =
            PopCredentialToriiRuntimeV1::open(config, Some(as_runtime_registry(&registry)))
                .expect("exact provider registry must qualify");
        finalized_time.drift_on_sample.store(true, Ordering::SeqCst);

        assert_eq!(
            runtime.current_epoch(),
            Err(PopCredentialServiceError::RuntimeProviderRegistryDrift)
        );
        assert_eq!(finalized_time.calls.load(Ordering::SeqCst), 1);
        assert!(
            runtime
                .accepted_finalized_time
                .lock()
                .expect("accepted-time lock")
                .is_none()
        );
    }

    #[test]
    fn qualified_hsm_and_kms_discard_results_when_policy_drifts_during_call() {
        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (config, registry, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        let qualified_registry = QualifiedPopCredentialRuntimeProviderRegistryV1::try_new(
            &config,
            Some(as_runtime_registry(&registry)),
        )
        .expect("exact provider registry must qualify");
        let hsm = QualifiedPopIssuerHsmV1 {
            inner: Arc::new(TestRuntimeHsm {
                key_id: config.service_policy.issuer_hsm_key_id.clone(),
                keypair: ed25519(0x41),
                drift_revision: Some(Arc::clone(&registry.revision)),
            }),
            key_id: config.service_policy.issuer_hsm_key_id.clone(),
            public_key: config.service_policy.issuer_public_key,
            registry: qualified_registry,
        };
        assert_eq!(
            hsm.sign_digest([0x91; 32]),
            Err(POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (config, registry, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        let qualified_registry = QualifiedPopCredentialRuntimeProviderRegistryV1::try_new(
            &config,
            Some(as_runtime_registry(&registry)),
        )
        .expect("exact provider registry must qualify");
        let wrapper = QualifiedPopWalletKeyWrapperV1 {
            inner: Arc::new(TestWalletKeyWrapper {
                key_id: config.wallet_wrapping_key_id.clone(),
                wrapping_key: [0x92; 32],
                drift_revision: Some(Arc::clone(&registry.revision)),
            }),
            active_key_id: config.wallet_wrapping_key_id,
            registry: qualified_registry,
        };
        assert_eq!(
            wrapper.wrap_dek([0x93; 32], &[0x94; 32]),
            Err(POP_RUNTIME_PROVIDER_REDACTED_FAILURE_V1.to_owned())
        );
    }

    #[test]
    fn runtime_provider_handles_use_central_production_grammar() {
        const FIELD: &str = "runtime_provider_registry_handle";
        for handle in [
            "runtime:pop/providers/primary",
            "pkcs11:prod/pop.providers-v1_slot-a",
        ] {
            validate_pop_runtime_provider_handle(handle, FIELD)
                .expect("canonical production provider handle");
        }
        for handle in [
            "https://operator:secret@pop-provider",
            "https://pop-provider/path?credential=secret",
            "https://pop-provider/path#fragment",
            "runtime:pop/%70roviders/primary",
            "runtime:pop\\providers\\primary",
        ] {
            assert!(matches!(
                validate_pop_runtime_provider_handle(handle, FIELD),
                Err(PopCredentialServiceError::InvalidInput { field: FIELD })
            ));
        }
    }

    #[test]
    fn runtime_startup_rejects_missing_substituted_and_test_registries_before_state() {
        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (config, _, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        assert_startup_failure_before_state(
            config,
            None,
            PopCredentialServiceError::RuntimeProviderRegistryMissing,
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (config, registry, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:secondary",
        );
        assert_startup_failure_before_state(
            config,
            Some(as_runtime_registry(&registry)),
            PopCredentialServiceError::RuntimeProviderRegistryMismatch,
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (config, registry, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:test",
            "runtime:pop:providers:test",
        );
        assert_startup_failure_before_state(
            config,
            Some(as_runtime_registry(&registry)),
            PopCredentialServiceError::InvalidInput {
                field: "runtime_provider_registry_handle",
            },
        );
    }

    #[test]
    fn runtime_startup_rejects_stale_mismatched_and_drifting_qualification() {
        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (config, registry, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        registry.qualification_refused.store(true, Ordering::SeqCst);
        assert_startup_failure_before_state(
            config,
            Some(as_runtime_registry(&registry)),
            PopCredentialServiceError::RuntimeProviderRegistryUnavailable,
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (config, registry, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        assert!(
            registry
                .secrets
                .lock()
                .expect("runtime secrets lock")
                .take()
                .is_some()
        );
        assert_startup_failure_before_state(
            config,
            Some(as_runtime_registry(&registry)),
            PopCredentialServiceError::RuntimeProviderRegistryUnavailable,
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (config, registry, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        registry.revision.store(8, Ordering::SeqCst);
        assert_startup_failure_before_state(
            config,
            Some(as_runtime_registry(&registry)),
            PopCredentialServiceError::RuntimeProviderRegistryMismatch,
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (mut config, registry, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        config.runtime_provider_registry_policy_digest = [0x62; 32];
        assert_startup_failure_before_state(
            config,
            Some(as_runtime_registry(&registry)),
            PopCredentialServiceError::RuntimeProviderRegistryMismatch,
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (config, registry, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        registry.drift_on_resolve.store(true, Ordering::SeqCst);
        assert_startup_failure_before_state(
            config,
            Some(as_runtime_registry(&registry)),
            PopCredentialServiceError::RuntimeProviderRegistryDrift,
        );
    }

    #[test]
    fn runtime_startup_rejects_substituted_hsm_and_wallet_identities_before_state() {
        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (mut config, registry, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        config.service_policy.issuer_public_key = public_key_bytes(&ed25519(0x77));
        assert_startup_failure_before_state(
            config,
            Some(as_runtime_registry(&registry)),
            PopCredentialServiceError::HsmPolicyMismatch,
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (mut config, registry, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        config.wallet_wrapping_key_id = "kms:pop:wallet:secondary".to_owned();
        assert_startup_failure_before_state(
            config,
            Some(as_runtime_registry(&registry)),
            PopCredentialServiceError::RuntimeProviderRegistryMismatch,
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let (config, registry, _) = runtime_fixture(
            temporary.path(),
            "runtime:pop:providers:primary",
            "runtime:pop:providers:primary",
        );
        let test_marked_wrapper: Arc<dyn PopWalletKeyWrapper> = Arc::new(TestWalletKeyWrapper {
            key_id: "kms:pop:wallet:test".to_owned(),
            wrapping_key: [0x72; 32],
            drift_revision: None,
        });
        registry
            .secrets
            .lock()
            .expect("runtime secrets lock")
            .as_mut()
            .expect("runtime secrets")
            .wallet_key_wrapper = test_marked_wrapper;
        assert_startup_failure_before_state(
            config,
            Some(as_runtime_registry(&registry)),
            PopCredentialServiceError::RuntimeProviderRegistryMismatch,
        );
    }

    #[test]
    fn finalized_time_rejects_rollback_and_same_height_fork() {
        let previous = finalized_time_sample(10, 0x11, 1_000, 1_001);
        let next = finalized_time_sample(11, 0x12, 1_002, 1_003);
        assert_eq!(
            validate_finalized_time_sample(Some(&previous), &next, 30),
            Ok(())
        );

        for rollback in [
            finalized_time_sample(9, 0x10, 1_002, 1_003),
            finalized_time_sample(11, 0x12, 999, 1_003),
            finalized_time_sample(11, 0x12, 1_002, 1_000),
        ] {
            assert_eq!(
                validate_finalized_time_sample(Some(&previous), &rollback, 30),
                Err(PopFinalizedTimeSampleErrorV1::Rollback)
            );
        }
        assert_eq!(
            validate_finalized_time_sample(
                Some(&previous),
                &finalized_time_sample(10, 0x22, 1_000, 1_002),
                30,
            ),
            Err(PopFinalizedTimeSampleErrorV1::Fork)
        );
    }

    #[test]
    fn finalized_time_rejects_future_and_stale_samples() {
        assert_eq!(
            validate_finalized_time_sample(
                None,
                &finalized_time_sample(10, 0x11, 1_031, 1_000),
                30,
            ),
            Err(PopFinalizedTimeSampleErrorV1::Skew)
        );
        assert_eq!(
            validate_finalized_time_sample(
                None,
                &finalized_time_sample(10, 0x11, 1_000, 1_031),
                30,
            ),
            Err(PopFinalizedTimeSampleErrorV1::Skew)
        );
        assert_eq!(
            validate_finalized_time_sample(
                None,
                &finalized_time_sample(10, 0x11, 1_030, 1_000),
                30,
            ),
            Ok(())
        );
    }

    #[test]
    fn auth_header_is_strict_canonical_and_bounded() {
        let mut headers = HeaderMap::new();
        headers.insert(
            POP_AUTHORIZATION_HEADER_V1,
            HeaderValue::from_static("PopV1 Y3JlZGVudGlhbA"),
        );
        let credential = parse_authentication(&headers).expect("canonical credential");
        assert_eq!(credential.as_bytes(), b"credential");
        assert_eq!(format!("{credential:?}"), "PopApiCredentialV1([REDACTED])");

        for value in [
            "Bearer Y3JlZGVudGlhbA",
            "PopV1 ",
            "PopV1 Y3JlZGVudGlhbA==",
            "PopV1 Y3Jl ZGVudGlhbA",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                POP_AUTHORIZATION_HEADER_V1,
                HeaderValue::from_str(value).expect("test header"),
            );
            assert!(matches!(
                parse_authentication(&headers),
                Err(PopCredentialServiceError::Unauthorized)
            ));
        }
    }

    #[test]
    fn empty_request_accepts_only_an_empty_object() {
        assert!(norito::json::from_json::<PopEmptyRequestV1>("{}").is_ok());
        assert!(norito::json::from_json::<PopEmptyRequestV1>("{ }").is_ok());
        for malformed in [r#"{"unexpected":true}"#, "[]", "null", ""] {
            assert!(norito::json::from_json::<PopEmptyRequestV1>(malformed).is_err());
        }

        let encoded = norito::to_bytes(&PopEmptyRequestV1 {}).expect("encode empty request");
        assert!(
            norito::decode_from_bytes::<PopEmptyRequestV1>(&encoded).is_ok(),
            "native Norito must preserve the exact empty request shape"
        );
    }

    #[test]
    fn private_provider_access_requires_current_action_authorization() {
        let valid = FixedAuthenticator {
            principal_digest: [0x44; 32],
            expires_at_epoch: 101,
            reject: false,
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        let binding = pop_digest_domain(POP_ISSUE_TRIGGER_BINDING_DOMAIN_V1, &[0x22; 32]);
        assert_eq!(
            authorize_private_provider_access(
                &valid,
                b"opaque",
                PopCredentialApiActionV1::TriggerCredentialIssuance,
                binding,
                100,
            ),
            Ok(())
        );
        assert_eq!(valid.calls.load(std::sync::atomic::Ordering::Relaxed), 1);

        for invalid in [
            FixedAuthenticator {
                principal_digest: [0; 32],
                expires_at_epoch: 101,
                reject: false,
                calls: std::sync::atomic::AtomicUsize::new(0),
            },
            FixedAuthenticator {
                principal_digest: [0x44; 32],
                expires_at_epoch: 100,
                reject: false,
                calls: std::sync::atomic::AtomicUsize::new(0),
            },
            FixedAuthenticator {
                principal_digest: [0x44; 32],
                expires_at_epoch: 101,
                reject: true,
                calls: std::sync::atomic::AtomicUsize::new(0),
            },
        ] {
            assert_eq!(
                authorize_private_provider_access(
                    &invalid,
                    b"opaque",
                    PopCredentialApiActionV1::TriggerCredentialIssuance,
                    binding,
                    100,
                ),
                Err(PopCredentialServiceError::Unauthorized)
            );
        }

        let not_called = FixedAuthenticator {
            principal_digest: [0x44; 32],
            expires_at_epoch: 101,
            reject: false,
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        assert_eq!(
            authorize_private_provider_access(
                &not_called,
                &[],
                PopCredentialApiActionV1::TriggerCredentialIssuance,
                binding,
                100,
            ),
            Err(PopCredentialServiceError::Unauthorized)
        );
        assert_eq!(
            not_called.calls.load(std::sync::atomic::Ordering::Relaxed),
            0
        );
    }

    #[test]
    fn authentication_guard_zeroizes_on_early_error() {
        fn fail_after_authentication(
            headers: &HeaderMap,
            probe: Arc<std::sync::Mutex<Vec<u8>>>,
        ) -> Result<(), PopCredentialServiceError> {
            let mut credential = parse_authentication(headers)?;
            credential.drop_probe = Some(probe);
            Err(PopCredentialServiceError::Codec)
        }

        let sentinel = b"pop-auth-sentinel-v1".to_vec();
        let mut headers = HeaderMap::new();
        let encoded = format!("PopV1 {}", URL_SAFE_NO_PAD.encode(&sentinel));
        headers.insert(
            POP_AUTHORIZATION_HEADER_V1,
            HeaderValue::from_str(&encoded).expect("sentinel header"),
        );
        let probe = Arc::new(std::sync::Mutex::new(sentinel.clone()));

        assert!(matches!(
            fail_after_authentication(&headers, Arc::clone(&probe)),
            Err(PopCredentialServiceError::Codec)
        ));
        assert_eq!(
            *probe.lock().expect("drop probe"),
            vec![0_u8; sentinel.len()]
        );
    }

    #[test]
    fn canonical_payload_rejects_padding_malformed_and_oversized_data() {
        assert!(decode_base64url("YQ", 1).is_ok());
        assert_eq!(
            decode_base64url("YQ==", 1),
            Err(PopCredentialServiceError::Codec)
        );
        assert_eq!(
            decode_base64url("**", 8),
            Err(PopCredentialServiceError::Codec)
        );
        assert_eq!(
            decode_base64url(&URL_SAFE_NO_PAD.encode([0_u8; 9]), 8),
            Err(PopCredentialServiceError::Codec)
        );
    }

    #[test]
    fn canonical_decode_rejects_total_allocation_bomb() {
        let payload = vec![vec![0x5a_u8; 64]; 4];
        let encoded = norito::to_bytes(&payload).expect("encode allocation probe");
        let limits = norito::DecodeLimits::new(
            encoded.len(),
            encoded.len(),
            encoded.len(),
            1,
            POP_CANONICAL_DECODE_MAX_DEPTH_V1,
        );

        assert_eq!(
            decode_canonical_bytes_with_limits::<Vec<Vec<u8>>>(&encoded, limits),
            Err(PopCredentialServiceError::Codec)
        );
    }

    #[test]
    fn canonical_decode_rejects_nesting_depth_bomb() {
        let payload = vec![vec![vec![0x5a_u8]]];
        let encoded = norito::to_bytes(&payload).expect("encode depth probe");
        let limits = norito::DecodeLimits::new(
            encoded.len(),
            encoded.len(),
            encoded.len(),
            encoded.len().saturating_mul(4),
            0,
        );

        assert_eq!(
            decode_canonical_bytes_with_limits::<Vec<Vec<Vec<u8>>>>(&encoded, limits),
            Err(PopCredentialServiceError::Codec)
        );
    }

    #[test]
    fn digest_and_context_parsing_reject_noncanonical_values() {
        assert_eq!(
            decode_hex_32(&"ab".repeat(32), "digest").expect("digest"),
            [0xab; 32]
        );
        for value in ["", &"AB".repeat(32), &"00".repeat(32), &"ab".repeat(31)] {
            assert!(decode_hex_32(value, "digest").is_err());
        }
        assert!(valid_context("moderation.assignment.v1"));
        assert!(!valid_context(""));
        assert!(!valid_context(" padded"));
        assert!(!valid_context("line\nbreak"));
        assert!(!valid_context(
            &"x".repeat(POP_MEMBERSHIP_CONTEXT_MAX_BYTES_V1 + 1)
        ));
    }

    #[test]
    fn projection_response_contains_only_public_projection_material() {
        let response = projection_response(Some(PopFinalizedRegistryProjectionV1 {
            version: 1,
            cursor: sorafs_node::pop_credentials::PopFinalizedCursorV1 {
                block_height: 7,
                block_hash: [0x11; 32],
            },
            previous_block_hash: Some([0x10; 32]),
            issuer_policy_digest: [0x22; 32],
            canonical_commitment_root: vec![1, 2, 3],
            canonical_revocation_list: vec![4, 5, 6],
            committed_operation_digests: vec![[0x33; 32]],
            rejected_operation_digests: Vec::new(),
            revoked_issuer_public_keys: vec![[0x44; 32]],
        }));
        let json = norito::json::to_string(&response).expect("serialize response");
        assert!(json.contains("\"block_height\":7"));
        assert!(json.contains("\"canonical_commitment_root_base64url\":\"AQID\""));
        for forbidden in [
            "credential",
            "witness",
            "holder_secret",
            "attestation",
            "applicant",
        ] {
            assert!(!json.contains(forbidden));
        }
    }
}
