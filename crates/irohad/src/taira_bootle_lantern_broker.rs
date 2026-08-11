//! Native Taira Bootle/Lantern issuer broker and public-policy exporter.
//!
//! The executable built on this module has one deliberately narrow role: it
//! exposes slot 56 through the stock authenticated local broker while keeping
//! the Falcon trapdoor, bearer token, and stable principal seed in hardened
//! service-credential files. Torii remains the only issuance replay-state authority.

use std::{
    fmt,
    fs::File,
    io::{Read as _, Write as _},
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use crate::{
    BootleLanternIssuanceBrokerBackendErrorV1, BootleLanternIssuanceBrokerBackendV1,
    IrohaRuntimeProviderBindingsV1, RuntimeProviderBrokerBackendsV1,
    RuntimeProviderBrokerLifecycleV1, serve_runtime_provider_broker_with_lifecycle_v1,
};
use clap::{Args, Parser, Subcommand};
use iroha_config::parameters::validate_production_runtime_handle;
use iroha_core::privacy_engines::bootle_lantern::issuer::{
    BootleLanternBlindIssuanceResponseV1, BootleLanternIssuanceAuthorizationV1,
    BootleLanternIssuanceErrorV1, BootleLanternIssuerKeyPairV1,
    BootleLanternIssuerPolicyMetadataV1, MAX_BOOTLE_LANTERN_AUTHORIZATION_LIFETIME_BLOCKS_V1,
    TairaBootleLanternBrokerQualificationErrorV1, TairaBootleLanternBrokerQualificationInputsV1,
    derive_taira_bootle_lantern_broker_qualification_digest_v1,
    issuer_issue_validated_blind_issuance_request_encoded_v1,
    issuer_prepare_blind_issuance_authorization_candidate_v1,
    issuer_validate_blind_issuance_request_for_issuer_encoded_v1,
    taira_bootle_lantern_broker_contract_digest_v1,
    taira_bootle_lantern_issuer_profile_contract_digest_v1,
};
use iroha_data_model::{
    ChainId, NetworkId,
    isi::{InstructionBox, privacy::RegisterPrivacyBootleLanternIssuerPolicyV1},
    privacy::{
        BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1, BootleLanternAllowedAttributeValuesV1,
        BootleLanternIssuerPolicyV1, PrivacyIssuerIdV1, PrivacyParameterIdV1, PrivacyPolicyIdV1,
        PrivacyStatementContextV1,
    },
};
use iroha_torii::privacy_issuance_api::{
    BootleLanternIssuanceActionV1, BootleLanternIssuanceAuthenticatedPrincipalV1,
    BootleLanternIssuanceAuthenticationErrorV1, BootleLanternIssuanceRuntimeProviderBindingsV1,
    BootleLanternIssuanceRuntimeProviderQualificationV1,
    BootleLanternIssuanceRuntimeProviderRegistryErrorV1,
};

const ISSUER_SEED_BYTES_V1: usize = 32;
const PRINCIPAL_SEED_BYTES_V1: usize = 32;
const MIN_BEARER_TOKEN_BYTES_V1: usize = 32;
const MAX_BEARER_TOKEN_BYTES_V1: usize = 4_096;
const CREDENTIAL_FILE_MODE_V1: u32 = 0o400;
const MAX_CREDENTIAL_PATH_COMPONENTS_V1: usize = 64;
#[cfg(target_os = "linux")]
const SYSTEMD_CREDENTIAL_DIRECTORY_V1: &str =
    "/run/credentials/taira-bootle-lantern-broker.service";

const PARAMETER_ID_DOMAIN_V1: &[u8] = b"iroha.taira.privacy.bootle-lantern.issuer-parameter-id.v1";
const PRINCIPAL_DIGEST_DOMAIN_V1: &[u8] = b"iroha.taira.privacy.bootle-lantern.stable-principal.v1";
const BEARER_TOKEN_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.taira.privacy.bootle-lantern.opaque-bearer-digest.v1";

/// Stable payload-free failure from the standalone Taira broker launcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TairaBootleLanternBrokerErrorV1 {
    /// A public CLI binding was malformed, weak, or internally inconsistent.
    InvalidPublicBinding,
    /// A credential path or credential value failed closed validation.
    CredentialRejected,
    /// Native issuer construction or cryptographic randomness was unavailable.
    CryptographyUnavailable,
    /// Canonical Norito or JSON export encoding failed.
    EncodingFailed,
    /// A serve-time expected public digest did not match derived state.
    ExpectedDigestMismatch,
    /// Writing the public export failed.
    OutputFailed,
    /// The stock authenticated broker could not start or stopped with an error.
    BrokerFailed,
    /// Secure credential loading is unsupported on this platform.
    UnsupportedPlatform,
}

impl fmt::Display for TairaBootleLanternBrokerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPublicBinding => "Taira Bootle/Lantern public binding is invalid",
            Self::CredentialRejected => "Taira Bootle/Lantern credential was rejected",
            Self::CryptographyUnavailable => {
                "Taira Bootle/Lantern issuer cryptography is unavailable"
            }
            Self::EncodingFailed => "Taira Bootle/Lantern public export encoding failed",
            Self::ExpectedDigestMismatch => {
                "Taira Bootle/Lantern expected public digest does not match"
            }
            Self::OutputFailed => "Taira Bootle/Lantern public export could not be written",
            Self::BrokerFailed => "Taira Bootle/Lantern broker failed",
            Self::UnsupportedPlatform => {
                "Taira Bootle/Lantern secure credentials are unsupported on this platform"
            }
        })
    }
}

impl std::error::Error for TairaBootleLanternBrokerErrorV1 {}

/// Exact public deployment inputs for one Taira issuer broker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TairaBootleLanternBrokerPublicConfigV1 {
    chain_id: ChainId,
    network_id: NetworkId,
    handle: String,
    revision: u64,
    issuer_id: PrivacyIssuerIdV1,
    policy_id: PrivacyPolicyIdV1,
    authorization_lifetime_blocks: u64,
}

impl TairaBootleLanternBrokerPublicConfigV1 {
    /// Validate and construct exact public broker inputs.
    ///
    /// # Errors
    ///
    /// Rejects test/default-marked handles, zero or weak identities, a zero
    /// revision, and a lifetime outside the native first-release bound.
    pub fn try_new(
        chain_id: ChainId,
        network_id: NetworkId,
        handle: impl Into<String>,
        revision: u64,
        issuer_id: PrivacyIssuerIdV1,
        policy_id: PrivacyPolicyIdV1,
        authorization_lifetime_blocks: u64,
    ) -> Result<Self, TairaBootleLanternBrokerErrorV1> {
        let handle = handle.into();
        if validate_production_runtime_handle(&handle).is_err()
            || contains_forbidden_public_marker_v1(&handle)
            || network_id.as_bytes().iter().all(|byte| *byte == 0)
            || revision == 0
            || !is_strong_public_digest_v1(issuer_id.as_bytes())
            || !is_strong_public_digest_v1(policy_id.as_bytes())
            || authorization_lifetime_blocks == 0
            || authorization_lifetime_blocks > MAX_BOOTLE_LANTERN_AUTHORIZATION_LIFETIME_BLOCKS_V1
        {
            return Err(TairaBootleLanternBrokerErrorV1::InvalidPublicBinding);
        }
        BootleLanternIssuanceRuntimeProviderBindingsV1::try_new(
            issuer_id,
            policy_id,
            authorization_lifetime_blocks,
        )
        .map_err(|_| TairaBootleLanternBrokerErrorV1::InvalidPublicBinding)?;
        Ok(Self {
            chain_id,
            network_id,
            handle,
            revision,
            issuer_id,
            policy_id,
            authorization_lifetime_blocks,
        })
    }

    /// Human-readable chain label retained for catalog metadata and display.
    ///
    /// Security bindings use [`Self::network_id`] and never this label alone.
    #[must_use]
    pub fn chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    /// Exact genesis-header-derived identity used by every security binding.
    #[must_use]
    pub const fn network_id(&self) -> NetworkId {
        self.network_id
    }

    /// Exact production provider handle.
    #[must_use]
    pub fn handle(&self) -> &str {
        &self.handle
    }

    /// Exact non-zero provider-policy revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Exact governed issuer identity.
    #[must_use]
    pub const fn issuer_id(&self) -> PrivacyIssuerIdV1 {
        self.issuer_id
    }

    /// Exact governed issuer-policy identity.
    #[must_use]
    pub const fn policy_id(&self) -> PrivacyPolicyIdV1 {
        self.policy_id
    }

    /// Exact authorization lifetime in committed block heights.
    #[must_use]
    pub const fn authorization_lifetime_blocks(&self) -> u64 {
        self.authorization_lifetime_blocks
    }

    fn bindings(
        &self,
    ) -> Result<BootleLanternIssuanceRuntimeProviderBindingsV1, TairaBootleLanternBrokerErrorV1>
    {
        BootleLanternIssuanceRuntimeProviderBindingsV1::try_new(
            self.issuer_id,
            self.policy_id,
            self.authorization_lifetime_blocks,
        )
        .map_err(|_| TairaBootleLanternBrokerErrorV1::InvalidPublicBinding)
    }
}

struct SecretMaterialV1 {
    bytes: Vec<u8>,
}

impl SecretMaterialV1 {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn as_exact_32(&self) -> Option<&[u8; 32]> {
        self.bytes.as_slice().try_into().ok()
    }

    fn scrub(&mut self) {
        self.bytes.fill(0);
        let _ = std::hint::black_box(&self.bytes);
    }
}

impl fmt::Debug for SecretMaterialV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretMaterialV1")
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

impl Drop for SecretMaterialV1 {
    fn drop(&mut self) {
        self.scrub();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CredentialFileIdentityV1 {
    device: u64,
    inode: u64,
}

struct OpenedCredentialV1 {
    secret: SecretMaterialV1,
    identity: CredentialFileIdentityV1,
}

struct CredentialBundleV1 {
    issuer_seed: SecretMaterialV1,
    bearer_token: SecretMaterialV1,
    principal_seed: SecretMaterialV1,
}

impl CredentialBundleV1 {
    fn load(
        issuer_seed_path: &Path,
        bearer_token_path: &Path,
        principal_seed_path: &Path,
    ) -> Result<Self, TairaBootleLanternBrokerErrorV1> {
        let issuer_seed =
            load_credential_v1(issuer_seed_path, ISSUER_SEED_BYTES_V1, ISSUER_SEED_BYTES_V1)?;
        let bearer_token = load_credential_v1(
            bearer_token_path,
            MIN_BEARER_TOKEN_BYTES_V1,
            MAX_BEARER_TOKEN_BYTES_V1,
        )?;
        let principal_seed = load_credential_v1(
            principal_seed_path,
            PRINCIPAL_SEED_BYTES_V1,
            PRINCIPAL_SEED_BYTES_V1,
        )?;
        if issuer_seed.identity == bearer_token.identity
            || issuer_seed.identity == principal_seed.identity
            || bearer_token.identity == principal_seed.identity
            || is_weak_secret_v1(issuer_seed.secret.as_bytes())
            || is_weak_secret_v1(bearer_token.secret.as_bytes())
            || is_weak_secret_v1(principal_seed.secret.as_bytes())
        {
            return Err(TairaBootleLanternBrokerErrorV1::CredentialRejected);
        }
        Ok(Self {
            issuer_seed: issuer_seed.secret,
            bearer_token: bearer_token.secret,
            principal_seed: principal_seed.secret,
        })
    }
}

/// Native deployment-owned implementation of broker slot 56.
pub struct TairaBootleLanternIssuanceBrokerBackendV1 {
    config: TairaBootleLanternBrokerPublicConfigV1,
    issuer: BootleLanternIssuerKeyPairV1,
    policy: BootleLanternIssuerPolicyV1,
    bearer_token_digest: [u8; 32],
    principal_digest: [u8; 32],
    qualification: BootleLanternIssuanceRuntimeProviderQualificationV1,
}

impl fmt::Debug for TairaBootleLanternIssuanceBrokerBackendV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TairaBootleLanternIssuanceBrokerBackendV1")
            .field("handle", &self.config.handle)
            .field("revision", &self.config.revision)
            .field("issuer_id", &self.config.issuer_id)
            .field("policy_id", &self.config.policy_id)
            .field("private_material", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl TairaBootleLanternIssuanceBrokerBackendV1 {
    /// Load exactly three hardened service credentials and construct the native backend.
    ///
    /// Credential contents are read through immutable opened descriptors and
    /// retained in memory; rotating a pathname cannot silently rotate a live
    /// process. Operators must restart the broker to activate a credential
    /// rotation.
    ///
    /// # Errors
    ///
    /// Rejects every unsafe path, metadata or size mismatch, weak credential,
    /// duplicate credential inode, invalid key/policy, or encoding failure.
    pub fn load_from_hardened_service_credentials_v1(
        config: TairaBootleLanternBrokerPublicConfigV1,
        issuer_seed_path: &Path,
        bearer_token_path: &Path,
        principal_seed_path: &Path,
    ) -> Result<Self, TairaBootleLanternBrokerErrorV1> {
        let credentials =
            CredentialBundleV1::load(issuer_seed_path, bearer_token_path, principal_seed_path)?;
        Self::from_credentials_v1(config, credentials)
    }

    fn from_credentials_v1(
        config: TairaBootleLanternBrokerPublicConfigV1,
        mut credentials: CredentialBundleV1,
    ) -> Result<Self, TairaBootleLanternBrokerErrorV1> {
        let mut issuer_seed = *credentials
            .issuer_seed
            .as_exact_32()
            .ok_or(TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
        let issuer_result = (|| {
            let parameter_id = derive_nonzero_digest_v1(
                PARAMETER_ID_DOMAIN_V1,
                &[
                    config.network_id.as_bytes(),
                    config.handle.as_bytes(),
                    &config.revision.to_be_bytes(),
                    config.issuer_id.as_bytes(),
                    config.policy_id.as_bytes(),
                    &issuer_seed,
                ],
            )?;
            BootleLanternIssuerKeyPairV1::generate_from_secret_seed_v1(
                PrivacyParameterIdV1::new(parameter_id),
                &issuer_seed,
            )
            .map_err(map_startup_crypto_error_v1)
        })();
        issuer_seed.fill(0);
        let _ = std::hint::black_box(&issuer_seed);
        credentials.issuer_seed.scrub();
        let issuer = issuer_result?;
        let policy = issuer
            .active_policy_v1(BootleLanternIssuerPolicyMetadataV1 {
                issuer_id: config.issuer_id,
                policy_id: config.policy_id,
                epoch: 1,
                required_disclosure_bitmap: 0,
                allowed_values: (0..BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1)
                    .map(|_| BootleLanternAllowedAttributeValuesV1 { values: Vec::new() })
                    .collect(),
            })
            .map_err(map_startup_crypto_error_v1)?;
        policy
            .validate_initial()
            .map_err(|_| TairaBootleLanternBrokerErrorV1::InvalidPublicBinding)?;

        let principal_digest_result = derive_nonzero_digest_v1(
            PRINCIPAL_DIGEST_DOMAIN_V1,
            &[
                config.network_id.as_bytes(),
                config.issuer_id.as_bytes(),
                config.policy_id.as_bytes(),
                credentials.principal_seed.as_bytes(),
            ],
        );
        credentials.principal_seed.scrub();
        let principal_digest = principal_digest_result?;
        let bearer_token_digest_result = derive_nonzero_digest_v1(
            BEARER_TOKEN_DIGEST_DOMAIN_V1,
            &[credentials.bearer_token.as_bytes()],
        );
        credentials.bearer_token.scrub();
        let bearer_token_digest = bearer_token_digest_result?;
        let qualification_digest = derive_taira_bootle_lantern_broker_qualification_digest_v1(
            &TairaBootleLanternBrokerQualificationInputsV1 {
                network_id: config.network_id,
                runtime_provider_handle: &config.handle,
                runtime_provider_revision: config.revision,
                issuer_id: config.issuer_id,
                policy_id: config.policy_id,
                authorization_lifetime_blocks: config.authorization_lifetime_blocks,
                policy: &policy,
                stable_principal_digest: principal_digest,
            },
        )
        .map_err(map_qualification_error_v1)?;
        let qualification = BootleLanternIssuanceRuntimeProviderQualificationV1::new(
            config.revision,
            qualification_digest,
        );
        Ok(Self {
            config,
            issuer,
            policy,
            bearer_token_digest,
            principal_digest,
            qualification,
        })
    }

    /// Complete governed issuer policy exported by this backend.
    #[must_use]
    pub fn policy(&self) -> &BootleLanternIssuerPolicyV1 {
        &self.policy
    }

    /// Stable public principal commitment, independent of bearer rotation.
    #[must_use]
    pub const fn principal_digest(&self) -> [u8; 32] {
        self.principal_digest
    }

    /// Exact provider qualification derived from public policy and executable contracts.
    #[must_use]
    pub const fn public_qualification(
        &self,
    ) -> BootleLanternIssuanceRuntimeProviderQualificationV1 {
        self.qualification
    }

    fn validate_expected_digests_v1(
        &self,
        expected_policy_record_digest: [u8; 32],
        expected_qualification_policy_digest: [u8; 32],
    ) -> Result<(), TairaBootleLanternBrokerErrorV1> {
        if !constant_time_equal_fixed_v1(
            self.policy.record_digest.as_bytes(),
            &expected_policy_record_digest,
        ) || !constant_time_equal_fixed_v1(
            &self.qualification.policy_digest,
            &expected_qualification_policy_digest,
        ) {
            return Err(TairaBootleLanternBrokerErrorV1::ExpectedDigestMismatch);
        }
        Ok(())
    }

    fn render_public_export_v1(&self) -> Result<String, TairaBootleLanternBrokerErrorV1> {
        let instruction = RegisterPrivacyBootleLanternIssuerPolicyV1::new(self.policy.clone());
        let instruction_box = InstructionBox::from(instruction.clone());
        let instruction_bytes = norito::to_bytes(&instruction_box)
            .map_err(|_| TairaBootleLanternBrokerErrorV1::EncodingFailed)?;
        let instruction_sha256 = iroha_crypto::sha256(&instruction_bytes);
        let profile_digest = taira_bootle_lantern_issuer_profile_contract_digest_v1();
        let contract_digest = taira_bootle_lantern_broker_contract_digest_v1();
        let instruction_json = norito::json::to_value(&instruction)
            .map_err(|_| TairaBootleLanternBrokerErrorV1::EncodingFailed)?;
        let mut export = norito::json::Map::new();
        export.insert(
            "schema".into(),
            norito::json::Value::from("iroha.taira.privacy.bootle-lantern-broker-public.v1"),
        );
        export.insert(
            "chain_id".into(),
            norito::json::Value::from(self.config.chain_id.to_string()),
        );
        export.insert(
            "network_id".into(),
            norito::json::Value::from(self.config.network_id.to_string()),
        );
        export.insert(
            "runtime_provider_handle".into(),
            norito::json::Value::from(self.config.handle.clone()),
        );
        export.insert(
            "runtime_provider_revision".into(),
            norito::json::Value::from(self.config.revision),
        );
        export.insert(
            "runtime_provider_policy_digest_hex".into(),
            norito::json::Value::from(hex::encode(self.qualification.policy_digest)),
        );
        export.insert(
            "issuer_id_hex".into(),
            norito::json::Value::from(hex::encode(self.config.issuer_id.as_bytes())),
        );
        export.insert(
            "policy_id_hex".into(),
            norito::json::Value::from(hex::encode(self.config.policy_id.as_bytes())),
        );
        export.insert(
            "authorization_lifetime_blocks".into(),
            norito::json::Value::from(self.config.authorization_lifetime_blocks),
        );
        export.insert(
            "issuer_parameter_id_hex".into(),
            norito::json::Value::from(hex::encode(self.policy.issuer_parameter_id.as_bytes())),
        );
        export.insert(
            "issuer_parameter_digest_hex".into(),
            norito::json::Value::from(hex::encode(self.policy.issuer_parameter_digest.as_bytes())),
        );
        export.insert(
            "policy_record_digest_hex".into(),
            norito::json::Value::from(hex::encode(self.policy.record_digest.as_bytes())),
        );
        export.insert(
            "stable_principal_digest_hex".into(),
            norito::json::Value::from(hex::encode(self.principal_digest)),
        );
        export.insert(
            "issuer_profile_digest_hex".into(),
            norito::json::Value::from(hex::encode(profile_digest)),
        );
        export.insert(
            "broker_contract_digest_hex".into(),
            norito::json::Value::from(hex::encode(contract_digest)),
        );
        export.insert(
            "registration_instruction_norito_hex".into(),
            norito::json::Value::from(hex::encode(&instruction_bytes)),
        );
        export.insert(
            "registration_instruction_norito_sha256".into(),
            norito::json::Value::from(hex::encode(instruction_sha256)),
        );
        export.insert("registration_instruction".into(), instruction_json);
        norito::json::to_json(&norito::json::Value::Object(export))
            .map_err(|_| TairaBootleLanternBrokerErrorV1::EncodingFailed)
    }

    fn validate_crypto_call_bindings_v1(
        &self,
        context: &PrivacyStatementContextV1,
        canonical_genesis_hash: [u8; 32],
        policy: &BootleLanternIssuerPolicyV1,
    ) -> Result<(), BootleLanternIssuanceBrokerBackendErrorV1> {
        if policy != &self.policy {
            return Err(BootleLanternIssuanceBrokerBackendErrorV1::PolicyMismatch);
        }
        if context.network_id != self.config.network_id
            || context.network_id.as_bytes() != &canonical_genesis_hash
        {
            return Err(BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest);
        }
        Ok(())
    }

    fn validate_authorization_principal_v1(
        &self,
        authorization: &BootleLanternIssuanceAuthorizationV1,
    ) -> Result<(), BootleLanternIssuanceBrokerBackendErrorV1> {
        if !constant_time_equal_fixed_v1(
            &authorization.requester_authorization_digest(),
            &self.principal_digest,
        ) || authorization.issued_at_height() == 0
            || authorization
                .expires_at_height()
                .checked_sub(authorization.issued_at_height())
                != Some(self.config.authorization_lifetime_blocks)
        {
            return Err(BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest);
        }
        Ok(())
    }
}

impl BootleLanternIssuanceBrokerBackendV1 for TairaBootleLanternIssuanceBrokerBackendV1 {
    fn handle(&self) -> &str {
        &self.config.handle
    }

    fn qualification(
        &self,
    ) -> Result<
        BootleLanternIssuanceRuntimeProviderQualificationV1,
        BootleLanternIssuanceRuntimeProviderRegistryErrorV1,
    > {
        Ok(self.qualification)
    }

    fn bindings(
        &self,
    ) -> Result<
        BootleLanternIssuanceRuntimeProviderBindingsV1,
        BootleLanternIssuanceRuntimeProviderRegistryErrorV1,
    > {
        self.config
            .bindings()
            .map_err(|_| BootleLanternIssuanceRuntimeProviderRegistryErrorV1::RejectedBindings)
    }

    fn authenticate(
        &self,
        opaque_credential: &[u8],
        _action: BootleLanternIssuanceActionV1,
        request_binding: [u8; 32],
        committed_height: u64,
    ) -> Result<
        BootleLanternIssuanceAuthenticatedPrincipalV1,
        BootleLanternIssuanceAuthenticationErrorV1,
    > {
        let candidate_digest =
            hash_length_framed_v1(BEARER_TOKEN_DIGEST_DOMAIN_V1, &[opaque_credential])
                .map_err(|_| BootleLanternIssuanceAuthenticationErrorV1::Unavailable)?;
        let bearer_matches =
            constant_time_equal_fixed_v1(&self.bearer_token_digest, &candidate_digest);
        let public_binding_valid = opaque_credential.len() >= MIN_BEARER_TOKEN_BYTES_V1
            && opaque_credential.len() <= MAX_BEARER_TOKEN_BYTES_V1
            && request_binding != [0; 32]
            && committed_height != 0;
        if !bearer_matches || !public_binding_valid {
            return Err(BootleLanternIssuanceAuthenticationErrorV1::Denied);
        }
        let expires_at_height = committed_height
            .checked_add(self.config.authorization_lifetime_blocks)
            .ok_or(BootleLanternIssuanceAuthenticationErrorV1::Unavailable)?;
        Ok(BootleLanternIssuanceAuthenticatedPrincipalV1 {
            principal_digest: self.principal_digest,
            issued_at_height: committed_height,
            expires_at_height,
        })
    }

    fn prepare_authorization(
        &self,
        context: &PrivacyStatementContextV1,
        canonical_genesis_hash: [u8; 32],
        policy: &BootleLanternIssuerPolicyV1,
        requester_authorization_digest: [u8; 32],
        issued_at_height: u64,
        expires_at_height: u64,
    ) -> Result<BootleLanternIssuanceAuthorizationV1, BootleLanternIssuanceBrokerBackendErrorV1>
    {
        self.validate_crypto_call_bindings_v1(context, canonical_genesis_hash, policy)?;
        if !constant_time_equal_fixed_v1(&requester_authorization_digest, &self.principal_digest)
            || issued_at_height == 0
            || expires_at_height.checked_sub(issued_at_height)
                != Some(self.config.authorization_lifetime_blocks)
        {
            return Err(BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest);
        }
        issuer_prepare_blind_issuance_authorization_candidate_v1(
            &self.issuer,
            context,
            canonical_genesis_hash,
            policy,
            requester_authorization_digest,
            issued_at_height,
            expires_at_height,
        )
        .map_err(map_backend_crypto_error_v1)
    }

    fn validate_request(
        &self,
        context: &PrivacyStatementContextV1,
        canonical_genesis_hash: [u8; 32],
        policy: &BootleLanternIssuerPolicyV1,
        authorization: &BootleLanternIssuanceAuthorizationV1,
        request_bytes: &[u8],
        current_height: u64,
    ) -> Result<[u8; 32], BootleLanternIssuanceBrokerBackendErrorV1> {
        self.validate_crypto_call_bindings_v1(context, canonical_genesis_hash, policy)?;
        self.validate_authorization_principal_v1(authorization)?;
        issuer_validate_blind_issuance_request_for_issuer_encoded_v1(
            &self.issuer,
            context,
            canonical_genesis_hash,
            policy,
            authorization,
            request_bytes,
            current_height,
        )
        .map_err(map_backend_crypto_error_v1)
    }

    fn issue_validated(
        &self,
        context: &PrivacyStatementContextV1,
        canonical_genesis_hash: [u8; 32],
        policy: &BootleLanternIssuerPolicyV1,
        authorization: &BootleLanternIssuanceAuthorizationV1,
        request_bytes: &[u8],
        current_height: u64,
    ) -> Result<BootleLanternBlindIssuanceResponseV1, BootleLanternIssuanceBrokerBackendErrorV1>
    {
        self.validate_crypto_call_bindings_v1(context, canonical_genesis_hash, policy)?;
        self.validate_authorization_principal_v1(authorization)?;
        issuer_issue_validated_blind_issuance_request_encoded_v1(
            &self.issuer,
            context,
            canonical_genesis_hash,
            policy,
            authorization,
            request_bytes,
            current_height,
        )
        .map_err(map_backend_crypto_error_v1)
    }
}

#[derive(Parser)]
#[command(
    name = "taira_bootle_lantern_broker",
    about = "Native Falcon-backed Taira Bootle/Lantern issuance broker",
    disable_help_subcommand = true
)]
struct BrokerCliV1 {
    #[command(subcommand)]
    command: BrokerCommandV1,
}

#[derive(Subcommand)]
enum BrokerCommandV1 {
    /// Emit the complete public policy and registration instruction to stdout.
    ExportPublic(ExportPublicArgsV1),
    /// Validate expected public digests and serve the stock slot-56 endpoint.
    Serve(ServeArgsV1),
}

#[derive(Clone, Args)]
struct PublicArgsV1 {
    /// Human-readable chain name used only for configuration and display.
    #[arg(long)]
    chain_id: ChainId,
    /// Exact genesis-header-derived network identity used for security bindings.
    #[arg(long)]
    network_id: NetworkId,
    /// Stable credential-free production provider handle.
    #[arg(long)]
    handle: String,
    /// Non-zero provider policy revision in canonical decimal.
    #[arg(long, value_parser = parse_canonical_nonzero_u64_v1)]
    revision: u64,
    /// Exact lowercase non-zero 32-byte issuer identity.
    #[arg(long, value_parser = parse_nonzero_digest_hex_v1)]
    issuer_id: [u8; 32],
    /// Exact lowercase non-zero 32-byte issuer-policy identity.
    #[arg(long, value_parser = parse_nonzero_digest_hex_v1)]
    policy_id: [u8; 32],
    /// Exact first-release authorization lifetime in committed heights.
    #[arg(long, value_parser = parse_canonical_nonzero_u64_v1)]
    authorization_lifetime_blocks: u64,
}

impl PublicArgsV1 {
    fn into_config(
        self,
    ) -> Result<TairaBootleLanternBrokerPublicConfigV1, TairaBootleLanternBrokerErrorV1> {
        TairaBootleLanternBrokerPublicConfigV1::try_new(
            self.chain_id,
            self.network_id,
            self.handle,
            self.revision,
            PrivacyIssuerIdV1::new(self.issuer_id),
            PrivacyPolicyIdV1::new(self.policy_id),
            self.authorization_lifetime_blocks,
        )
    }
}

#[derive(Clone, Args)]
struct CredentialPathArgsV1 {
    /// Absolute hardened credential path containing exactly 32 issuer-seed bytes.
    #[arg(long)]
    issuer_seed_credential: PathBuf,
    /// Absolute hardened credential path containing the exact opaque bearer bytes.
    #[arg(long)]
    bearer_token_credential: PathBuf,
    /// Absolute hardened credential path containing exactly 32 stable-principal bytes.
    #[arg(long)]
    principal_seed_credential: PathBuf,
}

#[derive(Args)]
struct ExportPublicArgsV1 {
    #[command(flatten)]
    public: PublicArgsV1,
    #[command(flatten)]
    credentials: CredentialPathArgsV1,
}

#[derive(Args)]
struct ServeArgsV1 {
    #[command(flatten)]
    public: PublicArgsV1,
    #[command(flatten)]
    credentials: CredentialPathArgsV1,
    /// Exact policy-record digest obtained from a reviewed `export-public` run.
    #[arg(long, value_parser = parse_nonzero_digest_hex_v1)]
    expected_policy_record_digest: [u8; 32],
    /// Exact provider qualification digest obtained from the same export.
    #[arg(long, value_parser = parse_nonzero_digest_hex_v1)]
    expected_qualification_policy_digest: [u8; 32],
}

/// Parse process arguments and run the standalone broker command.
///
/// # Errors
///
/// Returns only stable payload-free launcher failures. Clap handles malformed
/// command syntax before this function returns.
pub async fn run_taira_bootle_lantern_broker_v1() -> Result<(), TairaBootleLanternBrokerErrorV1> {
    execute_cli_v1(BrokerCliV1::parse()).await
}

async fn execute_cli_v1(cli: BrokerCliV1) -> Result<(), TairaBootleLanternBrokerErrorV1> {
    match cli.command {
        BrokerCommandV1::ExportPublic(args) => {
            let config = args.public.into_config()?;
            let backend =
                TairaBootleLanternIssuanceBrokerBackendV1::load_from_hardened_service_credentials_v1(
                    config,
                    &args.credentials.issuer_seed_credential,
                    &args.credentials.bearer_token_credential,
                    &args.credentials.principal_seed_credential,
                )?;
            let export = backend.render_public_export_v1()?;
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(export.as_bytes())
                .and_then(|()| stdout.write_all(b"\n"))
                .map_err(|_| TairaBootleLanternBrokerErrorV1::OutputFailed)
        }
        BrokerCommandV1::Serve(args) => {
            let config = args.public.into_config()?;
            let backend = Arc::new(
                TairaBootleLanternIssuanceBrokerBackendV1::load_from_hardened_service_credentials_v1(
                    config,
                    &args.credentials.issuer_seed_credential,
                    &args.credentials.bearer_token_credential,
                    &args.credentials.principal_seed_credential,
                )?,
            );
            backend.validate_expected_digests_v1(
                args.expected_policy_record_digest,
                args.expected_qualification_policy_digest,
            )?;
            let bindings =
                IrohaRuntimeProviderBindingsV1::try_from_bootle_lantern_issuance_service(
                    backend.config.chain_id(),
                    backend.config.network_id(),
                    backend.config.handle().to_owned(),
                    backend.config.revision(),
                    backend.qualification.policy_digest,
                    backend.config.bindings()?,
                )
                .map_err(|_| TairaBootleLanternBrokerErrorV1::InvalidPublicBinding)?;
            let backends =
                RuntimeProviderBrokerBackendsV1::new().with_bootle_lantern_issuance(backend);
            serve_until_termination_v1(bindings, backends).await
        }
    }
}

async fn serve_until_termination_v1(
    bindings: IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
) -> Result<(), TairaBootleLanternBrokerErrorV1> {
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    let server_lifecycle = Arc::clone(&lifecycle);
    let mut server = tokio::task::spawn_blocking(move || {
        serve_runtime_provider_broker_with_lifecycle_v1(
            &bindings,
            backends,
            server_lifecycle,
            || {},
        )
    });
    tokio::select! {
        result = &mut server => map_server_join_v1(result),
        signal = wait_for_termination_signal_v1() => {
            if signal.is_err() {
                lifecycle.request_shutdown();
                let _ = server.await;
                return Err(TairaBootleLanternBrokerErrorV1::BrokerFailed);
            }
            lifecycle.request_shutdown();
            map_server_join_v1(server.await)
        }
    }
}

fn map_server_join_v1(
    result: Result<Result<(), crate::RuntimeProviderBrokerServerErrorV1>, tokio::task::JoinError>,
) -> Result<(), TairaBootleLanternBrokerErrorV1> {
    result
        .map_err(|_| TairaBootleLanternBrokerErrorV1::BrokerFailed)?
        .map_err(|_| TairaBootleLanternBrokerErrorV1::BrokerFailed)
}

#[cfg(unix)]
async fn wait_for_termination_signal_v1() -> Result<(), TairaBootleLanternBrokerErrorV1> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|_| TairaBootleLanternBrokerErrorV1::BrokerFailed)?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            result.map_err(|_| TairaBootleLanternBrokerErrorV1::BrokerFailed)
        }
        observed = terminate.recv() => {
            observed.ok_or(TairaBootleLanternBrokerErrorV1::BrokerFailed).map(|_| ())
        }
    }
}

#[cfg(not(unix))]
async fn wait_for_termination_signal_v1() -> Result<(), TairaBootleLanternBrokerErrorV1> {
    tokio::signal::ctrl_c()
        .await
        .map_err(|_| TairaBootleLanternBrokerErrorV1::BrokerFailed)
}

fn parse_canonical_nonzero_u64_v1(input: &str) -> Result<u64, String> {
    if input.is_empty()
        || !input.bytes().all(|byte| byte.is_ascii_digit())
        || (input.len() > 1 && input.starts_with('0'))
    {
        return Err("value must be canonical non-zero unsigned decimal".to_owned());
    }
    let value = input
        .parse::<u64>()
        .map_err(|_| "value must fit u64".to_owned())?;
    if value == 0 {
        return Err("value must be non-zero".to_owned());
    }
    Ok(value)
}

fn parse_nonzero_digest_hex_v1(input: &str) -> Result<[u8; 32], String> {
    if input.len() != 64
        || !input
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err("value must be exact lowercase 32-byte hexadecimal".to_owned());
    }
    let mut digest = [0_u8; 32];
    hex::decode_to_slice(input, &mut digest)
        .map_err(|_| "value must be exact lowercase 32-byte hexadecimal".to_owned())?;
    if !is_strong_public_digest_v1(&digest) {
        return Err("value must be a non-weak 32-byte digest".to_owned());
    }
    Ok(digest)
}

fn contains_forbidden_public_marker_v1(value: &str) -> bool {
    value
        .to_ascii_lowercase()
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|component| {
            matches!(
                component,
                "default" | "example" | "sample" | "changeme" | "change" | "local"
            )
        })
}

fn is_strong_public_digest_v1(bytes: &[u8; 32]) -> bool {
    if *bytes == [0; 32] {
        return false;
    }
    let mut seen = [false; 256];
    let mut unique = 0_usize;
    for byte in bytes {
        let slot = &mut seen[usize::from(*byte)];
        if !*slot {
            *slot = true;
            unique += 1;
        }
    }
    unique >= 8
}

fn is_weak_secret_v1(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.iter().all(|byte| *byte == 0) {
        return true;
    }
    let mut seen = [false; 256];
    let mut unique = 0_usize;
    for byte in bytes {
        let slot = &mut seen[usize::from(*byte)];
        if !*slot {
            *slot = true;
            unique += 1;
        }
    }
    if unique < 8 {
        return true;
    }
    if bytes.iter().all(u8::is_ascii) {
        let lowercase = bytes.iter().map(u8::to_ascii_lowercase).collect::<Vec<_>>();
        return [
            b"test".as_slice(),
            b"default".as_slice(),
            b"example".as_slice(),
            b"dummy".as_slice(),
            b"placeholder".as_slice(),
            b"change-me".as_slice(),
            b"changeme".as_slice(),
        ]
        .iter()
        .any(|marker| {
            lowercase
                .windows(marker.len())
                .any(|window| window == *marker)
        });
    }
    false
}

fn hash_length_framed_v1(
    domain: &[u8],
    fields: &[&[u8]],
) -> Result<[u8; 32], TairaBootleLanternBrokerErrorV1> {
    let mut hasher = blake3::Hasher::new();
    let domain_len =
        u64::try_from(domain.len()).map_err(|_| TairaBootleLanternBrokerErrorV1::EncodingFailed)?;
    let field_count =
        u64::try_from(fields.len()).map_err(|_| TairaBootleLanternBrokerErrorV1::EncodingFailed)?;
    hasher.update(&domain_len.to_be_bytes());
    hasher.update(domain);
    hasher.update(&field_count.to_be_bytes());
    for field in fields {
        let length = u64::try_from(field.len())
            .map_err(|_| TairaBootleLanternBrokerErrorV1::EncodingFailed)?;
        hasher.update(&length.to_be_bytes());
        hasher.update(field);
    }
    Ok(*hasher.finalize().as_bytes())
}

fn derive_nonzero_digest_v1(
    domain: &[u8],
    fields: &[&[u8]],
) -> Result<[u8; 32], TairaBootleLanternBrokerErrorV1> {
    let digest = hash_length_framed_v1(domain, fields)?;
    if !is_strong_public_digest_v1(&digest) {
        return Err(TairaBootleLanternBrokerErrorV1::CryptographyUnavailable);
    }
    Ok(digest)
}

fn constant_time_equal_fixed_v1(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut difference = 0_u8;
    for index in 0..32 {
        difference |= left[index] ^ right[index];
    }
    difference == 0
}

fn map_qualification_error_v1(
    error: TairaBootleLanternBrokerQualificationErrorV1,
) -> TairaBootleLanternBrokerErrorV1 {
    match error {
        TairaBootleLanternBrokerQualificationErrorV1::InvalidPublicBinding
        | TairaBootleLanternBrokerQualificationErrorV1::InvalidIssuerPolicy => {
            TairaBootleLanternBrokerErrorV1::InvalidPublicBinding
        }
        TairaBootleLanternBrokerQualificationErrorV1::PolicyEncodingFailed => {
            TairaBootleLanternBrokerErrorV1::EncodingFailed
        }
        TairaBootleLanternBrokerQualificationErrorV1::DegenerateDigest => {
            TairaBootleLanternBrokerErrorV1::CryptographyUnavailable
        }
    }
}

fn map_startup_crypto_error_v1(
    error: BootleLanternIssuanceErrorV1,
) -> TairaBootleLanternBrokerErrorV1 {
    use BootleLanternIssuanceErrorV1 as Error;
    match error {
        Error::InvalidIssuerParameterId
        | Error::InvalidIssuerSecretSeed
        | Error::InvalidIssuerPublicMatrix
        | Error::InvalidIssuerPolicy
        | Error::IssuerPolicyNotActive
        | Error::IssuerKeyPolicyMismatch => TairaBootleLanternBrokerErrorV1::InvalidPublicBinding,
        Error::PolicyEncodingFailed => TairaBootleLanternBrokerErrorV1::EncodingFailed,
        _ => TairaBootleLanternBrokerErrorV1::CryptographyUnavailable,
    }
}

fn map_backend_crypto_error_v1(
    error: BootleLanternIssuanceErrorV1,
) -> BootleLanternIssuanceBrokerBackendErrorV1 {
    use BootleLanternIssuanceErrorV1 as Error;
    match error {
        Error::InvalidIssuerPolicy
        | Error::IssuerPolicyNotActive
        | Error::IssuerKeyPolicyMismatch => {
            BootleLanternIssuanceBrokerBackendErrorV1::PolicyMismatch
        }
        Error::RandomnessUnavailable
        | Error::RandomnessUnhealthy
        | Error::IssuerKeyGenerationExhausted
        | Error::AuthorizationIdExhausted
        | Error::PreimageSamplingExhausted
        | Error::InternalInvariant => BootleLanternIssuanceBrokerBackendErrorV1::Unavailable,
        _ => BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest,
    }
}

#[cfg(unix)]
fn load_credential_v1(
    path: &Path,
    minimum_bytes: usize,
    maximum_bytes: usize,
) -> Result<OpenedCredentialV1, TairaBootleLanternBrokerErrorV1> {
    use std::{ffi::OsString, os::unix::fs::MetadataExt as _};

    use rustix::fs::{AtFlags, FileType, Mode, OFlags};

    if minimum_bytes == 0 || minimum_bytes > maximum_bytes || !is_canonical_absolute_path_v1(path) {
        return Err(TairaBootleLanternBrokerErrorV1::CredentialRejected);
    }
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::RootDir => None,
            Component::Normal(value) => Some(value.to_os_string()),
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => None,
        })
        .collect::<Vec<_>>();
    if components.is_empty() || components.len() > MAX_CREDENTIAL_PATH_COMPONENTS_V1 {
        return Err(TairaBootleLanternBrokerErrorV1::CredentialRejected);
    }
    let (file_name, directory_components) = components
        .split_last()
        .ok_or(TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
    let allow_systemd_root_owner = is_exact_systemd_credential_path_v1(path);
    let mut current = File::from(
        rustix::fs::open(
            Path::new("/"),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?,
    );
    let mut ancestry: Vec<(File, OsString, CredentialFileIdentityV1)> = Vec::new();
    for component in directory_components {
        let before = rustix::fs::statat(&current, component, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
        if FileType::from_raw_mode(before.st_mode) != FileType::Directory {
            return Err(TairaBootleLanternBrokerErrorV1::CredentialRejected);
        }
        let identity = identity_from_stat_v1(&before)?;
        let child = File::from(
            rustix::fs::openat(
                &current,
                component,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?,
        );
        let opened = child
            .metadata()
            .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
        let after = rustix::fs::statat(&current, component, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
        if !opened.is_dir()
            || (CredentialFileIdentityV1 {
                device: opened.dev(),
                inode: opened.ino(),
            }) != identity
            || FileType::from_raw_mode(after.st_mode) != FileType::Directory
            || identity_from_stat_v1(&after)? != identity
        {
            return Err(TairaBootleLanternBrokerErrorV1::CredentialRejected);
        }
        ancestry.push((current, component.clone(), identity));
        current = child;
    }

    let named_before = rustix::fs::statat(&current, file_name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
    validate_credential_stat_v1(
        &named_before,
        minimum_bytes,
        maximum_bytes,
        allow_systemd_root_owner,
    )?;
    let identity = identity_from_stat_v1(&named_before)?;
    let mut file = File::from(
        rustix::fs::openat(
            &current,
            file_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?,
    );
    let opened_before = file
        .metadata()
        .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
    if !credential_metadata_is_valid_v1(
        &opened_before,
        identity,
        minimum_bytes,
        maximum_bytes,
        allow_systemd_root_owner,
    ) {
        return Err(TairaBootleLanternBrokerErrorV1::CredentialRejected);
    }

    run_after_credential_open_test_hook_v1(path);

    let read_limit = u64::try_from(maximum_bytes)
        .ok()
        .and_then(|maximum| maximum.checked_add(1))
        .ok_or(TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
    // Wrap the buffer before the first byte is read so every early return,
    // including read and metadata failures, scrubs any partial credential.
    let mut secret = SecretMaterialV1::new(Vec::with_capacity(maximum_bytes.min(4_096)));
    (&mut file)
        .take(read_limit)
        .read_to_end(&mut secret.bytes)
        .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
    if secret.bytes.len() < minimum_bytes || secret.bytes.len() > maximum_bytes {
        return Err(TairaBootleLanternBrokerErrorV1::CredentialRejected);
    }
    let opened_after = file
        .metadata()
        .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
    let named_after = rustix::fs::statat(&current, file_name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
    if !same_credential_metadata_v1(&opened_before, &opened_after)
        || !credential_metadata_is_valid_v1(
            &opened_after,
            identity,
            minimum_bytes,
            maximum_bytes,
            allow_systemd_root_owner,
        )
        || validate_credential_stat_v1(
            &named_after,
            minimum_bytes,
            maximum_bytes,
            allow_systemd_root_owner,
        )
        .is_err()
        || identity_from_stat_v1(&named_after)? != identity
    {
        return Err(TairaBootleLanternBrokerErrorV1::CredentialRejected);
    }
    for (parent, component, expected_identity) in ancestry {
        let observed = rustix::fs::statat(&parent, component, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
        if FileType::from_raw_mode(observed.st_mode) != FileType::Directory
            || identity_from_stat_v1(&observed)? != expected_identity
        {
            return Err(TairaBootleLanternBrokerErrorV1::CredentialRejected);
        }
    }
    Ok(OpenedCredentialV1 { secret, identity })
}

#[cfg(not(unix))]
fn load_credential_v1(
    _path: &Path,
    _minimum_bytes: usize,
    _maximum_bytes: usize,
) -> Result<OpenedCredentialV1, TairaBootleLanternBrokerErrorV1> {
    Err(TairaBootleLanternBrokerErrorV1::UnsupportedPlatform)
}

#[cfg(unix)]
fn is_canonical_absolute_path_v1(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt as _;

    let bytes = path.as_os_str().as_bytes();
    path.is_absolute()
        && bytes.len() > 1
        && bytes.last() != Some(&b'/')
        && !bytes.windows(2).any(|pair| pair == b"//")
        && !bytes
            .split(|byte| *byte == b'/')
            .any(|component| component == b"." || component == b"..")
}

#[cfg(target_os = "linux")]
fn is_exact_systemd_credential_path_v1(path: &Path) -> bool {
    path.parent() == Some(Path::new(SYSTEMD_CREDENTIAL_DIRECTORY_V1))
        && path.file_name().is_some_and(|name| {
            let name = name.as_encoded_bytes();
            name == b"taira-bootle-lantern-issuer-seed"
                || name == b"taira-bootle-lantern-bearer-token"
                || name == b"taira-bootle-lantern-principal-seed"
        })
}

#[cfg(all(unix, not(target_os = "linux")))]
const fn is_exact_systemd_credential_path_v1(_path: &Path) -> bool {
    false
}

#[cfg(unix)]
fn identity_from_stat_v1(
    stat: &rustix::fs::Stat,
) -> Result<CredentialFileIdentityV1, TairaBootleLanternBrokerErrorV1> {
    Ok(CredentialFileIdentityV1 {
        device: u64::try_from(stat.st_dev)
            .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?,
        inode: u64::try_from(stat.st_ino)
            .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?,
    })
}

#[cfg(unix)]
fn validate_credential_stat_v1(
    stat: &rustix::fs::Stat,
    minimum_bytes: usize,
    maximum_bytes: usize,
    allow_systemd_root_owner: bool,
) -> Result<(), TairaBootleLanternBrokerErrorV1> {
    use rustix::fs::FileType;

    let size = u64::try_from(stat.st_size)
        .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
    let minimum = u64::try_from(minimum_bytes)
        .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
    let maximum = u64::try_from(maximum_bytes)
        .map_err(|_| TairaBootleLanternBrokerErrorV1::CredentialRejected)?;
    if !credential_metadata_fields_are_valid_v1(
        FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile,
        stat.st_uid,
        u64::try_from(stat.st_nlink).ok(),
        u32::from(stat.st_mode),
        Some(size),
        rustix::process::geteuid().as_raw(),
        allow_systemd_root_owner,
        minimum,
        maximum,
    ) {
        return Err(TairaBootleLanternBrokerErrorV1::CredentialRejected);
    }
    Ok(())
}

#[cfg(unix)]
#[expect(
    clippy::too_many_arguments,
    reason = "the pure metadata predicate mirrors the complete kernel trust decision"
)]
fn credential_metadata_fields_are_valid_v1(
    is_regular_file: bool,
    owner_uid: u32,
    link_count: Option<u64>,
    mode: u32,
    size: Option<u64>,
    expected_uid: u32,
    allow_systemd_root_owner: bool,
    minimum_size: u64,
    maximum_size: u64,
) -> bool {
    is_regular_file
        && (owner_uid == expected_uid || (allow_systemd_root_owner && owner_uid == 0))
        && link_count == Some(1)
        && mode & 0o777 == CREDENTIAL_FILE_MODE_V1
        && size.is_some_and(|size| size >= minimum_size && size <= maximum_size)
}

#[cfg(unix)]
fn credential_metadata_is_valid_v1(
    metadata: &std::fs::Metadata,
    identity: CredentialFileIdentityV1,
    minimum_bytes: usize,
    maximum_bytes: usize,
    allow_systemd_root_owner: bool,
) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    metadata.dev() == identity.device
        && metadata.ino() == identity.inode
        && credential_metadata_fields_are_valid_v1(
            metadata.is_file(),
            metadata.uid(),
            Some(metadata.nlink()),
            metadata.mode(),
            Some(metadata.len()),
            rustix::process::geteuid().as_raw(),
            allow_systemd_root_owner,
            u64::try_from(minimum_bytes).unwrap_or(u64::MAX),
            u64::try_from(maximum_bytes).unwrap_or(0),
        )
}

#[cfg(unix)]
fn same_credential_metadata_v1(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(all(test, unix))]
static AFTER_CREDENTIAL_OPEN_TEST_HOOK_V1: std::sync::Mutex<Option<(PathBuf, PathBuf)>> =
    std::sync::Mutex::new(None);

#[cfg(all(test, unix))]
fn run_after_credential_open_test_hook_v1(path: &Path) {
    let replacement = {
        let mut hook = AFTER_CREDENTIAL_OPEN_TEST_HOOK_V1
            .lock()
            .expect("credential replacement test hook lock");
        match hook.as_ref() {
            Some((expected, _)) if expected == path => hook.take().map(|(_, path)| path),
            _ => None,
        }
    };
    if let Some(replacement) = replacement {
        std::fs::rename(replacement, path).expect("replace credential path after secure open");
    }
}

#[cfg(not(all(test, unix)))]
fn run_after_credential_open_test_hook_v1(_path: &Path) {}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock};

    use super::*;
    use iroha_core::privacy_engines::bootle_lantern::issuer::{
        holder_finalize_blind_issuance_v1, holder_prepare_blind_issuance_v1,
    };
    use iroha_data_model::privacy::{
        PrivacyEngineManifestDigestV1, PrivacyParameterDigestV1, PrivacyStatementSchemaDigestV1,
        PrivacyTransactionIntentDigestV1, PrivacyVerifierDigestV1,
    };

    fn strong_32_v1(label: &[u8]) -> [u8; 32] {
        let digest = hash_length_framed_v1(b"iroha.taira.broker.test.strong32.v1", &[label])
            .expect("bounded test hash");
        assert!(is_strong_public_digest_v1(&digest));
        digest
    }

    fn network_id_v1(label: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed(strong_32_v1(label)),
            ),
        )
    }

    fn issuer_seed_v1() -> [u8; 32] {
        strong_32_v1(b"issuer-seed-primary")
    }

    fn principal_seed_v1() -> [u8; 32] {
        strong_32_v1(b"principal-seed-primary")
    }

    fn bearer_token_v1() -> Vec<u8> {
        let first = strong_32_v1(b"bearer-token-primary-first");
        let second = strong_32_v1(b"bearer-token-primary-second");
        [first.as_slice(), second.as_slice()].concat()
    }

    fn public_config_v1() -> TairaBootleLanternBrokerPublicConfigV1 {
        TairaBootleLanternBrokerPublicConfigV1::try_new(
            "fc56984b-2be7-431d-840e-21514d1883f0"
                .parse()
                .expect("canonical Taira chain id"),
            network_id_v1(b"canonical-genesis"),
            "runtime://privacy/bootle-lantern/taira-primary",
            1,
            PrivacyIssuerIdV1::new(strong_32_v1(b"issuer-id-primary")),
            PrivacyPolicyIdV1::new(strong_32_v1(b"policy-id-primary")),
            64,
        )
        .expect("valid public test config")
    }

    fn backend_with_seed_v1(
        issuer_seed: [u8; 32],
        bearer_token: Vec<u8>,
    ) -> TairaBootleLanternIssuanceBrokerBackendV1 {
        TairaBootleLanternIssuanceBrokerBackendV1::from_credentials_v1(
            public_config_v1(),
            CredentialBundleV1 {
                issuer_seed: SecretMaterialV1::new(issuer_seed.to_vec()),
                bearer_token: SecretMaterialV1::new(bearer_token),
                principal_seed: SecretMaterialV1::new(principal_seed_v1().to_vec()),
            },
        )
        .expect("construct native test backend")
    }

    fn backend_v1() -> Arc<TairaBootleLanternIssuanceBrokerBackendV1> {
        static BACKEND: OnceLock<Arc<TairaBootleLanternIssuanceBrokerBackendV1>> = OnceLock::new();
        Arc::clone(
            BACKEND.get_or_init(|| {
                Arc::new(backend_with_seed_v1(issuer_seed_v1(), bearer_token_v1()))
            }),
        )
    }

    fn statement_context_v1() -> PrivacyStatementContextV1 {
        PrivacyStatementContextV1 {
            network_id: public_config_v1().network_id,
            action_index: 0,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new(strong_32_v1(
                b"transaction-intent",
            )),
            parameter_id: PrivacyParameterIdV1::new(strong_32_v1(b"statement-parameter-id")),
            parameter_digest: PrivacyParameterDigestV1::new(strong_32_v1(
                b"statement-parameter-digest",
            )),
            verifier_digest: PrivacyVerifierDigestV1::new(strong_32_v1(b"verifier-digest")),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new(strong_32_v1(
                b"statement-schema-digest",
            )),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new(strong_32_v1(
                b"engine-manifest-digest",
            )),
        }
    }

    fn authorization_v1(
        backend: &TairaBootleLanternIssuanceBrokerBackendV1,
    ) -> BootleLanternIssuanceAuthorizationV1 {
        backend
            .prepare_authorization(
                &statement_context_v1(),
                strong_32_v1(b"canonical-genesis"),
                backend.policy(),
                backend.principal_digest(),
                10,
                74,
            )
            .expect("prepare exact native authorization")
    }

    #[test]
    fn public_config_rejects_weak_marked_zero_and_overflow_bindings() {
        let baseline = public_config_v1();
        for handle in [
            "runtime://privacy/bootle-lantern/test",
            "runtime://privacy/bootle-lantern/default",
            "runtime://privacy/bootle-lantern/example",
            "runtime://privacy/bootle-lantern/local",
            "runtime://operator:secret@privacy",
        ] {
            assert!(
                TairaBootleLanternBrokerPublicConfigV1::try_new(
                    baseline.chain_id.clone(),
                    baseline.network_id,
                    handle,
                    1,
                    baseline.issuer_id,
                    baseline.policy_id,
                    64,
                )
                .is_err(),
                "must reject marked or credential-bearing handle {handle:?}"
            );
        }
        for (revision, issuer, policy, lifetime) in [
            (0, baseline.issuer_id, baseline.policy_id, 64),
            (1, PrivacyIssuerIdV1::new([0; 32]), baseline.policy_id, 64),
            (1, PrivacyIssuerIdV1::new([7; 32]), baseline.policy_id, 64),
            (1, baseline.issuer_id, PrivacyPolicyIdV1::new([9; 32]), 64),
            (1, baseline.issuer_id, baseline.policy_id, 0),
            (
                1,
                baseline.issuer_id,
                baseline.policy_id,
                MAX_BOOTLE_LANTERN_AUTHORIZATION_LIFETIME_BLOCKS_V1 + 1,
            ),
        ] {
            assert!(
                TairaBootleLanternBrokerPublicConfigV1::try_new(
                    baseline.chain_id.clone(),
                    baseline.network_id,
                    baseline.handle.clone(),
                    revision,
                    issuer,
                    policy,
                    lifetime,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn epoch_one_policy_is_closed_exact_and_self_validating() {
        let backend = backend_v1();
        let policy = backend.policy();
        assert_eq!(policy.issuer_id, backend.config.issuer_id);
        assert_eq!(policy.policy_id, backend.config.policy_id);
        assert_eq!(policy.epoch, 1);
        assert_eq!(policy.required_disclosure_bitmap, 0);
        assert_eq!(
            policy.allowed_values.len(),
            BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1
        );
        assert!(
            policy
                .allowed_values
                .iter()
                .all(|allowed| allowed.values.is_empty())
        );
        policy.validate_initial().expect("exact initial policy");
        assert_eq!(
            policy
                .computed_record_digest()
                .expect("compute exact policy digest"),
            policy.record_digest
        );
    }

    #[test]
    fn qualification_binds_public_inputs_shared_profile_contract_and_not_bearer() {
        let backend = backend_v1();
        let inputs = TairaBootleLanternBrokerQualificationInputsV1 {
            network_id: backend.config.network_id,
            runtime_provider_handle: &backend.config.handle,
            runtime_provider_revision: backend.config.revision,
            issuer_id: backend.config.issuer_id,
            policy_id: backend.config.policy_id,
            authorization_lifetime_blocks: backend.config.authorization_lifetime_blocks,
            policy: backend.policy(),
            stable_principal_digest: backend.principal_digest(),
        };
        let exact = derive_taira_bootle_lantern_broker_qualification_digest_v1(&inputs)
            .expect("exact shared qualification");
        assert_eq!(exact, backend.public_qualification().policy_digest);
        assert!(is_strong_public_digest_v1(
            &taira_bootle_lantern_issuer_profile_contract_digest_v1()
        ));
        assert!(is_strong_public_digest_v1(
            &taira_bootle_lantern_broker_contract_digest_v1()
        ));

        let mut substitutions = Vec::new();
        let mut changed = inputs;
        changed.runtime_provider_revision += 1;
        substitutions.push(
            derive_taira_bootle_lantern_broker_qualification_digest_v1(&changed)
                .expect("revision-substituted digest"),
        );
        let mut changed = inputs;
        changed.authorization_lifetime_blocks += 1;
        substitutions.push(
            derive_taira_bootle_lantern_broker_qualification_digest_v1(&changed)
                .expect("lifetime-substituted digest"),
        );
        let mut changed = inputs;
        changed.stable_principal_digest = strong_32_v1(b"substituted-principal");
        substitutions.push(
            derive_taira_bootle_lantern_broker_qualification_digest_v1(&changed)
                .expect("principal-substituted digest"),
        );
        assert!(substitutions.iter().all(|digest| *digest != exact));

        let token_digest_a =
            derive_nonzero_digest_v1(BEARER_TOKEN_DIGEST_DOMAIN_V1, &[&bearer_token_v1()])
                .expect("first token digest");
        let token_digest_b = derive_nonzero_digest_v1(
            BEARER_TOKEN_DIGEST_DOMAIN_V1,
            &[&strong_32_v1(b"rotated-bearer-token")],
        )
        .expect("rotated token digest");
        assert_ne!(token_digest_a, token_digest_b);
        assert_eq!(
            derive_taira_bootle_lantern_broker_qualification_digest_v1(&inputs)
                .expect("qualification excludes token"),
            exact
        );
    }

    #[test]
    fn expected_digest_gate_rejects_policy_or_qualification_drift() {
        let backend = backend_v1();
        let policy_digest = *backend.policy().record_digest.as_bytes();
        let qualification_digest = backend.public_qualification().policy_digest;
        backend
            .validate_expected_digests_v1(policy_digest, qualification_digest)
            .expect("accept exact reviewed digests");
        let mut wrong_policy = policy_digest;
        wrong_policy[0] ^= 1;
        assert_eq!(
            backend.validate_expected_digests_v1(wrong_policy, qualification_digest),
            Err(TairaBootleLanternBrokerErrorV1::ExpectedDigestMismatch)
        );
        let mut wrong_qualification = qualification_digest;
        wrong_qualification[31] ^= 1;
        assert_eq!(
            backend.validate_expected_digests_v1(policy_digest, wrong_qualification),
            Err(TairaBootleLanternBrokerErrorV1::ExpectedDigestMismatch)
        );
    }

    #[test]
    fn public_export_is_deterministic_complete_and_secret_free() {
        let backend = backend_v1();
        let first = backend
            .render_public_export_v1()
            .expect("render first public export");
        let second = backend
            .render_public_export_v1()
            .expect("render second public export");
        assert_eq!(first, second);
        let value: norito::json::Value =
            norito::json::from_str(&first).expect("decode public export JSON");
        let object = value.as_object().expect("public export object");
        for field in [
            "runtime_provider_policy_digest_hex",
            "issuer_parameter_digest_hex",
            "policy_record_digest_hex",
            "stable_principal_digest_hex",
            "registration_instruction_norito_hex",
            "registration_instruction_norito_sha256",
            "registration_instruction",
        ] {
            assert!(object.contains_key(field), "missing export field {field}");
        }
        for secret in [
            hex::encode(issuer_seed_v1()),
            hex::encode(principal_seed_v1()),
            hex::encode(bearer_token_v1()),
        ] {
            assert!(!first.contains(&secret));
        }
    }

    #[test]
    fn public_export_registration_is_the_exact_canonical_instruction_box() {
        let backend = backend_v1();
        let export = backend
            .render_public_export_v1()
            .expect("render canonical public export");
        let value: norito::json::Value =
            norito::json::from_str(&export).expect("decode public export JSON");
        let object = value.as_object().expect("public export object");
        let encoded = object
            .get("registration_instruction_norito_hex")
            .and_then(norito::json::Value::as_str)
            .expect("boxed registration hex");
        let boxed_bytes = hex::decode(encoded).expect("decode boxed registration hex");
        let decoded: InstructionBox =
            norito::decode_from_bytes(&boxed_bytes).expect("decode canonical InstructionBox");
        assert_eq!(
            norito::to_bytes(&decoded).expect("re-encode canonical InstructionBox"),
            boxed_bytes
        );
        let registration = decoded
            .as_any()
            .downcast_ref::<RegisterPrivacyBootleLanternIssuerPolicyV1>()
            .expect("exact Bootle/Lantern policy registration variant");
        assert_eq!(&registration.policy, backend.policy());
        let expected_sha256 = hex::encode(iroha_crypto::sha256(&boxed_bytes));
        assert_eq!(
            object
                .get("registration_instruction_norito_sha256")
                .and_then(norito::json::Value::as_str),
            Some(expected_sha256.as_str())
        );

        let direct = norito::to_bytes(&RegisterPrivacyBootleLanternIssuerPolicyV1::new(
            backend.policy().clone(),
        ))
        .expect("encode deliberately unboxed registration");
        assert_ne!(direct, boxed_bytes);
        assert!(norito::decode_from_bytes::<InstructionBox>(&direct).is_err());
        let mut trailing = boxed_bytes;
        trailing.push(0);
        assert!(norito::decode_from_bytes::<InstructionBox>(&trailing).is_err());
    }

    #[test]
    fn authentication_is_bounded_stable_and_rejects_every_bearer_mutation() {
        let backend = backend_v1();
        let token = bearer_token_v1();
        let authorize = backend
            .authenticate(
                &token,
                BootleLanternIssuanceActionV1::Authorize,
                strong_32_v1(b"authorize-body-binding"),
                17,
            )
            .expect("authenticate authorize action");
        let issue = backend
            .authenticate(
                &token,
                BootleLanternIssuanceActionV1::Issue,
                strong_32_v1(b"issue-body-binding"),
                17,
            )
            .expect("authenticate issue action");
        assert_eq!(authorize.principal_digest, issue.principal_digest);
        assert_eq!(authorize.issued_at_height, 17);
        assert_eq!(authorize.expires_at_height, 81);

        for index in 0..token.len() {
            let mut substituted = token.clone();
            substituted[index] ^= 1;
            assert!(matches!(
                backend.authenticate(
                    &substituted,
                    BootleLanternIssuanceActionV1::Authorize,
                    strong_32_v1(b"authorize-body-binding"),
                    17,
                ),
                Err(BootleLanternIssuanceAuthenticationErrorV1::Denied)
            ));
        }
        for substituted in [
            Vec::new(),
            token[..31].to_vec(),
            vec![0xA5; MAX_BEARER_TOKEN_BYTES_V1 + 1],
        ] {
            assert!(matches!(
                backend.authenticate(
                    &substituted,
                    BootleLanternIssuanceActionV1::Issue,
                    strong_32_v1(b"issue-body-binding"),
                    17,
                ),
                Err(BootleLanternIssuanceAuthenticationErrorV1::Denied)
            ));
        }
        assert!(matches!(
            backend.authenticate(
                &token,
                BootleLanternIssuanceActionV1::Authorize,
                [0; 32],
                17,
            ),
            Err(BootleLanternIssuanceAuthenticationErrorV1::Denied)
        ));
        assert!(matches!(
            backend.authenticate(
                &token,
                BootleLanternIssuanceActionV1::Authorize,
                strong_32_v1(b"authorize-body-binding"),
                0,
            ),
            Err(BootleLanternIssuanceAuthenticationErrorV1::Denied)
        ));
        assert!(matches!(
            backend.authenticate(
                &token,
                BootleLanternIssuanceActionV1::Authorize,
                strong_32_v1(b"authorize-body-binding"),
                u64::MAX,
            ),
            Err(BootleLanternIssuanceAuthenticationErrorV1::Unavailable)
        ));
    }

    #[test]
    fn crypto_boundary_rejects_network_genesis_policy_principal_lifetime_and_wire_substitution() {
        let backend = backend_v1();
        let context = statement_context_v1();
        let genesis = strong_32_v1(b"canonical-genesis");
        let authorization = authorization_v1(&backend);

        let mut wrong_context = context.clone();
        wrong_context.network_id = network_id_v1(b"substituted-genesis");
        assert!(matches!(
            backend.prepare_authorization(
                &wrong_context,
                genesis,
                backend.policy(),
                backend.principal_digest(),
                10,
                74,
            ),
            Err(BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest)
        ));
        assert!(matches!(
            backend.prepare_authorization(
                &context,
                [0; 32],
                backend.policy(),
                backend.principal_digest(),
                10,
                74,
            ),
            Err(BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest)
        ));
        let mut wrong_policy = backend.policy().clone();
        wrong_policy.required_disclosure_bitmap = 1;
        assert!(matches!(
            backend.prepare_authorization(
                &context,
                genesis,
                &wrong_policy,
                backend.principal_digest(),
                10,
                74,
            ),
            Err(BootleLanternIssuanceBrokerBackendErrorV1::PolicyMismatch)
        ));
        for (principal, issued, expires) in [
            ([0; 32], 10, 74),
            (strong_32_v1(b"substituted-principal"), 10, 74),
            (backend.principal_digest(), 0, 64),
            (backend.principal_digest(), 10, 9),
            (backend.principal_digest(), 10, 73),
            (backend.principal_digest(), 10, 75),
            (backend.principal_digest(), 10, 10 + 4_097),
        ] {
            assert!(
                backend
                    .prepare_authorization(
                        &context,
                        genesis,
                        backend.policy(),
                        principal,
                        issued,
                        expires,
                    )
                    .is_err()
            );
        }

        for substituted_authorization in [
            issuer_prepare_blind_issuance_authorization_candidate_v1(
                &backend.issuer,
                &context,
                genesis,
                backend.policy(),
                strong_32_v1(b"signed-substituted-principal"),
                10,
                74,
            )
            .expect("construct validly signed principal substitution"),
            issuer_prepare_blind_issuance_authorization_candidate_v1(
                &backend.issuer,
                &context,
                genesis,
                backend.policy(),
                backend.principal_digest(),
                10,
                75,
            )
            .expect("construct validly signed lifetime substitution"),
        ] {
            assert_eq!(
                backend.validate_request(
                    &context,
                    genesis,
                    backend.policy(),
                    &substituted_authorization,
                    &[],
                    10,
                ),
                Err(BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest)
            );
            assert!(matches!(
                backend.issue_validated(
                    &context,
                    genesis,
                    backend.policy(),
                    &substituted_authorization,
                    &[],
                    10,
                ),
                Err(BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest)
            ));
        }
        assert_eq!(
            backend.validate_request(&context, genesis, backend.policy(), &authorization, &[], 10,),
            Err(BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest)
        );
        assert!(matches!(
            backend.issue_validated(
                &context,
                genesis,
                backend.policy(),
                &authorization,
                &[0; 17],
                10,
            ),
            Err(BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest)
        ));
    }

    #[test]
    fn key_policy_substitution_is_a_policy_failure_before_request_decoding() {
        let primary = backend_v1();
        let secondary =
            backend_with_seed_v1(strong_32_v1(b"independent-issuer-seed"), bearer_token_v1());
        let authorization = authorization_v1(&primary);
        assert_ne!(
            primary.policy().issuer_parameter_digest,
            secondary.policy().issuer_parameter_digest
        );
        assert_eq!(
            secondary.validate_request(
                &statement_context_v1(),
                strong_32_v1(b"canonical-genesis"),
                primary.policy(),
                &authorization,
                &[],
                10,
            ),
            Err(BootleLanternIssuanceBrokerBackendErrorV1::PolicyMismatch)
        );
    }

    #[test]
    fn native_backend_completes_ila1_ilq1_ilr1_and_holder_finalization() {
        let backend = backend_v1();
        let context = statement_context_v1();
        let genesis = strong_32_v1(b"canonical-genesis");
        let authorization = authorization_v1(&backend);
        let (request, state) = holder_prepare_blind_issuance_v1(
            &context,
            genesis,
            backend.policy(),
            &authorization,
            [[0; 8]; BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1],
        )
        .expect("prepare exact native ILQ1");
        let request_bytes = request.encode().expect("encode canonical ILQ1");
        assert_eq!(
            backend
                .validate_request(
                    &context,
                    genesis,
                    backend.policy(),
                    &authorization,
                    &request_bytes,
                    10,
                )
                .expect("key-bound validate exact ILQ1"),
            request.request_digest()
        );
        let response = backend
            .issue_validated(
                &context,
                genesis,
                backend.policy(),
                &authorization,
                &request_bytes,
                10,
            )
            .expect("issue exact native ILR1");
        holder_finalize_blind_issuance_v1(state, &context, genesis, backend.policy(), response)
            .expect("independently finalize issued credential");
    }

    #[test]
    fn redacted_debug_errors_and_scrubbing_never_expose_secret_bytes() {
        let backend = backend_v1();
        let rendered = format!("{backend:?}");
        assert!(rendered.contains("[REDACTED]"));
        assert!(!rendered.contains(&hex::encode(issuer_seed_v1())));
        assert!(!rendered.contains(&hex::encode(bearer_token_v1())));
        let mut secret = SecretMaterialV1::new(bearer_token_v1());
        assert!(secret.as_bytes().iter().any(|byte| *byte != 0));
        secret.scrub();
        assert!(secret.as_bytes().iter().all(|byte| *byte == 0));
        for error in [
            TairaBootleLanternBrokerErrorV1::InvalidPublicBinding,
            TairaBootleLanternBrokerErrorV1::CredentialRejected,
            TairaBootleLanternBrokerErrorV1::CryptographyUnavailable,
            TairaBootleLanternBrokerErrorV1::ExpectedDigestMismatch,
        ] {
            let rendered = format!("{error:?}/{error}");
            assert!(!rendered.contains("seed"));
            assert!(!rendered.contains("token"));
        }
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TairaBootleLanternIssuanceBrokerBackendV1>();
    }

    #[test]
    fn cli_is_canonical_and_has_no_secret_or_config_value_surface() {
        let issuer_id = hex::encode(strong_32_v1(b"issuer-id-primary"));
        let policy_id = hex::encode(strong_32_v1(b"policy-id-primary"));
        let base = [
            "taira_bootle_lantern_broker",
            "export-public",
            "--chain-id",
            "fc56984b-2be7-431d-840e-21514d1883f0",
            "--network-id",
            "hash:A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5#95D7",
            "--handle",
            "runtime://privacy/bootle-lantern/taira-primary",
            "--revision",
            "1",
            "--issuer-id",
            issuer_id.as_str(),
            "--policy-id",
            policy_id.as_str(),
            "--authorization-lifetime-blocks",
            "64",
            "--issuer-seed-credential",
            "/run/credentials/issuer-seed",
            "--bearer-token-credential",
            "/run/credentials/bearer-token",
            "--principal-seed-credential",
            "/run/credentials/principal-seed",
        ];
        BrokerCliV1::try_parse_from(base).expect("accept canonical path-only secret CLI");

        for forbidden in [
            "--issuer-seed",
            "--issuer-seed-hex",
            "--bearer-token",
            "--principal-seed",
            "--config",
        ] {
            let mut arguments = base.to_vec();
            arguments.extend([forbidden, "secret-material"]);
            assert!(
                BrokerCliV1::try_parse_from(arguments).is_err(),
                "must reject forbidden secret/config option {forbidden}"
            );
        }
        for bad_revision in ["0", "01", "+1", "-1"] {
            assert!(parse_canonical_nonzero_u64_v1(bad_revision).is_err());
        }
        for bad_digest in [
            "00",
            "0c63367874569862486026c04717783e35546cb6f41a95b34d09d64153f5c5ed",
            "0C63367874569862486026C04717783E35546CB6F41A95B34D09D64153F5C5ED",
            "0707070707070707070707070707070707070707070707070707070707070707",
        ] {
            assert!(parse_nonzero_digest_hex_v1(bad_digest).is_err());
        }
    }

    #[cfg(unix)]
    fn write_credential_v1(directory: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;

        let path = directory
            .canonicalize()
            .expect("canonical credential tempdir")
            .join(name);
        std::fs::write(&path, bytes).expect("write test credential");
        std::fs::set_permissions(
            &path,
            std::fs::Permissions::from_mode(CREDENTIAL_FILE_MODE_V1),
        )
        .expect("set exact credential mode");
        path
    }

    #[cfg(unix)]
    fn credential_paths_v1(directory: &Path) -> (PathBuf, PathBuf, PathBuf) {
        (
            write_credential_v1(directory, "issuer.seed", &issuer_seed_v1()),
            write_credential_v1(directory, "bearer.token", &bearer_token_v1()),
            write_credential_v1(directory, "principal.seed", &principal_seed_v1()),
        )
    }

    #[cfg(unix)]
    #[test]
    fn credential_loader_accepts_only_three_distinct_exact_owner_mode_files() {
        let directory = tempfile::tempdir().expect("credential tempdir");
        let (issuer, bearer, principal) = credential_paths_v1(directory.path());
        let bundle = CredentialBundleV1::load(&issuer, &bearer, &principal)
            .expect("load exact three credentials");
        assert_eq!(bundle.issuer_seed.as_bytes(), issuer_seed_v1());
        assert_eq!(bundle.bearer_token.as_bytes(), bearer_token_v1());
        assert_eq!(bundle.principal_seed.as_bytes(), principal_seed_v1());
        assert!(matches!(
            CredentialBundleV1::load(&issuer, &issuer, &principal),
            Err(TairaBootleLanternBrokerErrorV1::CredentialRejected)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn credential_loader_rejects_relative_symlink_ancestor_directory_and_hardlink_attacks() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("credential tempdir");
        let seed = write_credential_v1(directory.path(), "seed", &issuer_seed_v1());
        assert_eq!(
            load_credential_v1(Path::new("relative-seed"), 32, 32).map(|_| ()),
            Err(TairaBootleLanternBrokerErrorV1::CredentialRejected)
        );

        let symlink_path = directory.path().join("seed-link");
        symlink(&seed, &symlink_path).expect("create final symlink attack");
        assert_eq!(
            load_credential_v1(&symlink_path, 32, 32).map(|_| ()),
            Err(TairaBootleLanternBrokerErrorV1::CredentialRejected)
        );

        let ancestor_link = directory.path().join("ancestor-link");
        symlink(directory.path(), &ancestor_link).expect("create ancestor symlink attack");
        assert_eq!(
            load_credential_v1(&ancestor_link.join("seed"), 32, 32).map(|_| ()),
            Err(TairaBootleLanternBrokerErrorV1::CredentialRejected)
        );

        let subdirectory = directory.path().join("not-a-file");
        std::fs::create_dir(&subdirectory).expect("create directory attack");
        assert_eq!(
            load_credential_v1(&subdirectory, 32, 32).map(|_| ()),
            Err(TairaBootleLanternBrokerErrorV1::CredentialRejected)
        );

        let hardlink = directory.path().join("seed-hardlink");
        std::fs::hard_link(&seed, &hardlink).expect("create hardlink attack");
        assert_eq!(
            load_credential_v1(&seed, 32, 32).map(|_| ()),
            Err(TairaBootleLanternBrokerErrorV1::CredentialRejected)
        );
    }

    #[cfg(unix)]
    #[test]
    fn credential_loader_accepts_only_service_or_systemd_root_owner_and_exact_metadata() {
        let uid = rustix::process::geteuid().as_raw();
        assert!(credential_metadata_fields_are_valid_v1(
            true,
            uid,
            Some(1),
            0o100400,
            Some(32),
            uid,
            false,
            32,
            32,
        ));
        assert!(credential_metadata_fields_are_valid_v1(
            true,
            0,
            Some(1),
            0o100400,
            Some(32),
            1_001,
            true,
            32,
            32,
        ));
        assert!(!credential_metadata_fields_are_valid_v1(
            true,
            1_002,
            Some(1),
            0o100400,
            Some(32),
            1_001,
            true,
            32,
            32,
        ));
        for fields in [
            (false, uid, Some(1), 0o100400, Some(32)),
            (true, uid.wrapping_add(1), Some(1), 0o100400, Some(32)),
            (true, uid, Some(2), 0o100400, Some(32)),
            (true, uid, Some(1), 0o100600, Some(32)),
            (true, uid, Some(1), 0o100440, Some(32)),
            (true, uid, Some(1), 0o100404, Some(32)),
            (true, uid, Some(1), 0o100400, Some(31)),
            (true, uid, Some(1), 0o100400, Some(33)),
        ] {
            assert!(!credential_metadata_fields_are_valid_v1(
                fields.0, fields.1, fields.2, fields.3, fields.4, uid, false, 32, 32,
            ));
        }

        use std::os::unix::fs::PermissionsExt as _;
        let directory = tempfile::tempdir().expect("credential tempdir");
        let path = write_credential_v1(directory.path(), "wrong-mode", &issuer_seed_v1());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("set unsafe mode");
        assert_eq!(
            load_credential_v1(&path, 32, 32).map(|_| ()),
            Err(TairaBootleLanternBrokerErrorV1::CredentialRejected)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn systemd_root_owner_exception_is_limited_to_three_exact_unit_credentials() {
        for name in [
            "taira-bootle-lantern-issuer-seed",
            "taira-bootle-lantern-bearer-token",
            "taira-bootle-lantern-principal-seed",
        ] {
            assert!(is_exact_systemd_credential_path_v1(
                &Path::new(SYSTEMD_CREDENTIAL_DIRECTORY_V1).join(name)
            ));
        }
        for rejected in [
            PathBuf::from("/run/credentials/other.service/taira-bootle-lantern-issuer-seed"),
            Path::new(SYSTEMD_CREDENTIAL_DIRECTORY_V1).join("unexpected"),
            PathBuf::from("/etc/iroha/root-owned-secret"),
        ] {
            assert!(!is_exact_systemd_credential_path_v1(&rejected));
        }
        assert!(!credential_metadata_fields_are_valid_v1(
            true,
            0,
            Some(1),
            0o100400,
            Some(32),
            1_001,
            false,
            32,
            32,
        ));
    }

    #[cfg(unix)]
    #[test]
    fn credential_loader_rejects_truncation_extension_zero_and_weak_markers() {
        let directory = tempfile::tempdir().expect("credential tempdir");
        for (name, bytes) in [("truncated", vec![0xA5; 31]), ("extended", vec![0xA5; 33])] {
            let path = write_credential_v1(directory.path(), name, &bytes);
            assert_eq!(
                load_credential_v1(&path, 32, 32).map(|_| ()),
                Err(TairaBootleLanternBrokerErrorV1::CredentialRejected)
            );
        }
        for weak in [
            vec![0; 32],
            vec![7; 32],
            b"test-credential-with-enough-length-123456".to_vec(),
            b"default-credential-with-enough-length-123".to_vec(),
            b"change-me-credential-with-enough-length".to_vec(),
        ] {
            assert!(is_weak_secret_v1(&weak));
        }
        let (issuer, bearer, principal) = credential_paths_v1(directory.path());
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&issuer, std::fs::Permissions::from_mode(0o600))
            .expect("make zero-seed file writable");
        std::fs::write(&issuer, [0; 32]).expect("replace with zero seed");
        std::fs::set_permissions(&issuer, std::fs::Permissions::from_mode(0o400))
            .expect("restore exact mode");
        assert!(matches!(
            CredentialBundleV1::load(&issuer, &bearer, &principal),
            Err(TairaBootleLanternBrokerErrorV1::CredentialRejected)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn credential_loader_detects_path_replacement_after_open() {
        let directory = tempfile::tempdir().expect("credential tempdir");
        let target = write_credential_v1(directory.path(), "target", &issuer_seed_v1());
        let replacement = write_credential_v1(
            directory.path(),
            "replacement",
            &strong_32_v1(b"replacement-seed"),
        );
        *AFTER_CREDENTIAL_OPEN_TEST_HOOK_V1
            .lock()
            .expect("install replacement hook") = Some((target.clone(), replacement));
        assert_eq!(
            load_credential_v1(&target, 32, 32).map(|_| ()),
            Err(TairaBootleLanternBrokerErrorV1::CredentialRejected)
        );
        assert!(
            AFTER_CREDENTIAL_OPEN_TEST_HOOK_V1
                .lock()
                .expect("replacement hook consumed")
                .is_none()
        );
    }

    #[cfg(unix)]
    #[test]
    fn live_backend_uses_immutable_opened_credentials_until_restart() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("credential tempdir");
        let (issuer, bearer, principal) = credential_paths_v1(directory.path());
        let backend =
            TairaBootleLanternIssuanceBrokerBackendV1::load_from_hardened_service_credentials_v1(
                public_config_v1(),
                &issuer,
                &bearer,
                &principal,
            )
            .expect("load exact credential snapshot");
        let old_token = bearer_token_v1();
        let new_token = [
            strong_32_v1(b"rotated-token-first").as_slice(),
            strong_32_v1(b"rotated-token-second").as_slice(),
        ]
        .concat();
        std::fs::set_permissions(&bearer, std::fs::Permissions::from_mode(0o600))
            .expect("make credential replaceable for rotation test");
        std::fs::write(&bearer, &new_token).expect("rotate credential pathname");
        std::fs::set_permissions(&bearer, std::fs::Permissions::from_mode(0o400))
            .expect("restore credential mode");
        assert!(
            backend
                .authenticate(
                    &old_token,
                    BootleLanternIssuanceActionV1::Authorize,
                    strong_32_v1(b"rotation-binding"),
                    20,
                )
                .is_ok()
        );
        assert_eq!(
            backend.authenticate(
                &new_token,
                BootleLanternIssuanceActionV1::Authorize,
                strong_32_v1(b"rotation-binding"),
                20,
            ),
            Err(BootleLanternIssuanceAuthenticationErrorV1::Denied)
        );
    }
}
