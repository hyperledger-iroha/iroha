//! Fail-closed construction of the config-bound SoraFS PoP runtime.
//!
//! The daemon owns only the public configuration-to-registry binding. Private
//! provider material remains behind the deployment-supplied registry.

use std::{fmt, sync::Arc};

use iroha_config::parameters::actual::SorafsPopCredentialService;
use iroha_torii::sorafs::pop_api::{
    PopCredentialRuntimeConfigV1, PopCredentialRuntimeProviderRegistryV1,
    PopCredentialToriiRuntimeV1,
};
use sorafs_node::pop_credentials::PopCredentialServiceError;

/// Fail-closed `PoP` runtime startup failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopRuntimeStartupError {
    /// A provider registry was injected while the service is disabled.
    UnexpectedProviderRegistry,
    /// The enabled service was not supplied its deployment provider registry.
    MissingProviderRegistry,
    /// The injected registry or its resolved providers failed qualification.
    Runtime(PopCredentialServiceError),
}

impl fmt::Display for PopRuntimeStartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedProviderRegistry => {
                formatter.write_str("PoP provider registry injected while service is disabled")
            }
            Self::MissingProviderRegistry => {
                formatter.write_str("enabled PoP service requires a provider registry")
            }
            Self::Runtime(error) => write!(formatter, "PoP runtime qualification failed: {error}"),
        }
    }
}

impl std::error::Error for PopRuntimeStartupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            Self::UnexpectedProviderRegistry | Self::MissingProviderRegistry => None,
        }
    }
}

/// Build the `PoP` runtime from public config and one deployment-owned registry.
///
/// The Torii constructor validates the exact configured provider handle,
/// revision, policy digest, resolved HSM identity, both protected recipient
/// identities, and wallet wrapping-key identity before opening durable service
/// state. It also guards every resolved provider operation against
/// qualification drift.
///
/// # Errors
///
/// Returns an error for either inconsistent enabled/injected state or any
/// failed runtime-provider qualification. No fallback provider is installed.
pub fn build(
    config: Option<&SorafsPopCredentialService>,
    provider_registry: Option<Arc<dyn PopCredentialRuntimeProviderRegistryV1>>,
) -> Result<Option<Arc<PopCredentialToriiRuntimeV1>>, PopRuntimeStartupError> {
    match (config, provider_registry) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(PopRuntimeStartupError::UnexpectedProviderRegistry),
        (Some(_), None) => Err(PopRuntimeStartupError::MissingProviderRegistry),
        (Some(config), Some(provider_registry)) => {
            let runtime = PopCredentialToriiRuntimeV1::open(
                PopCredentialRuntimeConfigV1::from(config),
                Some(provider_registry),
            )
            .map_err(PopRuntimeStartupError::Runtime)?;
            Ok(Some(Arc::new(runtime)))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use iroha_config::parameters::actual::SorafsPopApprovalSigner;
    use iroha_crypto::{Algorithm, HybridKeyPair, KeyPair};
    use iroha_torii::sorafs::pop_api::{
        PopCredentialRuntimeProviderBindingsV1, PopCredentialRuntimeProviderQualificationV1,
        PopCredentialRuntimeProviderRegistryErrorV1, PopCredentialRuntimeProvidersV1,
        PopFinalizedTimeProviderErrorV1, PopFinalizedTimeProviderV1, PopFinalizedTimeSampleV1,
        PopIssuanceDraftProviderV1, PopPrivateMaterialProviderErrorV1, PopWalletWitnessProviderV1,
    };
    use rand::{SeedableRng as _, rngs::StdRng};
    use sorafs_manifest::{
        hybrid_envelope::{HybridPayloadEnvelopeV1, decrypt_payload},
        pop_credentials::PopMembershipWitnessV1,
    };
    use sorafs_node::pop_credentials::{
        PopAuthenticatedPrincipalV1, PopCredentialApiActionV1, PopCredentialApiAuthenticator,
        PopEnrollmentRecipientV1, PopFinalizedRegistryProjectionV1, PopFinalizedRegistryReader,
        PopIssuanceDraftV1, PopIssuerHsm, PopRecipientOpenErrorV1, PopRegistryOperationV1,
        PopRegistrySubmitter, PopRequestAuthorityV1, PopWalletKeyWrapper, PopWalletRecipientV1,
        pop_enrollment_recipient_public_key_digest_v1,
    };

    use super::*;

    #[derive(Debug)]
    struct FixedIssuerHsm {
        key_id: String,
        public_key: [u8; 32],
    }

    impl PopIssuerHsm for FixedIssuerHsm {
        fn key_id(&self) -> &str {
            &self.key_id
        }

        fn public_key(&self) -> [u8; 32] {
            self.public_key
        }

        fn sign_digest(&self, _digest: [u8; 32]) -> Result<[u8; 64], String> {
            Ok([0x91; 64])
        }
    }

    #[derive(Debug)]
    struct FixedWalletKeyWrapper {
        key_id: String,
    }

    impl PopWalletKeyWrapper for FixedWalletKeyWrapper {
        fn active_key_id(&self) -> &str {
            &self.key_id
        }

        fn wrap_dek(&self, _context: [u8; 32], dek: &[u8; 32]) -> Result<Vec<u8>, String> {
            Ok(dek.to_vec())
        }

        fn unwrap_dek(
            &self,
            key_id: &str,
            _context: [u8; 32],
            wrapped_dek: &[u8],
        ) -> Result<[u8; 32], String> {
            if key_id != self.key_id {
                return Err("wallet key unavailable".to_owned());
            }
            wrapped_dek
                .try_into()
                .map_err(|_| "invalid wrapped key length".to_owned())
        }
    }

    struct FixedRecipient {
        key_id: String,
        secret: iroha_crypto::HybridSecretKey,
        public_key_digest: [u8; 32],
    }

    impl fmt::Debug for FixedRecipient {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("FixedRecipient")
                .field("key_id", &self.key_id)
                .field("private_key", &"[REDACTED]")
                .finish()
        }
    }

    impl PopEnrollmentRecipientV1 for FixedRecipient {
        fn key_id(&self) -> &str {
            &self.key_id
        }

        fn public_key_digest(&self) -> [u8; 32] {
            self.public_key_digest
        }

        fn open_enrollment(
            &self,
            encrypted_payload: &HybridPayloadEnvelopeV1,
            aad: &[u8],
        ) -> Result<Vec<u8>, PopRecipientOpenErrorV1> {
            decrypt_payload(encrypted_payload, aad, &self.secret)
                .map_err(|_| PopRecipientOpenErrorV1::Rejected)
        }
    }

    impl PopWalletRecipientV1 for FixedRecipient {
        fn key_id(&self) -> &str {
            &self.key_id
        }

        fn public_key_digest(&self) -> [u8; 32] {
            self.public_key_digest
        }

        fn open_wallet_delivery(
            &self,
            encrypted_payload: &HybridPayloadEnvelopeV1,
            aad: &[u8],
        ) -> Result<Vec<u8>, PopRecipientOpenErrorV1> {
            decrypt_payload(encrypted_payload, aad, &self.secret)
                .map_err(|_| PopRecipientOpenErrorV1::Rejected)
        }
    }

    #[derive(Debug)]
    struct FixedAuthenticator;

    impl PopCredentialApiAuthenticator for FixedAuthenticator {
        fn authenticate(
            &self,
            _opaque_credential: &[u8],
            _action: PopCredentialApiActionV1,
            _request_binding: [u8; 32],
            _now_epoch: u64,
        ) -> Result<PopAuthenticatedPrincipalV1, String> {
            Ok(PopAuthenticatedPrincipalV1 {
                principal_digest: [0x81; 32],
                expires_at_epoch: 1_000,
                request_authority: PopRequestAuthorityV1::CallerSignedTransaction,
            })
        }
    }

    #[derive(Debug)]
    struct NoopRegistrySubmitter;

    impl PopRegistrySubmitter for NoopRegistrySubmitter {
        fn submit(
            &self,
            _idempotency_key: [u8; 32],
            _operation: &PopRegistryOperationV1,
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
    struct FixedFinalizedTimeProvider;

    impl PopFinalizedTimeProviderV1 for FixedFinalizedTimeProvider {
        fn sample(&self) -> Result<PopFinalizedTimeSampleV1, PopFinalizedTimeProviderErrorV1> {
            Ok(PopFinalizedTimeSampleV1 {
                finalized_block_height: 1,
                finalized_block_hash: [0x82; 32],
                finalized_epoch: 100,
                observed_epoch: 100,
            })
        }
    }

    struct TestProviderRegistry {
        handle: String,
        revision: u64,
        policy_digest: [u8; 32],
        stale_or_revoked: bool,
        drift_after_first_qualification: bool,
        qualification_calls: AtomicUsize,
        providers: Mutex<Option<PopCredentialRuntimeProvidersV1>>,
        observed_bindings: Mutex<Option<PopCredentialRuntimeProviderBindingsV1>>,
    }

    impl fmt::Debug for TestProviderRegistry {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("TestProviderRegistry")
                .field("handle", &self.handle)
                .field("private_providers", &"[REDACTED]")
                .finish_non_exhaustive()
        }
    }

    impl PopCredentialRuntimeProviderRegistryV1 for TestProviderRegistry {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            PopCredentialRuntimeProviderQualificationV1,
            PopCredentialRuntimeProviderRegistryErrorV1,
        > {
            if self.stale_or_revoked {
                return Err(PopCredentialRuntimeProviderRegistryErrorV1::StaleOrRevoked);
            }
            let call = self.qualification_calls.fetch_add(1, Ordering::SeqCst);
            let revision = if self.drift_after_first_qualification && call > 0 {
                self.revision.saturating_add(1)
            } else {
                self.revision
            };
            Ok(PopCredentialRuntimeProviderQualificationV1::new(
                revision,
                self.policy_digest,
            ))
        }

        fn resolve(
            &self,
            bindings: &PopCredentialRuntimeProviderBindingsV1,
        ) -> Result<PopCredentialRuntimeProvidersV1, PopCredentialRuntimeProviderRegistryErrorV1>
        {
            *self
                .observed_bindings
                .lock()
                .map_err(|_| PopCredentialRuntimeProviderRegistryErrorV1::Unavailable)? =
                Some(bindings.clone());
            self.providers
                .lock()
                .map_err(|_| PopCredentialRuntimeProviderRegistryErrorV1::Unavailable)?
                .take()
                .ok_or(PopCredentialRuntimeProviderRegistryErrorV1::Unavailable)
        }
    }

    fn ed25519_public_key(seed: u8) -> [u8; 32] {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive fixture Ed25519 key")
            .public_key()
            .to_bytes()
            .1
            .try_into()
            .expect("Ed25519 public key width")
    }

    fn enrollment_recipient_key() -> HybridKeyPair {
        let mut rng = StdRng::from_seed([0x31; 32]);
        HybridKeyPair::generate(&mut rng).expect("deterministic enrollment recipient key")
    }

    fn wallet_recipient_key() -> HybridKeyPair {
        let mut rng = StdRng::from_seed([0x32; 32]);
        HybridKeyPair::generate(&mut rng).expect("deterministic wallet recipient key")
    }

    fn service_config(root: &Path) -> SorafsPopCredentialService {
        let enrollment_recipient = enrollment_recipient_key();
        let wallet_recipient = wallet_recipient_key();
        SorafsPopCredentialService {
            issuer_state_dir: root.join("issuer"),
            wallet_state_dir: root.join("wallet"),
            issuer_policy_digest: [0x51; 32],
            issuer_id: "pop-issuer-runtime-primary".to_owned(),
            issuer_hsm_key_id: "pkcs11:pop/issuer:primary".to_owned(),
            issuer_public_key: ed25519_public_key(0x41),
            enrollment_recipient_key_id: "kms:pop/enrollment:primary".to_owned(),
            enrollment_recipient_public_key_digest: pop_enrollment_recipient_public_key_digest_v1(
                enrollment_recipient.public(),
            ),
            wallet_recipient_key_id: "kms:pop/wallet-recipient:primary".to_owned(),
            wallet_recipient_public_key_digest: pop_enrollment_recipient_public_key_digest_v1(
                wallet_recipient.public(),
            ),
            wallet_wrapping_key_id: "kms:pop/wallet:primary".to_owned(),
            runtime_provider_registry_handle: "runtime:pop:providers:primary".to_owned(),
            runtime_provider_registry_revision: 7,
            runtime_provider_registry_policy_digest: [0x61; 32],
            approval_quorum: 2,
            approval_signers: vec![
                SorafsPopApprovalSigner {
                    signer_id: "approver-a".to_owned(),
                    public_key: ed25519_public_key(0x42),
                    revoked_at_epoch: None,
                },
                SorafsPopApprovalSigner {
                    signer_id: "approver-b".to_owned(),
                    public_key: ed25519_public_key(0x43),
                    revoked_at_epoch: None,
                },
            ],
            max_pending_enrollments: 16,
            max_outbox_entries: 16,
            max_dead_letters: 16,
            max_seen_nullifiers: 16,
            max_submission_attempts: 3,
            worker_interval: Duration::from_secs(1),
            max_finalized_time_skew: Duration::from_secs(30),
        }
    }

    fn temporary_service_config(temporary: &tempfile::TempDir) -> SorafsPopCredentialService {
        let canonical_root = temporary
            .path()
            .canonicalize()
            .expect("canonical temporary runtime root");
        service_config(&canonical_root)
    }

    fn provider_registry(
        config: &SorafsPopCredentialService,
        handle: impl Into<String>,
        revision: u64,
        policy_digest: [u8; 32],
        stale_or_revoked: bool,
        drift_after_first_qualification: bool,
    ) -> Arc<TestProviderRegistry> {
        let enrollment_recipient = enrollment_recipient_key();
        let wallet_recipient = wallet_recipient_key();
        Arc::new(TestProviderRegistry {
            handle: handle.into(),
            revision,
            policy_digest,
            stale_or_revoked,
            drift_after_first_qualification,
            qualification_calls: AtomicUsize::new(0),
            providers: Mutex::new(Some(PopCredentialRuntimeProvidersV1 {
                enrollment_recipient: Arc::new(FixedRecipient {
                    key_id: config.enrollment_recipient_key_id.clone(),
                    secret: enrollment_recipient.secret().clone(),
                    public_key_digest: config.enrollment_recipient_public_key_digest,
                }),
                issuer_hsm: Arc::new(FixedIssuerHsm {
                    key_id: config.issuer_hsm_key_id.clone(),
                    public_key: config.issuer_public_key,
                }),
                authenticator: Arc::new(FixedAuthenticator),
                registry_submitter: Arc::new(NoopRegistrySubmitter),
                registry_reader: Arc::new(EmptyRegistryReader),
                issuance_draft_provider: Arc::new(UnavailableIssuanceDraftProvider),
                wallet_recipient: Arc::new(FixedRecipient {
                    key_id: config.wallet_recipient_key_id.clone(),
                    secret: wallet_recipient.secret().clone(),
                    public_key_digest: config.wallet_recipient_public_key_digest,
                }),
                wallet_key_wrapper: Arc::new(FixedWalletKeyWrapper {
                    key_id: config.wallet_wrapping_key_id.clone(),
                }),
                wallet_witness_provider: Arc::new(UnavailableWalletWitnessProvider),
                finalized_time_provider: Arc::new(FixedFinalizedTimeProvider),
            })),
            observed_bindings: Mutex::new(None),
        })
    }

    fn erased_registry(
        registry: &Arc<TestProviderRegistry>,
    ) -> Arc<dyn PopCredentialRuntimeProviderRegistryV1> {
        registry.clone()
    }

    fn exact_registry(config: &SorafsPopCredentialService) -> Arc<TestProviderRegistry> {
        provider_registry(
            config,
            config.runtime_provider_registry_handle.clone(),
            config.runtime_provider_registry_revision,
            config.runtime_provider_registry_policy_digest,
            false,
            false,
        )
    }

    fn assert_startup_failure_before_state(
        config: &SorafsPopCredentialService,
        registry: &Arc<TestProviderRegistry>,
        expected: PopRuntimeStartupError,
    ) {
        let issuer_state_dir = config.issuer_state_dir.clone();
        let wallet_state_dir = config.wallet_state_dir.clone();
        let error = build(Some(config), Some(erased_registry(registry)))
            .expect_err("PoP startup must fail");
        assert_eq!(error, expected);
        assert!(!issuer_state_dir.exists());
        assert!(!wallet_state_dir.exists());
    }

    #[test]
    fn builder_covers_all_enabled_and_injected_combinations() {
        assert!(build(None, None).expect("disabled PoP service").is_none());

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let config = temporary_service_config(&temporary);
        let registry = exact_registry(&config);
        assert_eq!(
            build(None, Some(erased_registry(&registry))).err(),
            Some(PopRuntimeStartupError::UnexpectedProviderRegistry)
        );
        assert!(!config.issuer_state_dir.exists());
        assert!(!config.wallet_state_dir.exists());

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let config = temporary_service_config(&temporary);
        assert_eq!(
            build(Some(&config), None).err(),
            Some(PopRuntimeStartupError::MissingProviderRegistry)
        );
        assert!(!config.issuer_state_dir.exists());
        assert!(!config.wallet_state_dir.exists());

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let config = temporary_service_config(&temporary);
        let registry = exact_registry(&config);
        let runtime = build(Some(&config), Some(erased_registry(&registry)))
            .expect("exact provider registry must qualify")
            .expect("enabled service must build a runtime");
        assert_eq!(
            runtime.config().runtime_provider_registry_handle,
            config.runtime_provider_registry_handle
        );
        assert_eq!(
            runtime.config().runtime_provider_registry_revision,
            config.runtime_provider_registry_revision
        );
        assert_eq!(
            runtime.config().runtime_provider_registry_policy_digest,
            config.runtime_provider_registry_policy_digest
        );
        assert!(config.issuer_state_dir.exists());
        assert!(config.wallet_state_dir.exists());

        let observed = registry
            .observed_bindings
            .lock()
            .expect("observed binding lock")
            .clone()
            .expect("registry must receive the exact public bindings");
        assert_eq!(observed.issuer_policy_digest(), config.issuer_policy_digest);
        assert_eq!(observed.issuer_id(), config.issuer_id);
        assert_eq!(observed.issuer_hsm_key_id(), config.issuer_hsm_key_id);
        assert_eq!(observed.issuer_public_key(), config.issuer_public_key);
        assert_eq!(
            observed.enrollment_recipient_key_id(),
            config.enrollment_recipient_key_id
        );
        assert_eq!(
            observed.enrollment_recipient_public_key_digest(),
            config.enrollment_recipient_public_key_digest
        );
        assert_eq!(
            observed.wallet_recipient_key_id(),
            config.wallet_recipient_key_id
        );
        assert_eq!(
            observed.wallet_recipient_public_key_digest(),
            config.wallet_recipient_public_key_digest
        );
        assert_eq!(
            observed.wallet_wrapping_key_id(),
            config.wallet_wrapping_key_id
        );
    }

    #[test]
    fn builder_rejects_substituted_stale_revoked_test_marked_and_drifting_registries() {
        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let config = temporary_service_config(&temporary);
        let registry = provider_registry(
            &config,
            "runtime:pop:providers:secondary",
            config.runtime_provider_registry_revision,
            config.runtime_provider_registry_policy_digest,
            false,
            false,
        );
        assert_startup_failure_before_state(
            &config,
            &registry,
            PopRuntimeStartupError::Runtime(
                PopCredentialServiceError::RuntimeProviderRegistryMismatch,
            ),
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let config = temporary_service_config(&temporary);
        let registry = provider_registry(
            &config,
            config.runtime_provider_registry_handle.clone(),
            config.runtime_provider_registry_revision + 1,
            config.runtime_provider_registry_policy_digest,
            false,
            false,
        );
        assert_startup_failure_before_state(
            &config,
            &registry,
            PopRuntimeStartupError::Runtime(
                PopCredentialServiceError::RuntimeProviderRegistryMismatch,
            ),
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let config = temporary_service_config(&temporary);
        let registry = provider_registry(
            &config,
            config.runtime_provider_registry_handle.clone(),
            config.runtime_provider_registry_revision,
            [0x62; 32],
            false,
            false,
        );
        assert_startup_failure_before_state(
            &config,
            &registry,
            PopRuntimeStartupError::Runtime(
                PopCredentialServiceError::RuntimeProviderRegistryMismatch,
            ),
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let config = temporary_service_config(&temporary);
        let registry = provider_registry(
            &config,
            config.runtime_provider_registry_handle.clone(),
            config.runtime_provider_registry_revision,
            config.runtime_provider_registry_policy_digest,
            true,
            false,
        );
        assert_startup_failure_before_state(
            &config,
            &registry,
            PopRuntimeStartupError::Runtime(
                PopCredentialServiceError::RuntimeProviderRegistryUnavailable,
            ),
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let mut config = temporary_service_config(&temporary);
        config.runtime_provider_registry_handle = "runtime:pop:providers:test".to_owned();
        let registry = exact_registry(&config);
        assert_startup_failure_before_state(
            &config,
            &registry,
            PopRuntimeStartupError::Runtime(PopCredentialServiceError::InvalidInput {
                field: "runtime_provider_registry_handle",
            }),
        );

        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let config = temporary_service_config(&temporary);
        let registry = provider_registry(
            &config,
            config.runtime_provider_registry_handle.clone(),
            config.runtime_provider_registry_revision,
            config.runtime_provider_registry_policy_digest,
            false,
            true,
        );
        assert_startup_failure_before_state(
            &config,
            &registry,
            PopRuntimeStartupError::Runtime(
                PopCredentialServiceError::RuntimeProviderRegistryDrift,
            ),
        );
    }

    #[test]
    fn production_builder_has_no_secret_or_fallback_source() {
        let source = include_str!("sorafs_pop_runtime.rs");
        let production_source = source
            .split_once("#[cfg(test)]")
            .map_or(source, |(production, _)| production);
        for forbidden in [
            "std::env",
            "std::fs",
            "SystemTime",
            "KeyPair",
            "PrivateKey",
            "HybridSecretKey",
            "PopCredentialRuntimeSecretsV1",
            "config.common.key_pair",
            "read_to_string",
            "from_env",
        ] {
            assert!(
                !production_source.contains(forbidden),
                "production PoP builder must not contain `{forbidden}`"
            );
        }
    }

    #[test]
    fn startup_error_exposes_only_stable_payload_free_text() {
        let error = PopRuntimeStartupError::Runtime(
            PopCredentialServiceError::RuntimeProviderRegistryMismatch,
        );
        let rendered = error.to_string();
        assert_eq!(
            rendered,
            "PoP runtime qualification failed: PoP runtime provider registry does not match configured policy"
        );
        for private_detail in [
            "runtime:pop:providers:primary",
            "pkcs11:pop/issuer:primary",
            "kms:pop/wallet:primary",
            "credential",
            "secret",
        ] {
            assert!(!rendered.contains(private_detail));
        }
    }
}
