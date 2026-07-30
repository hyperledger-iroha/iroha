//! Injectable SoraFS Governance DAG publisher and bounded public mirror service.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fmt,
    fs::{self, OpenOptions},
    future::{Future, IntoFuture},
    io::{self, Read},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Component, Path, PathBuf},
    process,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
use std::fs::File;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

pub use crate::governance::{
    GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1,
    GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1,
    GOVERNANCE_DAG_REQUEST_AUTH_SELECTED_HEADER_NAMES_V1, GovernanceDagHttpRequestReceiverV1,
    GovernanceDagRequestAuthenticationErrorV1, GovernanceDagRequestAuthenticationPolicyV1,
    GovernanceDagRequestAuthenticationReplayCacheV1,
    canonicalize_governance_dag_outbound_http_request_v1,
    governance_dag_request_authentication_headers_v1,
    parse_governance_dag_request_authentication_headers_v1,
    verify_governance_dag_request_authentication_v1,
};
use crate::{
    GovernanceDagAuthenticationScope, GovernanceDagCanonicalRequestV1,
    GovernanceDagRequestAuthenticationEnvelopeV1, GovernanceDagRequestAuthenticator,
    GovernanceDagRuntimeProviderQualificationV1, GovernanceDagSealedCheckpointStore,
    GovernanceDagSealedStateRecord, GovernanceDagSealedStateSlot,
    governance::{
        GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_VERSION_V1, GovernanceFilesystemRootGuard,
        RuntimeDagProducerCheckpointV1, runtime_dag_producer_root_digest,
        validate_runtime_dag_snapshot_authority_lineage,
    },
    governance_rooted_fs::{ExpectedFile, FileBinding, FileSnapshot, RetainedFile},
};
use axum::{
    Router,
    body::Body,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
    routing::get,
};
use iroha_config::{
    base::toml::TomlSource,
    parameters::{
        ProductionRuntimeHandleError,
        actual::{SorafsGovernanceDagService, SorafsGovernanceDagServiceView},
        validate_production_runtime_handle,
    },
};
use iroha_crypto::{Algorithm, PublicKey};
use norito::{
    core::DecodeLimits,
    derive::{NoritoDeserialize, NoritoSerialize},
    json::{self, Map as JsonMap, Value as JsonValue},
};
use reqwest::{Client, Method, RequestBuilder, redirect::Policy};
#[cfg(test)]
use sorafs_manifest::validate_governance_dag_head_against_chain_v1;
use sorafs_manifest::{
    GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1, GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1,
    GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1,
    GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1, GovernanceDagBlockV1,
    GovernanceDagHeadV1, GovernanceLogPayloadV1, GovernanceSignatureAlgorithm,
    MAX_REPUTATION_TRUST_EDGES, validate_governance_dag_head_against_rotatable_chain_v1,
};
use thiserror::Error;
use tokio::{net::TcpListener, signal, sync::RwLock, time};
use url::Url;

const CONFIG_MAX_BYTES: u64 = 1024 * 1024;
const MUTABLE_STATE_MAX_BYTES: u64 = 64 * 1024 * 1024;
const RUNTIME_INDEX_MAX_BYTES: u64 = 64 * 1024 * 1024;
const CHECKPOINT_VERSION_V1: u8 = 1;
const PUBLISH_INTENT_VERSION_V1: u8 = 1;
const RUNTIME_INDEX_SCHEMA: &str = "sorafs.governance_dag.runtime_signed_index.v1";
const MIRROR_INDEX_SCHEMA: &str = "sorafs.governance_dag.mirror.v1";
const MIRROR_INDEX_FILE: &str = "mirror-index.json";
const SERVICE_LOCK_FILE: &str = ".service.lock";
const MAX_DNS_ADDRESSES: usize = 8;
const MAX_RESPONSE_HEADERS: usize = 64;
const MAX_RESPONSE_HEADER_BYTES: usize = 16 * 1024;
const MAX_IPFS_CID_BYTES: usize = 160;
const MAX_PUBLIC_TOKEN_BYTES: usize = 512;
const SOURCE_ENTRY_HARD_CAP: usize = 131_072;
const SOURCE_TOTAL_BYTES_HARD_CAP: u64 = 1024 * 1024 * 1024;
const IPFS_MULTIPART_BOUNDARY_PREFIX: &str = "iroha-sorafs-gdag-v1";
// Norito temporarily copies nested length-delimited fields while decoding.
// The governed block/head schemas stay below this amplification, while the
// finite multiplier still rejects archives that attempt allocation bombs.
const CANONICAL_DECODE_ALLOCATION_MULTIPLIER: usize = 16;
const CANONICAL_DECODE_MAX_TOTAL_ELEMENTS: usize = 4_000_000;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)]
unsafe extern "C" {
    fn geteuid() -> std::os::raw::c_uint;
}

#[derive(Debug, Error)]
/// Fail-closed Governance DAG service startup or reconciliation error.
pub enum GovernanceDagServiceError {
    /// Resolved service policy is missing or invalid.
    #[error("configuration rejected: {0}")]
    Config(String),
    /// A filesystem object violates the service's ownership/link/type policy.
    #[error("filesystem safety check failed: {0}")]
    Filesystem(String),
    /// The signed local source snapshot is invalid or inconsistent.
    #[error("source snapshot rejected: {0}")]
    Source(String),
    /// Sealed durable state is invalid, unavailable, or non-monotonic.
    #[error("durable state rejected: {0}")]
    State(String),
    /// Authenticated publication or verified readback failed.
    #[error("network publication failed: {0}")]
    Network(String),
    /// Compare-and-swap or public-head continuity failed.
    #[error("public head conflict: {0}")]
    Conflict(String),
    /// The bounded local status/query listener failed.
    #[error("service listener failed: {0}")]
    Listener(String),
}

#[derive(Clone, Default)]
/// Deployment-owned runtime providers for the supervised Governance DAG service.
///
/// An empty container is intentionally constructible for assembly and negative
/// tests, but startup rejects it before service state is opened. Production
/// registries attach opaque HSM/credential/checkpoint implementations through
/// the builder methods below.
pub struct GovernanceDagServiceRuntimeProviders {
    ipfs_authenticator: Option<Arc<dyn GovernanceDagRequestAuthenticator>>,
    head_authenticator: Option<Arc<dyn GovernanceDagRequestAuthenticator>>,
    checkpoint_store: Option<Arc<dyn GovernanceDagSealedCheckpointStore>>,
}

impl fmt::Debug for GovernanceDagServiceRuntimeProviders {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernanceDagServiceRuntimeProviders")
            .field("ipfs_authenticator", &self.ipfs_authenticator.is_some())
            .field("head_authenticator", &self.head_authenticator.is_some())
            .field("checkpoint_store", &self.checkpoint_store.is_some())
            .finish()
    }
}

impl GovernanceDagServiceRuntimeProviders {
    /// Attach the rotation-aware Kubo/IPFS/IPNS authenticator.
    #[must_use]
    pub fn with_ipfs_authenticator(
        mut self,
        authenticator: Arc<dyn GovernanceDagRequestAuthenticator>,
    ) -> Self {
        self.ipfs_authenticator = Some(authenticator);
        self
    }

    /// Attach the rotation-aware signed-head CAS authenticator.
    #[must_use]
    pub fn with_head_authenticator(
        mut self,
        authenticator: Arc<dyn GovernanceDagRequestAuthenticator>,
    ) -> Self {
        self.head_authenticator = Some(authenticator);
        self
    }

    /// Attach the sealed, monotonic checkpoint and publish-intent store.
    #[must_use]
    pub fn with_checkpoint_store(
        mut self,
        checkpoint_store: Arc<dyn GovernanceDagSealedCheckpointStore>,
    ) -> Self {
        self.checkpoint_store = Some(checkpoint_store);
        self
    }
}

/// Public stable-handle bindings requested from a deployment runtime registry.
///
/// This value contains no credentials, private keys, provider diagnostics, or
/// endpoint secrets. A deployment registry uses it only to select already
/// provisioned HSM/authentication/sealed-CAS adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceDagServiceRuntimeProviderBindingsV1 {
    ipfs_authenticator_handle: String,
    ipfs_authenticator_qualification: GovernanceDagRuntimeProviderQualificationV1,
    ipfs_request_auth_public_key: [u8; 32],
    head_authenticator_handle: Option<String>,
    head_authenticator_qualification: Option<GovernanceDagRuntimeProviderQualificationV1>,
    head_request_auth_public_key: Option<[u8; 32]>,
    request_auth_max_envelope_lifetime_secs: u64,
    request_auth_max_future_skew_secs: u64,
    checkpoint_store_handle: String,
    checkpoint_store_qualification: GovernanceDagRuntimeProviderQualificationV1,
}

impl GovernanceDagServiceRuntimeProviderBindingsV1 {
    /// Stable handle for the Kubo/IPFS/IPNS authenticator.
    #[must_use]
    pub fn ipfs_authenticator_handle(&self) -> &str {
        &self.ipfs_authenticator_handle
    }

    /// Exact configured Kubo/IPFS/IPNS authenticator qualification.
    #[must_use]
    pub const fn ipfs_authenticator_qualification(
        &self,
    ) -> GovernanceDagRuntimeProviderQualificationV1 {
        self.ipfs_authenticator_qualification
    }

    /// Configured raw Ed25519 key pin for IPFS request authentication.
    #[must_use]
    pub const fn ipfs_request_auth_public_key(&self) -> [u8; 32] {
        self.ipfs_request_auth_public_key
    }

    /// Stable handle for signed-head authentication, when that mode is active.
    #[must_use]
    pub fn head_authenticator_handle(&self) -> Option<&str> {
        self.head_authenticator_handle.as_deref()
    }

    /// Exact configured signed-head authenticator qualification, when active.
    #[must_use]
    pub const fn head_authenticator_qualification(
        &self,
    ) -> Option<GovernanceDagRuntimeProviderQualificationV1> {
        self.head_authenticator_qualification
    }

    /// Configured raw Ed25519 key pin for signed-head request authentication.
    #[must_use]
    pub const fn head_request_auth_public_key(&self) -> Option<[u8; 32]> {
        self.head_request_auth_public_key
    }

    /// Maximum accepted signed-envelope lifetime in seconds.
    #[must_use]
    pub const fn request_auth_max_envelope_lifetime_secs(&self) -> u64 {
        self.request_auth_max_envelope_lifetime_secs
    }

    /// Maximum accepted future issuance skew in seconds.
    #[must_use]
    pub const fn request_auth_max_future_skew_secs(&self) -> u64 {
        self.request_auth_max_future_skew_secs
    }

    /// Stable handle for the sealed monotonic checkpoint store.
    #[must_use]
    pub fn checkpoint_store_handle(&self) -> &str {
        &self.checkpoint_store_handle
    }

    /// Exact configured sealed-checkpoint store qualification.
    #[must_use]
    pub const fn checkpoint_store_qualification(
        &self,
    ) -> GovernanceDagRuntimeProviderQualificationV1 {
        self.checkpoint_store_qualification
    }
}

/// Redacted deployment-registry resolution failure.
///
/// Variants deliberately carry no provider diagnostics because HSM, KMS, and
/// credential-control-plane errors can contain secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum GovernanceDagServiceRuntimeProviderRegistryErrorV1 {
    /// The deployment registry or its control plane is unavailable.
    #[error("Governance DAG runtime provider registry is unavailable")]
    Unavailable,
    /// The deployment registry policy is stale or revoked.
    #[error("Governance DAG runtime provider registry policy is stale or revoked")]
    StaleOrRevoked,
    /// The registry rejected the exact configured stable-handle bindings.
    #[error("Governance DAG runtime provider registry rejected the configured bindings")]
    RejectedBindings,
}

/// Deployment-owned factory for Governance DAG runtime providers.
///
/// Implementations resolve only the stable handles in `bindings`. Credentials,
/// private keys, tokens, and provider diagnostics must stay inside the registry
/// and returned adapters. The service independently qualifies every returned
/// adapter and verifies its exact configured handle before touching durable
/// service state.
pub trait GovernanceDagServiceRuntimeProviderRegistryV1: Send + Sync {
    /// Resolve one coherent provider set for the exact configured bindings.
    fn resolve(
        &self,
        bindings: &GovernanceDagServiceRuntimeProviderBindingsV1,
    ) -> Result<
        GovernanceDagServiceRuntimeProviders,
        GovernanceDagServiceRuntimeProviderRegistryErrorV1,
    >;
}

/// Typed startup failure for the registry-aware Governance DAG launcher.
#[derive(Debug, Error)]
pub enum GovernanceDagServiceLauncherError {
    /// The packaged launcher was not supplied a deployment runtime registry.
    #[error("Governance DAG runtime provider registry was not injected")]
    MissingRuntimeProviderRegistry,
    /// The injected registry failed without exposing provider diagnostics.
    #[error(transparent)]
    RuntimeProviderRegistry(#[from] GovernanceDagServiceRuntimeProviderRegistryErrorV1),
    /// Configuration or qualified service startup failed.
    #[error(transparent)]
    Service(#[from] GovernanceDagServiceError),
}

#[derive(Clone)]
struct OpaqueAuthenticator {
    handle: String,
    qualification: GovernanceDagRuntimeProviderQualificationV1,
    verification_policy: GovernanceDagRequestAuthenticationPolicyV1,
    provider: Arc<dyn GovernanceDagRequestAuthenticator>,
    replay_cache: Arc<Mutex<GovernanceDagRequestAuthenticationReplayCacheV1>>,
}

impl fmt::Debug for OpaqueAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpaqueAuthenticator")
            .field("handle", &self.handle)
            .field(
                "public_key",
                &hex::encode(self.verification_policy.public_key()),
            )
            .field(
                "max_envelope_lifetime_secs",
                &self.verification_policy.max_envelope_lifetime_secs(),
            )
            .field(
                "max_future_skew_secs",
                &self.verification_policy.max_future_skew_secs(),
            )
            .field("provider", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl OpaqueAuthenticator {
    fn try_new(
        expected_handle: &str,
        expected_qualification: GovernanceDagRuntimeProviderQualificationV1,
        expected_public_key: [u8; 32],
        max_envelope_lifetime_secs: u64,
        max_future_skew_secs: u64,
        provider: Arc<dyn GovernanceDagRequestAuthenticator>,
        label: &'static str,
    ) -> Result<Self, GovernanceDagServiceError> {
        let handle = validate_runtime_handle(expected_handle, label)?;
        if !expected_qualification.is_valid() {
            return Err(GovernanceDagServiceError::Config(format!(
                "{label} configured policy qualification is invalid"
            )));
        }
        let verification_policy = validate_request_auth_policy(
            expected_public_key,
            max_envelope_lifetime_secs,
            max_future_skew_secs,
            label,
        )?;
        let provider_handle = validate_runtime_handle(provider.handle(), label)?;
        if provider_handle != handle {
            return Err(GovernanceDagServiceError::Config(format!(
                "{label} provider handle does not match configured handle"
            )));
        }
        if provider.public_key() != expected_public_key {
            return Err(GovernanceDagServiceError::Config(format!(
                "{label} provider public key does not match configuration"
            )));
        }
        let qualification = provider.qualification().map_err(|_| {
            GovernanceDagServiceError::Config(format!(
                "{label} provider is unavailable, stale, or unqualified"
            ))
        })?;
        if !qualification.is_valid() || qualification != expected_qualification {
            return Err(GovernanceDagServiceError::Config(format!(
                "{label} provider qualification does not match configuration"
            )));
        }
        let rechecked_qualification = provider.qualification().map_err(|_| {
            GovernanceDagServiceError::Config(format!(
                "{label} provider is unavailable, stale, or unqualified"
            ))
        })?;
        if provider.handle() != handle
            || provider.public_key() != expected_public_key
            || rechecked_qualification != expected_qualification
        {
            return Err(GovernanceDagServiceError::Config(format!(
                "{label} provider identity, public key, or policy changed during startup qualification"
            )));
        }
        Ok(Self {
            handle,
            qualification: expected_qualification,
            verification_policy,
            provider,
            replay_cache: Arc::new(Mutex::new(
                GovernanceDagRequestAuthenticationReplayCacheV1::new(),
            )),
        })
    }

    fn authenticate(
        &self,
        request: &GovernanceDagCanonicalRequestV1,
    ) -> Result<GovernanceDagRequestAuthenticationEnvelopeV1, GovernanceDagServiceError> {
        self.assert_identity()?;
        let result = self.provider.authenticate(request);
        self.assert_identity()?;
        let envelope = result.map_err(|_| {
            GovernanceDagServiceError::Network(
                "Governance DAG authenticator refused the outbound request".to_owned(),
            )
        })?;
        self.validate_envelope(request, &envelope)?;
        Ok(envelope)
    }

    fn assert_identity(&self) -> Result<(), GovernanceDagServiceError> {
        let qualification = self.provider.qualification().map_err(|_| {
            GovernanceDagServiceError::Network(
                "Governance DAG authenticator is unavailable, stale, or unqualified".to_owned(),
            )
        })?;
        if self.provider.handle() != self.handle
            || self.provider.public_key() != self.verification_policy.public_key()
            || qualification != self.qualification
        {
            return Err(GovernanceDagServiceError::Network(
                "Governance DAG authenticator identity, public key, or policy changed after injection"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_envelope(
        &self,
        request: &GovernanceDagCanonicalRequestV1,
        envelope: &GovernanceDagRequestAuthenticationEnvelopeV1,
    ) -> Result<(), GovernanceDagServiceError> {
        let now = current_unix_timestamp_seconds();
        let mut replay_cache = self.replay_cache.lock().map_err(|_| {
            GovernanceDagServiceError::Network(
                "Governance DAG request-auth replay state is unavailable".to_owned(),
            )
        })?;
        verify_governance_dag_request_authentication_v1(
            request,
            envelope,
            request.scope(),
            &self.verification_policy,
            now,
            &mut replay_cache,
        )
        .map_err(|error| GovernanceDagServiceError::Network(error.to_string()))
    }
}

fn validate_request_auth_policy(
    public_key: [u8; 32],
    max_envelope_lifetime_secs: u64,
    max_future_skew_secs: u64,
    label: &str,
) -> Result<GovernanceDagRequestAuthenticationPolicyV1, GovernanceDagServiceError> {
    GovernanceDagRequestAuthenticationPolicyV1::try_new(
        public_key,
        max_envelope_lifetime_secs,
        max_future_skew_secs,
    )
    .map_err(|error| {
        let reason = match error {
            GovernanceDagRequestAuthenticationErrorV1::InvalidPolicyTiming => {
                "request-auth timing bounds are invalid"
            }
            GovernanceDagRequestAuthenticationErrorV1::InvalidPolicyPublicKey => {
                "request-auth public key is not canonical Ed25519"
            }
            _ => "request-auth policy is invalid",
        };
        GovernanceDagServiceError::Config(format!("{label} {reason}"))
    })
}

#[derive(Clone)]
struct OpaqueCheckpointStore {
    handle: String,
    qualification: GovernanceDagRuntimeProviderQualificationV1,
    provider: Arc<dyn GovernanceDagSealedCheckpointStore>,
}

impl fmt::Debug for OpaqueCheckpointStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpaqueCheckpointStore")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

impl OpaqueCheckpointStore {
    fn try_new(
        expected_handle: &str,
        expected_qualification: GovernanceDagRuntimeProviderQualificationV1,
        provider: Arc<dyn GovernanceDagSealedCheckpointStore>,
    ) -> Result<Self, GovernanceDagServiceError> {
        let handle = validate_runtime_handle(expected_handle, "sealed checkpoint store")?;
        if !expected_qualification.is_valid() {
            return Err(GovernanceDagServiceError::Config(
                "sealed checkpoint store configured policy qualification is invalid".to_owned(),
            ));
        }
        let provider_handle =
            validate_runtime_handle(provider.handle(), "sealed checkpoint store")?;
        if provider_handle != handle {
            return Err(GovernanceDagServiceError::Config(
                "sealed checkpoint store provider handle does not match configured handle"
                    .to_owned(),
            ));
        }
        let qualification = provider.qualification().map_err(|_| {
            GovernanceDagServiceError::Config(
                "sealed checkpoint store is unavailable, stale, or unqualified".to_owned(),
            )
        })?;
        if !qualification.is_valid() || qualification != expected_qualification {
            return Err(GovernanceDagServiceError::Config(
                "sealed checkpoint store qualification does not match configuration".to_owned(),
            ));
        }
        let rechecked_qualification = provider.qualification().map_err(|_| {
            GovernanceDagServiceError::Config(
                "sealed checkpoint store is unavailable, stale, or unqualified".to_owned(),
            )
        })?;
        if provider.handle() != handle || rechecked_qualification != expected_qualification {
            return Err(GovernanceDagServiceError::Config(
                "sealed checkpoint store identity or policy changed during startup qualification"
                    .to_owned(),
            ));
        }
        Ok(Self {
            handle,
            qualification: expected_qualification,
            provider,
        })
    }

    fn assert_identity(&self) -> Result<(), GovernanceDagServiceError> {
        let qualification = self.provider.qualification().map_err(|_| {
            GovernanceDagServiceError::State(
                "sealed checkpoint store is unavailable, stale, or unqualified".to_owned(),
            )
        })?;
        if self.provider.handle() != self.handle || qualification != self.qualification {
            return Err(GovernanceDagServiceError::State(
                "sealed checkpoint store identity or policy changed after injection".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct PublishedBlockV1 {
    sequence: u64,
    governance_block_cid: Vec<u8>,
    governance_node_cid: Vec<u8>,
    payload_kind: String,
    timestamp: u64,
    encoded_blake3: [u8; 32],
    encoded_len: u64,
    ipfs_cid: String,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct CheckpointBodyV1 {
    version: u8,
    generation: u64,
    head_block_cid: Vec<u8>,
    block_count: u64,
    head_bytes_blake3: [u8; 32],
    head_ipfs_cid: String,
    public_head_token: String,
    source_index_blake3: [u8; 32],
    mirror_blake3: [u8; 32],
    published_at_unix: u64,
    mirror_blocks: Vec<PublishedBlockV1>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct IntentBlockV1 {
    sequence: u64,
    governance_block_cid: Vec<u8>,
    governance_node_cid: Vec<u8>,
    payload_kind: String,
    timestamp: u64,
    encoded_blake3: [u8; 32],
    encoded_len: u64,
    ipfs_cid: Option<String>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct PublishIntentBodyV1 {
    version: u8,
    generation: u64,
    target_head_block_cid: Vec<u8>,
    target_block_count: u64,
    target_head_bytes: Vec<u8>,
    target_head_blake3: [u8; 32],
    target_source_index_blake3: [u8; 32],
    previous_public_head_blake3: Option<[u8; 32]>,
    created_at_unix: u64,
    blocks: Vec<IntentBlockV1>,
    head_ipfs_cid: Option<String>,
}

#[derive(Debug, Clone)]
struct SourceBlock {
    block: GovernanceDagBlockV1,
    bytes: Vec<u8>,
    encoded_blake3: [u8; 32],
    payload_kind: String,
}

#[derive(Debug, Clone)]
struct SourceSnapshot {
    index_blake3: [u8; 32],
    head: GovernanceDagHeadV1,
    head_bytes: Vec<u8>,
    blocks: Vec<SourceBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProducerCommitGuard {
    record: GovernanceDagSealedStateRecord,
    checkpoint: RuntimeDagProducerCheckpointV1,
}

#[derive(Debug)]
struct RuntimeConfig {
    source_dir: PathBuf,
    source_root_guard: GovernanceFilesystemRootGuard,
    state_root_guard: GovernanceFilesystemRootGuard,
    listen_addr: SocketAddr,
    poll_interval: Duration,
    max_response_bytes: u64,
    max_request_bytes: u64,
    mirror_max_entries: usize,
    mirror_max_bytes: u64,
    max_head_age_secs: u64,
    max_future_skew_secs: u64,
    allow_head_bootstrap: bool,
    expected_producer_signer_handle: String,
    expected_producer_signer_qualification: GovernanceDagRuntimeProviderQualificationV1,
    expected_publisher_peer_id: Vec<u8>,
    expected_public_key: [u8; 32],
}

impl RuntimeConfig {
    fn revalidate_source_root(&self) -> Result<(), GovernanceDagServiceError> {
        self.source_root_guard.revalidate().map_err(|err| {
            GovernanceDagServiceError::Filesystem(format!(
                "Governance DAG source root identity changed: {err}"
            ))
        })
    }

    fn revalidate_state_root(&self) -> Result<(), GovernanceDagServiceError> {
        self.state_root_guard.revalidate().map_err(|err| {
            GovernanceDagServiceError::Filesystem(format!(
                "Governance DAG state root identity changed: {err}"
            ))
        })
    }
}

#[derive(Debug)]
struct PinnedEndpoint {
    url: Url,
    client: Client,
    authentication_scope: GovernanceDagAuthenticationScope,
    authenticator: OpaqueAuthenticator,
    max_request_bytes: u64,
}

#[derive(Debug)]
enum HeadMode {
    SignedHttp(PinnedEndpoint),
    Ipns { name: String, key_name: String },
}

#[derive(Debug, Clone)]
enum PublicHead {
    Missing,
    Present { bytes: Vec<u8>, token: String },
}

#[derive(Debug, Clone, Default)]
struct ServiceMetrics {
    publish_success_total: u64,
    publish_failure_total: u64,
    published_bytes_total: u64,
    last_publish_timestamp_seconds: u64,
    backlog: u64,
    head_age_seconds: u64,
    ipfs_pin_lag_seconds: u64,
    ipns_update_success_total: u64,
    ipns_update_failure_total: u64,
    last_ipns_update_timestamp_seconds: u64,
    validation_failure_total: u64,
    mirror_drift: u64,
}

#[derive(Debug, Clone, Default)]
struct ApiSnapshot {
    live: bool,
    ready: bool,
    last_error: Option<String>,
    mirror: Option<JsonValue>,
    checkpoint: Option<CheckpointBodyV1>,
    metrics: ServiceMetrics,
}

#[derive(Clone)]
struct ApiState(Arc<RwLock<ApiSnapshot>>);

struct Service {
    config: RuntimeConfig,
    checkpoint_store: OpaqueCheckpointStore,
    checkpoint_revision: Option<[u8; 32]>,
    checkpoint_generation_floor: u64,
    checkpoint: Option<CheckpointBodyV1>,
    intent_revision: Option<[u8; 32]>,
    intent_generation_floor: u64,
    intent: Option<PublishIntentBodyV1>,
    ipfs: PinnedEndpoint,
    head_mode: HeadMode,
    api: ApiState,
    state_lock: RetainedFile,
}

/// Run the supervised Governance DAG publisher using injected runtime providers.
///
/// The config contains only public endpoint policy, expected identities, and
/// opaque provider handles. The supplied providers must match those handles;
/// missing or mismatched dependencies fail before listener or network startup.
///
/// # Errors
///
/// Returns a fail-closed error for invalid configuration/source/state,
/// unavailable providers, CAS conflicts, publication failures, or listener
/// failures.
pub async fn run_governance_dag_service(
    config_path: impl AsRef<Path>,
    once: bool,
    providers: GovernanceDagServiceRuntimeProviders,
) -> Result<(), GovernanceDagServiceError> {
    let view = load_service_config(config_path.as_ref())?;
    run_governance_dag_service_from_view(view, once, providers).await
}

/// Run the supervised Governance DAG publisher through a deployment registry.
///
/// The registry receives only validated stable provider handles. The packaged
/// launcher passes `None` when no supported deployment registry was linked and
/// therefore fails with a typed error before service state is opened.
///
/// # Errors
///
/// Returns [`GovernanceDagServiceLauncherError::MissingRuntimeProviderRegistry`]
/// when no registry was injected, a redacted typed registry error when provider
/// construction fails, or the underlying fail-closed service startup error.
pub async fn run_governance_dag_service_with_runtime_registry(
    config_path: impl AsRef<Path>,
    once: bool,
    registry: Option<Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1>>,
) -> Result<(), GovernanceDagServiceLauncherError> {
    let view = load_service_config(config_path.as_ref())?;
    let providers = resolve_runtime_registry_providers(&view, registry)?;
    run_governance_dag_service_from_view(view, once, providers).await?;
    Ok(())
}

/// Qualify every runtime-only provider against one resolved service view.
///
/// This performs the same exact handle/revision/policy checks as service
/// construction without opening filesystem state, resolving endpoints, or
/// starting a listener. Embedding launchers use it before starting any node
/// subsystem so missing, substituted, stale, or test-marked adapters fail the
/// launch rather than a later background task.
///
/// # Errors
///
/// Returns a fail-closed configuration error when a required provider is
/// missing, its public identity differs from configuration, its qualification
/// is unavailable or stale, or an unexpected signed-head provider is supplied
/// in IPNS mode.
pub fn validate_governance_dag_service_runtime_providers(
    view: &SorafsGovernanceDagServiceView,
    providers: &GovernanceDagServiceRuntimeProviders,
) -> Result<(), GovernanceDagServiceError> {
    let bindings = runtime_provider_bindings(view)?;
    OpaqueAuthenticator::try_new(
        bindings.ipfs_authenticator_handle(),
        bindings.ipfs_authenticator_qualification(),
        bindings.ipfs_request_auth_public_key(),
        bindings.request_auth_max_envelope_lifetime_secs(),
        bindings.request_auth_max_future_skew_secs(),
        providers.ipfs_authenticator.clone().ok_or_else(|| {
            GovernanceDagServiceError::Config(
                "IPFS authentication is enabled but no runtime provider was injected".to_owned(),
            )
        })?,
        "IPFS authenticator",
    )?;
    OpaqueCheckpointStore::try_new(
        bindings.checkpoint_store_handle(),
        bindings.checkpoint_store_qualification(),
        providers.checkpoint_store.clone().ok_or_else(|| {
            GovernanceDagServiceError::Config(
                "sealed checkpoint store is enabled but no runtime provider was injected"
                    .to_owned(),
            )
        })?,
    )?;
    match (
        bindings.head_authenticator_handle(),
        bindings.head_authenticator_qualification(),
        bindings.head_request_auth_public_key(),
        providers.head_authenticator.clone(),
    ) {
        (Some(handle), Some(qualification), Some(public_key), Some(provider)) => {
            OpaqueAuthenticator::try_new(
                handle,
                qualification,
                public_key,
                bindings.request_auth_max_envelope_lifetime_secs(),
                bindings.request_auth_max_future_skew_secs(),
                provider,
                "signed-head authenticator",
            )?;
        }
        (Some(_), Some(_), Some(_), None) => {
            return Err(GovernanceDagServiceError::Config(
                "signed-head authentication is enabled but no runtime provider was injected"
                    .to_owned(),
            ));
        }
        (None, None, None, None) => {}
        (None, None, None, Some(_)) => {
            return Err(GovernanceDagServiceError::Config(
                "signed-head authenticator provider must be absent in IPNS mode".to_owned(),
            ));
        }
        _ => {
            return Err(GovernanceDagServiceError::Config(
                "signed-head authenticator binding is incomplete".to_owned(),
            ));
        }
    }
    Ok(())
}

fn resolve_runtime_registry_providers(
    view: &SorafsGovernanceDagServiceView,
    registry: Option<Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1>>,
) -> Result<GovernanceDagServiceRuntimeProviders, GovernanceDagServiceLauncherError> {
    let bindings = runtime_provider_bindings(view)?;
    let registry =
        registry.ok_or(GovernanceDagServiceLauncherError::MissingRuntimeProviderRegistry)?;
    Ok(registry.resolve(&bindings)?)
}

fn runtime_provider_bindings(
    view: &SorafsGovernanceDagServiceView,
) -> Result<GovernanceDagServiceRuntimeProviderBindingsV1, GovernanceDagServiceError> {
    let service = &view.service;
    if !service.enabled {
        return Err(GovernanceDagServiceError::Config(
            "sorafs.storage.governance_dag_service.enabled must be true".to_owned(),
        ));
    }
    let ipfs_authenticator_handle = validate_runtime_handle(
        service
            .ipfs_authenticator_handle
            .as_deref()
            .ok_or_else(|| {
                GovernanceDagServiceError::Config("IPFS authenticator handle is missing".to_owned())
            })?,
        "IPFS authenticator",
    )?;
    let ipfs_authenticator_qualification = configured_provider_qualification(
        service.ipfs_authenticator_revision,
        service.ipfs_authenticator_policy_digest,
        "IPFS authenticator",
    )?;
    let ipfs_request_auth_public_key = service.ipfs_request_auth_public_key.ok_or_else(|| {
        GovernanceDagServiceError::Config("IPFS request-auth public key is missing".to_owned())
    })?;
    validate_request_auth_policy(
        ipfs_request_auth_public_key,
        service.request_auth_max_envelope_lifetime_secs,
        service.request_auth_max_future_skew_secs,
        "IPFS authenticator",
    )?;
    let checkpoint_store_handle = validate_runtime_handle(
        service.checkpoint_store_handle.as_deref().ok_or_else(|| {
            GovernanceDagServiceError::Config("checkpoint store handle is missing".to_owned())
        })?,
        "sealed checkpoint store",
    )?;
    let checkpoint_store_qualification = configured_provider_qualification(
        service.checkpoint_store_revision,
        service.checkpoint_store_policy_digest,
        "sealed checkpoint store",
    )?;
    let (head_authenticator_handle, head_authenticator_qualification, head_request_auth_public_key) =
        match service.head_mode.as_str() {
            "signed_http" => {
                let public_key = service.head_request_auth_public_key.ok_or_else(|| {
                    GovernanceDagServiceError::Config(
                        "signed-head request-auth public key is missing".to_owned(),
                    )
                })?;
                validate_request_auth_policy(
                    public_key,
                    service.request_auth_max_envelope_lifetime_secs,
                    service.request_auth_max_future_skew_secs,
                    "signed-head authenticator",
                )?;
                (
                    Some(validate_runtime_handle(
                        service
                            .head_authenticator_handle
                            .as_deref()
                            .ok_or_else(|| {
                                GovernanceDagServiceError::Config(
                                    "signed-head authenticator handle is missing".to_owned(),
                                )
                            })?,
                        "signed-head authenticator",
                    )?),
                    Some(configured_provider_qualification(
                        service.head_authenticator_revision,
                        service.head_authenticator_policy_digest,
                        "signed-head authenticator",
                    )?),
                    Some(public_key),
                )
            }
            "ipns" => {
                if service.head_authenticator_handle.is_some()
                    || service.head_authenticator_revision.is_some()
                    || service.head_authenticator_policy_digest.is_some()
                    || service.head_request_auth_public_key.is_some()
                {
                    return Err(GovernanceDagServiceError::Config(
                        "signed-head authenticator binding must be absent in IPNS mode".to_owned(),
                    ));
                }
                (None, None, None)
            }
            _ => {
                return Err(GovernanceDagServiceError::Config(
                    "head_mode must be signed_http or ipns".to_owned(),
                ));
            }
        };
    Ok(GovernanceDagServiceRuntimeProviderBindingsV1 {
        ipfs_authenticator_handle,
        ipfs_authenticator_qualification,
        ipfs_request_auth_public_key,
        head_authenticator_handle,
        head_authenticator_qualification,
        head_request_auth_public_key,
        request_auth_max_envelope_lifetime_secs: service.request_auth_max_envelope_lifetime_secs,
        request_auth_max_future_skew_secs: service.request_auth_max_future_skew_secs,
        checkpoint_store_handle,
        checkpoint_store_qualification,
    })
}

fn configured_provider_qualification(
    revision: Option<u64>,
    policy_digest: Option<[u8; 32]>,
    label: &'static str,
) -> Result<GovernanceDagRuntimeProviderQualificationV1, GovernanceDagServiceError> {
    let qualification = GovernanceDagRuntimeProviderQualificationV1::new(
        revision.ok_or_else(|| {
            GovernanceDagServiceError::Config(format!("{label} revision is missing"))
        })?,
        policy_digest.ok_or_else(|| {
            GovernanceDagServiceError::Config(format!("{label} policy digest is missing"))
        })?,
    );
    if !qualification.is_valid() {
        return Err(GovernanceDagServiceError::Config(format!(
            "{label} configured policy qualification is invalid"
        )));
    }
    Ok(qualification)
}

/// Run the supervised publisher from an already validated standalone view.
///
/// Embedding launchers use this entrypoint so they do not need to re-read a
/// configuration file that may also contain validator-only settings. Runtime
/// providers are requalified during service construction and around every
/// authenticated or sealed-store operation.
///
/// # Errors
///
/// Returns a fail-closed error for provider qualification, source/state
/// validation, endpoint pinning, publication/readback, public-head continuity,
/// or listener failures.
pub async fn run_governance_dag_service_from_view(
    view: SorafsGovernanceDagServiceView,
    once: bool,
    providers: GovernanceDagServiceRuntimeProviders,
) -> Result<(), GovernanceDagServiceError> {
    if once {
        let mut service = Service::from_view(view, providers).await?;
        service.reconcile_once().await?;
        return Ok(());
    }
    prepare_governance_dag_service_from_view(view, providers)
        .await?
        .run()
        .await
}

/// Prepared continuous Governance DAG service owned by a supervisor.
///
/// Construction completes provider qualification, source/state validation,
/// endpoint pinning, sealed-state load, and listener binding. A launcher may
/// therefore finish preparation before reporting successful startup.
pub struct GovernanceDagServiceRunner {
    service: Service,
    listener: TcpListener,
}

impl GovernanceDagServiceRunner {
    /// Reconcile continuously until the listener exits or an operating-system
    /// shutdown signal is received.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed publication, reconciliation, or listener error.
    pub async fn run(self) -> Result<(), GovernanceDagServiceError> {
        self.run_until(shutdown_signal()).await
    }

    /// Reconcile continuously until the listener exits or the embedding
    /// supervisor requests shutdown.
    ///
    /// Standalone launchers should use [`Self::run`]. Embedded launchers pass
    /// their existing supervisor signal here so the service does not install a
    /// competing operating-system signal consumer and can drain its listener
    /// gracefully during programmatic shutdown.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed publication, reconciliation, or listener error.
    pub async fn run_until<F>(mut self, shutdown: F) -> Result<(), GovernanceDagServiceError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let router = service_router(self.service.api.clone());
        let api = self.service.api.clone();
        api.0.write().await.live = true;
        let server = axum::serve(self.listener, router.into_make_service())
            .with_graceful_shutdown(shutdown)
            .into_future();
        tokio::pin!(server);

        let mut interval = time::interval(self.service.config.poll_interval);
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                result = &mut server => {
                    return result.map_err(|err| GovernanceDagServiceError::Listener(err.to_string()));
                }
                _ = interval.tick() => {
                    if let Err(err) = self.service.reconcile_once().await {
                        let mut state = self.service.api.0.write().await;
                        state.ready = false;
                        state.last_error = Some(err.to_string());
                        state.metrics.publish_failure_total = state.metrics.publish_failure_total.saturating_add(1);
                        state.metrics.validation_failure_total = state.metrics.validation_failure_total.saturating_add(1);
                        if matches!(&self.service.head_mode, HeadMode::Ipns { .. }) {
                            state.metrics.ipns_update_failure_total = state.metrics.ipns_update_failure_total.saturating_add(1);
                        }
                        eprintln!("governance DAG reconciliation failed; readiness withdrawn: {err}");
                    }
                }
            }
        }
    }
}

/// Prepare a continuous Governance DAG service from a resolved view.
///
/// The returned runner has already qualified all providers, opened and
/// reconciled sealed local state, pinned endpoint addresses, and bound its
/// loopback status listener. It has not yet attempted publication.
///
/// # Errors
///
/// Returns a fail-closed provider, source/state, endpoint, or listener error.
pub async fn prepare_governance_dag_service_from_view(
    view: SorafsGovernanceDagServiceView,
    providers: GovernanceDagServiceRuntimeProviders,
) -> Result<GovernanceDagServiceRunner, GovernanceDagServiceError> {
    let mut service = Service::from_view(view, providers).await?;
    service.validate_initial_state().await?;
    let listener = TcpListener::bind(service.config.listen_addr)
        .await
        .map_err(|err| GovernanceDagServiceError::Listener(err.to_string()))?;
    Ok(GovernanceDagServiceRunner { service, listener })
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut signal) = signal(SignalKind::terminate()) {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

fn load_service_config(
    path: &Path,
) -> Result<SorafsGovernanceDagServiceView, GovernanceDagServiceError> {
    let bytes = read_unrooted_regular_file(path, CONFIG_MAX_BYTES, false)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| {
        GovernanceDagServiceError::Config("configuration file is not UTF-8".to_owned())
    })?;
    let table = text.parse().map_err(|err| {
        GovernanceDagServiceError::Config(format!("configuration TOML is invalid: {err}"))
    })?;
    SorafsGovernanceDagServiceView::from_toml_source(TomlSource::new(path.to_owned(), table))
        .map_err(|err| GovernanceDagServiceError::Config(err.to_string()))
}

impl Service {
    async fn from_view(
        view: SorafsGovernanceDagServiceView,
        providers: GovernanceDagServiceRuntimeProviders,
    ) -> Result<Self, GovernanceDagServiceError> {
        let SorafsGovernanceDagServiceView {
            source_dir,
            producer_publisher_peer_id,
            producer_signer_handle,
            producer_signer_revision,
            producer_signer_policy_digest,
            producer_publisher_public_key_hex,
            service,
        } = view;
        if !service.enabled {
            return Err(GovernanceDagServiceError::Config(
                "sorafs.storage.governance_dag_service.enabled must be true".to_owned(),
            ));
        }
        let canonical_block_max = u64::try_from(GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1)
            .map_err(|_| {
                GovernanceDagServiceError::Config(
                    "canonical Governance DAG block ceiling exceeds host limits".to_owned(),
                )
            })?;
        if service.max_request_bytes.0 < canonical_block_max {
            return Err(GovernanceDagServiceError::Config(format!(
                "max_request_bytes must be at least the canonical Governance DAG block ceiling of {canonical_block_max} bytes"
            )));
        }
        let listen_addr = service.listen_addr.parse::<SocketAddr>().map_err(|_| {
            GovernanceDagServiceError::Config("listen_addr is not a socket address".to_owned())
        })?;
        if !listen_addr.ip().is_loopback() {
            return Err(GovernanceDagServiceError::Config(
                "the Governance DAG status listener must bind a loopback address".to_owned(),
            ));
        }
        let expected_public_key = decode_strong_ed25519_public_key_hex(
            service.publisher_public_key_hex.as_deref().ok_or_else(|| {
                GovernanceDagServiceError::Config("publisher public key is missing".to_owned())
            })?,
            "publisher public key",
        )?;
        let expected_producer_public_key = decode_strong_ed25519_public_key_hex(
            producer_publisher_public_key_hex
                .as_deref()
                .ok_or_else(|| {
                    GovernanceDagServiceError::Config(
                        "local Governance DAG producer public key is missing".to_owned(),
                    )
                })?,
            "local Governance DAG producer public key",
        )?;
        if expected_producer_public_key != expected_public_key {
            return Err(GovernanceDagServiceError::Config(
                "public service key does not match the signed local producer key".to_owned(),
            ));
        }
        let expected_producer_signer_handle = validate_runtime_handle(
            producer_signer_handle.as_deref().ok_or_else(|| {
                GovernanceDagServiceError::Config(
                    "local Governance DAG producer signer handle is missing".to_owned(),
                )
            })?,
            "local Governance DAG producer signer",
        )?;
        let expected_producer_signer_qualification = configured_provider_qualification(
            producer_signer_revision,
            producer_signer_policy_digest,
            "local Governance DAG producer signer",
        )?;
        let expected_publisher_peer_id = producer_publisher_peer_id
            .ok_or_else(|| {
                GovernanceDagServiceError::Config(
                    "local Governance DAG producer peer id is missing".to_owned(),
                )
            })?
            .into_bytes();
        if expected_publisher_peer_id.is_empty()
            || expected_publisher_peer_id.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1
            || !expected_publisher_peer_id
                .iter()
                .all(|byte| byte.is_ascii_graphic())
        {
            return Err(GovernanceDagServiceError::Config(
                "local Governance DAG producer peer id is invalid".to_owned(),
            ));
        }
        let source_dir_path = source_dir.ok_or_else(|| {
            GovernanceDagServiceError::Config("governance_dag_dir is missing".to_owned())
        })?;
        let ipfs_url = service.ipfs_api_url.clone().ok_or_else(|| {
            GovernanceDagServiceError::Config("IPFS API URL is missing".to_owned())
        })?;
        let GovernanceDagServiceRuntimeProviders {
            ipfs_authenticator,
            head_authenticator,
            checkpoint_store,
        } = providers;
        let checkpoint_store_handle =
            service.checkpoint_store_handle.as_deref().ok_or_else(|| {
                GovernanceDagServiceError::Config("checkpoint store handle is missing".to_owned())
            })?;
        let checkpoint_store_qualification = configured_provider_qualification(
            service.checkpoint_store_revision,
            service.checkpoint_store_policy_digest,
            "sealed checkpoint store",
        )?;
        let checkpoint_store = OpaqueCheckpointStore::try_new(
            checkpoint_store_handle,
            checkpoint_store_qualification,
            checkpoint_store.ok_or_else(|| {
                GovernanceDagServiceError::Config(
                    "sealed checkpoint store is enabled but no runtime provider was injected"
                        .to_owned(),
                )
            })?,
        )?;

        let ipfs_authenticator_handle =
            service
                .ipfs_authenticator_handle
                .as_deref()
                .ok_or_else(|| {
                    GovernanceDagServiceError::Config(
                        "IPFS authenticator handle is missing".to_owned(),
                    )
                })?;
        let ipfs_authenticator_qualification = configured_provider_qualification(
            service.ipfs_authenticator_revision,
            service.ipfs_authenticator_policy_digest,
            "IPFS authenticator",
        )?;
        let ipfs_authenticator = OpaqueAuthenticator::try_new(
            ipfs_authenticator_handle,
            ipfs_authenticator_qualification,
            service.ipfs_request_auth_public_key.ok_or_else(|| {
                GovernanceDagServiceError::Config(
                    "IPFS request-auth public key is missing".to_owned(),
                )
            })?,
            service.request_auth_max_envelope_lifetime_secs,
            service.request_auth_max_future_skew_secs,
            ipfs_authenticator.ok_or_else(|| {
                GovernanceDagServiceError::Config(
                    "IPFS authentication is enabled but no runtime provider was injected"
                        .to_owned(),
                )
            })?,
            "IPFS authenticator",
        )?;
        enum QualifiedHeadMode {
            SignedHttp {
                url: String,
                authenticator: OpaqueAuthenticator,
            },
            Ipns {
                name: String,
                key_name: String,
            },
        }
        let qualified_head_mode = match service.head_mode.as_str() {
            "signed_http" => {
                let handle = service
                    .head_authenticator_handle
                    .as_deref()
                    .ok_or_else(|| {
                        GovernanceDagServiceError::Config(
                            "signed-head authenticator handle is missing".to_owned(),
                        )
                    })?;
                let qualification = configured_provider_qualification(
                    service.head_authenticator_revision,
                    service.head_authenticator_policy_digest,
                    "signed-head authenticator",
                )?;
                let authenticator = OpaqueAuthenticator::try_new(
                    handle,
                    qualification,
                    service.head_request_auth_public_key.ok_or_else(|| {
                        GovernanceDagServiceError::Config(
                            "signed-head request-auth public key is missing".to_owned(),
                        )
                    })?,
                    service.request_auth_max_envelope_lifetime_secs,
                    service.request_auth_max_future_skew_secs,
                    head_authenticator.ok_or_else(|| {
                        GovernanceDagServiceError::Config(
                            "signed-head authentication is enabled but no runtime provider was injected"
                                .to_owned(),
                        )
                    })?,
                    "signed-head authenticator",
                )?;
                let url = service.signed_head_url.clone().ok_or_else(|| {
                    GovernanceDagServiceError::Config("signed head URL is missing".to_owned())
                })?;
                QualifiedHeadMode::SignedHttp { url, authenticator }
            }
            "ipns" => {
                if service.head_authenticator_handle.is_some()
                    || service.head_authenticator_revision.is_some()
                    || service.head_authenticator_policy_digest.is_some()
                    || service.head_request_auth_public_key.is_some()
                    || head_authenticator.is_some()
                {
                    return Err(GovernanceDagServiceError::Config(
                        "signed-head authenticator binding must be absent in IPNS mode".to_owned(),
                    ));
                }
                QualifiedHeadMode::Ipns {
                    name: validate_public_token(
                        service.ipns_name.as_deref().ok_or_else(|| {
                            GovernanceDagServiceError::Config("IPNS name is missing".to_owned())
                        })?,
                        "IPNS name",
                    )?,
                    key_name: validate_public_token(
                        service.ipns_key_name.as_deref().ok_or_else(|| {
                            GovernanceDagServiceError::Config("IPNS key name is missing".to_owned())
                        })?,
                        "IPNS key name",
                    )?,
                }
            }
            _ => {
                return Err(GovernanceDagServiceError::Config(
                    "head_mode must be signed_http or ipns".to_owned(),
                ));
            }
        };

        // Provider qualification above is deliberately complete before any
        // mutable directory, lock, sealed checkpoint, or publication endpoint
        // is opened.
        let (source_dir, source_root_guard) = secure_existing_directory(&source_dir_path, false)?;
        let (_, state_root_guard) = secure_state_directory(
            &service
                .state_dir
                .clone()
                .unwrap_or_else(|| source_dir.join("governance-dag-service")),
        )?;
        state_root_guard.revalidate().map_err(|err| {
            GovernanceDagServiceError::Filesystem(format!(
                "state root identity changed before lock acquisition: {err}"
            ))
        })?;
        let state_lock = acquire_service_lock(&state_root_guard)?;
        state_root_guard.revalidate().map_err(|err| {
            GovernanceDagServiceError::Filesystem(format!(
                "state root identity changed during lock acquisition: {err}"
            ))
        })?;
        let runtime_config = RuntimeConfig {
            source_dir,
            source_root_guard,
            state_root_guard,
            listen_addr,
            poll_interval: service.poll_interval,
            max_response_bytes: service.max_response_bytes.0,
            max_request_bytes: service.max_request_bytes.0,
            mirror_max_entries: service.mirror_max_entries,
            mirror_max_bytes: service.mirror_max_bytes.0,
            max_head_age_secs: service.max_head_age_secs,
            max_future_skew_secs: service.max_future_skew_secs,
            allow_head_bootstrap: service.allow_head_bootstrap,
            expected_producer_signer_handle,
            expected_producer_signer_qualification,
            expected_publisher_peer_id,
            expected_public_key,
        };
        let (checkpoint, checkpoint_revision) = load_checkpoint(&checkpoint_store)?;
        let (intent, intent_revision) = load_publish_intent(&checkpoint_store)?;
        let ipfs = build_pinned_endpoint(
            &ipfs_url,
            ipfs_authenticator,
            GovernanceDagAuthenticationScope::Ipfs,
            &service,
            true,
        )
        .await?;
        let head_mode = match qualified_head_mode {
            QualifiedHeadMode::SignedHttp { url, authenticator } => HeadMode::SignedHttp(
                build_pinned_endpoint(
                    &url,
                    authenticator,
                    GovernanceDagAuthenticationScope::SignedHead,
                    &service,
                    false,
                )
                .await?,
            ),
            QualifiedHeadMode::Ipns { name, key_name } => HeadMode::Ipns { name, key_name },
        };
        let checkpoint_generation_floor = checkpoint
            .as_ref()
            .map_or(0, |checkpoint| checkpoint.generation);
        let intent_generation_floor = intent.as_ref().map_or(0, |intent| intent.generation);
        let api = ApiState(Arc::new(RwLock::new(ApiSnapshot::default())));
        Ok(Self {
            config: runtime_config,
            checkpoint_store,
            checkpoint_revision,
            checkpoint_generation_floor,
            checkpoint,
            intent_revision,
            intent_generation_floor,
            intent,
            ipfs,
            head_mode,
            api,
            state_lock,
        })
    }
}

fn secure_existing_directory(
    path: &Path,
    secret: bool,
) -> Result<(PathBuf, GovernanceFilesystemRootGuard), GovernanceDagServiceError> {
    let metadata = fs::symlink_metadata(path).map_err(|err| {
        GovernanceDagServiceError::Filesystem(format!("cannot inspect `{}`: {err}", path.display()))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(GovernanceDagServiceError::Filesystem(format!(
            "`{}` must be a real directory",
            path.display()
        )));
    }
    #[cfg(unix)]
    if secret
        && (metadata.uid() != unsafe { geteuid() } || metadata.permissions().mode() & 0o077 != 0)
    {
        return Err(GovernanceDagServiceError::Filesystem(format!(
            "state directory `{}` must be owned by the service user and mode 0700 or stricter",
            path.display()
        )));
    }
    let guard = if secret {
        GovernanceFilesystemRootGuard::capture_writer(path)
    } else {
        GovernanceFilesystemRootGuard::capture_source(path)
    }
    .map_err(|err| {
        GovernanceDagServiceError::Filesystem(format!(
            "cannot fence filesystem root `{}`: {err}",
            path.display()
        ))
    })?;
    let canonical = guard.root().to_path_buf();
    Ok((canonical, guard))
}

fn secure_state_directory(
    path: &Path,
) -> Result<(PathBuf, GovernanceFilesystemRootGuard), GovernanceDagServiceError> {
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                GovernanceDagServiceError::Filesystem(format!(
                    "cannot resolve current directory for state root: {error}"
                ))
            })?
            .join(path)
    };
    let path = absolute_path.as_path();
    if fs::symlink_metadata(path).is_err_and(|error| error.kind() == io::ErrorKind::NotFound) {
        let mut ancestor = path.parent().ok_or_else(|| {
            GovernanceDagServiceError::Filesystem(format!(
                "state directory `{}` has no parent",
                path.display()
            ))
        })?;
        loop {
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => break,
                Ok(_) => {
                    return Err(GovernanceDagServiceError::Filesystem(format!(
                        "state ancestor `{}` must be a real directory",
                        ancestor.display()
                    )));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    ancestor = ancestor.parent().ok_or_else(|| {
                        GovernanceDagServiceError::Filesystem(format!(
                            "state directory `{}` has no existing ancestor",
                            path.display()
                        ))
                    })?;
                }
                Err(error) => {
                    return Err(GovernanceDagServiceError::Filesystem(format!(
                        "cannot inspect state ancestor `{}`: {error}",
                        ancestor.display()
                    )));
                }
            }
        }
        let relative = path.strip_prefix(ancestor).map_err(|_| {
            GovernanceDagServiceError::Filesystem(format!(
                "state directory `{}` escaped its retained ancestor",
                path.display()
            ))
        })?;
        let ancestor_guard =
            GovernanceFilesystemRootGuard::capture_writer(ancestor).map_err(|error| {
                GovernanceDagServiceError::Filesystem(format!(
                    "cannot retain writable state ancestor `{}`: {error}",
                    ancestor.display()
                ))
            })?;
        let mut directory = ancestor_guard.rooted_directory().clone();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(GovernanceDagServiceError::Filesystem(format!(
                    "state directory `{}` contains a non-canonical component",
                    path.display()
                )));
            };
            directory = directory.open_or_create_directory(name).map_err(|error| {
                GovernanceDagServiceError::Filesystem(format!(
                    "cannot create rooted state directory `{}`: {error}",
                    path.display()
                ))
            })?;
        }
        directory.sync_all().map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot durably create state directory `{}`: {error}",
                path.display()
            ))
        })?;
        ancestor_guard.revalidate().map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "state ancestor changed while creating `{}`: {error}",
                path.display()
            ))
        })?;
    }
    secure_existing_directory(path, true)
}

fn read_unrooted_regular_file(
    path: &Path,
    max_bytes: u64,
    secret: bool,
) -> Result<Vec<u8>, GovernanceDagServiceError> {
    let before = fs::symlink_metadata(path).map_err(|err| {
        GovernanceDagServiceError::Filesystem(format!("cannot inspect `{}`: {err}", path.display()))
    })?;
    validate_regular_metadata(path, &before, max_bytes, secret)?;
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let mut file = options.open(path).map_err(|err| {
        GovernanceDagServiceError::Filesystem(format!("cannot open `{}`: {err}", path.display()))
    })?;
    let opened = file.metadata().map_err(|err| {
        GovernanceDagServiceError::Filesystem(format!(
            "cannot inspect open `{}`: {err}",
            path.display()
        ))
    })?;
    validate_regular_metadata(path, &opened, max_bytes, secret)?;
    if !same_file(&before, &opened) {
        return Err(GovernanceDagServiceError::Filesystem(format!(
            "`{}` changed while being opened",
            path.display()
        )));
    }
    let capacity = usize::try_from(opened.len()).map_err(|_| {
        GovernanceDagServiceError::Filesystem(format!(
            "`{}` exceeds host size limits",
            path.display()
        ))
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|err| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot read `{}`: {err}",
                path.display()
            ))
        })?;
    if bytes.len() as u64 > max_bytes || bytes.len() as u64 != opened.len() {
        return Err(GovernanceDagServiceError::Filesystem(format!(
            "`{}` grew, shrank, or exceeds its {} byte limit",
            path.display(),
            max_bytes
        )));
    }
    let after = fs::symlink_metadata(path).map_err(|err| {
        GovernanceDagServiceError::Filesystem(format!(
            "cannot re-inspect `{}`: {err}",
            path.display()
        ))
    })?;
    validate_regular_metadata(path, &after, max_bytes, secret)?;
    if !same_file(&opened, &after) || after.len() != opened.len() {
        return Err(GovernanceDagServiceError::Filesystem(format!(
            "`{}` changed while being read",
            path.display()
        )));
    }
    Ok(bytes)
}

fn rooted_byte_limit(max_bytes: u64, label: &str) -> Result<usize, GovernanceDagServiceError> {
    usize::try_from(max_bytes).map_err(|_| {
        GovernanceDagServiceError::Filesystem(format!(
            "{label} byte limit exceeds host address space"
        ))
    })
}

fn read_rooted_file(
    root_guard: &GovernanceFilesystemRootGuard,
    relative: &Path,
    max_bytes: u64,
    private: bool,
) -> Result<FileSnapshot, GovernanceDagServiceError> {
    root_guard.revalidate().map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "root identity changed before reading `{}`: {error}",
            root_guard.root().join(relative).display()
        ))
    })?;
    let (parent, name) = root_guard
        .rooted_directory()
        .resolve_parent(relative, false)
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot resolve rooted file `{}`: {error}",
                root_guard.root().join(relative).display()
            ))
        })?;
    let max_bytes = rooted_byte_limit(max_bytes, "rooted governance file")?;
    let snapshot = if private {
        parent.read_private_file(&name, max_bytes)
    } else {
        parent.read_file(&name, max_bytes)
    }
    .map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "cannot read rooted file `{}`: {error}",
            root_guard.root().join(relative).display()
        ))
    })?;
    root_guard.revalidate().map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "root identity changed after reading `{}`: {error}",
            root_guard.root().join(relative).display()
        ))
    })?;
    Ok(snapshot)
}

fn verify_rooted_file_binding(
    root_guard: &GovernanceFilesystemRootGuard,
    binding: &FileBinding,
) -> Result<(), GovernanceDagServiceError> {
    root_guard.revalidate().map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "root identity changed before verifying a retained source file: {error}"
        ))
    })?;
    binding.verify().map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "rooted source file was substituted during snapshot loading: {error}"
        ))
    })?;
    root_guard.revalidate().map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "root identity changed after verifying a retained source file: {error}"
        ))
    })
}

fn validate_regular_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    max_bytes: u64,
    secret: bool,
) -> Result<(), GovernanceDagServiceError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(GovernanceDagServiceError::Filesystem(format!(
            "`{}` must be a regular file",
            path.display()
        )));
    }
    if metadata.len() > max_bytes {
        return Err(GovernanceDagServiceError::Filesystem(format!(
            "`{}` exceeds its {} byte limit",
            path.display(),
            max_bytes
        )));
    }
    #[cfg(unix)]
    {
        if metadata.nlink() != 1 {
            return Err(GovernanceDagServiceError::Filesystem(format!(
                "`{}` must have exactly one hard link",
                path.display()
            )));
        }
        if secret
            && (metadata.uid() != unsafe { geteuid() }
                || metadata.permissions().mode() & 0o077 != 0)
        {
            return Err(GovernanceDagServiceError::Filesystem(format!(
                "secret file `{}` must be owned by the service user and mode 0600 or stricter",
                path.display()
            )));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
}

fn acquire_service_lock(
    state_root_guard: &GovernanceFilesystemRootGuard,
) -> Result<RetainedFile, GovernanceDagServiceError> {
    state_root_guard.revalidate().map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "state root changed before service lock acquisition: {error}"
        ))
    })?;
    let file = state_root_guard
        .rooted_directory()
        .open_or_create_private_file(OsStr::new(SERVICE_LOCK_FILE), 4096)
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot open rooted service lock: {error}"
            ))
        })?;
    match file.handle().try_lock() {
        Ok(()) => {
            file.verify().map_err(|error| {
                GovernanceDagServiceError::Filesystem(format!(
                    "service lock binding changed during acquisition: {error}"
                ))
            })?;
            state_root_guard.revalidate().map_err(|error| {
                GovernanceDagServiceError::Filesystem(format!(
                    "state root changed during service lock acquisition: {error}"
                ))
            })?;
            Ok(file)
        }
        Err(fs::TryLockError::WouldBlock) => Err(GovernanceDagServiceError::Filesystem(
            "another Governance DAG service owns the configured state directory".to_owned(),
        )),
        Err(fs::TryLockError::Error(err)) => Err(GovernanceDagServiceError::Filesystem(format!(
            "cannot acquire Governance DAG service lock: {err}"
        ))),
    }
}

fn write_rooted_atomic_secret(
    state_root_guard: &GovernanceFilesystemRootGuard,
    relative: &Path,
    bytes: &[u8],
) -> Result<(), GovernanceDagServiceError> {
    if bytes.len() as u64 > MUTABLE_STATE_MAX_BYTES {
        return Err(GovernanceDagServiceError::Filesystem(format!(
            "durable state `{}` exceeds its byte bound",
            state_root_guard.root().join(relative).display()
        )));
    }
    state_root_guard.revalidate().map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "state root changed before writing `{}`: {error}",
            state_root_guard.root().join(relative).display()
        ))
    })?;
    let (parent, name) = state_root_guard
        .rooted_directory()
        .resolve_parent(relative, false)
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot resolve durable state `{}`: {error}",
                state_root_guard.root().join(relative).display()
            ))
        })?;
    let target_name = name.to_str().ok_or_else(|| {
        GovernanceDagServiceError::Filesystem(
            "durable state target name must be canonical UTF-8".to_owned(),
        )
    })?;
    parent
        .remove_atomic_temps_for(target_name)
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot recover durable temporaries for `{}`: {error}",
                state_root_guard.root().join(relative).display()
            ))
        })?;
    let expected = parent
        .private_file_binding(
            &name,
            rooted_byte_limit(MUTABLE_STATE_MAX_BYTES, "durable state")?,
        )
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot inspect durable predecessor `{}`: {error}",
                state_root_guard.root().join(relative).display()
            ))
        })?
        .map_or(ExpectedFile::Missing, ExpectedFile::Identity);
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temporary_name = OsString::from(format!(".{target_name}.tmp-{}-{counter}", process::id()));
    parent
        .atomic_write(&name, &temporary_name, bytes, expected)
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot install rooted durable state `{}`: {error}",
                state_root_guard.root().join(relative).display()
            ))
        })?;
    let readback = parent
        .read_private_file(
            &name,
            rooted_byte_limit(MUTABLE_STATE_MAX_BYTES, "durable state")?,
        )
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot read back durable state `{}`: {error}",
                state_root_guard.root().join(relative).display()
            ))
        })?;
    if readback.bytes() != bytes {
        return Err(GovernanceDagServiceError::Filesystem(format!(
            "durable state `{}` readback diverged",
            state_root_guard.root().join(relative).display()
        )));
    }
    state_root_guard.revalidate().map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "state root changed after writing `{}`: {error}",
            state_root_guard.root().join(relative).display()
        ))
    })
}

#[cfg(unix)]
fn set_no_follow_flag(options: &mut OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}

#[cfg(not(unix))]
fn set_no_follow_flag(_options: &mut OpenOptions) {}

#[cfg(all(
    target_os = "android",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64",
        target_arch = "x86",
        target_arch = "x86_64"
    ))
))]
compile_error!(
    "Governance DAG service filesystem flags are not qualified for this Android architecture"
);

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("Governance DAG service filesystem flags are not qualified for this Unix target");

#[cfg(all(target_os = "android", target_arch = "riscv64"))]
fn platform_no_follow_flag() -> i32 {
    0x400000
}

#[cfg(all(
    target_os = "android",
    any(target_arch = "aarch64", target_arch = "arm")
))]
fn platform_no_follow_flag() -> i32 {
    0x8000
}

#[cfg(all(
    target_os = "android",
    any(target_arch = "x86", target_arch = "x86_64")
))]
fn platform_no_follow_flag() -> i32 {
    0x20000
}

#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
fn platform_no_follow_flag() -> i32 {
    0x8000
}

#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
fn platform_no_follow_flag() -> i32 {
    0x20000
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
fn platform_no_follow_flag() -> i32 {
    0x100
}

fn decode_fixed_hex<const N: usize>(
    value: &str,
    label: &str,
) -> Result<[u8; N], GovernanceDagServiceError> {
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(GovernanceDagServiceError::Config(format!(
            "{label} must be canonical lowercase {}-byte hex",
            N
        )));
    }
    let bytes = hex::decode(value)
        .map_err(|_| GovernanceDagServiceError::Config(format!("{label} is invalid hex")))?;
    let mut out = [0_u8; N];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn decode_strong_ed25519_public_key_hex(
    value: &str,
    label: &str,
) -> Result<[u8; 32], GovernanceDagServiceError> {
    let bytes = decode_fixed_hex::<32>(value, label)?;
    let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &bytes).map_err(|_| {
        GovernanceDagServiceError::Config(format!(
            "{label} must be a canonical strong Ed25519 point"
        ))
    })?;
    let (algorithm, canonical_bytes) = public_key.try_to_bytes().map_err(|_| {
        GovernanceDagServiceError::Config(format!(
            "{label} must be a canonical strong Ed25519 point"
        ))
    })?;
    if algorithm != Algorithm::Ed25519 || canonical_bytes != bytes.as_slice() {
        return Err(GovernanceDagServiceError::Config(format!(
            "{label} must be a canonical strong Ed25519 point"
        )));
    }
    Ok(bytes)
}

fn validate_public_token(value: &str, label: &str) -> Result<String, GovernanceDagServiceError> {
    if value.is_empty()
        || value.len() > MAX_PUBLIC_TOKEN_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(GovernanceDagServiceError::Config(format!(
            "{label} is not canonical"
        )));
    }
    Ok(value.to_owned())
}

fn validate_runtime_handle(value: &str, label: &str) -> Result<String, GovernanceDagServiceError> {
    match validate_production_runtime_handle(value) {
        Ok(()) => Ok(value.to_owned()),
        Err(ProductionRuntimeHandleError::InvalidSyntax) => Err(GovernanceDagServiceError::Config(
            format!("{label} handle is not a canonical credential-free production runtime handle"),
        )),
        Err(ProductionRuntimeHandleError::TestMarked) => Err(GovernanceDagServiceError::Config(
            format!("{label} handle is test-marked and cannot qualify a production adapter"),
        )),
    }
}

fn current_unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn blake3_array(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

fn durable_decode_limits(max_bytes: u64) -> DecodeLimits {
    let max = usize::try_from(max_bytes).unwrap_or(usize::MAX);
    DecodeLimits::new(150_000, max, 1_000_000, max.saturating_mul(2), 128)
}

fn load_checkpoint(
    store: &OpaqueCheckpointStore,
) -> Result<(Option<CheckpointBodyV1>, Option<[u8; 32]>), GovernanceDagServiceError> {
    let Some(record) = load_sealed_record(store, GovernanceDagSealedStateSlot::Checkpoint)? else {
        return Ok((None, None));
    };
    let body: CheckpointBodyV1 = norito::decode_from_bytes_with_limits(
        &record.payload,
        durable_decode_limits(MUTABLE_STATE_MAX_BYTES),
    )
    .map_err(|err| GovernanceDagServiceError::State(format!("checkpoint decode failed: {err}")))?;
    if norito::to_bytes(&body).map_err(|err| GovernanceDagServiceError::State(err.to_string()))?
        != record.payload
    {
        return Err(GovernanceDagServiceError::State(
            "checkpoint encoding is not canonical".to_owned(),
        ));
    }
    if body.version != CHECKPOINT_VERSION_V1 {
        return Err(GovernanceDagServiceError::State(
            "checkpoint version is unsupported".to_owned(),
        ));
    }
    if body.generation != record.generation {
        return Err(GovernanceDagServiceError::State(
            "checkpoint generation does not match its sealed monotonic record".to_owned(),
        ));
    }
    validate_checkpoint_body(&body)?;
    Ok((Some(body), Some(record.revision)))
}

fn save_checkpoint(
    store: &OpaqueCheckpointStore,
    expected_revision: Option<[u8; 32]>,
    body: &CheckpointBodyV1,
) -> Result<[u8; 32], GovernanceDagServiceError> {
    validate_checkpoint_body(body)?;
    let bytes = norito::to_bytes(body).map_err(|err| {
        GovernanceDagServiceError::State(format!("checkpoint encode failed: {err}"))
    })?;
    save_sealed_record(
        store,
        GovernanceDagSealedStateSlot::Checkpoint,
        expected_revision,
        body.generation,
        bytes,
        "checkpoint",
    )
}

fn validate_checkpoint_body(body: &CheckpointBodyV1) -> Result<(), GovernanceDagServiceError> {
    if body.version != CHECKPOINT_VERSION_V1
        || body.generation == 0
        || body.block_count == 0
        || body.head_block_cid.len() != 32
        || !is_canonical_cid_v1(&body.head_ipfs_cid)
        || body.public_head_token.is_empty()
        || body.public_head_token.len() > MAX_PUBLIC_TOKEN_BYTES
        || body.mirror_blocks.is_empty()
    {
        return Err(GovernanceDagServiceError::State(
            "checkpoint fields violate first-release bounds".to_owned(),
        ));
    }
    let mut previous = None;
    let mut seen = BTreeSet::new();
    for block in &body.mirror_blocks {
        validate_published_block(block)?;
        if previous.is_some_and(|value| block.sequence != value + 1)
            || !seen.insert(block.governance_block_cid.clone())
        {
            return Err(GovernanceDagServiceError::State(
                "checkpoint mirror block order is invalid".to_owned(),
            ));
        }
        previous = Some(block.sequence);
    }
    if body
        .mirror_blocks
        .last()
        .is_none_or(|block| block.governance_block_cid != body.head_block_cid)
    {
        return Err(GovernanceDagServiceError::State(
            "checkpoint mirror does not end at the public head".to_owned(),
        ));
    }
    Ok(())
}

fn validate_published_block(block: &PublishedBlockV1) -> Result<(), GovernanceDagServiceError> {
    if block.governance_block_cid.len() != 32
        || block.governance_node_cid.len() != 32
        || block.payload_kind.is_empty()
        || block.encoded_len == 0
        || !is_canonical_cid_v1(&block.ipfs_cid)
    {
        return Err(GovernanceDagServiceError::State(
            "published block fields violate first-release bounds".to_owned(),
        ));
    }
    Ok(())
}

fn load_publish_intent(
    store: &OpaqueCheckpointStore,
) -> Result<(Option<PublishIntentBodyV1>, Option<[u8; 32]>), GovernanceDagServiceError> {
    let Some(record) = load_sealed_record(store, GovernanceDagSealedStateSlot::PublishIntent)?
    else {
        return Ok((None, None));
    };
    let body: PublishIntentBodyV1 = norito::decode_from_bytes_with_limits(
        &record.payload,
        durable_decode_limits(MUTABLE_STATE_MAX_BYTES),
    )
    .map_err(|err| {
        GovernanceDagServiceError::State(format!("publish intent decode failed: {err}"))
    })?;
    if norito::to_bytes(&body).map_err(|err| GovernanceDagServiceError::State(err.to_string()))?
        != record.payload
    {
        return Err(GovernanceDagServiceError::State(
            "publish intent encoding is not canonical".to_owned(),
        ));
    }
    if body.version != PUBLISH_INTENT_VERSION_V1 {
        return Err(GovernanceDagServiceError::State(
            "publish intent version is unsupported".to_owned(),
        ));
    }
    if body.generation != record.generation {
        return Err(GovernanceDagServiceError::State(
            "publish intent generation does not match its sealed monotonic record".to_owned(),
        ));
    }
    validate_publish_intent(&body)?;
    Ok((Some(body), Some(record.revision)))
}

fn save_publish_intent(
    store: &OpaqueCheckpointStore,
    expected_revision: Option<[u8; 32]>,
    body: &PublishIntentBodyV1,
) -> Result<[u8; 32], GovernanceDagServiceError> {
    validate_publish_intent(body)?;
    let bytes = norito::to_bytes(body).map_err(|err| {
        GovernanceDagServiceError::State(format!("publish intent encode failed: {err}"))
    })?;
    save_sealed_record(
        store,
        GovernanceDagSealedStateSlot::PublishIntent,
        expected_revision,
        body.generation,
        bytes,
        "publish intent",
    )
}

fn load_sealed_record(
    store: &OpaqueCheckpointStore,
    slot: GovernanceDagSealedStateSlot,
) -> Result<Option<GovernanceDagSealedStateRecord>, GovernanceDagServiceError> {
    store.assert_identity()?;
    let record = store.provider.load(slot);
    store.assert_identity()?;
    let record = record.map_err(|_| {
        GovernanceDagServiceError::State("sealed checkpoint store read failed".to_owned())
    })?;
    let Some(record) = record else {
        return Ok(None);
    };
    if record.generation == 0
        || record.payload.is_empty()
        || record.payload.len() as u64 > MUTABLE_STATE_MAX_BYTES
        || !record.has_valid_revision(slot)
    {
        return Err(GovernanceDagServiceError::State(
            "sealed checkpoint store returned an invalid record".to_owned(),
        ));
    }
    Ok(Some(record))
}

fn load_producer_commit_guard(
    config: &RuntimeConfig,
    store: &OpaqueCheckpointStore,
) -> Result<ProducerCommitGuard, GovernanceDagServiceError> {
    if load_sealed_record(store, GovernanceDagSealedStateSlot::ProducerPublishIntent)?.is_some() {
        return Err(GovernanceDagServiceError::State(
            "local Governance DAG producer has an active sealed publish intent".to_owned(),
        ));
    }
    let record = load_sealed_record(store, GovernanceDagSealedStateSlot::ProducerCheckpoint)?
        .ok_or_else(|| {
            GovernanceDagServiceError::State(
                "local Governance DAG producer checkpoint is missing".to_owned(),
            )
        })?;
    if load_sealed_record(store, GovernanceDagSealedStateSlot::ProducerPublishIntent)?.is_some() {
        return Err(GovernanceDagServiceError::State(
            "local Governance DAG producer began a transaction during checkpoint read".to_owned(),
        ));
    }
    let checkpoint: RuntimeDagProducerCheckpointV1 = norito::decode_from_bytes_with_limits(
        &record.payload,
        durable_decode_limits(MUTABLE_STATE_MAX_BYTES),
    )
    .map_err(|err| {
        GovernanceDagServiceError::State(format!(
            "local Governance DAG producer checkpoint decode failed: {err}"
        ))
    })?;
    if norito::to_bytes(&checkpoint)
        .map_err(|err| GovernanceDagServiceError::State(err.to_string()))?
        != record.payload
    {
        return Err(GovernanceDagServiceError::State(
            "local Governance DAG producer checkpoint encoding is not canonical".to_owned(),
        ));
    }
    let expected_generation = checkpoint
        .block_count
        .checked_add(checkpoint.qualification_transition_generation)
        .and_then(|generation| generation.checked_add(checkpoint.qualification_archive_generation))
        .and_then(|generation| generation.checked_add(1))
        .ok_or_else(|| {
            GovernanceDagServiceError::State(
                "local Governance DAG producer checkpoint generation exhausted".to_owned(),
            )
        })?;
    let signer_qualification = GovernanceDagRuntimeProviderQualificationV1::new(
        checkpoint.signer_revision,
        checkpoint.signer_policy_digest,
    );
    let store_qualification = GovernanceDagRuntimeProviderQualificationV1::new(
        checkpoint.checkpoint_store_revision,
        checkpoint.checkpoint_store_policy_digest,
    );
    if checkpoint.version != GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_VERSION_V1
        || record.generation != expected_generation
        || validate_runtime_handle(
            &checkpoint.signer_handle,
            "local Governance DAG producer signer",
        )
        .is_err()
        || !signer_qualification.is_valid()
        || checkpoint.signer_handle != config.expected_producer_signer_handle
        || signer_qualification != config.expected_producer_signer_qualification
        || checkpoint.publisher_peer_id.is_empty()
        || checkpoint.publisher_peer_id.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1
        || checkpoint.publisher_peer_id != config.expected_publisher_peer_id
        || checkpoint.publisher_public_key != config.expected_public_key
        || checkpoint.checkpoint_store_handle != store.handle
        || !store_qualification.is_valid()
        || store_qualification != store.qualification
    {
        return Err(GovernanceDagServiceError::State(
            "local Governance DAG producer checkpoint identity or generation is invalid".to_owned(),
        ));
    }
    Ok(ProducerCommitGuard { record, checkpoint })
}

fn validate_source_against_producer_guard(
    config: &RuntimeConfig,
    source: &SourceSnapshot,
    guard: &ProducerCommitGuard,
) -> Result<(), GovernanceDagServiceError> {
    let checkpoint = &guard.checkpoint;
    let root_digest = runtime_dag_producer_root_digest(&config.source_dir).map_err(|err| {
        GovernanceDagServiceError::State(format!(
            "local Governance DAG producer root binding failed: {err}"
        ))
    })?;
    let source_block_count = u64::try_from(source.blocks.len()).map_err(|_| {
        GovernanceDagServiceError::State("source block count exceeds u64".to_owned())
    })?;
    if checkpoint.root_digest != root_digest
        || checkpoint.publisher_public_key != config.expected_public_key
        || checkpoint.block_count != source_block_count
        || checkpoint.block_count != source.head.block_count
        || checkpoint.head_block_cid.as_slice() != source.head.head_block_cid.as_slice()
        || checkpoint.head_bytes_digest != blake3_array(&source.head_bytes)
        || checkpoint.index_bytes_digest != source.index_blake3
    {
        return Err(GovernanceDagServiceError::Conflict(
            "verified source snapshot does not match the sealed local producer checkpoint"
                .to_owned(),
        ));
    }
    config.revalidate_source_root()?;
    validate_runtime_dag_snapshot_authority_lineage(
        &config.source_dir,
        checkpoint,
        source.blocks.iter().map(|block| &block.block),
        &source.head,
    )
    .map_err(|error| {
        GovernanceDagServiceError::Conflict(format!(
            "verified source snapshot has an invalid authority-segment lineage: {error}"
        ))
    })?;
    config.revalidate_source_root()?;
    Ok(())
}

fn load_committed_source_snapshot(
    config: &RuntimeConfig,
    store: &OpaqueCheckpointStore,
) -> Result<SourceSnapshot, GovernanceDagServiceError> {
    config.revalidate_source_root()?;
    let first_guard = load_producer_commit_guard(config, store)?;
    let source = load_source_snapshot(config)?;
    config.revalidate_source_root()?;
    let second_guard = load_producer_commit_guard(config, store)?;
    if second_guard != first_guard {
        return Err(GovernanceDagServiceError::Conflict(
            "local Governance DAG producer checkpoint changed while reading source".to_owned(),
        ));
    }
    validate_source_against_producer_guard(config, &source, &second_guard)?;
    config.revalidate_source_root()?;
    Ok(source)
}

fn save_sealed_record(
    store: &OpaqueCheckpointStore,
    slot: GovernanceDagSealedStateSlot,
    expected_revision: Option<[u8; 32]>,
    generation: u64,
    payload: Vec<u8>,
    label: &'static str,
) -> Result<[u8; 32], GovernanceDagServiceError> {
    if payload.is_empty() || payload.len() as u64 > MUTABLE_STATE_MAX_BYTES {
        return Err(GovernanceDagServiceError::State(format!(
            "{label} exceeds the sealed-state byte bound"
        )));
    }
    store.assert_identity()?;
    let next = GovernanceDagSealedStateRecord::new(slot, generation, payload);
    let revision = next.revision;
    let result = store
        .provider
        .compare_and_swap(slot, expected_revision, next.clone());
    store.assert_identity()?;
    result.map_err(|_| {
        GovernanceDagServiceError::State(format!(
            "sealed checkpoint store compare-and-swap failed for {label}"
        ))
    })?;
    let observed = load_sealed_record(store, slot)?.ok_or_else(|| {
        GovernanceDagServiceError::State(format!(
            "sealed checkpoint store lost {label} after compare-and-swap"
        ))
    })?;
    if observed != next {
        return Err(GovernanceDagServiceError::State(format!(
            "sealed checkpoint store readback diverged for {label}"
        )));
    }
    Ok(revision)
}

fn delete_publish_intent(
    store: &OpaqueCheckpointStore,
    expected_revision: Option<[u8; 32]>,
) -> Result<(), GovernanceDagServiceError> {
    let revision = expected_revision.ok_or_else(|| {
        GovernanceDagServiceError::State(
            "cannot delete publish intent without its exact sealed revision".to_owned(),
        )
    })?;
    store.assert_identity()?;
    let result = store
        .provider
        .delete(GovernanceDagSealedStateSlot::PublishIntent, revision);
    store.assert_identity()?;
    result.map_err(|_| {
        GovernanceDagServiceError::State(
            "sealed checkpoint store publish-intent delete failed".to_owned(),
        )
    })?;
    if load_sealed_record(store, GovernanceDagSealedStateSlot::PublishIntent)?.is_some() {
        return Err(GovernanceDagServiceError::State(
            "sealed checkpoint store retained publish intent after delete".to_owned(),
        ));
    }
    Ok(())
}

fn validate_publish_intent(body: &PublishIntentBodyV1) -> Result<(), GovernanceDagServiceError> {
    if body.version != PUBLISH_INTENT_VERSION_V1
        || body.generation == 0
        || body.target_block_count == 0
        || body.target_head_block_cid.len() != 32
        || body.target_head_bytes.is_empty()
        || body.target_head_bytes.len() as u64 > MUTABLE_STATE_MAX_BYTES
        || body.blocks.is_empty()
    {
        return Err(GovernanceDagServiceError::State(
            "publish intent fields violate first-release bounds".to_owned(),
        ));
    }
    let mut previous = None;
    let mut seen = BTreeSet::new();
    for block in &body.blocks {
        if block.governance_block_cid.len() != 32
            || block.governance_node_cid.len() != 32
            || block.payload_kind.is_empty()
            || block.encoded_len == 0
            || block
                .ipfs_cid
                .as_ref()
                .is_some_and(|cid| !is_canonical_cid_v1(cid))
        {
            return Err(GovernanceDagServiceError::State(
                "publish intent block fields are invalid".to_owned(),
            ));
        }
        if previous.is_some_and(|value| block.sequence != value + 1)
            || !seen.insert(block.governance_block_cid.clone())
        {
            return Err(GovernanceDagServiceError::State(
                "publish intent block order is invalid".to_owned(),
            ));
        }
        previous = Some(block.sequence);
    }
    if body
        .head_ipfs_cid
        .as_ref()
        .is_some_and(|cid| !is_canonical_cid_v1(cid))
    {
        return Err(GovernanceDagServiceError::State(
            "publish intent head CID is not canonical CIDv1 base32".to_owned(),
        ));
    }
    Ok(())
}

fn resolve_index_relative_path(raw: &str) -> Result<PathBuf, GovernanceDagServiceError> {
    if raw.is_empty() || raw.contains('\\') {
        return Err(GovernanceDagServiceError::Source(
            "runtime index path is empty or contains a backslash".to_owned(),
        ));
    }
    let relative = Path::new(raw);
    if relative.is_absolute() {
        return Err(GovernanceDagServiceError::Source(
            "runtime index path must be relative".to_owned(),
        ));
    }
    let mut path = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => path.push(value),
            _ => {
                return Err(GovernanceDagServiceError::Source(
                    "runtime index path contains traversal or platform prefixes".to_owned(),
                ));
            }
        }
    }
    Ok(path)
}

fn digest_sidecar_path(path: &Path) -> PathBuf {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map_or_else(|| "blake3".to_owned(), |value| format!("{value}.blake3"));
    path.with_extension(extension)
}

fn read_verified_sidecar_file(
    root_guard: &GovernanceFilesystemRootGuard,
    relative: &Path,
    max_bytes: u64,
    retained_bindings: Option<&mut Vec<FileBinding>>,
) -> Result<Vec<u8>, GovernanceDagServiceError> {
    let file = read_rooted_file(root_guard, relative, max_bytes, false)?;
    let sidecar_path = digest_sidecar_path(relative);
    let sidecar = read_rooted_file(root_guard, &sidecar_path, 65, false)?;
    let expected = format!("{}\n", hex::encode(blake3_array(file.bytes())));
    if sidecar.bytes() != expected.as_bytes() {
        return Err(GovernanceDagServiceError::Source(format!(
            "digest sidecar does not match `{}`",
            root_guard.root().join(relative).display()
        )));
    }
    if let Some(bindings) = retained_bindings {
        bindings.push(file.binding());
        bindings.push(sidecar.binding());
    }
    Ok(file.into_bytes())
}

fn decode_canonical<T>(bytes: &[u8], label: &str) -> Result<T, GovernanceDagServiceError>
where
    for<'de> T: norito::NoritoDeserialize<'de>,
    T: norito::NoritoSerialize,
{
    let max = bytes.len().max(1);
    let value = norito::decode_from_bytes_with_limits(
        bytes,
        DecodeLimits::new(
            MAX_REPUTATION_TRUST_EDGES,
            max,
            CANONICAL_DECODE_MAX_TOTAL_ELEMENTS,
            max.saturating_mul(CANONICAL_DECODE_ALLOCATION_MULTIPLIER),
            128,
        ),
    )
    .map_err(|err| GovernanceDagServiceError::Source(format!("{label} decode failed: {err}")))?;
    let canonical = norito::to_bytes(&value).map_err(|err| {
        GovernanceDagServiceError::Source(format!("{label} encode failed: {err}"))
    })?;
    if canonical != bytes {
        return Err(GovernanceDagServiceError::Source(format!(
            "{label} is not canonical Norito"
        )));
    }
    Ok(value)
}

fn required_json_string(map: &JsonMap, field: &str) -> Result<String, GovernanceDagServiceError> {
    map.get(field)
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            GovernanceDagServiceError::Source(format!("runtime index is missing `{field}`"))
        })
}

fn required_json_u64(map: &JsonMap, field: &str) -> Result<u64, GovernanceDagServiceError> {
    map.get(field).and_then(JsonValue::as_u64).ok_or_else(|| {
        GovernanceDagServiceError::Source(format!("runtime index is missing `{field}`"))
    })
}

fn optional_json_string(
    map: &JsonMap,
    field: &str,
) -> Result<Option<String>, GovernanceDagServiceError> {
    match map.get(field) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| {
                GovernanceDagServiceError::Source(format!(
                    "runtime index `{field}` is not a string"
                ))
            }),
    }
}

fn canonical_hex_vec(
    value: &str,
    expected_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, GovernanceDagServiceError> {
    if value.len() != expected_bytes * 2
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(GovernanceDagServiceError::Source(format!(
            "{label} must be canonical lowercase {expected_bytes}-byte hex"
        )));
    }
    hex::decode(value)
        .map_err(|_| GovernanceDagServiceError::Source(format!("{label} is invalid hex")))
}

fn canonical_source_payload_bytes(
    payload: &GovernanceLogPayloadV1,
) -> Result<Vec<u8>, GovernanceDagServiceError> {
    fn encode_bounded<T: norito::NoritoSerialize>(
        value: &T,
    ) -> Result<Vec<u8>, GovernanceDagServiceError> {
        let exact = value.encoded_len_exact().ok_or_else(|| {
            GovernanceDagServiceError::Source(
                "canonical governance source payload has no allocation-free exact size".to_owned(),
            )
        })?;
        if exact > GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1 {
            return Err(GovernanceDagServiceError::Source(format!(
                "canonical governance source payload exceeds the V1 ceiling of {GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1} bytes"
            )));
        }
        let bytes = norito::to_bytes(value).map_err(|err| {
            GovernanceDagServiceError::Source(format!(
                "failed to encode canonical governance source payload: {err}"
            ))
        })?;
        if bytes.len() > GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1 {
            return Err(GovernanceDagServiceError::Source(format!(
                "canonical governance source payload exceeds the V1 ceiling of {GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1} bytes"
            )));
        }
        Ok(bytes)
    }

    macro_rules! encode {
        ($value:expr) => {
            encode_bounded($value)
        };
    }

    match payload {
        GovernanceLogPayloadV1::ProviderAdvert(value) => encode!(value),
        GovernanceLogPayloadV1::ReplicationOrder(value) => encode!(value),
        GovernanceLogPayloadV1::PorProof(value) => encode!(value),
        GovernanceLogPayloadV1::PdpArchive(value) => encode!(value),
        GovernanceLogPayloadV1::AuditVerdict(value) => encode!(value),
        GovernanceLogPayloadV1::DealSettlement(value) => encode!(value.as_ref()),
        GovernanceLogPayloadV1::SignedReputationSnapshot(value) => encode!(value),
        GovernanceLogPayloadV1::ModerationBallotEvent(value) => encode!(value),
        GovernanceLogPayloadV1::AppealFinanceReport(value) => encode!(value),
        GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(value) => encode!(value),
        GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(value) => encode!(value),
        GovernanceLogPayloadV1::OrderbookSettlementReceipt(value) => encode!(value),
        GovernanceLogPayloadV1::ExternalPayload(value) => {
            if value.encoded_payload.is_empty()
                || value.encoded_payload.len()
                    > GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1
            {
                return Err(GovernanceDagServiceError::Source(format!(
                    "canonical governance source payload exceeds the V1 ceiling of {GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1} bytes"
                )));
            }
            Ok(value.encoded_payload.clone())
        }
        GovernanceLogPayloadV1::PorChallengePublication(value) => encode!(value),
        GovernanceLogPayloadV1::PorWeeklyReport(value) => encode!(value),
    }
}

#[cfg(test)]
fn validate_expected_signer(
    block: &GovernanceDagBlockV1,
    expected_public_key: &[u8; 32],
    expected_peer_id: &[u8],
) -> Result<(), GovernanceDagServiceError> {
    if block.block_signature.algorithm != GovernanceSignatureAlgorithm::Ed25519
        || block.node.publisher_signature.algorithm != GovernanceSignatureAlgorithm::Ed25519
        || block.block_signature.public_key.as_slice() != expected_public_key
        || block.node.publisher_signature.public_key.as_slice() != expected_public_key
    {
        return Err(GovernanceDagServiceError::Source(
            "runtime DAG block or node is signed by an unexpected key".to_owned(),
        ));
    }
    if block.publisher_peer_id != expected_peer_id
        || block.node.publisher_peer_id != expected_peer_id
    {
        return Err(GovernanceDagServiceError::Source(
            "runtime DAG block or node uses an unexpected publisher peer id".to_owned(),
        ));
    }
    Ok(())
}

fn load_source_snapshot(
    config: &RuntimeConfig,
) -> Result<SourceSnapshot, GovernanceDagServiceError> {
    config.revalidate_source_root()?;
    let mut observed_bindings = Vec::<FileBinding>::new();
    let index_path = Path::new("runtime-dag-index.json");
    let index_bytes = read_verified_sidecar_file(
        &config.source_root_guard,
        index_path,
        RUNTIME_INDEX_MAX_BYTES,
        Some(&mut observed_bindings),
    )?;
    let index_blake3 = blake3_array(&index_bytes);
    let index: JsonValue = json::from_slice(&index_bytes).map_err(|err| {
        GovernanceDagServiceError::Source(format!("runtime index JSON is invalid: {err}"))
    })?;
    let map = index.as_object().ok_or_else(|| {
        GovernanceDagServiceError::Source("runtime index root is not an object".to_owned())
    })?;
    if map.get("schema").and_then(JsonValue::as_str) != Some(RUNTIME_INDEX_SCHEMA) {
        return Err(GovernanceDagServiceError::Source(
            "runtime index schema is unsupported".to_owned(),
        ));
    }
    let key_hex = required_json_string(map, "publisher_public_key_hex")?;
    if decode_fixed_hex::<32>(&key_hex, "runtime index publisher key")?
        != config.expected_public_key
    {
        return Err(GovernanceDagServiceError::Source(
            "runtime index publisher key does not match configuration".to_owned(),
        ));
    }
    let peer_hex = required_json_string(map, "publisher_peer_id_hex")?;
    if peer_hex.is_empty()
        || peer_hex.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 * 2
        || peer_hex.len() % 2 != 0
    {
        return Err(GovernanceDagServiceError::Source(
            "runtime index publisher peer id is invalid".to_owned(),
        ));
    }
    let peer_id = canonical_hex_vec(&peer_hex, peer_hex.len() / 2, "publisher peer id")?;
    if peer_id != config.expected_publisher_peer_id {
        return Err(GovernanceDagServiceError::Source(
            "runtime index publisher peer id does not match configuration".to_owned(),
        ));
    }
    let block_values = map
        .get("blocks")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| {
            GovernanceDagServiceError::Source("runtime index blocks are missing".to_owned())
        })?;
    if block_values.is_empty() || block_values.len() > SOURCE_ENTRY_HARD_CAP {
        return Err(GovernanceDagServiceError::Source(format!(
            "runtime index block count must be within 1..={SOURCE_ENTRY_HARD_CAP}"
        )));
    }
    let advertised_count = required_json_u64(map, "block_count")?;
    let available_block_count = u64::try_from(block_values.len()).map_err(|_| {
        GovernanceDagServiceError::Source("runtime index block count exceeds u64".to_owned())
    })?;
    if advertised_count != available_block_count {
        return Err(GovernanceDagServiceError::Source(
            "runtime index block_count does not match its blocks array".to_owned(),
        ));
    }

    let now = current_unix_timestamp_seconds();
    let latest_allowed = now.saturating_add(config.max_future_skew_secs);
    let mut blocks = Vec::with_capacity(block_values.len());
    let mut decoded_blocks = Vec::with_capacity(block_values.len());
    let mut expected_by_digest = BTreeMap::<String, Vec<JsonValue>>::new();
    let mut expected_by_source_payload_digest = BTreeMap::<String, Vec<JsonValue>>::new();
    let mut expected_by_kind = BTreeMap::<String, Vec<JsonValue>>::new();
    let mut total_bytes = 0_u64;
    let mut previous_node_cid: Option<Vec<u8>> = None;
    for (position, value) in block_values.iter().enumerate() {
        let position_u64 = u64::try_from(position).map_err(|_| {
            GovernanceDagServiceError::Source("runtime index position exceeds u64".to_owned())
        })?;
        let entry = value.as_object().ok_or_else(|| {
            GovernanceDagServiceError::Source(format!(
                "runtime index block {position} is not an object"
            ))
        })?;
        if required_json_u64(entry, "position")? != position_u64
            || required_json_u64(entry, "sequence")? != position_u64
        {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} position or sequence is invalid"
            )));
        }
        let block_path = required_json_string(entry, "block_path")?;
        let path = resolve_index_relative_path(&block_path)?;
        let bytes = read_verified_sidecar_file(
            &config.source_root_guard,
            &path,
            u64::try_from(GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1).map_err(|_| {
                GovernanceDagServiceError::Source(
                    "canonical Governance DAG block ceiling exceeds host limits".to_owned(),
                )
            })?,
            None,
        )?;
        let block_encoded_len = u64::try_from(bytes.len()).map_err(|_| {
            GovernanceDagServiceError::Source(format!(
                "runtime index block {position} length exceeds u64"
            ))
        })?;
        if required_json_u64(entry, "encoded_len")? != block_encoded_len {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} encoded_len is invalid"
            )));
        }
        total_bytes = total_bytes.checked_add(block_encoded_len).ok_or_else(|| {
            GovernanceDagServiceError::Source("source byte count overflow".to_owned())
        })?;
        if total_bytes > SOURCE_TOTAL_BYTES_HARD_CAP {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime DAG exceeds the {SOURCE_TOTAL_BYTES_HARD_CAP} byte hard cap"
            )));
        }
        let block: GovernanceDagBlockV1 = decode_canonical(&bytes, "governance DAG block")?;
        block.validate().map_err(|err| {
            GovernanceDagServiceError::Source(format!("block {position} is invalid: {err}"))
        })?;
        if block.sequence != position_u64 || block.timestamp > latest_allowed {
            return Err(GovernanceDagServiceError::Source(format!(
                "block {position} sequence or timestamp is invalid"
            )));
        }
        if block.node.prev_cid != previous_node_cid {
            return Err(GovernanceDagServiceError::Source(format!(
                "block {position} node parent link is invalid"
            )));
        }
        previous_node_cid = Some(block.node.node_cid.clone());
        let block_cid_hex = required_json_string(entry, "block_cid_hex")?;
        let node_cid_hex = required_json_string(entry, "node_cid_hex")?;
        if canonical_hex_vec(&block_cid_hex, 32, "block CID")? != block.block_cid
            || canonical_hex_vec(&node_cid_hex, 32, "node CID")? != block.node.node_cid
        {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} CID does not match canonical bytes"
            )));
        }
        let expected_block_path = format!(
            "runtime-dag/blocks/{:020}_{}.to",
            block.sequence, block_cid_hex
        );
        if block_path != expected_block_path {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} path does not bind its sequence and CID"
            )));
        }
        let expected_prev_block = optional_json_string(entry, "prev_block_cid_hex")?
            .map(|value| canonical_hex_vec(&value, 32, "previous block CID"))
            .transpose()?;
        let expected_prev_node = optional_json_string(entry, "prev_node_cid_hex")?
            .map(|value| canonical_hex_vec(&value, 32, "previous node CID"))
            .transpose()?;
        if expected_prev_block != block.prev_block_cid || expected_prev_node != block.node.prev_cid
        {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} parent metadata is invalid"
            )));
        }
        let kind = crate::governance::runtime_dag_payload_kind(&block.node.payload).to_owned();
        if !crate::governance::runtime_dag_payload_kind_is_supported(&kind) {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} uses unsupported payload kind `{kind}`"
            )));
        }
        if required_json_string(entry, "payload_kind")? != kind {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} payload kind is invalid"
            )));
        }
        let digest = blake3_array(&bytes);
        if required_json_string(entry, "encoded_blake3")? != hex::encode(digest) {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} digest is invalid"
            )));
        }
        expected_by_digest
            .entry(hex::encode(digest))
            .or_default()
            .push(JsonValue::from(position_u64));

        let source_payload_path = required_json_string(entry, "encoded_path")?;
        let source_payload_path = resolve_index_relative_path(&source_payload_path)?;
        let source_payload_bytes = read_verified_sidecar_file(
            &config.source_root_guard,
            &source_payload_path,
            u64::try_from(GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1).map_err(|_| {
                GovernanceDagServiceError::Source(
                    "canonical Governance DAG source-payload ceiling exceeds host limits"
                        .to_owned(),
                )
            })?,
            None,
        )?;
        let source_payload_len = u64::try_from(source_payload_bytes.len()).map_err(|_| {
            GovernanceDagServiceError::Source(format!(
                "runtime index block {position} source payload length exceeds u64"
            ))
        })?;
        total_bytes = total_bytes.checked_add(source_payload_len).ok_or_else(|| {
            GovernanceDagServiceError::Source("source byte count overflow".to_owned())
        })?;
        if total_bytes > SOURCE_TOTAL_BYTES_HARD_CAP {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime DAG exceeds the {SOURCE_TOTAL_BYTES_HARD_CAP} byte hard cap"
            )));
        }
        if required_json_u64(entry, "source_payload_len")? != source_payload_len {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} source_payload_len is invalid"
            )));
        }
        let source_payload_digest = blake3_array(&source_payload_bytes);
        if required_json_string(entry, "source_payload_blake3")?
            != hex::encode(source_payload_digest)
        {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} source payload digest is invalid"
            )));
        }
        if canonical_source_payload_bytes(&block.node.payload)? != source_payload_bytes {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} source payload does not match its signed governance node"
            )));
        }
        expected_by_source_payload_digest
            .entry(hex::encode(source_payload_digest))
            .or_default()
            .push(JsonValue::from(position_u64));
        expected_by_kind
            .entry(kind.clone())
            .or_default()
            .push(JsonValue::from(position_u64));
        decoded_blocks.push(block.clone());
        blocks.push(SourceBlock {
            block,
            bytes,
            encoded_blake3: digest,
            payload_kind: kind,
        });
    }

    let expected_by_digest = expected_by_digest
        .into_iter()
        .map(|(digest, positions)| (digest, JsonValue::Array(positions)))
        .collect::<JsonMap>();
    let expected_by_source_payload_digest = expected_by_source_payload_digest
        .into_iter()
        .map(|(digest, positions)| (digest, JsonValue::Array(positions)))
        .collect::<JsonMap>();
    let expected_by_kind = expected_by_kind
        .into_iter()
        .map(|(kind, positions)| (kind, JsonValue::Array(positions)))
        .collect::<JsonMap>();
    if map.get("by_encoded_blake3") != Some(&JsonValue::Object(expected_by_digest))
        || map.get("by_source_payload_blake3")
            != Some(&JsonValue::Object(expected_by_source_payload_digest))
        || map.get("by_payload_kind") != Some(&JsonValue::Object(expected_by_kind))
    {
        return Err(GovernanceDagServiceError::Source(
            "runtime index lookup maps are non-canonical or inconsistent".to_owned(),
        ));
    }

    let head_path_label = required_json_string(map, "head_path")?;
    if head_path_label != "runtime-dag/head.to" {
        return Err(GovernanceDagServiceError::Source(
            "runtime index head_path is not canonical".to_owned(),
        ));
    }
    let head_path = resolve_index_relative_path(&head_path_label)?;
    let head_bytes = read_verified_sidecar_file(
        &config.source_root_guard,
        &head_path,
        MUTABLE_STATE_MAX_BYTES,
        None,
    )?;
    let head: GovernanceDagHeadV1 = decode_canonical(&head_bytes, "governance DAG head")?;
    validate_source_head_chain(&head, &decoded_blocks)?;
    if head.generated_at > latest_allowed
        || blocks
            .last()
            .is_some_and(|block| head.generated_at < block.block.timestamp)
        || now.saturating_sub(head.generated_at) > config.max_head_age_secs
    {
        return Err(GovernanceDagServiceError::Source(
            "signed head is stale, future-dated, or predates its tip".to_owned(),
        ));
    }
    if required_json_string(map, "head_block_cid_hex")? != hex::encode(&head.head_block_cid)
        || required_json_u64(map, "head_generated_at")? != head.generated_at
    {
        return Err(GovernanceDagServiceError::Source(
            "runtime index head metadata does not match signed head bytes".to_owned(),
        ));
    }
    let stable_index = read_verified_sidecar_file(
        &config.source_root_guard,
        index_path,
        RUNTIME_INDEX_MAX_BYTES,
        Some(&mut observed_bindings),
    )?;
    if stable_index != index_bytes {
        return Err(GovernanceDagServiceError::Source(
            "runtime index changed while the source snapshot was being read".to_owned(),
        ));
    }
    let source = SourceSnapshot {
        index_blake3,
        head,
        head_bytes,
        blocks,
    };
    for binding in &observed_bindings {
        verify_rooted_file_binding(&config.source_root_guard, binding)?;
    }
    config.revalidate_source_root()?;
    Ok(source)
}

fn validate_source_head_chain(
    head: &GovernanceDagHeadV1,
    blocks: &[GovernanceDagBlockV1],
) -> Result<(), GovernanceDagServiceError> {
    validate_governance_dag_head_against_rotatable_chain_v1(head, blocks).map_err(|err| {
        GovernanceDagServiceError::Source(format!("signed head chain is invalid: {err}"))
    })?;
    if blocks.len() > GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 {
        let tail_start = blocks.len() - GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1;
        validate_governance_dag_head_against_rotatable_chain_v1(head, &blocks[tail_start..])
            .map_err(|err| {
                GovernanceDagServiceError::Source(format!(
                    "signed head checkpoint window is invalid: {err}"
                ))
            })?;
    }
    Ok(())
}

async fn build_pinned_endpoint(
    raw: &str,
    authenticator: OpaqueAuthenticator,
    authentication_scope: GovernanceDagAuthenticationScope,
    config: &SorafsGovernanceDagService,
    ipfs_base: bool,
) -> Result<PinnedEndpoint, GovernanceDagServiceError> {
    if raw.is_empty()
        || raw.trim() != raw
        || raw.contains('\\')
        || raw.chars().any(char::is_control)
    {
        return Err(GovernanceDagServiceError::Config(
            "endpoint URL contains non-canonical text".to_owned(),
        ));
    }
    let mut url = Url::parse(raw).map_err(|_| {
        GovernanceDagServiceError::Config("endpoint URL is not absolute".to_owned())
    })?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(GovernanceDagServiceError::Config(
            "endpoint URL must not contain credentials, query, or fragment".to_owned(),
        ));
    }
    match url.scheme() {
        "https" => {}
        "http" if config.allow_insecure_http => {}
        "http" => {
            return Err(GovernanceDagServiceError::Config(
                "plain HTTP endpoint requires allow_insecure_http".to_owned(),
            ));
        }
        _ => {
            return Err(GovernanceDagServiceError::Config(
                "endpoint URL scheme must be http or https".to_owned(),
            ));
        }
    }
    let host = url
        .host_str()
        .ok_or_else(|| GovernanceDagServiceError::Config("endpoint URL has no host".to_owned()))?
        .to_owned();
    let port = url.port_or_known_default().ok_or_else(|| {
        GovernanceDagServiceError::Config("endpoint URL has no usable port".to_owned())
    })?;
    if ipfs_base {
        let path = url.path().trim_end_matches('/');
        let normalized_path = if path.is_empty() {
            "/".to_owned()
        } else {
            format!("{path}/")
        };
        url.set_path(&normalized_path);
    }

    let allow_private_endpoint = if ipfs_base {
        config.allow_private_ipfs_endpoint
    } else {
        config.allow_private_head_endpoint
    };
    let resolution = async {
        tokio::net::lookup_host((host.as_str(), port))
            .await
            .map(|addresses| addresses.collect::<Vec<_>>())
    };
    let addresses =
        resolve_endpoint_addresses(resolution, config.dns_timeout, allow_private_endpoint).await?;
    let mut builder = Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .referer(false)
        .connect_timeout(config.connect_timeout)
        .timeout(config.request_timeout)
        .pool_max_idle_per_host(2)
        .user_agent("iroha-sorafs-governance-dag/1");
    if host.parse::<IpAddr>().is_err() {
        builder = builder.resolve_to_addrs(&host, &addresses);
    }
    let client = builder.build().map_err(|_| {
        GovernanceDagServiceError::Config("cannot construct hardened HTTP client".to_owned())
    })?;
    Ok(PinnedEndpoint {
        url,
        client,
        authentication_scope,
        authenticator,
        max_request_bytes: config.max_request_bytes.0,
    })
}

async fn resolve_endpoint_addresses<F>(
    resolution: F,
    timeout: Duration,
    allow_private: bool,
) -> Result<Vec<SocketAddr>, GovernanceDagServiceError>
where
    F: Future<Output = io::Result<Vec<SocketAddr>>>,
{
    let mut addresses = time::timeout(timeout, resolution)
        .await
        .map_err(|_| {
            GovernanceDagServiceError::Config("endpoint DNS resolution timed out".to_owned())
        })?
        .map_err(|_| {
            GovernanceDagServiceError::Config("endpoint DNS resolution failed".to_owned())
        })?;
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.is_empty() || addresses.len() > MAX_DNS_ADDRESSES {
        return Err(GovernanceDagServiceError::Config(format!(
            "endpoint DNS must resolve to 1..={MAX_DNS_ADDRESSES} addresses"
        )));
    }
    if !allow_private
        && addresses
            .iter()
            .any(|address| !is_publicly_routable(address.ip()))
    {
        return Err(GovernanceDagServiceError::Config(
            "endpoint DNS includes a private, local, reserved, or documentation address".to_owned(),
        ));
    }
    Ok(addresses)
}

fn is_publicly_routable(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => {
            if let Some(ipv4) = ip.to_ipv4_mapped() {
                return is_public_ipv4(ipv4);
            }
            is_public_ipv6(ip)
        }
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || octets[0] == 0
        || octets[0] >= 240
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 198 && matches!(octets[1], 18 | 19)))
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || (segments[0] == 0x2001 && segments[1] == 0x0010))
}

impl PinnedEndpoint {
    fn request(
        &self,
        method: Method,
        url: Url,
    ) -> Result<reqwest::RequestBuilder, GovernanceDagServiceError> {
        Ok(self
            .client
            .request(method, url)
            .header(header::ACCEPT_ENCODING.as_str(), "identity"))
    }

    async fn execute(
        &self,
        request: RequestBuilder,
        failure: &'static str,
    ) -> Result<reqwest::Response, GovernanceDagServiceError> {
        // Build exactly once after the caller has attached its final byte body
        // and conditional headers. The runtime adapter receives only the
        // bounded data-only descriptor and cannot mutate HTTP state.
        let mut request = request.build().map_err(|_| {
            GovernanceDagServiceError::Network(
                "Governance DAG outbound request could not be finalized".to_owned(),
            )
        })?;
        let descriptor = canonical_outbound_request_descriptor(
            &request,
            self.authentication_scope,
            self.max_request_bytes,
        )?;
        let envelope = self.authenticator.authenticate(&descriptor)?;
        attach_request_authentication_headers(&mut request, &envelope)?;
        let response = self.client.execute(request).await;
        // A provider may be revoked or substituted while the request is in
        // flight. Discard the response unless the same qualified identity is
        // still active after execution.
        self.authenticator.assert_identity()?;
        response.map_err(|_| GovernanceDagServiceError::Network(failure.to_owned()))
    }

    fn ipfs_url(
        &self,
        operation: &str,
        query: &[(&str, &str)],
    ) -> Result<Url, GovernanceDagServiceError> {
        if operation.is_empty()
            || operation.len() > 256
            || operation.starts_with('/')
            || operation.contains('\\')
            || operation.split('/').any(|component| {
                component.is_empty()
                    || matches!(component, "." | "..")
                    || !component.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                    })
            })
        {
            return Err(GovernanceDagServiceError::Network(
                "IPFS operation path is not canonical".to_owned(),
            ));
        }
        let mut url = self.url.join(operation).map_err(|_| {
            GovernanceDagServiceError::Network("cannot construct configured IPFS URL".to_owned())
        })?;
        let mut canonical_query = query.to_vec();
        canonical_query.sort_unstable();
        if canonical_query
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0)
            || canonical_query.iter().any(|(key, value)| {
                key.is_empty()
                    || key.len() > 128
                    || value.len() > 1024
                    || !key.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                    })
                    || value.chars().any(char::is_control)
            })
        {
            return Err(GovernanceDagServiceError::Network(
                "IPFS query is not canonical or bounded".to_owned(),
            ));
        }
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in canonical_query {
                pairs.append_pair(key, value);
            }
        }
        Ok(url)
    }
}

fn canonical_outbound_request_descriptor(
    request: &reqwest::Request,
    scope: GovernanceDagAuthenticationScope,
    max_request_bytes: u64,
) -> Result<GovernanceDagCanonicalRequestV1, GovernanceDagServiceError> {
    let body = match request.body() {
        None => &[][..],
        Some(body) => body.as_bytes().ok_or_else(|| {
            GovernanceDagServiceError::Network(
                "Governance DAG outbound request body must be complete in-memory bytes".to_owned(),
            )
        })?,
    };
    canonicalize_governance_dag_outbound_http_request_v1(
        scope,
        request.method().as_str(),
        request.url().as_str(),
        request
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_bytes())),
        body,
        max_request_bytes,
    )
    .map_err(|error| {
        GovernanceDagServiceError::Network(format!(
            "Governance DAG outbound request was rejected: {error}"
        ))
    })
}

fn attach_request_authentication_headers(
    request: &mut reqwest::Request,
    envelope: &GovernanceDagRequestAuthenticationEnvelopeV1,
) -> Result<(), GovernanceDagServiceError> {
    for (name, value) in governance_dag_request_authentication_headers_v1(envelope) {
        let value = HeaderValue::from_str(&value).map_err(|_| {
            GovernanceDagServiceError::Network(
                "Governance DAG request-auth public header encoding failed".to_owned(),
            )
        })?;
        request
            .headers_mut()
            .insert(header::HeaderName::from_static(name), value);
    }
    Ok(())
}

async fn read_bounded_response(
    mut response: reqwest::Response,
    max_bytes: u64,
) -> Result<Vec<u8>, GovernanceDagServiceError> {
    let headers = response.headers();
    if headers.len() > MAX_RESPONSE_HEADERS {
        return Err(GovernanceDagServiceError::Network(
            "remote response contains too many headers".to_owned(),
        ));
    }
    let header_bytes = headers
        .iter()
        .try_fold(0_usize, |total, (name, value)| {
            total
                .checked_add(name.as_str().len())?
                .checked_add(value.as_bytes().len())
        })
        .ok_or_else(|| {
            GovernanceDagServiceError::Network("remote header size overflow".to_owned())
        })?;
    if header_bytes > MAX_RESPONSE_HEADER_BYTES {
        return Err(GovernanceDagServiceError::Network(
            "remote response headers exceed the configured safety limit".to_owned(),
        ));
    }
    if let Some(encoding) = headers.get(header::CONTENT_ENCODING)
        && encoding.as_bytes() != b"identity"
    {
        return Err(GovernanceDagServiceError::Network(
            "compressed remote responses are forbidden".to_owned(),
        ));
    }
    let advertised_len = response.content_length();
    if advertised_len.is_some_and(|length| length > max_bytes) {
        return Err(GovernanceDagServiceError::Network(
            "remote response exceeds the configured body limit".to_owned(),
        ));
    }
    let capacity = usize::try_from(advertised_len.unwrap_or(0).min(max_bytes)).unwrap_or(0);
    let mut body = Vec::with_capacity(capacity);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| GovernanceDagServiceError::Network("remote response body failed".to_owned()))?
    {
        let next_len = body.len().checked_add(chunk.len()).ok_or_else(|| {
            GovernanceDagServiceError::Network("remote response size overflow".to_owned())
        })?;
        if next_len as u64 > max_bytes {
            return Err(GovernanceDagServiceError::Network(
                "chunked remote response exceeds the configured body limit".to_owned(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    if advertised_len.is_some_and(|length| length != body.len() as u64) {
        return Err(GovernanceDagServiceError::Network(
            "remote response Content-Length does not match the body".to_owned(),
        ));
    }
    Ok(body)
}

fn validate_ipfs_cid(value: &str) -> Result<String, GovernanceDagServiceError> {
    if !is_canonical_cid_v1(value) {
        return Err(GovernanceDagServiceError::Network(
            "IPFS API returned a non-canonical CIDv1 base32 value".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn validate_ipfs_cid_for_bytes(
    value: &str,
    bytes: &[u8],
) -> Result<String, GovernanceDagServiceError> {
    let cid = validate_ipfs_cid(value)?;
    let expected = canonical_raw_sha256_cid(bytes);
    if cid != expected {
        return Err(GovernanceDagServiceError::Network(
            "IPFS API returned a CID that does not commit to the uploaded bytes".to_owned(),
        ));
    }
    Ok(cid)
}

fn canonical_raw_sha256_cid(bytes: &[u8]) -> String {
    const CID_VERSION_V1: u8 = 0x01;
    const RAW_CODEC: u8 = 0x55;
    const SHA2_256_MULTIHASH: u8 = 0x12;
    const SHA2_256_DIGEST_LENGTH: u8 = 32;

    let digest = iroha_crypto::sha256(bytes);
    let mut cid = Vec::with_capacity(4 + digest.len());
    cid.extend_from_slice(&[
        CID_VERSION_V1,
        RAW_CODEC,
        SHA2_256_MULTIHASH,
        SHA2_256_DIGEST_LENGTH,
    ]);
    cid.extend_from_slice(&digest);
    encode_base32_lower_no_pad(&cid)
}

fn encode_base32_lower_no_pad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

    let mut accumulator = 0_u32;
    let mut bits = 0_u32;
    let mut encoded = String::with_capacity(1 + (bytes.len() * 8).div_ceil(5));
    encoded.push('b');
    for byte in bytes {
        accumulator = (accumulator << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            let index = ((accumulator >> (bits - 5)) & 0x1f) as usize;
            encoded.push(char::from(ALPHABET[index]));
            bits -= 5;
        }
        accumulator = if bits == 0 {
            0
        } else {
            accumulator & ((1_u32 << bits) - 1)
        };
    }
    if bits > 0 {
        let index = ((accumulator << (5 - bits)) & 0x1f) as usize;
        encoded.push(char::from(ALPHABET[index]));
    }
    encoded
}

fn is_canonical_cid_v1(value: &str) -> bool {
    if value.len() < 2
        || value.len() > MAX_IPFS_CID_BYTES
        || !value.starts_with('b')
        || !value[1..]
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'2'..=b'7'))
    {
        return false;
    }
    let Some(bytes) = decode_base32_lower_no_pad(&value[1..]) else {
        return false;
    };
    let Some((version, version_len)) = decode_canonical_uvarint(&bytes) else {
        return false;
    };
    if version != 1 {
        return false;
    }
    let Some((codec, codec_len)) = decode_canonical_uvarint(&bytes[version_len..]) else {
        return false;
    };
    if codec == 0 {
        return false;
    }
    let multihash_offset = version_len.saturating_add(codec_len);
    let Some((multihash, multihash_len)) = decode_canonical_uvarint(&bytes[multihash_offset..])
    else {
        return false;
    };
    if multihash == 0 {
        return false;
    }
    let digest_len_offset = multihash_offset.saturating_add(multihash_len);
    let Some((digest_len, digest_len_bytes)) =
        decode_canonical_uvarint(&bytes[digest_len_offset..])
    else {
        return false;
    };
    if digest_len == 0 || digest_len > 64 {
        return false;
    }
    let digest_offset = digest_len_offset.saturating_add(digest_len_bytes);
    let Ok(digest_len) = usize::try_from(digest_len) else {
        return false;
    };
    digest_offset
        .checked_add(digest_len)
        .is_some_and(|end| end == bytes.len())
}

fn decode_base32_lower_no_pad(value: &str) -> Option<Vec<u8>> {
    let mut accumulator = 0_u32;
    let mut bits = 0_u32;
    let mut bytes = Vec::with_capacity((value.len() * 5) / 8);
    for byte in value.bytes() {
        let digit = match byte {
            b'a'..=b'z' => u32::from(byte - b'a'),
            b'2'..=b'7' => 26 + u32::from(byte - b'2'),
            _ => return None,
        };
        accumulator = (accumulator << 5) | digit;
        bits += 5;
        while bits >= 8 {
            bytes.push(((accumulator >> (bits - 8)) & 0xff) as u8);
            bits -= 8;
        }
        accumulator = if bits == 0 {
            0
        } else {
            accumulator & ((1_u32 << bits) - 1)
        };
    }
    if bits > 0 {
        let mask = (1_u32 << bits) - 1;
        if accumulator & mask != 0 {
            return None;
        }
    }
    (!bytes.is_empty()).then_some(bytes)
}

fn decode_canonical_uvarint(bytes: &[u8]) -> Option<(u64, usize)> {
    let mut value = 0_u64;
    for (index, byte) in bytes.iter().copied().take(10).enumerate() {
        let payload = u64::from(byte & 0x7f);
        if index == 9 && payload > 1 {
            return None;
        }
        value |= payload << (index * 7);
        if byte & 0x80 == 0 {
            if index > 0 && payload == 0 {
                return None;
            }
            return Some((value, index + 1));
        }
    }
    None
}

async fn ipfs_add_verified(
    endpoint: &PinnedEndpoint,
    name: &str,
    bytes: &[u8],
    max_request_bytes: u64,
    max_response_bytes: u64,
) -> Result<String, GovernanceDagServiceError> {
    if bytes.is_empty() || bytes.len() as u64 > max_request_bytes {
        return Err(GovernanceDagServiceError::Network(
            "local IPFS object violates the configured request bound".to_owned(),
        ));
    }
    let url = endpoint.ipfs_url(
        "api/v0/add",
        &[
            ("pin", "false"),
            ("cid-version", "1"),
            ("hash", "sha2-256"),
            ("raw-leaves", "true"),
            ("wrap-with-directory", "false"),
            ("quieter", "true"),
        ],
    )?;
    let (boundary, body) = canonical_ipfs_multipart_body(name, bytes)?;
    let request = endpoint
        .request(Method::POST, url)?
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body);
    let response = endpoint.execute(request, "IPFS add request failed").await?;
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(GovernanceDagServiceError::Network(format!(
            "IPFS add returned HTTP {status}"
        )));
    }
    let body = read_bounded_response(response, max_response_bytes).await?;
    let value: JsonValue = json::from_slice(&body).map_err(|_| {
        GovernanceDagServiceError::Network("IPFS add returned malformed JSON".to_owned())
    })?;
    let cid = value
        .get("Hash")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            GovernanceDagServiceError::Network("IPFS add response has no Hash".to_owned())
        })?;
    let cid = validate_ipfs_cid_for_bytes(cid, bytes)?;
    ipfs_pin(endpoint, &cid, max_response_bytes).await?;
    ipfs_verify_pin(endpoint, &cid, max_response_bytes).await?;
    let readback = ipfs_cat(endpoint, &cid, bytes.len() as u64, max_request_bytes).await?;
    if readback != bytes {
        return Err(GovernanceDagServiceError::Network(
            "IPFS readback bytes do not match the published object".to_owned(),
        ));
    }
    Ok(cid)
}

fn canonical_ipfs_multipart_body(
    name: &str,
    bytes: &[u8],
) -> Result<(String, Vec<u8>), GovernanceDagServiceError> {
    if name.is_empty()
        || name.len() > 160
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(GovernanceDagServiceError::Network(
            "IPFS multipart filename is not a bounded ASCII token".to_owned(),
        ));
    }
    let digest = hex::encode(blake3_array(bytes));
    let digest_prefix = &digest[..32];
    let boundary = (0_u8..=16)
        .map(|attempt| {
            if attempt == 0 {
                format!("{IPFS_MULTIPART_BOUNDARY_PREFIX}-{digest_prefix}")
            } else {
                format!("{IPFS_MULTIPART_BOUNDARY_PREFIX}-{digest_prefix}-{attempt}")
            }
        })
        .find(|candidate| {
            let marker = format!("--{candidate}");
            !bytes
                .windows(marker.len())
                .any(|window| window == marker.as_bytes())
        })
        .ok_or_else(|| {
            GovernanceDagServiceError::Network(
                "IPFS object conflicts with every deterministic multipart boundary".to_owned(),
            )
        })?;
    let prelude = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\n\
         Content-Type: application/vnd.ipld.raw\r\n\r\n"
    );
    let epilogue = format!("\r\n--{boundary}--\r\n");
    let capacity = prelude
        .len()
        .checked_add(bytes.len())
        .and_then(|length| length.checked_add(epilogue.len()))
        .ok_or_else(|| {
            GovernanceDagServiceError::Network(
                "IPFS multipart body length exceeds host limits".to_owned(),
            )
        })?;
    let mut body = Vec::with_capacity(capacity);
    body.extend_from_slice(prelude.as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(epilogue.as_bytes());
    Ok((boundary, body))
}

async fn ipfs_pin(
    endpoint: &PinnedEndpoint,
    cid: &str,
    max_response_bytes: u64,
) -> Result<(), GovernanceDagServiceError> {
    let url = endpoint.ipfs_url("api/v0/pin/add", &[("arg", cid), ("recursive", "true")])?;
    let request = endpoint.request(Method::POST, url)?;
    let response = endpoint.execute(request, "IPFS pin request failed").await?;
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(GovernanceDagServiceError::Network(format!(
            "IPFS pin returned HTTP {status}"
        )));
    }
    let _ = read_bounded_response(response, max_response_bytes).await?;
    Ok(())
}

async fn ipfs_verify_pin(
    endpoint: &PinnedEndpoint,
    cid: &str,
    max_response_bytes: u64,
) -> Result<(), GovernanceDagServiceError> {
    let url = endpoint.ipfs_url("api/v0/pin/ls", &[("arg", cid), ("type", "recursive")])?;
    let request = endpoint.request(Method::POST, url)?;
    let response = endpoint
        .execute(request, "IPFS pin verification failed")
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(GovernanceDagServiceError::Network(format!(
            "IPFS pin verification returned HTTP {status}"
        )));
    }
    let body = read_bounded_response(response, max_response_bytes).await?;
    let value: JsonValue = json::from_slice(&body).map_err(|_| {
        GovernanceDagServiceError::Network("IPFS pin verification JSON is invalid".to_owned())
    })?;
    if value
        .get("Keys")
        .and_then(JsonValue::as_object)
        .is_none_or(|keys| !keys.contains_key(cid))
    {
        return Err(GovernanceDagServiceError::Network(
            "IPFS object is not recursively pinned".to_owned(),
        ));
    }
    Ok(())
}

async fn ipfs_cat(
    endpoint: &PinnedEndpoint,
    cid: &str,
    expected_max: u64,
    configured_max: u64,
) -> Result<Vec<u8>, GovernanceDagServiceError> {
    let url = endpoint.ipfs_url("api/v0/cat", &[("arg", cid)])?;
    let request = endpoint.request(Method::POST, url)?;
    let response = endpoint.execute(request, "IPFS cat request failed").await?;
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, configured_max).await;
        return Err(GovernanceDagServiceError::Network(format!(
            "IPFS cat returned HTTP {status}"
        )));
    }
    read_bounded_response(response, expected_max.min(configured_max)).await
}

fn validate_remote_head(
    bytes: &[u8],
    source: &SourceSnapshot,
    config: &RuntimeConfig,
) -> Result<GovernanceDagHeadV1, GovernanceDagServiceError> {
    let head: GovernanceDagHeadV1 = decode_canonical(bytes, "public Governance DAG head")?;
    head.validate().map_err(|err| {
        GovernanceDagServiceError::Conflict(format!("public head is invalid: {err}"))
    })?;
    let source_block_count = u64::try_from(source.blocks.len()).map_err(|_| {
        GovernanceDagServiceError::State("source block count exceeds u64".to_owned())
    })?;
    if head.head_signature.algorithm != GovernanceSignatureAlgorithm::Ed25519
        || head.block_count == 0
        || head.block_count > source_block_count
    {
        return Err(GovernanceDagServiceError::Conflict(
            "public head algorithm or block count is incompatible with the source chain".to_owned(),
        ));
    }
    let block_count = usize::try_from(head.block_count).map_err(|_| {
        GovernanceDagServiceError::Conflict("public head count exceeds host limits".to_owned())
    })?;
    let blocks = source.blocks[..block_count]
        .iter()
        .map(|block| block.block.clone())
        .collect::<Vec<_>>();
    validate_source_head_chain(&head, &blocks).map_err(|err| {
        GovernanceDagServiceError::Conflict(format!(
            "public head is not a verified prefix of the local chain: {err}"
        ))
    })?;
    if head.generated_at
        > current_unix_timestamp_seconds().saturating_add(config.max_future_skew_secs)
    {
        return Err(GovernanceDagServiceError::Conflict(
            "public head is not a verified prefix of the local chain".to_owned(),
        ));
    }
    Ok(head)
}

async fn fetch_signed_http_head(
    endpoint: &PinnedEndpoint,
    max_response_bytes: u64,
) -> Result<PublicHead, GovernanceDagServiceError> {
    let request = endpoint.request(Method::GET, endpoint.url.clone())?;
    let response = endpoint.execute(request, "signed-head GET failed").await?;
    if response.status() == StatusCode::NOT_FOUND {
        let _ = read_bounded_response(response, max_response_bytes).await?;
        return Ok(PublicHead::Missing);
    }
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(GovernanceDagServiceError::Network(format!(
            "signed-head GET returned HTTP {status}"
        )));
    }
    let etag = response
        .headers()
        .get(header::ETAG)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.starts_with('"') && value.ends_with('"'))
        .filter(|value| value.len() <= MAX_PUBLIC_TOKEN_BYTES)
        .ok_or_else(|| {
            GovernanceDagServiceError::Network("signed-head GET has no canonical ETag".to_owned())
        })?
        .to_owned();
    let bytes = read_bounded_response(response, max_response_bytes).await?;
    Ok(PublicHead::Present { bytes, token: etag })
}

async fn put_signed_http_head(
    endpoint: &PinnedEndpoint,
    bytes: &[u8],
    current: &PublicHead,
    allow_bootstrap: bool,
    max_response_bytes: u64,
) -> Result<PublicHead, GovernanceDagServiceError> {
    let mut request = endpoint
        .request(Method::PUT, endpoint.url.clone())?
        .header(header::CONTENT_TYPE, "application/vnd.iroha.norito")
        .body(bytes.to_vec());
    match current {
        PublicHead::Present { token, .. } => {
            request = request.header(header::IF_MATCH, token);
        }
        PublicHead::Missing if allow_bootstrap => {
            request = request.header(header::IF_NONE_MATCH, "*");
        }
        PublicHead::Missing => {
            return Err(GovernanceDagServiceError::Conflict(
                "public signed head is missing and bootstrap is disabled".to_owned(),
            ));
        }
    }
    let response = endpoint.execute(request, "signed-head PUT failed").await?;
    if matches!(
        response.status(),
        StatusCode::CONFLICT | StatusCode::PRECONDITION_FAILED
    ) {
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(GovernanceDagServiceError::Conflict(
            "signed-head conditional update lost a concurrent-writer race".to_owned(),
        ));
    }
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(GovernanceDagServiceError::Network(format!(
            "signed-head PUT returned HTTP {status}"
        )));
    }
    let _ = read_bounded_response(response, max_response_bytes).await?;
    let readback = fetch_signed_http_head(endpoint, max_response_bytes).await?;
    if !matches!(&readback, PublicHead::Present { bytes: observed, .. } if observed == bytes) {
        return Err(GovernanceDagServiceError::Conflict(
            "signed-head readback does not match the conditional update".to_owned(),
        ));
    }
    Ok(readback)
}

async fn resolve_ipns_head(
    ipfs: &PinnedEndpoint,
    name: &str,
    max_response_bytes: u64,
) -> Result<PublicHead, GovernanceDagServiceError> {
    let url = ipfs.ipfs_url(
        "api/v0/name/resolve",
        &[("arg", name), ("recursive", "true"), ("nocache", "true")],
    )?;
    let request = ipfs.request(Method::POST, url)?;
    let response = ipfs.execute(request, "IPNS resolve failed").await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = read_bounded_response(response, max_response_bytes).await?;
        if is_authenticated_ipns_absence(status, &body) {
            return Ok(PublicHead::Missing);
        }
        return Err(GovernanceDagServiceError::Network(format!(
            "IPNS resolve returned HTTP {status}"
        )));
    }
    let body = read_bounded_response(response, max_response_bytes).await?;
    let value: JsonValue = json::from_slice(&body).map_err(|_| {
        GovernanceDagServiceError::Network("IPNS resolve JSON is invalid".to_owned())
    })?;
    let path = value
        .get("Path")
        .and_then(JsonValue::as_str)
        .and_then(|value| value.strip_prefix("/ipfs/"))
        .ok_or_else(|| {
            GovernanceDagServiceError::Network("IPNS resolve path is invalid".to_owned())
        })?;
    let cid = validate_ipfs_cid(path)?;
    let bytes = ipfs_cat(ipfs, &cid, max_response_bytes, max_response_bytes).await?;
    Ok(PublicHead::Present { bytes, token: cid })
}

fn is_authenticated_ipns_absence(status: StatusCode, body: &[u8]) -> bool {
    if status == StatusCode::NOT_FOUND {
        return true;
    }
    if status != StatusCode::INTERNAL_SERVER_ERROR {
        return false;
    }
    let Ok(value) = json::from_slice::<JsonValue>(body) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 3
        && object
            .get("Message")
            .and_then(JsonValue::as_str)
            .is_some_and(|message| message == "could not resolve name")
        && object.get("Code").and_then(JsonValue::as_u64) == Some(0)
        && object
            .get("Type")
            .and_then(JsonValue::as_str)
            .is_some_and(|kind| kind == "error")
}

struct IpnsHeadPublishRequest<'a> {
    name: &'a str,
    key_name: &'a str,
    head_cid: &'a str,
    bytes: &'a [u8],
    initial: &'a PublicHead,
    allow_bootstrap: bool,
    max_response_bytes: u64,
}

async fn publish_ipns_head(
    ipfs: &PinnedEndpoint,
    request: IpnsHeadPublishRequest<'_>,
) -> Result<PublicHead, GovernanceDagServiceError> {
    let IpnsHeadPublishRequest {
        name,
        key_name,
        head_cid,
        bytes,
        initial,
        allow_bootstrap,
        max_response_bytes,
    } = request;
    let before = resolve_ipns_head(ipfs, name, max_response_bytes).await?;
    if public_head_identity(&before) != public_head_identity(initial) {
        return Err(GovernanceDagServiceError::Conflict(
            "IPNS name moved before publication".to_owned(),
        ));
    }
    if matches!(before, PublicHead::Missing) && !allow_bootstrap {
        return Err(GovernanceDagServiceError::Conflict(
            "IPNS name is unresolved and bootstrap is disabled".to_owned(),
        ));
    }
    let target = format!("/ipfs/{head_cid}");
    let url = ipfs.ipfs_url(
        "api/v0/name/publish",
        &[
            ("arg", target.as_str()),
            ("key", key_name),
            ("allow-offline", "false"),
            ("lifetime", "24h"),
        ],
    )?;
    let request = ipfs.request(Method::POST, url)?;
    let response = ipfs.execute(request, "IPNS publish failed").await?;
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(GovernanceDagServiceError::Network(format!(
            "IPNS publish returned HTTP {status}"
        )));
    }
    let _ = read_bounded_response(response, max_response_bytes).await?;
    let after = resolve_ipns_head(ipfs, name, max_response_bytes).await?;
    if !matches!(&after, PublicHead::Present { bytes: observed, token } if observed == bytes && token == head_cid)
    {
        return Err(GovernanceDagServiceError::Conflict(
            "IPNS readback does not match the published head".to_owned(),
        ));
    }
    Ok(after)
}

fn public_head_identity(head: &PublicHead) -> Option<([u8; 32], String)> {
    match head {
        PublicHead::Missing => None,
        PublicHead::Present { bytes, token } => Some((blake3_array(bytes), token.clone())),
    }
}

fn public_head_digest(head: &PublicHead) -> Option<[u8; 32]> {
    match head {
        PublicHead::Missing => None,
        PublicHead::Present { bytes, .. } => Some(blake3_array(bytes)),
    }
}

impl Service {
    fn refresh_durable_state(&mut self) -> Result<(), GovernanceDagServiceError> {
        let (checkpoint, checkpoint_revision) = load_checkpoint(&self.checkpoint_store)?;
        if self.checkpoint.is_some() && checkpoint.is_none() {
            return Err(GovernanceDagServiceError::State(
                "sealed checkpoint store removed the active checkpoint".to_owned(),
            ));
        }
        if checkpoint
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.generation < self.checkpoint_generation_floor)
        {
            return Err(GovernanceDagServiceError::State(
                "sealed checkpoint store rolled back below the process generation floor".to_owned(),
            ));
        }
        let (intent, intent_revision) = load_publish_intent(&self.checkpoint_store)?;
        if self.intent.is_some() && intent.is_none() {
            return Err(GovernanceDagServiceError::State(
                "sealed checkpoint store removed the active publish intent".to_owned(),
            ));
        }
        if intent
            .as_ref()
            .is_some_and(|intent| intent.generation < self.intent_generation_floor)
        {
            return Err(GovernanceDagServiceError::State(
                "sealed publish-intent store replayed an older generation".to_owned(),
            ));
        }
        self.checkpoint_generation_floor = checkpoint
            .as_ref()
            .map_or(self.checkpoint_generation_floor, |checkpoint| {
                checkpoint.generation
            });
        self.intent_generation_floor = intent
            .as_ref()
            .map_or(self.intent_generation_floor, |intent| intent.generation);
        self.checkpoint = checkpoint;
        self.checkpoint_revision = checkpoint_revision;
        self.intent = intent;
        self.intent_revision = intent_revision;
        Ok(())
    }

    fn assert_durable_state_unchanged(&self) -> Result<(), GovernanceDagServiceError> {
        let (_, checkpoint_revision) = load_checkpoint(&self.checkpoint_store)?;
        let (_, intent_revision) = load_publish_intent(&self.checkpoint_store)?;
        if checkpoint_revision != self.checkpoint_revision
            || intent_revision != self.intent_revision
        {
            return Err(GovernanceDagServiceError::State(
                "sealed checkpoint or publish intent changed during initial reconciliation"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    async fn validate_initial_state(&mut self) -> Result<(), GovernanceDagServiceError> {
        self.refresh_durable_state()?;
        let source = load_committed_source_snapshot(&self.config, &self.checkpoint_store)?;
        validate_checkpoint_against_source(self.checkpoint.as_ref(), &source)?;
        if let Some(intent) = &self.intent {
            validate_intent_against_source(
                intent,
                self.checkpoint.as_ref(),
                &source,
                &self.config,
            )?;
        }

        // Initial reconciliation must not publish. It establishes that the
        // authenticated public head is one of the durable crash-recovery
        // states the first reconciliation is permitted to advance.
        let public = self.fetch_public_head().await?;
        if let PublicHead::Present { bytes, .. } = &public {
            validate_remote_head(bytes, &source, &self.config)?;
        } else if !self.config.allow_head_bootstrap {
            return Err(GovernanceDagServiceError::Conflict(
                "no public head exists and bootstrap is disabled".to_owned(),
            ));
        }
        self.assert_durable_state_unchanged()?;

        match (&self.checkpoint, &self.intent) {
            (Some(checkpoint), Some(intent))
                if checkpoint.generation == intent.generation
                    && checkpoint.head_block_cid == intent.target_head_block_cid =>
            {
                if checkpoint.head_bytes_blake3 != intent.target_head_blake3 {
                    return Err(GovernanceDagServiceError::State(
                        "checkpoint and publish intent disagree on the installed target head"
                            .to_owned(),
                    ));
                }
                require_public_matches_checkpoint(&public, checkpoint)
            }
            (checkpoint, Some(intent)) => {
                if let Some(checkpoint) = checkpoint
                    && intent.previous_public_head_blake3 != Some(checkpoint.head_bytes_blake3)
                {
                    return Err(GovernanceDagServiceError::State(
                        "publish intent predecessor does not match the authenticated checkpoint"
                            .to_owned(),
                    ));
                }
                let observed = public_head_digest(&public);
                if observed != intent.previous_public_head_blake3
                    && observed != Some(intent.target_head_blake3)
                {
                    return Err(GovernanceDagServiceError::Conflict(
                        "public head is neither the durable publish-intent predecessor nor target"
                            .to_owned(),
                    ));
                }
                Ok(())
            }
            (Some(checkpoint), None) => require_public_matches_checkpoint(&public, checkpoint),
            (None, None) => Ok(()),
        }
    }

    async fn fetch_public_head(&self) -> Result<PublicHead, GovernanceDagServiceError> {
        match &self.head_mode {
            HeadMode::SignedHttp(endpoint) => {
                fetch_signed_http_head(endpoint, self.config.max_response_bytes).await
            }
            HeadMode::Ipns { name, .. } => {
                resolve_ipns_head(&self.ipfs, name, self.config.max_response_bytes).await
            }
        }
    }

    async fn install_public_head(
        &self,
        bytes: &[u8],
        head_cid: &str,
        current: &PublicHead,
    ) -> Result<PublicHead, GovernanceDagServiceError> {
        match &self.head_mode {
            HeadMode::SignedHttp(endpoint) => {
                put_signed_http_head(
                    endpoint,
                    bytes,
                    current,
                    self.config.allow_head_bootstrap,
                    self.config.max_response_bytes,
                )
                .await
            }
            HeadMode::Ipns { name, key_name } => {
                publish_ipns_head(
                    &self.ipfs,
                    IpnsHeadPublishRequest {
                        name,
                        key_name,
                        head_cid,
                        bytes,
                        initial: current,
                        allow_bootstrap: self.config.allow_head_bootstrap,
                        max_response_bytes: self.config.max_response_bytes,
                    },
                )
                .await
            }
        }
    }

    async fn reconcile_once(&mut self) -> Result<(), GovernanceDagServiceError> {
        self.state_lock.verify().map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "service lock binding changed before reconciliation: {error}"
            ))
        })?;
        self.config.revalidate_state_root()?;
        self.refresh_durable_state()?;
        let source = load_committed_source_snapshot(&self.config, &self.checkpoint_store)?;
        self.config.revalidate_state_root()?;
        validate_checkpoint_against_source(self.checkpoint.as_ref(), &source)?;
        if let Some(intent) = &self.intent {
            validate_intent_against_source(
                intent,
                self.checkpoint.as_ref(),
                &source,
                &self.config,
            )?;
        }

        if let Some(checkpoint) = &self.checkpoint
            && checkpoint.head_block_cid == source.head.head_block_cid
            && self.intent.is_none()
        {
            self.verify_steady_state(&source, checkpoint).await?;
            self.publish_api_snapshot(&source, checkpoint, false)
                .await?;
            return Ok(());
        }

        if self.intent.is_none() {
            let current = self.fetch_public_head().await?;
            if let PublicHead::Present { bytes, .. } = &current {
                validate_remote_head(bytes, &source, &self.config)?;
            } else if !self.config.allow_head_bootstrap {
                return Err(GovernanceDagServiceError::Conflict(
                    "no public head exists and bootstrap is disabled".to_owned(),
                ));
            }
            if let Some(checkpoint) = &self.checkpoint {
                require_public_matches_checkpoint(&current, checkpoint)?;
            }
            let previous_public_head_blake3 = match &current {
                PublicHead::Missing => None,
                PublicHead::Present { bytes, .. } => Some(blake3_array(bytes)),
            };
            let start = match self.checkpoint.as_ref() {
                Some(checkpoint) => usize::try_from(checkpoint.block_count).map_err(|_| {
                    GovernanceDagServiceError::State(
                        "checkpoint block count exceeds host limits".to_owned(),
                    )
                })?,
                None => 0,
            };
            let generation = match self.checkpoint.as_ref() {
                Some(checkpoint) => checkpoint.generation.checked_add(1).ok_or_else(|| {
                    GovernanceDagServiceError::State("checkpoint generation exhausted".to_owned())
                })?,
                None => 1,
            };
            let blocks = source.blocks[start..]
                .iter()
                .map(|block| {
                    Ok(IntentBlockV1 {
                        sequence: block.block.sequence,
                        governance_block_cid: block.block.block_cid.clone(),
                        governance_node_cid: block.block.node.node_cid.clone(),
                        payload_kind: block.payload_kind.clone(),
                        timestamp: block.block.timestamp,
                        encoded_blake3: block.encoded_blake3,
                        encoded_len: u64::try_from(block.bytes.len()).map_err(|_| {
                            GovernanceDagServiceError::State(
                                "source block length exceeds u64 while preparing intent".to_owned(),
                            )
                        })?,
                        ipfs_cid: None,
                    })
                })
                .collect::<Result<Vec<_>, GovernanceDagServiceError>>()?;
            if blocks.is_empty() {
                return Err(GovernanceDagServiceError::State(
                    "source head changed without adding a block".to_owned(),
                ));
            }
            let intent = PublishIntentBodyV1 {
                version: PUBLISH_INTENT_VERSION_V1,
                generation,
                target_head_block_cid: source.head.head_block_cid.clone(),
                target_block_count: source.head.block_count,
                target_head_bytes: source.head_bytes.clone(),
                target_head_blake3: blake3_array(&source.head_bytes),
                target_source_index_blake3: source.index_blake3,
                previous_public_head_blake3,
                created_at_unix: current_unix_timestamp_seconds(),
                blocks,
                head_ipfs_cid: None,
            };
            self.intent_revision = Some(save_publish_intent(
                &self.checkpoint_store,
                self.intent_revision,
                &intent,
            )?);
            self.intent_generation_floor = intent.generation;
            self.intent = Some(intent);
        }

        let mut intent = self.intent.take().ok_or_else(|| {
            GovernanceDagServiceError::State(
                "durable publish intent disappeared before execution".to_owned(),
            )
        })?;
        if let Some(checkpoint) = &self.checkpoint
            && checkpoint.generation == intent.generation
            && checkpoint.head_block_cid == intent.target_head_block_cid
        {
            let current = self.fetch_public_head().await?;
            require_public_matches_checkpoint(&current, checkpoint)?;
            verify_or_recover_mirror_file(&self.config, checkpoint, &source)?;
            delete_publish_intent(&self.checkpoint_store, self.intent_revision)?;
            self.intent_revision = None;
            self.intent = None;
            self.publish_api_snapshot(&source, checkpoint, false)
                .await?;
            return Ok(());
        }

        let mut published_bytes = 0_u64;
        let mut pin_lag = 0_u64;
        for position in 0..intent.blocks.len() {
            if intent.blocks[position].ipfs_cid.is_some() {
                continue;
            }
            let sequence = usize::try_from(intent.blocks[position].sequence).map_err(|_| {
                GovernanceDagServiceError::State("intent sequence exceeds host limits".to_owned())
            })?;
            let source_block = source.blocks.get(sequence).ok_or_else(|| {
                GovernanceDagServiceError::State(
                    "intent block no longer exists in the source".to_owned(),
                )
            })?;
            let cid = ipfs_add_verified(
                &self.ipfs,
                &format!(
                    "governance-dag-block-{:020}.to",
                    source_block.block.sequence
                ),
                &source_block.bytes,
                self.config.max_request_bytes,
                self.config.max_response_bytes,
            )
            .await?;
            intent.blocks[position].ipfs_cid = Some(cid);
            published_bytes = published_bytes.saturating_add(source_block.bytes.len() as u64);
            pin_lag = pin_lag
                .max(current_unix_timestamp_seconds().saturating_sub(source_block.block.timestamp));
            self.intent_revision = Some(save_publish_intent(
                &self.checkpoint_store,
                self.intent_revision,
                &intent,
            )?);
            self.intent_generation_floor = intent.generation;
        }
        if intent.head_ipfs_cid.is_none() {
            let cid = ipfs_add_verified(
                &self.ipfs,
                "governance-dag-head.to",
                &intent.target_head_bytes,
                self.config.max_request_bytes,
                self.config.max_response_bytes,
            )
            .await?;
            published_bytes = published_bytes.saturating_add(intent.target_head_bytes.len() as u64);
            intent.head_ipfs_cid = Some(cid);
            self.intent_revision = Some(save_publish_intent(
                &self.checkpoint_store,
                self.intent_revision,
                &intent,
            )?);
            self.intent_generation_floor = intent.generation;
        }
        let head_ipfs_cid = intent.head_ipfs_cid.clone().ok_or_else(|| {
            GovernanceDagServiceError::State(
                "head IPFS CID is missing after verified publication".to_owned(),
            )
        })?;

        let current = self.fetch_public_head().await?;
        if let PublicHead::Present { bytes, .. } = &current {
            validate_remote_head(bytes, &source, &self.config)?;
        }
        let current_digest = public_head_digest(&current);
        let target_already_installed = current_digest == Some(intent.target_head_blake3);
        if !target_already_installed && current_digest != intent.previous_public_head_blake3 {
            self.intent = Some(intent);
            return Err(GovernanceDagServiceError::Conflict(
                "public head moved away from the durable publish intent".to_owned(),
            ));
        }
        let installed = if target_already_installed {
            current
        } else {
            self.install_public_head(&intent.target_head_bytes, &head_ipfs_cid, &current)
                .await?
        };
        let public_token = match &installed {
            PublicHead::Present { bytes, token }
                if blake3_array(bytes) == intent.target_head_blake3 =>
            {
                token.clone()
            }
            _ => {
                self.intent = Some(intent);
                return Err(GovernanceDagServiceError::Conflict(
                    "public head installation did not converge".to_owned(),
                ));
            }
        };

        let published_blocks = merge_published_blocks(
            self.checkpoint.as_ref(),
            &intent,
            &source,
            self.config.mirror_max_entries,
            self.config.mirror_max_bytes,
        )?;
        let published_at = current_unix_timestamp_seconds();
        let mirror = mirror_index_value(
            &source,
            &published_blocks,
            intent.generation,
            &head_ipfs_cid,
            &public_token,
            published_at,
        )?;
        let mirror_bytes = json::to_json_pretty(&mirror)
            .map_err(|err| {
                GovernanceDagServiceError::State(format!("mirror JSON encode failed: {err}"))
            })?
            .into_bytes();
        self.config.revalidate_state_root()?;
        write_rooted_atomic_secret(
            &self.config.state_root_guard,
            Path::new(MIRROR_INDEX_FILE),
            &mirror_bytes,
        )?;
        self.config.revalidate_state_root()?;
        let checkpoint = CheckpointBodyV1 {
            version: CHECKPOINT_VERSION_V1,
            generation: intent.generation,
            head_block_cid: intent.target_head_block_cid.clone(),
            block_count: intent.target_block_count,
            head_bytes_blake3: intent.target_head_blake3,
            head_ipfs_cid,
            public_head_token: public_token,
            source_index_blake3: intent.target_source_index_blake3,
            mirror_blake3: blake3_array(&mirror_bytes),
            published_at_unix: published_at,
            mirror_blocks: published_blocks,
        };
        self.checkpoint_revision = Some(save_checkpoint(
            &self.checkpoint_store,
            self.checkpoint_revision,
            &checkpoint,
        )?);
        self.checkpoint_generation_floor = checkpoint.generation;
        delete_publish_intent(&self.checkpoint_store, self.intent_revision)?;
        self.intent_revision = None;
        self.checkpoint = Some(checkpoint.clone());
        self.intent = None;
        {
            let mut state = self.api.0.write().await;
            state.metrics.publish_success_total =
                state.metrics.publish_success_total.saturating_add(1);
            state.metrics.published_bytes_total = state
                .metrics
                .published_bytes_total
                .saturating_add(published_bytes);
            state.metrics.last_publish_timestamp_seconds = published_at;
            state.metrics.ipfs_pin_lag_seconds = pin_lag;
            if matches!(&self.head_mode, HeadMode::Ipns { .. }) {
                state.metrics.ipns_update_success_total =
                    state.metrics.ipns_update_success_total.saturating_add(1);
                state.metrics.last_ipns_update_timestamp_seconds = published_at;
            }
        }
        self.publish_api_snapshot(&source, &checkpoint, true).await
    }

    async fn verify_steady_state(
        &self,
        source: &SourceSnapshot,
        checkpoint: &CheckpointBodyV1,
    ) -> Result<(), GovernanceDagServiceError> {
        let public = self.fetch_public_head().await?;
        require_public_matches_checkpoint(&public, checkpoint)?;
        if let PublicHead::Present { bytes, .. } = &public {
            validate_remote_head(bytes, source, &self.config)?;
        }
        ipfs_verify_pin(
            &self.ipfs,
            &checkpoint.head_ipfs_cid,
            self.config.max_response_bytes,
        )
        .await?;
        let public_bytes = match public {
            PublicHead::Present { bytes, .. } => bytes,
            PublicHead::Missing => {
                return Err(GovernanceDagServiceError::Conflict(
                    "public head disappeared while verifying the checkpoint".to_owned(),
                ));
            }
        };
        let readback = ipfs_cat(
            &self.ipfs,
            &checkpoint.head_ipfs_cid,
            public_bytes.len() as u64,
            self.config.max_response_bytes,
        )
        .await?;
        if readback != public_bytes {
            return Err(GovernanceDagServiceError::State(
                "checkpoint head IPFS readback drifted".to_owned(),
            ));
        }
        verify_or_recover_mirror_file(&self.config, checkpoint, source)
    }

    async fn publish_api_snapshot(
        &self,
        source: &SourceSnapshot,
        checkpoint: &CheckpointBodyV1,
        just_published: bool,
    ) -> Result<(), GovernanceDagServiceError> {
        self.config.revalidate_state_root()?;
        let bytes = read_rooted_file(
            &self.config.state_root_guard,
            Path::new(MIRROR_INDEX_FILE),
            MUTABLE_STATE_MAX_BYTES,
            true,
        )?
        .into_bytes();
        self.config.revalidate_state_root()?;
        let mirror: JsonValue = json::from_slice(&bytes).map_err(|err| {
            GovernanceDagServiceError::State(format!("mirror JSON decode failed: {err}"))
        })?;
        let mut state = self.api.0.write().await;
        state.live = true;
        state.ready = true;
        state.last_error = None;
        state.mirror = Some(mirror);
        state.checkpoint = Some(checkpoint.clone());
        state.metrics.backlog = source
            .head
            .block_count
            .saturating_sub(checkpoint.block_count);
        state.metrics.head_age_seconds =
            current_unix_timestamp_seconds().saturating_sub(source.head.generated_at);
        state.metrics.mirror_drift = 0;
        if !just_published && state.metrics.last_publish_timestamp_seconds == 0 {
            state.metrics.last_publish_timestamp_seconds = checkpoint.published_at_unix;
        }
        Ok(())
    }
}

fn validate_checkpoint_against_source(
    checkpoint: Option<&CheckpointBodyV1>,
    source: &SourceSnapshot,
) -> Result<(), GovernanceDagServiceError> {
    let Some(checkpoint) = checkpoint else {
        return Ok(());
    };
    let source_block_count = u64::try_from(source.blocks.len()).map_err(|_| {
        GovernanceDagServiceError::State("source block count exceeds u64".to_owned())
    })?;
    if checkpoint.block_count > source_block_count {
        return Err(GovernanceDagServiceError::Conflict(
            "source chain rolled back behind the authenticated checkpoint".to_owned(),
        ));
    }
    if checkpoint.block_count == source_block_count
        && checkpoint.source_index_blake3 != source.index_blake3
    {
        return Err(GovernanceDagServiceError::Conflict(
            "authenticated checkpoint source-index digest does not match the verified source"
                .to_owned(),
        ));
    }
    let position = usize::try_from(checkpoint.block_count - 1).map_err(|_| {
        GovernanceDagServiceError::State("checkpoint count exceeds host limits".to_owned())
    })?;
    if source.blocks[position].block.block_cid != checkpoint.head_block_cid {
        return Err(GovernanceDagServiceError::Conflict(
            "source chain forked from the authenticated checkpoint".to_owned(),
        ));
    }
    for published in &checkpoint.mirror_blocks {
        let position = usize::try_from(published.sequence).map_err(|_| {
            GovernanceDagServiceError::State("mirror sequence exceeds host limits".to_owned())
        })?;
        let source_block = source.blocks.get(position).ok_or_else(|| {
            GovernanceDagServiceError::Conflict(
                "checkpoint mirror points outside the source chain".to_owned(),
            )
        })?;
        let source_encoded_len = u64::try_from(source_block.bytes.len()).map_err(|_| {
            GovernanceDagServiceError::State("source block length exceeds u64".to_owned())
        })?;
        if source_block.block.block_cid != published.governance_block_cid
            || source_block.block.node.node_cid != published.governance_node_cid
            || source_block.payload_kind != published.payload_kind
            || source_block.encoded_blake3 != published.encoded_blake3
            || source_encoded_len != published.encoded_len
        {
            return Err(GovernanceDagServiceError::Conflict(
                "checkpoint mirror no longer matches the verified source chain".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_intent_against_source(
    intent: &PublishIntentBodyV1,
    checkpoint: Option<&CheckpointBodyV1>,
    source: &SourceSnapshot,
    config: &RuntimeConfig,
) -> Result<(), GovernanceDagServiceError> {
    validate_publish_intent(intent)?;
    let source_block_count = u64::try_from(source.blocks.len()).map_err(|_| {
        GovernanceDagServiceError::State("source block count exceeds u64".to_owned())
    })?;
    if intent.target_block_count > source_block_count {
        return Err(GovernanceDagServiceError::Conflict(
            "source rolled back behind the durable publish intent".to_owned(),
        ));
    }
    if intent.target_block_count == source_block_count
        && intent.target_source_index_blake3 != source.index_blake3
    {
        return Err(GovernanceDagServiceError::Conflict(
            "durable publish-intent source-index digest does not match the verified source"
                .to_owned(),
        ));
    }
    let target_position = usize::try_from(intent.target_block_count - 1).map_err(|_| {
        GovernanceDagServiceError::State("intent count exceeds host limits".to_owned())
    })?;
    if source.blocks[target_position].block.block_cid != intent.target_head_block_cid
        || blake3_array(&intent.target_head_bytes) != intent.target_head_blake3
    {
        return Err(GovernanceDagServiceError::Conflict(
            "source forked from the durable publish intent".to_owned(),
        ));
    }
    let target_head = validate_remote_head(&intent.target_head_bytes, source, config)?;
    if target_head.block_count != intent.target_block_count
        || target_head.head_block_cid != intent.target_head_block_cid
    {
        return Err(GovernanceDagServiceError::State(
            "durable intent head metadata is inconsistent".to_owned(),
        ));
    }
    let expected_generation = match checkpoint {
        Some(checkpoint) if checkpoint.head_block_cid == intent.target_head_block_cid => {
            checkpoint.generation
        }
        Some(checkpoint) => checkpoint.generation.checked_add(1).ok_or_else(|| {
            GovernanceDagServiceError::State("checkpoint generation exhausted".to_owned())
        })?,
        None => 1,
    };
    if intent.generation != expected_generation {
        return Err(GovernanceDagServiceError::State(
            "publish intent generation is not monotonic".to_owned(),
        ));
    }
    for block in &intent.blocks {
        let position = usize::try_from(block.sequence).map_err(|_| {
            GovernanceDagServiceError::State("intent sequence exceeds host limits".to_owned())
        })?;
        let source_block = source.blocks.get(position).ok_or_else(|| {
            GovernanceDagServiceError::Conflict("intent block is absent from the source".to_owned())
        })?;
        let source_encoded_len = u64::try_from(source_block.bytes.len()).map_err(|_| {
            GovernanceDagServiceError::State("source block length exceeds u64".to_owned())
        })?;
        if source_block.block.block_cid != block.governance_block_cid
            || source_block.block.node.node_cid != block.governance_node_cid
            || source_block.payload_kind != block.payload_kind
            || source_block.encoded_blake3 != block.encoded_blake3
            || source_encoded_len != block.encoded_len
        {
            return Err(GovernanceDagServiceError::Conflict(
                "durable intent block no longer matches source bytes".to_owned(),
            ));
        }
    }
    Ok(())
}

fn require_public_matches_checkpoint(
    public: &PublicHead,
    checkpoint: &CheckpointBodyV1,
) -> Result<(), GovernanceDagServiceError> {
    match public {
        PublicHead::Present { bytes, .. }
            if blake3_array(bytes) == checkpoint.head_bytes_blake3 =>
        {
            Ok(())
        }
        PublicHead::Missing => Err(GovernanceDagServiceError::Conflict(
            "public head disappeared after an authenticated checkpoint".to_owned(),
        )),
        PublicHead::Present { .. } => Err(GovernanceDagServiceError::Conflict(
            "public head diverges from the authenticated checkpoint".to_owned(),
        )),
    }
}

fn merge_published_blocks(
    checkpoint: Option<&CheckpointBodyV1>,
    intent: &PublishIntentBodyV1,
    source: &SourceSnapshot,
    max_entries: usize,
    max_bytes: u64,
) -> Result<Vec<PublishedBlockV1>, GovernanceDagServiceError> {
    let mut by_sequence = BTreeMap::<u64, PublishedBlockV1>::new();
    if let Some(checkpoint) = checkpoint {
        for block in &checkpoint.mirror_blocks {
            by_sequence.insert(block.sequence, block.clone());
        }
    }
    for block in &intent.blocks {
        let ipfs_cid = block.ipfs_cid.clone().ok_or_else(|| {
            GovernanceDagServiceError::State(
                "intent block was not pinned before checkpointing".to_owned(),
            )
        })?;
        by_sequence.insert(
            block.sequence,
            PublishedBlockV1 {
                sequence: block.sequence,
                governance_block_cid: block.governance_block_cid.clone(),
                governance_node_cid: block.governance_node_cid.clone(),
                payload_kind: block.payload_kind.clone(),
                timestamp: block.timestamp,
                encoded_blake3: block.encoded_blake3,
                encoded_len: block.encoded_len,
                ipfs_cid,
            },
        );
    }
    if max_entries == 0 || max_bytes == 0 {
        return Err(GovernanceDagServiceError::State(
            "mirror retention bounds must be non-zero".to_owned(),
        ));
    }
    let mut retained_sequences = Vec::new();
    let mut retained_bytes = 0_u64;
    for source_block in source.blocks.iter().rev() {
        if retained_sequences.len() == max_entries {
            break;
        }
        let encoded_len = u64::try_from(source_block.bytes.len()).map_err(|_| {
            GovernanceDagServiceError::State("source block length exceeds u64".to_owned())
        })?;
        let next = retained_bytes.checked_add(encoded_len).ok_or_else(|| {
            GovernanceDagServiceError::State("mirror byte count overflow".to_owned())
        })?;
        if next > max_bytes {
            if retained_sequences.is_empty() {
                return Err(GovernanceDagServiceError::State(
                    "the head block alone exceeds mirror_max_bytes".to_owned(),
                ));
            }
            break;
        }
        retained_sequences.push(source_block.block.sequence);
        retained_bytes = next;
    }
    retained_sequences.reverse();
    retained_sequences
        .into_iter()
        .map(|sequence| {
            by_sequence.get(&sequence).cloned().ok_or_else(|| {
                GovernanceDagServiceError::State(
                    "retained source suffix has no authenticated IPFS mapping".to_owned(),
                )
            })
        })
        .collect()
}

fn mirror_index_value(
    source: &SourceSnapshot,
    blocks: &[PublishedBlockV1],
    generation: u64,
    head_ipfs_cid: &str,
    public_token: &str,
    published_at: u64,
) -> Result<JsonValue, GovernanceDagServiceError> {
    if blocks.is_empty() {
        return Err(GovernanceDagServiceError::State(
            "mirror index cannot be empty".to_owned(),
        ));
    }
    let mut block_values = Vec::with_capacity(blocks.len());
    let mut by_block_cid = JsonMap::new();
    let mut by_node_cid = JsonMap::new();
    let mut by_digest = JsonMap::new();
    let mut by_kind_positions = BTreeMap::<String, Vec<JsonValue>>::new();
    for (position, block) in blocks.iter().enumerate() {
        let block_cid_hex = hex::encode(&block.governance_block_cid);
        let node_cid_hex = hex::encode(&block.governance_node_cid);
        let digest_hex = hex::encode(block.encoded_blake3);
        by_block_cid.insert(block_cid_hex.clone(), JsonValue::from(position as u64));
        by_node_cid.insert(node_cid_hex.clone(), JsonValue::from(position as u64));
        by_digest.insert(digest_hex.clone(), JsonValue::from(position as u64));
        by_kind_positions
            .entry(block.payload_kind.clone())
            .or_default()
            .push(JsonValue::from(position as u64));
        let mut value = JsonMap::new();
        value.insert("position".into(), JsonValue::from(position as u64));
        value.insert("sequence".into(), JsonValue::from(block.sequence));
        value.insert("timestamp".into(), JsonValue::from(block.timestamp));
        value.insert(
            "payload_kind".into(),
            JsonValue::from(block.payload_kind.clone()),
        );
        value.insert("block_cid_hex".into(), JsonValue::from(block_cid_hex));
        value.insert("node_cid_hex".into(), JsonValue::from(node_cid_hex));
        value.insert("blake3".into(), JsonValue::from(digest_hex));
        value.insert("encoded_len".into(), JsonValue::from(block.encoded_len));
        value.insert("ipfs_cid".into(), JsonValue::from(block.ipfs_cid.clone()));
        block_values.push(JsonValue::Object(value));
    }
    let by_kind = by_kind_positions
        .into_iter()
        .map(|(kind, positions)| (kind, JsonValue::Array(positions)))
        .collect::<JsonMap>();
    let mut head = JsonMap::new();
    head.insert(
        "head_block_cid_hex".into(),
        JsonValue::from(hex::encode(&source.head.head_block_cid)),
    );
    head.insert(
        "block_count".into(),
        JsonValue::from(source.head.block_count),
    );
    head.insert(
        "generated_at".into(),
        JsonValue::from(source.head.generated_at),
    );
    head.insert("ipfs_cid".into(), JsonValue::from(head_ipfs_cid));
    head.insert("public_token".into(), JsonValue::from(public_token));
    head.insert(
        "blake3".into(),
        JsonValue::from(hex::encode(blake3_array(&source.head_bytes))),
    );
    let mut root = JsonMap::new();
    root.insert("schema".into(), JsonValue::from(MIRROR_INDEX_SCHEMA));
    root.insert("generation".into(), JsonValue::from(generation));
    root.insert("generated_at".into(), JsonValue::from(published_at));
    root.insert("head".into(), JsonValue::Object(head));
    root.insert(
        "block_count".into(),
        JsonValue::from(source.head.block_count),
    );
    root.insert(
        "indexed_block_count".into(),
        JsonValue::from(block_values.len() as u64),
    );
    root.insert("blocks".into(), JsonValue::Array(block_values));
    root.insert("by_block_cid_hex".into(), JsonValue::Object(by_block_cid));
    root.insert("by_node_cid_hex".into(), JsonValue::Object(by_node_cid));
    root.insert("by_encoded_blake3".into(), JsonValue::Object(by_digest));
    root.insert("by_payload_kind".into(), JsonValue::Object(by_kind));
    Ok(JsonValue::Object(root))
}

fn verify_mirror_file(
    config: &RuntimeConfig,
    checkpoint: &CheckpointBodyV1,
) -> Result<(), GovernanceDagServiceError> {
    config.revalidate_state_root()?;
    let bytes = read_rooted_file(
        &config.state_root_guard,
        Path::new(MIRROR_INDEX_FILE),
        MUTABLE_STATE_MAX_BYTES,
        true,
    )?
    .into_bytes();
    config.revalidate_state_root()?;
    if blake3_array(&bytes) != checkpoint.mirror_blake3 {
        return Err(GovernanceDagServiceError::State(
            "mirror index digest does not match the authenticated checkpoint".to_owned(),
        ));
    }
    let value: JsonValue = json::from_slice(&bytes).map_err(|err| {
        GovernanceDagServiceError::State(format!("mirror index JSON is invalid: {err}"))
    })?;
    let expected_head_cid = hex::encode(&checkpoint.head_block_cid);
    if value.get("schema").and_then(JsonValue::as_str) != Some(MIRROR_INDEX_SCHEMA)
        || value.get("generation").and_then(JsonValue::as_u64) != Some(checkpoint.generation)
        || value
            .get("head")
            .and_then(|head| head.get("head_block_cid_hex"))
            .and_then(JsonValue::as_str)
            != Some(expected_head_cid.as_str())
    {
        return Err(GovernanceDagServiceError::State(
            "mirror index metadata is inconsistent with the checkpoint".to_owned(),
        ));
    }
    Ok(())
}

fn verify_or_recover_mirror_file(
    config: &RuntimeConfig,
    checkpoint: &CheckpointBodyV1,
    source: &SourceSnapshot,
) -> Result<(), GovernanceDagServiceError> {
    config.revalidate_state_root()?;
    match config
        .state_root_guard
        .rooted_directory()
        .private_file_identity(OsStr::new(MIRROR_INDEX_FILE))
    {
        Ok(Some(_)) => return verify_mirror_file(config, checkpoint),
        Ok(None) => {}
        Err(error) => {
            return Err(GovernanceDagServiceError::Filesystem(format!(
                "cannot inspect rooted mirror index during recovery: {error}"
            )));
        }
    }
    config.revalidate_state_root()?;
    if source.head.head_block_cid != checkpoint.head_block_cid
        || source.head.block_count != checkpoint.block_count
        || blake3_array(&source.head_bytes) != checkpoint.head_bytes_blake3
    {
        return Err(GovernanceDagServiceError::State(
            "missing mirror cannot be rebuilt from a source at a different head".to_owned(),
        ));
    }
    let mirror = mirror_index_value(
        source,
        &checkpoint.mirror_blocks,
        checkpoint.generation,
        &checkpoint.head_ipfs_cid,
        &checkpoint.public_head_token,
        checkpoint.published_at_unix,
    )?;
    let bytes = json::to_json_pretty(&mirror)
        .map_err(|err| {
            GovernanceDagServiceError::State(format!("mirror recovery encode failed: {err}"))
        })?
        .into_bytes();
    if blake3_array(&bytes) != checkpoint.mirror_blake3 {
        return Err(GovernanceDagServiceError::State(
            "deterministic mirror recovery does not match the checkpoint digest".to_owned(),
        ));
    }
    config.revalidate_state_root()?;
    write_rooted_atomic_secret(
        &config.state_root_guard,
        Path::new(MIRROR_INDEX_FILE),
        &bytes,
    )?;
    config.revalidate_state_root()?;
    verify_mirror_file(config, checkpoint)
}

fn service_router(state: ApiState) -> Router {
    Router::new()
        .route("/healthz", get(health_handler))
        .route("/readyz", get(readiness_handler))
        .route("/metrics", get(metrics_handler))
        .route(
            "/v1/sorafs/governance/dag/dashboard",
            get(dashboard_handler),
        )
        .route("/v1/sorafs/governance/dag/head", get(head_handler))
        .route(
            "/v1/sorafs/governance/dag/blocks/{block_cid_hex}",
            get(block_handler),
        )
        .route(
            "/v1/sorafs/governance/dag/nodes/{node_cid_hex}",
            get(node_handler),
        )
        .route(
            "/v1/sorafs/governance/dag/digests/{encoded_blake3_hex}",
            get(digest_handler),
        )
        .route(
            "/v1/sorafs/governance/dag/checkpoint",
            get(checkpoint_handler),
        )
        .with_state(state)
}

async fn health_handler(State(state): State<ApiState>) -> Response {
    let snapshot = state.0.read().await;
    let mut value = JsonMap::new();
    value.insert(
        "schema".into(),
        JsonValue::from("sorafs.governance_dag.health.v1"),
    );
    value.insert("live".into(), JsonValue::from(snapshot.live));
    json_response(
        if snapshot.live {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        JsonValue::Object(value),
        &HeaderMap::new(),
    )
}

async fn readiness_handler(State(state): State<ApiState>) -> Response {
    let snapshot = state.0.read().await;
    let mut value = JsonMap::new();
    value.insert(
        "schema".into(),
        JsonValue::from("sorafs.governance_dag.readiness.v1"),
    );
    value.insert("ready".into(), JsonValue::from(snapshot.ready));
    value.insert(
        "error".into(),
        snapshot
            .last_error
            .as_ref()
            .map_or(JsonValue::Null, |error| JsonValue::from(error.clone())),
    );
    json_response(
        if snapshot.ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        JsonValue::Object(value),
        &HeaderMap::new(),
    )
}

async fn metrics_handler(State(state): State<ApiState>) -> Response {
    let snapshot = state.0.read().await;
    let metrics = snapshot.metrics.clone();
    let mut body = format!(
        "# TYPE sorafs_governance_dag_publish_total counter\n\
sorafs_governance_dag_publish_total{{sink=\"ipfs\",result=\"success\"}} {}\n\
sorafs_governance_dag_publish_total{{sink=\"ipfs\",result=\"failure\"}} {}\n\
# TYPE sorafs_governance_dag_published_bytes_total counter\n\
sorafs_governance_dag_published_bytes_total{{sink=\"ipfs\"}} {}\n\
# TYPE sorafs_governance_dag_last_publish_timestamp_seconds gauge\n\
sorafs_governance_dag_last_publish_timestamp_seconds{{sink=\"public\"}} {}\n\
# TYPE sorafs_governance_dag_backlog gauge\n\
sorafs_governance_dag_backlog{{sink=\"ipfs\"}} {}\n\
# TYPE sorafs_governance_dag_head_age_seconds gauge\n\
sorafs_governance_dag_head_age_seconds{{sink=\"public\"}} {}\n\
# TYPE sorafs_governance_dag_ipfs_pin_lag_seconds gauge\n\
sorafs_governance_dag_ipfs_pin_lag_seconds {}\n\
# TYPE sorafs_governance_dag_ipns_update_total counter\n\
sorafs_governance_dag_ipns_update_total{{result=\"success\"}} {}\n\
sorafs_governance_dag_ipns_update_total{{result=\"failure\"}} {}\n\
# TYPE sorafs_governance_dag_last_ipns_update_timestamp_seconds gauge\n\
sorafs_governance_dag_last_ipns_update_timestamp_seconds {}\n\
# TYPE sorafs_governance_dag_validation_failure_total counter\n\
sorafs_governance_dag_validation_failure_total {}\n\
# TYPE sorafs_governance_dag_mirror_drift gauge\n\
sorafs_governance_dag_mirror_drift {}\n",
        metrics.publish_success_total,
        metrics.publish_failure_total,
        metrics.published_bytes_total,
        metrics.last_publish_timestamp_seconds,
        metrics.backlog,
        metrics.head_age_seconds,
        metrics.ipfs_pin_lag_seconds,
        metrics.ipns_update_success_total,
        metrics.ipns_update_failure_total,
        metrics.last_ipns_update_timestamp_seconds,
        metrics.validation_failure_total,
        metrics.mirror_drift,
    );
    let mut kind_counts = BTreeMap::<String, u64>::new();
    if let Some(blocks) = snapshot
        .mirror
        .as_ref()
        .and_then(|mirror| mirror.get("blocks"))
        .and_then(JsonValue::as_array)
    {
        for block in blocks {
            if let Some(kind) = block.get("payload_kind").and_then(JsonValue::as_str) {
                let count = kind_counts.entry(kind.to_owned()).or_default();
                *count = count.saturating_add(1);
            }
        }
    }
    body.push_str("# TYPE sorafs_governance_dag_blocks gauge\n");
    for (kind, count) in kind_counts {
        body.push_str(&format!(
            "sorafs_governance_dag_blocks{{payload_kind=\"{kind}\"}} {count}\n"
        ));
    }
    drop(snapshot);
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4"),
    );
    response
}

async fn dashboard_handler(State(state): State<ApiState>, headers: HeaderMap) -> Response {
    let snapshot = state.0.read().await;
    let Some(mirror) = &snapshot.mirror else {
        return json_error(StatusCode::SERVICE_UNAVAILABLE, "mirror is not ready");
    };
    let blocks = mirror
        .get("blocks")
        .and_then(JsonValue::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut counts = BTreeMap::<String, u64>::new();
    for block in blocks {
        if let Some(kind) = block.get("payload_kind").and_then(JsonValue::as_str) {
            let count = counts.entry(kind.to_owned()).or_default();
            *count = count.saturating_add(1);
        }
    }
    let counts = counts
        .into_iter()
        .map(|(kind, count)| (kind, JsonValue::from(count)))
        .collect::<JsonMap>();
    let mut value = JsonMap::new();
    value.insert(
        "schema".into(),
        JsonValue::from("sorafs.governance_dag.dashboard.v1"),
    );
    value.insert(
        "head".into(),
        mirror.get("head").cloned().unwrap_or(JsonValue::Null),
    );
    value.insert(
        "block_count".into(),
        mirror
            .get("block_count")
            .cloned()
            .unwrap_or(JsonValue::Null),
    );
    value.insert(
        "indexed_block_count".into(),
        JsonValue::from(blocks.len() as u64),
    );
    value.insert("payload_kind_counts".into(), JsonValue::Object(counts));
    json_response(StatusCode::OK, JsonValue::Object(value), &headers)
}

async fn head_handler(State(state): State<ApiState>, headers: HeaderMap) -> Response {
    let snapshot = state.0.read().await;
    let Some(head) = snapshot
        .mirror
        .as_ref()
        .and_then(|mirror| mirror.get("head"))
        .cloned()
    else {
        return json_error(StatusCode::SERVICE_UNAVAILABLE, "mirror is not ready");
    };
    let mut value = JsonMap::new();
    value.insert(
        "schema".into(),
        JsonValue::from("sorafs.governance_dag.head.v1"),
    );
    value.insert("head".into(), head);
    json_response(StatusCode::OK, JsonValue::Object(value), &headers)
}

async fn block_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    AxumPath(cid): AxumPath<String>,
) -> Response {
    lookup_handler(state, headers, cid, "block_cid_hex", "block").await
}

async fn node_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    AxumPath(cid): AxumPath<String>,
) -> Response {
    lookup_handler(state, headers, cid, "node_cid_hex", "node").await
}

async fn lookup_handler(
    state: ApiState,
    headers: HeaderMap,
    cid: String,
    field: &str,
    query: &str,
) -> Response {
    if !is_canonical_digest_hex(&cid) {
        return json_error(
            StatusCode::BAD_REQUEST,
            "lookup CID must be lowercase 32-byte hex",
        );
    }
    let snapshot = state.0.read().await;
    let block = snapshot
        .mirror
        .as_ref()
        .and_then(|mirror| mirror.get("blocks"))
        .and_then(JsonValue::as_array)
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|block| block.get(field).and_then(JsonValue::as_str) == Some(cid.as_str()))
        })
        .cloned();
    let Some(block) = block else {
        return json_error(StatusCode::NOT_FOUND, "governance DAG lookup was not found");
    };
    let mut value = JsonMap::new();
    value.insert(
        "schema".into(),
        JsonValue::from("sorafs.governance_dag.lookup.v1"),
    );
    value.insert("query".into(), JsonValue::from(query));
    value.insert("cid_hex".into(), JsonValue::from(cid));
    value.insert("found".into(), JsonValue::from(true));
    value.insert("block".into(), block);
    json_response(StatusCode::OK, JsonValue::Object(value), &headers)
}

async fn digest_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    AxumPath(digest): AxumPath<String>,
) -> Response {
    if !is_canonical_digest_hex(&digest) {
        return json_error(
            StatusCode::BAD_REQUEST,
            "encoded digest must be lowercase 32-byte hex",
        );
    }
    let snapshot = state.0.read().await;
    let blocks = snapshot
        .mirror
        .as_ref()
        .and_then(|mirror| mirror.get("blocks"))
        .and_then(JsonValue::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|block| {
                    block.get("blake3").and_then(JsonValue::as_str) == Some(digest.as_str())
                })
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if blocks.is_empty() {
        return json_error(StatusCode::NOT_FOUND, "governance DAG digest was not found");
    }
    let mut value = JsonMap::new();
    value.insert(
        "schema".into(),
        JsonValue::from("sorafs.governance_dag.digest.lookup.v1"),
    );
    value.insert("encoded_blake3_hex".into(), JsonValue::from(digest));
    value.insert("count".into(), JsonValue::from(blocks.len() as u64));
    value.insert("blocks".into(), JsonValue::Array(blocks));
    json_response(StatusCode::OK, JsonValue::Object(value), &headers)
}

async fn checkpoint_handler(State(state): State<ApiState>, headers: HeaderMap) -> Response {
    let snapshot = state.0.read().await;
    let Some(checkpoint) = &snapshot.checkpoint else {
        return json_error(StatusCode::SERVICE_UNAVAILABLE, "checkpoint is not ready");
    };
    let mut value = JsonMap::new();
    value.insert(
        "schema".into(),
        JsonValue::from("sorafs.governance_dag.checkpoint.public.v1"),
    );
    value.insert("generation".into(), JsonValue::from(checkpoint.generation));
    value.insert(
        "head_block_cid_hex".into(),
        JsonValue::from(hex::encode(&checkpoint.head_block_cid)),
    );
    value.insert(
        "block_count".into(),
        JsonValue::from(checkpoint.block_count),
    );
    value.insert(
        "head_ipfs_cid".into(),
        JsonValue::from(checkpoint.head_ipfs_cid.clone()),
    );
    value.insert(
        "head_blake3_hex".into(),
        JsonValue::from(hex::encode(checkpoint.head_bytes_blake3)),
    );
    value.insert(
        "mirror_blake3_hex".into(),
        JsonValue::from(hex::encode(checkpoint.mirror_blake3)),
    );
    value.insert(
        "published_at_unix".into(),
        JsonValue::from(checkpoint.published_at_unix),
    );
    json_response(StatusCode::OK, JsonValue::Object(value), &headers)
}

fn is_canonical_digest_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn json_error(status: StatusCode, message: &str) -> Response {
    let mut value = JsonMap::new();
    value.insert("error".into(), JsonValue::from(message));
    json_response(status, JsonValue::Object(value), &HeaderMap::new())
}

fn json_response(status: StatusCode, value: JsonValue, request_headers: &HeaderMap) -> Response {
    let body = match json::to_json(&value) {
        Ok(body) => body,
        Err(_) => {
            return empty_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let etag = format!("\"{}\"", hex::encode(blake3_array(body.as_bytes())));
    let etag_header = match HeaderValue::from_str(&etag) {
        Ok(value) => value,
        Err(_) => return empty_response(StatusCode::INTERNAL_SERVER_ERROR),
    };
    if request_headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        == Some(etag.as_str())
    {
        let mut response = empty_response(StatusCode::NOT_MODIFIED);
        response.headers_mut().insert(header::ETAG, etag_header);
        return response;
    }
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response.headers_mut().insert(header::ETAG, etag_header);
    response
}

fn empty_response(status: StatusCode) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = status;
    response
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        fmt,
        process::{Child, Command, Stdio},
        sync::{
            Arc, Mutex as StdMutex,
            atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
        },
    };

    use crate::{
        FilesystemGovernancePublisher, GovernanceDagCanonicalRequestHeaderV1,
        GovernanceDagRuntimeSigner, GovernancePublisher,
        governance::{
            qualify_governance_dag_runtime_checkpoint_store,
            qualify_governance_dag_runtime_signer_provider,
        },
    };
    use axum::{
        body::Bytes,
        extract::{RawQuery, State},
        http::{HeaderName, Request},
        response::Redirect,
        routing::{any, post},
    };
    use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature as IrohaSignature};
    use sorafs_manifest::{
        GOVERNANCE_DAG_BLOCK_VERSION_V1, GOVERNANCE_DAG_HEAD_VERSION_V1, GOVERNANCE_LOG_VERSION_V1,
        GovernanceLogNodeV1, GovernanceLogSignatureV1,
        deal::{
            DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
            DealSettlementStatusV1, DealSettlementV1, XorQuantity,
        },
        governance_dag_block_cid_v1,
    };
    use tempfile::TempDir;
    use tokio::{sync::Mutex, task::JoinHandle};
    use tower::ServiceExt as _;

    use super::*;
    #[test]
    fn service_default_request_bound_matches_canonical_governance_block_ceiling() {
        let service = SorafsGovernanceDagService::default();
        assert_eq!(
            service.max_request_bytes.0,
            u64::try_from(GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1)
                .expect("canonical block ceiling fits u64")
        );
        assert!(CANONICAL_DECODE_MAX_TOTAL_ELEMENTS > MAX_REPUTATION_TRUST_EDGES);
    }

    // Keep one target-gated assertion for every ABI branch. Overlapping branches
    // fail with duplicate definitions; missing branches fail to resolve the flag.
    #[cfg(all(
        target_os = "linux",
        any(
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "m68k",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )
    ))]
    #[test]
    fn linux_no_follow_flag_matches_low_flag_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x8000);
    }

    #[cfg(all(
        target_os = "linux",
        not(any(
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "m68k",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        ))
    ))]
    #[test]
    fn linux_no_follow_flag_matches_generic_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x20000);
    }

    #[cfg(all(
        target_os = "android",
        any(target_arch = "aarch64", target_arch = "arm")
    ))]
    #[test]
    fn android_arm_no_follow_flag_matches_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x8000);
    }

    #[cfg(all(
        target_os = "android",
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    #[test]
    fn android_x86_no_follow_flag_matches_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x20000);
    }

    #[cfg(all(target_os = "android", target_arch = "riscv64"))]
    #[test]
    fn android_riscv64_no_follow_flag_matches_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x400000);
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "riscv32", target_arch = "riscv64")
    ))]
    #[test]
    fn linux_riscv_no_follow_flag_remains_generic_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x20000);
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    #[test]
    fn apple_and_bsd_no_follow_flag_matches_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x100);
    }

    const TEST_CID_PAYLOAD: &str = "bafkreibdt5m62vphg7dxcr6pkwwqygydbnwx5z2iu5bgsuxzxbjnlkjv4u";
    const TEST_CID_BLOCK: &str = "bafkreicjnlfibzgy6kp3r2gnqfwdv62i2pyqhfylhixocyambdfgomtn5y";
    const TEST_CID_HEAD: &str = "bafkreie7fzwthi3rp3ucmnj2ibf2iymndlxlnb4226jwxtuo2x2gqfesju";

    fn xor(value: &str) -> XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }
    const TEST_CID_OLD: &str = "bafkreiglubvvonx26z7fjmd3kypk5fbzlz3uyul2pwiquvbwtyjghth32q";
    const TEST_CID_NEW: &str = "bafkreiarkb5a4l26nhk57jakmkq3263o4v7gxtmfyz6jxbbrwnx76ioeg4";
    const TEST_CID_ATTACKER: &str = "bafkreihgjoryus4vrrzlydkccfilursggzbcjbpnol5locdmo2i44qaizq";
    const KUBO_INTEGRATION_ENV: &str = "SORAFS_RUN_KUBO_INTEGRATION";
    const KUBO_BIN_ENV: &str = "SORAFS_KUBO_BIN";
    const KUBO_IPNS_KEY_ALIAS: &str = "sorafs-gdag-integration";
    const TEST_IPFS_AUTH_HANDLE: &str = "vault:governance/ipfs:primary";
    const TEST_HEAD_AUTH_HANDLE: &str = "vault:governance/head:primary";
    const TEST_CHECKPOINT_STORE_HANDLE: &str = "kms:governance/checkpoint:primary";
    const TEST_PRODUCER_SIGNER_HANDLE: &str = "hsm:governance/source-signer:primary";
    const TEST_PRODUCER_PEER_ID: &str = "12D3KooWGovernanceServiceTest";
    const TEST_AUTH_QUALIFICATION: GovernanceDagRuntimeProviderQualificationV1 =
        GovernanceDagRuntimeProviderQualificationV1::new(1, [0x81; 32]);
    const TEST_STORE_QUALIFICATION: GovernanceDagRuntimeProviderQualificationV1 =
        GovernanceDagRuntimeProviderQualificationV1::new(1, [0x82; 32]);
    const TEST_PRODUCER_SIGNER_QUALIFICATION: GovernanceDagRuntimeProviderQualificationV1 =
        GovernanceDagRuntimeProviderQualificationV1::new(1, [0x83; 32]);

    struct TestAuthenticator {
        handle: String,
        private_key: PrivateKey,
        public_key: [u8; 32],
        provider_secret: StdMutex<String>,
        nonce_counter: AtomicU64,
        qualification_revision: AtomicU64,
        qualification_refuse: AtomicBool,
        drift_during_authentication: AtomicBool,
        refuse: AtomicBool,
    }

    impl fmt::Debug for TestAuthenticator {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("TestAuthenticator")
                .field("handle", &self.handle)
                .field("hsm", &"[REDACTED]")
                .finish()
        }
    }

    impl TestAuthenticator {
        fn new(handle: &str, provider_secret: &str) -> Self {
            let (private_key, public_key) = test_request_auth_keypair(handle);
            Self {
                handle: handle.to_owned(),
                private_key,
                public_key,
                provider_secret: StdMutex::new(provider_secret.to_owned()),
                nonce_counter: AtomicU64::new(1),
                qualification_revision: AtomicU64::new(1),
                qualification_refuse: AtomicBool::new(false),
                drift_during_authentication: AtomicBool::new(false),
                refuse: AtomicBool::new(false),
            }
        }

        fn rotate(&self, provider_secret: &str) {
            *self
                .provider_secret
                .lock()
                .expect("lock test provider diagnostic") = provider_secret.to_owned();
        }

        fn signed_envelope(
            &self,
            request: &GovernanceDagCanonicalRequestV1,
        ) -> Result<GovernanceDagRequestAuthenticationEnvelopeV1, String> {
            let now = current_unix_timestamp_seconds();
            let counter = self.nonce_counter.fetch_add(1, AtomicOrdering::SeqCst);
            let mut nonce = blake3_array(self.handle.as_bytes());
            nonce[..8].copy_from_slice(&counter.to_be_bytes());
            let payload = GovernanceDagRequestAuthenticationEnvelopeV1::signing_payload(
                request,
                now,
                now.saturating_add(15),
                nonce,
                self.public_key,
            );
            let signature =
                IrohaSignature::try_new(&self.private_key, &payload).map_err(|_| "signing")?;
            let signature: [u8; 64] = signature
                .payload()
                .try_into()
                .map_err(|_| "signature length")?;
            GovernanceDagRequestAuthenticationEnvelopeV1::try_new(
                request,
                now,
                now.saturating_add(15),
                nonce,
                self.public_key,
                signature,
            )
            .map_err(str::to_owned)
        }
    }

    fn test_request_auth_keypair(handle: &str) -> (PrivateKey, [u8; 32]) {
        let seed = blake3_array(handle.as_bytes());
        let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, &seed)
            .expect("test request-auth Ed25519 seed is valid");
        let keypair = KeyPair::from_private_key(private_key.clone())
            .expect("derive test request-auth keypair");
        let (algorithm, bytes) = keypair
            .public_key()
            .try_to_bytes()
            .expect("encode test request-auth public key");
        assert_eq!(algorithm, Algorithm::Ed25519);
        let public_key = bytes
            .try_into()
            .expect("test Ed25519 public key has 32 bytes");
        (private_key, public_key)
    }

    fn test_request_auth_public_key(handle: &str) -> [u8; 32] {
        test_request_auth_keypair(handle).1
    }

    fn signed_test_request_auth_envelope(
        handle: &str,
        request: &GovernanceDagCanonicalRequestV1,
        issued_at: u64,
        expires_at: u64,
        nonce: [u8; 32],
    ) -> GovernanceDagRequestAuthenticationEnvelopeV1 {
        let (private_key, public_key) = test_request_auth_keypair(handle);
        let payload = GovernanceDagRequestAuthenticationEnvelopeV1::signing_payload(
            request, issued_at, expires_at, nonce, public_key,
        );
        let signature = IrohaSignature::try_new(&private_key, &payload)
            .expect("sign test request-auth payload");
        let signature = signature
            .payload()
            .try_into()
            .expect("test Ed25519 signature has 64 bytes");
        GovernanceDagRequestAuthenticationEnvelopeV1::try_new(
            request, issued_at, expires_at, nonce, public_key, signature,
        )
        .expect("construct test request-auth envelope")
    }

    fn request_auth_header_fields(
        envelope: &GovernanceDagRequestAuthenticationEnvelopeV1,
    ) -> Vec<(String, Vec<u8>)> {
        governance_dag_request_authentication_headers_v1(envelope)
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.into_bytes()))
            .collect()
    }

    fn verify_request_before_test_backend(
        request: &GovernanceDagCanonicalRequestV1,
        headers: &[(String, Vec<u8>)],
        body: &[u8],
        expected_scope: GovernanceDagAuthenticationScope,
        policy: &GovernanceDagRequestAuthenticationPolicyV1,
        now: u64,
        replay_cache: &mut GovernanceDagRequestAuthenticationReplayCacheV1,
        backend_calls: &AtomicU64,
    ) -> Result<(), GovernanceDagRequestAuthenticationErrorV1> {
        let mut receiver = GovernanceDagHttpRequestReceiverV1::try_new(
            expected_scope,
            1024 * 1024,
            policy,
            replay_cache,
        )?;
        let verified_request = receiver.verify_http_request(
            request.method(),
            request.canonical_url(),
            request
                .selected_headers()
                .iter()
                .map(|header| (header.name(), header.value().as_bytes()))
                .chain(
                    headers
                        .iter()
                        .map(|(name, value)| (name.as_str(), value.as_slice())),
                ),
            body,
            now,
        )?;
        if verified_request != *request {
            return Err(GovernanceDagRequestAuthenticationErrorV1::RequestMismatch);
        }
        backend_calls.fetch_add(1, AtomicOrdering::SeqCst);
        Ok(())
    }

    fn canonical_test_request(
        scope: GovernanceDagAuthenticationScope,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> GovernanceDagCanonicalRequestV1 {
        let mut headers = headers
            .iter()
            .map(|(name, value)| {
                GovernanceDagCanonicalRequestHeaderV1::try_new(name, value)
                    .expect("canonical test request header")
            })
            .collect::<Vec<_>>();
        headers.sort_unstable();
        GovernanceDagCanonicalRequestV1::try_new(
            scope,
            method,
            url,
            headers,
            body.len() as u64,
            blake3_array(body),
            1024 * 1024,
        )
        .expect("canonical test request")
    }

    impl GovernanceDagRequestAuthenticator for TestAuthenticator {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
            if self.qualification_refuse.load(AtomicOrdering::SeqCst) {
                return Err("auth_token=must-never-escape".to_owned());
            }
            Ok(GovernanceDagRuntimeProviderQualificationV1::new(
                self.qualification_revision.load(AtomicOrdering::SeqCst),
                [0x81; 32],
            ))
        }

        fn public_key(&self) -> [u8; 32] {
            self.public_key
        }

        fn authenticate(
            &self,
            request: &GovernanceDagCanonicalRequestV1,
        ) -> Result<GovernanceDagRequestAuthenticationEnvelopeV1, String> {
            if self
                .drift_during_authentication
                .swap(false, AtomicOrdering::SeqCst)
            {
                self.qualification_revision
                    .fetch_add(1, AtomicOrdering::SeqCst);
            }
            if self.refuse.load(AtomicOrdering::SeqCst) {
                return Err(format!(
                    "hsm_diagnostic={}",
                    self.provider_secret.lock().map_err(|_| "poisoned")?
                ));
            }
            self.signed_envelope(request)
        }
    }

    struct FinalRequestAuthenticator {
        signer: TestAuthenticator,
        expected_body_length: u64,
        expected_body_blake3: [u8; 32],
        expected_condition: HeaderName,
        expected_condition_value: HeaderValue,
        observed_put: AtomicBool,
    }

    impl fmt::Debug for FinalRequestAuthenticator {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("FinalRequestAuthenticator")
                .field("expected_body", &"[REDACTED]")
                .field("expected_condition", &self.expected_condition)
                .finish_non_exhaustive()
        }
    }

    impl FinalRequestAuthenticator {
        fn new(
            expected_body: &[u8],
            expected_condition: HeaderName,
            expected_condition_value: HeaderValue,
        ) -> Self {
            Self {
                signer: TestAuthenticator::new(TEST_HEAD_AUTH_HANDLE, "final-request-hsm"),
                expected_body_length: expected_body.len() as u64,
                expected_body_blake3: blake3_array(expected_body),
                expected_condition,
                expected_condition_value,
                observed_put: AtomicBool::new(false),
            }
        }
    }

    impl GovernanceDagRequestAuthenticator for FinalRequestAuthenticator {
        fn handle(&self) -> &str {
            TEST_HEAD_AUTH_HANDLE
        }

        fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(TEST_AUTH_QUALIFICATION)
        }

        fn public_key(&self) -> [u8; 32] {
            self.signer.public_key()
        }

        fn authenticate(
            &self,
            request: &GovernanceDagCanonicalRequestV1,
        ) -> Result<GovernanceDagRequestAuthenticationEnvelopeV1, String> {
            if request.method() == Method::PUT.as_str() {
                let expected_condition = self.expected_condition.as_str();
                let expected_condition_value = self
                    .expected_condition_value
                    .to_str()
                    .map_err(|_| "noncanonical expected condition")?;
                let observed = request
                    .selected_headers()
                    .iter()
                    .map(|header| (header.name(), header.value()))
                    .collect::<BTreeMap<_, _>>();
                if request.scope() != GovernanceDagAuthenticationScope::SignedHead
                    || observed.get(header::CONTENT_TYPE.as_str()).copied()
                        != Some("application/vnd.iroha.norito")
                    || observed.get(expected_condition).copied() != Some(expected_condition_value)
                    || request.body_length() != self.expected_body_length
                    || request.body_blake3() != self.expected_body_blake3
                {
                    return Err(
                        "signed-head authenticator received an incomplete PUT request".to_owned(),
                    );
                }
                self.observed_put.store(true, AtomicOrdering::SeqCst);
            }
            self.signer.signed_envelope(request)
        }
    }

    #[derive(Default)]
    struct TestSealedStoreInner {
        checkpoint: Option<GovernanceDagSealedStateRecord>,
        publish_intent: Option<GovernanceDagSealedStateRecord>,
        producer_checkpoint: Option<GovernanceDagSealedStateRecord>,
        producer_publish_intent: Option<GovernanceDagSealedStateRecord>,
        checkpoint_generation_floor: u64,
        intent_generation_floor: u64,
        producer_checkpoint_generation_floor: u64,
        producer_intent_generation_floor: u64,
    }

    struct TestSealedStore {
        handle: String,
        inner: StdMutex<TestSealedStoreInner>,
        qualification_revision: AtomicU64,
        qualification_refuse: AtomicBool,
        drift_during_operation: AtomicBool,
        refuse: AtomicBool,
    }

    impl fmt::Debug for TestSealedStore {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("TestSealedStore")
                .field("handle", &self.handle)
                .finish_non_exhaustive()
        }
    }

    impl TestSealedStore {
        fn new(handle: &str) -> Self {
            Self {
                handle: handle.to_owned(),
                inner: StdMutex::new(TestSealedStoreInner::default()),
                qualification_revision: AtomicU64::new(1),
                qualification_refuse: AtomicBool::new(false),
                drift_during_operation: AtomicBool::new(false),
                refuse: AtomicBool::new(false),
            }
        }

        fn maybe_drift(&self) {
            if self
                .drift_during_operation
                .swap(false, AtomicOrdering::SeqCst)
            {
                self.qualification_revision
                    .fetch_add(1, AtomicOrdering::SeqCst);
            }
        }

        fn slot(
            inner: &TestSealedStoreInner,
            slot: GovernanceDagSealedStateSlot,
        ) -> &Option<GovernanceDagSealedStateRecord> {
            match slot {
                GovernanceDagSealedStateSlot::Checkpoint => &inner.checkpoint,
                GovernanceDagSealedStateSlot::PublishIntent => &inner.publish_intent,
                GovernanceDagSealedStateSlot::ProducerCheckpoint => &inner.producer_checkpoint,
                GovernanceDagSealedStateSlot::ProducerPublishIntent => {
                    &inner.producer_publish_intent
                }
            }
        }

        fn slot_mut(
            inner: &mut TestSealedStoreInner,
            slot: GovernanceDagSealedStateSlot,
        ) -> &mut Option<GovernanceDagSealedStateRecord> {
            match slot {
                GovernanceDagSealedStateSlot::Checkpoint => &mut inner.checkpoint,
                GovernanceDagSealedStateSlot::PublishIntent => &mut inner.publish_intent,
                GovernanceDagSealedStateSlot::ProducerCheckpoint => &mut inner.producer_checkpoint,
                GovernanceDagSealedStateSlot::ProducerPublishIntent => {
                    &mut inner.producer_publish_intent
                }
            }
        }
    }

    impl GovernanceDagSealedCheckpointStore for TestSealedStore {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
            if self.qualification_refuse.load(AtomicOrdering::SeqCst) {
                return Err("kms_access_token=must-never-escape".to_owned());
            }
            Ok(GovernanceDagRuntimeProviderQualificationV1::new(
                self.qualification_revision.load(AtomicOrdering::SeqCst),
                [0x82; 32],
            ))
        }

        fn load(
            &self,
            slot: GovernanceDagSealedStateSlot,
        ) -> Result<Option<GovernanceDagSealedStateRecord>, String> {
            self.maybe_drift();
            if self.refuse.load(AtomicOrdering::SeqCst) {
                return Err("kms_access_token=must-never-escape".to_owned());
            }
            let inner = self.inner.lock().map_err(|_| "poisoned".to_owned())?;
            Ok(Self::slot(&inner, slot).clone())
        }

        fn compare_and_swap(
            &self,
            slot: GovernanceDagSealedStateSlot,
            expected_revision: Option<[u8; 32]>,
            next: GovernanceDagSealedStateRecord,
        ) -> Result<(), String> {
            self.maybe_drift();
            if self.refuse.load(AtomicOrdering::SeqCst) {
                return Err("kms_access_token=must-never-escape".to_owned());
            }
            if !next.has_valid_revision(slot) || next.generation == 0 {
                return Err("invalid sealed record".to_owned());
            }
            let mut inner = self.inner.lock().map_err(|_| "poisoned".to_owned())?;
            let current_revision = Self::slot(&inner, slot)
                .as_ref()
                .map(|record| record.revision);
            if current_revision != expected_revision {
                return Err("compare-and-swap conflict".to_owned());
            }
            let floor = match slot {
                GovernanceDagSealedStateSlot::Checkpoint => inner.checkpoint_generation_floor,
                GovernanceDagSealedStateSlot::PublishIntent => inner.intent_generation_floor,
                GovernanceDagSealedStateSlot::ProducerCheckpoint => {
                    inner.producer_checkpoint_generation_floor
                }
                GovernanceDagSealedStateSlot::ProducerPublishIntent => {
                    inner.producer_intent_generation_floor
                }
            };
            let generation_valid = match slot {
                GovernanceDagSealedStateSlot::Checkpoint => next.generation > floor,
                GovernanceDagSealedStateSlot::PublishIntent
                    if Self::slot(&inner, slot).is_some() =>
                {
                    next.generation >= floor
                }
                GovernanceDagSealedStateSlot::PublishIntent => next.generation > floor,
                GovernanceDagSealedStateSlot::ProducerCheckpoint
                | GovernanceDagSealedStateSlot::ProducerPublishIntent => next.generation > floor,
            };
            if !generation_valid {
                return Err("monotonic generation rollback".to_owned());
            }
            match slot {
                GovernanceDagSealedStateSlot::Checkpoint => {
                    inner.checkpoint_generation_floor = next.generation;
                }
                GovernanceDagSealedStateSlot::PublishIntent => {
                    inner.intent_generation_floor = next.generation;
                }
                GovernanceDagSealedStateSlot::ProducerCheckpoint => {
                    inner.producer_checkpoint_generation_floor = next.generation;
                }
                GovernanceDagSealedStateSlot::ProducerPublishIntent => {
                    inner.producer_intent_generation_floor = next.generation;
                }
            }
            *Self::slot_mut(&mut inner, slot) = Some(next);
            Ok(())
        }

        fn delete(
            &self,
            slot: GovernanceDagSealedStateSlot,
            expected_revision: [u8; 32],
        ) -> Result<(), String> {
            self.maybe_drift();
            if self.refuse.load(AtomicOrdering::SeqCst) {
                return Err("kms_access_token=must-never-escape".to_owned());
            }
            let mut inner = self.inner.lock().map_err(|_| "poisoned".to_owned())?;
            let current_revision = Self::slot(&inner, slot)
                .as_ref()
                .map(|record| record.revision);
            if current_revision != Some(expected_revision) {
                return Err("compare-and-swap conflict".to_owned());
            }
            *Self::slot_mut(&mut inner, slot) = None;
            Ok(())
        }
    }

    fn test_authenticator(handle: &str) -> OpaqueAuthenticator {
        let provider = Arc::new(TestAuthenticator::new(handle, "test-only-hsm"));
        OpaqueAuthenticator::try_new(
            handle,
            TEST_AUTH_QUALIFICATION,
            provider.public_key(),
            30,
            5,
            provider,
            "test authenticator",
        )
        .expect("bind test authenticator")
    }

    fn test_runtime_providers(
        checkpoint_store: Arc<TestSealedStore>,
    ) -> GovernanceDagServiceRuntimeProviders {
        GovernanceDagServiceRuntimeProviders {
            ipfs_authenticator: Some(Arc::new(TestAuthenticator::new(
                TEST_IPFS_AUTH_HANDLE,
                "test-only-ipfs-bearer",
            ))),
            head_authenticator: Some(Arc::new(TestAuthenticator::new(
                TEST_HEAD_AUTH_HANDLE,
                "test-only-head-bearer",
            ))),
            checkpoint_store: Some(checkpoint_store),
        }
    }

    struct TestRuntimeProviderRegistry {
        providers: GovernanceDagServiceRuntimeProviders,
        failure: Option<GovernanceDagServiceRuntimeProviderRegistryErrorV1>,
        observed_bindings: StdMutex<Option<GovernanceDagServiceRuntimeProviderBindingsV1>>,
    }

    impl TestRuntimeProviderRegistry {
        fn returning(providers: GovernanceDagServiceRuntimeProviders) -> Self {
            Self {
                providers,
                failure: None,
                observed_bindings: StdMutex::new(None),
            }
        }

        fn failing(failure: GovernanceDagServiceRuntimeProviderRegistryErrorV1) -> Self {
            Self {
                providers: GovernanceDagServiceRuntimeProviders::default(),
                failure: Some(failure),
                observed_bindings: StdMutex::new(None),
            }
        }
    }

    impl GovernanceDagServiceRuntimeProviderRegistryV1 for TestRuntimeProviderRegistry {
        fn resolve(
            &self,
            bindings: &GovernanceDagServiceRuntimeProviderBindingsV1,
        ) -> Result<
            GovernanceDagServiceRuntimeProviders,
            GovernanceDagServiceRuntimeProviderRegistryErrorV1,
        > {
            *self
                .observed_bindings
                .lock()
                .expect("lock observed registry bindings") = Some(bindings.clone());
            if let Some(failure) = self.failure {
                return Err(failure);
            }
            Ok(self.providers.clone())
        }
    }

    fn test_checkpoint_store(provider: Arc<TestSealedStore>) -> OpaqueCheckpointStore {
        OpaqueCheckpointStore::try_new(
            TEST_CHECKPOINT_STORE_HANDLE,
            TEST_STORE_QUALIFICATION,
            provider,
        )
        .expect("bind test sealed checkpoint store")
    }

    fn runtime_boundary_view(root: &Path) -> SorafsGovernanceDagServiceView {
        let source_dir = root.join("source");
        let state_dir = root.join("state");
        fs::create_dir_all(&source_dir).expect("create test source directory");
        let publisher_public_key_hex = hex::encode(
            ed25519_dalek::SigningKey::from_bytes(&[0x42; 32])
                .verifying_key()
                .to_bytes(),
        );
        SorafsGovernanceDagServiceView {
            source_dir: Some(source_dir),
            producer_publisher_peer_id: Some(TEST_PRODUCER_PEER_ID.to_owned()),
            producer_signer_handle: Some(TEST_PRODUCER_SIGNER_HANDLE.to_owned()),
            producer_signer_revision: Some(TEST_PRODUCER_SIGNER_QUALIFICATION.revision),
            producer_signer_policy_digest: Some(TEST_PRODUCER_SIGNER_QUALIFICATION.policy_digest),
            producer_publisher_public_key_hex: Some(publisher_public_key_hex.clone()),
            service: SorafsGovernanceDagService {
                enabled: true,
                state_dir: Some(state_dir),
                ipfs_api_url: Some("http://127.0.0.1:5001".to_owned()),
                signed_head_url: Some("http://127.0.0.1:9099/head".to_owned()),
                ipfs_authenticator_handle: Some(TEST_IPFS_AUTH_HANDLE.to_owned()),
                ipfs_authenticator_revision: Some(TEST_AUTH_QUALIFICATION.revision),
                ipfs_authenticator_policy_digest: Some(TEST_AUTH_QUALIFICATION.policy_digest),
                ipfs_request_auth_public_key: Some(test_request_auth_public_key(
                    TEST_IPFS_AUTH_HANDLE,
                )),
                head_authenticator_handle: Some(TEST_HEAD_AUTH_HANDLE.to_owned()),
                head_authenticator_revision: Some(TEST_AUTH_QUALIFICATION.revision),
                head_authenticator_policy_digest: Some(TEST_AUTH_QUALIFICATION.policy_digest),
                head_request_auth_public_key: Some(test_request_auth_public_key(
                    TEST_HEAD_AUTH_HANDLE,
                )),
                checkpoint_store_handle: Some(TEST_CHECKPOINT_STORE_HANDLE.to_owned()),
                checkpoint_store_revision: Some(TEST_STORE_QUALIFICATION.revision),
                checkpoint_store_policy_digest: Some(TEST_STORE_QUALIFICATION.policy_digest),
                publisher_public_key_hex: Some(publisher_public_key_hex),
                allow_insecure_http: true,
                allow_private_ipfs_endpoint: true,
                allow_private_head_endpoint: true,
                listen_addr: "127.0.0.1:0".to_owned(),
                ..SorafsGovernanceDagService::default()
            },
        }
    }

    #[test]
    fn configured_publisher_key_requires_one_canonical_strong_ed25519_point() {
        let valid = ed25519_dalek::SigningKey::from_bytes(&[0x42; 32])
            .verifying_key()
            .to_bytes();
        assert_eq!(
            decode_strong_ed25519_public_key_hex(&hex::encode(valid), "publisher key")
                .expect("strong canonical key"),
            valid
        );

        let identity = {
            let mut encoded = [0_u8; 32];
            encoded[0] = 1;
            encoded
        };
        assert!(matches!(
            decode_strong_ed25519_public_key_hex(&hex::encode(identity), "publisher key"),
            Err(GovernanceDagServiceError::Config(message))
                if message.contains("canonical strong Ed25519")
        ));

        let mut noncanonical = [0xff_u8; 32];
        noncanonical[0] = 0xed;
        noncanonical[31] = 0x7f;
        assert!(
            decode_strong_ed25519_public_key_hex(&hex::encode(noncanonical), "publisher key")
                .is_err()
        );
        assert!(
            decode_strong_ed25519_public_key_hex(&"11".repeat(32), "publisher key").is_err(),
            "mixed-torsion Ed25519 encodings must fail the production subgroup check"
        );
    }

    struct KuboHarness {
        _root: TempDir,
        repo: PathBuf,
        binary: PathBuf,
        api_url: String,
        daemon_log: PathBuf,
        child: Option<Child>,
    }

    impl KuboHarness {
        async fn start() -> Self {
            assert_eq!(
                std::env::var(KUBO_INTEGRATION_ENV).as_deref(),
                Ok("1"),
                "set {KUBO_INTEGRATION_ENV}=1 to run the isolated Kubo integration lane"
            );
            let binary = std::env::var_os(KUBO_BIN_ENV)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("ipfs"));
            let root = secure_temp_dir();
            let repo = root.path().join("ipfs-repo");
            fs::create_dir(&repo).expect("create isolated Kubo repository");
            #[cfg(unix)]
            fs::set_permissions(&repo, fs::Permissions::from_mode(0o700))
                .expect("secure isolated Kubo repository");

            Self::run_command(
                &binary,
                &repo,
                &[
                    "init",
                    "--empty-repo",
                    "--profile=test,autoconf-off,announce-off",
                ],
            );
            Self::run_command(
                &binary,
                &repo,
                &["config", "Addresses.API", "/ip4/127.0.0.1/tcp/0"],
            );
            Self::run_command(
                &binary,
                &repo,
                &["config", "Addresses.Gateway", "/ip4/127.0.0.1/tcp/0"],
            );
            Self::run_command(
                &binary,
                &repo,
                &[
                    "config",
                    "--json",
                    "Addresses.Swarm",
                    r#"["/ip4/127.0.0.1/tcp/0"]"#,
                ],
            );
            Self::run_command(
                &binary,
                &repo,
                &["config", "--bool", "Discovery.MDNS.Enabled", "false"],
            );
            Self::assert_network_isolation(&binary, &repo);

            let daemon_log = root.path().join("kubo-daemon.log");
            let stdout = File::create(&daemon_log).expect("create Kubo daemon log");
            let stderr = stdout.try_clone().expect("clone Kubo daemon log handle");
            let child = Command::new(&binary)
                .arg("daemon")
                .env("IPFS_PATH", &repo)
                .env("IPFS_TELEMETRY", "off")
                .stdin(Stdio::null())
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr))
                .spawn()
                .unwrap_or_else(|err| panic!("start isolated Kubo daemon: {err}"));
            let mut harness = Self {
                _root: root,
                repo,
                binary,
                api_url: String::new(),
                daemon_log,
                child: Some(child),
            };
            harness.api_url = harness.wait_for_api().await;
            harness.wait_until_ready().await;
            harness
        }

        fn run_command(binary: &Path, repo: &Path, args: &[&str]) -> Vec<u8> {
            let output = Command::new(binary)
                .args(args)
                .env("IPFS_PATH", repo)
                .env("IPFS_TELEMETRY", "off")
                .stdin(Stdio::null())
                .output()
                .unwrap_or_else(|err| panic!("run isolated Kubo command `{args:?}`: {err}"));
            assert!(
                output.status.success(),
                "isolated Kubo command `{args:?}` failed with {}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            output.stdout
        }

        fn assert_network_isolation(binary: &Path, repo: &Path) {
            let bytes = Self::run_command(binary, repo, &["config", "show"]);
            let config: JsonValue =
                json::from_slice(&bytes).expect("isolated Kubo config must be JSON");
            let null_or_empty = |value: Option<&JsonValue>| {
                value.is_none_or(|value| {
                    value.is_null() || value.as_array().is_some_and(Vec::is_empty)
                })
            };
            assert_eq!(
                config
                    .get("AutoConf")
                    .and_then(|value| value.get("Enabled"))
                    .and_then(JsonValue::as_bool),
                Some(false),
                "isolated Kubo must disable remote AutoConf"
            );
            assert!(
                null_or_empty(config.get("Bootstrap")),
                "isolated Kubo must have no bootstrap peers"
            );
            assert!(null_or_empty(
                config.get("DNS").and_then(|value| value.get("Resolvers"))
            ));
            assert!(null_or_empty(
                config
                    .get("Ipns")
                    .and_then(|value| value.get("DelegatedPublishers"))
            ));
            assert!(null_or_empty(
                config
                    .get("Routing")
                    .and_then(|value| value.get("DelegatedRouters"))
            ));
            assert_eq!(
                config
                    .get("Provide")
                    .and_then(|value| value.get("Enabled"))
                    .and_then(JsonValue::as_bool),
                Some(false),
                "isolated Kubo must disable content announcements"
            );
            let addresses = config
                .get("Addresses")
                .expect("isolated Kubo config has Addresses");
            for field in ["API", "Gateway"] {
                assert_eq!(
                    addresses.get(field).and_then(JsonValue::as_str),
                    Some("/ip4/127.0.0.1/tcp/0"),
                    "isolated Kubo {field} listener must be loopback-only"
                );
            }
            assert_eq!(
                addresses
                    .get("Swarm")
                    .and_then(JsonValue::as_array)
                    .and_then(|values| values.first())
                    .and_then(JsonValue::as_str),
                Some("/ip4/127.0.0.1/tcp/0"),
                "isolated Kubo swarm listener must be loopback-only"
            );
            assert_eq!(
                addresses
                    .get("Swarm")
                    .and_then(JsonValue::as_array)
                    .map(Vec::len),
                Some(1),
                "isolated Kubo must expose only one loopback swarm listener"
            );
        }

        async fn wait_for_api(&mut self) -> String {
            let api_path = self.repo.join("api");
            let deadline = time::Instant::now() + Duration::from_secs(20);
            loop {
                if let Ok(raw) = fs::read_to_string(&api_path) {
                    let raw = raw.trim();
                    let components = raw.split('/').collect::<Vec<_>>();
                    if components.len() == 5
                        && components[1] == "ip4"
                        && components[2] == "127.0.0.1"
                        && components[3] == "tcp"
                        && components[4].parse::<u16>().is_ok_and(|port| port != 0)
                    {
                        return format!("http://127.0.0.1:{}/", components[4]);
                    }
                    panic!("Kubo published a non-loopback or malformed API address: {raw}");
                }
                if let Some(status) = self
                    .child
                    .as_mut()
                    .expect("Kubo child exists while starting")
                    .try_wait()
                    .expect("inspect Kubo daemon status")
                {
                    panic!(
                        "isolated Kubo daemon exited early with {status}\n{}",
                        self.log_text()
                    );
                }
                assert!(
                    time::Instant::now() < deadline,
                    "timed out waiting for isolated Kubo API\n{}",
                    self.log_text()
                );
                time::sleep(Duration::from_millis(25)).await;
            }
        }

        async fn wait_until_ready(&self) {
            let endpoint = self.endpoint();
            let url = endpoint
                .ipfs_url("api/v0/version", &[])
                .expect("construct Kubo version URL");
            let deadline = time::Instant::now() + Duration::from_secs(20);
            loop {
                let request = endpoint
                    .request(Method::POST, url.clone())
                    .expect("construct Kubo readiness request");
                if let Ok(response) = endpoint
                    .execute(request, "Kubo readiness request failed")
                    .await
                    && response.status().is_success()
                {
                    let body = read_bounded_response(response, 64 * 1024)
                        .await
                        .expect("read Kubo version response");
                    let value: JsonValue =
                        json::from_slice(&body).expect("Kubo version response must be JSON");
                    let version = value
                        .get("Version")
                        .and_then(JsonValue::as_str)
                        .expect("Kubo version response has Version");
                    eprintln!("isolated Kubo {version} ready at {}", self.api_url);
                    return;
                }
                assert!(
                    time::Instant::now() < deadline,
                    "timed out waiting for isolated Kubo readiness\n{}",
                    self.log_text()
                );
                time::sleep(Duration::from_millis(25)).await;
            }
        }

        fn endpoint(&self) -> PinnedEndpoint {
            PinnedEndpoint {
                url: Url::parse(&self.api_url).expect("parse isolated Kubo API URL"),
                client: Client::builder()
                    .no_proxy()
                    .redirect(Policy::none())
                    .connect_timeout(Duration::from_secs(5))
                    .timeout(Duration::from_secs(20))
                    .build()
                    .expect("construct isolated Kubo HTTP client"),
                authentication_scope: GovernanceDagAuthenticationScope::Ipfs,
                authenticator: test_authenticator(TEST_IPFS_AUTH_HANDLE),
                max_request_bytes: GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1 as u64,
            }
        }

        fn log_text(&self) -> String {
            fs::read_to_string(&self.daemon_log)
                .unwrap_or_else(|err| format!("cannot read Kubo daemon log: {err}"))
        }

        fn stop_child(&mut self) {
            let Some(mut child) = self.child.take() else {
                return;
            };
            let _ = Command::new(&self.binary)
                .arg("shutdown")
                .env("IPFS_PATH", &self.repo)
                .env("IPFS_TELEMETRY", "off")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => return,
                    Ok(None) if std::time::Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Ok(None) | Err(_) => {
                        // This fallback can only target the exact child spawned above.
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                }
            }
        }

        fn shutdown(mut self) {
            self.stop_child();
        }
    }

    impl Drop for KuboHarness {
        fn drop(&mut self) {
            self.stop_child();
        }
    }

    struct TestSigner {
        private_key: PrivateKey,
        public_key: [u8; 32],
    }

    impl TestSigner {
        fn new(seed: u8) -> Self {
            let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, &[seed; 32])
                .expect("test Ed25519 seed is valid");
            let keypair = KeyPair::from_private_key(private_key.clone())
                .expect("derive test Ed25519 keypair");
            let (algorithm, bytes) = keypair
                .public_key()
                .try_to_bytes()
                .expect("encode test public key");
            assert_eq!(algorithm, Algorithm::Ed25519);
            let mut public_key = [0_u8; 32];
            public_key.copy_from_slice(bytes);
            Self {
                private_key,
                public_key,
            }
        }

        fn sign(&self, payload: &[u8]) -> GovernanceLogSignatureV1 {
            let signature = IrohaSignature::try_new(&self.private_key, payload)
                .expect("sign test governance payload");
            GovernanceLogSignatureV1 {
                algorithm: GovernanceSignatureAlgorithm::Ed25519,
                public_key: self.public_key.to_vec(),
                signature: signature.payload().to_vec(),
            }
        }
    }

    struct PublisherTestSigner {
        handle: String,
        peer_id: Vec<u8>,
        signer: TestSigner,
    }

    impl fmt::Debug for PublisherTestSigner {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("PublisherTestSigner")
                .field("handle", &self.handle)
                .field("peer_id", &self.peer_id)
                .finish_non_exhaustive()
        }
    }

    impl GovernanceDagRuntimeSigner for PublisherTestSigner {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(GovernanceDagRuntimeProviderQualificationV1::new(
                1, [0x83; 32],
            ))
        }

        fn publisher_peer_id(&self) -> &[u8] {
            &self.peer_id
        }

        fn public_key(&self) -> [u8; 32] {
            self.signer.public_key
        }

        fn sign(&self, payload: &[u8]) -> Result<[u8; 64], String> {
            self.signer
                .sign(payload)
                .signature
                .try_into()
                .map_err(|_| "test signature length".to_owned())
        }
    }

    fn empty_signature() -> GovernanceLogSignatureV1 {
        GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn settlement(sequence: u64, timestamp: u64) -> DealSettlementV1 {
        let mut deal_id = [0x11; 32];
        deal_id[..8].copy_from_slice(&sequence.saturating_add(1).to_le_bytes());
        let settled_at = timestamp.saturating_sub(1);
        let mut ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            snapshot_id: [0; 32],
            sequence: 1,
            previous_snapshot_id: None,
            deal_id,
            terms_digest: [0x44; 32],
            provider_id: [0x22; 32],
            client_id: [0x33; 32],
            deal_start_epoch: settled_at.saturating_sub(2),
            deal_end_epoch: settled_at.saturating_sub(1),
            settlement_window_epochs: 2,
            window_start_epoch: settled_at.saturating_sub(2),
            window_end_epoch: settled_at,
            provider_accrual: xor("0.00000001"),
            client_liability: xor("0.00000001"),
            micropayment_credit_generated: XorQuantity::zero(),
            micropayment_credit_applied: XorQuantity::zero(),
            micropayment_credit_carry: XorQuantity::zero(),
            client_debit: xor("0.00000001"),
            outstanding_liability: XorQuantity::zero(),
            bond_total: xor("0.00000002"),
            bond_locked: XorQuantity::zero(),
            bond_slashed: XorQuantity::zero(),
            bond_released: xor("0.00000002"),
            window_expected_charge: xor("0.00000001"),
            window_micropayment_generated: XorQuantity::zero(),
            window_micropayment_applied: XorQuantity::zero(),
            window_client_debit: xor("0.00000001"),
            window_bond_slashed: XorQuantity::zero(),
            window_bond_released: xor("0.00000002"),
            captured_at: settled_at,
        };
        ledger.snapshot_id = ledger.derive_snapshot_id().expect("ledger id");
        let mut settlement = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            settlement_id: [0; 32],
            deal_id,
            ledger,
            status: DealSettlementStatusV1::Completed,
            settled_at,
            audit_notes: None,
        };
        settlement.settlement_id = settlement.derive_settlement_id().expect("settlement id");
        settlement
    }

    fn signed_source(count: usize, seed: u8, first_timestamp: u64) -> SourceSnapshot {
        let signer = TestSigner::new(seed);
        let peer_id = TEST_PRODUCER_PEER_ID.as_bytes().to_vec();
        let mut previous_node_cid = None;
        let mut previous_block_cid = None;
        let mut source_blocks = Vec::new();
        let mut decoded_blocks = Vec::new();
        for sequence in 0..count as u64 {
            let timestamp = first_timestamp.saturating_add(sequence);
            let mut node = GovernanceLogNodeV1 {
                version: GOVERNANCE_LOG_VERSION_V1,
                node_cid: Vec::new(),
                prev_cid: previous_node_cid.clone(),
                timestamp,
                publisher_peer_id: peer_id.clone(),
                payload: GovernanceLogPayloadV1::DealSettlement(Box::new(settlement(
                    sequence, timestamp,
                ))),
                publisher_signature: empty_signature(),
            };
            node.node_cid = node.recompute_node_cid().expect("derive test node CID");
            node.publisher_signature = signer.sign(
                &node
                    .signature_payload_bytes()
                    .expect("encode test node signing payload"),
            );
            let block_cid = governance_dag_block_cid_v1(
                previous_block_cid.as_deref(),
                sequence,
                timestamp,
                &peer_id,
                &node,
            )
            .expect("derive test block CID");
            let mut block = GovernanceDagBlockV1 {
                version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
                block_cid,
                prev_block_cid: previous_block_cid.clone(),
                sequence,
                timestamp,
                publisher_peer_id: peer_id.clone(),
                node,
                block_signature: empty_signature(),
            };
            block.block_signature = signer.sign(
                &block
                    .signature_payload_bytes()
                    .expect("encode test block signing payload"),
            );
            block.validate().expect("test block is valid");
            let bytes = norito::to_bytes(&block).expect("encode test block");
            previous_node_cid = Some(block.node.node_cid.clone());
            previous_block_cid = Some(block.block_cid.clone());
            decoded_blocks.push(block.clone());
            source_blocks.push(SourceBlock {
                encoded_blake3: blake3_array(&bytes),
                payload_kind: "deal_settlement".to_owned(),
                block,
                bytes,
            });
        }
        let last = source_blocks.last().expect("test source is non-empty");
        let checkpoint_cid = (count > GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1).then(|| {
            source_blocks[count - GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1]
                .block
                .block_cid
                .clone()
        });
        let mut head = GovernanceDagHeadV1 {
            version: GOVERNANCE_DAG_HEAD_VERSION_V1,
            head_block_cid: last.block.block_cid.clone(),
            block_count: count as u64,
            generated_at: last.block.timestamp,
            publisher_peer_id: peer_id,
            checkpoint_cid,
            head_signature: empty_signature(),
        };
        head.head_signature = signer.sign(
            &head
                .signature_payload_bytes()
                .expect("encode test head signing payload"),
        );
        validate_governance_dag_head_against_chain_v1(&head, &decoded_blocks)
            .expect("test source chain is valid");
        let head_bytes = norito::to_bytes(&head).expect("encode test head");
        SourceSnapshot {
            index_blake3: [0x44; 32],
            head,
            head_bytes,
            blocks: source_blocks,
        }
    }

    fn test_runtime_config(source: &SourceSnapshot, root: &Path) -> RuntimeConfig {
        let mut expected_public_key = [0_u8; 32];
        expected_public_key.copy_from_slice(&source.head.head_signature.public_key);
        let source_dir = root.join("source");
        let state_dir = root.join("state");
        fs::create_dir_all(&source_dir).expect("create test source root");
        fs::create_dir_all(&state_dir).expect("create test state root");
        RuntimeConfig {
            source_root_guard: GovernanceFilesystemRootGuard::capture_source(&source_dir)
                .expect("fence test source root"),
            source_dir,
            state_root_guard: GovernanceFilesystemRootGuard::capture_writer(&state_dir)
                .expect("fence test state root"),
            listen_addr: "127.0.0.1:0".parse().expect("test address"),
            poll_interval: Duration::from_millis(10),
            max_response_bytes: 1024 * 1024,
            max_request_bytes: 1024 * 1024,
            mirror_max_entries: 1024,
            mirror_max_bytes: 1024 * 1024,
            max_head_age_secs: 3600,
            max_future_skew_secs: 60,
            allow_head_bootstrap: true,
            expected_producer_signer_handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
            expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
            expected_publisher_peer_id: source.head.publisher_peer_id.clone(),
            expected_public_key,
        }
    }

    fn checkpoint_from_source(source: &SourceSnapshot) -> CheckpointBodyV1 {
        let mirror_blocks = source
            .blocks
            .iter()
            .map(|block| PublishedBlockV1 {
                sequence: block.block.sequence,
                governance_block_cid: block.block.block_cid.clone(),
                governance_node_cid: block.block.node.node_cid.clone(),
                payload_kind: block.payload_kind.clone(),
                timestamp: block.block.timestamp,
                encoded_blake3: block.encoded_blake3,
                encoded_len: block.bytes.len() as u64,
                ipfs_cid: TEST_CID_BLOCK.to_owned(),
            })
            .collect();
        CheckpointBodyV1 {
            version: CHECKPOINT_VERSION_V1,
            generation: 1,
            head_block_cid: source.head.head_block_cid.clone(),
            block_count: source.head.block_count,
            head_bytes_blake3: blake3_array(&source.head_bytes),
            head_ipfs_cid: TEST_CID_HEAD.to_owned(),
            public_head_token: "public-token".to_owned(),
            source_index_blake3: source.index_blake3,
            mirror_blake3: [0x55; 32],
            published_at_unix: source.head.generated_at,
            mirror_blocks,
        }
    }

    fn intent_from_source(source: &SourceSnapshot) -> PublishIntentBodyV1 {
        PublishIntentBodyV1 {
            version: PUBLISH_INTENT_VERSION_V1,
            generation: 1,
            target_head_block_cid: source.head.head_block_cid.clone(),
            target_block_count: source.head.block_count,
            target_head_bytes: source.head_bytes.clone(),
            target_head_blake3: blake3_array(&source.head_bytes),
            target_source_index_blake3: source.index_blake3,
            previous_public_head_blake3: None,
            created_at_unix: source.head.generated_at,
            blocks: source
                .blocks
                .iter()
                .map(|block| IntentBlockV1 {
                    sequence: block.block.sequence,
                    governance_block_cid: block.block.block_cid.clone(),
                    governance_node_cid: block.block.node.node_cid.clone(),
                    payload_kind: block.payload_kind.clone(),
                    timestamp: block.block.timestamp,
                    encoded_blake3: block.encoded_blake3,
                    encoded_len: block.bytes.len() as u64,
                    ipfs_cid: Some(TEST_CID_BLOCK.to_owned()),
                })
                .collect(),
            head_ipfs_cid: Some(TEST_CID_HEAD.to_owned()),
        }
    }

    fn secure_temp_dir() -> TempDir {
        let temp_root = std::env::temp_dir()
            .canonicalize()
            .expect("resolve the physical temporary-directory root");
        let dir = tempfile::Builder::new()
            .prefix("sorafs-governance-service-")
            .tempdir_in(temp_root)
            .expect("create test directory");
        #[cfg(unix)]
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700))
            .expect("secure test directory");
        dir
    }

    fn write_test_sidecar_file(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create source sidecar parent");
        }
        fs::write(path, bytes).expect("write source sidecar payload");
        fs::write(
            digest_sidecar_path(path),
            format!("{}\n", hex::encode(blake3_array(bytes))),
        )
        .expect("write source sidecar digest");
    }

    fn materialize_source_snapshot(root: &Path, source: &mut SourceSnapshot) {
        fs::create_dir_all(root).expect("create Governance DAG source root");
        let mut entries = Vec::with_capacity(source.blocks.len());
        let mut by_digest = JsonMap::new();
        let mut by_source_payload_digest = BTreeMap::<String, Vec<JsonValue>>::new();
        let mut by_kind = BTreeMap::<String, Vec<JsonValue>>::new();
        for (position, block) in source.blocks.iter().enumerate() {
            let block_cid_hex = hex::encode(&block.block.block_cid);
            let block_path_label = format!(
                "runtime-dag/blocks/{:020}_{block_cid_hex}.to",
                block.block.sequence
            );
            write_test_sidecar_file(&root.join(&block_path_label), &block.bytes);
            let source_payload_bytes = canonical_source_payload_bytes(&block.block.node.payload)
                .expect("encode test source payload");
            let source_payload_path_label =
                format!("source-payloads/{:020}.to", block.block.sequence);
            write_test_sidecar_file(
                &root.join(&source_payload_path_label),
                &source_payload_bytes,
            );
            let source_payload_digest_hex = hex::encode(blake3_array(&source_payload_bytes));

            let digest_hex = hex::encode(block.encoded_blake3);
            let mut entry = JsonMap::new();
            entry.insert("position".into(), JsonValue::from(position as u64));
            entry.insert("sequence".into(), JsonValue::from(block.block.sequence));
            entry.insert("block_path".into(), JsonValue::from(block_path_label));
            entry.insert(
                "encoded_path".into(),
                JsonValue::from(source_payload_path_label),
            );
            entry.insert(
                "json_path".into(),
                JsonValue::from(format!("source-payloads/{:020}.json", block.block.sequence)),
            );
            entry.insert(
                "encoded_len".into(),
                JsonValue::from(block.bytes.len() as u64),
            );
            entry.insert(
                "source_payload_len".into(),
                JsonValue::from(source_payload_bytes.len() as u64),
            );
            entry.insert(
                "source_payload_blake3".into(),
                JsonValue::from(source_payload_digest_hex.clone()),
            );
            entry.insert("block_cid_hex".into(), JsonValue::from(block_cid_hex));
            entry.insert(
                "node_cid_hex".into(),
                JsonValue::from(hex::encode(&block.block.node.node_cid)),
            );
            entry.insert(
                "prev_block_cid_hex".into(),
                block
                    .block
                    .prev_block_cid
                    .as_ref()
                    .map(hex::encode)
                    .map(JsonValue::from)
                    .unwrap_or(JsonValue::Null),
            );
            entry.insert(
                "prev_node_cid_hex".into(),
                block
                    .block
                    .node
                    .prev_cid
                    .as_ref()
                    .map(hex::encode)
                    .map(JsonValue::from)
                    .unwrap_or(JsonValue::Null),
            );
            entry.insert(
                "payload_kind".into(),
                JsonValue::from(block.payload_kind.clone()),
            );
            entry.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            entries.push(JsonValue::Object(entry));
            by_digest.insert(
                digest_hex,
                JsonValue::Array(vec![JsonValue::from(position as u64)]),
            );
            by_source_payload_digest
                .entry(source_payload_digest_hex)
                .or_default()
                .push(JsonValue::from(position as u64));
            by_kind
                .entry(block.payload_kind.clone())
                .or_default()
                .push(JsonValue::from(position as u64));
        }
        write_test_sidecar_file(&root.join("runtime-dag/head.to"), &source.head_bytes);

        let mut index = JsonMap::new();
        index.insert("schema".into(), JsonValue::from(RUNTIME_INDEX_SCHEMA));
        index.insert(
            "publisher_public_key_hex".into(),
            JsonValue::from(hex::encode(&source.head.head_signature.public_key)),
        );
        index.insert(
            "publisher_peer_id_hex".into(),
            JsonValue::from(hex::encode(&source.head.publisher_peer_id)),
        );
        index.insert(
            "head_block_cid_hex".into(),
            JsonValue::from(hex::encode(&source.head.head_block_cid)),
        );
        index.insert(
            "head_generated_at".into(),
            JsonValue::from(source.head.generated_at),
        );
        index.insert("head_path".into(), JsonValue::from("runtime-dag/head.to"));
        index.insert(
            "block_count".into(),
            JsonValue::from(source.head.block_count),
        );
        index.insert("by_encoded_blake3".into(), JsonValue::Object(by_digest));
        index.insert(
            "by_source_payload_blake3".into(),
            JsonValue::Object(
                by_source_payload_digest
                    .into_iter()
                    .map(|(digest, positions)| (digest, JsonValue::Array(positions)))
                    .collect(),
            ),
        );
        index.insert(
            "by_payload_kind".into(),
            JsonValue::Object(
                by_kind
                    .into_iter()
                    .map(|(kind, positions)| (kind, JsonValue::Array(positions)))
                    .collect(),
            ),
        );
        index.insert("blocks".into(), JsonValue::Array(entries));
        let index_bytes = json::to_json_pretty(&JsonValue::Object(index))
            .expect("encode Governance DAG runtime index")
            .into_bytes();
        source.index_blake3 = blake3_array(&index_bytes);
        write_test_sidecar_file(&root.join("runtime-dag-index.json"), &index_bytes);
    }

    fn producer_checkpoint_from_source(
        root: &Path,
        source: &SourceSnapshot,
    ) -> RuntimeDagProducerCheckpointV1 {
        RuntimeDagProducerCheckpointV1 {
            version: GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_VERSION_V1,
            root_digest: runtime_dag_producer_root_digest(root)
                .expect("derive canonical test producer root digest"),
            signer_handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
            signer_revision: TEST_PRODUCER_SIGNER_QUALIFICATION.revision,
            signer_policy_digest: TEST_PRODUCER_SIGNER_QUALIFICATION.policy_digest,
            checkpoint_store_handle: TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
            checkpoint_store_revision: TEST_STORE_QUALIFICATION.revision,
            checkpoint_store_policy_digest: TEST_STORE_QUALIFICATION.policy_digest,
            publisher_peer_id: source.head.publisher_peer_id.clone(),
            publisher_public_key: source
                .head
                .head_signature
                .public_key
                .as_slice()
                .try_into()
                .expect("test source public key is 32 bytes"),
            block_count: source.head.block_count,
            head_block_cid: source
                .head
                .head_block_cid
                .as_slice()
                .try_into()
                .expect("test source head CID is 32 bytes"),
            head_bytes_digest: blake3_array(&source.head_bytes),
            index_bytes_digest: source.index_blake3,
            qualification_transition_generation: 0,
            qualification_transition_digest: [0; 32],
            qualification_archive_generation: 0,
            qualification_archive_digest: [0; 32],
        }
    }

    fn seed_producer_checkpoint(
        provider: &TestSealedStore,
        root: &Path,
        source: &SourceSnapshot,
    ) -> GovernanceDagSealedStateRecord {
        let checkpoint = producer_checkpoint_from_source(root, source);
        let generation = checkpoint
            .block_count
            .checked_add(checkpoint.qualification_transition_generation)
            .and_then(|generation| {
                generation.checked_add(checkpoint.qualification_archive_generation)
            })
            .and_then(|generation| generation.checked_add(1))
            .expect("test producer generation");
        let record = GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
            generation,
            norito::to_bytes(&checkpoint).expect("encode test producer checkpoint"),
        );
        provider
            .compare_and_swap(
                GovernanceDagSealedStateSlot::ProducerCheckpoint,
                None,
                record.clone(),
            )
            .expect("seed test producer checkpoint");
        record
    }

    async fn kubo_key_generate(endpoint: &PinnedEndpoint, alias: &str) -> String {
        let url = endpoint
            .ipfs_url(
                "api/v0/key/gen",
                &[("arg", alias), ("type", "ed25519"), ("ipns-base", "base36")],
            )
            .expect("construct Kubo key generation URL");
        let request = endpoint
            .request(Method::POST, url)
            .expect("construct Kubo key generation request");
        let response = endpoint
            .execute(request, "Kubo key generation request failed")
            .await
            .expect("send Kubo key generation request");
        assert!(response.status().is_success(), "Kubo key generation failed");
        let body = read_bounded_response(response, 64 * 1024)
            .await
            .expect("read Kubo key generation response");
        let value: JsonValue = json::from_slice(&body).expect("Kubo key response must be JSON");
        let name = value
            .get("Name")
            .and_then(JsonValue::as_str)
            .expect("Kubo key response has Name");
        assert_eq!(name, alias);
        validate_public_token(
            value
                .get("Id")
                .and_then(JsonValue::as_str)
                .expect("Kubo key response has Id"),
            "Kubo IPNS key id",
        )
        .expect("Kubo returns a canonical IPNS key id")
    }

    async fn kubo_unpin(endpoint: &PinnedEndpoint, cid: &str) {
        let url = endpoint
            .ipfs_url("api/v0/pin/rm", &[("arg", cid), ("recursive", "true")])
            .expect("construct Kubo unpin URL");
        let request = endpoint
            .request(Method::POST, url)
            .expect("construct Kubo unpin request");
        let response = endpoint
            .execute(request, "Kubo unpin request failed")
            .await
            .expect("send Kubo unpin request");
        assert!(response.status().is_success(), "Kubo unpin failed");
        let _ = read_bounded_response(response, 64 * 1024)
            .await
            .expect("read Kubo unpin response");
    }

    async fn assert_kubo_has_no_swarm_peers(endpoint: &PinnedEndpoint) {
        let url = endpoint
            .ipfs_url("api/v0/swarm/peers", &[])
            .expect("construct Kubo swarm peers URL");
        let request = endpoint
            .request(Method::POST, url)
            .expect("construct Kubo swarm peers request");
        let response = endpoint
            .execute(request, "Kubo swarm-peers request failed")
            .await
            .expect("send Kubo swarm peers request");
        assert!(response.status().is_success());
        let body = read_bounded_response(response, 64 * 1024)
            .await
            .expect("read Kubo swarm peers response");
        let value: JsonValue = json::from_slice(&body).expect("Kubo swarm response must be JSON");
        assert!(
            value
                .get("Peers")
                .is_none_or(|peers| peers.is_null() || peers.as_array().is_some_and(Vec::is_empty)),
            "isolated Kubo must have no swarm peers: {value:?}"
        );
    }

    fn real_kubo_service_view(
        source: &SourceSnapshot,
        source_dir: &Path,
        state_dir: &Path,
        api_url: &str,
        ipns_name: &str,
    ) -> SorafsGovernanceDagServiceView {
        let paths = [source_dir, state_dir];
        assert!(paths.iter().all(|path| {
            let path = path.to_string_lossy();
            !path.contains(['"', '\\', '\n', '\r'])
        }));
        let config = format!(
            r#"[sorafs.storage]
governance_dag_dir = "{}"
governance_dag_publisher_peer_id = "{TEST_PRODUCER_PEER_ID}"
governance_dag_signer_handle = "{TEST_PRODUCER_SIGNER_HANDLE}"
governance_dag_signer_revision = 1
governance_dag_signer_policy_digest_hex = "{}"
governance_dag_publisher_public_key_hex = "{}"

[sorafs.storage.governance_dag_service]
enabled = true
state_dir = "{}"
ipfs_api_url = "{}"
head_mode = "ipns"
ipns_name = "{}"
ipns_key_name = "{}"
ipfs_authenticator_handle = "{TEST_IPFS_AUTH_HANDLE}"
ipfs_authenticator_revision = 1
ipfs_authenticator_policy_digest_hex = "{}"
ipfs_request_auth_public_key_hex = "{}"
checkpoint_store_handle = "{TEST_CHECKPOINT_STORE_HANDLE}"
checkpoint_store_revision = 1
checkpoint_store_policy_digest_hex = "{}"
publisher_public_key_hex = "{}"
poll_interval_secs = 1
connect_timeout_ms = 5000
request_timeout_ms = 20000
dns_timeout_ms = 5000
max_head_age_secs = 3600
max_future_skew_secs = 60
allow_insecure_http = true
allow_private_ipfs_endpoint = true
allow_head_bootstrap = true
listen_addr = "127.0.0.1:0"
"#,
            source_dir.display(),
            "83".repeat(32),
            hex::encode(&source.head.head_signature.public_key),
            state_dir.display(),
            api_url,
            ipns_name,
            KUBO_IPNS_KEY_ALIAS,
            "81".repeat(32),
            hex::encode(test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE)),
            "82".repeat(32),
            hex::encode(&source.head.head_signature.public_key),
        );
        let config_path = state_dir
            .parent()
            .expect("integration state directory has parent")
            .join("governance-dag-service.toml");
        fs::write(&config_path, config).expect("write standalone G-DAG service config");
        load_service_config(&config_path).expect("parse standalone G-DAG service config")
    }

    async fn spawn_router_with_authenticator(
        router: Router,
        path: &str,
        authentication_scope: GovernanceDagAuthenticationScope,
        authenticator: OpaqueAuthenticator,
    ) -> (PinnedEndpoint, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock service");
        let address = listener.local_addr().expect("mock listener address");
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, router.into_make_service()).await;
        });
        let url = Url::parse(&format!("http://{address}{path}")).expect("mock URL");
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .build()
            .expect("mock HTTP client");
        (
            PinnedEndpoint {
                url,
                client,
                authentication_scope,
                authenticator,
                max_request_bytes: GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1 as u64,
            },
            handle,
        )
    }

    async fn spawn_router(router: Router, path: &str) -> (PinnedEndpoint, JoinHandle<()>) {
        spawn_router_with_authenticator(
            router,
            path,
            GovernanceDagAuthenticationScope::Ipfs,
            test_authenticator(TEST_IPFS_AUTH_HANDLE),
        )
        .await
    }

    fn test_response(status: StatusCode, body: impl Into<Body>) -> Response {
        let mut response = Response::new(body.into());
        *response.status_mut() = status;
        response
    }

    #[derive(Clone)]
    struct MockIpfsState {
        add_body: Arc<Vec<u8>>,
        cat_body: Arc<Vec<u8>>,
        pin_present: bool,
    }

    async fn mock_ipfs_add(State(state): State<MockIpfsState>) -> Response {
        test_response(StatusCode::OK, state.add_body.as_ref().clone())
    }

    async fn mock_ipfs_pin_add() -> Response {
        test_response(StatusCode::OK, "{}")
    }

    async fn mock_ipfs_pin_ls(State(state): State<MockIpfsState>) -> Response {
        let body = if state.pin_present {
            format!(r#"{{"Keys":{{"{TEST_CID_PAYLOAD}":{{}}}}}}"#)
        } else {
            r#"{"Keys":{}}"#.to_owned()
        };
        test_response(StatusCode::OK, body)
    }

    async fn mock_ipfs_cat(State(state): State<MockIpfsState>) -> Response {
        test_response(StatusCode::OK, state.cat_body.as_ref().clone())
    }

    fn mock_ipfs_router(state: MockIpfsState) -> Router {
        Router::new()
            .route("/api/v0/add", post(mock_ipfs_add))
            .route("/api/v0/pin/add", post(mock_ipfs_pin_add))
            .route("/api/v0/pin/ls", post(mock_ipfs_pin_ls))
            .route("/api/v0/cat", post(mock_ipfs_cat))
            .with_state(state)
    }

    async fn count_unexpected_publication_io(
        State(request_count): State<Arc<AtomicU64>>,
    ) -> Response {
        request_count.fetch_add(1, AtomicOrdering::SeqCst);
        test_response(StatusCode::INTERNAL_SERVER_ERROR, "unexpected request")
    }

    #[derive(Default)]
    struct SignedHeadInner {
        bytes: Option<Vec<u8>>,
        etag: String,
        put_status: Option<StatusCode>,
        readback_override: Option<Vec<u8>>,
        put_count: u64,
    }

    #[derive(Clone)]
    struct SignedHeadState(Arc<Mutex<SignedHeadInner>>);

    async fn mock_signed_head_get(State(state): State<SignedHeadState>) -> Response {
        let state = state.0.lock().await;
        let Some(bytes) = &state.bytes else {
            return test_response(StatusCode::NOT_FOUND, Body::empty());
        };
        let mut response = test_response(StatusCode::OK, bytes.clone());
        response.headers_mut().insert(
            header::ETAG,
            HeaderValue::from_str(&state.etag).expect("mock ETag"),
        );
        response
    }

    async fn mock_signed_head_put(
        State(state): State<SignedHeadState>,
        _headers: HeaderMap,
        body: Bytes,
    ) -> Response {
        let mut state = state.0.lock().await;
        state.put_count = state.put_count.saturating_add(1);
        if let Some(status) = state.put_status {
            return test_response(status, Body::empty());
        }
        state.bytes = Some(
            state
                .readback_override
                .clone()
                .unwrap_or_else(|| body.to_vec()),
        );
        state.etag = "\"v2\"".to_owned();
        test_response(StatusCode::NO_CONTENT, Body::empty())
    }

    async fn spawn_signed_head(
        inner: SignedHeadInner,
    ) -> (PinnedEndpoint, SignedHeadState, JoinHandle<()>) {
        spawn_signed_head_with_authenticator(inner, test_authenticator(TEST_HEAD_AUTH_HANDLE)).await
    }

    async fn spawn_signed_head_with_authenticator(
        inner: SignedHeadInner,
        authenticator: OpaqueAuthenticator,
    ) -> (PinnedEndpoint, SignedHeadState, JoinHandle<()>) {
        let state = SignedHeadState(Arc::new(Mutex::new(inner)));
        let router = Router::new()
            .route("/head", get(mock_signed_head_get).put(mock_signed_head_put))
            .with_state(state.clone());
        let (endpoint, handle) = spawn_router_with_authenticator(
            router,
            "/head",
            GovernanceDagAuthenticationScope::SignedHead,
            authenticator,
        )
        .await;
        (endpoint, state, handle)
    }

    #[derive(Clone)]
    struct IpnsMockState {
        resolutions: Arc<Mutex<VecDeque<String>>>,
        bodies: Arc<HashMap<String, Vec<u8>>>,
        publish_count: Arc<AtomicU64>,
    }

    fn raw_query_arg(raw: Option<&str>) -> Option<&str> {
        raw?.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == "arg").then_some(value)
        })
    }

    async fn mock_ipns_resolve(
        State(state): State<IpnsMockState>,
        RawQuery(_raw): RawQuery,
    ) -> Response {
        let cid = state.resolutions.lock().await.pop_front();
        match cid {
            Some(cid) => test_response(StatusCode::OK, format!(r#"{{"Path":"/ipfs/{cid}"}}"#)),
            None => test_response(StatusCode::NOT_FOUND, "{}"),
        }
    }

    async fn mock_ipns_publish(State(state): State<IpnsMockState>) -> Response {
        state.publish_count.fetch_add(1, Ordering::SeqCst);
        test_response(StatusCode::OK, "{}")
    }

    async fn mock_ipns_cat(
        State(state): State<IpnsMockState>,
        RawQuery(raw): RawQuery,
    ) -> Response {
        let Some(cid) = raw_query_arg(raw.as_deref()) else {
            return test_response(StatusCode::BAD_REQUEST, Body::empty());
        };
        match state.bodies.get(cid) {
            Some(bytes) => test_response(StatusCode::OK, bytes.clone()),
            None => test_response(StatusCode::NOT_FOUND, Body::empty()),
        }
    }

    fn mock_ipns_router(state: IpnsMockState) -> Router {
        Router::new()
            .route("/api/v0/name/resolve", post(mock_ipns_resolve))
            .route("/api/v0/name/publish", post(mock_ipns_publish))
            .route("/api/v0/cat", post(mock_ipns_cat))
            .with_state(state)
    }

    #[derive(Clone)]
    struct IpnsResolveFailureState {
        status: StatusCode,
        publish_count: Arc<AtomicU64>,
    }

    async fn mock_ipns_resolve_failure(State(state): State<IpnsResolveFailureState>) -> Response {
        test_response(state.status, "{}")
    }

    async fn mock_ipns_publish_after_failure(
        State(state): State<IpnsResolveFailureState>,
    ) -> Response {
        state.publish_count.fetch_add(1, Ordering::SeqCst);
        test_response(StatusCode::OK, "{}")
    }

    fn mock_ipns_resolve_failure_router(state: IpnsResolveFailureState) -> Router {
        Router::new()
            .route("/api/v0/name/resolve", post(mock_ipns_resolve_failure))
            .route(
                "/api/v0/name/publish",
                post(mock_ipns_publish_after_failure),
            )
            .with_state(state)
    }

    async fn response_header_bomb() -> Response {
        let mut response = test_response(StatusCode::OK, "ok");
        for index in 0..=MAX_RESPONSE_HEADERS {
            let name = HeaderName::from_bytes(format!("x-test-{index}").as_bytes())
                .expect("mock header name");
            response
                .headers_mut()
                .insert(name, HeaderValue::from_static("value"));
        }
        response
    }

    async fn response_body_bomb() -> Response {
        test_response(StatusCode::OK, vec![0_u8; 17])
    }

    async fn response_gzip() -> Response {
        let mut response = test_response(StatusCode::OK, "abc");
        response
            .headers_mut()
            .insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        response
    }

    async fn mock_authenticator_drift(State(provider): State<Arc<TestAuthenticator>>) -> Response {
        provider
            .qualification_revision
            .fetch_add(1, AtomicOrdering::SeqCst);
        test_response(StatusCode::OK, "qualified-before-response")
    }

    #[test]
    fn canonical_decode_rejects_trailing_and_compressed_bytes() {
        let source = signed_source(1, 0x31, 1_800_000_000);
        let block = &source.blocks[0];
        let decoded_block: GovernanceDagBlockV1 =
            decode_canonical(&block.bytes, "governance DAG block")
                .expect("a valid signed governance block fits the bounded decoder budget");
        assert_eq!(decoded_block, block.block);
        let checkpoint = checkpoint_from_source(&source);
        let canonical = norito::to_bytes(&checkpoint).expect("encode checkpoint body");
        let decoded: CheckpointBodyV1 =
            decode_canonical(&canonical, "checkpoint").expect("canonical bytes accepted");
        assert_eq!(decoded, checkpoint);

        let mut trailing = canonical.clone();
        trailing.push(0);
        assert!(decode_canonical::<CheckpointBodyV1>(&trailing, "checkpoint").is_err());

        let compressed =
            norito::to_compressed_bytes(&checkpoint, Some(norito::CompressionConfig::default()))
                .expect("compress checkpoint body");
        assert_ne!(compressed, canonical);
        assert!(decode_canonical::<CheckpointBodyV1>(&compressed, "checkpoint").is_err());
    }

    #[test]
    fn bounded_norito_decode_rejects_sequence_allocation_bomb() {
        let encoded = norito::to_bytes(&vec![7_u64; 64]).expect("encode bounded vector");
        let limits = DecodeLimits::new(4, encoded.len(), 8, encoded.len() * 2, 16);
        assert!(norito::decode_from_bytes_with_limits::<Vec<u64>>(&encoded, limits).is_err());
    }

    #[test]
    fn expected_signer_rejects_wrong_key_and_peer() {
        let source = signed_source(1, 0x32, 1_800_000_000);
        let block = &source.blocks[0].block;
        let attacker = TestSigner::new(0x33);
        assert!(
            validate_expected_signer(block, &attacker.public_key, &block.publisher_peer_id,)
                .is_err()
        );
        let mut expected_key = [0_u8; 32];
        expected_key.copy_from_slice(&block.block_signature.public_key);
        assert!(validate_expected_signer(block, &expected_key, b"wrong-peer").is_err());
    }

    #[test]
    fn runtime_handle_uses_central_production_grammar() {
        assert_eq!(
            validate_runtime_handle(
                "kms://governance/checkpoint.primary-v1_slot-a",
                "sealed checkpoint store",
            )
            .expect("canonical production runtime handle"),
            "kms://governance/checkpoint.primary-v1_slot-a"
        );
        for handle in [
            "https://operator:secret@checkpoint",
            "https://checkpoint/path?credential=secret",
            "https://checkpoint/path#fragment",
            "kms://governance/%63heckpoint",
            "kms:\\governance\\checkpoint",
        ] {
            let error = validate_runtime_handle(handle, "sealed checkpoint store")
                .expect_err("forbidden runtime-handle character must fail closed");
            assert!(error.to_string().contains("canonical credential-free"));
        }
        let error =
            validate_runtime_handle("kms:governance/checkpoint:dummy", "sealed checkpoint store")
                .expect_err("dummy-marked provider handle must fail closed");
        assert!(error.to_string().contains("test-marked"));
    }

    #[test]
    fn service_validates_full_history_and_canonical_checkpoint_tail() {
        let source = signed_source(
            GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 + 1,
            0x73,
            1_800_000_000,
        );
        let blocks = source
            .blocks
            .iter()
            .map(|block| block.block.clone())
            .collect::<Vec<_>>();
        let tail = &blocks[blocks.len() - GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1..];

        assert_eq!(source.head.checkpoint_cid, Some(tail[0].block_cid.clone()));
        assert_eq!(tail[0].sequence, 1);
        assert_eq!(
            tail[0].prev_block_cid,
            Some(blocks[0].block_cid.clone()),
            "the canonical tail may retain a parent outside the checkpoint window"
        );
        assert_eq!(tail[0].node.prev_cid, Some(blocks[0].node.node_cid.clone()));
        validate_source_head_chain(&source.head, &blocks)
            .expect("service accepts and validates the complete root history");
        validate_source_head_chain(&source.head, tail)
            .expect("service accepts the canonical signed checkpoint tail");

        let governed_public_key = &source.head.head_signature.public_key;
        for block in &blocks {
            assert_eq!(block.publisher_peer_id, source.head.publisher_peer_id);
            assert_eq!(block.node.publisher_peer_id, source.head.publisher_peer_id);
            assert_eq!(&block.block_signature.public_key, governed_public_key);
            assert_eq!(
                &block.node.publisher_signature.public_key,
                governed_public_key
            );
        }
    }

    #[test]
    fn service_rejects_checkpoint_tail_signature_and_continuity_drift() {
        let source = signed_source(
            GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 + 1,
            0x74,
            1_800_000_000,
        );
        let tail_start = source.blocks.len() - GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1;
        let canonical_tail = source.blocks[tail_start..]
            .iter()
            .map(|block| block.block.clone())
            .collect::<Vec<_>>();

        let attacker = TestSigner::new(0x75);
        let mut wrong_head_identity = source.head.clone();
        wrong_head_identity.head_signature = attacker.sign(
            &wrong_head_identity
                .signature_payload_bytes()
                .expect("encode attacker head payload"),
        );
        assert!(
            validate_source_head_chain(&wrong_head_identity, &canonical_tail).is_err(),
            "a byte-valid head signature from another identity must fail closed"
        );

        let mut wrong_identity = canonical_tail.clone();
        wrong_identity[0].block_signature = attacker.sign(
            &wrong_identity[0]
                .signature_payload_bytes()
                .expect("encode attacker block payload"),
        );
        assert!(
            validate_source_head_chain(&source.head, &wrong_identity).is_err(),
            "a byte-valid block signature from another identity must fail closed"
        );

        let governed = TestSigner::new(0x74);
        let mut broken_continuity = canonical_tail;
        broken_continuity[1].prev_block_cid = Some(vec![0xA5; 32]);
        broken_continuity[1].block_cid = broken_continuity[1]
            .recompute_block_cid()
            .expect("recompute continuity-drift block CID");
        broken_continuity[1].block_signature = governed.sign(
            &broken_continuity[1]
                .signature_payload_bytes()
                .expect("encode continuity-drift block payload"),
        );
        assert!(
            validate_source_head_chain(&source.head, &broken_continuity).is_err(),
            "a re-signed internal parent discontinuity must fail closed"
        );
    }

    #[test]
    fn source_loader_accepts_checkpointed_full_history_from_real_publisher() {
        let root = secure_temp_dir();
        let source_dir = root.path().join("source");
        let publisher_peer_id = TEST_PRODUCER_PEER_ID.as_bytes().to_vec();
        let producer_signer_handle = "pkcs11:governance-dag:source-primary";
        let signer = Arc::new(PublisherTestSigner {
            handle: producer_signer_handle.to_owned(),
            peer_id: publisher_peer_id.clone(),
            signer: TestSigner::new(0x76),
        });
        let signer = qualify_governance_dag_runtime_signer_provider(
            signer.handle().to_owned(),
            publisher_peer_id,
            signer.public_key(),
            GovernanceDagRuntimeProviderQualificationV1::new(1, [0x83; 32]),
            signer,
        )
        .expect("qualify real runtime DAG signer");
        let checkpoint_store = Arc::new(TestSealedStore::new(
            "kms:governance-dag:source-producer-checkpoint",
        ));
        let checkpoint_store = qualify_governance_dag_runtime_checkpoint_store(
            checkpoint_store.handle().to_owned(),
            GovernanceDagRuntimeProviderQualificationV1::new(1, [0x82; 32]),
            checkpoint_store,
        )
        .expect("qualify real runtime DAG producer checkpoint store");
        let publisher = FilesystemGovernancePublisher::try_new(source_dir.clone())
            .expect("create real filesystem governance publisher")
            .with_qualified_runtime_dag_providers(signer, checkpoint_store)
            .expect("configure real runtime DAG providers");
        let timestamp = current_unix_timestamp_seconds();
        for sequence in 0..=GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 as u64 {
            let settlement = settlement(sequence, timestamp);
            let encoded = norito::to_bytes(&settlement).expect("encode source settlement");
            publisher
                .publish_deal_settlement(&settlement, &encoded)
                .expect("publish source settlement");
        }
        drop(publisher);

        let signer = TestSigner::new(0x76);
        let state_dir = root.path().join("state");
        fs::create_dir_all(&state_dir).expect("create source-loader state root");
        let config = RuntimeConfig {
            source_root_guard: GovernanceFilesystemRootGuard::capture_source(&source_dir)
                .expect("fence real publisher source root"),
            source_dir,
            state_root_guard: GovernanceFilesystemRootGuard::capture_writer(&state_dir)
                .expect("fence source-loader state root"),
            listen_addr: "127.0.0.1:0".parse().expect("test address"),
            poll_interval: Duration::from_millis(10),
            max_response_bytes: 1024 * 1024,
            max_request_bytes: 1024 * 1024,
            mirror_max_entries: 1024,
            mirror_max_bytes: 1024 * 1024,
            max_head_age_secs: 3600,
            max_future_skew_secs: 60,
            allow_head_bootstrap: true,
            expected_producer_signer_handle: producer_signer_handle.to_owned(),
            expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
            expected_publisher_peer_id: TEST_PRODUCER_PEER_ID.as_bytes().to_vec(),
            expected_public_key: signer.public_key,
        };

        let loaded = load_source_snapshot(&config)
            .expect("service loads and revalidates checkpointed full source history");
        assert_eq!(
            loaded.blocks.len(),
            GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 + 1
        );
        assert_eq!(
            loaded.head.checkpoint_cid,
            Some(
                loaded.blocks[loaded.blocks.len() - GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1]
                    .block
                    .block_cid
                    .clone()
            )
        );

        let index_path = config.source_dir.join("runtime-dag-index.json");
        let mut index: JsonValue =
            json::from_slice(&fs::read(&index_path).expect("read publisher runtime index"))
                .expect("decode publisher runtime index");
        let first_entry = index
            .get_mut("blocks")
            .and_then(JsonValue::as_array_mut)
            .and_then(|blocks| blocks.first_mut())
            .and_then(JsonValue::as_object_mut)
            .expect("first publisher runtime index entry");
        let source_payload_path = first_entry
            .get("encoded_path")
            .and_then(JsonValue::as_str)
            .expect("source payload path")
            .to_owned();
        let substituted = settlement(999, timestamp);
        let substituted_bytes =
            norito::to_bytes(&substituted).expect("encode substituted source payload");
        write_test_sidecar_file(
            &config.source_dir.join(source_payload_path),
            &substituted_bytes,
        );
        first_entry.insert(
            "source_payload_len".into(),
            JsonValue::from(
                u64::try_from(substituted_bytes.len())
                    .expect("test source payload length fits u64"),
            ),
        );
        first_entry.insert(
            "source_payload_blake3".into(),
            JsonValue::from(hex::encode(blake3_array(&substituted_bytes))),
        );
        let tampered_index = json::to_json_pretty(&index)
            .expect("encode substituted runtime index")
            .into_bytes();
        write_test_sidecar_file(&index_path, &tampered_index);
        let error = load_source_snapshot(&config)
            .expect_err("source payload substitution must not escape the signed node binding");
        assert!(
            error
                .to_string()
                .contains("source payload does not match its signed governance node"),
            "unexpected source substitution error: {error}"
        );
    }

    #[test]
    fn committed_source_loader_authenticates_distinct_signing_key_segments() {
        let root = secure_temp_dir();
        let source_dir = root.path().join("source");
        let publisher_peer_id = TEST_PRODUCER_PEER_ID.as_bytes().to_vec();
        let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let producer_store = qualify_governance_dag_runtime_checkpoint_store(
            TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
            TEST_STORE_QUALIFICATION,
            checkpoint_provider.clone(),
        )
        .expect("qualify producer checkpoint store");

        let outgoing_provider = Arc::new(PublisherTestSigner {
            handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
            peer_id: publisher_peer_id.clone(),
            signer: TestSigner::new(0x76),
        });
        let outgoing_public_key = outgoing_provider.public_key();
        let outgoing_signer = qualify_governance_dag_runtime_signer_provider(
            TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
            publisher_peer_id.clone(),
            outgoing_public_key,
            TEST_PRODUCER_SIGNER_QUALIFICATION,
            outgoing_provider,
        )
        .expect("qualify outgoing producer signer");
        let mut publisher = FilesystemGovernancePublisher::try_new(source_dir.clone())
            .expect("create segmented filesystem governance publisher")
            .with_qualified_runtime_dag_providers(outgoing_signer, producer_store)
            .expect("configure outgoing producer providers");
        let timestamp = current_unix_timestamp_seconds();
        let outgoing_settlement = settlement(0, timestamp);
        let outgoing_encoded =
            norito::to_bytes(&outgoing_settlement).expect("encode outgoing settlement");
        publisher
            .publish_deal_settlement(&outgoing_settlement, &outgoing_encoded)
            .expect("publish outgoing authority block");

        let incoming_provider = Arc::new(PublisherTestSigner {
            handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
            peer_id: publisher_peer_id.clone(),
            signer: TestSigner::new(0x77),
        });
        let incoming_public_key = incoming_provider.public_key();
        assert_ne!(outgoing_public_key, incoming_public_key);
        let incoming_signer = qualify_governance_dag_runtime_signer_provider(
            TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
            publisher_peer_id.clone(),
            incoming_public_key,
            TEST_PRODUCER_SIGNER_QUALIFICATION,
            incoming_provider,
        )
        .expect("qualify incoming producer signer");
        let incoming_store = qualify_governance_dag_runtime_checkpoint_store(
            TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
            TEST_STORE_QUALIFICATION,
            checkpoint_provider.clone(),
        )
        .expect("qualify incoming producer checkpoint store");
        publisher
            .transition_qualified_runtime_dag_providers(incoming_signer, incoming_store)
            .expect("install dual-signed key transition");

        let state_dir = root.path().join("state");
        fs::create_dir_all(&state_dir).expect("create segmented source-loader state root");
        let config = RuntimeConfig {
            source_root_guard: GovernanceFilesystemRootGuard::capture_source(&source_dir)
                .expect("fence segmented publisher source root"),
            source_dir,
            state_root_guard: GovernanceFilesystemRootGuard::capture_writer(&state_dir)
                .expect("fence segmented source-loader state root"),
            listen_addr: "127.0.0.1:0".parse().expect("test address"),
            poll_interval: Duration::from_millis(10),
            max_response_bytes: 1024 * 1024,
            max_request_bytes: 1024 * 1024,
            mirror_max_entries: 1024,
            mirror_max_bytes: 1024 * 1024,
            max_head_age_secs: 3600,
            max_future_skew_secs: 60,
            allow_head_bootstrap: true,
            expected_producer_signer_handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
            expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
            expected_publisher_peer_id: publisher_peer_id,
            expected_public_key: incoming_public_key,
        };
        let service_store = test_checkpoint_store(Arc::clone(&checkpoint_provider));
        let rotated_without_append = load_committed_source_snapshot(&config, &service_store)
            .expect("incoming binding authenticates the outgoing-signed retained tip");
        assert_eq!(rotated_without_append.blocks.len(), 1);
        assert_eq!(
            rotated_without_append.head.head_signature.public_key,
            outgoing_public_key.to_vec()
        );

        let incoming_settlement = settlement(1, timestamp.saturating_add(1));
        let incoming_encoded =
            norito::to_bytes(&incoming_settlement).expect("encode incoming settlement");
        publisher
            .publish_deal_settlement(&incoming_settlement, &incoming_encoded)
            .expect("publish incoming authority block");
        let segmented = load_committed_source_snapshot(&config, &service_store)
            .expect("service readback authenticates both signing-key segments");
        assert_eq!(segmented.blocks.len(), 2);
        assert_eq!(
            segmented.blocks[0].block.block_signature.public_key,
            outgoing_public_key.to_vec()
        );
        assert_eq!(
            segmented.blocks[1].block.block_signature.public_key,
            incoming_public_key.to_vec()
        );
        assert_eq!(
            segmented.head.head_signature.public_key,
            incoming_public_key.to_vec()
        );

        let mut sealed = checkpoint_provider
            .inner
            .lock()
            .expect("lock segmented producer checkpoint");
        let current_record = sealed
            .producer_checkpoint
            .as_ref()
            .expect("segmented producer checkpoint")
            .clone();
        let mut substituted_checkpoint: RuntimeDagProducerCheckpointV1 =
            norito::decode_from_bytes(&current_record.payload)
                .expect("decode segmented producer checkpoint");
        substituted_checkpoint.qualification_transition_digest[0] ^= 0x80;
        sealed.producer_checkpoint = Some(GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
            current_record.generation,
            norito::to_bytes(&substituted_checkpoint)
                .expect("encode substituted segmented producer checkpoint"),
        ));
        drop(sealed);
        let error = load_committed_source_snapshot(&config, &service_store)
            .expect_err("sealed key-transition lineage substitution must fail closed");
        assert!(
            error.to_string().contains("authority lineage diverges"),
            "unexpected sealed lineage substitution error: {error}"
        );
    }

    #[test]
    fn checkpoint_rejects_rollback_and_fork() {
        let original = signed_source(3, 0x34, 1_800_000_000);
        let checkpoint = checkpoint_from_source(&original);
        let rolled_back = signed_source(2, 0x34, 1_800_000_000);
        assert!(validate_checkpoint_against_source(Some(&checkpoint), &rolled_back).is_err());

        let fork = signed_source(3, 0x34, 1_800_000_100);
        assert!(validate_checkpoint_against_source(Some(&checkpoint), &fork).is_err());
    }

    #[test]
    fn producer_commit_guard_binds_the_exact_verified_source_index() {
        let root = secure_temp_dir();
        let source_dir = root.path().join("source");
        let mut source = signed_source(2, 0x74, 1_800_000_000);
        materialize_source_snapshot(&source_dir, &mut source);
        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        seed_producer_checkpoint(&provider, &source_dir, &source);
        let store = test_checkpoint_store(provider.clone());
        let config = test_runtime_config(&source, root.path());
        let loaded = load_committed_source_snapshot(&config, &store)
            .expect("stable sealed producer checkpoint admits the exact source snapshot");
        assert_eq!(loaded.index_blake3, source.index_blake3);

        let mut checkpoint = producer_checkpoint_from_source(&source_dir, &source);
        checkpoint.index_bytes_digest[0] ^= 0x80;
        let replacement = GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
            checkpoint.block_count.saturating_add(1),
            norito::to_bytes(&checkpoint).expect("encode tampered producer checkpoint"),
        );
        provider
            .inner
            .lock()
            .expect("lock test producer store")
            .producer_checkpoint = Some(replacement);
        let error = load_committed_source_snapshot(&config, &store)
            .expect_err("mismatched sealed producer index digest must fail closed");
        assert!(error.to_string().contains("does not match"));
    }

    #[test]
    fn service_checkpoint_and_intent_bind_current_source_index_digest() {
        let source = signed_source(2, 0x75, 1_800_000_000);
        let mut checkpoint = checkpoint_from_source(&source);
        checkpoint.source_index_blake3[0] ^= 0x80;
        assert!(
            validate_checkpoint_against_source(Some(&checkpoint), &source)
                .expect_err("current checkpoint must bind the exact source index")
                .to_string()
                .contains("source-index digest")
        );

        let mut intent = intent_from_source(&source);
        intent.target_source_index_blake3[0] ^= 0x80;
        let root = secure_temp_dir();
        let config = test_runtime_config(&source, root.path());
        assert!(
            validate_intent_against_source(&intent, None, &source, &config)
                .expect_err("current publish intent must bind the exact source index")
                .to_string()
                .contains("source-index digest")
        );
    }

    #[test]
    fn manifest_chain_rejects_sequence_gap_and_timestamp_regression() {
        let signer = TestSigner::new(0x35);
        let source = signed_source(2, 0x35, 1_800_000_000);
        let mut sequence_blocks = source
            .blocks
            .iter()
            .map(|block| block.block.clone())
            .collect::<Vec<_>>();
        sequence_blocks[1].sequence = 7;
        sequence_blocks[1].block_cid = sequence_blocks[1]
            .recompute_block_cid()
            .expect("recompute sequence-gap CID");
        sequence_blocks[1].block_signature = signer.sign(
            &sequence_blocks[1]
                .signature_payload_bytes()
                .expect("encode sequence-gap block"),
        );
        let mut sequence_head = source.head.clone();
        sequence_head.head_block_cid = sequence_blocks[1].block_cid.clone();
        sequence_head.head_signature = signer.sign(
            &sequence_head
                .signature_payload_bytes()
                .expect("encode sequence-gap head"),
        );
        assert!(
            validate_governance_dag_head_against_chain_v1(&sequence_head, &sequence_blocks)
                .is_err()
        );

        let mut time_blocks = source
            .blocks
            .iter()
            .map(|block| block.block.clone())
            .collect::<Vec<_>>();
        time_blocks[1].timestamp = time_blocks[0].timestamp.saturating_sub(1);
        time_blocks[1].block_cid = time_blocks[1]
            .recompute_block_cid()
            .expect("recompute regressed CID");
        time_blocks[1].block_signature = signer.sign(
            &time_blocks[1]
                .signature_payload_bytes()
                .expect("encode regressed block"),
        );
        let mut time_head = source.head.clone();
        time_head.head_block_cid = time_blocks[1].block_cid.clone();
        time_head.head_signature = signer.sign(
            &time_head
                .signature_payload_bytes()
                .expect("encode regressed head"),
        );
        assert!(validate_governance_dag_head_against_chain_v1(&time_head, &time_blocks).is_err());
    }

    #[test]
    fn bounded_file_read_rejects_oversize() {
        let dir = secure_temp_dir();
        let path = dir.path().join("oversize.bin");
        fs::write(&path, [0_u8; 9]).expect("write oversized file");
        assert!(read_unrooted_regular_file(&path, 8, false).is_err());
    }

    #[test]
    fn rooted_source_binding_rejects_equal_byte_substitution() {
        let dir = secure_temp_dir();
        let path = dir.path().join("source.to");
        fs::write(&path, b"same-bytes").expect("seed rooted source");
        let guard =
            GovernanceFilesystemRootGuard::capture_source(dir.path()).expect("retain source root");
        let snapshot = read_rooted_file(&guard, Path::new("source.to"), 32, false)
            .expect("read rooted source");

        fs::remove_file(&path).expect("remove original source");
        fs::write(&path, b"same-bytes").expect("replace source with equal bytes");
        let error = verify_rooted_file_binding(&guard, &snapshot.binding())
            .expect_err("equal-byte identity substitution must fail closed");
        assert!(error.to_string().contains("substituted"));
    }

    #[cfg(unix)]
    #[test]
    fn rooted_source_read_rejects_descendant_symlink() {
        use std::os::unix::fs::symlink;

        let dir = secure_temp_dir();
        fs::write(dir.path().join("target.to"), b"target").expect("seed target");
        symlink(dir.path().join("target.to"), dir.path().join("linked.to"))
            .expect("create descendant symlink");
        let guard =
            GovernanceFilesystemRootGuard::capture_source(dir.path()).expect("retain source root");
        read_rooted_file(&guard, Path::new("linked.to"), 32, false)
            .expect_err("rooted source read must reject symlink");
    }

    #[test]
    fn rooted_state_recovery_is_deterministic_after_restart() {
        let dir = secure_temp_dir();
        let guard =
            GovernanceFilesystemRootGuard::capture_writer(dir.path()).expect("retain state root");
        write_rooted_atomic_secret(&guard, Path::new(MIRROR_INDEX_FILE), b"first-generation")
            .expect("write first state generation");
        drop(guard);

        let stale = dir.path().join(format!(".{MIRROR_INDEX_FILE}.tmp-42000-9"));
        fs::write(&stale, b"crash-temporary").expect("seed restart temporary");
        let restarted = GovernanceFilesystemRootGuard::capture_writer(dir.path())
            .expect("retain restarted state root");
        write_rooted_atomic_secret(
            &restarted,
            Path::new(MIRROR_INDEX_FILE),
            b"second-generation",
        )
        .expect("recover and write second state generation");

        assert!(!stale.exists());
        assert_eq!(
            read_rooted_file(
                &restarted,
                Path::new(MIRROR_INDEX_FILE),
                MUTABLE_STATE_MAX_BYTES,
                true,
            )
            .expect("read restarted state")
            .bytes(),
            b"second-generation"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bounded_file_read_rejects_symlink_hardlink_and_permissive_secret() {
        use std::os::unix::fs::symlink;

        let dir = secure_temp_dir();
        let target = dir.path().join("target.bin");
        fs::write(&target, [0x11; 32]).expect("write target");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("secure target");

        let symlink_path = dir.path().join("symlink.bin");
        symlink(&target, &symlink_path).expect("create symlink");
        assert!(read_unrooted_regular_file(&symlink_path, 32, true).is_err());

        let hardlink_path = dir.path().join("hardlink.bin");
        fs::hard_link(&target, &hardlink_path).expect("create hard link");
        assert!(read_unrooted_regular_file(&target, 32, true).is_err());
        fs::remove_file(&hardlink_path).expect("remove hard link");

        fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
            .expect("make secret permissive");
        assert!(read_unrooted_regular_file(&target, 32, true).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn legacy_secret_paths_are_rejected_without_following_symlinks_or_reading_files() {
        use std::os::unix::fs::symlink;

        let dir = secure_temp_dir();
        let source_dir = dir.path().join("source");
        fs::create_dir(&source_dir).expect("create source directory");
        let target = dir.path().join("permissive-secret");
        let sentinel = b"must-never-be-read-or-overwritten";
        fs::write(&target, sentinel).expect("write legacy secret sentinel");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
            .expect("make legacy secret permissive");
        let link = dir.path().join("legacy-secret-link");
        symlink(&target, &link).expect("create legacy secret symlink");

        for (field, path) in [
            ("ipfs_bearer_token_path", &link),
            ("head_bearer_token_path", &target),
            ("checkpoint_key_path", &link),
        ] {
            let config_path = dir.path().join(format!("{field}.toml"));
            fs::write(
                &config_path,
                format!(
                    r#"[sorafs.storage]
governance_dag_dir = "{}"

[sorafs.storage.governance_dag_service]
enabled = false
{field} = "{}"
"#,
                    source_dir.display(),
                    path.display(),
                ),
            )
            .expect("write legacy config");
            let error =
                load_service_config(&config_path).expect_err("legacy secret path must fail");
            assert!(matches!(&error, GovernanceDagServiceError::Config(_)));
            assert!(
                !error.to_string().contains(&path.display().to_string()),
                "legacy secret path leaked into the parser error: {error}"
            );
            assert_eq!(
                fs::read(&target).expect("read sentinel after config rejection"),
                sentinel
            );
        }

        let config_path = dir.path().join("governance_dag_signing_key_path.toml");
        fs::write(
            &config_path,
            format!(
                r#"[sorafs.storage]
governance_dag_dir = "{}"
governance_dag_signing_key_path = "{}"

[sorafs.storage.governance_dag_service]
enabled = false
"#,
                source_dir.display(),
                link.display(),
            ),
        )
        .expect("write legacy signer config");
        let error =
            load_service_config(&config_path).expect_err("legacy signing-key path must fail");
        assert!(matches!(&error, GovernanceDagServiceError::Config(_)));
        assert!(
            !error.to_string().contains(&link.display().to_string()),
            "legacy signing-key path leaked into the parser error: {error}"
        );
        assert_eq!(
            fs::read(&target).expect("read sentinel after signer config rejection"),
            sentinel
        );
    }

    #[test]
    fn sealed_checkpoint_rejects_tamper_and_mismatched_store_handle() {
        let source = signed_source(1, 0x36, 1_800_000_000);
        let checkpoint = checkpoint_from_source(&source);
        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let store = test_checkpoint_store(provider.clone());
        let revision = save_checkpoint(&store, None, &checkpoint).expect("save sealed checkpoint");
        assert_eq!(
            load_checkpoint(&store).expect("load sealed checkpoint"),
            (Some(checkpoint.clone()), Some(revision))
        );

        let mismatch = OpaqueCheckpointStore::try_new(
            "kms:governance/checkpoint:other",
            TEST_STORE_QUALIFICATION,
            provider.clone(),
        )
        .expect_err("mismatched checkpoint provider handle must fail");
        assert!(mismatch.to_string().contains("does not match"));

        provider
            .qualification_revision
            .store(2, AtomicOrdering::SeqCst);
        let error =
            load_checkpoint(&store).expect_err("checkpoint provider policy drift must fail");
        assert!(error.to_string().contains("policy changed"));
        provider
            .qualification_revision
            .store(1, AtomicOrdering::SeqCst);

        let drifting_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let drifting_store = test_checkpoint_store(drifting_provider.clone());
        drifting_provider
            .drift_during_operation
            .store(true, AtomicOrdering::SeqCst);
        let error = load_checkpoint(&drifting_store)
            .expect_err("policy drift during a sealed read must discard its result");
        assert!(error.to_string().contains("policy changed"));

        let drifting_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let drifting_store = test_checkpoint_store(drifting_provider.clone());
        drifting_provider
            .drift_during_operation
            .store(true, AtomicOrdering::SeqCst);
        let error = save_checkpoint(&drifting_store, None, &checkpoint)
            .expect_err("policy drift during sealed CAS must fail closed");
        assert!(error.to_string().contains("policy changed"));

        let mut inner = provider.inner.lock().expect("lock test store");
        let record = inner.checkpoint.as_mut().expect("checkpoint record");
        let last = record.payload.last_mut().expect("checkpoint is non-empty");
        *last ^= 0x80;
        drop(inner);
        assert!(load_checkpoint(&store).is_err());
    }

    #[test]
    fn sealed_intent_rejects_tamper_rollback_replay_and_store_outage() {
        let source = signed_source(1, 0x37, 1_800_000_000);
        let intent = intent_from_source(&source);
        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let store = test_checkpoint_store(provider.clone());
        let revision = save_publish_intent(&store, None, &intent).expect("save sealed intent");
        assert_eq!(
            load_publish_intent(&store).expect("load sealed intent"),
            (Some(intent.clone()), Some(revision))
        );

        delete_publish_intent(&store, Some(revision)).expect("delete exact intent revision");
        let error = save_publish_intent(&store, None, &intent)
            .expect_err("deleted intent generation replay must fail");
        assert!(error.to_string().contains("compare-and-swap failed"));

        let mut next_intent = intent.clone();
        next_intent.generation = next_intent.generation.saturating_add(1);
        let revision =
            save_publish_intent(&store, None, &next_intent).expect("next generation may resume");
        let error = save_publish_intent(&store, Some([0xA5; 32]), &next_intent)
            .expect_err("stale CAS revision must fail");
        assert!(error.to_string().contains("compare-and-swap failed"));

        let mut inner = provider.inner.lock().expect("lock test store");
        let record = inner.publish_intent.as_mut().expect("intent record");
        record
            .payload
            .truncate(record.payload.len().saturating_sub(1));
        drop(inner);
        assert!(load_publish_intent(&store).is_err());

        provider.refuse.store(true, AtomicOrdering::SeqCst);
        let error = load_publish_intent(&store).expect_err("store outage must fail closed");
        assert!(error.to_string().contains("read failed"));
        assert!(!error.to_string().contains("must-never-escape"));
        assert_ne!(revision, [0; 32]);
    }

    #[test]
    fn producer_and_public_service_sealed_slots_coexist_without_cross_mutation() {
        let root = secure_temp_dir();
        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let service_checkpoint = GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::Checkpoint,
            7,
            vec![0x71],
        );
        let service_intent = GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::PublishIntent,
            8,
            vec![0x72],
        );
        provider
            .compare_and_swap(
                GovernanceDagSealedStateSlot::Checkpoint,
                None,
                service_checkpoint.clone(),
            )
            .expect("seed service checkpoint slot");
        provider
            .compare_and_swap(
                GovernanceDagSealedStateSlot::PublishIntent,
                None,
                service_intent.clone(),
            )
            .expect("seed service intent slot");

        let publisher_peer_id = b"12D3KooWGovernanceSharedStore".to_vec();
        let signer = Arc::new(PublisherTestSigner {
            handle: "pkcs11:governance-dag:shared-store-primary".to_owned(),
            peer_id: publisher_peer_id.clone(),
            signer: TestSigner::new(0x77),
        });
        let signer = qualify_governance_dag_runtime_signer_provider(
            signer.handle().to_owned(),
            publisher_peer_id,
            signer.public_key(),
            GovernanceDagRuntimeProviderQualificationV1::new(1, [0x83; 32]),
            signer,
        )
        .expect("qualify shared-store producer signer");
        let producer_store = qualify_governance_dag_runtime_checkpoint_store(
            TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
            TEST_STORE_QUALIFICATION,
            provider.clone(),
        )
        .expect("qualify shared sealed store for producer slots");
        let publisher = FilesystemGovernancePublisher::try_new(root.path().join("producer"))
            .expect("create shared-store publisher")
            .with_qualified_runtime_dag_providers(signer, producer_store)
            .expect("bind producer providers to shared sealed store");
        let settlement = settlement(0, current_unix_timestamp_seconds());
        let encoded = norito::to_bytes(&settlement).expect("encode shared-store settlement");
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("commit producer transaction through producer-only slots");

        assert_eq!(
            provider
                .load(GovernanceDagSealedStateSlot::Checkpoint)
                .expect("read service checkpoint"),
            Some(service_checkpoint.clone())
        );
        assert_eq!(
            provider
                .load(GovernanceDagSealedStateSlot::PublishIntent)
                .expect("read service intent"),
            Some(service_intent)
        );
        let producer_checkpoint = provider
            .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)
            .expect("read producer checkpoint")
            .expect("producer checkpoint exists");
        let producer_intent = provider
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("read producer intent");
        assert!(producer_intent.is_none());

        let next_service_checkpoint = GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::Checkpoint,
            9,
            vec![0x73],
        );
        provider
            .compare_and_swap(
                GovernanceDagSealedStateSlot::Checkpoint,
                Some(service_checkpoint.revision),
                next_service_checkpoint,
            )
            .expect("advance service-only checkpoint slot");
        assert_eq!(
            provider
                .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)
                .expect("re-read producer checkpoint"),
            Some(producer_checkpoint)
        );
        assert!(
            provider
                .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
                .expect("re-read producer intent")
                .is_none()
        );
    }

    #[test]
    fn mirror_retention_honours_entry_and_byte_caps() {
        let source = signed_source(3, 0x38, 1_800_000_000);
        let intent = intent_from_source(&source);
        let latest = source.blocks[2].bytes.len() as u64;
        let previous = source.blocks[1].bytes.len() as u64;
        let exact_two = latest + previous;
        let retained = merge_published_blocks(None, &intent, &source, 2, exact_two)
            .expect("retain exact two-block suffix");
        assert_eq!(retained.len(), 2);
        assert_eq!(retained[0].sequence, 1);
        assert_eq!(retained[1].sequence, 2);

        let one = merge_published_blocks(None, &intent, &source, 1, exact_two)
            .expect("entry cap retains one block");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].sequence, 2);

        let byte_limited = merge_published_blocks(None, &intent, &source, 3, exact_two - 1)
            .expect("byte cap retains the newest fitting suffix");
        assert_eq!(byte_limited.len(), 1);
        assert!(merge_published_blocks(None, &intent, &source, 3, latest - 1).is_err());
    }

    #[test]
    fn canonical_lookup_ids_reject_uppercase_short_and_non_hex() {
        assert!(is_canonical_digest_hex(&"ab".repeat(32)));
        assert!(!is_canonical_digest_hex(&"AB".repeat(32)));
        assert!(!is_canonical_digest_hex("ab"));
        assert!(!is_canonical_digest_hex(&"gg".repeat(32)));
    }

    #[test]
    fn json_response_etag_supports_exact_not_modified() {
        let value = JsonValue::from("stable");
        let first = json_response(StatusCode::OK, value.clone(), &HeaderMap::new());
        assert_eq!(first.status(), StatusCode::OK);
        let etag = first
            .headers()
            .get(header::ETAG)
            .expect("response has ETag")
            .clone();
        let mut request_headers = HeaderMap::new();
        request_headers.insert(header::IF_NONE_MATCH, etag.clone());
        let second = json_response(StatusCode::OK, value, &request_headers);
        assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(second.headers().get(header::ETAG), Some(&etag));
    }

    #[tokio::test]
    async fn routes_reject_noncanonical_identifiers_before_lookup() {
        let state = ApiState(Arc::new(RwLock::new(ApiSnapshot {
            live: true,
            ready: true,
            ..ApiSnapshot::default()
        })));
        let app = service_router(state);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/sorafs/governance/dag/blocks/ABCD")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("route response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/sorafs/governance/dag/digests/gggg")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("route response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn private_ipfs_permission_does_not_authorize_private_head_endpoint() {
        let config = SorafsGovernanceDagService {
            allow_insecure_http: true,
            allow_private_ipfs_endpoint: true,
            allow_private_head_endpoint: false,
            ..SorafsGovernanceDagService::default()
        };
        let ipfs = build_pinned_endpoint(
            "http://127.0.0.1:5001",
            test_authenticator(TEST_IPFS_AUTH_HANDLE),
            GovernanceDagAuthenticationScope::Ipfs,
            &config,
            true,
        )
        .await;
        assert!(ipfs.is_ok());
        let head = build_pinned_endpoint(
            "http://127.0.0.1:9099/head",
            test_authenticator(TEST_HEAD_AUTH_HANDLE),
            GovernanceDagAuthenticationScope::SignedHead,
            &config,
            false,
        )
        .await;
        assert!(head.is_err());
    }

    #[tokio::test]
    async fn dns_policy_rejects_mixed_mapped_overcap_and_timeout_answers() {
        let public = "8.8.8.8:443".parse().expect("public address");
        let private = "127.0.0.1:443".parse().expect("private address");
        assert!(
            resolve_endpoint_addresses(
                std::future::ready(Ok(vec![public, private])),
                Duration::from_secs(1),
                false,
            )
            .await
            .is_err()
        );

        let mapped = SocketAddr::new(
            IpAddr::V6("::ffff:127.0.0.1".parse().expect("mapped IPv6")),
            443,
        );
        assert!(
            resolve_endpoint_addresses(
                std::future::ready(Ok(vec![mapped])),
                Duration::from_secs(1),
                false,
            )
            .await
            .is_err()
        );

        let over_cap = (1..=(MAX_DNS_ADDRESSES + 1))
            .map(|last| SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 4, last as u8)), 443))
            .collect::<Vec<_>>();
        assert!(
            resolve_endpoint_addresses(
                std::future::ready(Ok(over_cap)),
                Duration::from_secs(1),
                false,
            )
            .await
            .is_err()
        );

        let delayed = async {
            time::sleep(Duration::from_millis(50)).await;
            Ok(vec![public])
        };
        assert!(
            resolve_endpoint_addresses(delayed, Duration::from_millis(1), false)
                .await
                .is_err()
        );

        let calls = Arc::new(AtomicU64::new(0));
        let calls_for_resolution = calls.clone();
        let resolved = resolve_endpoint_addresses(
            async move {
                calls_for_resolution.fetch_add(1, Ordering::SeqCst);
                Ok(vec![public, public])
            },
            Duration::from_secs(1),
            false,
        )
        .await
        .expect("one pinned public DNS snapshot");
        assert_eq!(resolved, vec![public]);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn ipfs_urls_cids_and_secret_debug_output_are_canonical() {
        let provider = Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "never-log-this-token",
        ));
        let authenticator = OpaqueAuthenticator::try_new(
            TEST_IPFS_AUTH_HANDLE,
            TEST_AUTH_QUALIFICATION,
            provider.public_key(),
            30,
            5,
            provider,
            "IPFS authenticator",
        )
        .expect("bind test authenticator");
        let endpoint = PinnedEndpoint {
            url: Url::parse("http://127.0.0.1:5001/").expect("test URL"),
            client: Client::builder().no_proxy().build().expect("test client"),
            authentication_scope: GovernanceDagAuthenticationScope::Ipfs,
            authenticator: authenticator.clone(),
            max_request_bytes: GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1 as u64,
        };
        let url = endpoint
            .ipfs_url(
                "api/v0/cat",
                &[("arg", TEST_CID_PAYLOAD), ("progress", "false")],
            )
            .expect("canonical IPFS URL");
        let pairs = url.query_pairs().collect::<Vec<_>>();
        assert_eq!(pairs.len(), 2, "query fields must not be duplicated");
        assert_eq!(pairs[0], ("arg".into(), TEST_CID_PAYLOAD.into()));
        assert_eq!(pairs[1], ("progress".into(), "false".into()));

        for cid in [
            TEST_CID_PAYLOAD,
            TEST_CID_BLOCK,
            TEST_CID_HEAD,
            TEST_CID_OLD,
            TEST_CID_NEW,
            TEST_CID_ATTACKER,
        ] {
            assert!(is_canonical_cid_v1(cid), "valid CID rejected: {cid}");
            assert_eq!(
                validate_ipfs_cid(cid).expect("canonical CID must validate"),
                cid
            );
        }
        let uppercase = TEST_CID_PAYLOAD.to_ascii_uppercase();
        let padded = format!("{TEST_CID_PAYLOAD}=");
        let truncated = &TEST_CID_PAYLOAD[..TEST_CID_PAYLOAD.len() - 1];
        for cid in [
            "",
            "QmYwAPJzv5CZsnAzt8auVZRnGi2j4XQJKiTyrZq4XgNLwN",
            "bafytestcid",
            uppercase.as_str(),
            padded.as_str(),
            truncated,
        ] {
            assert!(!is_canonical_cid_v1(cid), "invalid CID accepted: {cid}");
            assert!(validate_ipfs_cid(cid).is_err());
        }

        let rendered = format!("{authenticator:?}");
        assert!(rendered.contains("[REDACTED]"));
        assert!(!rendered.contains("never-log-this-token"));
        assert!(rendered.contains(TEST_IPFS_AUTH_HANDLE));
    }

    #[test]
    fn authenticator_rotates_per_request_and_redacts_provider_failures() {
        let provider = Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "first-secret-token",
        ));
        let authenticator = OpaqueAuthenticator::try_new(
            TEST_IPFS_AUTH_HANDLE,
            TEST_AUTH_QUALIFICATION,
            provider.public_key(),
            30,
            5,
            provider.clone(),
            "IPFS authenticator",
        )
        .expect("bind runtime authenticator");
        let client = Client::builder().no_proxy().build().expect("test client");
        let url = Url::parse("https://example.invalid/").expect("test URL");
        let request = client
            .get(url)
            .header(header::ACCEPT_ENCODING, "identity")
            .build()
            .expect("build test request");
        let descriptor = canonical_outbound_request_descriptor(
            &request,
            GovernanceDagAuthenticationScope::Ipfs,
            1024,
        )
        .expect("canonical test request");

        let first = authenticator
            .authenticate(&descriptor)
            .expect("authenticate first request");
        assert_eq!(first.request_digest(), descriptor.request_digest());
        assert_eq!(first.public_key(), provider.public_key());

        provider.rotate("rotated-secret-token");
        let rotated = authenticator
            .authenticate(&descriptor)
            .expect("authenticate rotated request");
        assert_ne!(first.nonce(), rotated.nonce());

        provider
            .qualification_revision
            .store(2, AtomicOrdering::SeqCst);
        let error = authenticator
            .authenticate(&descriptor)
            .expect_err("authenticator policy drift must fail closed");
        assert!(error.to_string().contains("policy changed"));
        provider
            .qualification_revision
            .store(1, AtomicOrdering::SeqCst);

        let drifting_provider = Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "must-not-be-returned",
        ));
        let drifting_authenticator = OpaqueAuthenticator::try_new(
            TEST_IPFS_AUTH_HANDLE,
            TEST_AUTH_QUALIFICATION,
            drifting_provider.public_key(),
            30,
            5,
            drifting_provider.clone(),
            "IPFS authenticator",
        )
        .expect("bind stable runtime authenticator");
        drifting_provider
            .drift_during_authentication
            .store(true, AtomicOrdering::SeqCst);
        let error = drifting_authenticator
            .authenticate(&descriptor)
            .expect_err("policy drift during authentication must discard the request");
        assert!(error.to_string().contains("policy changed"));
        assert!(!error.to_string().contains("must-not-be-returned"));

        provider.refuse.store(true, AtomicOrdering::SeqCst);
        let error = authenticator
            .authenticate(&descriptor)
            .expect_err("authenticator outage must fail closed");
        assert!(error.to_string().contains("refused"));
        assert!(!error.to_string().contains("rotated-secret-token"));

        let mismatch = OpaqueAuthenticator::try_new(
            "vault:governance/ipfs:other",
            TEST_AUTH_QUALIFICATION,
            provider.public_key(),
            30,
            5,
            provider,
            "IPFS authenticator",
        )
        .expect_err("mismatched authenticator handle must fail");
        assert!(mismatch.to_string().contains("does not match"));
    }

    #[test]
    fn request_auth_envelope_rejects_tamper_replay_key_and_time_failures() {
        let provider = Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "never-expose-hsm-diagnostic",
        ));
        let authenticator = OpaqueAuthenticator::try_new(
            TEST_IPFS_AUTH_HANDLE,
            TEST_AUTH_QUALIFICATION,
            provider.public_key(),
            30,
            5,
            provider,
            "IPFS authenticator",
        )
        .expect("bind request-auth verifier");
        let request = canonical_test_request(
            GovernanceDagAuthenticationScope::Ipfs,
            "POST",
            "https://example.invalid/api/v0/pin/add?arg=cid&recursive=true",
            &[("accept-encoding", "identity")],
            b"",
        );
        let tampered = canonical_test_request(
            GovernanceDagAuthenticationScope::Ipfs,
            "POST",
            "https://example.invalid/api/v0/pin/add?arg=other&recursive=true",
            &[("accept-encoding", "identity")],
            b"",
        );
        let now = current_unix_timestamp_seconds();
        let envelope = signed_test_request_auth_envelope(
            TEST_IPFS_AUTH_HANDLE,
            &request,
            now,
            now + 15,
            [0x11; 32],
        );
        authenticator
            .validate_envelope(&request, &envelope)
            .expect("accept first exact envelope");
        let replay = authenticator
            .validate_envelope(&request, &envelope)
            .expect_err("reject exact nonce replay");
        assert!(replay.to_string().contains("replay"));
        let tamper = authenticator
            .validate_envelope(&tampered, &envelope)
            .expect_err("reject URL/request-digest tamper");
        assert!(tamper.to_string().contains("does not match"));

        let wrong_key = signed_test_request_auth_envelope(
            TEST_HEAD_AUTH_HANDLE,
            &request,
            now,
            now + 15,
            [0x12; 32],
        );
        assert!(
            authenticator
                .validate_envelope(&request, &wrong_key)
                .expect_err("reject wrong public key")
                .to_string()
                .contains("does not match")
        );

        let invalid_signature = GovernanceDagRequestAuthenticationEnvelopeV1::try_new(
            &request,
            now,
            now + 15,
            [0x13; 32],
            test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE),
            [0x55; 64],
        )
        .expect("structurally non-zero envelope");
        assert!(
            authenticator
                .validate_envelope(&request, &invalid_signature)
                .expect_err("reject invalid signature")
                .to_string()
                .contains("signature")
        );

        for (issued_at, expires_at, nonce, label) in [
            (now - 20, now - 1, [0x21; 32], "stale"),
            (now + 6, now + 16, [0x22; 32], "future"),
            (now, now + 31, [0x23; 32], "overlong"),
        ] {
            let envelope = signed_test_request_auth_envelope(
                TEST_IPFS_AUTH_HANDLE,
                &request,
                issued_at,
                expires_at,
                nonce,
            );
            let error = authenticator
                .validate_envelope(&request, &envelope)
                .unwrap_err();
            assert!(
                error.to_string().contains(label)
                    || error.to_string().contains("future")
                    || error.to_string().contains("overlong"),
                "{label} envelope returned unexpected error: {error}"
            );
        }
    }

    #[test]
    fn inbound_request_auth_accepts_canonical_ipfs_and_head_operations() {
        let now = 1_700_000_000;
        let ipfs_policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
            test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE),
            30,
            5,
        )
        .expect("valid IPFS receiver policy");
        let head_policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
            test_request_auth_public_key(TEST_HEAD_AUTH_HANDLE),
            30,
            5,
        )
        .expect("valid signed-head receiver policy");
        let cases = vec![
            (
                GovernanceDagAuthenticationScope::Ipfs,
                "GET",
                "https://example.invalid/api/v0/cat?arg=cid",
                vec![("accept-encoding", b"identity".as_slice())],
                b"".as_slice(),
                TEST_IPFS_AUTH_HANDLE,
                [0x31; 32],
            ),
            (
                GovernanceDagAuthenticationScope::Ipfs,
                "POST",
                "https://example.invalid/api/v0/add?pin=false",
                vec![
                    (
                        "content-type",
                        b"multipart/form-data;boundary=gdag".as_slice(),
                    ),
                    ("accept-encoding", b"identity".as_slice()),
                ],
                b"canonical-block".as_slice(),
                TEST_IPFS_AUTH_HANDLE,
                [0x32; 32],
            ),
            (
                GovernanceDagAuthenticationScope::SignedHead,
                "GET",
                "https://example.invalid/governance/head",
                vec![
                    ("if-none-match", b"\"v7\"".as_slice()),
                    ("accept-encoding", b"identity".as_slice()),
                ],
                b"".as_slice(),
                TEST_HEAD_AUTH_HANDLE,
                [0x33; 32],
            ),
            (
                GovernanceDagAuthenticationScope::SignedHead,
                "PUT",
                "https://example.invalid/governance/head",
                vec![
                    ("if-match", b"\"v7\"".as_slice()),
                    ("content-type", b"application/vnd.iroha.norito".as_slice()),
                    ("accept-encoding", b"identity".as_slice()),
                ],
                b"canonical-head".as_slice(),
                TEST_HEAD_AUTH_HANDLE,
                [0x34; 32],
            ),
        ];
        let backend_calls = AtomicU64::new(0);
        let mut ipfs_replay_cache = GovernanceDagRequestAuthenticationReplayCacheV1::new();
        let mut head_replay_cache = GovernanceDagRequestAuthenticationReplayCacheV1::new();
        for (scope, method, url, headers, body, handle, nonce) in cases {
            let request = GovernanceDagCanonicalRequestV1::try_from_http_parts(
                scope,
                method,
                url,
                headers,
                body,
                1024 * 1024,
            )
            .expect("canonical inbound request");
            let envelope =
                signed_test_request_auth_envelope(handle, &request, now, now + 15, nonce);
            let mut headers = request_auth_header_fields(&envelope);
            headers.push((
                "content-length".to_owned(),
                body.len().to_string().into_bytes(),
            ));
            headers.push(("cache-control".to_owned(), b"no-store".to_vec()));
            headers.push(("x-request-id".to_owned(), b"public-request-id".to_vec()));
            let (policy, replay_cache) = match scope {
                GovernanceDagAuthenticationScope::Ipfs => (&ipfs_policy, &mut ipfs_replay_cache),
                GovernanceDagAuthenticationScope::SignedHead => {
                    (&head_policy, &mut head_replay_cache)
                }
            };
            verify_request_before_test_backend(
                &request,
                &headers,
                body,
                scope,
                policy,
                now,
                replay_cache,
                &backend_calls,
            )
            .expect("verified request reaches the test backend");
        }
        assert_eq!(backend_calls.load(AtomicOrdering::SeqCst), 4);
    }

    #[test]
    fn inbound_request_auth_header_mapping_is_an_exact_hard_cut() {
        let now = 1_700_000_000;
        let request = canonical_test_request(
            GovernanceDagAuthenticationScope::Ipfs,
            "POST",
            "https://example.invalid/api/v0/pin/add?arg=cid&recursive=true",
            &[("accept-encoding", "identity")],
            b"",
        );
        let envelope = signed_test_request_auth_envelope(
            TEST_IPFS_AUTH_HANDLE,
            &request,
            now,
            now + 15,
            [0xab; 32],
        );
        let canonical = request_auth_header_fields(&envelope);
        let parsed = parse_governance_dag_request_authentication_headers_v1(
            canonical
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_slice()))
                .chain(std::iter::once(("accept-encoding", b"identity".as_slice()))),
        )
        .expect("ignore ordinary headers and parse the exact auth header set");
        assert_eq!(parsed, envelope);
        let policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
            test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE),
            30,
            5,
        )
        .expect("valid IPFS receiver policy");
        let mut zero_bound_cache = GovernanceDagRequestAuthenticationReplayCacheV1::new();
        let zero_bound_error = GovernanceDagHttpRequestReceiverV1::try_new(
            GovernanceDagAuthenticationScope::Ipfs,
            0,
            &policy,
            &mut zero_bound_cache,
        )
        .expect_err("receiver must reject a zero body ceiling");
        assert_eq!(
            zero_bound_error,
            GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest
        );
        let backend_calls = AtomicU64::new(0);

        let mut missing = canonical.clone();
        missing.remove(0);
        let cases = [
            (
                missing,
                GovernanceDagRequestAuthenticationErrorV1::MissingHeader,
            ),
            (
                {
                    let mut fields = canonical.clone();
                    fields.push(canonical[0].clone());
                    fields
                },
                GovernanceDagRequestAuthenticationErrorV1::DuplicateHeader,
            ),
            (
                {
                    let mut fields = canonical.clone();
                    fields.push(("x-sorafs-governance-auth-key".to_owned(), vec![b'a'; 64]));
                    fields
                },
                GovernanceDagRequestAuthenticationErrorV1::UnknownHeader,
            ),
            (
                {
                    let mut fields = canonical.clone();
                    fields.push((
                        "x-sorafs-governance-auth-extension".to_owned(),
                        b"1".to_vec(),
                    ));
                    fields
                },
                GovernanceDagRequestAuthenticationErrorV1::UnknownHeader,
            ),
            (
                {
                    let mut fields = canonical.clone();
                    fields.push(("X-Sorafs-Governance-Auth-Version".to_owned(), b"1".to_vec()));
                    fields
                },
                GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader,
            ),
        ];
        for (fields, expected) in cases {
            let error = verify_request_before_test_backend(
                &request,
                &fields,
                b"",
                GovernanceDagAuthenticationScope::Ipfs,
                &policy,
                now,
                &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
                &backend_calls,
            )
            .expect_err("noncanonical header map must stop before backend dispatch");
            assert_eq!(error, expected);
        }

        for (index, value) in [
            (0, b"01".to_vec()),
            (1, b"IPFS".to_vec()),
            (2, b"01".to_vec()),
            (4, "AA".repeat(32).into_bytes()),
            (5, b"00".to_vec()),
        ] {
            let mut fields = canonical.clone();
            fields[index].1 = value;
            let error = verify_request_before_test_backend(
                &request,
                &fields,
                b"",
                GovernanceDagAuthenticationScope::Ipfs,
                &policy,
                now,
                &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
                &backend_calls,
            )
            .expect_err("noncanonical header value must stop before backend dispatch");
            assert_eq!(
                error,
                GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader
            );
        }
        assert_eq!(
            backend_calls.load(AtomicOrdering::SeqCst),
            0,
            "no header-mapping failure may reach the backend"
        );
    }

    #[test]
    fn inbound_request_auth_binds_every_request_part_before_backend_dispatch() {
        let now = 1_700_000_000;
        let request = canonical_test_request(
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head?revision=7",
            &[
                ("accept-encoding", "identity"),
                ("content-type", "application/vnd.iroha.norito"),
                ("if-match", "\"v7\""),
            ],
            b"head-v7",
        );
        let envelope = signed_test_request_auth_envelope(
            TEST_HEAD_AUTH_HANDLE,
            &request,
            now,
            now + 15,
            [0x61; 32],
        );
        let headers = request_auth_header_fields(&envelope);
        let policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
            test_request_auth_public_key(TEST_HEAD_AUTH_HANDLE),
            30,
            5,
        )
        .expect("valid signed-head receiver policy");
        let tampered = [
            canonical_test_request(
                GovernanceDagAuthenticationScope::Ipfs,
                "PUT",
                "https://example.invalid/governance/head?revision=7",
                &[
                    ("accept-encoding", "identity"),
                    ("content-type", "application/vnd.iroha.norito"),
                    ("if-match", "\"v7\""),
                ],
                b"head-v7",
            ),
            canonical_test_request(
                GovernanceDagAuthenticationScope::SignedHead,
                "POST",
                "https://example.invalid/governance/head?revision=7",
                &[
                    ("accept-encoding", "identity"),
                    ("content-type", "application/vnd.iroha.norito"),
                    ("if-match", "\"v7\""),
                ],
                b"head-v7",
            ),
            canonical_test_request(
                GovernanceDagAuthenticationScope::SignedHead,
                "PUT",
                "https://example.invalid/governance/head?revision=8",
                &[
                    ("accept-encoding", "identity"),
                    ("content-type", "application/vnd.iroha.norito"),
                    ("if-match", "\"v7\""),
                ],
                b"head-v7",
            ),
            canonical_test_request(
                GovernanceDagAuthenticationScope::SignedHead,
                "PUT",
                "https://example.invalid/governance/head?revision=7",
                &[
                    ("accept-encoding", "identity"),
                    ("content-type", "application/vnd.iroha.norito"),
                    ("if-match", "\"v6\""),
                ],
                b"head-v7",
            ),
            canonical_test_request(
                GovernanceDagAuthenticationScope::SignedHead,
                "PUT",
                "https://example.invalid/governance/head?revision=7",
                &[
                    ("accept-encoding", "identity"),
                    ("content-type", "application/vnd.iroha.norito"),
                ],
                b"head-v7",
            ),
            canonical_test_request(
                GovernanceDagAuthenticationScope::SignedHead,
                "PUT",
                "https://example.invalid/governance/head?revision=7",
                &[
                    ("accept-encoding", "identity"),
                    ("content-type", "application/vnd.iroha.norito"),
                    ("if-match", "\"v7\""),
                ],
                b"HEAD-v7",
            ),
            GovernanceDagCanonicalRequestV1::try_new(
                GovernanceDagAuthenticationScope::SignedHead,
                "PUT",
                "https://example.invalid/governance/head?revision=7",
                request.selected_headers().to_vec(),
                request.body_length().saturating_add(1),
                request.body_blake3(),
                1024 * 1024,
            )
            .expect("canonical body-length tamper descriptor"),
        ];
        let backend_calls = AtomicU64::new(0);
        for (index, tampered_request) in tampered.iter().enumerate() {
            let body = if index == 5 {
                b"HEAD-v7".as_slice()
            } else {
                b"head-v7".as_slice()
            };
            let error = verify_request_before_test_backend(
                tampered_request,
                &headers,
                body,
                GovernanceDagAuthenticationScope::SignedHead,
                &policy,
                now,
                &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
                &backend_calls,
            )
            .expect_err("tampered request must stop before backend dispatch");
            assert_eq!(
                error,
                GovernanceDagRequestAuthenticationErrorV1::RequestMismatch
            );
        }
        let error = verify_request_before_test_backend(
            &request,
            &headers,
            b"head-v7",
            GovernanceDagAuthenticationScope::Ipfs,
            &policy,
            now,
            &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
            &backend_calls,
        )
        .expect_err("wrong receiver scope must stop before backend dispatch");
        assert_eq!(
            error,
            GovernanceDagRequestAuthenticationErrorV1::RequestMismatch
        );
        let wrong_key_policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
            test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE),
            30,
            5,
        )
        .expect("alternate valid receiver key");
        let error = verify_request_before_test_backend(
            &request,
            &headers,
            b"head-v7",
            GovernanceDagAuthenticationScope::SignedHead,
            &wrong_key_policy,
            now,
            &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
            &backend_calls,
        )
        .expect_err("wrong pinned key must stop before backend dispatch");
        assert_eq!(
            error,
            GovernanceDagRequestAuthenticationErrorV1::RequestMismatch
        );
        assert_eq!(
            backend_calls.load(AtomicOrdering::SeqCst),
            0,
            "no binding failure may reach the backend"
        );
    }

    #[test]
    fn inbound_request_auth_rejects_time_nonce_signature_and_replay_failures() {
        let now = 1_700_000_000;
        let request = canonical_test_request(
            GovernanceDagAuthenticationScope::Ipfs,
            "GET",
            "https://example.invalid/api/v0/cat?arg=cid",
            &[("accept-encoding", "identity")],
            b"",
        );
        let policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
            test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE),
            30,
            5,
        )
        .expect("valid IPFS receiver policy");
        let backend_calls = AtomicU64::new(0);
        for (issued_at, expires_at, nonce) in [
            (now - 20, now - 1, [0x71; 32]),
            (now + 6, now + 16, [0x72; 32]),
            (now, now + 31, [0x73; 32]),
        ] {
            let envelope = signed_test_request_auth_envelope(
                TEST_IPFS_AUTH_HANDLE,
                &request,
                issued_at,
                expires_at,
                nonce,
            );
            let error = verify_request_before_test_backend(
                &request,
                &request_auth_header_fields(&envelope),
                b"",
                GovernanceDagAuthenticationScope::Ipfs,
                &policy,
                now,
                &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
                &backend_calls,
            )
            .expect_err("invalid timing must stop before backend dispatch");
            assert_eq!(
                error,
                GovernanceDagRequestAuthenticationErrorV1::InvalidTiming
            );
        }

        let valid = signed_test_request_auth_envelope(
            TEST_IPFS_AUTH_HANDLE,
            &request,
            now,
            now + 15,
            [0x74; 32],
        );
        let mut zero_nonce_headers = request_auth_header_fields(&valid);
        zero_nonce_headers[4].1 = "00".repeat(32).into_bytes();
        let error = verify_request_before_test_backend(
            &request,
            &zero_nonce_headers,
            b"",
            GovernanceDagAuthenticationScope::Ipfs,
            &policy,
            now,
            &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
            &backend_calls,
        )
        .expect_err("zero nonce must stop before backend dispatch");
        assert_eq!(
            error,
            GovernanceDagRequestAuthenticationErrorV1::MalformedEnvelope
        );

        let mut bad_signature_headers = request_auth_header_fields(&valid);
        let mut invalid_signature = valid.signature();
        invalid_signature[32..].fill(0);
        bad_signature_headers[7].1 = hex::encode(invalid_signature).into_bytes();
        let error = verify_request_before_test_backend(
            &request,
            &bad_signature_headers,
            b"",
            GovernanceDagAuthenticationScope::Ipfs,
            &policy,
            now,
            &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
            &backend_calls,
        )
        .expect_err("invalid signature must stop before backend dispatch");
        assert_eq!(
            error,
            GovernanceDagRequestAuthenticationErrorV1::SignatureVerification
        );
        assert_eq!(backend_calls.load(AtomicOrdering::SeqCst), 0);

        let headers = request_auth_header_fields(&valid);
        let mut replay_cache = GovernanceDagRequestAuthenticationReplayCacheV1::new();
        verify_request_before_test_backend(
            &request,
            &headers,
            b"",
            GovernanceDagAuthenticationScope::Ipfs,
            &policy,
            now,
            &mut replay_cache,
            &backend_calls,
        )
        .expect("first nonce use reaches backend");
        let error = verify_request_before_test_backend(
            &request,
            &headers,
            b"",
            GovernanceDagAuthenticationScope::Ipfs,
            &policy,
            now,
            &mut replay_cache,
            &backend_calls,
        )
        .expect_err("replayed nonce must stop before backend dispatch");
        assert_eq!(error, GovernanceDagRequestAuthenticationErrorV1::Replay);
        assert_eq!(
            backend_calls.load(AtomicOrdering::SeqCst),
            1,
            "replay rejection must not invoke the backend again"
        );

        let second = signed_test_request_auth_envelope(
            TEST_IPFS_AUTH_HANDLE,
            &request,
            now,
            now + 15,
            [0x75; 32],
        );
        let mut bounded_cache =
            GovernanceDagRequestAuthenticationReplayCacheV1::try_with_capacity(1)
                .expect("one-entry replay cache");
        let capacity_backend_calls = AtomicU64::new(0);
        verify_request_before_test_backend(
            &request,
            &headers,
            b"",
            GovernanceDagAuthenticationScope::Ipfs,
            &policy,
            now,
            &mut bounded_cache,
            &capacity_backend_calls,
        )
        .expect("first live nonce fits bounded cache");
        let error = verify_request_before_test_backend(
            &request,
            &request_auth_header_fields(&second),
            b"",
            GovernanceDagAuthenticationScope::Ipfs,
            &policy,
            now,
            &mut bounded_cache,
            &capacity_backend_calls,
        )
        .expect_err("full live replay cache must fail closed");
        assert_eq!(
            error,
            GovernanceDagRequestAuthenticationErrorV1::ReplayCacheFull
        );
        assert_eq!(capacity_backend_calls.load(AtomicOrdering::SeqCst), 1);
    }

    #[test]
    fn inbound_receiver_rejects_framing_before_replay_consumption_or_dispatch() {
        let now = 1_700_000_000;
        let body = b"canonical-head";
        let request = canonical_test_request(
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head",
            &[
                ("accept-encoding", "identity"),
                ("content-type", "application/vnd.iroha.norito"),
                ("if-match", "\"v7\""),
            ],
            body,
        );
        let envelope = signed_test_request_auth_envelope(
            TEST_HEAD_AUTH_HANDLE,
            &request,
            now,
            now + 15,
            [0x76; 32],
        );
        let mut ambiguous_headers = request_auth_header_fields(&envelope);
        ambiguous_headers.push(("transfer-encoding".to_owned(), b"chunked".to_vec()));
        let policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
            test_request_auth_public_key(TEST_HEAD_AUTH_HANDLE),
            30,
            5,
        )
        .expect("valid signed-head receiver policy");
        let mut replay_cache = GovernanceDagRequestAuthenticationReplayCacheV1::new();
        let backend_calls = AtomicU64::new(0);
        let error = verify_request_before_test_backend(
            &request,
            &ambiguous_headers,
            body,
            GovernanceDagAuthenticationScope::SignedHead,
            &policy,
            now,
            &mut replay_cache,
            &backend_calls,
        )
        .expect_err("ambiguous framing must stop before verification and dispatch");
        assert_eq!(
            error,
            GovernanceDagRequestAuthenticationErrorV1::InvalidFraming
        );
        assert_eq!(backend_calls.load(AtomicOrdering::SeqCst), 0);

        verify_request_before_test_backend(
            &request,
            &request_auth_header_fields(&envelope),
            body,
            GovernanceDagAuthenticationScope::SignedHead,
            &policy,
            now,
            &mut replay_cache,
            &backend_calls,
        )
        .expect("same nonce remains usable after pre-verification framing rejection");
        assert_eq!(backend_calls.load(AtomicOrdering::SeqCst), 1);
    }

    #[test]
    fn canonical_request_hard_cut_rejects_credentials_aliases_and_bounds() {
        assert!(
            GovernanceDagCanonicalRequestHeaderV1::try_new(
                "authorization",
                "Bearer must-not-escape"
            )
            .is_err()
        );
        assert!(
            GovernanceDagCanonicalRequestHeaderV1::try_new("cookie", "session=secret").is_err()
        );
        assert!(GovernanceDagCanonicalRequestHeaderV1::try_new("content-type", " value").is_err());
        let duplicate = vec![
            GovernanceDagCanonicalRequestHeaderV1::try_new("accept-encoding", "identity")
                .expect("first header"),
            GovernanceDagCanonicalRequestHeaderV1::try_new("accept-encoding", "identity")
                .expect("duplicate header"),
        ];
        assert!(
            GovernanceDagCanonicalRequestV1::try_new(
                GovernanceDagAuthenticationScope::Ipfs,
                "POST",
                "https://example.invalid/api/v0/cat?arg=cid",
                duplicate,
                0,
                blake3_array(b""),
                1024,
            )
            .is_err()
        );
        assert!(
            GovernanceDagCanonicalRequestV1::try_new(
                GovernanceDagAuthenticationScope::Ipfs,
                "GET",
                "https://example.invalid/",
                Vec::new(),
                0,
                [0x55; 32],
                1024,
            )
            .is_err()
        );
        assert!(
            GovernanceDagCanonicalRequestV1::try_new(
                GovernanceDagAuthenticationScope::Ipfs,
                "PATCH",
                "https://example.invalid/",
                Vec::new(),
                0,
                blake3_array(b""),
                1024,
            )
            .is_err()
        );
        assert!(
            GovernanceDagCanonicalRequestV1::try_new(
                GovernanceDagAuthenticationScope::Ipfs,
                "POST",
                "https://example.invalid/",
                Vec::new(),
                1025,
                blake3_array(&[0; 1025]),
                1024,
            )
            .is_err()
        );
        for noncanonical_url in [
            "/api/v0/cat?arg=cid",
            "https://user@example.invalid/api/v0/cat?arg=cid",
            "https://example.invalid/api/v0/cat?z=1&a=2",
            "https://example.invalid/api/v0/cat?arg=%2f",
            "https://example.invalid/api/%41",
            "https://example.invalid/api/v0/cat?arg=cid#fragment",
        ] {
            assert!(
                GovernanceDagCanonicalRequestV1::try_new(
                    GovernanceDagAuthenticationScope::Ipfs,
                    "GET",
                    noncanonical_url,
                    Vec::new(),
                    0,
                    blake3_array(b""),
                    1024,
                )
                .is_err(),
                "{noncanonical_url} must fail the canonical URL hard cut"
            );
        }
        assert!(
            GovernanceDagRequestAuthenticationEnvelopeV1::try_new(
                &canonical_test_request(
                    GovernanceDagAuthenticationScope::Ipfs,
                    "GET",
                    "https://example.invalid/",
                    &[],
                    b"",
                ),
                0,
                1,
                [0; 32],
                [0; 32],
                [0; 64],
            )
            .is_err()
        );

        let client = Client::builder().no_proxy().build().expect("test client");
        let credential_request = client
            .get("https://example.invalid/")
            .header(header::AUTHORIZATION, "Bearer must-not-escape")
            .build()
            .expect("build credential-bearing request");
        assert!(
            canonical_outbound_request_descriptor(
                &credential_request,
                GovernanceDagAuthenticationScope::Ipfs,
                1024,
            )
            .is_err()
        );
        let unsorted_query = client
            .get("https://example.invalid/?z=1&a=2")
            .build()
            .expect("build noncanonical query request");
        assert!(
            canonical_outbound_request_descriptor(
                &unsorted_query,
                GovernanceDagAuthenticationScope::Ipfs,
                1024,
            )
            .is_err()
        );
    }

    #[test]
    fn outbound_descriptor_binds_only_selected_public_headers() {
        let body = b"canonical-body";
        let baseline_headers = [
            ("accept-encoding", b"identity".as_slice()),
            ("content-type", b"application/vnd.iroha.norito".as_slice()),
        ];
        let baseline = canonicalize_governance_dag_outbound_http_request_v1(
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head",
            baseline_headers,
            body,
            1024,
        )
        .expect("canonical baseline descriptor");
        let with_ordinary_headers = [
            ("accept-encoding", b"identity".as_slice()),
            ("content-type", b"application/vnd.iroha.norito".as_slice()),
            ("content-length", b"14".as_slice()),
            ("cache-control", b"no-cache".as_slice()),
            ("x-request-id", b"request-7".as_slice()),
        ];
        let with_ordinary = canonicalize_governance_dag_outbound_http_request_v1(
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head",
            with_ordinary_headers,
            body,
            1024,
        )
        .expect("ordinary public headers are safely excluded");
        assert_eq!(with_ordinary, baseline);

        let changed_selected = canonicalize_governance_dag_outbound_http_request_v1(
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head",
            [
                ("accept-encoding", b"gzip".as_slice()),
                ("content-type", b"application/vnd.iroha.norito".as_slice()),
                ("content-length", b"14".as_slice()),
                ("cache-control", b"no-cache".as_slice()),
                ("x-request-id", b"request-7".as_slice()),
            ],
            body,
            1024,
        )
        .expect("alternate selected public header remains canonical");
        assert_ne!(
            changed_selected.request_digest(),
            baseline.request_digest(),
            "a selected public header must change the signed request digest"
        );
    }

    #[test]
    fn outbound_descriptor_rejects_credentials_auth_prefixes_and_ambiguous_framing() {
        for forbidden_name in [
            "authorization",
            "Proxy-Authorization",
            "cookie",
            "x-api-key",
            "x-auth-token",
            "x-sorafs-governance-auth-version",
            "X-Sorafs-Governance-Auth-Extension",
        ] {
            let error = canonicalize_governance_dag_outbound_http_request_v1(
                GovernanceDagAuthenticationScope::Ipfs,
                "GET",
                "https://example.invalid/api/v0/cat?arg=cid",
                [(forbidden_name, b"must-not-pass".as_slice())],
                b"",
                1024,
            )
            .expect_err("credential and authentication-prefix headers must fail closed");
            assert_eq!(
                error,
                GovernanceDagRequestAuthenticationErrorV1::ForbiddenHeader,
                "unexpected rejection for {forbidden_name}"
            );
        }

        let framing_cases = [
            vec![("content-length", b"13".as_slice())],
            vec![("content-length", b"014".as_slice())],
            vec![
                ("content-length", b"14".as_slice()),
                ("content-length", b"14".as_slice()),
            ],
            vec![("content-length", b"14, 14".as_slice())],
            vec![("Content-Length", b"14".as_slice())],
            vec![("transfer-encoding", b"chunked".as_slice())],
            vec![("Transfer-Encoding", b"identity".as_slice())],
        ];
        for headers in framing_cases {
            let error = canonicalize_governance_dag_outbound_http_request_v1(
                GovernanceDagAuthenticationScope::SignedHead,
                "PUT",
                "https://example.invalid/governance/head",
                headers,
                b"canonical-body",
                1024,
            )
            .expect_err("ambiguous HTTP framing must fail closed");
            assert_eq!(
                error,
                GovernanceDagRequestAuthenticationErrorV1::InvalidFraming
            );
        }
    }

    #[test]
    fn public_auth_headers_preserve_final_body_and_conditional_headers() {
        let client = Client::builder().no_proxy().build().expect("test client");
        let mut request = client
            .put("https://example.invalid/governance/head")
            .header(header::ACCEPT_ENCODING, "identity")
            .header(header::CONTENT_TYPE, "application/vnd.iroha.norito")
            .header(header::IF_MATCH, "\"v7\"")
            .body(b"canonical-head".to_vec())
            .build()
            .expect("build final signed-head PUT");
        let descriptor = canonical_outbound_request_descriptor(
            &request,
            GovernanceDagAuthenticationScope::SignedHead,
            1024,
        )
        .expect("canonical final signed-head descriptor");
        let now = current_unix_timestamp_seconds();
        let envelope = signed_test_request_auth_envelope(
            TEST_HEAD_AUTH_HANDLE,
            &descriptor,
            now,
            now + 15,
            [0x44; 32],
        );
        attach_request_authentication_headers(&mut request, &envelope)
            .expect("attach fixed public authentication headers");
        assert_eq!(
            request
                .body()
                .and_then(reqwest::Body::as_bytes)
                .expect("byte body"),
            b"canonical-head"
        );
        assert_eq!(
            request.headers().get(header::IF_MATCH),
            Some(&HeaderValue::from_static("\"v7\""))
        );
        assert!(request.headers().get(header::AUTHORIZATION).is_none());
        assert!(request.headers().get(header::COOKIE).is_none());
        for name in GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1 {
            assert!(
                request.headers().contains_key(name),
                "missing fixed public request-auth header {name}"
            );
        }
    }

    #[tokio::test]
    async fn authenticated_execute_discards_response_after_qualification_drift() {
        let provider = Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "in-flight-secret-token",
        ));
        let authenticator = OpaqueAuthenticator::try_new(
            TEST_IPFS_AUTH_HANDLE,
            TEST_AUTH_QUALIFICATION,
            provider.public_key(),
            30,
            5,
            provider.clone(),
            "IPFS authenticator",
        )
        .expect("bind stable runtime authenticator");
        let router = Router::new()
            .route("/drift", get(mock_authenticator_drift))
            .with_state(provider);
        let (endpoint, task) = spawn_router_with_authenticator(
            router,
            "/drift",
            GovernanceDagAuthenticationScope::Ipfs,
            authenticator,
        )
        .await;
        let request = endpoint
            .request(Method::GET, endpoint.url.clone())
            .expect("construct drift request");
        let error = endpoint
            .execute(request, "drift request failed")
            .await
            .expect_err("post-execute policy drift must discard the response");
        assert!(error.to_string().contains("policy changed"));
        assert!(!error.to_string().contains("in-flight-secret-token"));
        task.abort();
    }

    #[tokio::test]
    async fn runtime_registry_injection_reaches_startup_with_exact_bindings() {
        let root = secure_temp_dir();
        let view = runtime_boundary_view(root.path());
        let state_dir = view
            .service
            .state_dir
            .clone()
            .expect("test state directory");
        let registry = Arc::new(TestRuntimeProviderRegistry::returning(
            test_runtime_providers(Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE))),
        ));
        let runtime_registry: Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1> =
            registry.clone();
        let providers = resolve_runtime_registry_providers(&view, Some(runtime_registry))
            .expect("registry resolves the configured providers");
        let _service = Service::from_view(view, providers)
            .await
            .expect("registry providers reach qualified service startup");

        let observed = registry
            .observed_bindings
            .lock()
            .expect("lock observed registry bindings")
            .clone()
            .expect("registry was called");
        assert_eq!(observed.ipfs_authenticator_handle(), TEST_IPFS_AUTH_HANDLE);
        assert_eq!(
            observed.ipfs_authenticator_qualification(),
            TEST_AUTH_QUALIFICATION
        );
        assert_eq!(
            observed.head_authenticator_handle(),
            Some(TEST_HEAD_AUTH_HANDLE)
        );
        assert_eq!(
            observed.head_authenticator_qualification(),
            Some(TEST_AUTH_QUALIFICATION)
        );
        assert_eq!(
            observed.checkpoint_store_handle(),
            TEST_CHECKPOINT_STORE_HANDLE
        );
        assert_eq!(
            observed.checkpoint_store_qualification(),
            TEST_STORE_QUALIFICATION
        );
        assert!(state_dir.exists());
    }

    #[test]
    fn embedding_launcher_preflight_qualifies_adapters_without_opening_state() {
        let root = secure_temp_dir();
        let view = runtime_boundary_view(root.path());
        let state_dir = view
            .service
            .state_dir
            .clone()
            .expect("test state directory");
        validate_governance_dag_service_runtime_providers(
            &view,
            &test_runtime_providers(Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE))),
        )
        .expect("qualify the exact deployment adapter set");
        assert!(
            !state_dir.exists(),
            "provider-only launcher preflight must not open mutable state"
        );

        let error = validate_governance_dag_service_runtime_providers(
            &view,
            &GovernanceDagServiceRuntimeProviders::default(),
        )
        .expect_err("missing providers must fail launcher preflight");
        assert!(error.to_string().contains("no runtime provider"));
        assert!(!state_dir.exists());

        let error = validate_governance_dag_service_runtime_providers(
            &view,
            &test_runtime_providers(Arc::new(TestSealedStore::new(
                "kms:governance/checkpoint:test",
            ))),
        )
        .expect_err("test-marked provider must fail launcher preflight");
        assert!(error.to_string().contains("test-marked"));
        assert!(!state_dir.exists());
    }

    #[tokio::test]
    async fn prepare_reconciles_initial_state_without_publication() {
        let root = secure_temp_dir();
        let mut view = runtime_boundary_view(root.path());
        let mut source = signed_source(1, 0x6d, current_unix_timestamp_seconds().saturating_sub(1));
        materialize_source_snapshot(
            view.source_dir.as_deref().expect("test source directory"),
            &mut source,
        );
        let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
        view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
        view.service.publisher_public_key_hex = Some(publisher_key_hex);
        view.service.allow_head_bootstrap = true;

        let (head_endpoint, head_state, task) = spawn_signed_head(SignedHeadInner::default()).await;
        view.service.signed_head_url = Some(head_endpoint.url.to_string());
        let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        seed_producer_checkpoint(
            &checkpoint_provider,
            view.source_dir.as_deref().expect("test source directory"),
            &source,
        );

        let runner = prepare_governance_dag_service_from_view(
            view.clone(),
            test_runtime_providers(checkpoint_provider.clone()),
        )
        .await
        .expect("empty authenticated state may prepare for an allowed bootstrap");
        assert!(runner.service.checkpoint.is_none());
        assert!(runner.service.intent.is_none());
        assert_eq!(head_state.0.lock().await.put_count, 0);
        drop(runner);

        let checkpoint = checkpoint_from_source(&source);
        save_checkpoint(
            &test_checkpoint_store(checkpoint_provider.clone()),
            None,
            &checkpoint,
        )
        .expect("seed authenticated checkpoint");
        let error = prepare_governance_dag_service_from_view(
            view,
            test_runtime_providers(checkpoint_provider),
        )
        .await
        .err()
        .expect("a missing public head cannot satisfy an existing checkpoint");
        assert!(error.to_string().contains("public head disappeared"));
        assert_eq!(
            head_state.0.lock().await.put_count,
            0,
            "prepare must not repair or publish the public head"
        );
        task.abort();
    }

    #[tokio::test]
    async fn prepare_rejects_source_conflicting_publish_intent_before_publication() {
        let root = secure_temp_dir();
        let mut view = runtime_boundary_view(root.path());
        let now = current_unix_timestamp_seconds().saturating_sub(1);
        let mut source = signed_source(1, 0x6e, now);
        materialize_source_snapshot(
            view.source_dir.as_deref().expect("test source directory"),
            &mut source,
        );
        let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
        view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
        view.service.publisher_public_key_hex = Some(publisher_key_hex);
        view.service.allow_head_bootstrap = true;

        let (head_endpoint, head_state, task) = spawn_signed_head(SignedHeadInner::default()).await;
        view.service.signed_head_url = Some(head_endpoint.url.to_string());
        let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        seed_producer_checkpoint(
            &checkpoint_provider,
            view.source_dir.as_deref().expect("test source directory"),
            &source,
        );
        let conflicting_source = signed_source(1, 0x6f, now);
        save_publish_intent(
            &test_checkpoint_store(checkpoint_provider.clone()),
            None,
            &intent_from_source(&conflicting_source),
        )
        .expect("seed independently valid but source-conflicting intent");

        let error = prepare_governance_dag_service_from_view(
            view,
            test_runtime_providers(checkpoint_provider),
        )
        .await
        .err()
        .expect("prepare must reconcile the durable intent against the source");
        assert!(
            error.to_string().contains("source forked")
                || error.to_string().contains("incompatible with the source")
        );
        assert_eq!(head_state.0.lock().await.put_count, 0);
        task.abort();
    }

    #[tokio::test]
    async fn sealed_producer_intent_blocks_all_publication_io_before_checkpoint_commit() {
        let root = secure_temp_dir();
        let mut view = runtime_boundary_view(root.path());
        let now = current_unix_timestamp_seconds().saturating_sub(2);
        let previous_source = signed_source(1, 0x70, now);
        let mut visible_uncommitted_source = signed_source(2, 0x70, now);
        let source_dir = view.source_dir.as_deref().expect("test source directory");
        materialize_source_snapshot(source_dir, &mut visible_uncommitted_source);
        let publisher_key_hex =
            hex::encode(&visible_uncommitted_source.head.head_signature.public_key);
        view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
        view.service.publisher_public_key_hex = Some(publisher_key_hex);

        let request_count = Arc::new(AtomicU64::new(0));
        let router = Router::new()
            .fallback(any(count_unexpected_publication_io))
            .with_state(request_count.clone());
        let (endpoint, task) = spawn_router(router, "/").await;
        view.service.ipfs_api_url = Some(endpoint.url.to_string());
        view.service.signed_head_url = Some(endpoint.url.to_string());

        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let producer_checkpoint = seed_producer_checkpoint(&provider, source_dir, &previous_source);
        let intent = GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::ProducerPublishIntent,
            producer_checkpoint.generation.saturating_add(1),
            vec![0xA5],
        );
        provider
            .compare_and_swap(
                GovernanceDagSealedStateSlot::ProducerPublishIntent,
                None,
                intent,
            )
            .expect("pause producer after sealing its intent");

        let mut service = Service::from_view(view, test_runtime_providers(provider))
            .await
            .expect("construct service without performing public I/O");
        let error = service
            .reconcile_once()
            .await
            .expect_err("uncommitted producer transaction must block reconciliation");
        assert!(error.to_string().contains("active sealed publish intent"));
        assert_eq!(
            request_count.load(AtomicOrdering::SeqCst),
            0,
            "service must perform no Kubo or public-head I/O before producer checkpoint commit"
        );
        task.abort();
    }

    #[tokio::test]
    async fn substituted_producer_binding_fails_before_all_publication_io() {
        let request_count = Arc::new(AtomicU64::new(0));
        let router = Router::new()
            .fallback(any(count_unexpected_publication_io))
            .with_state(request_count.clone());
        let (endpoint, task) = spawn_router(router, "/").await;

        for substitution in ["handle", "revision", "policy", "peer", "key"] {
            let root = secure_temp_dir();
            let mut view = runtime_boundary_view(root.path());
            let mut source =
                signed_source(1, 0x78, current_unix_timestamp_seconds().saturating_sub(1));
            let source_dir = view.source_dir.as_deref().expect("test source directory");
            materialize_source_snapshot(source_dir, &mut source);
            let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
            view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
            view.service.publisher_public_key_hex = Some(publisher_key_hex);
            view.service.ipfs_api_url = Some(endpoint.url.to_string());
            view.service.signed_head_url = Some(endpoint.url.to_string());

            let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
            let mut checkpoint = producer_checkpoint_from_source(source_dir, &source);
            match substitution {
                "handle" => {
                    checkpoint.signer_handle = "hsm:governance/source-signer:alternate".to_owned();
                }
                "revision" => checkpoint.signer_revision = 2,
                "policy" => checkpoint.signer_policy_digest = [0x84; 32],
                "peer" => checkpoint.publisher_peer_id = b"12D3KooWGovernanceAlternate".to_vec(),
                "key" => checkpoint.publisher_public_key = [0x55; 32],
                _ => unreachable!("enumerated producer substitution"),
            }
            let record = GovernanceDagSealedStateRecord::new(
                GovernanceDagSealedStateSlot::ProducerCheckpoint,
                checkpoint.block_count.saturating_add(1),
                norito::to_bytes(&checkpoint).expect("encode substituted producer checkpoint"),
            );
            provider
                .compare_and_swap(
                    GovernanceDagSealedStateSlot::ProducerCheckpoint,
                    None,
                    record,
                )
                .expect("seed substituted producer checkpoint");

            let mut service = Service::from_view(view, test_runtime_providers(provider))
                .await
                .expect("construct service without performing public I/O");
            let error = service
                .reconcile_once()
                .await
                .expect_err("producer binding substitution must fail closed");
            assert!(
                error.to_string().contains("identity or generation"),
                "unexpected {substitution} substitution error: {error}"
            );
            assert_eq!(
                request_count.load(AtomicOrdering::SeqCst),
                0,
                "{substitution} substitution reached Kubo or public-head I/O"
            );
        }
        task.abort();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn replaced_source_or_state_root_fails_before_all_publication_io() {
        let request_count = Arc::new(AtomicU64::new(0));
        let router = Router::new()
            .fallback(any(count_unexpected_publication_io))
            .with_state(request_count.clone());
        let (endpoint, task) = spawn_router(router, "/").await;

        for replaced_role in ["source", "state"] {
            let root = secure_temp_dir();
            let mut view = runtime_boundary_view(root.path());
            let mut source =
                signed_source(1, 0x79, current_unix_timestamp_seconds().saturating_sub(1));
            let source_dir = view
                .source_dir
                .clone()
                .expect("test source directory is configured");
            let state_dir = view
                .service
                .state_dir
                .clone()
                .expect("test state directory is configured");
            materialize_source_snapshot(&source_dir, &mut source);
            let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
            view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
            view.service.publisher_public_key_hex = Some(publisher_key_hex);
            view.service.ipfs_api_url = Some(endpoint.url.to_string());
            view.service.signed_head_url = Some(endpoint.url.to_string());

            let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
            seed_producer_checkpoint(&provider, &source_dir, &source);
            let mut service = Service::from_view(view, test_runtime_providers(provider))
                .await
                .expect("construct service with pinned source and state roots");
            let replaced = if replaced_role == "source" {
                source_dir
            } else {
                state_dir
            };
            let detached = root.path().join(format!("{replaced_role}.detached"));
            fs::rename(&replaced, &detached).expect("detach pinned service root");
            fs::create_dir(&replaced).expect("create replacement service root");
            fs::set_permissions(&replaced, fs::Permissions::from_mode(0o700))
                .expect("secure replacement service root");
            let marker = replaced.join("must-remain");
            fs::write(&marker, replaced_role.as_bytes()).expect("seed replacement marker");

            let error = service
                .reconcile_once()
                .await
                .expect_err("root replacement must fail before publication I/O");
            assert!(
                error.to_string().contains("root identity changed")
                    || error.to_string().contains("changed identity")
                    || error.to_string().contains("changed"),
                "unexpected {replaced_role} replacement error: {error}"
            );
            assert_eq!(
                fs::read(&marker).expect("replacement marker remains"),
                replaced_role.as_bytes()
            );
            assert_eq!(
                request_count.load(AtomicOrdering::SeqCst),
                0,
                "{replaced_role} replacement reached Kubo or public-head I/O"
            );
        }
        task.abort();
    }

    #[tokio::test]
    async fn service_rejects_configured_provider_qualification_substitution_before_state_access() {
        let root = secure_temp_dir();
        let mut view = runtime_boundary_view(root.path());
        let state_dir = view
            .service
            .state_dir
            .clone()
            .expect("test state directory");
        view.service.ipfs_authenticator_policy_digest = Some([0x99; 32]);

        let error = Service::from_view(
            view,
            test_runtime_providers(Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE))),
        )
        .await
        .err()
        .expect("substituted configured provider qualification must fail");
        assert!(
            error
                .to_string()
                .contains("qualification does not match configuration")
        );
        assert!(
            !state_dir.exists(),
            "qualification substitution must fail before mutable state is opened"
        );
    }

    #[tokio::test]
    async fn runtime_registry_failures_precede_service_state() {
        let root = secure_temp_dir();
        let view = runtime_boundary_view(root.path());
        let state_dir = view
            .service
            .state_dir
            .clone()
            .expect("test state directory");

        let missing = resolve_runtime_registry_providers(&view, None)
            .expect_err("missing registry must fail closed");
        assert!(matches!(
            missing,
            GovernanceDagServiceLauncherError::MissingRuntimeProviderRegistry
        ));
        assert!(!state_dir.exists());

        let stale_registry: Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1> =
            Arc::new(TestRuntimeProviderRegistry::failing(
                GovernanceDagServiceRuntimeProviderRegistryErrorV1::StaleOrRevoked,
            ));
        let stale = resolve_runtime_registry_providers(&view, Some(stale_registry))
            .expect_err("stale registry must fail closed");
        assert!(matches!(
            stale,
            GovernanceDagServiceLauncherError::RuntimeProviderRegistry(
                GovernanceDagServiceRuntimeProviderRegistryErrorV1::StaleOrRevoked
            )
        ));
        assert!(!state_dir.exists());

        let default_registry: Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1> = Arc::new(
            TestRuntimeProviderRegistry::returning(GovernanceDagServiceRuntimeProviders::default()),
        );
        let providers = resolve_runtime_registry_providers(&view, Some(default_registry))
            .expect("registry may return an incomplete set for service qualification to reject");
        let error = Service::from_view(view.clone(), providers)
            .await
            .err()
            .expect("empty provider set must fail startup");
        assert!(error.to_string().contains("no runtime provider"));
        assert!(!state_dir.exists());

        for provider_handle in [
            "kms:governance/checkpoint:other",
            "kms:governance/checkpoint:test",
        ] {
            let registry: Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1> =
                Arc::new(TestRuntimeProviderRegistry::returning(
                    test_runtime_providers(Arc::new(TestSealedStore::new(provider_handle))),
                ));
            let providers = resolve_runtime_registry_providers(&view, Some(registry))
                .expect("registry returns provider for startup qualification");
            let error = Service::from_view(view.clone(), providers)
                .await
                .err()
                .expect("substituted or test provider must fail startup");
            if provider_handle.ends_with(":test") {
                assert!(error.to_string().contains("test-marked"));
            } else {
                assert!(error.to_string().contains("does not match"));
            }
            assert!(!state_dir.exists());
        }
    }

    #[tokio::test]
    async fn service_fails_closed_when_runtime_providers_are_missing_or_mismatched() {
        let root = secure_temp_dir();
        let view = runtime_boundary_view(root.path());
        let state_dir = view
            .service
            .state_dir
            .clone()
            .expect("test state directory");

        let error = Service::from_view(
            view.clone(),
            GovernanceDagServiceRuntimeProviders::default(),
        )
        .await
        .err()
        .expect("missing sealed store must fail");
        assert!(error.to_string().contains("no runtime provider"));
        assert!(
            !state_dir.exists(),
            "missing provider must fail before mutable state is opened"
        );

        let mismatched_store = Arc::new(TestSealedStore::new("kms:governance/checkpoint:other"));
        let error = Service::from_view(
            view.clone(),
            GovernanceDagServiceRuntimeProviders {
                checkpoint_store: Some(mismatched_store),
                ..GovernanceDagServiceRuntimeProviders::default()
            },
        )
        .await
        .err()
        .expect("mismatched sealed store handle must fail");
        assert!(error.to_string().contains("does not match"));
        assert!(
            !state_dir.exists(),
            "substituted provider must fail before mutable state is opened"
        );

        let checkpoint_store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let error = Service::from_view(
            view.clone(),
            GovernanceDagServiceRuntimeProviders {
                checkpoint_store: Some(checkpoint_store.clone()),
                ..GovernanceDagServiceRuntimeProviders::default()
            },
        )
        .await
        .err()
        .expect("missing IPFS authenticator must fail");
        assert!(error.to_string().contains("IPFS authentication"));

        let error = Service::from_view(
            view,
            GovernanceDagServiceRuntimeProviders {
                checkpoint_store: Some(checkpoint_store),
                ipfs_authenticator: Some(Arc::new(TestAuthenticator::new(
                    TEST_IPFS_AUTH_HANDLE,
                    "test-only-ipfs",
                ))),
                head_authenticator: None,
            },
        )
        .await
        .err()
        .expect("missing signed-head authenticator must fail");
        assert!(error.to_string().contains("signed-head authentication"));
        assert!(!state_dir.exists());
    }

    #[tokio::test]
    async fn service_rejects_stale_providers_before_state_access() {
        let root = secure_temp_dir();
        let view = runtime_boundary_view(root.path());
        let state_dir = view
            .service
            .state_dir
            .clone()
            .expect("test state directory");
        let stale_store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        stale_store
            .qualification_refuse
            .store(true, AtomicOrdering::SeqCst);
        let error = Service::from_view(view.clone(), test_runtime_providers(stale_store))
            .await
            .err()
            .expect("stale sealed store must fail startup");
        let rendered = error.to_string();
        assert!(rendered.contains("stale"));
        assert!(!rendered.contains("must-never-escape"));
        assert!(
            !state_dir.exists(),
            "stale provider must fail before mutable state is opened"
        );

        let stale_ipfs = Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "test-only-ipfs",
        ));
        stale_ipfs
            .qualification_refuse
            .store(true, AtomicOrdering::SeqCst);
        let error = Service::from_view(
            view.clone(),
            GovernanceDagServiceRuntimeProviders {
                checkpoint_store: Some(Arc::new(TestSealedStore::new(
                    TEST_CHECKPOINT_STORE_HANDLE,
                ))),
                ipfs_authenticator: Some(stale_ipfs),
                head_authenticator: Some(Arc::new(TestAuthenticator::new(
                    TEST_HEAD_AUTH_HANDLE,
                    "test-only-head",
                ))),
            },
        )
        .await
        .err()
        .expect("stale IPFS authenticator must fail startup");
        assert!(error.to_string().contains("stale"));
        assert!(!state_dir.exists());

        let stale_head = Arc::new(TestAuthenticator::new(
            TEST_HEAD_AUTH_HANDLE,
            "test-only-head",
        ));
        stale_head
            .qualification_refuse
            .store(true, AtomicOrdering::SeqCst);
        let error = Service::from_view(
            view.clone(),
            GovernanceDagServiceRuntimeProviders {
                checkpoint_store: Some(Arc::new(TestSealedStore::new(
                    TEST_CHECKPOINT_STORE_HANDLE,
                ))),
                ipfs_authenticator: Some(Arc::new(TestAuthenticator::new(
                    TEST_IPFS_AUTH_HANDLE,
                    "test-only-ipfs",
                ))),
                head_authenticator: Some(stale_head),
            },
        )
        .await
        .err()
        .expect("stale signed-head authenticator must fail startup");
        assert!(error.to_string().contains("stale"));
        assert!(!state_dir.exists());
    }

    #[tokio::test]
    async fn service_rejects_test_marked_provider_before_state_access() {
        let root = secure_temp_dir();
        let view = runtime_boundary_view(root.path());
        let state_dir = view
            .service
            .state_dir
            .clone()
            .expect("test state directory");
        let mut test_marked_view = view;
        test_marked_view.service.checkpoint_store_handle =
            Some("kms:governance/checkpoint:test".to_owned());
        let error = Service::from_view(
            test_marked_view,
            GovernanceDagServiceRuntimeProviders {
                checkpoint_store: Some(Arc::new(TestSealedStore::new(
                    "kms:governance/checkpoint:test",
                ))),
                ipfs_authenticator: Some(Arc::new(TestAuthenticator::new(
                    TEST_IPFS_AUTH_HANDLE,
                    "test-only-ipfs",
                ))),
                head_authenticator: Some(Arc::new(TestAuthenticator::new(
                    TEST_HEAD_AUTH_HANDLE,
                    "test-only-head",
                ))),
            },
        )
        .await
        .err()
        .expect("test-marked provider handle must fail startup");
        assert!(error.to_string().contains("test-marked"));
        assert!(!state_dir.exists());
    }

    #[tokio::test]
    async fn service_rejects_in_process_sealed_checkpoint_rollback() {
        let root = secure_temp_dir();
        let view = runtime_boundary_view(root.path());
        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let store = test_checkpoint_store(provider.clone());
        let source = signed_source(1, 0x6A, 1_800_000_000);
        let mut checkpoint = checkpoint_from_source(&source);
        checkpoint.generation = 2;
        save_checkpoint(&store, None, &checkpoint).expect("seed generation-two checkpoint");

        let mut service = Service::from_view(view, test_runtime_providers(provider.clone()))
            .await
            .expect("initialize service at generation two");

        let mut rolled_back = checkpoint;
        rolled_back.generation = 1;
        let payload = norito::to_bytes(&rolled_back).expect("encode rollback checkpoint");
        let record = GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::Checkpoint,
            1,
            payload,
        );
        provider
            .inner
            .lock()
            .expect("lock test checkpoint store")
            .checkpoint = Some(record);

        let error = service
            .reconcile_once()
            .await
            .expect_err("checkpoint rollback must fail before source/network work");
        assert!(error.to_string().contains("rolled back"));

        provider
            .inner
            .lock()
            .expect("lock test checkpoint store")
            .checkpoint = None;
        let error = service
            .reconcile_once()
            .await
            .expect_err("checkpoint deletion must fail before source/network work");
        assert!(error.to_string().contains("removed the active checkpoint"));
    }

    #[tokio::test]
    async fn hardened_http_refuses_redirect_header_body_and_encoding_attacks() {
        let redirect_router = Router::new()
            .route(
                "/redirect",
                get(|| async { Redirect::temporary("/target") }),
            )
            .route("/target", get(|| async { "followed" }));
        let (redirect, redirect_task) = spawn_router(redirect_router, "/redirect").await;
        let request = redirect
            .request(Method::GET, redirect.url.clone())
            .expect("build redirect request");
        let response = redirect
            .execute(request, "redirect test request failed")
            .await
            .expect("receive redirect response");
        assert!(response.status().is_redirection());
        redirect_task.abort();

        let router = Router::new()
            .route("/headers", get(response_header_bomb))
            .route("/body", get(response_body_bomb))
            .route("/gzip", get(response_gzip));
        let (endpoint, task) = spawn_router(router, "/headers").await;
        let request = endpoint
            .request(Method::GET, endpoint.url.clone())
            .expect("build header request");
        let response = endpoint
            .execute(request, "header-bound test request failed")
            .await
            .expect("receive header response");
        assert!(read_bounded_response(response, 1024).await.is_err());

        let mut body_url = endpoint.url.clone();
        body_url.set_path("/body");
        let request = endpoint
            .request(Method::GET, body_url)
            .expect("build body request");
        let response = endpoint
            .execute(request, "body-bound test request failed")
            .await
            .expect("receive body response");
        assert!(read_bounded_response(response, 16).await.is_err());

        let mut gzip_url = endpoint.url.clone();
        gzip_url.set_path("/gzip");
        let request = endpoint
            .request(Method::GET, gzip_url)
            .expect("build gzip request");
        let response = endpoint
            .execute(request, "encoding test request failed")
            .await
            .expect("receive gzip response");
        assert!(read_bounded_response(response, 16).await.is_err());
        task.abort();
    }

    #[tokio::test]
    async fn ipfs_publication_rejects_malformed_cid_missing_pin_and_wrong_readback() {
        let cases = [
            MockIpfsState {
                add_body: Arc::new(b"not-json".to_vec()),
                cat_body: Arc::new(b"payload".to_vec()),
                pin_present: true,
            },
            MockIpfsState {
                add_body: Arc::new(br#"{"Hash":"bad/cid"}"#.to_vec()),
                cat_body: Arc::new(b"payload".to_vec()),
                pin_present: true,
            },
            MockIpfsState {
                add_body: Arc::new(format!(r#"{{"Hash":"{TEST_CID_PAYLOAD}"}}"#).into_bytes()),
                cat_body: Arc::new(b"payload".to_vec()),
                pin_present: false,
            },
            MockIpfsState {
                add_body: Arc::new(format!(r#"{{"Hash":"{TEST_CID_BLOCK}"}}"#).into_bytes()),
                cat_body: Arc::new(b"payload".to_vec()),
                pin_present: true,
            },
            MockIpfsState {
                add_body: Arc::new(format!(r#"{{"Hash":"{TEST_CID_PAYLOAD}"}}"#).into_bytes()),
                cat_body: Arc::new(b"different".to_vec()),
                pin_present: true,
            },
        ];
        for state in cases {
            let (endpoint, task) = spawn_router(mock_ipfs_router(state), "/").await;
            let result = ipfs_add_verified(&endpoint, "block.to", b"payload", 1024, 1024).await;
            assert!(result.is_err());
            task.abort();
        }

        let valid = MockIpfsState {
            add_body: Arc::new(format!(r#"{{"Hash":"{TEST_CID_PAYLOAD}"}}"#).into_bytes()),
            cat_body: Arc::new(b"payload".to_vec()),
            pin_present: true,
        };
        let (endpoint, task) = spawn_router(mock_ipfs_router(valid), "/").await;
        assert_eq!(
            ipfs_add_verified(&endpoint, "block.to", b"payload", 1024, 1024)
                .await
                .expect("valid mock IPFS publication"),
            TEST_CID_PAYLOAD
        );
        task.abort();
    }

    #[test]
    fn canonical_ipfs_cid_is_derived_from_exact_payload_bytes() {
        assert_eq!(canonical_raw_sha256_cid(b"payload"), TEST_CID_PAYLOAD);
        assert_eq!(
            validate_ipfs_cid_for_bytes(TEST_CID_PAYLOAD, b"payload")
                .expect("canonical CID commits to the exact bytes"),
            TEST_CID_PAYLOAD
        );
        assert!(
            validate_ipfs_cid_for_bytes(TEST_CID_PAYLOAD, b"payload-tampered").is_err(),
            "a canonical but substituted CID must not authenticate different bytes"
        );
    }

    #[test]
    fn ipfs_multipart_body_is_deterministic_bounded_and_cloneable() {
        let (boundary, body) =
            canonical_ipfs_multipart_body("governance-head.to", b"\0payload\r\n")
                .expect("construct canonical multipart body");
        let (replayed_boundary, replayed_body) =
            canonical_ipfs_multipart_body("governance-head.to", b"\0payload\r\n")
                .expect("replay canonical multipart body");
        assert_eq!(boundary, replayed_boundary);
        assert_eq!(body, replayed_body);
        assert!(boundary.len() <= 70);
        assert!(body.starts_with(format!("--{boundary}\r\n").as_bytes()));
        assert!(body.ends_with(format!("\r\n--{boundary}--\r\n").as_bytes()));
        assert!(
            body.windows(b"\0payload\r\n".len())
                .any(|window| window == b"\0payload\r\n")
        );
        assert!(canonical_ipfs_multipart_body("../escape", b"payload").is_err());

        let request = Client::new()
            .post("https://example.invalid/api/v0/add")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body);
        assert!(
            request.try_clone().is_some(),
            "the final multipart request must remain inspectable by the authenticator"
        );
    }

    #[tokio::test]
    async fn signed_head_authenticator_receives_final_body_and_cas_headers() {
        let cases = [
            (
                SignedHeadInner {
                    bytes: Some(b"old".to_vec()),
                    etag: "\"v1\"".to_owned(),
                    ..SignedHeadInner::default()
                },
                PublicHead::Present {
                    bytes: b"old".to_vec(),
                    token: "\"v1\"".to_owned(),
                },
                header::IF_MATCH,
                HeaderValue::from_static("\"v1\""),
                false,
            ),
            (
                SignedHeadInner::default(),
                PublicHead::Missing,
                header::IF_NONE_MATCH,
                HeaderValue::from_static("*"),
                true,
            ),
        ];
        for (inner, current, condition, condition_value, allow_bootstrap) in cases {
            let provider = Arc::new(FinalRequestAuthenticator::new(
                b"new",
                condition,
                condition_value,
            ));
            let authenticator = OpaqueAuthenticator::try_new(
                TEST_HEAD_AUTH_HANDLE,
                TEST_AUTH_QUALIFICATION,
                provider.public_key(),
                30,
                5,
                provider.clone(),
                "signed-head authenticator",
            )
            .expect("bind final-request authenticator");
            let (endpoint, _state, task) =
                spawn_signed_head_with_authenticator(inner, authenticator).await;
            let installed =
                put_signed_http_head(&endpoint, b"new", &current, allow_bootstrap, 1024)
                    .await
                    .expect("authenticate and install the final conditional request");
            assert!(matches!(
                installed,
                PublicHead::Present { bytes, .. } if bytes == b"new"
            ));
            assert!(
                provider.observed_put.load(AtomicOrdering::SeqCst),
                "authenticator must observe the body and conditional headers before execution"
            );
            task.abort();
        }
    }

    #[tokio::test]
    async fn signed_head_cas_rejects_conflict_bootstrap_and_readback_drift() {
        for status in [StatusCode::CONFLICT, StatusCode::PRECONDITION_FAILED] {
            let (endpoint, _state, task) = spawn_signed_head(SignedHeadInner {
                bytes: Some(b"old".to_vec()),
                etag: "\"v1\"".to_owned(),
                put_status: Some(status),
                ..SignedHeadInner::default()
            })
            .await;
            let current = PublicHead::Present {
                bytes: b"old".to_vec(),
                token: "\"v1\"".to_owned(),
            };
            assert!(
                put_signed_http_head(&endpoint, b"new", &current, false, 1024)
                    .await
                    .is_err()
            );
            task.abort();
        }

        let (endpoint, state, task) = spawn_signed_head(SignedHeadInner::default()).await;
        assert!(
            put_signed_http_head(&endpoint, b"new", &PublicHead::Missing, false, 1024)
                .await
                .is_err()
        );
        assert_eq!(state.0.lock().await.put_count, 0);
        task.abort();

        let (endpoint, _state, task) = spawn_signed_head(SignedHeadInner {
            bytes: Some(b"old".to_vec()),
            etag: "\"v1\"".to_owned(),
            readback_override: Some(b"attacker".to_vec()),
            ..SignedHeadInner::default()
        })
        .await;
        let current = PublicHead::Present {
            bytes: b"old".to_vec(),
            token: "\"v1\"".to_owned(),
        };
        assert!(
            put_signed_http_head(&endpoint, b"new", &current, false, 1024)
                .await
                .is_err()
        );
        task.abort();
    }

    #[tokio::test]
    async fn ipns_publication_rejects_pre_post_movement_and_readback_drift() {
        let initial = PublicHead::Present {
            bytes: b"old".to_vec(),
            token: TEST_CID_OLD.to_owned(),
        };
        let cases = [
            (
                VecDeque::from([TEST_CID_ATTACKER.to_owned()]),
                HashMap::from([(TEST_CID_ATTACKER.to_owned(), b"attacker".to_vec())]),
            ),
            (
                VecDeque::from([TEST_CID_OLD.to_owned(), TEST_CID_ATTACKER.to_owned()]),
                HashMap::from([
                    (TEST_CID_OLD.to_owned(), b"old".to_vec()),
                    (TEST_CID_ATTACKER.to_owned(), b"attacker".to_vec()),
                ]),
            ),
            (
                VecDeque::from([TEST_CID_OLD.to_owned(), TEST_CID_NEW.to_owned()]),
                HashMap::from([
                    (TEST_CID_OLD.to_owned(), b"old".to_vec()),
                    (TEST_CID_NEW.to_owned(), b"wrong".to_vec()),
                ]),
            ),
        ];
        for (resolutions, bodies) in cases {
            let state = IpnsMockState {
                resolutions: Arc::new(Mutex::new(resolutions)),
                bodies: Arc::new(bodies),
                publish_count: Arc::new(AtomicU64::new(0)),
            };
            let (endpoint, task) = spawn_router(mock_ipns_router(state), "/").await;
            assert!(
                publish_ipns_head(
                    &endpoint,
                    IpnsHeadPublishRequest {
                        name: "test-name",
                        key_name: "test-key",
                        head_cid: TEST_CID_NEW,
                        bytes: b"new",
                        initial: &initial,
                        allow_bootstrap: false,
                        max_response_bytes: 1024,
                    },
                )
                .await
                .is_err()
            );
            task.abort();
        }
    }

    #[test]
    fn ipns_absence_profile_is_narrow_and_exact() {
        assert!(is_authenticated_ipns_absence(StatusCode::NOT_FOUND, b"{}"));
        assert!(is_authenticated_ipns_absence(
            StatusCode::INTERNAL_SERVER_ERROR,
            br#"{"Message":"could not resolve name","Code":0,"Type":"error"}"#
        ));
        assert!(!is_authenticated_ipns_absence(
            StatusCode::INTERNAL_SERVER_ERROR,
            br#"{"Message":"routing unavailable","Code":0,"Type":"error"}"#
        ));
        assert!(!is_authenticated_ipns_absence(
            StatusCode::INTERNAL_SERVER_ERROR,
            br#"{"Message":"could not resolve name","Code":0,"Type":"error","Retry":true}"#
        ));
        assert!(!is_authenticated_ipns_absence(
            StatusCode::TOO_MANY_REQUESTS,
            br#"{"Message":"could not resolve name","Code":0,"Type":"error"}"#
        ));
    }

    #[tokio::test]
    async fn ipns_resolution_errors_never_authorize_bootstrap_publication() {
        for status in [
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            let publish_count = Arc::new(AtomicU64::new(0));
            let state = IpnsResolveFailureState {
                status,
                publish_count: publish_count.clone(),
            };
            let (endpoint, task) = spawn_router(mock_ipns_resolve_failure_router(state), "/").await;
            let error = publish_ipns_head(
                &endpoint,
                IpnsHeadPublishRequest {
                    name: "test-name",
                    key_name: "test-key",
                    head_cid: TEST_CID_NEW,
                    bytes: b"new",
                    initial: &PublicHead::Missing,
                    allow_bootstrap: true,
                    max_response_bytes: 1024,
                },
            )
            .await
            .expect_err("authenticated resolver failure must fail closed");
            assert!(error.to_string().contains(status.as_str()));
            assert_eq!(
                publish_count.load(AtomicOrdering::SeqCst),
                0,
                "resolver failure must not be reclassified as authenticated absence"
            );
            task.abort();
        }
    }

    #[test]
    fn mirror_file_rejects_truncation_metadata_drift_and_recovers_when_missing() {
        let dir = secure_temp_dir();
        let source = signed_source(2, 0x3a, 1_800_000_000);
        let config = test_runtime_config(&source, dir.path());
        let mut checkpoint = checkpoint_from_source(&source);
        let mirror = mirror_index_value(
            &source,
            &checkpoint.mirror_blocks,
            checkpoint.generation,
            &checkpoint.head_ipfs_cid,
            &checkpoint.public_head_token,
            checkpoint.published_at_unix,
        )
        .expect("build test mirror");
        let canonical = json::to_json_pretty(&mirror)
            .expect("encode test mirror")
            .into_bytes();
        checkpoint.mirror_blake3 = blake3_array(&canonical);
        let path = config.state_root_guard.root().join(MIRROR_INDEX_FILE);
        write_rooted_atomic_secret(
            &config.state_root_guard,
            Path::new(MIRROR_INDEX_FILE),
            &canonical,
        )
        .expect("write test mirror");
        verify_mirror_file(&config, &checkpoint).expect("valid mirror accepted");

        fs::remove_file(&path).expect("remove mirror for recovery");
        verify_or_recover_mirror_file(&config, &checkpoint, &source)
            .expect("missing mirror rebuilt deterministically");
        assert_eq!(fs::read(&path).expect("read rebuilt mirror"), canonical);

        fs::write(&path, &canonical[..canonical.len() / 2]).expect("truncate mirror");
        assert!(verify_mirror_file(&config, &checkpoint).is_err());

        for field in ["schema", "generation", "head"] {
            let mut value = mirror.clone();
            match field {
                "schema" => {
                    value
                        .as_object_mut()
                        .expect("mirror object")
                        .insert("schema".into(), JsonValue::from("wrong.schema"));
                }
                "generation" => {
                    value
                        .as_object_mut()
                        .expect("mirror object")
                        .insert("generation".into(), JsonValue::from(99_u64));
                }
                "head" => {
                    value
                        .get_mut("head")
                        .and_then(JsonValue::as_object_mut)
                        .expect("head object")
                        .insert(
                            "head_block_cid_hex".into(),
                            JsonValue::from("00".repeat(32)),
                        );
                }
                _ => unreachable!("closed test field set"),
            }
            let bytes = json::to_json_pretty(&value)
                .expect("encode drifted mirror")
                .into_bytes();
            let mut matching_digest_checkpoint = checkpoint.clone();
            matching_digest_checkpoint.mirror_blake3 = blake3_array(&bytes);
            fs::write(&path, bytes).expect("write drifted mirror");
            assert!(verify_mirror_file(&config, &matching_digest_checkpoint).is_err());
        }
    }

    #[test]
    fn durable_restart_state_preserves_every_publish_phase() {
        let source = signed_source(2, 0x3b, 1_800_000_000);
        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let store = test_checkpoint_store(provider);
        let mut intent = intent_from_source(&source);
        for block in &mut intent.blocks {
            block.ipfs_cid = None;
        }
        intent.head_ipfs_cid = None;
        let mut intent_revision =
            save_publish_intent(&store, None, &intent).expect("persist prepared intent");
        assert_eq!(
            load_publish_intent(&store)
                .expect("reload prepared intent")
                .0
                .expect("prepared intent exists")
                .blocks
                .iter()
                .filter(|block| block.ipfs_cid.is_some())
                .count(),
            0
        );

        intent.blocks[0].ipfs_cid = Some(TEST_CID_BLOCK.to_owned());
        intent_revision = save_publish_intent(&store, Some(intent_revision), &intent)
            .expect("persist partial pins");
        assert_eq!(
            load_publish_intent(&store)
                .expect("reload partial pins")
                .0
                .expect("partial intent exists")
                .blocks[0]
                .ipfs_cid
                .as_deref(),
            Some(TEST_CID_BLOCK)
        );

        intent.blocks[1].ipfs_cid = Some(TEST_CID_PAYLOAD.to_owned());
        intent.head_ipfs_cid = Some(TEST_CID_HEAD.to_owned());
        intent_revision =
            save_publish_intent(&store, Some(intent_revision), &intent).expect("persist head pin");
        let loaded = load_publish_intent(&store)
            .expect("reload head pin")
            .0
            .expect("head intent exists");
        assert_eq!(loaded.head_ipfs_cid.as_deref(), Some(TEST_CID_HEAD));

        let target = PublicHead::Present {
            bytes: intent.target_head_bytes.clone(),
            token: "\"target\"".to_owned(),
        };
        assert_eq!(
            public_head_digest(&target),
            Some(intent.target_head_blake3),
            "restart recognizes a public head already at the durable target"
        );

        let checkpoint = checkpoint_from_source(&source);
        save_checkpoint(&store, None, &checkpoint).expect("persist checkpoint before cleanup");
        assert!(
            load_checkpoint(&store)
                .expect("reload checkpoint")
                .0
                .is_some()
        );
        assert!(
            load_publish_intent(&store)
                .expect("reload stale completed intent")
                .0
                .is_some()
        );
        delete_publish_intent(&store, Some(intent_revision))
            .expect("restart removes completed intent");
        assert!(
            load_publish_intent(&store)
                .expect("intent remains absent")
                .0
                .is_none()
        );
    }

    #[tokio::test]
    async fn metrics_expose_exact_values_and_payload_kind_counts() {
        let mut block = JsonMap::new();
        block.insert("payload_kind".into(), JsonValue::from("deal_settlement"));
        let mut mirror = JsonMap::new();
        mirror.insert(
            "blocks".into(),
            JsonValue::Array(vec![
                JsonValue::Object(block.clone()),
                JsonValue::Object(block),
            ]),
        );
        let state = ApiState(Arc::new(RwLock::new(ApiSnapshot {
            mirror: Some(JsonValue::Object(mirror)),
            metrics: ServiceMetrics {
                publish_success_total: 2,
                publish_failure_total: 3,
                published_bytes_total: 5,
                last_publish_timestamp_seconds: 7,
                backlog: 11,
                head_age_seconds: 13,
                ipfs_pin_lag_seconds: 17,
                ipns_update_success_total: 19,
                ipns_update_failure_total: 23,
                last_ipns_update_timestamp_seconds: 29,
                validation_failure_total: 31,
                mirror_drift: 37,
            },
            ..ApiSnapshot::default()
        })));
        let response = metrics_handler(State(state)).await;
        let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("read metrics body");
        let body = std::str::from_utf8(&body).expect("metrics are UTF-8");
        for expected in [
            "result=\"success\"} 2",
            "result=\"failure\"} 3",
            "published_bytes_total{sink=\"ipfs\"} 5",
            "last_ipns_update_timestamp_seconds 29",
            "validation_failure_total 31",
            "mirror_drift 37",
            "blocks{payload_kind=\"deal_settlement\"} 2",
        ] {
            assert!(body.contains(expected), "missing metric row: {expected}");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires SORAFS_RUN_KUBO_INTEGRATION=1 and a local Kubo binary"]
    async fn real_kubo_publication_ipns_restart_and_tamper_lane() {
        let kubo = KuboHarness::start().await;
        let endpoint = kubo.endpoint();
        assert_kubo_has_no_swarm_peers(&endpoint).await;
        let ipns_name = kubo_key_generate(&endpoint, KUBO_IPNS_KEY_ALIAS).await;

        let direct_payload = b"sorafs-governance-dag-real-kubo-integration-v1";
        let direct_cid = ipfs_add_verified(
            &endpoint,
            "direct-integration-object.to",
            direct_payload,
            1024 * 1024,
            1024 * 1024,
        )
        .await
        .expect("real Kubo add/pin/ls/cat roundtrip");
        assert!(is_canonical_cid_v1(&direct_cid));
        assert_eq!(
            ipfs_cat(
                &endpoint,
                &direct_cid,
                direct_payload.len() as u64,
                1024 * 1024
            )
            .await
            .expect("cat direct Kubo object"),
            direct_payload
        );
        assert!(
            ipfs_cat(
                &endpoint,
                &direct_cid,
                direct_payload.len() as u64 - 1,
                1024 * 1024,
            )
            .await
            .is_err(),
            "bounded cat must reject a real response larger than expected"
        );
        kubo_unpin(&endpoint, &direct_cid).await;
        assert!(
            ipfs_verify_pin(&endpoint, &direct_cid, 1024 * 1024)
                .await
                .is_err(),
            "real Kubo pin/ls must expose a removed recursive pin"
        );
        ipfs_pin(&endpoint, &direct_cid, 1024 * 1024)
            .await
            .expect("restore direct object pin");
        assert!(
            ipfs_cat(&endpoint, TEST_CID_ATTACKER, 1024, 1024)
                .await
                .is_err(),
            "unknown content-addressed bytes must fail closed"
        );

        let work = secure_temp_dir();
        let source_dir = work.path().join("source");
        let state_dir = work.path().join("state");
        let checkpoint_store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));

        let first_timestamp = current_unix_timestamp_seconds().saturating_sub(5);
        let mut source = signed_source(3, 0x72, first_timestamp);
        materialize_source_snapshot(&source_dir, &mut source);
        seed_producer_checkpoint(&checkpoint_store, &source_dir, &source);
        let view =
            real_kubo_service_view(&source, &source_dir, &state_dir, &kubo.api_url, &ipns_name);

        let mut service = Service::from_view(
            view.clone(),
            test_runtime_providers(checkpoint_store.clone()),
        )
        .await
        .expect("initialize G-DAG service against real Kubo");
        service
            .reconcile_once()
            .await
            .expect("publish verified source through real Kubo and IPNS");
        let checkpoint = service
            .checkpoint
            .clone()
            .expect("first reconciliation persists checkpoint");
        assert_eq!(checkpoint.block_count, source.blocks.len() as u64);
        assert_eq!(checkpoint.mirror_blocks.len(), source.blocks.len());
        assert!(state_dir.join(MIRROR_INDEX_FILE).is_file());
        assert!(
            checkpoint_store
                .load(GovernanceDagSealedStateSlot::PublishIntent)
                .expect("read integration sealed intent")
                .is_none()
        );
        assert!(
            checkpoint_store
                .load(GovernanceDagSealedStateSlot::Checkpoint)
                .expect("read integration sealed checkpoint")
                .is_some()
        );
        for (published, block) in checkpoint.mirror_blocks.iter().zip(&source.blocks) {
            ipfs_verify_pin(&service.ipfs, &published.ipfs_cid, 1024 * 1024)
                .await
                .expect("real Kubo retains recursive block pin");
            assert_eq!(
                ipfs_cat(
                    &service.ipfs,
                    &published.ipfs_cid,
                    block.bytes.len() as u64,
                    1024 * 1024,
                )
                .await
                .expect("read real Kubo block"),
                block.bytes
            );
        }
        let public = resolve_ipns_head(&service.ipfs, &ipns_name, 1024 * 1024)
            .await
            .expect("resolve published IPNS head");
        assert!(matches!(
            &public,
            PublicHead::Present { bytes, token }
                if bytes == &source.head_bytes && token == &checkpoint.head_ipfs_cid
        ));

        fs::remove_file(state_dir.join(MIRROR_INDEX_FILE))
            .expect("remove mirror to exercise deterministic recovery");
        service
            .reconcile_once()
            .await
            .expect("steady-state reconciliation rebuilds missing mirror");
        assert!(state_dir.join(MIRROR_INDEX_FILE).is_file());

        kubo_unpin(&service.ipfs, &checkpoint.head_ipfs_cid).await;
        let missing_pin = service
            .reconcile_once()
            .await
            .expect_err("steady state must reject a missing real Kubo head pin");
        assert!(matches!(missing_pin, GovernanceDagServiceError::Network(_)));
        ipfs_pin(&service.ipfs, &checkpoint.head_ipfs_cid, 1024 * 1024)
            .await
            .expect("restore real Kubo head pin");
        service
            .reconcile_once()
            .await
            .expect("steady state recovers after head repin");

        let checkpoint_record = checkpoint_store
            .load(GovernanceDagSealedStateSlot::Checkpoint)
            .expect("read sealed checkpoint")
            .expect("sealed checkpoint exists");
        {
            let mut inner = checkpoint_store
                .inner
                .lock()
                .expect("lock integration store");
            let record = inner.checkpoint.as_mut().expect("checkpoint record");
            let tamper_position = record.payload.len() / 2;
            record.payload[tamper_position] ^= 0x80;
        }
        let checkpoint_error = service
            .reconcile_once()
            .await
            .expect_err("authenticated checkpoint tamper must fail closed");
        assert!(matches!(
            checkpoint_error,
            GovernanceDagServiceError::State(_)
        ));
        {
            let mut inner = checkpoint_store
                .inner
                .lock()
                .expect("lock integration store");
            inner.checkpoint = Some(checkpoint_record);
        }
        service
            .reconcile_once()
            .await
            .expect("restored authenticated checkpoint reconciles");

        drop(service);
        let mut restarted = Service::from_view(view, test_runtime_providers(checkpoint_store))
            .await
            .expect("restart G-DAG service from durable state");
        restarted
            .reconcile_once()
            .await
            .expect("restart verifies checkpoint, IPNS head, pins, and readback");
        assert_eq!(
            restarted
                .checkpoint
                .as_ref()
                .expect("restart loaded checkpoint")
                .generation,
            checkpoint.generation
        );
        assert!(restarted.api.0.read().await.ready);

        let attacker_bytes = b"concurrent-authorized-but-unexpected-ipns-head";
        let attacker_cid = ipfs_add_verified(
            &restarted.ipfs,
            "attacker-head.to",
            attacker_bytes,
            1024 * 1024,
            1024 * 1024,
        )
        .await
        .expect("publish adversarial head bytes to real Kubo");
        let current = resolve_ipns_head(&restarted.ipfs, &ipns_name, 1024 * 1024)
            .await
            .expect("read current IPNS head before adversarial movement");
        publish_ipns_head(
            &restarted.ipfs,
            IpnsHeadPublishRequest {
                name: &ipns_name,
                key_name: KUBO_IPNS_KEY_ALIAS,
                head_cid: &attacker_cid,
                bytes: attacker_bytes,
                initial: &current,
                allow_bootstrap: false,
                max_response_bytes: 1024 * 1024,
            },
        )
        .await
        .expect("move test IPNS name with its isolated key");
        let moved = restarted
            .reconcile_once()
            .await
            .expect_err("checkpoint reconciliation must reject unexpected IPNS movement");
        assert!(matches!(moved, GovernanceDagServiceError::Conflict(_)));

        let attacker = resolve_ipns_head(&restarted.ipfs, &ipns_name, 1024 * 1024)
            .await
            .expect("resolve adversarial IPNS value");
        publish_ipns_head(
            &restarted.ipfs,
            IpnsHeadPublishRequest {
                name: &ipns_name,
                key_name: KUBO_IPNS_KEY_ALIAS,
                head_cid: &checkpoint.head_ipfs_cid,
                bytes: &source.head_bytes,
                initial: &attacker,
                allow_bootstrap: false,
                max_response_bytes: 1024 * 1024,
            },
        )
        .await
        .expect("restore checkpointed IPNS value");
        restarted
            .reconcile_once()
            .await
            .expect("restored IPNS head returns service to steady state");

        eprintln!(
            "real Kubo G-DAG lane passed: direct_cid={direct_cid} head_cid={} ipns_name={ipns_name}",
            checkpoint.head_ipfs_cid
        );
        drop(restarted);
        kubo.shutdown();
    }

    #[test]
    fn remote_head_validates_complete_prefix_and_rejects_checkpoint_tamper() {
        let source = signed_source(2, 0x39, current_unix_timestamp_seconds().saturating_sub(1));
        let dir = secure_temp_dir();
        let config = test_runtime_config(&source, dir.path());
        validate_remote_head(&source.head_bytes, &source, &config)
            .expect("canonical public head binds the complete source prefix");

        let signer = TestSigner::new(0x39);
        let mut tampered = source.head.clone();
        tampered.checkpoint_cid = Some(source.blocks[0].block.block_cid.clone());
        tampered.head_signature = signer.sign(
            &tampered
                .signature_payload_bytes()
                .expect("encode checkpoint-tampered head"),
        );
        let tampered_bytes = norito::to_bytes(&tampered).expect("encode checkpoint-tampered head");
        assert!(
            validate_remote_head(&tampered_bytes, &source, &config).is_err(),
            "a validly signed head with a noncanonical checkpoint must fail"
        );
    }

    #[test]
    fn remote_head_rejects_future_timestamp() {
        let now = current_unix_timestamp_seconds();
        let signer = TestSigner::new(0x3c);
        let mut source = signed_source(1, 0x3c, now);
        source.head.generated_at = now + 120;
        source.head.head_signature = signer.sign(
            &source
                .head
                .signature_payload_bytes()
                .expect("encode future head"),
        );
        source.head_bytes = norito::to_bytes(&source.head).expect("encode future head");
        let dir = secure_temp_dir();
        let config = test_runtime_config(&source, dir.path());
        assert!(validate_remote_head(&source.head_bytes, &source, &config).is_err());
    }
}
