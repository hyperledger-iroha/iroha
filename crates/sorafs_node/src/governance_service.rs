//! Injectable SoraFS Governance DAG publisher and bounded public mirror service.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ffi::{OsStr, OsString},
    fmt,
    fs::{self, OpenOptions},
    future::{Future, IntoFuture},
    io::{self, Read},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    ops::Deref,
    path::{Component, Path, PathBuf},
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
    GovernanceDagRequestIngressBindingV1, GovernanceDagRequestIngressQualificationV1,
    GovernanceDagRuntimeProviderQualificationV1, GovernanceDagSealedCheckpointStore,
    GovernanceDagSealedStateRecord, GovernanceDagSealedStateSlot,
    governance::{
        GOVERNANCE_DAG_LOGICAL_ROOT as RUNTIME_INDEX_LOGICAL_ROOT,
        GOVERNANCE_DAG_SINK_FILESYSTEM as RUNTIME_INDEX_SOURCE, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES,
        GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR, GOVERNANCE_RUNTIME_DAG_DIR,
        GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1,
        GOVERNANCE_RUNTIME_DAG_INDEX_BLOCK_FIELDS_V1 as RUNTIME_INDEX_BLOCK_FIELDS_V1,
        GOVERNANCE_RUNTIME_DAG_INDEX_FIELDS_V1 as RUNTIME_INDEX_TOP_LEVEL_FIELDS_V1,
        GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA as RUNTIME_INDEX_SCHEMA,
        GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_VERSION_V1, GovernanceFilesystemRootGuard,
        RuntimeDagCommittedSnapshotV1, RuntimeDagProducerCheckpointV1,
        governance_source_pair_relative_paths, load_runtime_dag_committed_snapshot_v1,
        runtime_dag_producer_root_digest, validate_governance_car_source_lengths,
        validate_runtime_dag_snapshot_authority_lineage,
        verify_governance_dag_request_authentication_without_replay_v1,
    },
    governance_dag_request_ingress_endpoint_binding_v1,
    governance_rooted_fs::{
        FileBinding, FileSnapshot, RetainedFile, TwoSlotSnapshotV1, TwoSlotStoreConfigV1,
        TwoSlotStoreV1,
    },
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
/// Maximum canonical bytes accepted for one mutable service-state artifact.
///
/// This is the Governance DAG service's semantic limit for checkpoint,
/// publication-intent, and mirror state. It is intentionally narrower than
/// [`crate::governance_dag_sealed_state_payload_max_bytes_v1`], whose 192 MiB
/// checkpoint/publish-intent ceiling is a generic provider transport bound.
/// A provider's ability to carry a larger record does not authorize this
/// service to allocate or interpret it.
pub const GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1: u64 = 64 * 1024 * 1024;
const CHECKPOINT_VERSION_V1: u8 = 1;
const PUBLISH_INTENT_VERSION_V1: u8 = 1;
const MIRROR_INDEX_SCHEMA: &str = "sorafs.governance_dag.mirror.v1";
const MIRROR_INDEX_STORE_PAYLOAD_VERSION_V1: u8 = 1;
const MIRROR_INDEX_STORE_NAME: &str = "mirror-index-v1";
const MIRROR_INDEX_STORE_MAX_PAYLOAD_BYTES: usize = 65 * 1024 * 1024;
/// Maximum number of source blocks retained by every version-1 governance mirror.
///
/// This is a protocol constant, not node policy: every qualified publisher must
/// derive byte-identical mirror and checkpoint state for the same sealed intent.
pub const GOVERNANCE_DAG_MIRROR_MAX_ENTRIES_V1: usize = 65_536;
/// Maximum canonical source-block bytes retained by every version-1 governance mirror.
///
/// This is a protocol constant, not node policy: every qualified publisher must
/// derive byte-identical mirror and checkpoint state for the same sealed intent.
pub const GOVERNANCE_DAG_MIRROR_MAX_BYTES_V1: u64 = 512 * 1024 * 1024;
const LEGACY_MIRROR_INDEX_FILE: &str = "mirror-index.json";
const LEGACY_MIRROR_INDEX_SIDECAR_FILE: &str = "mirror-index.json.blake3";
const LEGACY_MIRROR_RECOVERY_QUARANTINE_DIR: &str = ".governance-service-recovery-quarantine-v1";
const SERVICE_LOCK_FILE: &str = ".service.lock";
const MAX_DNS_ADDRESSES: usize = 8;
const MAX_RESPONSE_HEADERS: usize = 64;
const MAX_RESPONSE_HEADER_BYTES: usize = 16 * 1024;
const MAX_IPFS_CID_BYTES: usize = 160;
const MAX_PUBLIC_TOKEN_BYTES: usize = 512;
const SOURCE_ENTRY_HARD_CAP: usize = 131_072;
const SOURCE_TOTAL_BYTES_HARD_CAP: u64 = 1024 * 1024 * 1024;
const IPFS_MULTIPART_BOUNDARY_PREFIX: &str = "iroha-sorafs-gdag-v1";
const IPFS_MULTIPART_FILENAME_MAX_BYTES: usize = 160;
const IPFS_MULTIPART_BOUNDARY_ATTEMPTS: u8 = 16;
const IPFS_UNIXFS_CHUNK_BYTES: usize = 1024 * 1024;
const IPFS_UNIXFS_MAX_FILE_LINKS: usize = 1024;
const IPFS_RAW_CODEC: u64 = 0x55;
const IPFS_DAG_PB_CODEC: u64 = 0x70;
const IPFS_OBJECT_MAX_BYTES: u64 = GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1 as u64;
const IPFS_UNIXFS_V1_ADD_QUERY: &[(&str, &str)] = &[
    ("pin", "false"),
    ("cid-version", "1"),
    ("hash", "sha2-256"),
    ("chunker", "size-1048576"),
    ("trickle", "false"),
    ("max-file-links", "1024"),
    ("raw-leaves", "true"),
    ("wrap-with-directory", "false"),
    ("quieter", "true"),
];
// Norito temporarily copies nested length-delimited fields while decoding.
// The governed block/head schemas stay below this amplification, while the
// finite multiplier still rejects archives that attempt allocation bombs.
const CANONICAL_DECODE_ALLOCATION_MULTIPLIER: usize = 16;
const CANONICAL_DECODE_MAX_TOTAL_ELEMENTS: usize = 4_000_000;
const STEADY_IPFS_AUDIT_MAX_ENTRIES_PER_POLL: usize = 64;
const STEADY_IPFS_AUDIT_MAX_BYTES_PER_POLL: u64 = 16 * 1024 * 1024;

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
    /// The service has not completed, or has withdrawn, external readiness.
    #[error("service unavailable: {0}")]
    Unavailable(String),
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
    /// Attach the rotation-aware Kubo/IPFS authenticator.
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
    ipfs_request_ingress_binding: GovernanceDagRequestIngressBindingV1,
    head_authenticator_handle: String,
    head_authenticator_qualification: GovernanceDagRuntimeProviderQualificationV1,
    head_request_ingress_binding: GovernanceDagRequestIngressBindingV1,
    checkpoint_store_handle: String,
    checkpoint_store_qualification: GovernanceDagRuntimeProviderQualificationV1,
}

impl GovernanceDagServiceRuntimeProviderBindingsV1 {
    /// Stable handle for the Kubo/IPFS authenticator.
    #[must_use]
    pub fn ipfs_authenticator_handle(&self) -> &str {
        &self.ipfs_authenticator_handle
    }

    /// Exact configured Kubo/IPFS authenticator qualification.
    #[must_use]
    pub const fn ipfs_authenticator_qualification(
        &self,
    ) -> GovernanceDagRuntimeProviderQualificationV1 {
        self.ipfs_authenticator_qualification
    }

    /// Exact configured IPFS endpoint, key, request-size, and timing policy.
    #[must_use]
    pub const fn ipfs_request_ingress_binding(&self) -> GovernanceDagRequestIngressBindingV1 {
        self.ipfs_request_ingress_binding
    }

    /// Stable handle for signed-head compare-and-swap authentication.
    #[must_use]
    pub fn head_authenticator_handle(&self) -> &str {
        &self.head_authenticator_handle
    }

    /// Exact configured signed-head authenticator qualification.
    #[must_use]
    pub const fn head_authenticator_qualification(
        &self,
    ) -> GovernanceDagRuntimeProviderQualificationV1 {
        self.head_authenticator_qualification
    }

    /// Exact configured signed-head endpoint, key, request-size, and timing policy.
    #[must_use]
    pub const fn head_request_ingress_binding(&self) -> GovernanceDagRequestIngressBindingV1 {
        self.head_request_ingress_binding
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
    ingress_qualification: GovernanceDagRequestIngressQualificationV1,
    verification_policy: GovernanceDagRequestAuthenticationPolicyV1,
    provider: Arc<dyn GovernanceDagRequestAuthenticator>,
    recent_outbound_nonces: Arc<Mutex<OutboundRequestNonceWindowV1>>,
}

/// Bounded sender-side sanity window for a malfunctioning signer that reuses
/// a still-live nonce. Receiver replay protection belongs exclusively to the
/// shared sealed replay namespace proven by `ingress_qualification`.
#[derive(Debug)]
struct OutboundRequestNonceWindowV1 {
    live: BTreeMap<[u8; 32], u64>,
    order: VecDeque<[u8; 32]>,
    capacity: usize,
}

impl OutboundRequestNonceWindowV1 {
    fn new() -> Self {
        Self {
            live: BTreeMap::new(),
            order: VecDeque::new(),
            capacity: GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1,
        }
    }

    fn observe(
        &mut self,
        nonce: [u8; 32],
        expires_at_unix_secs: u64,
        now_unix_secs: u64,
    ) -> Result<(), GovernanceDagServiceError> {
        self.live.retain(|_, expiry| *expiry > now_unix_secs);
        self.order.retain(|entry| self.live.contains_key(entry));
        if self.live.contains_key(&nonce) {
            return Err(GovernanceDagServiceError::Network(
                "Governance DAG authenticator reused a live outbound nonce".to_owned(),
            ));
        }
        while self.live.len() >= self.capacity {
            let oldest = self.order.pop_front().ok_or_else(|| {
                GovernanceDagServiceError::Network(
                    "Governance DAG outbound nonce window is internally inconsistent".to_owned(),
                )
            })?;
            self.live.remove(&oldest);
        }
        self.live.insert(nonce, expires_at_unix_secs);
        self.order.push_back(nonce);
        Ok(())
    }
}

impl fmt::Debug for OpaqueAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpaqueAuthenticator")
            .field("handle", &self.handle)
            .field(
                "endpoint_binding",
                &hex::encode(self.ingress_qualification.binding().endpoint_binding()),
            )
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
        expected_provider_qualification: GovernanceDagRuntimeProviderQualificationV1,
        expected_ingress_binding: GovernanceDagRequestIngressBindingV1,
        provider: Arc<dyn GovernanceDagRequestAuthenticator>,
        label: &'static str,
    ) -> Result<Self, GovernanceDagServiceError> {
        let handle = validate_runtime_handle(expected_handle, label)?;
        if !expected_provider_qualification.is_valid() {
            return Err(GovernanceDagServiceError::Config(format!(
                "{label} configured policy qualification is invalid"
            )));
        }
        let verification_policy = validate_request_auth_policy(
            expected_ingress_binding.public_key(),
            expected_ingress_binding.max_envelope_lifetime_secs(),
            expected_ingress_binding.max_future_skew_secs(),
            label,
        )?;
        let provider_handle = validate_runtime_handle(provider.handle(), label)?;
        if provider_handle != handle {
            return Err(GovernanceDagServiceError::Config(format!(
                "{label} provider handle does not match configured handle"
            )));
        }
        let ingress_qualification = provider.ingress_qualification().map_err(|_| {
            GovernanceDagServiceError::Config(format!(
                "{label} provider is unavailable, stale, or unqualified"
            ))
        })?;
        if ingress_qualification.provider() != expected_provider_qualification
            || ingress_qualification.binding() != expected_ingress_binding
        {
            return Err(GovernanceDagServiceError::Config(format!(
                "{label} provider qualification or ingress binding does not match configuration"
            )));
        }
        let rechecked_qualification = provider.ingress_qualification().map_err(|_| {
            GovernanceDagServiceError::Config(format!(
                "{label} provider is unavailable, stale, or unqualified"
            ))
        })?;
        if provider.handle() != handle || rechecked_qualification != ingress_qualification {
            return Err(GovernanceDagServiceError::Config(format!(
                "{label} provider identity or ingress qualification changed during startup qualification"
            )));
        }
        Ok(Self {
            handle,
            ingress_qualification,
            verification_policy,
            provider,
            recent_outbound_nonces: Arc::new(Mutex::new(OutboundRequestNonceWindowV1::new())),
        })
    }

    fn authenticate(
        &self,
        request: &GovernanceDagCanonicalRequestV1,
    ) -> Result<GovernanceDagRequestAuthenticationEnvelopeV1, GovernanceDagServiceError> {
        let binding = self.ingress_qualification.binding();
        if request.scope() != binding.scope() || request.body_length() > binding.max_body_bytes() {
            return Err(GovernanceDagServiceError::Network(
                "Governance DAG outbound request does not match the qualified ingress policy"
                    .to_owned(),
            ));
        }
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
        let ingress_qualification = self.provider.ingress_qualification().map_err(|_| {
            GovernanceDagServiceError::Network(
                "Governance DAG authenticator is unavailable, stale, or unqualified".to_owned(),
            )
        })?;
        if self.provider.handle() != self.handle
            || ingress_qualification != self.ingress_qualification
        {
            return Err(GovernanceDagServiceError::Network(
                "Governance DAG authenticator identity or ingress qualification changed after injection"
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
        verify_governance_dag_request_authentication_without_replay_v1(
            request,
            envelope,
            request.scope(),
            &self.verification_policy,
            now,
        )
        .map_err(|error| GovernanceDagServiceError::Network(error.to_string()))?;
        let mut recent_outbound_nonces = self.recent_outbound_nonces.lock().map_err(|_| {
            GovernanceDagServiceError::Network(
                "Governance DAG outbound nonce sanity window is unavailable".to_owned(),
            )
        })?;
        recent_outbound_nonces.observe(envelope.nonce(), envelope.expires_at_unix_secs(), now)
    }

    const fn ingress_binding(&self) -> GovernanceDagRequestIngressBindingV1 {
        self.ingress_qualification.binding()
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
struct MirrorIndexStorePayloadV1 {
    version: u8,
    checkpoint_generation: u64,
    publish_intent_blake3: [u8; 32],
    mirror_blake3: [u8; 32],
    canonical_json: Vec<u8>,
}

impl MirrorIndexStorePayloadV1 {
    fn empty() -> Self {
        Self {
            version: MIRROR_INDEX_STORE_PAYLOAD_VERSION_V1,
            checkpoint_generation: 0,
            publish_intent_blake3: [0; 32],
            mirror_blake3: [0; 32],
            canonical_json: Vec::new(),
        }
    }

    fn committed(
        checkpoint_generation: u64,
        publish_intent_blake3: [u8; 32],
        canonical_json: Vec<u8>,
    ) -> Result<Self, GovernanceDagServiceError> {
        let payload = Self {
            version: MIRROR_INDEX_STORE_PAYLOAD_VERSION_V1,
            checkpoint_generation,
            publish_intent_blake3,
            mirror_blake3: blake3_array(&canonical_json),
            canonical_json,
        };
        validate_mirror_index_store_payload(&payload)?;
        Ok(payload)
    }

    fn is_empty(&self) -> bool {
        self.checkpoint_generation == 0
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct CheckpointBodyV1 {
    version: u8,
    generation: u64,
    head_block_cid: Vec<u8>,
    block_count: u64,
    head_bytes: Vec<u8>,
    head_bytes_blake3: [u8; 32],
    head_ipfs_cid: String,
    source_chain_blake3: [u8; 32],
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
    target_source_chain_blake3: [u8; 32],
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
    chain_blake3: [u8; 32],
    head: GovernanceDagHeadV1,
    head_bytes: Vec<u8>,
    blocks: Vec<SourceBlock>,
}

#[derive(Debug, Clone)]
struct SourcePrefix<'a> {
    head: GovernanceDagHeadV1,
    head_bytes: Vec<u8>,
    blocks: &'a [SourceBlock],
    chain_blake3: [u8; 32],
}

trait SourceChainView {
    fn head(&self) -> &GovernanceDagHeadV1;
    fn head_bytes(&self) -> &[u8];
    fn blocks(&self) -> &[SourceBlock];
}

impl SourceChainView for SourceSnapshot {
    fn head(&self) -> &GovernanceDagHeadV1 {
        &self.head
    }

    fn head_bytes(&self) -> &[u8] {
        &self.head_bytes
    }

    fn blocks(&self) -> &[SourceBlock] {
        &self.blocks
    }
}

impl SourceChainView for SourcePrefix<'_> {
    fn head(&self) -> &GovernanceDagHeadV1 {
        &self.head
    }

    fn head_bytes(&self) -> &[u8] {
        &self.head_bytes
    }

    fn blocks(&self) -> &[SourceBlock] {
        self.blocks
    }
}

fn source_chain_blake3_v1(head_bytes: &[u8], blocks: &[SourceBlock]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance-dag.service-source-chain.v1\0");
    hasher.update(
        &u64::try_from(blocks.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for block in blocks {
        hasher.update(
            &u64::try_from(block.bytes.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        hasher.update(&block.bytes);
    }
    hasher.update(
        &u64::try_from(head_bytes.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(head_bytes);
    *hasher.finalize().as_bytes()
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
    max_future_skew_secs: u64,
    allow_head_bootstrap: bool,
    expected_producer_signer_handle: String,
    expected_producer_signer_qualification: GovernanceDagRuntimeProviderQualificationV1,
    expected_checkpoint_store_handle: String,
    expected_checkpoint_store_qualification: GovernanceDagRuntimeProviderQualificationV1,
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

/// Public, non-secret identity binding for a Governance DAG mirror reader.
///
/// The descriptor intentionally excludes filesystem paths, provider objects,
/// credentials, and private keys. Embedding nodes use it to reject a reader
/// wired to a different producer root or provider policy before installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceDagMirrorReadBindingV1 {
    source_root_digest: [u8; 32],
    source_root_identity_digest: [u8; 32],
    producer_signer_handle: String,
    producer_signer_qualification: GovernanceDagRuntimeProviderQualificationV1,
    producer_publisher_peer_id: Vec<u8>,
    producer_public_key: [u8; 32],
    checkpoint_store_handle: String,
    checkpoint_store_qualification: GovernanceDagRuntimeProviderQualificationV1,
}

impl GovernanceDagMirrorReadBindingV1 {
    /// Return the producer's canonical, domain-separated root digest.
    #[must_use]
    pub const fn source_root_digest(&self) -> [u8; 32] {
        self.source_root_digest
    }

    /// Return the path-free digest of the retained physical source root.
    #[must_use]
    pub const fn source_root_identity_digest(&self) -> [u8; 32] {
        self.source_root_identity_digest
    }

    /// Return the stable handle of the expected producer signer.
    #[must_use]
    pub fn producer_signer_handle(&self) -> &str {
        &self.producer_signer_handle
    }

    /// Return the exact expected producer-signer qualification.
    #[must_use]
    pub const fn producer_signer_qualification(
        &self,
    ) -> GovernanceDagRuntimeProviderQualificationV1 {
        self.producer_signer_qualification
    }

    /// Return the expected producer publisher peer identifier.
    #[must_use]
    pub fn producer_publisher_peer_id(&self) -> &[u8] {
        &self.producer_publisher_peer_id
    }

    /// Return the expected producer's canonical Ed25519 public key.
    #[must_use]
    pub const fn producer_public_key(&self) -> [u8; 32] {
        self.producer_public_key
    }

    /// Return the stable handle of the sealed checkpoint store.
    #[must_use]
    pub fn checkpoint_store_handle(&self) -> &str {
        &self.checkpoint_store_handle
    }

    /// Return the exact sealed-checkpoint-store qualification.
    #[must_use]
    pub const fn checkpoint_store_qualification(
        &self,
    ) -> GovernanceDagRuntimeProviderQualificationV1 {
        self.checkpoint_store_qualification
    }
}

/// Exact sealed-checkpoint identity authenticating one mirror snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GovernanceDagMirrorCheckpointIdentityV1 {
    generation: u64,
    revision: [u8; 32],
}

impl GovernanceDagMirrorCheckpointIdentityV1 {
    /// Return the sealed monotonic checkpoint generation.
    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the complete sealed-record revision digest.
    #[must_use]
    pub const fn revision(self) -> [u8; 32] {
        self.revision
    }
}

/// One canonical mirror snapshot authenticated by exact durable identities.
#[derive(Debug, PartialEq, Eq)]
pub struct GovernanceDagMirrorSnapshotV1 {
    canonical_bytes: Vec<u8>,
    mirror: JsonValue,
    checkpoint: CheckpointBodyV1,
    mirror_store_generation: u64,
    mirror_store_record_digest: [u8; 32],
    checkpoint_identity: GovernanceDagMirrorCheckpointIdentityV1,
}

impl GovernanceDagMirrorSnapshotV1 {
    /// Borrow the canonical mirror JSON bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    fn mirror(&self) -> &JsonValue {
        &self.mirror
    }

    fn checkpoint(&self) -> &CheckpointBodyV1 {
        &self.checkpoint
    }

    /// Return the typed mirror store generation and complete record digest.
    #[must_use]
    pub const fn mirror_store_identity(&self) -> (u64, [u8; 32]) {
        (
            self.mirror_store_generation,
            self.mirror_store_record_digest,
        )
    }

    /// Return the sealed checkpoint identity that authenticates these bytes.
    #[must_use]
    pub const fn checkpoint_identity(&self) -> GovernanceDagMirrorCheckpointIdentityV1 {
        self.checkpoint_identity
    }
}

/// Cloneable, service-owned capability for coherent Governance DAG mirror reads.
///
/// Each read consults the typed two-slot mirror and sealed checkpoint store
/// directly. It never trusts the listener's cached API state.
#[derive(Clone)]
pub struct GovernanceDagMirrorReadHandleV1 {
    binding: GovernanceDagMirrorReadBindingV1,
    source_root_guard: GovernanceFilesystemRootGuard,
    state_root_guard: GovernanceFilesystemRootGuard,
    mirror_store_config: TwoSlotStoreConfigV1,
    checkpoint_store: OpaqueCheckpointStore,
    readiness: Arc<GovernanceDagMirrorReadinessV1>,
}

#[derive(Debug)]
struct GovernanceDagMirrorReadinessV1 {
    epoch: AtomicU64,
}

impl GovernanceDagMirrorReadinessV1 {
    fn new(bootstrap: bool) -> Self {
        Self {
            epoch: AtomicU64::new(u64::from(!bootstrap)),
        }
    }

    fn transition_to_parity(&self, ready: bool) {
        let desired_parity = u64::from(!ready);
        let _ = self
            .epoch
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                let increment = if current & 1 == desired_parity { 2 } else { 1 };
                current.checked_add(increment)
            });
    }

    fn mark_unready(&self) {
        self.transition_to_parity(false);
    }

    fn mark_ready(&self) {
        self.transition_to_parity(true);
    }

    fn begin_read(&self) -> Result<u64, GovernanceDagServiceError> {
        let epoch = self.epoch.load(Ordering::Acquire);
        if epoch != 0 && epoch & 1 == 1 {
            return Err(GovernanceDagServiceError::Unavailable(
                "Governance DAG mirror readiness is withdrawn".to_owned(),
            ));
        }
        Ok(epoch)
    }

    fn finish_read(&self, expected: u64) -> Result<(), GovernanceDagServiceError> {
        if self.epoch.load(Ordering::Acquire) != expected {
            return Err(GovernanceDagServiceError::Unavailable(
                "Governance DAG mirror readiness changed during the read".to_owned(),
            ));
        }
        Ok(())
    }
}

impl fmt::Debug for GovernanceDagMirrorReadHandleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernanceDagMirrorReadHandleV1")
            .field("binding", &self.binding)
            .field("provider", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl GovernanceDagMirrorReadHandleV1 {
    fn try_new(
        config: &RuntimeConfig,
        checkpoint_store: OpaqueCheckpointStore,
        bootstrap: bool,
    ) -> Result<Self, GovernanceDagServiceError> {
        config.revalidate_source_root()?;
        config.revalidate_state_root()?;
        checkpoint_store.assert_identity()?;
        let source_root_digest =
            runtime_dag_producer_root_digest(&config.source_dir).map_err(|error| {
                GovernanceDagServiceError::Filesystem(format!(
                    "cannot bind Governance DAG mirror reader to producer root: {error}"
                ))
            })?;
        let source_root_identity_digest = config.source_root_guard.identity_digest().map_err(
            |error| {
                GovernanceDagServiceError::Filesystem(format!(
                    "cannot bind Governance DAG mirror reader to physical producer root: {error}"
                ))
            },
        )?;
        let writer_state_identity = config.state_root_guard.identity_digest().map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot bind Governance DAG mirror reader to service state root: {error}"
            ))
        })?;
        let state_root_guard = GovernanceFilesystemRootGuard::capture_source(
            config.state_root_guard.root(),
        )
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot capture read-only Governance DAG mirror state root: {error}"
            ))
        })?;
        if state_root_guard.identity_digest().map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot bind read-only Governance DAG mirror state root: {error}"
            ))
        })? != writer_state_identity
        {
            return Err(GovernanceDagServiceError::Filesystem(
                "Governance DAG mirror state root changed while deriving its read-only capability"
                    .to_owned(),
            ));
        }
        let mirror_store_config = mirror_index_store_config()?;
        let initial_snapshot = state_root_guard
            .rooted_directory()
            .load_existing_two_slot_store_v1(mirror_store_config.clone())
            .map_err(|error| {
                GovernanceDagServiceError::Filesystem(format!(
                    "cannot open existing Governance DAG mirror through read-only handles: {error}"
                ))
            })?;
        let _ = decode_mirror_index_store_payload(initial_snapshot.payload())?;
        let binding = GovernanceDagMirrorReadBindingV1 {
            source_root_digest,
            source_root_identity_digest,
            producer_signer_handle: config.expected_producer_signer_handle.clone(),
            producer_signer_qualification: config.expected_producer_signer_qualification,
            producer_publisher_peer_id: config.expected_publisher_peer_id.clone(),
            producer_public_key: config.expected_public_key,
            checkpoint_store_handle: checkpoint_store.handle.clone(),
            checkpoint_store_qualification: checkpoint_store.qualification,
        };
        let handle = Self {
            binding,
            source_root_guard: config.source_root_guard.clone(),
            state_root_guard,
            mirror_store_config,
            checkpoint_store,
            readiness: Arc::new(GovernanceDagMirrorReadinessV1::new(bootstrap)),
        };
        handle.assert_bindings()?;
        Ok(handle)
    }

    /// Return the immutable, non-secret installation binding.
    #[must_use]
    pub const fn binding(&self) -> &GovernanceDagMirrorReadBindingV1 {
        &self.binding
    }

    /// Read one mirror generation coherent with an exact sealed checkpoint.
    ///
    /// Returns `Ok(None)` only for the authenticated bootstrap state where the
    /// typed mirror is empty and both sealed checkpoint/intent slots remain
    /// absent across the read.
    ///
    /// # Errors
    ///
    /// Fails closed when either retained root or the sealed provider changes,
    /// a publication intent is active, the typed mirror disagrees with its
    /// checkpoint, or the checkpoint changes during the read.
    pub fn read(&self) -> Result<Option<GovernanceDagMirrorSnapshotV1>, GovernanceDagServiceError> {
        let readiness_epoch = self.readiness.begin_read()?;
        self.assert_bindings()?;
        let (checkpoint_a, checkpoint_revision_a) = load_checkpoint(&self.checkpoint_store)?;
        let intent_identity_a = load_publish_intent(&self.checkpoint_store)?;
        if intent_identity_a.0.is_some() || intent_identity_a.1.is_some() {
            return Err(GovernanceDagServiceError::State(
                "Governance DAG mirror has an active sealed publish intent".to_owned(),
            ));
        }

        let mirror_snapshot = self
            .state_root_guard
            .rooted_directory()
            .load_existing_two_slot_store_v1(self.mirror_store_config.clone())
            .map_err(|error| {
                GovernanceDagServiceError::Filesystem(format!(
                    "cannot load existing Governance DAG mirror through read-only handles: {error}"
                ))
            })?;
        let mirror_payload = decode_mirror_index_store_payload(mirror_snapshot.payload())?;

        let (checkpoint_a, checkpoint_revision_a) = match (checkpoint_a, checkpoint_revision_a) {
            (None, None) => {
                if readiness_epoch != 0 {
                    return Err(GovernanceDagServiceError::Unavailable(
                        "ready Governance DAG mirror lost its sealed checkpoint".to_owned(),
                    ));
                }
                if !mirror_payload.is_empty() {
                    return Err(GovernanceDagServiceError::State(
                        "Governance DAG mirror exists without a sealed checkpoint".to_owned(),
                    ));
                }
                let (checkpoint_b, checkpoint_revision_b) =
                    load_checkpoint(&self.checkpoint_store)?;
                let intent_identity_b = load_publish_intent(&self.checkpoint_store)?;
                if intent_identity_b != intent_identity_a {
                    return Err(GovernanceDagServiceError::State(
                        "Governance DAG mirror publish intent changed during bootstrap read"
                            .to_owned(),
                    ));
                }
                if checkpoint_b.is_some() || checkpoint_revision_b.is_some() {
                    return Err(GovernanceDagServiceError::Conflict(
                        "Governance DAG mirror checkpoint changed during bootstrap read".to_owned(),
                    ));
                }
                self.assert_bindings()?;
                self.readiness.finish_read(readiness_epoch)?;
                return Ok(None);
            }
            (Some(checkpoint), Some(revision)) => {
                if readiness_epoch == 0 {
                    return Err(GovernanceDagServiceError::Unavailable(
                        "Governance DAG mirror has not verified its checkpoint externally"
                            .to_owned(),
                    ));
                }
                (checkpoint, revision)
            }
            _ => {
                return Err(GovernanceDagServiceError::State(
                    "Governance DAG mirror checkpoint lacks an exact revision".to_owned(),
                ));
            }
        };
        let mirror = verify_mirror_payload_against_checkpoint(&mirror_payload, &checkpoint_a)?;

        let (checkpoint_b, checkpoint_revision_b) = load_checkpoint(&self.checkpoint_store)?;
        let intent_identity_b = load_publish_intent(&self.checkpoint_store)?;
        if intent_identity_b != intent_identity_a {
            return Err(GovernanceDagServiceError::Conflict(
                "Governance DAG mirror publish intent changed during read".to_owned(),
            ));
        }
        if checkpoint_b.as_ref() != Some(&checkpoint_a)
            || checkpoint_revision_b != Some(checkpoint_revision_a)
        {
            return Err(GovernanceDagServiceError::Conflict(
                "Governance DAG mirror checkpoint changed during read".to_owned(),
            ));
        }
        self.assert_bindings()?;
        self.readiness.finish_read(readiness_epoch)?;

        Ok(Some(GovernanceDagMirrorSnapshotV1 {
            canonical_bytes: mirror_payload.canonical_json,
            mirror,
            checkpoint: checkpoint_a.clone(),
            mirror_store_generation: mirror_snapshot.generation(),
            mirror_store_record_digest: mirror_snapshot.record_digest(),
            checkpoint_identity: GovernanceDagMirrorCheckpointIdentityV1 {
                generation: checkpoint_a.generation,
                revision: checkpoint_revision_a,
            },
        }))
    }

    /// Revalidate the retained roots, provider binding, and existing typed
    /// mirror store without requiring an initialized mirror checkpoint.
    pub(crate) fn assert_install_ready(&self) -> Result<(), GovernanceDagServiceError> {
        self.assert_bindings()?;
        let mirror_snapshot = self
            .state_root_guard
            .rooted_directory()
            .load_existing_two_slot_store_v1(self.mirror_store_config.clone())
            .map_err(|error| {
                GovernanceDagServiceError::Filesystem(format!(
                    "cannot revalidate the existing Governance DAG mirror store before installation: {error}"
                ))
            })?;
        let _ = decode_mirror_index_store_payload(mirror_snapshot.payload())?;
        self.assert_bindings()
    }

    fn mark_unready(&self) {
        self.readiness.mark_unready();
    }

    fn mark_ready(&self) {
        self.readiness.mark_ready();
    }

    fn assert_bindings(&self) -> Result<(), GovernanceDagServiceError> {
        self.source_root_guard.revalidate().map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "Governance DAG mirror source root identity changed: {error}"
            ))
        })?;
        self.state_root_guard.revalidate().map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "Governance DAG mirror state root identity changed: {error}"
            ))
        })?;
        let source_root_digest = runtime_dag_producer_root_digest(self.source_root_guard.root())
            .map_err(|error| {
                GovernanceDagServiceError::Filesystem(format!(
                    "cannot revalidate Governance DAG mirror producer root: {error}"
                ))
            })?;
        let source_root_identity_digest =
            self.source_root_guard.identity_digest().map_err(|error| {
                GovernanceDagServiceError::Filesystem(format!(
                    "cannot revalidate physical Governance DAG mirror producer root: {error}"
                ))
            })?;
        if source_root_digest != self.binding.source_root_digest
            || source_root_identity_digest != self.binding.source_root_identity_digest
            || self.checkpoint_store.handle != self.binding.checkpoint_store_handle
            || self.checkpoint_store.qualification != self.binding.checkpoint_store_qualification
        {
            return Err(GovernanceDagServiceError::State(
                "Governance DAG mirror reader binding changed after construction".to_owned(),
            ));
        }
        self.checkpoint_store.assert_identity()?;
        self.source_root_guard.revalidate().map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "Governance DAG mirror source root changed during binding validation: {error}"
            ))
        })?;
        self.state_root_guard.revalidate().map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "Governance DAG mirror state root changed during binding validation: {error}"
            ))
        })?;
        Ok(())
    }
}

#[derive(Debug)]
struct PinnedEndpoint {
    url: Url,
    client: Client,
    authentication_scope: GovernanceDagAuthenticationScope,
    authenticator: OpaqueAuthenticator,
    authenticated_wire_body_max_bytes: u64,
}

struct AuthenticatedResponse {
    response: reqwest::Response,
    authenticator: OpaqueAuthenticator,
}

impl Deref for AuthenticatedResponse {
    type Target = reqwest::Response;

    fn deref(&self) -> &Self::Target {
        &self.response
    }
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

#[derive(Clone)]
struct ServiceApiState {
    telemetry: ApiState,
    mirror_reader: GovernanceDagMirrorReadHandleV1,
}

struct GovernanceDagServiceLivenessGuard {
    readiness: Arc<GovernanceDagMirrorReadinessV1>,
}

impl GovernanceDagServiceLivenessGuard {
    fn new(reader: &GovernanceDagMirrorReadHandleV1) -> Self {
        Self {
            readiness: Arc::clone(&reader.readiness),
        }
    }
}

impl Drop for GovernanceDagServiceLivenessGuard {
    fn drop(&mut self) {
        // Read capabilities can outlive the runner (for example after task
        // cancellation). Withdraw the shared epoch synchronously so no clone
        // can authenticate a cached generation after supervision ends.
        self.readiness.mark_unready();
    }
}

struct Service {
    config: RuntimeConfig,
    mirror_store: TwoSlotStoreV1,
    checkpoint_store: OpaqueCheckpointStore,
    checkpoint_revision: Option<[u8; 32]>,
    checkpoint_generation_floor: u64,
    checkpoint: Option<CheckpointBodyV1>,
    intent_revision: Option<[u8; 32]>,
    intent_generation_floor: u64,
    intent: Option<PublishIntentBodyV1>,
    ipfs: PinnedEndpoint,
    head: PinnedEndpoint,
    api: ApiState,
    state_lock: RetainedFile,
    mirror_reader: GovernanceDagMirrorReadHandleV1,
    _liveness_guard: GovernanceDagServiceLivenessGuard,
    steady_audit_generation: Option<u64>,
    steady_audit_cursor: usize,
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
/// is unavailable or stale.
pub fn validate_governance_dag_service_runtime_providers(
    view: &SorafsGovernanceDagServiceView,
    providers: &GovernanceDagServiceRuntimeProviders,
) -> Result<(), GovernanceDagServiceError> {
    let bindings = runtime_provider_bindings(view)?;
    OpaqueAuthenticator::try_new(
        bindings.ipfs_authenticator_handle(),
        bindings.ipfs_authenticator_qualification(),
        bindings.ipfs_request_ingress_binding(),
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
    OpaqueAuthenticator::try_new(
        bindings.head_authenticator_handle(),
        bindings.head_authenticator_qualification(),
        bindings.head_request_ingress_binding(),
        providers.head_authenticator.clone().ok_or_else(|| {
            GovernanceDagServiceError::Config(
                "signed-head authentication is enabled but no runtime provider was injected"
                    .to_owned(),
            )
        })?,
        "signed-head authenticator",
    )?;
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
    let ipfs_request_ingress_binding = configured_request_ingress_binding(
        GovernanceDagAuthenticationScope::Ipfs,
        service.ipfs_api_url.as_deref().ok_or_else(|| {
            GovernanceDagServiceError::Config("IPFS API URL is missing".to_owned())
        })?,
        ipfs_request_auth_public_key,
        authenticated_ipfs_wire_body_max_bytes(service.max_request_bytes.0)?,
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
    let head_request_auth_public_key = service.head_request_auth_public_key.ok_or_else(|| {
        GovernanceDagServiceError::Config(
            "signed-head request-auth public key is missing".to_owned(),
        )
    })?;
    let head_request_ingress_binding = configured_request_ingress_binding(
        GovernanceDagAuthenticationScope::SignedHead,
        service.signed_head_url.as_deref().ok_or_else(|| {
            GovernanceDagServiceError::Config("signed head URL is missing".to_owned())
        })?,
        head_request_auth_public_key,
        service.max_request_bytes.0,
        service.request_auth_max_envelope_lifetime_secs,
        service.request_auth_max_future_skew_secs,
        "signed-head authenticator",
    )?;
    let head_authenticator_handle = validate_runtime_handle(
        service
            .head_authenticator_handle
            .as_deref()
            .ok_or_else(|| {
                GovernanceDagServiceError::Config(
                    "signed-head authenticator handle is missing".to_owned(),
                )
            })?,
        "signed-head authenticator",
    )?;
    let head_authenticator_qualification = configured_provider_qualification(
        service.head_authenticator_revision,
        service.head_authenticator_policy_digest,
        "signed-head authenticator",
    )?;
    Ok(GovernanceDagServiceRuntimeProviderBindingsV1 {
        ipfs_authenticator_handle,
        ipfs_authenticator_qualification,
        ipfs_request_ingress_binding,
        head_authenticator_handle,
        head_authenticator_qualification,
        head_request_ingress_binding,
        checkpoint_store_handle,
        checkpoint_store_qualification,
    })
}

fn configured_request_ingress_binding(
    scope: GovernanceDagAuthenticationScope,
    endpoint: &str,
    public_key: [u8; 32],
    max_body_bytes: u64,
    max_envelope_lifetime_secs: u64,
    max_future_skew_secs: u64,
    label: &str,
) -> Result<GovernanceDagRequestIngressBindingV1, GovernanceDagServiceError> {
    let endpoint_binding = governance_dag_request_ingress_endpoint_binding_v1(scope, endpoint)
        .map_err(|_| {
            GovernanceDagServiceError::Config(format!(
                "{label} endpoint is not a canonical request-ingress binding"
            ))
        })?;
    GovernanceDagRequestIngressBindingV1::try_new(
        scope,
        endpoint_binding,
        public_key,
        max_body_bytes,
        max_envelope_lifetime_secs,
        max_future_skew_secs,
    )
    .map_err(|_| {
        GovernanceDagServiceError::Config(format!("{label} request-ingress binding is invalid"))
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
    /// Clone the service-owned coherent mirror read capability before the
    /// runner is moved into [`Self::run`] or [`Self::run_until`].
    #[must_use]
    pub fn mirror_read_handle(&self) -> GovernanceDagMirrorReadHandleV1 {
        self.service.mirror_reader.clone()
    }

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
        let router = service_router(ServiceApiState {
            telemetry: self.service.api.clone(),
            mirror_reader: self.service.mirror_reader.clone(),
        });
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
        let ipfs_request_ingress_binding = configured_request_ingress_binding(
            GovernanceDagAuthenticationScope::Ipfs,
            &ipfs_url,
            service.ipfs_request_auth_public_key.ok_or_else(|| {
                GovernanceDagServiceError::Config(
                    "IPFS request-auth public key is missing".to_owned(),
                )
            })?,
            authenticated_ipfs_wire_body_max_bytes(service.max_request_bytes.0)?,
            service.request_auth_max_envelope_lifetime_secs,
            service.request_auth_max_future_skew_secs,
            "IPFS authenticator",
        )?;
        let ipfs_authenticator = OpaqueAuthenticator::try_new(
            ipfs_authenticator_handle,
            ipfs_authenticator_qualification,
            ipfs_request_ingress_binding,
            ipfs_authenticator.ok_or_else(|| {
                GovernanceDagServiceError::Config(
                    "IPFS authentication is enabled but no runtime provider was injected"
                        .to_owned(),
                )
            })?,
            "IPFS authenticator",
        )?;
        let head_authenticator_handle =
            service
                .head_authenticator_handle
                .as_deref()
                .ok_or_else(|| {
                    GovernanceDagServiceError::Config(
                        "signed-head authenticator handle is missing".to_owned(),
                    )
                })?;
        let head_authenticator_qualification = configured_provider_qualification(
            service.head_authenticator_revision,
            service.head_authenticator_policy_digest,
            "signed-head authenticator",
        )?;
        let signed_head_url = service.signed_head_url.clone().ok_or_else(|| {
            GovernanceDagServiceError::Config("signed head URL is missing".to_owned())
        })?;
        let head_request_ingress_binding = configured_request_ingress_binding(
            GovernanceDagAuthenticationScope::SignedHead,
            &signed_head_url,
            service.head_request_auth_public_key.ok_or_else(|| {
                GovernanceDagServiceError::Config(
                    "signed-head request-auth public key is missing".to_owned(),
                )
            })?,
            service.max_request_bytes.0,
            service.request_auth_max_envelope_lifetime_secs,
            service.request_auth_max_future_skew_secs,
            "signed-head authenticator",
        )?;
        let head_authenticator = OpaqueAuthenticator::try_new(
            head_authenticator_handle,
            head_authenticator_qualification,
            head_request_ingress_binding,
            head_authenticator.ok_or_else(|| {
                GovernanceDagServiceError::Config(
                    "signed-head authentication is enabled but no runtime provider was injected"
                        .to_owned(),
                )
            })?,
            "signed-head authenticator",
        )?;

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
            max_future_skew_secs: service.max_future_skew_secs,
            allow_head_bootstrap: service.allow_head_bootstrap,
            expected_producer_signer_handle,
            expected_producer_signer_qualification,
            expected_checkpoint_store_handle: checkpoint_store.handle.clone(),
            expected_checkpoint_store_qualification: checkpoint_store.qualification,
            expected_publisher_peer_id,
            expected_public_key,
        };
        let (checkpoint, checkpoint_revision) = load_checkpoint(&checkpoint_store)?;
        let (intent, intent_revision) = load_publish_intent(&checkpoint_store)?;
        let mirror_store = open_mirror_index_store(&runtime_config)?;
        let mirror_reader = GovernanceDagMirrorReadHandleV1::try_new(
            &runtime_config,
            checkpoint_store.clone(),
            checkpoint.is_none() && intent.is_none(),
        )?;
        let ipfs = build_pinned_endpoint(
            &ipfs_url,
            ipfs_authenticator,
            GovernanceDagAuthenticationScope::Ipfs,
            &service,
            true,
        )
        .await?;
        let head = build_pinned_endpoint(
            &signed_head_url,
            head_authenticator,
            GovernanceDagAuthenticationScope::SignedHead,
            &service,
            false,
        )
        .await?;
        let checkpoint_generation_floor = checkpoint
            .as_ref()
            .map_or(0, |checkpoint| checkpoint.generation);
        let intent_generation_floor = intent.as_ref().map_or(0, |intent| intent.generation);
        let api = ApiState(Arc::new(RwLock::new(ApiSnapshot::default())));
        let liveness_guard = GovernanceDagServiceLivenessGuard::new(&mirror_reader);
        Ok(Self {
            config: runtime_config,
            mirror_store,
            checkpoint_store,
            checkpoint_revision,
            checkpoint_generation_floor,
            checkpoint,
            intent_revision,
            intent_generation_floor,
            intent,
            ipfs,
            head,
            api,
            state_lock,
            mirror_reader,
            _liveness_guard: liveness_guard,
            steady_audit_generation: None,
            steady_audit_cursor: 0,
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

#[cfg(test)]
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
    DecodeLimits::new(
        max,
        max,
        max.saturating_mul(2),
        max.saturating_mul(CANONICAL_DECODE_ALLOCATION_MULTIPLIER),
        128,
    )
}

fn mirror_index_store_config() -> Result<TwoSlotStoreConfigV1, GovernanceDagServiceError> {
    TwoSlotStoreConfigV1::try_new(
        MIRROR_INDEX_STORE_NAME,
        blake3_array(b"sorafs.governance-dag.service.mirror-index.store-domain.v1\0"),
        blake3_array(b"sorafs.governance-dag.service.mirror-index.stable-store-id.v1\0"),
        MIRROR_INDEX_STORE_MAX_PAYLOAD_BYTES,
    )
    .map_err(|error| {
        GovernanceDagServiceError::State(format!(
            "mirror two-slot store configuration is invalid: {error}"
        ))
    })
}

fn validate_mirror_index_store_payload(
    payload: &MirrorIndexStorePayloadV1,
) -> Result<(), GovernanceDagServiceError> {
    if payload.version != MIRROR_INDEX_STORE_PAYLOAD_VERSION_V1 {
        return Err(GovernanceDagServiceError::State(
            "mirror two-slot payload version is unsupported".to_owned(),
        ));
    }
    if payload.is_empty() {
        if payload.publish_intent_blake3 != [0; 32]
            || payload.mirror_blake3 != [0; 32]
            || !payload.canonical_json.is_empty()
        {
            return Err(GovernanceDagServiceError::State(
                "empty mirror two-slot payload contains committed fields".to_owned(),
            ));
        }
        return Ok(());
    }
    if payload.canonical_json.is_empty()
        || payload.canonical_json.len() as u64 > GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1
        || blake3_array(&payload.canonical_json) != payload.mirror_blake3
    {
        return Err(GovernanceDagServiceError::State(
            "committed mirror two-slot payload violates its byte or digest binding".to_owned(),
        ));
    }
    let mirror: JsonValue = json::from_slice(&payload.canonical_json).map_err(|error| {
        GovernanceDagServiceError::State(format!(
            "mirror two-slot JSON payload is invalid: {error}"
        ))
    })?;
    let canonical = json::to_json_pretty(&mirror).map_err(|error| {
        GovernanceDagServiceError::State(format!(
            "mirror two-slot JSON canonicalization failed: {error}"
        ))
    })?;
    if canonical.as_bytes() != payload.canonical_json
        || mirror.get("schema").and_then(JsonValue::as_str) != Some(MIRROR_INDEX_SCHEMA)
        || mirror.get("generation").and_then(JsonValue::as_u64)
            != Some(payload.checkpoint_generation)
    {
        return Err(GovernanceDagServiceError::State(
            "mirror two-slot JSON is noncanonical or disagrees with its typed metadata".to_owned(),
        ));
    }
    Ok(())
}

fn encode_mirror_index_store_payload(
    payload: &MirrorIndexStorePayloadV1,
) -> Result<Vec<u8>, GovernanceDagServiceError> {
    validate_mirror_index_store_payload(payload)?;
    let encoded = norito::to_bytes(payload).map_err(|error| {
        GovernanceDagServiceError::State(format!("mirror two-slot payload encode failed: {error}"))
    })?;
    if encoded.len() > MIRROR_INDEX_STORE_MAX_PAYLOAD_BYTES {
        return Err(GovernanceDagServiceError::State(
            "mirror two-slot payload exceeds its fixed-store bound".to_owned(),
        ));
    }
    Ok(encoded)
}

fn decode_mirror_index_store_payload(
    encoded: &[u8],
) -> Result<MirrorIndexStorePayloadV1, GovernanceDagServiceError> {
    if encoded.len() > MIRROR_INDEX_STORE_MAX_PAYLOAD_BYTES {
        return Err(GovernanceDagServiceError::State(
            "mirror two-slot payload exceeds its fixed-store bound".to_owned(),
        ));
    }
    let allocation = MIRROR_INDEX_STORE_MAX_PAYLOAD_BYTES.saturating_mul(2);
    let payload: MirrorIndexStorePayloadV1 = norito::decode_from_bytes_with_limits(
        encoded,
        DecodeLimits::new(
            MIRROR_INDEX_STORE_MAX_PAYLOAD_BYTES,
            MIRROR_INDEX_STORE_MAX_PAYLOAD_BYTES,
            MIRROR_INDEX_STORE_MAX_PAYLOAD_BYTES,
            allocation,
            16,
        ),
    )
    .map_err(|error| {
        GovernanceDagServiceError::State(format!("mirror two-slot payload decode failed: {error}"))
    })?;
    if encode_mirror_index_store_payload(&payload)? != encoded {
        return Err(GovernanceDagServiceError::State(
            "mirror two-slot payload is not canonical Norito".to_owned(),
        ));
    }
    Ok(payload)
}

fn load_mirror_index_store(
    config: &RuntimeConfig,
    store: &TwoSlotStoreV1,
) -> Result<(TwoSlotSnapshotV1, MirrorIndexStorePayloadV1), GovernanceDagServiceError> {
    load_mirror_index_store_from_root(&config.state_root_guard, store)
}

fn load_mirror_index_store_from_root(
    state_root_guard: &GovernanceFilesystemRootGuard,
    store: &TwoSlotStoreV1,
) -> Result<(TwoSlotSnapshotV1, MirrorIndexStorePayloadV1), GovernanceDagServiceError> {
    state_root_guard.revalidate().map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "Governance DAG mirror state root identity changed: {error}"
        ))
    })?;
    let snapshot = store.load().map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "cannot load rooted mirror two-slot store: {error}"
        ))
    })?;
    let payload = decode_mirror_index_store_payload(snapshot.payload())?;
    state_root_guard.revalidate().map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "Governance DAG mirror state root identity changed during read: {error}"
        ))
    })?;
    Ok((snapshot, payload))
}

fn open_mirror_index_store(
    config: &RuntimeConfig,
) -> Result<TwoSlotStoreV1, GovernanceDagServiceError> {
    reject_legacy_mirror_index_authority(config)?;
    let initial = encode_mirror_index_store_payload(&MirrorIndexStorePayloadV1::empty())?;
    let store_config = mirror_index_store_config()?;
    config.revalidate_state_root()?;
    let store = config
        .state_root_guard
        .rooted_directory()
        .open_or_create_two_slot_store_v1(store_config, &initial)
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot open rooted mirror two-slot store: {error}"
            ))
        })?;
    let _ = load_mirror_index_store(config, &store)?;
    Ok(store)
}

fn reject_legacy_mirror_index_authority(
    config: &RuntimeConfig,
) -> Result<(), GovernanceDagServiceError> {
    config.revalidate_state_root()?;
    for name in config
        .state_root_guard
        .rooted_directory()
        .child_names_bounded(SOURCE_ENTRY_HARD_CAP)
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot inventory the rooted mirror authority: {error}"
            ))
        })?
    {
        let Some(name) = name.to_str() else {
            continue;
        };
        let claims_legacy_authority = [
            LEGACY_MIRROR_INDEX_FILE,
            LEGACY_MIRROR_INDEX_SIDECAR_FILE,
            LEGACY_MIRROR_RECOVERY_QUARANTINE_DIR,
        ]
        .into_iter()
        .any(|target| {
            name == target
                || name
                    .strip_prefix(target)
                    .or_else(|| {
                        name.strip_prefix('.')
                            .and_then(|name| name.strip_prefix(target))
                    })
                    .is_some_and(|suffix| {
                        suffix.starts_with(".tmp-") || suffix.starts_with(".retained-v1-")
                    })
        });
        if claims_legacy_authority {
            return Err(GovernanceDagServiceError::State(format!(
                "legacy mirror authority `{name}` is unsupported; archive or remove it offline before first-release initialization"
            )));
        }
    }
    config.revalidate_state_root()
}

fn mirror_json_value(
    payload: &MirrorIndexStorePayloadV1,
) -> Result<JsonValue, GovernanceDagServiceError> {
    validate_mirror_index_store_payload(payload)?;
    if payload.is_empty() {
        return Err(GovernanceDagServiceError::State(
            "mirror two-slot store has no committed index".to_owned(),
        ));
    }
    json::from_slice(&payload.canonical_json).map_err(|error| {
        GovernanceDagServiceError::State(format!(
            "mirror two-slot JSON payload is invalid: {error}"
        ))
    })
}

fn verify_mirror_payload_against_checkpoint(
    payload: &MirrorIndexStorePayloadV1,
    checkpoint: &CheckpointBodyV1,
) -> Result<JsonValue, GovernanceDagServiceError> {
    if payload.checkpoint_generation != checkpoint.generation
        || payload.mirror_blake3 != checkpoint.mirror_blake3
    {
        return Err(GovernanceDagServiceError::State(
            "mirror two-slot metadata does not match the authenticated checkpoint".to_owned(),
        ));
    }
    let value = mirror_json_value(payload)?;
    let expected_head_cid = hex::encode(&checkpoint.head_block_cid);
    if value.get("generation").and_then(JsonValue::as_u64) != Some(checkpoint.generation)
        || value
            .get("head")
            .and_then(|head| head.get("head_block_cid_hex"))
            .and_then(JsonValue::as_str)
            != Some(expected_head_cid.as_str())
    {
        return Err(GovernanceDagServiceError::State(
            "mirror two-slot JSON metadata is inconsistent with the checkpoint".to_owned(),
        ));
    }
    Ok(value)
}

fn verify_mirror_payload_against_intent(
    payload: &MirrorIndexStorePayloadV1,
    intent: &PublishIntentBodyV1,
) -> Result<(), GovernanceDagServiceError> {
    let value = mirror_json_value(payload)?;
    let expected_head_cid = hex::encode(&intent.target_head_block_cid);
    let intent_bytes = norito::to_bytes(intent).map_err(|error| {
        GovernanceDagServiceError::State(format!(
            "publish intent encode failed while verifying mirror ownership: {error}"
        ))
    })?;
    if payload.publish_intent_blake3 != blake3_array(&intent_bytes)
        || payload.checkpoint_generation != intent.generation
        || value.get("generation").and_then(JsonValue::as_u64) != Some(intent.generation)
        || value.get("block_count").and_then(JsonValue::as_u64) != Some(intent.target_block_count)
        || value
            .get("head")
            .and_then(|head| head.get("head_block_cid_hex"))
            .and_then(JsonValue::as_str)
            != Some(expected_head_cid.as_str())
    {
        return Err(GovernanceDagServiceError::State(
            "in-progress mirror two-slot candidate is not bound to the sealed publish intent"
                .to_owned(),
        ));
    }
    Ok(())
}

fn load_checkpoint(
    store: &OpaqueCheckpointStore,
) -> Result<(Option<CheckpointBodyV1>, Option<[u8; 32]>), GovernanceDagServiceError> {
    let Some(record) = load_sealed_record(store, GovernanceDagSealedStateSlot::Checkpoint)? else {
        return Ok((None, None));
    };
    let body: CheckpointBodyV1 = norito::decode_from_bytes_with_limits(
        &record.payload,
        durable_decode_limits(GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1),
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
        || body.head_bytes.is_empty()
        || body.head_bytes.len() as u64 > GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1
        || blake3_array(&body.head_bytes) != body.head_bytes_blake3
        || body.head_bytes_blake3 == [0; 32]
        || body.source_chain_blake3 == [0; 32]
        || body.mirror_blake3 == [0; 32]
        || !is_canonical_cid_v1(&body.head_ipfs_cid)
        || canonical_ipfs_file_cid(&body.head_bytes).as_deref() != Some(body.head_ipfs_cid.as_str())
        || body.mirror_blocks.is_empty()
        || body.mirror_blocks.len() > GOVERNANCE_DAG_MIRROR_MAX_ENTRIES_V1
    {
        return Err(GovernanceDagServiceError::State(
            "checkpoint fields violate first-release bounds".to_owned(),
        ));
    }
    let mut previous: Option<u64> = None;
    let mut seen = BTreeSet::new();
    for block in &body.mirror_blocks {
        validate_published_block(block)?;
        if previous.is_some_and(|value| value.checked_add(1) != Some(block.sequence))
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
        durable_decode_limits(GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1),
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
        || record.payload.len() as u64 > GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1
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
        durable_decode_limits(GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1),
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
    let committed = load_runtime_dag_committed_snapshot_v1(&config.source_root_guard)
        .map_err(|error| {
            GovernanceDagServiceError::Source(format!(
                "typed runtime DAG committed state is invalid: {error}"
            ))
        })?
        .ok_or_else(|| {
            GovernanceDagServiceError::Conflict(
                "sealed local producer checkpoint has no typed head/index generation".to_owned(),
            )
        })?;
    let checkpoint = &first_guard.checkpoint;
    if checkpoint.block_count == 0
        || checkpoint.head_bytes_digest != blake3_array(committed.head_bytes())
        || checkpoint.index_bytes_digest != blake3_array(committed.index_bytes())
    {
        return Err(GovernanceDagServiceError::Conflict(
            "typed runtime DAG byte generation does not match the sealed local producer checkpoint"
                .to_owned(),
        ));
    }
    let source = load_source_snapshot_from_committed(config, committed)?;
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
    if payload.is_empty()
        || payload.len() as u64 > GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1
    {
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
        || body.target_head_bytes.len() as u64 > GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1
        || body.target_head_blake3 == [0; 32]
        || body.target_source_chain_blake3 == [0; 32]
        || body.blocks.is_empty()
        || body.blocks.len() > SOURCE_ENTRY_HARD_CAP
    {
        return Err(GovernanceDagServiceError::State(
            "publish intent fields violate first-release bounds".to_owned(),
        ));
    }
    let mut previous: Option<u64> = None;
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
        if previous.is_some_and(|value| value.checked_add(1) != Some(block.sequence))
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

fn publish_intent_is_monotonic_refinement(
    previous: &PublishIntentBodyV1,
    next: &PublishIntentBodyV1,
) -> bool {
    if previous.generation != next.generation
        || previous.version != next.version
        || previous.target_head_block_cid != next.target_head_block_cid
        || previous.target_block_count != next.target_block_count
        || previous.target_head_bytes != next.target_head_bytes
        || previous.target_head_blake3 != next.target_head_blake3
        || previous.target_source_chain_blake3 != next.target_source_chain_blake3
        || previous.previous_public_head_blake3 != next.previous_public_head_blake3
        || previous.created_at_unix != next.created_at_unix
        || previous.blocks.len() != next.blocks.len()
        || previous
            .head_ipfs_cid
            .as_ref()
            .is_some_and(|cid| next.head_ipfs_cid.as_ref() != Some(cid))
    {
        return false;
    }
    previous
        .blocks
        .iter()
        .zip(&next.blocks)
        .all(|(previous, next)| {
            previous.sequence == next.sequence
                && previous.governance_block_cid == next.governance_block_cid
                && previous.governance_node_cid == next.governance_node_cid
                && previous.payload_kind == next.payload_kind
                && previous.timestamp == next.timestamp
                && previous.encoded_blake3 == next.encoded_blake3
                && previous.encoded_len == next.encoded_len
                && previous
                    .ipfs_cid
                    .as_ref()
                    .is_none_or(|cid| next.ipfs_cid.as_ref() == Some(cid))
        })
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

fn require_exact_runtime_index_fields(
    map: &JsonMap,
    expected: &[&str],
    context: &str,
) -> Result<(), GovernanceDagServiceError> {
    if map.len() != expected.len() || !expected.iter().all(|field| map.contains_key(*field)) {
        return Err(GovernanceDagServiceError::Source(format!(
            "{context} fields do not match the first-release schema"
        )));
    }
    Ok(())
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

fn required_optional_json_string(
    map: &JsonMap,
    field: &str,
) -> Result<Option<String>, GovernanceDagServiceError> {
    match map.get(field) {
        Some(JsonValue::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| {
                GovernanceDagServiceError::Source(format!(
                    "runtime index `{field}` is not a string or null"
                ))
            }),
        None => Err(GovernanceDagServiceError::Source(format!(
            "runtime index is missing `{field}`"
        ))),
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

#[cfg(test)]
fn load_source_snapshot(
    config: &RuntimeConfig,
) -> Result<SourceSnapshot, GovernanceDagServiceError> {
    config.revalidate_source_root()?;
    let committed = load_runtime_dag_committed_snapshot_v1(&config.source_root_guard)
        .map_err(|error| {
            GovernanceDagServiceError::Source(format!(
                "typed runtime DAG committed state is invalid: {error}"
            ))
        })?
        .ok_or_else(|| {
            GovernanceDagServiceError::Source(
                "typed runtime DAG committed state has no head/index generation".to_owned(),
            )
        })?;
    load_source_snapshot_from_committed(config, committed)
}

fn load_source_snapshot_from_committed(
    config: &RuntimeConfig,
    committed: RuntimeDagCommittedSnapshotV1,
) -> Result<SourceSnapshot, GovernanceDagServiceError> {
    config.revalidate_source_root()?;
    let index_bytes = committed.index_bytes().to_vec();
    let index_blake3 = blake3_array(&index_bytes);
    let index: JsonValue = json::from_slice(&index_bytes).map_err(|err| {
        GovernanceDagServiceError::Source(format!("runtime index JSON is invalid: {err}"))
    })?;
    let canonical_index = json::to_json_pretty(&index).map_err(|error| {
        GovernanceDagServiceError::Source(format!(
            "runtime index JSON canonicalization failed: {error}"
        ))
    })?;
    if canonical_index.as_bytes() != index_bytes {
        return Err(GovernanceDagServiceError::Source(
            "runtime index JSON bytes are not canonical".to_owned(),
        ));
    }
    let map = index.as_object().ok_or_else(|| {
        GovernanceDagServiceError::Source("runtime index root is not an object".to_owned())
    })?;
    require_exact_runtime_index_fields(map, RUNTIME_INDEX_TOP_LEVEL_FIELDS_V1, "runtime index")?;
    if map.get("schema").and_then(JsonValue::as_str) != Some(RUNTIME_INDEX_SCHEMA) {
        return Err(GovernanceDagServiceError::Source(
            "runtime index schema is unsupported".to_owned(),
        ));
    }
    if map.contains_key("head_path") {
        return Err(GovernanceDagServiceError::Source(
            "runtime index contains the obsolete loose-head authority field".to_owned(),
        ));
    }
    if required_json_string(map, "source")? != RUNTIME_INDEX_SOURCE
        || required_json_string(map, "root")? != RUNTIME_INDEX_LOGICAL_ROOT
    {
        return Err(GovernanceDagServiceError::Source(
            "runtime index source or logical root marker is invalid".to_owned(),
        ));
    }
    if required_json_string(map, "signer_handle")? != config.expected_producer_signer_handle
        || required_json_u64(map, "signer_revision")?
            != config.expected_producer_signer_qualification.revision
        || decode_fixed_hex::<32>(
            &required_json_string(map, "signer_policy_digest_hex")?,
            "runtime index signer policy digest",
        )? != config.expected_producer_signer_qualification.policy_digest
        || required_json_string(map, "checkpoint_store_handle")?
            != config.expected_checkpoint_store_handle
        || required_json_u64(map, "checkpoint_store_revision")?
            != config.expected_checkpoint_store_qualification.revision
        || decode_fixed_hex::<32>(
            &required_json_string(map, "checkpoint_store_policy_digest_hex")?,
            "runtime index checkpoint-store policy digest",
        )? != config.expected_checkpoint_store_qualification.policy_digest
    {
        return Err(GovernanceDagServiceError::Source(
            "runtime index provider binding does not match the qualified source boundary"
                .to_owned(),
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
    if required_json_string(map, "publisher_peer_id")?
        != String::from_utf8_lossy(&config.expected_publisher_peer_id).as_ref()
    {
        return Err(GovernanceDagServiceError::Source(
            "runtime index publisher peer text does not match configuration".to_owned(),
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
    let mut expected_block_names = BTreeSet::<OsString>::new();
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
        require_exact_runtime_index_fields(
            entry,
            RUNTIME_INDEX_BLOCK_FIELDS_V1,
            "runtime index block",
        )?;
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
        if required_json_u64(entry, "published_at_unix")? != block.timestamp {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} publication timestamp is invalid"
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
        expected_block_names.insert(
            path.file_name()
                .ok_or_else(|| {
                    GovernanceDagServiceError::Source(format!(
                        "runtime index block {position} path has no file name"
                    ))
                })?
                .to_os_string(),
        );
        expected_block_names.insert(
            digest_sidecar_path(&path)
                .file_name()
                .ok_or_else(|| {
                    GovernanceDagServiceError::Source(format!(
                        "runtime index block {position} sidecar path has no file name"
                    ))
                })?
                .to_os_string(),
        );
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
        let indexed_submission_account_digest =
            required_optional_json_string(entry, "submission_publisher_account_digest_hex")?;
        let indexed_submission_origin = required_optional_json_string(entry, "submission_origin")?;
        let signed_submission_account_digest = block
            .node
            .submission_provenance
            .as_ref()
            .map(|provenance| hex::encode(provenance.publisher_account_digest));
        let signed_submission_origin = block
            .node
            .submission_provenance
            .as_ref()
            .map(|provenance| provenance.origin.label().to_owned());
        if indexed_submission_account_digest != signed_submission_account_digest
            || indexed_submission_origin != signed_submission_origin
        {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} submission provenance does not match its signed governance node"
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

        let source_payload_path_string = required_json_string(entry, "encoded_path")?;
        let source_payload_path = resolve_index_relative_path(&source_payload_path_string)?;
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
        let json_path_string = required_json_string(entry, "json_path")?;
        let json_path = resolve_index_relative_path(&json_path_string)?;
        let json_bytes = read_verified_sidecar_file(
            &config.source_root_guard,
            &json_path,
            u64::try_from(GOVERNANCE_MUTABLE_INDEX_MAX_BYTES).map_err(|_| {
                GovernanceDagServiceError::Source(
                    "canonical Governance DAG JSON-source ceiling exceeds host limits".to_owned(),
                )
            })?,
            None,
        )?;
        validate_governance_car_source_lengths(source_payload_bytes.len(), json_bytes.len())
            .map_err(|error| GovernanceDagServiceError::Source(error.to_string()))?;
        let json_len = u64::try_from(json_bytes.len()).map_err(|_| {
            GovernanceDagServiceError::Source(format!(
                "runtime index block {position} JSON source length exceeds u64"
            ))
        })?;
        let json_digest = blake3_array(&json_bytes);
        total_bytes = total_bytes.checked_add(json_len).ok_or_else(|| {
            GovernanceDagServiceError::Source("source byte count overflow".to_owned())
        })?;
        if total_bytes > SOURCE_TOTAL_BYTES_HARD_CAP {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime DAG exceeds the {SOURCE_TOTAL_BYTES_HARD_CAP} byte hard cap"
            )));
        }
        let expected_source_paths = governance_source_pair_relative_paths(
            &kind,
            source_payload_len,
            &hex::encode(source_payload_digest),
            json_len,
            &hex::encode(json_digest),
        )
        .map_err(|error| GovernanceDagServiceError::Source(error.to_string()))?;
        if source_payload_path_string != expected_source_paths.0
            || json_path_string != expected_source_paths.1
        {
            return Err(GovernanceDagServiceError::Source(format!(
                "runtime index block {position} source paths do not bind their immutable bytes"
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

    let runtime_root = config
        .source_root_guard
        .rooted_directory()
        .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_DIR))
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot open authenticated runtime DAG root: {error}"
            ))
        })?;
    if runtime_root.child_names_bounded(2).map_err(|error| {
        GovernanceDagServiceError::Filesystem(format!(
            "cannot inventory authenticated runtime DAG root: {error}"
        ))
    })? != vec![OsString::from(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR)]
    {
        return Err(GovernanceDagServiceError::Source(
            "runtime DAG immutable root inventory is noncanonical".to_owned(),
        ));
    }
    let blocks_directory = runtime_root
        .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR))
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot open authenticated runtime DAG blocks directory: {error}"
            ))
        })?;
    let inventory_bound = GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1
        .checked_mul(2)
        .and_then(|bound| bound.checked_add(1))
        .ok_or_else(|| {
            GovernanceDagServiceError::State("runtime DAG inventory bound overflowed".to_owned())
        })?;
    let actual_block_names = blocks_directory
        .child_names_bounded(inventory_bound)
        .map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "cannot inventory authenticated runtime DAG blocks: {error}"
            ))
        })?
        .into_iter()
        .collect::<BTreeSet<_>>();
    if actual_block_names != expected_block_names {
        return Err(GovernanceDagServiceError::Source(
            "runtime DAG block inventory contains an unindexed or missing artifact".to_owned(),
        ));
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

    let head_bytes = committed.head_bytes().to_vec();
    let head: GovernanceDagHeadV1 = decode_canonical(&head_bytes, "governance DAG head")?;
    validate_source_head_chain(&head, &decoded_blocks)?;
    if head.generated_at > latest_allowed
        || blocks
            .last()
            .is_some_and(|block| head.generated_at < block.block.timestamp)
    {
        return Err(GovernanceDagServiceError::Source(
            "signed head is future-dated or predates its tip".to_owned(),
        ));
    }
    if required_json_string(map, "head_block_cid_hex")? != hex::encode(&head.head_block_cid)
        || required_json_u64(map, "head_generated_at")? != head.generated_at
        || required_json_u64(map, "generated_at")? != head.generated_at
    {
        return Err(GovernanceDagServiceError::Source(
            "runtime index head metadata does not match signed head bytes".to_owned(),
        ));
    }
    let stable_committed = load_runtime_dag_committed_snapshot_v1(&config.source_root_guard)
        .map_err(|error| {
            GovernanceDagServiceError::Source(format!(
                "typed runtime DAG committed state changed invalidly while reading: {error}"
            ))
        })?
        .ok_or_else(|| {
            GovernanceDagServiceError::Source(
                "typed runtime DAG committed state disappeared while reading".to_owned(),
            )
        })?;
    if stable_committed.store_identity() != committed.store_identity()
        || stable_committed != committed
    {
        return Err(GovernanceDagServiceError::Source(
            "typed runtime DAG head/index generation changed while the source snapshot was being read"
                .to_owned(),
        ));
    }
    let chain_blake3 = source_chain_blake3_v1(&head_bytes, &blocks);
    let source = SourceSnapshot {
        index_blake3,
        chain_blake3,
        head,
        head_bytes,
        blocks,
    };
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
    let authenticated_wire_body_max_bytes = if ipfs_base {
        authenticated_ipfs_wire_body_max_bytes(config.max_request_bytes.0)?
    } else {
        config.max_request_bytes.0
    };
    let endpoint_binding =
        governance_dag_request_ingress_endpoint_binding_v1(authentication_scope, url.as_str())
            .map_err(|_| {
                GovernanceDagServiceError::Config(
                    "endpoint URL does not match a canonical request-ingress binding".to_owned(),
                )
            })?;
    let live_binding = authenticator.ingress_binding();
    if live_binding.scope() != authentication_scope
        || live_binding.endpoint_binding() != endpoint_binding
        || live_binding.max_body_bytes() != authenticated_wire_body_max_bytes
    {
        return Err(GovernanceDagServiceError::Config(
            "endpoint URL or request bound does not match the qualified ingress provider"
                .to_owned(),
        ));
    }
    authenticator.assert_identity()?;

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
        authenticated_wire_body_max_bytes,
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
    fn validate_qualified_request_url(&self, url: &Url) -> Result<(), GovernanceDagServiceError> {
        let same_origin = url.scheme() == self.url.scheme()
            && url.host_str() == self.url.host_str()
            && url.port_or_known_default() == self.url.port_or_known_default()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && !url.path().contains('%');
        let qualified = match self.authentication_scope {
            GovernanceDagAuthenticationScope::SignedHead => same_origin && url == &self.url,
            GovernanceDagAuthenticationScope::Ipfs => {
                let base_path = self.url.path();
                let within_base = if base_path.ends_with('/') {
                    url.path().starts_with(base_path)
                } else {
                    url.path() == base_path
                };
                same_origin && within_base
            }
        };
        if !qualified {
            return Err(GovernanceDagServiceError::Network(
                "Governance DAG request URL is outside the qualified ingress endpoint".to_owned(),
            ));
        }
        Ok(())
    }

    fn request(
        &self,
        method: Method,
        url: Url,
    ) -> Result<reqwest::RequestBuilder, GovernanceDagServiceError> {
        self.validate_qualified_request_url(&url)?;
        Ok(self
            .client
            .request(method, url)
            .header(header::ACCEPT_ENCODING.as_str(), "identity"))
    }

    async fn execute(
        &self,
        request: RequestBuilder,
        failure: &'static str,
    ) -> Result<AuthenticatedResponse, GovernanceDagServiceError> {
        // Build exactly once after the caller has attached its final byte body
        // and conditional headers. The runtime adapter receives only the
        // bounded data-only descriptor and cannot mutate HTTP state.
        let mut request = request.build().map_err(|_| {
            GovernanceDagServiceError::Network(
                "Governance DAG outbound request could not be finalized".to_owned(),
            )
        })?;
        self.validate_qualified_request_url(request.url())?;
        let descriptor = canonical_outbound_request_descriptor(
            &request,
            self.authentication_scope,
            self.authenticated_wire_body_max_bytes,
        )?;
        let envelope = self.authenticator.authenticate(&descriptor)?;
        attach_request_authentication_headers(&mut request, &envelope)?;
        let response = self.client.execute(request).await;
        // A provider may be revoked or substituted while the request is in
        // flight. Discard the response unless the same qualified identity is
        // still active after execution.
        self.authenticator.assert_identity()?;
        Ok(AuthenticatedResponse {
            response: response
                .map_err(|_| GovernanceDagServiceError::Network(failure.to_owned()))?,
            authenticator: self.authenticator.clone(),
        })
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
    authenticated_wire_body_max_bytes: u64,
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
        authenticated_wire_body_max_bytes,
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
    response: AuthenticatedResponse,
    max_bytes: u64,
) -> Result<Vec<u8>, GovernanceDagServiceError> {
    let AuthenticatedResponse {
        mut response,
        authenticator,
    } = response;
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
    authenticator.assert_identity()?;
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

#[cfg(test)]
fn validate_ipfs_cid_for_bytes(
    value: &str,
    bytes: &[u8],
) -> Result<String, GovernanceDagServiceError> {
    let cid = validate_ipfs_cid(value)?;
    let expected = canonical_ipfs_file_cid(bytes).ok_or_else(|| {
        GovernanceDagServiceError::Network(
            "local IPFS object cannot be represented by the fixed UnixFS profile".to_owned(),
        )
    })?;
    if cid != expected {
        return Err(GovernanceDagServiceError::Network(
            "IPFS API returned a CID that does not commit to the uploaded bytes".to_owned(),
        ));
    }
    Ok(cid)
}

fn canonical_raw_sha256_cid(bytes: &[u8]) -> String {
    encode_base32_lower_no_pad(&canonical_sha256_cid_bytes(IPFS_RAW_CODEC, bytes))
}

fn canonical_ipfs_file_cid(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    if bytes.len() <= IPFS_UNIXFS_CHUNK_BYTES {
        return Some(canonical_raw_sha256_cid(bytes));
    }
    let chunk_count = bytes.len().div_ceil(IPFS_UNIXFS_CHUNK_BYTES);
    if chunk_count > IPFS_UNIXFS_MAX_FILE_LINKS {
        return None;
    }

    // `unixfs-v1-2025` encodes a file larger than one fixed 1 MiB chunk as
    // raw CIDv1 leaves beneath one canonical DAG-PB UnixFS File node. Iroha's
    // canonical object ceiling is far below 1,024 chunks, so this profile has
    // exactly one parent level for every admissible object.
    let mut root = Vec::with_capacity(chunk_count.saturating_mul(52).saturating_add(64));
    let mut block_sizes = Vec::with_capacity(chunk_count);
    for chunk in bytes.chunks(IPFS_UNIXFS_CHUNK_BYTES) {
        let chunk_len = u64::try_from(chunk.len()).ok()?;
        let leaf_cid = canonical_sha256_cid_bytes(IPFS_RAW_CODEC, chunk);
        let mut link = Vec::with_capacity(leaf_cid.len().saturating_add(10));
        append_protobuf_bytes(&mut link, 1, &leaf_cid);
        // Kubo's canonical DAG-PB encoder includes the empty link name.
        append_protobuf_bytes(&mut link, 2, &[]);
        // Raw-leaf cumulative DAG size is exactly the raw block length.
        append_protobuf_varint(&mut link, 3, chunk_len);
        append_protobuf_bytes(&mut root, 2, &link);
        block_sizes.push(chunk_len);
    }

    let file_size = u64::try_from(bytes.len()).ok()?;
    let mut unixfs = Vec::with_capacity(block_sizes.len().saturating_mul(5).saturating_add(16));
    append_protobuf_varint(&mut unixfs, 1, 2); // UnixFS DataType::File
    append_protobuf_varint(&mut unixfs, 3, file_size);
    for block_size in block_sizes {
        append_protobuf_varint(&mut unixfs, 4, block_size);
    }
    // Canonical DAG-PB orders Links before Data.
    append_protobuf_bytes(&mut root, 1, &unixfs);
    Some(encode_base32_lower_no_pad(&canonical_sha256_cid_bytes(
        IPFS_DAG_PB_CODEC,
        &root,
    )))
}

fn canonical_sha256_cid_bytes(codec: u64, bytes: &[u8]) -> Vec<u8> {
    const CID_VERSION_V1: u64 = 1;
    const SHA2_256_MULTIHASH: u64 = 0x12;
    const SHA2_256_DIGEST_LENGTH: u64 = 32;

    let digest = iroha_crypto::sha256(bytes);
    let mut cid = Vec::with_capacity(4 + digest.len());
    append_uvarint(&mut cid, CID_VERSION_V1);
    append_uvarint(&mut cid, codec);
    append_uvarint(&mut cid, SHA2_256_MULTIHASH);
    append_uvarint(&mut cid, SHA2_256_DIGEST_LENGTH);
    cid.extend_from_slice(&digest);
    cid
}

fn append_protobuf_bytes(encoded: &mut Vec<u8>, field: u64, bytes: &[u8]) {
    append_uvarint(encoded, (field << 3) | 2);
    append_uvarint(encoded, bytes.len() as u64);
    encoded.extend_from_slice(bytes);
}

fn append_protobuf_varint(encoded: &mut Vec<u8>, field: u64, value: u64) {
    append_uvarint(encoded, field << 3);
    append_uvarint(encoded, value);
}

fn append_uvarint(encoded: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        encoded.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    encoded.push(value as u8);
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
    let structurally_complete = digest_offset
        .checked_add(digest_len)
        .is_some_and(|end| end == bytes.len());
    structurally_complete && encode_base32_lower_no_pad(&bytes) == value
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
    let expected_cid = canonical_ipfs_file_cid(bytes).ok_or_else(|| {
        GovernanceDagServiceError::Network(
            "local IPFS object exceeds the fixed UnixFS file-DAG profile".to_owned(),
        )
    })?;
    let url = endpoint.ipfs_url("api/v0/add", IPFS_UNIXFS_V1_ADD_QUERY)?;
    let (boundary, body) = canonical_ipfs_multipart_body(name, bytes)?;
    let authenticated_wire_max = authenticated_ipfs_wire_body_max_bytes(max_request_bytes)?;
    if body.len() as u64 > authenticated_wire_max {
        return Err(GovernanceDagServiceError::Network(
            "IPFS multipart request exceeds the authenticated wire-body ceiling".to_owned(),
        ));
    }
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
    let cid = validate_ipfs_cid(cid)?;
    if cid != expected_cid {
        return Err(GovernanceDagServiceError::Network(
            "IPFS API returned a root outside the fixed UnixFS file-DAG profile".to_owned(),
        ));
    }
    ipfs_pin(endpoint, &cid, max_response_bytes).await?;
    ipfs_verify_pin(endpoint, &cid, max_response_bytes).await?;
    let readback = ipfs_cat(endpoint, &cid, bytes.len() as u64, max_response_bytes).await?;
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
        || name.len() > IPFS_MULTIPART_FILENAME_MAX_BYTES
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
    let boundary = (0_u8..=IPFS_MULTIPART_BOUNDARY_ATTEMPTS)
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
    let overhead = ipfs_multipart_wire_overhead(boundary.len(), name.len()).ok_or_else(|| {
        GovernanceDagServiceError::Network(
            "IPFS multipart framing length exceeds host limits".to_owned(),
        )
    })?;
    let capacity = bytes.len().checked_add(overhead).ok_or_else(|| {
        GovernanceDagServiceError::Network(
            "IPFS multipart body length exceeds host limits".to_owned(),
        )
    })?;
    let mut body = Vec::with_capacity(capacity);
    body.extend_from_slice(prelude.as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(epilogue.as_bytes());
    if body.len() != capacity {
        return Err(GovernanceDagServiceError::Network(
            "IPFS multipart framing is not canonical".to_owned(),
        ));
    }
    Ok((boundary, body))
}

fn ipfs_multipart_wire_overhead(boundary_len: usize, name_len: usize) -> Option<usize> {
    const DISPOSITION_PREFIX: &[u8] = b"Content-Disposition: form-data; name=\"file\"; filename=\"";
    const DISPOSITION_SUFFIX: &[u8] = b"\"\r\n";
    const CONTENT_TYPE: &[u8] = b"Content-Type: application/vnd.ipld.raw\r\n\r\n";

    // Prelude: `--BOUNDARY\r\n`, disposition, and content type.
    // Epilogue: `\r\n--BOUNDARY--\r\n`.
    2_usize
        .checked_add(boundary_len)?
        .checked_add(2)?
        .checked_add(DISPOSITION_PREFIX.len())?
        .checked_add(name_len)?
        .checked_add(DISPOSITION_SUFFIX.len())?
        .checked_add(CONTENT_TYPE.len())?
        .checked_add(4)?
        .checked_add(boundary_len)?
        .checked_add(4)
}

/// Return the exact authenticated IPFS request-body ceiling for an object bound.
///
/// The ingress receiver authenticates the complete deterministic multipart
/// body, so its bound includes the maximum V1 boundary and filename framing.
///
/// # Errors
///
/// Returns a configuration error when the framing calculation or final sum
/// overflows the host-independent `u64` bound.
pub fn authenticated_ipfs_wire_body_max_bytes(
    object_max_bytes: u64,
) -> Result<u64, GovernanceDagServiceError> {
    let max_boundary_len = IPFS_MULTIPART_BOUNDARY_PREFIX
        .len()
        .checked_add(1 + 32 + 3)
        .ok_or_else(|| {
            GovernanceDagServiceError::Config(
                "IPFS multipart boundary ceiling exceeds host limits".to_owned(),
            )
        })?;
    let overhead =
        ipfs_multipart_wire_overhead(max_boundary_len, IPFS_MULTIPART_FILENAME_MAX_BYTES)
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| {
                GovernanceDagServiceError::Config(
                    "IPFS multipart framing ceiling exceeds host limits".to_owned(),
                )
            })?;
    object_max_bytes.checked_add(overhead).ok_or_else(|| {
        GovernanceDagServiceError::Config(
            "IPFS authenticated request-body ceiling overflows u64".to_owned(),
        )
    })
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
    expected_bytes: u64,
    control_response_max: u64,
) -> Result<Vec<u8>, GovernanceDagServiceError> {
    if expected_bytes == 0 || expected_bytes > IPFS_OBJECT_MAX_BYTES {
        return Err(GovernanceDagServiceError::Network(
            "IPFS cat object length violates the canonical Governance DAG ceiling".to_owned(),
        ));
    }
    let url = endpoint.ipfs_url("api/v0/cat", &[("arg", cid)])?;
    let request = endpoint.request(Method::POST, url)?;
    let response = endpoint.execute(request, "IPFS cat request failed").await?;
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, control_response_max).await;
        return Err(GovernanceDagServiceError::Network(format!(
            "IPFS cat returned HTTP {status}"
        )));
    }
    read_bounded_response(response, expected_bytes).await
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
    let mut etags = response.headers().get_all(header::ETAG).iter();
    let first_etag = etags.next();
    let has_duplicate_etag = etags.next().is_some();
    let etag = first_etag
        .filter(|_| !has_duplicate_etag)
        .and_then(strong_http_entity_tag)
        .ok_or_else(|| {
            GovernanceDagServiceError::Network(
                "signed-head GET has no single canonical strong ETag".to_owned(),
            )
        })?;
    let bytes = read_bounded_response(response, max_response_bytes).await?;
    Ok(PublicHead::Present { bytes, token: etag })
}

fn strong_http_entity_tag(value: &HeaderValue) -> Option<String> {
    let bytes = value.as_bytes();
    if bytes.len() < 2
        || bytes.len() > MAX_PUBLIC_TOKEN_BYTES
        || bytes.first() != Some(&b'"')
        || bytes.last() != Some(&b'"')
        || !bytes[1..bytes.len() - 1]
            .iter()
            .all(|byte| *byte == 0x21 || (0x23..=0x7e).contains(byte))
    {
        return None;
    }
    std::str::from_utf8(bytes).ok().map(str::to_owned)
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
        if let (Some(previous), Some(next)) = (&self.checkpoint, &checkpoint)
            && next.generation == previous.generation
            && (next != previous || checkpoint_revision != self.checkpoint_revision)
        {
            return Err(GovernanceDagServiceError::State(
                "sealed checkpoint store equivocated within one generation".to_owned(),
            ));
        }
        let (intent, intent_revision) = load_publish_intent(&self.checkpoint_store)?;
        let removed_intent_was_completed = self.intent.as_ref().is_some_and(|previous_intent| {
            checkpoint.as_ref().is_some_and(|checkpoint| {
                checkpoint.generation == previous_intent.generation
                    && checkpoint.head_block_cid == previous_intent.target_head_block_cid
                    && checkpoint.head_bytes_blake3 == previous_intent.target_head_blake3
                    && checkpoint.source_chain_blake3 == previous_intent.target_source_chain_blake3
            })
        });
        if self.intent.is_some() && intent.is_none() && !removed_intent_was_completed {
            return Err(GovernanceDagServiceError::State(
                "sealed checkpoint store removed an uncompleted publish intent".to_owned(),
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
        if let (Some(previous), Some(next)) = (&self.intent, &intent)
            && next.generation == previous.generation
            && !publish_intent_is_monotonic_refinement(previous, next)
        {
            return Err(GovernanceDagServiceError::State(
                "sealed publish intent regressed or equivocated within one generation".to_owned(),
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
                "sealed checkpoint or publish intent changed during reconciliation".to_owned(),
            ));
        }
        Ok(())
    }

    async fn validate_initial_state(&mut self) -> Result<(), GovernanceDagServiceError> {
        self.refresh_durable_state()?;
        let source = load_committed_source_snapshot(&self.config, &self.checkpoint_store)?;
        let checkpoint_source =
            validate_checkpoint_against_source(self.checkpoint.as_ref(), &source)?;
        if let Some(intent) = &self.intent {
            let _ = validate_intent_against_source(intent, self.checkpoint.as_ref(), &source)?;
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

        let require_ready_mirror = match (&self.checkpoint, &self.intent) {
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
                require_public_matches_checkpoint(&public, checkpoint)?;
                false
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
                false
            }
            (Some(checkpoint), None) => {
                require_public_matches_checkpoint(&public, checkpoint)?;
                let checkpoint_source = checkpoint_source.as_ref().ok_or_else(|| {
                    GovernanceDagServiceError::State(
                        "validated checkpoint source prefix disappeared".to_owned(),
                    )
                })?;
                let _ = verify_or_recover_mirror_index_store(
                    &self.config,
                    &self.mirror_store,
                    checkpoint,
                    checkpoint_source,
                )?;
                true
            }
            (None, None) => {
                let (_, mirror) = load_mirror_index_store(&self.config, &self.mirror_store)?;
                if !mirror.is_empty() {
                    return Err(GovernanceDagServiceError::State(
                        "Governance DAG mirror exists without a sealed checkpoint".to_owned(),
                    ));
                }
                false
            }
        };
        self.assert_durable_state_unchanged()?;
        if require_ready_mirror {
            // Startup proves the retained roots and exact derived/checkpoint
            // coherence above, but does not expose readiness until the first
            // full public-head and IPFS audit completes in reconciliation.
            self.mirror_reader.assert_install_ready()?;
        }
        Ok(())
    }

    async fn fetch_public_head(&self) -> Result<PublicHead, GovernanceDagServiceError> {
        fetch_signed_http_head(&self.head, self.config.max_response_bytes).await
    }

    async fn install_public_head(
        &self,
        bytes: &[u8],
        current: &PublicHead,
    ) -> Result<PublicHead, GovernanceDagServiceError> {
        put_signed_http_head(
            &self.head,
            bytes,
            current,
            self.config.allow_head_bootstrap,
            self.config.max_response_bytes,
        )
        .await
    }

    async fn reconcile_once(&mut self) -> Result<(), GovernanceDagServiceError> {
        let result = self.reconcile_once_inner().await;
        if result.is_err() {
            self.withdraw_readiness_after_reconciliation_failure().await;
        }
        result
    }

    async fn withdraw_readiness_after_reconciliation_failure(&self) {
        let mut state = self.api.0.write().await;
        state.ready = false;
        // Keep this latched until `publish_api_snapshot` has completed a
        // checkpoint-coherent reconciliation. A transient failure must remain
        // visible to the alerting surface even after the failing call returns.
        state.metrics.mirror_drift = 1;
        self.mirror_reader.mark_unready();
    }

    async fn withdraw_readiness(&self) {
        let mut state = self.api.0.write().await;
        state.ready = false;
        self.mirror_reader.mark_unready();
    }

    async fn reconcile_once_inner(&mut self) -> Result<(), GovernanceDagServiceError> {
        self.state_lock.verify().map_err(|error| {
            GovernanceDagServiceError::Filesystem(format!(
                "service lock binding changed before reconciliation: {error}"
            ))
        })?;
        self.config.revalidate_state_root()?;
        self.refresh_durable_state()?;
        let source = load_committed_source_snapshot(&self.config, &self.checkpoint_store)?;
        self.config.revalidate_state_root()?;
        let _ = validate_checkpoint_against_source(self.checkpoint.as_ref(), &source)?;
        if let Some(intent) = &self.intent {
            let _ = validate_intent_against_source(intent, self.checkpoint.as_ref(), &source)?;
        }

        if let Some(checkpoint) = self.checkpoint.clone()
            && checkpoint.head_block_cid == source.head.head_block_cid
            && self.intent.is_none()
        {
            if self.steady_audit_generation == Some(checkpoint.generation) {
                self.verify_rotating_steady_state(&source, &checkpoint)
                    .await?;
            } else {
                self.verify_steady_state(&source, &checkpoint).await?;
                self.steady_audit_generation = Some(checkpoint.generation);
                self.steady_audit_cursor = 0;
            }
            self.publish_api_snapshot(&source, &checkpoint, false)
                .await?;
            return Ok(());
        }

        self.withdraw_readiness().await;

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
                target_source_chain_blake3: source.chain_blake3,
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
        let target_source =
            validate_intent_against_source(&intent, self.checkpoint.as_ref(), &source)?;
        if let Some(checkpoint) = self.checkpoint.clone()
            && checkpoint.generation == intent.generation
            && checkpoint.head_block_cid == intent.target_head_block_cid
        {
            self.verify_steady_state(&source, &checkpoint).await?;
            self.steady_audit_generation = Some(checkpoint.generation);
            self.steady_audit_cursor = 0;
            delete_publish_intent(&self.checkpoint_store, self.intent_revision)?;
            self.intent_revision = None;
            self.intent = None;
            self.publish_api_snapshot(&source, &checkpoint, false)
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
            let source_block = target_source.blocks.get(sequence).ok_or_else(|| {
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

        let known_sequences = self
            .checkpoint
            .iter()
            .flat_map(|checkpoint| checkpoint.mirror_blocks.iter())
            .map(|block| block.sequence)
            .chain(intent.blocks.iter().map(|block| block.sequence))
            .collect::<BTreeSet<_>>();
        let mut backfilled = Vec::new();
        for source_block in retained_source_suffix(&target_source)? {
            if known_sequences.contains(&source_block.block.sequence) {
                continue;
            }
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
            published_bytes = published_bytes.saturating_add(source_block.bytes.len() as u64);
            pin_lag = pin_lag
                .max(current_unix_timestamp_seconds().saturating_sub(source_block.block.timestamp));
            backfilled.push(published_block_from_source(source_block, cid)?);
        }
        let published_blocks = merge_published_blocks(
            self.checkpoint.as_ref(),
            &intent,
            &backfilled,
            &target_source,
        )?;
        // Seal the publication time in the intent so retries and competing
        // instances derive byte-identical mirror payloads.
        let published_at = intent.created_at_unix;
        let mirror = mirror_index_value(
            &target_source,
            &published_blocks,
            intent.generation,
            &head_ipfs_cid,
            published_at,
        )?;
        let mirror_bytes = json::to_json_pretty(&mirror)
            .map_err(|err| {
                GovernanceDagServiceError::State(format!("mirror JSON encode failed: {err}"))
            })?
            .into_bytes();
        let mirror_blake3 = blake3_array(&mirror_bytes);
        commit_mirror_index_store(
            &self.config,
            &self.mirror_store,
            self.checkpoint.as_ref(),
            &intent,
            mirror_bytes,
        )?;

        // The public-head CAS is the externally visible commit point. Repair
        // and revalidate every object referenced by the final mirror before
        // crossing it, including inherited mappings and crash-resumed intent
        // CIDs that were not uploaded in this process.
        self.ensure_published_objects(
            &target_source,
            &head_ipfs_cid,
            &intent.target_head_bytes,
            &published_blocks,
        )
        .await?;
        self.assert_durable_state_unchanged()?;

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
            self.install_public_head(&intent.target_head_bytes, &current)
                .await?
        };
        match &installed {
            PublicHead::Present { bytes, .. }
                if blake3_array(bytes) == intent.target_head_blake3 => {}
            _ => {
                self.intent = Some(intent);
                return Err(GovernanceDagServiceError::Conflict(
                    "public head installation did not converge".to_owned(),
                ));
            }
        }
        let checkpoint = CheckpointBodyV1 {
            version: CHECKPOINT_VERSION_V1,
            generation: intent.generation,
            head_block_cid: intent.target_head_block_cid.clone(),
            block_count: intent.target_block_count,
            head_bytes: intent.target_head_bytes.clone(),
            head_bytes_blake3: intent.target_head_blake3,
            head_ipfs_cid,
            source_chain_blake3: target_source.chain_blake3,
            mirror_blake3,
            published_at_unix: published_at,
            mirror_blocks: published_blocks,
        };
        self.checkpoint_revision = Some(save_checkpoint(
            &self.checkpoint_store,
            self.checkpoint_revision,
            &checkpoint,
        )?);
        self.checkpoint_generation_floor = checkpoint.generation;
        self.verify_steady_state(&source, &checkpoint).await?;
        self.steady_audit_generation = Some(checkpoint.generation);
        self.steady_audit_cursor = 0;
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
        }
        self.publish_api_snapshot(&source, &checkpoint, true).await
    }

    async fn ensure_published_objects<S: SourceChainView + ?Sized>(
        &self,
        source: &S,
        head_ipfs_cid: &str,
        head_bytes: &[u8],
        published_blocks: &[PublishedBlockV1],
    ) -> Result<(), GovernanceDagServiceError> {
        self.ensure_published_object(
            "governance-dag-head.to",
            "Governance DAG head",
            head_ipfs_cid,
            head_bytes,
        )
        .await?;
        for published in published_blocks {
            let position = usize::try_from(published.sequence).map_err(|_| {
                GovernanceDagServiceError::State(
                    "published mirror sequence exceeds host limits".to_owned(),
                )
            })?;
            let source_block = source.blocks().get(position).ok_or_else(|| {
                GovernanceDagServiceError::State(
                    "published mirror points outside its authenticated source prefix".to_owned(),
                )
            })?;
            self.ensure_published_object(
                &format!(
                    "governance-dag-block-{:020}.to",
                    source_block.block.sequence
                ),
                "Governance DAG block",
                &published.ipfs_cid,
                &source_block.bytes,
            )
            .await?;
        }
        Ok(())
    }

    async fn ensure_published_object(
        &self,
        filename: &str,
        label: &'static str,
        cid: &str,
        expected_bytes: &[u8],
    ) -> Result<(), GovernanceDagServiceError> {
        if self.verify_ipfs_object(cid, expected_bytes).await.is_ok() {
            return Ok(());
        }
        let repaired = ipfs_add_verified(
            &self.ipfs,
            filename,
            expected_bytes,
            self.config.max_request_bytes,
            self.config.max_response_bytes,
        )
        .await?;
        if repaired != cid {
            return Err(GovernanceDagServiceError::State(format!(
                "repaired {label} produced a different content identifier"
            )));
        }
        Ok(())
    }

    async fn verify_ipfs_object(
        &self,
        cid: &str,
        expected_bytes: &[u8],
    ) -> Result<(), GovernanceDagServiceError> {
        ipfs_verify_pin(&self.ipfs, cid, self.config.max_response_bytes).await?;
        let expected_len = u64::try_from(expected_bytes.len()).map_err(|_| {
            GovernanceDagServiceError::State(
                "Governance DAG object length exceeds host limits".to_owned(),
            )
        })?;
        let readback = ipfs_cat(
            &self.ipfs,
            cid,
            expected_len,
            self.config.max_response_bytes,
        )
        .await?;
        if readback != expected_bytes {
            return Err(GovernanceDagServiceError::State(
                "Governance DAG IPFS object readback drifted".to_owned(),
            ));
        }
        Ok(())
    }

    async fn verify_steady_state(
        &self,
        source: &SourceSnapshot,
        checkpoint: &CheckpointBodyV1,
    ) -> Result<(), GovernanceDagServiceError> {
        let checkpoint_source = validate_checkpoint_against_source(Some(checkpoint), source)?
            .ok_or_else(|| {
                GovernanceDagServiceError::State(
                    "validated checkpoint source prefix disappeared".to_owned(),
                )
            })?;
        let public = self.fetch_public_head().await?;
        require_public_matches_checkpoint(&public, checkpoint)?;
        if let PublicHead::Present { bytes, .. } = &public {
            validate_remote_head(bytes, source, &self.config)?;
        }
        if matches!(public, PublicHead::Missing) {
            return Err(GovernanceDagServiceError::Conflict(
                "public head disappeared while verifying the checkpoint".to_owned(),
            ));
        }
        // The checkpoint is the durable authority for these exact bytes. A
        // missing pin or object after the public CAS is recoverable derived
        // state, so deterministically restore it before exposing readiness.
        self.ensure_published_objects(
            &checkpoint_source,
            &checkpoint.head_ipfs_cid,
            &checkpoint.head_bytes,
            &checkpoint.mirror_blocks,
        )
        .await?;
        let _ = verify_or_recover_mirror_index_store(
            &self.config,
            &self.mirror_store,
            checkpoint,
            &checkpoint_source,
        )?;
        self.assert_durable_state_unchanged()?;
        let public_readback = self.fetch_public_head().await?;
        require_public_matches_checkpoint(&public_readback, checkpoint)?;
        Ok(())
    }

    async fn verify_rotating_steady_state(
        &mut self,
        source: &SourceSnapshot,
        checkpoint: &CheckpointBodyV1,
    ) -> Result<(), GovernanceDagServiceError> {
        let checkpoint_source = validate_checkpoint_against_source(Some(checkpoint), source)?
            .ok_or_else(|| {
                GovernanceDagServiceError::State(
                    "validated checkpoint source prefix disappeared".to_owned(),
                )
            })?;
        let public = self.fetch_public_head().await?;
        require_public_matches_checkpoint(&public, checkpoint)?;
        if let PublicHead::Present { bytes, .. } = &public {
            validate_remote_head(bytes, source, &self.config)?;
            self.ensure_published_object(
                "governance-dag-head.to",
                "Governance DAG head",
                &checkpoint.head_ipfs_cid,
                &checkpoint.head_bytes,
            )
            .await?;
        } else {
            return Err(GovernanceDagServiceError::Conflict(
                "public head disappeared while auditing the checkpoint".to_owned(),
            ));
        }

        let retained_len = checkpoint.mirror_blocks.len();
        let mut audited_entries = 0_usize;
        let mut audited_bytes = 0_u64;
        let mut cursor = self.steady_audit_cursor % retained_len;
        while audited_entries < retained_len
            && audited_entries < STEADY_IPFS_AUDIT_MAX_ENTRIES_PER_POLL
        {
            let published = &checkpoint.mirror_blocks[cursor];
            let next_bytes = audited_bytes
                .checked_add(published.encoded_len)
                .ok_or_else(|| {
                    GovernanceDagServiceError::State(
                        "steady Governance DAG audit byte count overflowed".to_owned(),
                    )
                })?;
            if audited_entries > 0 && next_bytes > STEADY_IPFS_AUDIT_MAX_BYTES_PER_POLL {
                break;
            }
            let position = usize::try_from(published.sequence).map_err(|_| {
                GovernanceDagServiceError::State(
                    "checkpoint mirror sequence exceeds host limits".to_owned(),
                )
            })?;
            let source_block = checkpoint_source.blocks.get(position).ok_or_else(|| {
                GovernanceDagServiceError::Conflict(
                    "checkpoint mirror points outside the authenticated source".to_owned(),
                )
            })?;
            self.ensure_published_object(
                &format!(
                    "governance-dag-block-{:020}.to",
                    source_block.block.sequence
                ),
                "Governance DAG block",
                &published.ipfs_cid,
                &source_block.bytes,
            )
            .await?;
            audited_entries += 1;
            audited_bytes = next_bytes;
            cursor = (cursor + 1) % retained_len;
        }
        self.steady_audit_cursor = cursor;
        let _ = verify_or_recover_mirror_index_store(
            &self.config,
            &self.mirror_store,
            checkpoint,
            &checkpoint_source,
        )?;
        self.assert_durable_state_unchanged()?;
        let public_readback = self.fetch_public_head().await?;
        require_public_matches_checkpoint(&public_readback, checkpoint)?;
        Ok(())
    }

    async fn publish_api_snapshot(
        &self,
        source: &SourceSnapshot,
        checkpoint: &CheckpointBodyV1,
        just_published: bool,
    ) -> Result<(), GovernanceDagServiceError> {
        let mirror = verify_mirror_index_store(&self.config, &self.mirror_store, checkpoint)?;
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
        self.mirror_reader.mark_ready();
        drop(state);
        Ok(())
    }
}

fn authenticated_source_prefix<'a>(
    source: &'a SourceSnapshot,
    head_bytes: &[u8],
    expected_block_count: u64,
    expected_head_block_cid: &[u8],
    expected_head_blake3: [u8; 32],
    expected_chain_blake3: [u8; 32],
    label: &'static str,
) -> Result<SourcePrefix<'a>, GovernanceDagServiceError> {
    if blake3_array(head_bytes) != expected_head_blake3 {
        return Err(GovernanceDagServiceError::Conflict(format!(
            "{label} head bytes do not match their authenticated digest"
        )));
    }
    let head: GovernanceDagHeadV1 = decode_canonical(head_bytes, label)?;
    head.validate().map_err(|error| {
        GovernanceDagServiceError::Conflict(format!("{label} head is invalid: {error}"))
    })?;
    let block_count = usize::try_from(expected_block_count).map_err(|_| {
        GovernanceDagServiceError::State(format!("{label} block count exceeds host limits"))
    })?;
    if expected_block_count == 0
        || block_count > source.blocks.len()
        || head.block_count != expected_block_count
        || head.head_block_cid != expected_head_block_cid
    {
        return Err(GovernanceDagServiceError::Conflict(format!(
            "{label} head metadata is not an authenticated source prefix"
        )));
    }
    let blocks = &source.blocks[..block_count];
    let chain = blocks
        .iter()
        .map(|block| block.block.clone())
        .collect::<Vec<_>>();
    validate_source_head_chain(&head, &chain).map_err(|error| {
        GovernanceDagServiceError::Conflict(format!(
            "{label} is not an authenticated source prefix: {error}"
        ))
    })?;
    let chain_blake3 = source_chain_blake3_v1(head_bytes, blocks);
    if chain_blake3 != expected_chain_blake3 {
        return Err(GovernanceDagServiceError::Conflict(format!(
            "{label} source-chain digest does not match its exact prefix bytes"
        )));
    }
    Ok(SourcePrefix {
        head,
        head_bytes: head_bytes.to_vec(),
        blocks,
        chain_blake3,
    })
}

fn validate_checkpoint_against_source<'a>(
    checkpoint: Option<&CheckpointBodyV1>,
    source: &'a SourceSnapshot,
) -> Result<Option<SourcePrefix<'a>>, GovernanceDagServiceError> {
    let Some(checkpoint) = checkpoint else {
        return Ok(None);
    };
    let source_block_count = u64::try_from(source.blocks.len()).map_err(|_| {
        GovernanceDagServiceError::State("source block count exceeds u64".to_owned())
    })?;
    if checkpoint.block_count > source_block_count {
        return Err(GovernanceDagServiceError::Conflict(
            "source chain rolled back behind the authenticated checkpoint".to_owned(),
        ));
    }
    if canonical_ipfs_file_cid(&checkpoint.head_bytes).as_deref()
        != Some(checkpoint.head_ipfs_cid.as_str())
    {
        return Err(GovernanceDagServiceError::Conflict(
            "authenticated checkpoint IPFS CID does not match its exact head bytes".to_owned(),
        ));
    }
    let prefix = authenticated_source_prefix(
        source,
        &checkpoint.head_bytes,
        checkpoint.block_count,
        &checkpoint.head_block_cid,
        checkpoint.head_bytes_blake3,
        checkpoint.source_chain_blake3,
        "authenticated checkpoint",
    )?;
    let retained_source = retained_source_suffix(&prefix)?;
    if checkpoint.mirror_blocks.len() != retained_source.len() {
        return Err(GovernanceDagServiceError::Conflict(
            "checkpoint mirror is not the exact version-1 retained source suffix".to_owned(),
        ));
    }
    for (published, source_block) in checkpoint.mirror_blocks.iter().zip(retained_source) {
        let source_encoded_len = u64::try_from(source_block.bytes.len()).map_err(|_| {
            GovernanceDagServiceError::State("source block length exceeds u64".to_owned())
        })?;
        if source_block.block.block_cid != published.governance_block_cid
            || source_block.block.node.node_cid != published.governance_node_cid
            || source_block.payload_kind != published.payload_kind
            || source_block.block.timestamp != published.timestamp
            || source_block.encoded_blake3 != published.encoded_blake3
            || source_encoded_len != published.encoded_len
            || canonical_ipfs_file_cid(&source_block.bytes).as_deref()
                != Some(published.ipfs_cid.as_str())
        {
            return Err(GovernanceDagServiceError::Conflict(
                "checkpoint mirror no longer matches the verified source chain".to_owned(),
            ));
        }
    }
    Ok(Some(prefix))
}

fn validate_intent_against_source<'a>(
    intent: &PublishIntentBodyV1,
    checkpoint: Option<&CheckpointBodyV1>,
    source: &'a SourceSnapshot,
) -> Result<SourcePrefix<'a>, GovernanceDagServiceError> {
    validate_publish_intent(intent)?;
    let source_block_count = u64::try_from(source.blocks.len()).map_err(|_| {
        GovernanceDagServiceError::State("source block count exceeds u64".to_owned())
    })?;
    if intent.target_block_count > source_block_count {
        return Err(GovernanceDagServiceError::Conflict(
            "source rolled back behind the durable publish intent".to_owned(),
        ));
    }
    if intent.head_ipfs_cid.as_deref().is_some_and(|cid| {
        canonical_ipfs_file_cid(&intent.target_head_bytes).as_deref() != Some(cid)
    }) {
        return Err(GovernanceDagServiceError::Conflict(
            "durable publish-intent IPFS CID does not match its exact head bytes".to_owned(),
        ));
    }
    let prefix = authenticated_source_prefix(
        source,
        &intent.target_head_bytes,
        intent.target_block_count,
        &intent.target_head_block_cid,
        intent.target_head_blake3,
        intent.target_source_chain_blake3,
        "durable publish intent",
    )?;
    let completed_same_generation = checkpoint.is_some_and(|checkpoint| {
        checkpoint.generation == intent.generation
            && checkpoint.block_count == intent.target_block_count
            && checkpoint.head_block_cid == intent.target_head_block_cid
    });
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
    if completed_same_generation {
        let checkpoint = checkpoint.ok_or_else(|| {
            GovernanceDagServiceError::State(
                "completed publish intent lost its committed checkpoint".to_owned(),
            )
        })?;
        if checkpoint.head_bytes_blake3 != intent.target_head_blake3
            || intent.head_ipfs_cid.as_deref() != Some(checkpoint.head_ipfs_cid.as_str())
            || checkpoint.source_chain_blake3 != intent.target_source_chain_blake3
            || intent.blocks.iter().any(|block| block.ipfs_cid.is_none())
            || intent
                .blocks
                .last()
                .and_then(|block| block.sequence.checked_add(1))
                != Some(intent.target_block_count)
        {
            return Err(GovernanceDagServiceError::State(
                "completed publish intent does not match its committed checkpoint".to_owned(),
            ));
        }
    } else {
        let expected_start = checkpoint.map_or(0, |checkpoint| checkpoint.block_count);
        let expected_count = intent
            .target_block_count
            .checked_sub(expected_start)
            .filter(|count| *count > 0)
            .ok_or_else(|| {
                GovernanceDagServiceError::State(
                    "active publish intent does not advance its predecessor checkpoint".to_owned(),
                )
            })?;
        let expected_count = usize::try_from(expected_count).map_err(|_| {
            GovernanceDagServiceError::State(
                "active publish-intent suffix length exceeds host limits".to_owned(),
            )
        })?;
        if intent.blocks.len() != expected_count
            || intent.blocks.first().map(|block| block.sequence) != Some(expected_start)
            || intent
                .blocks
                .last()
                .and_then(|block| block.sequence.checked_add(1))
                != Some(intent.target_block_count)
        {
            return Err(GovernanceDagServiceError::State(
                "active publish intent is not the complete unpublished source suffix".to_owned(),
            ));
        }
        if let Some(checkpoint) = checkpoint
            && intent.previous_public_head_blake3 != Some(checkpoint.head_bytes_blake3)
        {
            return Err(GovernanceDagServiceError::State(
                "active publish intent does not bind its predecessor checkpoint".to_owned(),
            ));
        }
    }
    for block in &intent.blocks {
        let position = usize::try_from(block.sequence).map_err(|_| {
            GovernanceDagServiceError::State("intent sequence exceeds host limits".to_owned())
        })?;
        let source_block = prefix.blocks.get(position).ok_or_else(|| {
            GovernanceDagServiceError::Conflict("intent block is absent from the source".to_owned())
        })?;
        let source_encoded_len = u64::try_from(source_block.bytes.len()).map_err(|_| {
            GovernanceDagServiceError::State("source block length exceeds u64".to_owned())
        })?;
        if source_block.block.block_cid != block.governance_block_cid
            || source_block.block.node.node_cid != block.governance_node_cid
            || source_block.payload_kind != block.payload_kind
            || source_block.block.timestamp != block.timestamp
            || source_block.encoded_blake3 != block.encoded_blake3
            || source_encoded_len != block.encoded_len
            || block.ipfs_cid.as_deref().is_some_and(|cid| {
                canonical_ipfs_file_cid(&source_block.bytes).as_deref() != Some(cid)
            })
        {
            return Err(GovernanceDagServiceError::Conflict(
                "durable intent block no longer matches source bytes".to_owned(),
            ));
        }
    }
    Ok(prefix)
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

fn retained_source_suffix<'a, S: SourceChainView + ?Sized>(
    source: &'a S,
) -> Result<Vec<&'a SourceBlock>, GovernanceDagServiceError> {
    retained_source_suffix_with_limits(
        source,
        GOVERNANCE_DAG_MIRROR_MAX_ENTRIES_V1,
        GOVERNANCE_DAG_MIRROR_MAX_BYTES_V1,
    )
}

fn retained_source_suffix_with_limits<'a, S: SourceChainView + ?Sized>(
    source: &'a S,
    max_entries: usize,
    max_bytes: u64,
) -> Result<Vec<&'a SourceBlock>, GovernanceDagServiceError> {
    if max_entries == 0 || max_bytes == 0 {
        return Err(GovernanceDagServiceError::State(
            "mirror retention bounds must be non-zero".to_owned(),
        ));
    }
    let mut retained = Vec::new();
    let mut retained_bytes = 0_u64;
    for source_block in source.blocks().iter().rev() {
        if retained.len() == max_entries {
            break;
        }
        let encoded_len = u64::try_from(source_block.bytes.len()).map_err(|_| {
            GovernanceDagServiceError::State("source block length exceeds u64".to_owned())
        })?;
        let next = retained_bytes.checked_add(encoded_len).ok_or_else(|| {
            GovernanceDagServiceError::State("mirror byte count overflow".to_owned())
        })?;
        if next > max_bytes {
            if retained.is_empty() {
                return Err(GovernanceDagServiceError::State(
                    "the newest block alone exceeds the version-1 mirror byte ceiling".to_owned(),
                ));
            }
            break;
        }
        retained.push(source_block);
        retained_bytes = next;
    }
    retained.reverse();
    Ok(retained)
}

fn published_block_from_source(
    source: &SourceBlock,
    ipfs_cid: String,
) -> Result<PublishedBlockV1, GovernanceDagServiceError> {
    Ok(PublishedBlockV1 {
        sequence: source.block.sequence,
        governance_block_cid: source.block.block_cid.clone(),
        governance_node_cid: source.block.node.node_cid.clone(),
        payload_kind: source.payload_kind.clone(),
        timestamp: source.block.timestamp,
        encoded_blake3: source.encoded_blake3,
        encoded_len: u64::try_from(source.bytes.len()).map_err(|_| {
            GovernanceDagServiceError::State("source block length exceeds u64".to_owned())
        })?,
        ipfs_cid,
    })
}

fn insert_published_block(
    by_sequence: &mut BTreeMap<u64, PublishedBlockV1>,
    block: PublishedBlockV1,
) -> Result<(), GovernanceDagServiceError> {
    if let Some(existing) = by_sequence.get(&block.sequence) {
        if existing != &block {
            return Err(GovernanceDagServiceError::State(
                "conflicting authenticated IPFS mappings exist for one source sequence".to_owned(),
            ));
        }
        return Ok(());
    }
    by_sequence.insert(block.sequence, block);
    Ok(())
}

fn merge_published_blocks<S: SourceChainView + ?Sized>(
    checkpoint: Option<&CheckpointBodyV1>,
    intent: &PublishIntentBodyV1,
    backfilled: &[PublishedBlockV1],
    source: &S,
) -> Result<Vec<PublishedBlockV1>, GovernanceDagServiceError> {
    let mut by_sequence = BTreeMap::<u64, PublishedBlockV1>::new();
    if let Some(checkpoint) = checkpoint {
        for block in &checkpoint.mirror_blocks {
            insert_published_block(&mut by_sequence, block.clone())?;
        }
    }
    for block in &intent.blocks {
        let ipfs_cid = block.ipfs_cid.clone().ok_or_else(|| {
            GovernanceDagServiceError::State(
                "intent block was not pinned before checkpointing".to_owned(),
            )
        })?;
        insert_published_block(
            &mut by_sequence,
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
        )?;
    }
    for block in backfilled {
        insert_published_block(&mut by_sequence, block.clone())?;
    }
    retained_source_suffix(source)?
        .into_iter()
        .map(|source_block| {
            by_sequence
                .get(&source_block.block.sequence)
                .cloned()
                .ok_or_else(|| {
                    GovernanceDagServiceError::State(
                        "retained source suffix has no authenticated IPFS mapping".to_owned(),
                    )
                })
        })
        .collect()
}

fn mirror_index_value<S: SourceChainView + ?Sized>(
    source: &S,
    blocks: &[PublishedBlockV1],
    generation: u64,
    head_ipfs_cid: &str,
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
    let source_by_sequence = source
        .blocks()
        .iter()
        .map(|source_block| (source_block.block.sequence, source_block))
        .collect::<BTreeMap<_, _>>();
    for (position, block) in blocks.iter().enumerate() {
        let source_block = source_by_sequence.get(&block.sequence).ok_or_else(|| {
            GovernanceDagServiceError::State(
                "published mirror block has no signed source block".to_owned(),
            )
        })?;
        if source_block.block.block_cid != block.governance_block_cid
            || source_block.block.node.node_cid != block.governance_node_cid
            || source_block.encoded_blake3 != block.encoded_blake3
            || source_block.payload_kind != block.payload_kind
        {
            return Err(GovernanceDagServiceError::State(
                "published mirror block does not match its signed source block".to_owned(),
            ));
        }
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
        value.insert(
            "submission_publisher_account_digest_hex".into(),
            source_block
                .block
                .node
                .submission_provenance
                .as_ref()
                .map(|provenance| JsonValue::from(hex::encode(provenance.publisher_account_digest)))
                .unwrap_or(JsonValue::Null),
        );
        value.insert(
            "submission_origin".into(),
            source_block
                .block
                .node
                .submission_provenance
                .as_ref()
                .map(|provenance| JsonValue::from(provenance.origin.label()))
                .unwrap_or(JsonValue::Null),
        );
        block_values.push(JsonValue::Object(value));
    }
    let by_kind = by_kind_positions
        .into_iter()
        .map(|(kind, positions)| (kind, JsonValue::Array(positions)))
        .collect::<JsonMap>();
    let mut head = JsonMap::new();
    head.insert(
        "head_block_cid_hex".into(),
        JsonValue::from(hex::encode(&source.head().head_block_cid)),
    );
    head.insert(
        "block_count".into(),
        JsonValue::from(source.head().block_count),
    );
    head.insert(
        "generated_at".into(),
        JsonValue::from(source.head().generated_at),
    );
    head.insert("ipfs_cid".into(), JsonValue::from(head_ipfs_cid));
    head.insert(
        "blake3".into(),
        JsonValue::from(hex::encode(blake3_array(source.head_bytes()))),
    );
    let mut root = JsonMap::new();
    root.insert("schema".into(), JsonValue::from(MIRROR_INDEX_SCHEMA));
    root.insert("generation".into(), JsonValue::from(generation));
    root.insert("generated_at".into(), JsonValue::from(published_at));
    root.insert("head".into(), JsonValue::Object(head));
    root.insert(
        "block_count".into(),
        JsonValue::from(source.head().block_count),
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

fn verify_mirror_index_store(
    config: &RuntimeConfig,
    store: &TwoSlotStoreV1,
    checkpoint: &CheckpointBodyV1,
) -> Result<JsonValue, GovernanceDagServiceError> {
    let (_, payload) = load_mirror_index_store(config, store)?;
    verify_mirror_payload_against_checkpoint(&payload, checkpoint)
}

fn compare_and_swap_mirror_index_store(
    config: &RuntimeConfig,
    store: &TwoSlotStoreV1,
    expected: &TwoSlotSnapshotV1,
    desired: &MirrorIndexStorePayloadV1,
) -> Result<MirrorIndexStorePayloadV1, GovernanceDagServiceError> {
    let encoded = encode_mirror_index_store_payload(desired)?;
    config.revalidate_state_root()?;
    let committed = store
        .compare_and_swap(expected, &encoded)
        .map_err(|error| {
            GovernanceDagServiceError::State(format!(
                "mirror two-slot compare-and-swap failed: {error}"
            ))
        })?;
    let readback = decode_mirror_index_store_payload(committed.payload())?;
    config.revalidate_state_root()?;
    if readback != *desired {
        return Err(GovernanceDagServiceError::State(
            "mirror two-slot compare-and-swap readback diverged".to_owned(),
        ));
    }
    Ok(readback)
}

fn commit_mirror_index_store(
    config: &RuntimeConfig,
    store: &TwoSlotStoreV1,
    checkpoint: Option<&CheckpointBodyV1>,
    intent: &PublishIntentBodyV1,
    canonical_json: Vec<u8>,
) -> Result<(), GovernanceDagServiceError> {
    let expected_generation = match checkpoint {
        Some(checkpoint) => checkpoint.generation.checked_add(1).ok_or_else(|| {
            GovernanceDagServiceError::State("checkpoint generation exhausted".to_owned())
        })?,
        None => 1,
    };
    if intent.generation != expected_generation {
        return Err(GovernanceDagServiceError::State(
            "mirror update is not the direct successor of the authenticated checkpoint".to_owned(),
        ));
    }
    let intent_bytes = norito::to_bytes(intent).map_err(|error| {
        GovernanceDagServiceError::State(format!(
            "publish intent encode failed while binding mirror candidate: {error}"
        ))
    })?;
    let desired = MirrorIndexStorePayloadV1::committed(
        intent.generation,
        blake3_array(&intent_bytes),
        canonical_json,
    )?;
    verify_mirror_payload_against_intent(&desired, intent)?;
    let (snapshot, current) = load_mirror_index_store(config, store)?;
    if current == desired {
        return Ok(());
    }

    if current.is_empty() {
        // A hard-cut deployment deliberately starts empty even when its sealed
        // checkpoint predates this local representation. The checkpoint and
        // active intent together authorize installing the direct successor.
    } else if current.checkpoint_generation == intent.generation {
        if checkpoint.is_some_and(|checkpoint| checkpoint.generation == intent.generation) {
            return Err(GovernanceDagServiceError::State(
                "authenticated mirror generation cannot be rewritten".to_owned(),
            ));
        }
        // A crash may leave a candidate for this exact sealed intent. Only the
        // same intent generation may replace it before checkpoint commit.
        verify_mirror_payload_against_intent(&current, intent)?;
    } else if let Some(checkpoint) = checkpoint
        && current.checkpoint_generation == checkpoint.generation
    {
        let _ = verify_mirror_payload_against_checkpoint(&current, checkpoint)?;
    } else {
        return Err(GovernanceDagServiceError::State(
            "mirror two-slot predecessor is neither empty, checkpointed, nor owned by the active intent"
                .to_owned(),
        ));
    }

    let committed = compare_and_swap_mirror_index_store(config, store, &snapshot, &desired)?;
    verify_mirror_payload_against_intent(&committed, intent)
}

fn verify_or_recover_mirror_index_store<S: SourceChainView + ?Sized>(
    config: &RuntimeConfig,
    store: &TwoSlotStoreV1,
    checkpoint: &CheckpointBodyV1,
    source: &S,
) -> Result<JsonValue, GovernanceDagServiceError> {
    let (snapshot, current) = load_mirror_index_store(config, store)?;
    if let Ok(value) = verify_mirror_payload_against_checkpoint(&current, checkpoint) {
        return Ok(value);
    }
    if !current.is_empty() && current.checkpoint_generation > checkpoint.generation {
        return Err(GovernanceDagServiceError::State(
            "local mirror generation is ahead of the authenticated checkpoint".to_owned(),
        ));
    }
    if source.head().head_block_cid != checkpoint.head_block_cid
        || source.head().block_count != checkpoint.block_count
        || blake3_array(source.head_bytes()) != checkpoint.head_bytes_blake3
    {
        return Err(GovernanceDagServiceError::State(
            "derived mirror cannot be rebuilt from a source at a different head".to_owned(),
        ));
    }
    let mirror = mirror_index_value(
        source,
        &checkpoint.mirror_blocks,
        checkpoint.generation,
        &checkpoint.head_ipfs_cid,
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
    let desired = MirrorIndexStorePayloadV1::committed(checkpoint.generation, [0; 32], bytes)?;
    let committed = compare_and_swap_mirror_index_store(config, store, &snapshot, &desired)?;
    verify_mirror_payload_against_checkpoint(&committed, checkpoint)
}

fn service_router(state: ServiceApiState) -> Router {
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

async fn health_handler(State(state): State<ServiceApiState>) -> Response {
    let snapshot = state.telemetry.0.read().await;
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

async fn readiness_handler(State(state): State<ServiceApiState>) -> Response {
    let ready = authenticated_mirror_snapshot(&state).await.is_ok();
    let snapshot = state.telemetry.0.read().await;
    let mut value = JsonMap::new();
    value.insert(
        "schema".into(),
        JsonValue::from("sorafs.governance_dag.readiness.v1"),
    );
    value.insert("ready".into(), JsonValue::from(ready));
    value.insert(
        "error".into(),
        snapshot
            .last_error
            .as_ref()
            .map_or(JsonValue::Null, |error| JsonValue::from(error.clone())),
    );
    json_response(
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        JsonValue::Object(value),
        &HeaderMap::new(),
    )
}

async fn metrics_handler(State(state): State<ServiceApiState>) -> Response {
    metrics_response(&state.telemetry).await
}

async fn metrics_response(state: &ApiState) -> Response {
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

async fn authenticated_mirror_snapshot(
    state: &ServiceApiState,
) -> Result<GovernanceDagMirrorSnapshotV1, Response> {
    let reader = state.mirror_reader.clone();
    match tokio::task::spawn_blocking(move || reader.read()).await {
        Ok(Ok(Some(snapshot))) => Ok(snapshot),
        Ok(Ok(None)) | Ok(Err(_)) => Err(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "authenticated Governance DAG mirror is not ready",
        )),
        Err(_) => Err(json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "authenticated Governance DAG mirror read failed",
        )),
    }
}

async fn dashboard_handler(State(state): State<ServiceApiState>, headers: HeaderMap) -> Response {
    let snapshot = match authenticated_mirror_snapshot(&state).await {
        Ok(snapshot) => snapshot,
        Err(response) => return response,
    };
    let mirror = snapshot.mirror();
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

async fn head_handler(State(state): State<ServiceApiState>, headers: HeaderMap) -> Response {
    let snapshot = match authenticated_mirror_snapshot(&state).await {
        Ok(snapshot) => snapshot,
        Err(response) => return response,
    };
    let Some(head) = snapshot.mirror().get("head").cloned() else {
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
    State(state): State<ServiceApiState>,
    headers: HeaderMap,
    AxumPath(cid): AxumPath<String>,
) -> Response {
    lookup_handler(state, headers, cid, "block_cid_hex", "block").await
}

async fn node_handler(
    State(state): State<ServiceApiState>,
    headers: HeaderMap,
    AxumPath(cid): AxumPath<String>,
) -> Response {
    lookup_handler(state, headers, cid, "node_cid_hex", "node").await
}

async fn lookup_handler(
    state: ServiceApiState,
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
    let snapshot = match authenticated_mirror_snapshot(&state).await {
        Ok(snapshot) => snapshot,
        Err(response) => return response,
    };
    let block = snapshot
        .mirror()
        .get("blocks")
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
    State(state): State<ServiceApiState>,
    headers: HeaderMap,
    AxumPath(digest): AxumPath<String>,
) -> Response {
    if !is_canonical_digest_hex(&digest) {
        return json_error(
            StatusCode::BAD_REQUEST,
            "encoded digest must be lowercase 32-byte hex",
        );
    }
    let snapshot = match authenticated_mirror_snapshot(&state).await {
        Ok(snapshot) => snapshot,
        Err(response) => return response,
    };
    let blocks = snapshot
        .mirror()
        .get("blocks")
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

async fn checkpoint_handler(State(state): State<ServiceApiState>, headers: HeaderMap) -> Response {
    let snapshot = match authenticated_mirror_snapshot(&state).await {
        Ok(snapshot) => snapshot,
        Err(response) => return response,
    };
    let checkpoint = snapshot.checkpoint();
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
    use super::*;
    use crate::{
        FilesystemGovernancePublisher, GovernanceDagCanonicalRequestHeaderV1,
        GovernanceDagRuntimeSigner, GovernancePublisher, NodeHandle, NodeRuntimeDeps,
        config::StorageConfig,
        governance::{
            qualify_governance_dag_runtime_checkpoint_store,
            qualify_governance_dag_runtime_signer_provider,
            write_runtime_dag_committed_snapshot_fixture_v1,
        },
    };
    use axum::{
        body::Bytes,
        extract::State,
        http::{self, HeaderName, Request},
        response::Redirect,
        routing::{any, post},
    };
    use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature as IrohaSignature};
    use norito::codec::Encode as _;
    use sorafs_manifest::{
        GOVERNANCE_DAG_BLOCK_VERSION_V1, GOVERNANCE_DAG_HEAD_VERSION_V1, GOVERNANCE_LOG_VERSION_V1,
        GovernanceDagSubmissionOriginV1, GovernanceDagSubmissionProvenanceV1, GovernanceLogNodeV1,
        GovernanceLogSignatureV1, SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
        SoraFsAppealFinanceAccountFlowV1, SoraFsAppealFinanceJurorPayoutV1,
        SoraFsAppealFinanceOutcomeV1, SoraFsAppealFinanceReportV1,
        deal::{
            DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
            DealSettlementStatusV1, DealSettlementV1, XorQuantity,
        },
        governance_dag_block_cid_v1, governance_dag_submission_account_digest_v1,
    };
    use std::{
        fmt,
        process::{Child, Command, Stdio},
        sync::{
            Arc, Mutex as StdMutex,
            atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
        },
    };
    use tempfile::TempDir;
    use tokio::{sync::Mutex, task::JoinHandle};
    use tower::ServiceExt as _;
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
    const KUBO_CONFORMANCE_VERSION_V1: &str = "0.42.0";
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
    const TEST_RECEIVER_POLICY_DIGEST: [u8; 32] = [0x91; 32];
    const TEST_REPLAY_NAMESPACE_DIGEST: [u8; 32] = [0x92; 32];
    const TEST_INGRESS_REPLICA_SET_DIGEST: [u8; 32] = [0x93; 32];

    struct TestAuthenticator {
        handle: String,
        private_key: PrivateKey,
        public_key: [u8; 32],
        ingress_binding: StdMutex<GovernanceDagRequestIngressBindingV1>,
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
                ingress_binding: StdMutex::new(default_test_request_ingress_binding(handle)),
                provider_secret: StdMutex::new(provider_secret.to_owned()),
                nonce_counter: AtomicU64::new(1),
                qualification_revision: AtomicU64::new(1),
                qualification_refuse: AtomicBool::new(false),
                drift_during_authentication: AtomicBool::new(false),
                refuse: AtomicBool::new(false),
            }
        }

        fn with_ingress_binding(
            mut self,
            ingress_binding: GovernanceDagRequestIngressBindingV1,
        ) -> Self {
            *self
                .ingress_binding
                .get_mut()
                .expect("access test ingress binding") = ingress_binding;
            self
        }

        fn ingress_binding(&self) -> GovernanceDagRequestIngressBindingV1 {
            *self
                .ingress_binding
                .lock()
                .expect("lock test ingress binding")
        }

        fn rebind_ingress(&self, ingress_binding: GovernanceDagRequestIngressBindingV1) {
            *self
                .ingress_binding
                .lock()
                .expect("lock test ingress binding") = ingress_binding;
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

    fn default_test_request_ingress_binding(handle: &str) -> GovernanceDagRequestIngressBindingV1 {
        let config = SorafsGovernanceDagService::default();
        let (scope, endpoint, max_body_bytes) = if handle == TEST_HEAD_AUTH_HANDLE {
            (
                GovernanceDagAuthenticationScope::SignedHead,
                "http://127.0.0.1:9099/head",
                config.max_request_bytes.0,
            )
        } else {
            (
                GovernanceDagAuthenticationScope::Ipfs,
                "http://127.0.0.1:5001",
                authenticated_ipfs_wire_body_max_bytes(config.max_request_bytes.0)
                    .expect("derive test IPFS ingress body bound"),
            )
        };
        configured_request_ingress_binding(
            scope,
            endpoint,
            test_request_auth_public_key(handle),
            max_body_bytes,
            config.request_auth_max_envelope_lifetime_secs,
            config.request_auth_max_future_skew_secs,
            "test authenticator",
        )
        .expect("construct test request-ingress binding")
    }

    fn test_ingress_qualification(
        provider: GovernanceDagRuntimeProviderQualificationV1,
        binding: GovernanceDagRequestIngressBindingV1,
    ) -> GovernanceDagRequestIngressQualificationV1 {
        GovernanceDagRequestIngressQualificationV1::try_new(
            provider,
            binding,
            TEST_RECEIVER_POLICY_DIGEST,
            TEST_REPLAY_NAMESPACE_DIGEST,
            TEST_INGRESS_REPLICA_SET_DIGEST,
        )
        .expect("construct live test request-ingress qualification")
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
        replay_cache: &mut dyn crate::GovernanceDagRequestAuthenticationReplayStoreV1,
        backend_calls: &AtomicU64,
    ) -> Result<(), GovernanceDagRequestAuthenticationErrorV1> {
        if request.scope() != expected_scope {
            return Err(GovernanceDagRequestAuthenticationErrorV1::RequestMismatch);
        }
        let request_url = Url::parse(request.canonical_url())
            .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
        let mut endpoint = request_url.clone();
        endpoint.set_query(None);
        if expected_scope == GovernanceDagAuthenticationScope::Ipfs {
            endpoint.set_path("/");
        }
        let endpoint_binding =
            governance_dag_request_ingress_endpoint_binding_v1(expected_scope, endpoint.as_str())
                .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
        let binding = GovernanceDagRequestIngressBindingV1::try_new(
            expected_scope,
            endpoint_binding,
            policy.public_key(),
            1024 * 1024,
            policy.max_envelope_lifetime_secs(),
            policy.max_future_skew_secs(),
        )
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
        let origin = request_url.origin().ascii_serialization();
        let authority = origin
            .split_once("://")
            .map(|(_, authority)| authority)
            .ok_or(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
        let target = match request_url.query() {
            Some(query) => format!("{}?{query}", request_url.path()),
            None => request_url.path().to_owned(),
        };
        let mut http_request = Request::builder()
            .method(request.method())
            .uri(target.as_str())
            .version(http::Version::HTTP_11)
            .header(header::HOST, authority)
            .body(body.to_vec())
            .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
        for selected in request.selected_headers() {
            let name = HeaderName::from_bytes(selected.name().as_bytes())
                .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader)?;
            let value = HeaderValue::from_bytes(selected.value().as_bytes())
                .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader)?;
            http_request.headers_mut().append(name, value);
        }
        for (name, value) in headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader)?;
            let value = HeaderValue::from_bytes(value)
                .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader)?;
            http_request.headers_mut().append(name, value);
        }
        let mut receiver =
            GovernanceDagHttpRequestReceiverV1::try_new(endpoint.as_str(), binding, replay_cache)?;
        let verified = receiver.verify_http_request(http_request, now)?;
        if verified.descriptor() != request
            || !verified.request().headers().contains_key(header::HOST)
            || verified.request().uri().scheme().is_some()
            || verified.request().uri().authority().is_some()
            || verified
                .request()
                .uri()
                .path_and_query()
                .map(http::uri::PathAndQuery::as_str)
                != Some(target.as_str())
            || GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1
                .iter()
                .any(|name| verified.request().headers().contains_key(*name))
        {
            return Err(GovernanceDagRequestAuthenticationErrorV1::RequestMismatch);
        }
        backend_calls.fetch_add(1, AtomicOrdering::SeqCst);
        Ok(())
    }

    #[derive(Debug)]
    struct UnavailableTestReplayStore;

    impl crate::GovernanceDagRequestAuthenticationReplayStoreV1 for UnavailableTestReplayStore {
        fn consume_nonce(
            &mut self,
            _nonce: [u8; 32],
            _expires_at_unix_secs: u64,
            _now_unix_secs: u64,
        ) -> Result<(), GovernanceDagRequestAuthenticationErrorV1> {
            Err(GovernanceDagRequestAuthenticationErrorV1::ReplayStoreUnavailable)
        }
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

        fn ingress_qualification(
            &self,
        ) -> Result<GovernanceDagRequestIngressQualificationV1, String> {
            if self.qualification_refuse.load(AtomicOrdering::SeqCst) {
                return Err("auth_token=must-never-escape".to_owned());
            }
            Ok(test_ingress_qualification(
                GovernanceDagRuntimeProviderQualificationV1::new(
                    self.qualification_revision.load(AtomicOrdering::SeqCst),
                    [0x81; 32],
                ),
                self.ingress_binding(),
            ))
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

    trait TestRebindableRequestAuthenticator: GovernanceDagRequestAuthenticator {
        fn rebind_ingress(&self, ingress_binding: GovernanceDagRequestIngressBindingV1);
    }

    impl TestRebindableRequestAuthenticator for TestAuthenticator {
        fn rebind_ingress(&self, ingress_binding: GovernanceDagRequestIngressBindingV1) {
            TestAuthenticator::rebind_ingress(self, ingress_binding);
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

        fn ingress_qualification(
            &self,
        ) -> Result<GovernanceDagRequestIngressQualificationV1, String> {
            self.signer.ingress_qualification()
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

    impl TestRebindableRequestAuthenticator for FinalRequestAuthenticator {
        fn rebind_ingress(&self, ingress_binding: GovernanceDagRequestIngressBindingV1) {
            self.signer.rebind_ingress(ingress_binding);
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
        checkpoint_load_count: AtomicU64,
        checkpoint_second_load: StdMutex<Option<GovernanceDagSealedStateRecord>>,
        intent_load_count: AtomicU64,
        intent_second_load: StdMutex<Option<GovernanceDagSealedStateRecord>>,
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
                checkpoint_load_count: AtomicU64::new(0),
                checkpoint_second_load: StdMutex::new(None),
                intent_load_count: AtomicU64::new(0),
                intent_second_load: StdMutex::new(None),
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

        fn return_checkpoint_on_second_load(&self, record: GovernanceDagSealedStateRecord) {
            *self
                .checkpoint_second_load
                .lock()
                .expect("lock checkpoint race fixture") = Some(record);
            self.checkpoint_load_count.store(0, AtomicOrdering::SeqCst);
        }

        fn return_intent_on_second_load(&self, record: GovernanceDagSealedStateRecord) {
            *self
                .intent_second_load
                .lock()
                .expect("lock intent race fixture") = Some(record);
            self.intent_load_count.store(0, AtomicOrdering::SeqCst);
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
            let raced = match slot {
                GovernanceDagSealedStateSlot::Checkpoint
                    if self
                        .checkpoint_load_count
                        .fetch_add(1, AtomicOrdering::SeqCst)
                        == 1 =>
                {
                    self.checkpoint_second_load
                        .lock()
                        .map_err(|_| "poisoned".to_owned())?
                        .clone()
                }
                GovernanceDagSealedStateSlot::PublishIntent
                    if self.intent_load_count.fetch_add(1, AtomicOrdering::SeqCst) == 1 =>
                {
                    self.intent_second_load
                        .lock()
                        .map_err(|_| "poisoned".to_owned())?
                        .clone()
                }
                _ => None,
            };
            if raced.is_some() {
                return Ok(raced);
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
            provider.ingress_binding(),
            provider,
            "test authenticator",
        )
        .expect("bind test authenticator")
    }

    fn bind_test_authenticator_to_endpoint<T>(
        provider: Arc<T>,
        scope: GovernanceDagAuthenticationScope,
        endpoint: &str,
        max_body_bytes: u64,
    ) -> OpaqueAuthenticator
    where
        T: TestRebindableRequestAuthenticator + 'static,
    {
        let current = provider
            .ingress_qualification()
            .expect("read test request-ingress qualification");
        let current_binding = current.binding();
        let endpoint_binding = governance_dag_request_ingress_endpoint_binding_v1(scope, endpoint)
            .expect("bind canonical test endpoint");
        let ingress_binding = GovernanceDagRequestIngressBindingV1::try_new(
            scope,
            endpoint_binding,
            current_binding.public_key(),
            max_body_bytes,
            current_binding.max_envelope_lifetime_secs(),
            current_binding.max_future_skew_secs(),
        )
        .expect("bind exact test request ingress");
        provider.rebind_ingress(ingress_binding);
        let handle = provider.handle().to_owned();
        let provider: Arc<dyn GovernanceDagRequestAuthenticator> = provider;
        OpaqueAuthenticator::try_new(
            &handle,
            current.provider(),
            ingress_binding,
            provider,
            "test authenticator",
        )
        .expect("qualify exact test endpoint authenticator")
    }

    fn test_runtime_providers(
        view: &SorafsGovernanceDagServiceView,
        checkpoint_store: Arc<TestSealedStore>,
    ) -> GovernanceDagServiceRuntimeProviders {
        let bindings = runtime_provider_bindings(view).expect("derive test runtime bindings");
        GovernanceDagServiceRuntimeProviders {
            ipfs_authenticator: Some(Arc::new(
                TestAuthenticator::new(TEST_IPFS_AUTH_HANDLE, "test-only-ipfs-bearer")
                    .with_ingress_binding(bindings.ipfs_request_ingress_binding()),
            )),
            head_authenticator: Some(Arc::new(
                TestAuthenticator::new(TEST_HEAD_AUTH_HANDLE, "test-only-head-bearer")
                    .with_ingress_binding(bindings.head_request_ingress_binding()),
            )),
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

            let version_bytes = Self::run_command(&binary, &repo, &["version", "--number"]);
            let version = std::str::from_utf8(&version_bytes)
                .expect("Kubo version must be UTF-8")
                .trim();
            assert_eq!(
                version, KUBO_CONFORMANCE_VERSION_V1,
                "the fixed UnixFS profile must be checked against the release-pinned Kubo version"
            );

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
            let authenticated_wire_body_max_bytes = authenticated_ipfs_wire_body_max_bytes(
                GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1 as u64,
            )
            .expect("derive Kubo authenticated wire-body bound");
            let provider = Arc::new(TestAuthenticator::new(
                TEST_IPFS_AUTH_HANDLE,
                "isolated-kubo-authenticator",
            ));
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
                authenticator: bind_test_authenticator_to_endpoint(
                    provider,
                    GovernanceDagAuthenticationScope::Ipfs,
                    &self.api_url,
                    authenticated_wire_body_max_bytes,
                ),
                authenticated_wire_body_max_bytes,
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
                submission_provenance: None,
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
        let chain_blake3 = source_chain_blake3_v1(&head_bytes, &source_blocks);
        SourceSnapshot {
            index_blake3: [0x44; 32],
            chain_blake3,
            head,
            head_bytes,
            blocks: source_blocks,
        }
    }

    fn appeal_finance_report(timestamp: u64) -> SoraFsAppealFinanceReportV1 {
        SoraFsAppealFinanceReportV1 {
            version: SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
            report_id: [0x42; 16],
            case_id: "case-42".to_owned(),
            round_id: Some("round-1".to_owned()),
            generated_at_unix_ms: timestamp.saturating_mul(1_000),
            appeal_finance_config_version: "baseline-v1".to_owned(),
            evidence_bundle_digest: Some([0xA7; 32]),
            outcome: SoraFsAppealFinanceOutcomeV1::Overturn,
            deposit_xor: xor("420"),
            refund: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "refund-account".to_owned(),
                amount_xor: xor("420"),
            },
            treasury: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "treasury-account".to_owned(),
                amount_xor: xor("50"),
            },
            held: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "escrow-account".to_owned(),
                amount_xor: XorQuantity::zero(),
            },
            panel_size: 3,
            panel_reward_total_xor: xor("85"),
            rewards_paid_total_xor: xor("60"),
            rewards_forfeited_treasury_xor: xor("25"),
            juror_payouts: vec![
                SoraFsAppealFinanceJurorPayoutV1 {
                    juror_id: "juror-a".to_owned(),
                    stipend_xor: xor("25"),
                    bonus_xor: xor("5"),
                    total_xor: xor("30"),
                },
                SoraFsAppealFinanceJurorPayoutV1 {
                    juror_id: "juror-b".to_owned(),
                    stipend_xor: xor("25"),
                    bonus_xor: xor("5"),
                    total_xor: xor("30"),
                },
            ],
            no_show_juror_ids: vec!["juror-c".to_owned()],
        }
    }

    fn signed_finance_source(seed: u8, timestamp: u64) -> SourceSnapshot {
        let signer = TestSigner::new(seed);
        let account_key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
            .expect("derive canonical submission account");
        let account = iroha_data_model::account::AccountId::new(account_key.public_key().clone());
        let mut source = signed_source(1, seed, timestamp);
        let source_block = source.blocks.first_mut().expect("single source block");
        source_block.block.node.payload =
            GovernanceLogPayloadV1::AppealFinanceReport(appeal_finance_report(timestamp));
        source_block.block.node.submission_provenance = Some(GovernanceDagSubmissionProvenanceV1 {
            publisher_account_digest: governance_dag_submission_account_digest_v1(
                &account.encode(),
            ),
            origin: GovernanceDagSubmissionOriginV1::AppealFinanceReport,
        });
        source_block.block.node.node_cid = source_block
            .block
            .node
            .recompute_node_cid()
            .expect("derive attributed node CID");
        source_block.block.node.publisher_signature = signer.sign(
            &source_block
                .block
                .node
                .signature_payload_bytes()
                .expect("encode attributed node signing payload"),
        );
        source_block.block.block_cid = source_block
            .block
            .recompute_block_cid()
            .expect("derive attributed block CID");
        source_block.block.block_signature = signer.sign(
            &source_block
                .block
                .signature_payload_bytes()
                .expect("encode attributed block signing payload"),
        );
        source_block
            .block
            .validate()
            .expect("attributed source block validates");
        source_block.bytes =
            norito::to_bytes(&source_block.block).expect("encode attributed source block");
        source_block.encoded_blake3 = blake3_array(&source_block.bytes);
        source_block.payload_kind = "appeal_finance_report".to_owned();

        source.head.head_block_cid = source_block.block.block_cid.clone();
        source.head.head_signature = signer.sign(
            &source
                .head
                .signature_payload_bytes()
                .expect("encode attributed head signing payload"),
        );
        validate_governance_dag_head_against_chain_v1(&source.head, &[source_block.block.clone()])
            .expect("attributed source head validates");
        source.head_bytes = norito::to_bytes(&source.head).expect("encode attributed source head");
        source.chain_blake3 = source_chain_blake3_v1(&source.head_bytes, &source.blocks);
        source
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
            max_future_skew_secs: 60,
            allow_head_bootstrap: true,
            expected_producer_signer_handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
            expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
            expected_checkpoint_store_handle: TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
            expected_checkpoint_store_qualification: TEST_STORE_QUALIFICATION,
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
                ipfs_cid: canonical_ipfs_file_cid(&block.bytes)
                    .expect("test block fits fixed UnixFS profile"),
            })
            .collect();
        CheckpointBodyV1 {
            version: CHECKPOINT_VERSION_V1,
            generation: 1,
            head_block_cid: source.head.head_block_cid.clone(),
            block_count: source.head.block_count,
            head_bytes: source.head_bytes.clone(),
            head_bytes_blake3: blake3_array(&source.head_bytes),
            head_ipfs_cid: canonical_ipfs_file_cid(&source.head_bytes)
                .expect("test head fits fixed UnixFS profile"),
            source_chain_blake3: source.chain_blake3,
            mirror_blake3: [0x55; 32],
            published_at_unix: source.head.generated_at,
            mirror_blocks,
        }
    }

    fn checkpoint_with_canonical_mirror(source: &SourceSnapshot) -> CheckpointBodyV1 {
        let mut checkpoint = checkpoint_from_source(source);
        let mirror = mirror_index_value(
            source,
            &checkpoint.mirror_blocks,
            checkpoint.generation,
            &checkpoint.head_ipfs_cid,
            checkpoint.published_at_unix,
        )
        .expect("build canonical checkpoint mirror");
        checkpoint.mirror_blake3 = blake3_array(
            json::to_json_pretty(&mirror)
                .expect("encode canonical checkpoint mirror")
                .as_bytes(),
        );
        checkpoint
    }

    fn intent_from_source(source: &SourceSnapshot) -> PublishIntentBodyV1 {
        PublishIntentBodyV1 {
            version: PUBLISH_INTENT_VERSION_V1,
            generation: 1,
            target_head_block_cid: source.head.head_block_cid.clone(),
            target_block_count: source.head.block_count,
            target_head_bytes: source.head_bytes.clone(),
            target_head_blake3: blake3_array(&source.head_bytes),
            target_source_chain_blake3: source.chain_blake3,
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
                    ipfs_cid: Some(
                        canonical_ipfs_file_cid(&block.bytes)
                            .expect("test block fits fixed UnixFS profile"),
                    ),
                })
                .collect(),
            head_ipfs_cid: Some(
                canonical_ipfs_file_cid(&source.head_bytes)
                    .expect("test head fits fixed UnixFS profile"),
            ),
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
            let source_payload_len = u64::try_from(source_payload_bytes.len())
                .expect("test source payload length fits u64");
            let source_payload_digest_hex = hex::encode(blake3_array(&source_payload_bytes));
            let mut source_json = JsonMap::new();
            source_json.insert(
                "payload_kind".into(),
                JsonValue::from(block.payload_kind.clone()),
            );
            source_json.insert("sequence".into(), JsonValue::from(block.block.sequence));
            source_json.insert(
                "source_payload_blake3".into(),
                JsonValue::from(source_payload_digest_hex.clone()),
            );
            source_json.insert(
                "source_payload_len".into(),
                JsonValue::from(source_payload_len),
            );
            let source_json_bytes = json::to_json_pretty(&JsonValue::Object(source_json))
                .expect("encode test JSON source")
                .into_bytes();
            validate_governance_car_source_lengths(
                source_payload_bytes.len(),
                source_json_bytes.len(),
            )
            .expect("test Governance DAG source pair satisfies size limits");
            let source_json_len =
                u64::try_from(source_json_bytes.len()).expect("test JSON source length fits u64");
            let source_json_digest_hex = hex::encode(blake3_array(&source_json_bytes));
            let (source_payload_path_label, source_json_path_label) =
                governance_source_pair_relative_paths(
                    &block.payload_kind,
                    source_payload_len,
                    &source_payload_digest_hex,
                    source_json_len,
                    &source_json_digest_hex,
                )
                .expect("derive test Governance DAG source-pair paths");
            write_test_sidecar_file(
                &root.join(&source_payload_path_label),
                &source_payload_bytes,
            );
            write_test_sidecar_file(&root.join(&source_json_path_label), &source_json_bytes);

            let digest_hex = hex::encode(block.encoded_blake3);
            let mut entry = JsonMap::new();
            entry.insert("position".into(), JsonValue::from(position as u64));
            entry.insert("sequence".into(), JsonValue::from(block.block.sequence));
            entry.insert("block_path".into(), JsonValue::from(block_path_label));
            entry.insert(
                "encoded_path".into(),
                JsonValue::from(source_payload_path_label),
            );
            entry.insert("json_path".into(), JsonValue::from(source_json_path_label));
            entry.insert(
                "encoded_len".into(),
                JsonValue::from(block.bytes.len() as u64),
            );
            entry.insert(
                "source_payload_len".into(),
                JsonValue::from(source_payload_len),
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
            entry.insert(
                "submission_publisher_account_digest_hex".into(),
                block
                    .block
                    .node
                    .submission_provenance
                    .as_ref()
                    .map(|provenance| {
                        JsonValue::from(hex::encode(provenance.publisher_account_digest))
                    })
                    .unwrap_or(JsonValue::Null),
            );
            entry.insert(
                "submission_origin".into(),
                block
                    .block
                    .node
                    .submission_provenance
                    .as_ref()
                    .map(|provenance| JsonValue::from(provenance.origin.label()))
                    .unwrap_or(JsonValue::Null),
            );
            entry.insert(
                "published_at_unix".into(),
                JsonValue::from(block.block.timestamp),
            );
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
        let mut index = JsonMap::new();
        index.insert("schema".into(), JsonValue::from(RUNTIME_INDEX_SCHEMA));
        index.insert("source".into(), JsonValue::from(RUNTIME_INDEX_SOURCE));
        index.insert("root".into(), JsonValue::from(RUNTIME_INDEX_LOGICAL_ROOT));
        index.insert(
            "generated_at".into(),
            JsonValue::from(source.head.generated_at),
        );
        index.insert(
            "signer_handle".into(),
            JsonValue::from(TEST_PRODUCER_SIGNER_HANDLE),
        );
        index.insert(
            "signer_revision".into(),
            JsonValue::from(TEST_PRODUCER_SIGNER_QUALIFICATION.revision),
        );
        index.insert(
            "signer_policy_digest_hex".into(),
            JsonValue::from(hex::encode(
                TEST_PRODUCER_SIGNER_QUALIFICATION.policy_digest,
            )),
        );
        index.insert(
            "checkpoint_store_handle".into(),
            JsonValue::from(TEST_CHECKPOINT_STORE_HANDLE),
        );
        index.insert(
            "checkpoint_store_revision".into(),
            JsonValue::from(TEST_STORE_QUALIFICATION.revision),
        );
        index.insert(
            "checkpoint_store_policy_digest_hex".into(),
            JsonValue::from(hex::encode(TEST_STORE_QUALIFICATION.policy_digest)),
        );
        index.insert(
            "publisher_public_key_hex".into(),
            JsonValue::from(hex::encode(&source.head.head_signature.public_key)),
        );
        index.insert(
            "publisher_peer_id_hex".into(),
            JsonValue::from(hex::encode(&source.head.publisher_peer_id)),
        );
        index.insert(
            "publisher_peer_id".into(),
            JsonValue::from(
                std::str::from_utf8(&source.head.publisher_peer_id)
                    .expect("test publisher peer id is UTF-8"),
            ),
        );
        index.insert(
            "head_block_cid_hex".into(),
            JsonValue::from(hex::encode(&source.head.head_block_cid)),
        );
        index.insert(
            "head_generated_at".into(),
            JsonValue::from(source.head.generated_at),
        );
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
        source.chain_blake3 = source_chain_blake3_v1(&source.head_bytes, &source.blocks);
        write_runtime_dag_committed_snapshot_fixture_v1(
            root,
            source.head_bytes.clone(),
            index_bytes,
        )
        .expect("commit typed Governance DAG runtime head/index fixture");
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
        signed_head_url: &str,
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
signed_head_url = "{}"
ipfs_authenticator_handle = "{TEST_IPFS_AUTH_HANDLE}"
ipfs_authenticator_revision = 1
ipfs_authenticator_policy_digest_hex = "{}"
ipfs_request_auth_public_key_hex = "{}"
head_authenticator_handle = "{TEST_HEAD_AUTH_HANDLE}"
head_authenticator_revision = 1
head_authenticator_policy_digest_hex = "{}"
head_request_auth_public_key_hex = "{}"
checkpoint_store_handle = "{TEST_CHECKPOINT_STORE_HANDLE}"
checkpoint_store_revision = 1
checkpoint_store_policy_digest_hex = "{}"
publisher_public_key_hex = "{}"
poll_interval_secs = 1
connect_timeout_ms = 5000
request_timeout_ms = 20000
dns_timeout_ms = 5000
max_future_skew_secs = 60
allow_insecure_http = true
allow_private_ipfs_endpoint = true
allow_private_head_endpoint = true
allow_head_bootstrap = true
listen_addr = "127.0.0.1:0"
"#,
            source_dir.display(),
            "83".repeat(32),
            hex::encode(&source.head.head_signature.public_key),
            state_dir.display(),
            api_url,
            signed_head_url,
            "81".repeat(32),
            hex::encode(test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE)),
            "81".repeat(32),
            hex::encode(test_request_auth_public_key(TEST_HEAD_AUTH_HANDLE)),
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

    async fn spawn_router_with_authenticator<T>(
        router: Router,
        path: &str,
        authentication_scope: GovernanceDagAuthenticationScope,
        provider: Arc<T>,
    ) -> (PinnedEndpoint, JoinHandle<()>)
    where
        T: TestRebindableRequestAuthenticator + 'static,
    {
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
        let config = SorafsGovernanceDagService::default();
        let authenticated_wire_body_max_bytes = match authentication_scope {
            GovernanceDagAuthenticationScope::Ipfs => {
                authenticated_ipfs_wire_body_max_bytes(config.max_request_bytes.0)
                    .expect("derive mock authenticated wire-body bound")
            }
            GovernanceDagAuthenticationScope::SignedHead => config.max_request_bytes.0,
        };
        let authenticator = bind_test_authenticator_to_endpoint(
            provider,
            authentication_scope,
            url.as_str(),
            authenticated_wire_body_max_bytes,
        );
        (
            PinnedEndpoint {
                url,
                client,
                authentication_scope,
                authenticator,
                authenticated_wire_body_max_bytes,
            },
            handle,
        )
    }

    async fn spawn_router(router: Router, path: &str) -> (PinnedEndpoint, JoinHandle<()>) {
        spawn_router_with_authenticator(
            router,
            path,
            GovernanceDagAuthenticationScope::Ipfs,
            Arc::new(TestAuthenticator::new(
                TEST_IPFS_AUTH_HANDLE,
                "mock-router-authenticator",
            )),
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
            let cid = json::from_slice::<JsonValue>(&state.add_body)
                .ok()
                .and_then(|value| {
                    value
                        .get("Hash")
                        .and_then(JsonValue::as_str)
                        .map(str::to_owned)
                })
                .unwrap_or_else(|| TEST_CID_PAYLOAD.to_owned());
            format!(r#"{{"Keys":{{"{cid}":{{}}}}}}"#)
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
        duplicate_etag: bool,
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
        response.headers_mut().append(
            header::ETAG,
            HeaderValue::from_str(&state.etag).expect("mock ETag"),
        );
        if state.duplicate_etag {
            response
                .headers_mut()
                .append(header::ETAG, HeaderValue::from_static("\"duplicate\""));
        }
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
        spawn_signed_head_with_authenticator(
            inner,
            Arc::new(TestAuthenticator::new(
                TEST_HEAD_AUTH_HANDLE,
                "mock-signed-head-authenticator",
            )),
        )
        .await
    }

    async fn spawn_signed_head_with_authenticator<T>(
        inner: SignedHeadInner,
        provider: Arc<T>,
    ) -> (PinnedEndpoint, SignedHeadState, JoinHandle<()>)
    where
        T: TestRebindableRequestAuthenticator + 'static,
    {
        let state = SignedHeadState(Arc::new(Mutex::new(inner)));
        let router = Router::new()
            .route("/head", get(mock_signed_head_get).put(mock_signed_head_put))
            .with_state(state.clone());
        let (endpoint, handle) = spawn_router_with_authenticator(
            router,
            "/head",
            GovernanceDagAuthenticationScope::SignedHead,
            provider,
        )
        .await;
        (endpoint, state, handle)
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
            max_future_skew_secs: 60,
            allow_head_bootstrap: true,
            expected_producer_signer_handle: producer_signer_handle.to_owned(),
            expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
            expected_checkpoint_store_handle: "kms:governance-dag:source-producer-checkpoint"
                .to_owned(),
            expected_checkpoint_store_qualification: TEST_STORE_QUALIFICATION,
            expected_publisher_peer_id: TEST_PRODUCER_PEER_ID.as_bytes().to_vec(),
            expected_public_key: signer.public_key,
        };

        assert!(
            !config.source_dir.join("runtime-dag-index.json").exists()
                && !config.source_dir.join("runtime-dag/head.to").exists(),
            "a fresh producer must expose mutable head/index state only through the typed store"
        );

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

        let committed = load_runtime_dag_committed_snapshot_v1(&config.source_root_guard)
            .expect("load publisher typed committed state")
            .expect("publisher committed state exists");
        let original_head_bytes = committed.head_bytes().to_vec();
        let original_index_bytes = committed.index_bytes().to_vec();
        let original_index: JsonValue = json::from_slice(&original_index_bytes)
            .expect("decode publisher runtime index for strict binding tests");
        let mut strict_drift_cases = Vec::new();
        for (field, replacement) in [
            ("source", JsonValue::from("substituted")),
            ("root", JsonValue::from("runtime-dag")),
            ("generated_at", JsonValue::from(timestamp.saturating_add(1))),
            ("signer_handle", JsonValue::from("hsm:attacker")),
            ("signer_revision", JsonValue::from(2_u64)),
            (
                "signer_policy_digest_hex",
                JsonValue::from(hex::encode([0xA1; 32])),
            ),
            ("publisher_peer_id", JsonValue::from("attacker-peer")),
            ("checkpoint_store_handle", JsonValue::from("kms:attacker")),
            ("checkpoint_store_revision", JsonValue::from(2_u64)),
            (
                "checkpoint_store_policy_digest_hex",
                JsonValue::from(hex::encode([0xA2; 32])),
            ),
        ] {
            let mut drifted = original_index.clone();
            drifted
                .as_object_mut()
                .expect("runtime index object")
                .insert(field.to_owned(), replacement);
            strict_drift_cases.push((field, json::to_json_pretty(&drifted).expect("encode drift")));
        }
        strict_drift_cases.push((
            "noncanonical-json",
            json::to_json(&original_index).expect("encode compact runtime index"),
        ));
        let mut unknown_top = original_index.clone();
        unknown_top
            .as_object_mut()
            .expect("runtime index object")
            .insert("unknown_top_level".into(), JsonValue::from(true));
        strict_drift_cases.push((
            "unknown-top-level",
            json::to_json_pretty(&unknown_top).expect("encode unknown top-level field"),
        ));
        let mut unknown_block = original_index.clone();
        unknown_block
            .get_mut("blocks")
            .and_then(JsonValue::as_array_mut)
            .and_then(|blocks| blocks.first_mut())
            .and_then(JsonValue::as_object_mut)
            .expect("first runtime index block")
            .insert("unknown_block_field".into(), JsonValue::from(true));
        strict_drift_cases.push((
            "unknown-block-field",
            json::to_json_pretty(&unknown_block).expect("encode unknown block field"),
        ));
        for (field, drifted) in strict_drift_cases {
            write_runtime_dag_committed_snapshot_fixture_v1(
                &config.source_dir,
                original_head_bytes.clone(),
                drifted.into_bytes(),
            )
            .expect("commit strict-boundary drift");
            let error = match load_source_snapshot(&config) {
                Ok(_) => panic!("runtime index `{field}` drift must fail closed"),
                Err(error) => error,
            };
            assert!(
                matches!(error, GovernanceDagServiceError::Source(_)),
                "unexpected `{field}` drift error: {error}"
            );
            write_runtime_dag_committed_snapshot_fixture_v1(
                &config.source_dir,
                original_head_bytes.clone(),
                original_index_bytes.clone(),
            )
            .expect("restore strict runtime index fixture");
        }
        let mut provenance_tampered_index: JsonValue = json::from_slice(&original_index_bytes)
            .expect("decode publisher runtime index for provenance tamper");
        provenance_tampered_index
            .get_mut("blocks")
            .and_then(JsonValue::as_array_mut)
            .and_then(|blocks| blocks.first_mut())
            .and_then(JsonValue::as_object_mut)
            .expect("first publisher runtime index entry")
            .insert(
                "submission_publisher_account_digest_hex".into(),
                JsonValue::from(hex::encode([0xA5; 32])),
            );
        let provenance_tampered_bytes = json::to_json_pretty(&provenance_tampered_index)
            .expect("encode provenance-tampered runtime index")
            .into_bytes();
        write_runtime_dag_committed_snapshot_fixture_v1(
            &config.source_dir,
            original_head_bytes.clone(),
            provenance_tampered_bytes,
        )
        .expect("commit provenance-tampered typed runtime state");
        let provenance_error = load_source_snapshot(&config)
            .expect_err("unsigned runtime-index provenance must not override the signed node");
        assert!(
            provenance_error
                .to_string()
                .contains("submission provenance does not match its signed governance node"),
            "unexpected provenance substitution error: {provenance_error}"
        );
        write_runtime_dag_committed_snapshot_fixture_v1(
            &config.source_dir,
            original_head_bytes.clone(),
            original_index_bytes.clone(),
        )
        .expect("restore typed runtime state");

        let original_index_value: JsonValue = json::from_slice(&original_index_bytes)
            .expect("decode runtime index for immutable-source tests");
        let first_original_entry = original_index_value
            .get("blocks")
            .and_then(JsonValue::as_array)
            .and_then(|blocks| blocks.first())
            .and_then(JsonValue::as_object)
            .expect("first immutable runtime entry");
        let json_source_path = first_original_entry
            .get("json_path")
            .and_then(JsonValue::as_str)
            .expect("JSON source path");
        let json_source_path = config.source_dir.join(json_source_path);
        let original_json_source = fs::read(&json_source_path).expect("read original JSON source");
        write_test_sidecar_file(&json_source_path, br#"{"substituted":true}"#);
        let error = load_source_snapshot(&config)
            .expect_err("JSON source substitution must violate its content-addressed pair path");
        assert!(
            error.to_string().contains("source paths do not bind"),
            "unexpected JSON source substitution error: {error}"
        );
        write_test_sidecar_file(&json_source_path, &original_json_source);

        let orphan_path = config
            .source_dir
            .join(GOVERNANCE_RUNTIME_DAG_DIR)
            .join(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR)
            .join("orphan.to");
        write_test_sidecar_file(&orphan_path, b"unindexed-runtime-artifact");
        let error = load_source_snapshot(&config)
            .expect_err("an unindexed immutable runtime artifact must fail exact inventory");
        assert!(
            error.to_string().contains("unindexed or missing artifact"),
            "unexpected runtime inventory error: {error}"
        );
        fs::remove_file(&orphan_path).expect("remove orphan test artifact");
        fs::remove_file(digest_sidecar_path(&orphan_path))
            .expect("remove orphan test digest sidecar");

        let mut index: JsonValue = json::from_slice(&original_index_bytes)
            .expect("decode publisher runtime index for source substitution");
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
        write_runtime_dag_committed_snapshot_fixture_v1(
            &config.source_dir,
            original_head_bytes,
            tampered_index,
        )
        .expect("commit source-substituted typed runtime state");
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
    fn source_loader_rejects_legacy_loose_authorities_without_cleanup() {
        for relative in [
            "runtime-dag-index.json",
            ".runtime-dag-index.json.tmp-42000-1",
            "runtime-dag/head.to",
            "runtime-dag/.head.to.retained-v1-0000",
        ] {
            let root = secure_temp_dir();
            let source_dir = root.path().join("source");
            let mut source =
                signed_source(1, 0x7a, current_unix_timestamp_seconds().saturating_sub(1));
            materialize_source_snapshot(&source_dir, &mut source);
            let legacy_path = source_dir.join(relative);
            if let Some(parent) = legacy_path.parent() {
                fs::create_dir_all(parent).expect("create legacy authority parent");
            }
            fs::write(&legacy_path, b"legacy-runtime-authority-must-remain")
                .expect("seed legacy runtime authority");
            let config = test_runtime_config(&source, root.path());

            let error = load_source_snapshot(&config)
                .expect_err("service must reject a competing legacy runtime authority");

            assert!(
                error.to_string().contains("legacy"),
                "unexpected error for `{relative}`: {error}"
            );
            assert_eq!(
                fs::read(&legacy_path).expect("read preserved legacy runtime authority"),
                b"legacy-runtime-authority-must-remain"
            );
            assert!(
                source_dir.join("governance-runtime-committed-v1").is_dir(),
                "legacy rejection must not mutate the typed committed store"
            );
        }
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

        let state_dir = root.path().join("state");
        fs::create_dir_all(&state_dir).expect("create segmented source-loader state root");
        let outgoing_config = RuntimeConfig {
            source_root_guard: GovernanceFilesystemRootGuard::capture_source(&source_dir)
                .expect("fence outgoing segmented publisher source root"),
            source_dir: source_dir.clone(),
            state_root_guard: GovernanceFilesystemRootGuard::capture_writer(&state_dir)
                .expect("fence outgoing segmented source-loader state root"),
            listen_addr: "127.0.0.1:0".parse().expect("test address"),
            poll_interval: Duration::from_millis(10),
            max_response_bytes: 1024 * 1024,
            max_request_bytes: 1024 * 1024,
            max_future_skew_secs: 60,
            allow_head_bootstrap: true,
            expected_producer_signer_handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
            expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
            expected_checkpoint_store_handle: TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
            expected_checkpoint_store_qualification: TEST_STORE_QUALIFICATION,
            expected_publisher_peer_id: publisher_peer_id.clone(),
            expected_public_key: outgoing_public_key,
        };
        let service_store = test_checkpoint_store(Arc::clone(&checkpoint_provider));
        let outgoing_source = load_committed_source_snapshot(&outgoing_config, &service_store)
            .expect("service authenticates the outgoing source before rotation");
        let outgoing_checkpoint = checkpoint_from_source(&outgoing_source);
        let outgoing_intent = intent_from_source(&outgoing_source);
        drop(outgoing_config);

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
            max_future_skew_secs: 60,
            allow_head_bootstrap: true,
            expected_producer_signer_handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
            expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
            expected_checkpoint_store_handle: TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
            expected_checkpoint_store_qualification: TEST_STORE_QUALIFICATION,
            expected_publisher_peer_id: publisher_peer_id,
            expected_public_key: incoming_public_key,
        };
        let rotated_without_append = load_committed_source_snapshot(&config, &service_store)
            .expect("incoming binding authenticates the outgoing-signed retained tip");
        assert_eq!(rotated_without_append.blocks.len(), 1);
        assert_eq!(
            rotated_without_append.head_bytes,
            outgoing_source.head_bytes
        );
        assert_ne!(
            rotated_without_append.index_blake3,
            outgoing_source.index_blake3
        );
        assert_eq!(
            rotated_without_append.chain_blake3,
            outgoing_source.chain_blake3
        );
        assert_eq!(
            rotated_without_append.head.head_signature.public_key,
            outgoing_public_key.to_vec()
        );
        validate_checkpoint_against_source(Some(&outgoing_checkpoint), &rotated_without_append)
            .expect("service checkpoint continuity survives a provider-only rotation");
        validate_intent_against_source(&outgoing_intent, None, &rotated_without_append)
            .expect("active service intent continuity survives a provider-only rotation");

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
        validate_checkpoint_against_source(Some(&outgoing_checkpoint), &segmented)
            .expect("an authenticated checkpoint at N remains valid after source advances to N+1");
        validate_intent_against_source(&outgoing_intent, None, &segmented)
            .expect("a sealed target at N remains recoverable after source advances to N+1");

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
    fn service_checkpoint_and_intent_bind_rotation_stable_signed_head() {
        let source = signed_source(2, 0x75, current_unix_timestamp_seconds().saturating_sub(10));
        let checkpoint = checkpoint_from_source(&source);
        let intent = intent_from_source(&source);
        let mut provider_rotated = source.clone();
        provider_rotated.index_blake3[0] ^= 0x80;
        validate_checkpoint_against_source(Some(&checkpoint), &provider_rotated)
            .expect("provider-only index drift preserves checkpoint chain continuity");
        validate_intent_against_source(&intent, None, &provider_rotated)
            .expect("provider-only index drift preserves intent chain continuity");

        let mut substituted_head = source.clone();
        substituted_head.head_bytes[0] ^= 0x80;
        assert!(
            validate_checkpoint_against_source(Some(&checkpoint), &substituted_head)
                .expect_err("checkpoint continuity must bind the exact signed source chain")
                .to_string()
                .contains("authenticated checkpoint")
        );
        assert!(
            validate_intent_against_source(&intent, None, &substituted_head)
                .expect_err("intent continuity must bind the exact signed source chain")
                .to_string()
                .contains("durable publish intent")
        );
    }

    #[test]
    fn active_publish_intent_must_cover_the_exact_unpublished_suffix() {
        let timestamp = current_unix_timestamp_seconds().saturating_sub(10);
        let source = signed_source(3, 0x7b, timestamp);

        let mut omitted_prefix = intent_from_source(&source);
        omitted_prefix.blocks.remove(0);
        let error = validate_intent_against_source(&omitted_prefix, None, &source)
            .expect_err("an active intent cannot omit the first unpublished block");
        assert!(
            error
                .to_string()
                .contains("complete unpublished source suffix")
        );

        let predecessor_source = signed_source(1, 0x7b, timestamp);
        let checkpoint = checkpoint_from_source(&predecessor_source);
        let mut successor_intent = intent_from_source(&source);
        successor_intent.blocks.remove(0);
        successor_intent.previous_public_head_blake3 = Some(checkpoint.head_bytes_blake3);
        validate_intent_against_source(&successor_intent, Some(&checkpoint), &source)
            .expect("an active intent may contain exactly the suffix after its checkpoint");

        let completed_checkpoint = checkpoint_from_source(&source);
        let completed_intent = intent_from_source(&source);
        validate_intent_against_source(&completed_intent, Some(&completed_checkpoint), &source)
            .expect("completed same-generation recovery validates without replaying publication");
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
    fn mirror_retention_uses_protocol_constants_and_honours_both_caps() {
        let source = signed_source(3, 0x38, 1_800_000_000);
        let intent = intent_from_source(&source);
        let latest = source.blocks[2].bytes.len() as u64;
        let previous = source.blocks[1].bytes.len() as u64;
        let exact_two = latest + previous;
        let retained = retained_source_suffix_with_limits(&source, 2, exact_two)
            .expect("retain exact two-block suffix");
        assert_eq!(retained.len(), 2);
        assert_eq!(retained[0].block.sequence, 1);
        assert_eq!(retained[1].block.sequence, 2);

        let one = retained_source_suffix_with_limits(&source, 1, exact_two)
            .expect("entry cap retains one block");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].block.sequence, 2);

        let byte_limited = retained_source_suffix_with_limits(&source, 3, exact_two - 1)
            .expect("byte cap retains the newest fitting suffix");
        assert_eq!(byte_limited.len(), 1);
        assert!(retained_source_suffix_with_limits(&source, 3, latest - 1).is_err());

        let protocol_retained = merge_published_blocks(None, &intent, &[], &source)
            .expect("protocol retention keeps this small complete source");
        assert_eq!(protocol_retained.len(), source.blocks.len());

        let prefix = signed_source(1, 0x38, 1_800_000_000);
        assert_eq!(prefix.blocks[0].bytes, source.blocks[0].bytes);
        let checkpoint = checkpoint_from_source(&prefix);
        let mut append_intent = intent_from_source(&source);
        append_intent.generation = checkpoint.generation + 1;
        append_intent.previous_public_head_blake3 = Some(checkpoint.head_bytes_blake3);
        append_intent.blocks.drain(..1);
        let expanded = merge_published_blocks(Some(&checkpoint), &append_intent, &[], &source)
            .expect("append from a one-block checkpoint backfills the complete retained suffix");
        assert_eq!(
            expanded
                .iter()
                .map(|block| block.sequence)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn checkpoint_requires_the_exact_protocol_retained_suffix() {
        let source = signed_source(3, 0x39, 1_800_000_000);
        let checkpoint = checkpoint_from_source(&source);
        validate_checkpoint_against_source(Some(&checkpoint), &source)
            .expect("complete protocol-retained suffix validates");

        let mut under_retained = checkpoint;
        under_retained.mirror_blocks.remove(0);
        validate_checkpoint_body(&under_retained)
            .expect("omitting the oldest retained mapping remains structurally well formed");
        let error = validate_checkpoint_against_source(Some(&under_retained), &source)
            .expect_err("an authenticated checkpoint must not under-retain the protocol suffix");
        assert!(
            error
                .to_string()
                .contains("exact version-1 retained source suffix"),
            "unexpected exact-retention error: {error}"
        );
    }

    #[test]
    fn checkpoint_body_rejects_inventory_above_protocol_retention_cap() {
        let source = signed_source(1, 0x3A, 1_800_000_000);
        let mut checkpoint = checkpoint_from_source(&source);
        let prototype = checkpoint.mirror_blocks[0].clone();
        let last_sequence =
            u64::try_from(GOVERNANCE_DAG_MIRROR_MAX_ENTRIES_V1).expect("V1 entry cap fits u64");
        checkpoint.mirror_blocks = (0..=last_sequence)
            .map(|sequence| {
                let mut published = prototype.clone();
                published.sequence = sequence;
                let mut block_cid = [0_u8; 32];
                block_cid[..8].copy_from_slice(&sequence.to_le_bytes());
                published.governance_block_cid = block_cid.to_vec();
                published
            })
            .collect();
        checkpoint.head_block_cid = checkpoint
            .mirror_blocks
            .last()
            .expect("over-cap inventory is nonempty")
            .governance_block_cid
            .clone();

        let error = validate_checkpoint_body(&checkpoint)
            .expect_err("one entry above the protocol retention cap must fail closed");
        assert!(
            error.to_string().contains("first-release bounds"),
            "unexpected over-cap checkpoint error: {error}"
        );
    }

    #[test]
    fn v1_max_retention_encoding_budget_fits_durable_stores() {
        // Canonical pretty JSON uses six spaces for nested block fields, four
        // for lookup rows, and six for payload-kind positions. This per-entry
        // budget covers the eleven block fields, three 64-byte-key lookup rows,
        // and one kind position at the widest V1 numeric/string widths. The
        // fixed allowance covers root/head fields, map framing, and all kind keys.
        const MAX_PAYLOAD_KIND_BYTES: usize = 48;
        const MAX_SUBMISSION_ORIGIN_BYTES: usize = 32;
        const MAX_MIRROR_FIXED_JSON_BYTES: usize = 1024 * 1024;
        const MAX_MIRROR_STORE_WRAPPER_BYTES: usize = 4 * 1024;
        const MAX_SEALED_FIXED_BYTES: usize = 1024 * 1024;

        let quoted = |bytes: usize| bytes.saturating_add(2);
        let block_fields = [
            ("position", 5),
            ("sequence", 20),
            ("timestamp", 20),
            ("payload_kind", quoted(MAX_PAYLOAD_KIND_BYTES)),
            ("block_cid_hex", quoted(64)),
            ("node_cid_hex", quoted(64)),
            ("blake3", quoted(64)),
            (
                "encoded_len",
                GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1
                    .to_string()
                    .len(),
            ),
            ("ipfs_cid", quoted(59)),
            ("submission_publisher_account_digest_hex", quoted(64)),
            ("submission_origin", quoted(MAX_SUBMISSION_ORIGIN_BYTES)),
        ];
        let block_object_bytes = block_fields
            .iter()
            .map(|(key, value_bytes)| 6 + quoted(key.len()) + 2 + *value_bytes + 2)
            .sum::<usize>()
            // Opening/closing lines, conservatively retaining a trailing comma.
            .saturating_add(13);
        let lookup_row_bytes = 4 + quoted(64) + 2 + 5 + 2;
        let kind_position_bytes = 6 + 5 + 2;
        let mirror_entry_bytes = block_object_bytes
            .saturating_add(lookup_row_bytes.saturating_mul(3))
            .saturating_add(kind_position_bytes);
        let mirror_json_upper = mirror_entry_bytes
            .checked_mul(GOVERNANCE_DAG_MIRROR_MAX_ENTRIES_V1)
            .and_then(|bytes| bytes.checked_add(MAX_MIRROR_FIXED_JSON_BYTES))
            .expect("V1 mirror JSON budget arithmetic cannot overflow");
        assert!(
            mirror_json_upper <= GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1 as usize,
            "V1 mirror JSON upper bound {mirror_json_upper} exceeds the durable byte ceiling"
        );
        assert!(
            mirror_json_upper.saturating_add(MAX_MIRROR_STORE_WRAPPER_BYTES)
                <= MIRROR_INDEX_STORE_MAX_PAYLOAD_BYTES,
            "V1 mirror wrapper no longer fits its two-slot store"
        );

        // A standalone Norito frame includes at least as much framing as the
        // same value nested in a vector, so multiplying a widest-string sample
        // frame gives a conservative, allocation-free maximum inventory bound.
        // Submission provenance is not stored in either sealed entry type; it
        // is derived from authenticated source blocks only for the JSON bound above.
        let source = signed_source(1, 0x3B, 1_800_000_000);
        let checkpoint = checkpoint_from_source(&source);
        let mut published_sample = checkpoint.mirror_blocks[0].clone();
        published_sample.sequence = u64::MAX;
        published_sample.timestamp = u64::MAX;
        published_sample.encoded_len = u64::MAX;
        published_sample.payload_kind = "x".repeat(MAX_PAYLOAD_KIND_BYTES);
        let published_frame_bytes = norito::to_bytes(&published_sample)
            .expect("encode maximum-width published-block sizing sample")
            .len();
        let checkpoint_upper = published_frame_bytes
            .checked_mul(GOVERNANCE_DAG_MIRROR_MAX_ENTRIES_V1)
            .and_then(|bytes| bytes.checked_add(MAX_SEALED_FIXED_BYTES))
            .expect("V1 checkpoint budget arithmetic cannot overflow");
        assert!(
            checkpoint_upper <= GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1 as usize,
            "V1 checkpoint upper bound {checkpoint_upper} exceeds the sealed-state ceiling"
        );

        let intent = intent_from_source(&source);
        let mut intent_sample = intent.blocks[0].clone();
        intent_sample.sequence = u64::MAX;
        intent_sample.timestamp = u64::MAX;
        intent_sample.encoded_len = u64::MAX;
        intent_sample.payload_kind = "x".repeat(MAX_PAYLOAD_KIND_BYTES);
        let intent_frame_bytes = norito::to_bytes(&intent_sample)
            .expect("encode maximum-width intent-block sizing sample")
            .len();
        let intent_upper = intent_frame_bytes
            .checked_mul(SOURCE_ENTRY_HARD_CAP)
            .and_then(|bytes| bytes.checked_add(MAX_SEALED_FIXED_BYTES))
            .expect("V1 publish-intent budget arithmetic cannot overflow");
        assert!(
            intent_upper <= GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1 as usize,
            "V1 publish-intent upper bound {intent_upper} exceeds the sealed-state ceiling"
        );
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

    #[test]
    fn signed_head_accepts_only_canonical_strong_entity_tags() {
        for valid in [r#"""#, r#""v1""#, r#""!#$%&'()*+-.^_`|~""#] {
            let value = HeaderValue::from_str(valid).expect("valid test header value");
            assert_eq!(strong_http_entity_tag(&value).as_deref(), Some(valid));
        }
        for invalid in ["v1", r#"W/"v1""#, r#""a\"b""#, r#""a b""#] {
            let value = HeaderValue::from_str(invalid).expect("representable invalid ETag");
            assert!(
                strong_http_entity_tag(&value).is_none(),
                "accepted noncanonical ETag {invalid:?}"
            );
        }
        let obs_text = HeaderValue::from_bytes(&[b'"', 0x80, b'"'])
            .expect("HTTP header values can represent obsolete text");
        assert!(strong_http_entity_tag(&obs_text).is_none());
    }

    #[tokio::test]
    async fn routes_reject_noncanonical_identifiers_before_lookup() {
        let telemetry = ApiState(Arc::new(RwLock::new(ApiSnapshot {
            live: true,
            ready: true,
            ..ApiSnapshot::default()
        })));
        let dir = secure_temp_dir();
        let source = signed_source(1, 0x2b, 1_800_000_000);
        let config = test_runtime_config(&source, dir.path());
        let mirror_store = open_mirror_index_store(&config).expect("initialize typed mirror store");
        drop(mirror_store);
        let mirror_reader = GovernanceDagMirrorReadHandleV1::try_new(
            &config,
            test_checkpoint_store(Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE))),
            true,
        )
        .expect("construct bootstrap mirror reader");
        let app = service_router(ServiceApiState {
            telemetry,
            mirror_reader,
        });
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
                calls_for_resolution.fetch_add(1, AtomicOrdering::SeqCst);
                Ok(vec![public, public])
            },
            Duration::from_secs(1),
            false,
        )
        .await
        .expect("one pinned public DNS snapshot");
        assert_eq!(resolved, vec![public]);
        assert_eq!(calls.load(AtomicOrdering::SeqCst), 1);
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
            provider.ingress_binding(),
            provider,
            "IPFS authenticator",
        )
        .expect("bind test authenticator");
        let endpoint = PinnedEndpoint {
            url: Url::parse("http://127.0.0.1:5001/").expect("test URL"),
            client: Client::builder().no_proxy().build().expect("test client"),
            authentication_scope: GovernanceDagAuthenticationScope::Ipfs,
            authenticator: authenticator.clone(),
            authenticated_wire_body_max_bytes: authenticated_ipfs_wire_body_max_bytes(
                GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1 as u64,
            )
            .expect("derive test authenticated wire-body bound"),
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

        let add_url = endpoint
            .ipfs_url("api/v0/add", IPFS_UNIXFS_V1_ADD_QUERY)
            .expect("construct fixed-profile IPFS add URL");
        let add_pairs = add_url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        assert_eq!(
            add_pairs,
            [
                ("cid-version", "1"),
                ("chunker", "size-1048576"),
                ("hash", "sha2-256"),
                ("max-file-links", "1024"),
                ("pin", "false"),
                ("quieter", "true"),
                ("raw-leaves", "true"),
                ("trickle", "false"),
                ("wrap-with-directory", "false"),
            ]
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
        );

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
            provider.ingress_binding(),
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
        assert_eq!(first.public_key(), provider.ingress_binding().public_key());

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
        assert!(error.to_string().contains("ingress qualification changed"));
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
            drifting_provider.ingress_binding(),
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
        assert!(error.to_string().contains("ingress qualification changed"));
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
            provider.ingress_binding(),
            provider,
            "IPFS authenticator",
        )
        .expect_err("mismatched authenticator handle must fail");
        assert!(mismatch.to_string().contains("does not match"));
    }

    #[test]
    fn outbound_nonce_sanity_window_never_exhausts_fresh_request_throughput() {
        let provider = Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "bounded-sender-window",
        ));
        let authenticator = OpaqueAuthenticator::try_new(
            TEST_IPFS_AUTH_HANDLE,
            TEST_AUTH_QUALIFICATION,
            provider.ingress_binding(),
            provider,
            "IPFS authenticator",
        )
        .expect("bind request authenticator");
        let request = canonical_test_request(
            GovernanceDagAuthenticationScope::Ipfs,
            "POST",
            "https://example.invalid/api/v0/pin/add?arg=cid&recursive=true",
            &[("accept-encoding", "identity")],
            b"",
        );

        let oldest = authenticator
            .authenticate(&request)
            .expect("authenticate initial request");
        for _ in 0..GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1 {
            authenticator
                .authenticate(&request)
                .expect("fresh outbound nonces must evict rather than exhaust the sender window");
        }
        authenticator
            .validate_envelope(&request, &oldest)
            .expect("sender eviction is not receiver replay authority");

        let now = current_unix_timestamp_seconds();
        let mut window = OutboundRequestNonceWindowV1::new();
        window
            .observe([0xA1; 32], now, now.saturating_sub(1))
            .expect("observe nonce before its expiry");
        window
            .observe([0xA1; 32], now.saturating_add(1), now)
            .expect("an envelope expiring at now is no longer live");
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
            provider.ingress_binding(),
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
        let zero_bound_error = GovernanceDagRequestIngressBindingV1::try_new(
            GovernanceDagAuthenticationScope::Ipfs,
            governance_dag_request_ingress_endpoint_binding_v1(
                GovernanceDagAuthenticationScope::Ipfs,
                "https://example.invalid/",
            )
            .expect("canonical zero-bound test endpoint"),
            policy.public_key(),
            0,
            policy.max_envelope_lifetime_secs(),
            policy.max_future_skew_secs(),
        )
        .expect_err("ingress binding must reject a zero body ceiling");
        assert_eq!(
            zero_bound_error,
            crate::GovernanceDagRequestIngressQualificationErrorV1::InvalidRequestBodyLimit
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

        for unexpected_name in [
            "cache-control",
            "x-request-id",
            "x-http-method-override",
            "x-original-url",
            "forwarded",
        ] {
            let mut fields = canonical.clone();
            fields.push((unexpected_name.to_owned(), b"semantic-extension".to_vec()));
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
            .expect_err("unsigned semantic headers must stop before backend dispatch");
            assert_eq!(
                error,
                GovernanceDagRequestAuthenticationErrorV1::UnexpectedHeader
            );
        }

        let unavailable_error = verify_request_before_test_backend(
            &request,
            &canonical,
            b"",
            GovernanceDagAuthenticationScope::Ipfs,
            &policy,
            now,
            &mut UnavailableTestReplayStore,
            &backend_calls,
        )
        .expect_err("unavailable shared replay state must stop before backend dispatch");
        assert_eq!(
            unavailable_error,
            GovernanceDagRequestAuthenticationErrorV1::ReplayStoreUnavailable
        );

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
            "https://example.invalid/governance/head",
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
                "https://example.invalid/governance/head",
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
                "https://example.invalid/governance/head",
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
                "https://example.invalid/governance/head-v8",
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
                "https://example.invalid/governance/head",
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
                "https://example.invalid/governance/head",
                &[
                    ("accept-encoding", "identity"),
                    ("content-type", "application/vnd.iroha.norito"),
                ],
                b"head-v7",
            ),
            canonical_test_request(
                GovernanceDagAuthenticationScope::SignedHead,
                "PUT",
                "https://example.invalid/governance/head",
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
                "https://example.invalid/governance/head",
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
    fn outbound_descriptor_binds_selected_headers_and_rejects_unsigned_semantics() {
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
        for unexpected_name in [
            "cache-control",
            "x-request-id",
            "x-http-method-override",
            "x-original-url",
            "forwarded",
        ] {
            let headers = [
                ("accept-encoding", b"identity".as_slice()),
                ("content-type", b"application/vnd.iroha.norito".as_slice()),
                ("content-length", b"14".as_slice()),
                (unexpected_name, b"semantic-extension".as_slice()),
            ];
            assert_eq!(
                canonicalize_governance_dag_outbound_http_request_v1(
                    GovernanceDagAuthenticationScope::SignedHead,
                    "PUT",
                    "https://example.invalid/governance/head",
                    headers,
                    body,
                    1024,
                ),
                Err(GovernanceDagRequestAuthenticationErrorV1::UnexpectedHeader)
            );
        }

        let changed_selected = canonicalize_governance_dag_outbound_http_request_v1(
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head",
            [
                ("accept-encoding", b"gzip".as_slice()),
                ("content-type", b"application/vnd.iroha.norito".as_slice()),
                ("content-length", b"14".as_slice()),
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
        let router = Router::new()
            .route("/drift", get(mock_authenticator_drift))
            .with_state(provider.clone());
        let (endpoint, task) = spawn_router_with_authenticator(
            router,
            "/drift",
            GovernanceDagAuthenticationScope::Ipfs,
            provider,
        )
        .await;
        let request = endpoint
            .request(Method::GET, endpoint.url.clone())
            .expect("construct drift request");
        let error = match endpoint.execute(request, "drift request failed").await {
            Ok(_) => panic!("post-execute policy drift must discard the response"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("ingress qualification changed"));
        assert!(!error.to_string().contains("in-flight-secret-token"));
        task.abort();
    }

    #[tokio::test]
    async fn authenticated_response_discards_body_when_qualification_drifts_before_eof() {
        let provider = Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "response-lifetime-secret",
        ));
        let router = Router::new().route("/body", get(|| async { "authenticated-body" }));
        let (endpoint, task) = spawn_router_with_authenticator(
            router,
            "/body",
            GovernanceDagAuthenticationScope::Ipfs,
            provider.clone(),
        )
        .await;
        let request = endpoint
            .request(Method::GET, endpoint.url.clone())
            .expect("construct authenticated body request");
        let response = endpoint
            .execute(request, "authenticated body request failed")
            .await
            .expect("receive response under the original qualification");
        provider
            .qualification_revision
            .store(2, AtomicOrdering::SeqCst);
        let error = read_bounded_response(response, 1024)
            .await
            .expect_err("qualification drift before completed consumption must discard the body");
        assert!(error.to_string().contains("ingress qualification changed"));
        assert!(!error.to_string().contains("response-lifetime-secret"));
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
        let request_auth_max_body_bytes =
            authenticated_ipfs_wire_body_max_bytes(view.service.max_request_bytes.0)
                .expect("derive authenticated wire-body binding");
        let registry = Arc::new(TestRuntimeProviderRegistry::returning(
            test_runtime_providers(
                &view,
                Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE)),
            ),
        ));
        let runtime_registry: Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1> =
            registry.clone();
        let providers = resolve_runtime_registry_providers(&view, Some(runtime_registry))
            .expect("registry resolves the configured providers");
        let _service = Service::from_view(view.clone(), providers)
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
        assert_eq!(observed.head_authenticator_handle(), TEST_HEAD_AUTH_HANDLE);
        assert_eq!(
            observed.head_authenticator_qualification(),
            TEST_AUTH_QUALIFICATION
        );
        assert_eq!(
            observed.ipfs_request_ingress_binding().max_body_bytes(),
            request_auth_max_body_bytes
        );
        assert_eq!(
            observed.head_request_ingress_binding().max_body_bytes(),
            view.service.max_request_bytes.0
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
            &test_runtime_providers(
                &view,
                Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE)),
            ),
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
            &test_runtime_providers(
                &view,
                Arc::new(TestSealedStore::new("kms:governance/checkpoint:test")),
            ),
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
            test_runtime_providers(&view, checkpoint_provider.clone()),
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
            view.clone(),
            test_runtime_providers(&view, checkpoint_provider),
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
    async fn prepare_recovers_empty_typed_mirror_before_exposing_runner() {
        let root = secure_temp_dir();
        let mut view = runtime_boundary_view(root.path());
        let mut source = signed_source(1, 0x70, current_unix_timestamp_seconds().saturating_sub(1));
        materialize_source_snapshot(
            view.source_dir.as_deref().expect("test source directory"),
            &mut source,
        );
        let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
        view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
        view.service.publisher_public_key_hex = Some(publisher_key_hex);

        let (head_endpoint, head_state, task) = spawn_signed_head(SignedHeadInner {
            bytes: Some(source.head_bytes.clone()),
            etag: "\"v1\"".to_owned(),
            ..SignedHeadInner::default()
        })
        .await;
        view.service.signed_head_url = Some(head_endpoint.url.to_string());

        let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        seed_producer_checkpoint(
            &checkpoint_provider,
            view.source_dir.as_deref().expect("test source directory"),
            &source,
        );
        let checkpoint = checkpoint_with_canonical_mirror(&source);
        let checkpoint_revision = save_checkpoint(
            &test_checkpoint_store(checkpoint_provider.clone()),
            None,
            &checkpoint,
        )
        .expect("seed authenticated checkpoint");

        let runner = prepare_governance_dag_service_from_view(
            view.clone(),
            test_runtime_providers(&view, checkpoint_provider),
        )
        .await
        .expect("prepare must recover an empty typed mirror before becoming ready");
        let mirror = runner
            .mirror_read_handle()
            .read()
            .expect("read prepared mirror capability")
            .expect("prepared runner exposes a checkpoint-coherent mirror");
        assert_eq!(
            blake3_array(mirror.canonical_bytes()),
            checkpoint.mirror_blake3
        );
        assert_eq!(
            mirror.checkpoint_identity(),
            GovernanceDagMirrorCheckpointIdentityV1 {
                generation: checkpoint.generation,
                revision: checkpoint_revision,
            }
        );
        assert_eq!(
            head_state.0.lock().await.put_count,
            0,
            "startup mirror recovery must not publish the public head"
        );
        drop(runner);
        task.abort();
    }

    #[tokio::test]
    async fn prepare_repairs_nonempty_checkpoint_incoherent_derived_mirror() {
        let root = secure_temp_dir();
        let mut view = runtime_boundary_view(root.path());
        let mut source = signed_source(1, 0x71, current_unix_timestamp_seconds().saturating_sub(1));
        materialize_source_snapshot(
            view.source_dir.as_deref().expect("test source directory"),
            &mut source,
        );
        let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
        view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
        view.service.publisher_public_key_hex = Some(publisher_key_hex);

        let (head_endpoint, head_state, task) = spawn_signed_head(SignedHeadInner {
            bytes: Some(source.head_bytes.clone()),
            etag: "\"v1\"".to_owned(),
            ..SignedHeadInner::default()
        })
        .await;
        view.service.signed_head_url = Some(head_endpoint.url.to_string());

        let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        seed_producer_checkpoint(
            &checkpoint_provider,
            view.source_dir.as_deref().expect("test source directory"),
            &source,
        );
        let checkpoint = checkpoint_with_canonical_mirror(&source);
        save_checkpoint(
            &test_checkpoint_store(checkpoint_provider.clone()),
            None,
            &checkpoint,
        )
        .expect("seed authenticated checkpoint");

        let service = Service::from_view(
            view.clone(),
            test_runtime_providers(&view, checkpoint_provider.clone()),
        )
        .await
        .expect("open service state for mismatch fixture");
        let mut drifted_mirror = mirror_index_value(
            &source,
            &checkpoint.mirror_blocks,
            checkpoint.generation,
            &checkpoint.head_ipfs_cid,
            checkpoint.published_at_unix,
        )
        .expect("build canonical mirror fixture");
        drifted_mirror
            .get_mut("head")
            .and_then(JsonValue::as_object_mut)
            .expect("mirror head object")
            .insert(
                "head_block_cid_hex".into(),
                JsonValue::from("00".repeat(32)),
            );
        let drifted_payload = MirrorIndexStorePayloadV1::committed(
            checkpoint.generation,
            [0; 32],
            json::to_json_pretty(&drifted_mirror)
                .expect("encode internally canonical drifted mirror")
                .into_bytes(),
        )
        .expect("construct internally valid drifted mirror payload");
        let (empty_snapshot, empty_payload) =
            load_mirror_index_store(&service.config, &service.mirror_store)
                .expect("load empty typed mirror");
        assert!(empty_payload.is_empty());
        compare_and_swap_mirror_index_store(
            &service.config,
            &service.mirror_store,
            &empty_snapshot,
            &drifted_payload,
        )
        .expect("install nonempty checkpoint-incoherent mirror fixture");
        drop(service);

        let runner = prepare_governance_dag_service_from_view(
            view.clone(),
            test_runtime_providers(&view, checkpoint_provider),
        )
        .await
        .expect("sealed checkpoint repairs a stale or corrupt local derived mirror");
        assert_eq!(
            verify_mirror_index_store(
                &runner.service.config,
                &runner.service.mirror_store,
                &checkpoint,
            )
            .expect("read repaired checkpoint-coherent mirror"),
            mirror_index_value(
                &source,
                &checkpoint.mirror_blocks,
                checkpoint.generation,
                &checkpoint.head_ipfs_cid,
                checkpoint.published_at_unix,
            )
            .expect("rebuild expected derived mirror")
        );
        assert_eq!(
            head_state.0.lock().await.put_count,
            0,
            "derived-cache recovery must not republish the public head"
        );
        drop(runner);
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
            view.clone(),
            test_runtime_providers(&view, checkpoint_provider),
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

        let mut service = Service::from_view(view.clone(), test_runtime_providers(&view, provider))
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
    async fn incomplete_service_intent_suffix_fails_before_all_publication_io() {
        let root = secure_temp_dir();
        let mut view = runtime_boundary_view(root.path());
        let mut source = signed_source(2, 0x7c, current_unix_timestamp_seconds().saturating_sub(2));
        let source_dir = view.source_dir.as_deref().expect("test source directory");
        materialize_source_snapshot(source_dir, &mut source);
        let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
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
        seed_producer_checkpoint(&provider, source_dir, &source);
        let mut incomplete_intent = intent_from_source(&source);
        incomplete_intent.blocks.remove(0);
        incomplete_intent.blocks[0].ipfs_cid = None;
        incomplete_intent.head_ipfs_cid = None;
        save_publish_intent(
            &test_checkpoint_store(provider.clone()),
            None,
            &incomplete_intent,
        )
        .expect("seal internally contiguous but incomplete service intent fixture");

        let mut service = Service::from_view(view.clone(), test_runtime_providers(&view, provider))
            .await
            .expect("construct service without performing public I/O");
        let error = service
            .reconcile_once()
            .await
            .expect_err("incomplete unpublished suffix must fail before publication");
        assert!(
            error
                .to_string()
                .contains("complete unpublished source suffix")
        );
        assert_eq!(
            request_count.load(AtomicOrdering::SeqCst),
            0,
            "invalid service intent must perform no Kubo or public-head I/O"
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

            let mut service =
                Service::from_view(view.clone(), test_runtime_providers(&view, provider))
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
            let mut service =
                Service::from_view(view.clone(), test_runtime_providers(&view, provider))
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
            view.clone(),
            test_runtime_providers(
                &view,
                Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE)),
            ),
        )
        .await
        .err()
        .expect("substituted configured provider qualification must fail");
        assert!(
            error
                .to_string()
                .contains("qualification or ingress binding does not match configuration")
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
                    test_runtime_providers(&view, Arc::new(TestSealedStore::new(provider_handle))),
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
        let error = Service::from_view(view.clone(), test_runtime_providers(&view, stale_store))
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

        let mut service = Service::from_view(
            view.clone(),
            test_runtime_providers(&view, provider.clone()),
        )
        .await
        .expect("initialize service at generation two");

        let original_record = provider
            .load(GovernanceDagSealedStateSlot::Checkpoint)
            .expect("load original checkpoint record")
            .expect("original checkpoint exists");
        let mut equivocated = checkpoint.clone();
        equivocated.published_at_unix = equivocated.published_at_unix.saturating_add(1);
        provider
            .inner
            .lock()
            .expect("lock test checkpoint store")
            .checkpoint = Some(GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::Checkpoint,
            equivocated.generation,
            norito::to_bytes(&equivocated).expect("encode same-generation equivocation"),
        ));
        let error = service
            .reconcile_once()
            .await
            .expect_err("same-generation checkpoint rewrite must fail closed");
        assert!(error.to_string().contains("equivocated"));
        provider
            .inner
            .lock()
            .expect("restore original checkpoint")
            .checkpoint = Some(original_record);

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
    async fn dropping_service_withdraws_every_retained_mirror_reader() {
        let root = secure_temp_dir();
        let view = runtime_boundary_view(root.path());
        let service = Service::from_view(
            view.clone(),
            test_runtime_providers(
                &view,
                Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE)),
            ),
        )
        .await
        .expect("construct supervised service");
        let reader = service.mirror_reader.clone();
        reader.mark_ready();
        drop(service);

        let error = reader
            .read()
            .expect_err("a reader retained past service shutdown must be unavailable");
        assert!(matches!(error, GovernanceDagServiceError::Unavailable(_)));
    }

    #[tokio::test]
    async fn pinned_endpoint_rejects_requests_outside_qualified_url_boundary() {
        let ipfs_provider = Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "qualified-url-ipfs",
        ));
        let ipfs_router = Router::new().route("/api/health", get(|| async { "ok" }));
        let (ipfs, ipfs_task) = spawn_router_with_authenticator(
            ipfs_router,
            "/api/",
            GovernanceDagAuthenticationScope::Ipfs,
            ipfs_provider,
        )
        .await;
        let allowed = ipfs.url.join("v0/add").expect("same-prefix Kubo URL");
        assert!(ipfs.request(Method::POST, allowed).is_ok());

        let sibling = ipfs
            .url
            .join("/api-shadow/v0/add")
            .expect("same-origin sibling URL");
        assert!(ipfs.request(Method::POST, sibling.clone()).is_err());
        let bypass = ipfs.client.request(Method::POST, sibling);
        let error = ipfs
            .execute(bypass, "unqualified URL test request failed")
            .await
            .err()
            .expect("execute must recheck a builder created outside PinnedEndpoint::request");
        assert!(error.to_string().contains("qualified ingress endpoint"));

        let cross_origin =
            Url::parse("http://example.com/api/v0/add").expect("canonical cross-origin test URL");
        assert!(ipfs.request(Method::POST, cross_origin).is_err());
        let encoded_separator = ipfs
            .url
            .join("v0/%2F..%2Fadmin")
            .expect("encoded-separator test URL");
        assert!(ipfs.request(Method::POST, encoded_separator).is_err());
        ipfs_task.abort();

        let head_provider = Arc::new(TestAuthenticator::new(
            TEST_HEAD_AUTH_HANDLE,
            "qualified-url-head",
        ));
        let head_router = Router::new().route("/head", get(|| async { "head" }));
        let (head, head_task) = spawn_router_with_authenticator(
            head_router,
            "/head",
            GovernanceDagAuthenticationScope::SignedHead,
            head_provider,
        )
        .await;
        assert!(head.request(Method::GET, head.url.clone()).is_ok());
        let mut altered_head = head.url.clone();
        altered_head.set_query(Some("generation=1"));
        assert!(head.request(Method::GET, altered_head).is_err());
        head_task.abort();
    }

    #[tokio::test]
    async fn hardened_http_refuses_redirect_header_body_and_encoding_attacks() {
        let redirect_router = Router::new()
            .route(
                "/redirect",
                get(|| async { Redirect::temporary("/target") }),
            )
            .route("/target", get(|| async { "followed" }));
        let (redirect, redirect_task) = spawn_router(redirect_router, "/").await;
        let mut redirect_url = redirect.url.clone();
        redirect_url.set_path("/redirect");
        let request = redirect
            .request(Method::GET, redirect_url)
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
        let (endpoint, task) = spawn_router(router, "/").await;
        let mut headers_url = endpoint.url.clone();
        headers_url.set_path("/headers");
        let request = endpoint
            .request(Method::GET, headers_url)
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

        let large = Arc::new(vec![0xA5; 5 * IPFS_UNIXFS_CHUNK_BYTES + 7]);
        let large_cid = canonical_ipfs_file_cid(&large)
            .expect("large test object fits the fixed UnixFS profile");
        let valid_large = MockIpfsState {
            add_body: Arc::new(format!(r#"{{"Hash":"{large_cid}"}}"#).into_bytes()),
            cat_body: large.clone(),
            pin_present: true,
        };
        let (endpoint, task) = spawn_router(mock_ipfs_router(valid_large), "/").await;
        assert_eq!(
            ipfs_add_verified(
                &endpoint,
                "large-block.to",
                &large,
                large.len() as u64,
                1024,
            )
            .await
            .expect("multi-chunk publication ignores the control-response cap for CAT"),
            large_cid
        );
        task.abort();
    }

    fn ipip_499_chacha20_bytes(seed: &[u8], length: usize) -> Vec<u8> {
        let key = iroha_crypto::sha256(seed);
        let mut initial = [0_u32; 16];
        initial[..4].copy_from_slice(&[0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574]);
        for (word, bytes) in initial[4..12].iter_mut().zip(key.chunks_exact(4)) {
            *word = u32::from_le_bytes(bytes.try_into().expect("SHA-256 word"));
        }
        let mut output = vec![0_u8; length];
        for (counter, output_block) in output.chunks_mut(64).enumerate() {
            initial[12] = u32::try_from(counter).expect("test stream counter fits u32");
            let mut state = initial;
            for _ in 0..10 {
                chacha20_quarter_round(&mut state, 0, 4, 8, 12);
                chacha20_quarter_round(&mut state, 1, 5, 9, 13);
                chacha20_quarter_round(&mut state, 2, 6, 10, 14);
                chacha20_quarter_round(&mut state, 3, 7, 11, 15);
                chacha20_quarter_round(&mut state, 0, 5, 10, 15);
                chacha20_quarter_round(&mut state, 1, 6, 11, 12);
                chacha20_quarter_round(&mut state, 2, 7, 8, 13);
                chacha20_quarter_round(&mut state, 3, 4, 9, 14);
            }
            for (word, original) in state.iter_mut().zip(initial) {
                *word = word.wrapping_add(original);
            }
            let mut encoded = [0_u8; 64];
            for (slot, word) in encoded.chunks_exact_mut(4).zip(state) {
                slot.copy_from_slice(&word.to_le_bytes());
            }
            output_block.copy_from_slice(&encoded[..output_block.len()]);
        }
        output
    }

    fn chacha20_quarter_round(
        state: &mut [u32; 16],
        a_index: usize,
        b_index: usize,
        c_index: usize,
        d_index: usize,
    ) {
        let mut a = state[a_index];
        let mut b = state[b_index];
        let mut c = state[c_index];
        let mut d = state[d_index];
        a = a.wrapping_add(b);
        d = (d ^ a).rotate_left(16);
        c = c.wrapping_add(d);
        b = (b ^ c).rotate_left(12);
        a = a.wrapping_add(b);
        d = (d ^ a).rotate_left(8);
        c = c.wrapping_add(d);
        b = (b ^ c).rotate_left(7);
        state[a_index] = a;
        state[b_index] = b;
        state[c_index] = c;
        state[d_index] = d;
    }

    #[test]
    fn fixed_unixfs_profile_matches_ipip_499_chunk_boundary_vectors() {
        const SMALL_CID: &str = "bafkreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e";
        const AT_CHUNK_CID: &str = "bafkreiacndfy443ter6qr2tmbbdhadvxxheowwf75s6zehscklu6ezxmta";
        const OVER_CHUNK_CID: &str = "bafybeigmix7t42i6jacydtquhet7srwvgpizfg7gjbq7627d35mjomtu64";

        assert_eq!(
            canonical_ipfs_file_cid(b"hello world").as_deref(),
            Some(SMALL_CID)
        );
        let bytes = ipip_499_chacha20_bytes(b"chunk-v1-seed", IPFS_UNIXFS_CHUNK_BYTES + 1);
        assert_eq!(
            canonical_ipfs_file_cid(&bytes[..IPFS_UNIXFS_CHUNK_BYTES]).as_deref(),
            Some(AT_CHUNK_CID)
        );
        assert_eq!(
            canonical_ipfs_file_cid(&bytes).as_deref(),
            Some(OVER_CHUNK_CID)
        );

        let mut tampered = bytes;
        *tampered.last_mut().expect("non-empty fixture") ^= 1;
        assert!(validate_ipfs_cid_for_bytes(OVER_CHUNK_CID, &tampered).is_err());
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
        let payload = b"\0payload\r\n";
        let name = "governance-head.to";
        let (boundary, body) = canonical_ipfs_multipart_body(name, payload)
            .expect("construct canonical multipart body");
        let (replayed_boundary, replayed_body) =
            canonical_ipfs_multipart_body(name, payload).expect("replay canonical multipart body");
        assert_eq!(boundary, replayed_boundary);
        assert_eq!(body, replayed_body);
        assert_eq!(
            body.len(),
            payload.len()
                + ipfs_multipart_wire_overhead(boundary.len(), name.len())
                    .expect("exact multipart framing overhead")
        );
        assert!(boundary.len() <= 70);
        assert!(body.starts_with(format!("--{boundary}\r\n").as_bytes()));
        assert!(body.ends_with(format!("\r\n--{boundary}--\r\n").as_bytes()));
        assert!(
            body.windows(b"\0payload\r\n".len())
                .any(|window| window == b"\0payload\r\n")
        );
        assert!(canonical_ipfs_multipart_body("../escape", b"payload").is_err());

        let object_max = payload.len() as u64;
        let authenticated_wire_max = authenticated_ipfs_wire_body_max_bytes(object_max)
            .expect("derive authenticated multipart wire bound");
        let max_boundary_len = IPFS_MULTIPART_BOUNDARY_PREFIX.len() + 1 + 32 + 3;
        let max_overhead =
            ipfs_multipart_wire_overhead(max_boundary_len, IPFS_MULTIPART_FILENAME_MAX_BYTES)
                .expect("derive maximum multipart framing overhead") as u64;
        assert_eq!(authenticated_wire_max - object_max, max_overhead);
        assert!(body.len() as u64 > object_max);
        assert!(body.len() as u64 <= authenticated_wire_max);
        assert!(authenticated_ipfs_wire_body_max_bytes(u64::MAX).is_err());

        let request = Client::new()
            .post("https://example.invalid/api/v0/add")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body.clone());
        assert!(
            request.try_clone().is_some(),
            "the final multipart request must remain inspectable by the authenticator"
        );
        let request = request.build().expect("finalize multipart request");
        assert!(
            canonical_outbound_request_descriptor(
                &request,
                GovernanceDagAuthenticationScope::Ipfs,
                object_max,
            )
            .is_err(),
            "the object ceiling must not be reused as the multipart wire ceiling"
        );
        canonical_outbound_request_descriptor(
            &request,
            GovernanceDagAuthenticationScope::Ipfs,
            authenticated_wire_max,
        )
        .expect("the checked multipart wire ceiling admits the exact final body");
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
            let (endpoint, _state, task) =
                spawn_signed_head_with_authenticator(inner, provider.clone()).await;
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
    async fn signed_head_read_rejects_duplicate_entity_tags() {
        let (endpoint, _state, task) = spawn_signed_head(SignedHeadInner {
            bytes: Some(b"head".to_vec()),
            etag: "\"v1\"".to_owned(),
            duplicate_etag: true,
            ..SignedHeadInner::default()
        })
        .await;
        let error = fetch_signed_http_head(&endpoint, 1024)
            .await
            .expect_err("multiple ETag fields must not define an ambiguous CAS token");
        assert!(error.to_string().contains("single canonical strong ETag"));
        task.abort();
    }

    #[test]
    fn mirror_index_exposes_only_signed_submission_provenance() {
        let source = signed_finance_source(0x39, 1_800_000_000);
        let checkpoint = checkpoint_from_source(&source);
        let mirror = mirror_index_value(
            &source,
            &checkpoint.mirror_blocks,
            checkpoint.generation,
            &checkpoint.head_ipfs_cid,
            checkpoint.published_at_unix,
        )
        .expect("build attributed mirror index");
        let entry = mirror
            .get("blocks")
            .and_then(JsonValue::as_array)
            .and_then(|blocks| blocks.first())
            .expect("attributed mirror block");
        let signed = source.blocks[0]
            .block
            .node
            .submission_provenance
            .as_ref()
            .expect("signed submission provenance");
        assert_eq!(
            entry
                .get("submission_publisher_account_digest_hex")
                .and_then(JsonValue::as_str),
            Some(hex::encode(signed.publisher_account_digest).as_str())
        );
        assert_eq!(
            entry.get("submission_origin").and_then(JsonValue::as_str),
            Some(signed.origin.label())
        );

        let internal_source = signed_source(1, 0x38, 1_800_000_000);
        let internal_checkpoint = checkpoint_from_source(&internal_source);
        let internal_mirror = mirror_index_value(
            &internal_source,
            &internal_checkpoint.mirror_blocks,
            internal_checkpoint.generation,
            &internal_checkpoint.head_ipfs_cid,
            internal_checkpoint.published_at_unix,
        )
        .expect("build internal-producer mirror index");
        let internal_entry = internal_mirror
            .get("blocks")
            .and_then(JsonValue::as_array)
            .and_then(|blocks| blocks.first())
            .expect("internal mirror block");
        assert_eq!(
            internal_entry.get("submission_publisher_account_digest_hex"),
            Some(&JsonValue::Null)
        );
        assert_eq!(
            internal_entry.get("submission_origin"),
            Some(&JsonValue::Null)
        );
    }

    #[test]
    fn mirror_two_slot_store_hard_cut_rejects_legacy_authority_without_cleanup() {
        for legacy_name in [
            LEGACY_MIRROR_INDEX_FILE,
            LEGACY_MIRROR_INDEX_SIDECAR_FILE,
            LEGACY_MIRROR_RECOVERY_QUARANTINE_DIR,
            ".governance-service-recovery-quarantine-v1.tmp-bad",
            ".governance-service-recovery-quarantine-v1.retained-v1-0000",
            ".governance-service-recovery-quarantine-v1.retained-v1-bad",
            "..governance-service-recovery-quarantine-v1.tmp-bad",
            "..governance-service-recovery-quarantine-v1.retained-v1-bad",
            ".mirror-index.json.tmp-42000-1",
            ".mirror-index.json.tmp-bad",
            ".mirror-index.json.retained-v1-0000",
            ".mirror-index.json.retained-v1-bad",
            ".mirror-index.json.blake3.tmp-42000-2",
            ".mirror-index.json.blake3.tmp-bad",
            ".mirror-index.json.blake3.retained-v1-0000",
            ".mirror-index.json.blake3.retained-v1-bad",
        ] {
            let dir = secure_temp_dir();
            let source = signed_source(2, 0x3a, 1_800_000_000);
            let config = test_runtime_config(&source, dir.path());
            let legacy_path = config.state_root_guard.root().join(legacy_name);
            fs::write(&legacy_path, b"legacy-sentinel-must-remain")
                .expect("seed retired mirror authority");

            let error = open_mirror_index_store(&config)
                .expect_err("legacy mirror authority must fail closed");

            assert!(
                error.to_string().contains("legacy mirror authority"),
                "unexpected error for `{legacy_name}`: {error}"
            );
            assert_eq!(
                fs::read(&legacy_path).expect("read preserved legacy mirror authority"),
                b"legacy-sentinel-must-remain"
            );
            assert!(
                !config
                    .state_root_guard
                    .root()
                    .join(MIRROR_INDEX_STORE_NAME)
                    .exists(),
                "legacy rejection must happen before typed-store initialization"
            );
        }
    }

    #[test]
    fn mirror_two_slot_payload_rejects_truncation_and_metadata_drift() {
        let dir = secure_temp_dir();
        let source = signed_source(2, 0x3b, 1_800_000_000);
        let config = test_runtime_config(&source, dir.path());
        let store = open_mirror_index_store(&config).expect("open mirror two-slot store");
        let mut checkpoint = checkpoint_from_source(&source);
        checkpoint.generation = 2;
        let mirror = mirror_index_value(
            &source,
            &checkpoint.mirror_blocks,
            checkpoint.generation,
            &checkpoint.head_ipfs_cid,
            checkpoint.published_at_unix,
        )
        .expect("build test mirror");
        let canonical = json::to_json_pretty(&mirror)
            .expect("encode test mirror")
            .into_bytes();
        checkpoint.mirror_blake3 = blake3_array(&canonical);

        let recovered = verify_or_recover_mirror_index_store(&config, &store, &checkpoint, &source)
            .expect("empty hard-cut store recovers from checkpoint");
        assert_eq!(recovered, mirror);

        let payload =
            MirrorIndexStorePayloadV1::committed(checkpoint.generation, [0; 32], canonical)
                .expect("construct canonical typed mirror");
        let encoded = encode_mirror_index_store_payload(&payload).expect("encode typed mirror");
        assert!(
            decode_mirror_index_store_payload(&encoded[..encoded.len() / 2]).is_err(),
            "a truncated typed payload must fail closed"
        );

        for (field, replacement) in [
            ("schema", JsonValue::from("wrong.schema")),
            ("generation", JsonValue::from(99_u64)),
        ] {
            let mut drifted = mirror.clone();
            drifted
                .as_object_mut()
                .expect("mirror object")
                .insert(field.into(), replacement);
            let bytes = json::to_json_pretty(&drifted)
                .expect("encode drifted mirror")
                .into_bytes();
            assert!(
                MirrorIndexStorePayloadV1::committed(checkpoint.generation, [0; 32], bytes)
                    .is_err(),
                "typed metadata must reject {field} drift"
            );
        }

        let mut head_drift = mirror.clone();
        head_drift
            .get_mut("head")
            .and_then(JsonValue::as_object_mut)
            .expect("head object")
            .insert(
                "head_block_cid_hex".into(),
                JsonValue::from("00".repeat(32)),
            );
        let head_drift_bytes = json::to_json_pretty(&head_drift)
            .expect("encode head-drifted mirror")
            .into_bytes();
        let head_drift_payload =
            MirrorIndexStorePayloadV1::committed(checkpoint.generation, [0; 32], head_drift_bytes)
                .expect("head drift remains internally canonical");
        let (snapshot, _) = load_mirror_index_store(&config, &store).expect("load typed mirror");
        compare_and_swap_mirror_index_store(&config, &store, &snapshot, &head_drift_payload)
            .expect("install internally canonical drift for verification test");
        let mut matching_digest_checkpoint = checkpoint.clone();
        matching_digest_checkpoint.mirror_blake3 = head_drift_payload.mirror_blake3;
        assert!(verify_mirror_index_store(&config, &store, &matching_digest_checkpoint).is_err());
        let repaired = verify_or_recover_mirror_index_store(&config, &store, &checkpoint, &source)
            .expect("checkpoint authority repairs a same-generation derived-cache drift");
        assert_eq!(repaired, mirror);

        let mut stale_mirror = mirror.clone();
        stale_mirror
            .as_object_mut()
            .expect("stale mirror object")
            .insert("generation".into(), JsonValue::from(1_u64));
        let stale_bytes = json::to_json_pretty(&stale_mirror)
            .expect("encode stale mirror")
            .into_bytes();
        let stale_payload = MirrorIndexStorePayloadV1::committed(1, [0; 32], stale_bytes)
            .expect("construct prior-generation derived mirror");
        let (snapshot, _) = load_mirror_index_store(&config, &store).expect("load repaired mirror");
        compare_and_swap_mirror_index_store(&config, &store, &snapshot, &stale_payload)
            .expect("represent an offline instance at the preceding local generation");
        assert_eq!(
            verify_or_recover_mirror_index_store(&config, &store, &checkpoint, &source)
                .expect("offline local mirror catches up from the authoritative checkpoint"),
            mirror
        );

        let mut ahead_mirror = mirror.clone();
        ahead_mirror
            .as_object_mut()
            .expect("ahead mirror object")
            .insert("generation".into(), JsonValue::from(3_u64));
        let ahead_bytes = json::to_json_pretty(&ahead_mirror)
            .expect("encode ahead mirror")
            .into_bytes();
        let ahead_payload = MirrorIndexStorePayloadV1::committed(3, [0; 32], ahead_bytes)
            .expect("construct ahead-generation derived mirror");
        let (snapshot, _) =
            load_mirror_index_store(&config, &store).expect("load caught-up mirror");
        compare_and_swap_mirror_index_store(&config, &store, &snapshot, &ahead_payload)
            .expect("represent a local generation ahead of authority");
        assert!(
            verify_or_recover_mirror_index_store(&config, &store, &checkpoint, &source).is_err(),
            "a local mirror ahead of sealed authority must fail closed"
        );
    }

    #[test]
    fn mirror_read_handle_returns_only_checkpoint_coherent_typed_bytes() {
        let dir = secure_temp_dir();
        let source = signed_source(2, 0x3d, 1_800_000_000);
        let config = test_runtime_config(&source, dir.path());
        let mirror_store = open_mirror_index_store(&config).expect("open mirror store");
        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let checkpoint_store = test_checkpoint_store(Arc::clone(&provider));

        let mut checkpoint = checkpoint_from_source(&source);
        let mirror = mirror_index_value(
            &source,
            &checkpoint.mirror_blocks,
            checkpoint.generation,
            &checkpoint.head_ipfs_cid,
            checkpoint.published_at_unix,
        )
        .expect("build mirror value");
        let canonical_bytes = json::to_json_pretty(&mirror)
            .expect("encode canonical mirror")
            .into_bytes();
        checkpoint.mirror_blake3 = blake3_array(&canonical_bytes);
        let (empty_snapshot, _) =
            load_mirror_index_store(&config, &mirror_store).expect("load empty mirror store");
        let committed_payload = MirrorIndexStorePayloadV1::committed(
            checkpoint.generation,
            [0; 32],
            canonical_bytes.clone(),
        )
        .expect("construct committed mirror payload");
        compare_and_swap_mirror_index_store(
            &config,
            &mirror_store,
            &empty_snapshot,
            &committed_payload,
        )
        .expect("commit mirror payload");
        let (committed_snapshot, committed_readback) =
            load_mirror_index_store(&config, &mirror_store).expect("reload committed mirror");
        assert_eq!(committed_readback, committed_payload);
        let checkpoint_revision =
            save_checkpoint(&checkpoint_store, None, &checkpoint).expect("seal checkpoint");
        let state_inventory_before = config
            .state_root_guard
            .rooted_directory()
            .child_names_bounded(SOURCE_ENTRY_HARD_CAP)
            .expect("inventory state before reader construction");

        let handle =
            GovernanceDagMirrorReadHandleV1::try_new(&config, checkpoint_store.clone(), false)
                .expect("construct coherent mirror reader");
        handle.mark_ready();
        assert_eq!(
            handle.binding().source_root_digest(),
            runtime_dag_producer_root_digest(&config.source_dir).expect("derive source digest")
        );
        assert_eq!(
            handle.binding().producer_signer_handle(),
            TEST_PRODUCER_SIGNER_HANDLE
        );
        assert_eq!(
            handle.binding().checkpoint_store_handle(),
            TEST_CHECKPOINT_STORE_HANDLE
        );

        let observed = handle
            .read()
            .expect("read coherent mirror capability")
            .expect("coherent checkpoint has a committed mirror snapshot");
        assert_eq!(observed.canonical_bytes(), canonical_bytes);
        assert_eq!(
            observed.mirror_store_identity(),
            (
                committed_snapshot.generation(),
                committed_snapshot.record_digest()
            )
        );
        assert_eq!(
            observed.checkpoint_identity().generation(),
            checkpoint.generation
        );
        assert_eq!(
            observed.checkpoint_identity().revision(),
            checkpoint_revision
        );
        assert_eq!(
            load_mirror_index_store(&config, &mirror_store)
                .expect("reload writer mirror after read")
                .0,
            committed_snapshot,
            "reader construction and read must not mutate either slot"
        );
        assert_eq!(
            config
                .state_root_guard
                .rooted_directory()
                .child_names_bounded(SOURCE_ENTRY_HARD_CAP)
                .expect("inventory state after reader read"),
            state_inventory_before,
            "reader construction and read must not create state"
        );

        let mut checkpoint_b = checkpoint.clone();
        checkpoint_b.generation = checkpoint
            .generation
            .checked_add(1)
            .expect("test checkpoint generation has successor");
        provider.return_checkpoint_on_second_load(GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::Checkpoint,
            checkpoint_b.generation,
            norito::to_bytes(&checkpoint_b).expect("encode raced checkpoint"),
        ));
        let error = handle
            .read()
            .expect_err("A/B checkpoint race must fail closed");
        assert!(error.to_string().contains("checkpoint changed during read"));

        let intent = intent_from_source(&source);
        provider.return_intent_on_second_load(GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::PublishIntent,
            intent.generation,
            norito::to_bytes(&intent).expect("encode raced intent"),
        ));
        let error = handle.read().expect_err("A/B intent race must fail closed");
        assert!(error.to_string().contains("intent changed during read"));

        let active_intent_revision =
            save_publish_intent(&checkpoint_store, None, &intent).expect("seal active intent");
        let error = handle
            .read()
            .expect_err("active intent must make mirror reads fail closed");
        assert!(error.to_string().contains("active sealed publish intent"));

        provider
            .qualification_revision
            .store(2, AtomicOrdering::SeqCst);
        let error = handle
            .read()
            .expect_err("provider qualification drift must invalidate reader");
        assert!(error.to_string().contains("identity or policy changed"));

        provider
            .qualification_revision
            .store(1, AtomicOrdering::SeqCst);
        delete_publish_intent(&checkpoint_store, Some(active_intent_revision))
            .expect("clear active test intent");
        let (current, _) =
            load_mirror_index_store(&config, &mirror_store).expect("load mirror before corruption");
        compare_and_swap_mirror_index_store(
            &config,
            &mirror_store,
            &current,
            &MirrorIndexStorePayloadV1::empty(),
        )
        .expect("commit internally valid but checkpoint-incoherent mirror");
        let error = handle
            .read()
            .expect_err("typed mirror corruption must fail closed");
        assert!(error.to_string().contains("no committed index"));

        #[cfg(unix)]
        {
            let state_root = config.state_root_guard.root().to_path_buf();
            let displaced = state_root.with_extension("displaced-reader-root");
            fs::rename(&state_root, &displaced).expect("displace retained state root");
            fs::create_dir(&state_root).expect("install substituted state root");
            fs::set_permissions(&state_root, fs::Permissions::from_mode(0o700))
                .expect("secure substituted state root");
            let error = handle
                .read()
                .expect_err("state-root substitution must invalidate reader");
            assert!(error.to_string().contains("state root"));
        }
    }

    #[test]
    fn mirror_read_handle_never_initializes_an_absent_store() {
        let dir = secure_temp_dir();
        let source = signed_source(1, 0x3e, 1_800_000_000);
        let config = test_runtime_config(&source, dir.path());
        let before = config
            .state_root_guard
            .rooted_directory()
            .child_names_bounded(1)
            .expect("inventory pristine state root");
        assert!(before.is_empty());
        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let checkpoint_store = test_checkpoint_store(provider);

        let error = GovernanceDagMirrorReadHandleV1::try_new(&config, checkpoint_store, true)
            .expect_err("reader must not initialize an absent mirror store");

        assert!(matches!(error, GovernanceDagServiceError::Filesystem(_)));
        assert_eq!(
            config
                .state_root_guard
                .rooted_directory()
                .child_names_bounded(1)
                .expect("inventory state after rejected reader"),
            before,
            "read capability construction must not create an init lock, directory, or slot"
        );
    }

    #[test]
    fn mirror_read_handle_install_readiness_requires_the_existing_typed_store() {
        let dir = secure_temp_dir();
        let source = signed_source(1, 0x3f, 1_800_000_000);
        let config = test_runtime_config(&source, dir.path());
        let mirror_store = open_mirror_index_store(&config).expect("initialize typed mirror store");
        drop(mirror_store);
        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let handle = GovernanceDagMirrorReadHandleV1::try_new(
            &config,
            test_checkpoint_store(provider),
            true,
        )
        .expect("construct genesis mirror reader from an existing empty store");
        handle
            .assert_install_ready()
            .expect("an existing canonical empty mirror store is install-ready at genesis");
        let bootstrap_inventory = config
            .state_root_guard
            .rooted_directory()
            .child_names_bounded(SOURCE_ENTRY_HARD_CAP)
            .expect("inventory bootstrap mirror state");
        assert!(
            handle
                .read()
                .expect("read authenticated bootstrap mirror state")
                .is_none(),
            "an empty mirror with no sealed checkpoint is authenticated bootstrap, not corruption"
        );
        assert_eq!(
            config
                .state_root_guard
                .rooted_directory()
                .child_names_bounded(SOURCE_ENTRY_HARD_CAP)
                .expect("inventory mirror state after bootstrap read"),
            bootstrap_inventory,
            "bootstrap reads must not initialize or mutate mirror state"
        );

        let mut store_directories = Vec::new();
        for entry in fs::read_dir(config.state_root_guard.root()).expect("list mirror state root") {
            let entry = entry.expect("read mirror state entry");
            if entry
                .file_type()
                .expect("inspect mirror state entry")
                .is_dir()
            {
                store_directories.push(entry.path());
            }
        }
        assert_eq!(
            store_directories.len(),
            1,
            "fresh mirror state has exactly one typed-store directory"
        );
        fs::remove_dir_all(&store_directories[0]).expect("remove typed mirror store fixture");

        let error = handle
            .assert_install_ready()
            .expect_err("a mirror capability whose typed store disappeared must not install");
        assert!(matches!(error, GovernanceDagServiceError::Filesystem(_)));
    }

    #[test]
    fn node_handle_installs_real_mirror_reader_once_across_preexisting_clones() {
        let dir = secure_temp_dir();
        let source = signed_source(1, 0x40, 1_800_000_000);
        let service_config = test_runtime_config(&source, dir.path());
        let mirror_store =
            open_mirror_index_store(&service_config).expect("initialize typed mirror store");
        drop(mirror_store);

        let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let signer = Arc::new(PublisherTestSigner {
            handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
            peer_id: TEST_PRODUCER_PEER_ID.as_bytes().to_vec(),
            signer: TestSigner::new(0x40),
        });
        let node_config = StorageConfig::builder()
            .enabled(true)
            .data_dir(dir.path().join("node-storage"))
            .governance_dir(Some(service_config.source_dir.clone()))
            .governance_dag_publisher_peer_id(Some(TEST_PRODUCER_PEER_ID.to_owned()))
            .governance_dag_signer_handle(Some(TEST_PRODUCER_SIGNER_HANDLE.to_owned()))
            .governance_dag_signer_qualification(Some(TEST_PRODUCER_SIGNER_QUALIFICATION))
            .governance_dag_publisher_public_key_hex(Some(hex::encode(signer.public_key())))
            .governance_dag_checkpoint_store_handle(Some(TEST_CHECKPOINT_STORE_HANDLE.to_owned()))
            .governance_dag_checkpoint_store_qualification(Some(TEST_STORE_QUALIFICATION))
            .build();
        let mut node = NodeHandle::try_new_with_runtime_deps(
            node_config,
            NodeRuntimeDeps::default()
                .with_governance_dag_signer(signer)
                .with_governance_dag_checkpoint_store(checkpoint_provider.clone()),
        )
        .expect("start node with the same retained Governance DAG providers");
        let mut clone_created_before_install = node.clone();

        let mismatch_root = dir.path().join("mismatched-reader");
        let mismatch_config = test_runtime_config(&source, &mismatch_root);
        let mismatch_store =
            open_mirror_index_store(&mismatch_config).expect("initialize mismatched mirror store");
        drop(mismatch_store);
        let mismatched_reader = GovernanceDagMirrorReadHandleV1::try_new(
            &mismatch_config,
            test_checkpoint_store(checkpoint_provider.clone()),
            true,
        )
        .expect("construct valid reader bound to the wrong producer root");
        let error = node
            .install_governance_dag_mirror_read_handle(mismatched_reader)
            .expect_err("a reader for another producer root must not install");
        assert!(error.to_string().contains("does not match"));
        assert!(
            node.governance_dag_mirror_snapshot()
                .expect("failed installation leaves the shared slot readable")
                .is_none(),
            "failed preflight must not consume or populate the installation slot"
        );

        let reader = GovernanceDagMirrorReadHandleV1::try_new(
            &service_config,
            test_checkpoint_store(checkpoint_provider.clone()),
            true,
        )
        .expect("construct reader for the node's retained producer root");
        node.install_governance_dag_mirror_read_handle(reader.clone())
            .expect("install the authenticated mirror reader exactly once");
        assert!(
            node.governance_dag_mirror_snapshot()
                .expect("node reads authenticated bootstrap mirror state")
                .is_none()
        );
        assert!(
            clone_created_before_install
                .governance_dag_mirror_snapshot()
                .expect("preexisting clone observes the installed reader")
                .is_none()
        );

        let mut checkpoint = checkpoint_from_source(&source);
        let mirror = mirror_index_value(
            &source,
            &checkpoint.mirror_blocks,
            checkpoint.generation,
            &checkpoint.head_ipfs_cid,
            checkpoint.published_at_unix,
        )
        .expect("build checkpoint-coherent mirror");
        let canonical_bytes = json::to_json_pretty(&mirror)
            .expect("encode checkpoint-coherent mirror")
            .into_bytes();
        checkpoint.mirror_blake3 = blake3_array(&canonical_bytes);
        let mirror_store =
            open_mirror_index_store(&service_config).expect("reopen typed mirror writer");
        let (empty_snapshot, empty_payload) =
            load_mirror_index_store(&service_config, &mirror_store).expect("load bootstrap mirror");
        assert!(empty_payload.is_empty());
        let committed_payload = MirrorIndexStorePayloadV1::committed(
            checkpoint.generation,
            [0; 32],
            canonical_bytes.clone(),
        )
        .expect("construct checkpoint-coherent typed mirror payload");
        compare_and_swap_mirror_index_store(
            &service_config,
            &mirror_store,
            &empty_snapshot,
            &committed_payload,
        )
        .expect("commit typed mirror payload");
        save_checkpoint(
            &test_checkpoint_store(checkpoint_provider),
            None,
            &checkpoint,
        )
        .expect("seal matching service checkpoint");
        reader.mark_ready();

        let node_snapshot = node
            .governance_dag_mirror_snapshot()
            .expect("node reads the installed checkpoint-coherent mirror")
            .expect("checkpointed mirror is available");
        assert_eq!(node_snapshot.canonical_bytes(), canonical_bytes);
        let clone_snapshot = clone_created_before_install
            .governance_dag_mirror_snapshot()
            .expect("preexisting clone reads the checkpoint-coherent mirror")
            .expect("checkpointed mirror is visible across clones");
        assert_eq!(clone_snapshot.canonical_bytes(), canonical_bytes);

        let error = clone_created_before_install
            .install_governance_dag_mirror_read_handle(reader)
            .expect_err("the shared installation slot must reject a second reader");
        assert!(error.to_string().contains("already installed"));
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

    #[test]
    fn publish_intent_progress_is_monotonic_within_one_generation() {
        let source = signed_source(2, 0x3b, 1_800_000_000);
        let mut prepared = intent_from_source(&source);
        for block in &mut prepared.blocks {
            block.ipfs_cid = None;
        }
        prepared.head_ipfs_cid = None;
        let mut progressed = prepared.clone();
        progressed.blocks[0].ipfs_cid = Some(
            canonical_ipfs_file_cid(&source.blocks[0].bytes)
                .expect("bounded source block has a deterministic IPFS file CID"),
        );
        assert!(publish_intent_is_monotonic_refinement(
            &prepared,
            &progressed
        ));

        let mut regressed = progressed.clone();
        regressed.blocks[0].ipfs_cid = None;
        assert!(!publish_intent_is_monotonic_refinement(
            &progressed,
            &regressed
        ));

        let mut equivocated = progressed.clone();
        equivocated.target_head_blake3[0] ^= 0x80;
        assert!(!publish_intent_is_monotonic_refinement(
            &progressed,
            &equivocated
        ));
    }

    #[test]
    fn checkpoint_and_intent_sequence_validation_rejects_u64_exhaustion() {
        let source = signed_source(2, 0x3c, 1_800_000_000);

        let mut checkpoint = checkpoint_from_source(&source);
        checkpoint.mirror_blocks[0].sequence = u64::MAX;
        checkpoint.mirror_blocks[1].sequence = 0;
        let checkpoint_error = validate_checkpoint_body(&checkpoint)
            .expect_err("checkpoint sequence exhaustion must fail closed");
        assert!(matches!(
            checkpoint_error,
            GovernanceDagServiceError::State(_)
        ));

        let mut intent = intent_from_source(&source);
        intent.blocks[0].sequence = u64::MAX;
        intent.blocks[1].sequence = 0;
        let intent_error = validate_publish_intent(&intent)
            .expect_err("publish-intent sequence exhaustion must fail closed");
        assert!(matches!(intent_error, GovernanceDagServiceError::State(_)));
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
                validation_failure_total: 31,
                mirror_drift: 37,
            },
            ..ApiSnapshot::default()
        })));
        let response = metrics_response(&state).await;
        let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("read metrics body");
        let body = std::str::from_utf8(&body).expect("metrics are UTF-8");
        for expected in [
            "result=\"success\"} 2",
            "result=\"failure\"} 3",
            "published_bytes_total{sink=\"ipfs\"} 5",
            "validation_failure_total 31",
            "mirror_drift 37",
            "blocks{payload_kind=\"deal_settlement\"} 2",
        ] {
            assert!(body.contains(expected), "missing metric row: {expected}");
        }
    }

    #[tokio::test]
    async fn failed_object_repair_latches_mirror_drift_until_coherent_retry() {
        #[derive(Default)]
        struct RecoveryHttpInner {
            objects: BTreeMap<String, Vec<u8>>,
            required_cids: BTreeSet<String>,
            head: Option<Vec<u8>>,
            head_generation: u64,
            early_head_put: bool,
            reject_add: bool,
            add_count: u64,
        }

        type RecoveryHttpState = Arc<Mutex<RecoveryHttpInner>>;

        fn query_arg(raw: Option<&str>) -> Option<&str> {
            raw?.split('&').find_map(|pair| {
                let (key, value) = pair.split_once('=')?;
                (key == "arg").then_some(value)
            })
        }

        async fn add(
            State(state): State<RecoveryHttpState>,
            headers: HeaderMap,
            body: Bytes,
        ) -> Response {
            let Some(boundary) = headers
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("multipart/form-data; boundary="))
            else {
                return test_response(StatusCode::BAD_REQUEST, Body::empty());
            };
            let Some(payload_start) = body
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .and_then(|position| position.checked_add(4))
            else {
                return test_response(StatusCode::BAD_REQUEST, Body::empty());
            };
            let suffix = format!("\r\n--{boundary}--\r\n");
            if !body.ends_with(suffix.as_bytes()) || payload_start > body.len() - suffix.len() {
                return test_response(StatusCode::BAD_REQUEST, Body::empty());
            }
            let payload = body[payload_start..body.len() - suffix.len()].to_vec();
            let Some(cid) = canonical_ipfs_file_cid(&payload) else {
                return test_response(StatusCode::BAD_REQUEST, Body::empty());
            };
            let mut state = state.lock().await;
            if state.reject_add {
                return test_response(StatusCode::SERVICE_UNAVAILABLE, Body::empty());
            }
            state.objects.insert(cid.clone(), payload);
            state.add_count = state.add_count.saturating_add(1);
            test_response(StatusCode::OK, format!(r#"{{"Hash":"{cid}"}}"#))
        }

        async fn pin_add() -> Response {
            test_response(StatusCode::OK, "{}")
        }

        async fn pin_ls(
            State(state): State<RecoveryHttpState>,
            axum::extract::RawQuery(raw): axum::extract::RawQuery,
        ) -> Response {
            let Some(cid) = query_arg(raw.as_deref()) else {
                return test_response(StatusCode::BAD_REQUEST, Body::empty());
            };
            let present = state.lock().await.objects.contains_key(cid);
            let body = if present {
                format!(r#"{{"Keys":{{"{cid}":{{}}}}}}"#)
            } else {
                r#"{"Keys":{}}"#.to_owned()
            };
            test_response(StatusCode::OK, body)
        }

        async fn cat(
            State(state): State<RecoveryHttpState>,
            axum::extract::RawQuery(raw): axum::extract::RawQuery,
        ) -> Response {
            let Some(cid) = query_arg(raw.as_deref()) else {
                return test_response(StatusCode::BAD_REQUEST, Body::empty());
            };
            state.lock().await.objects.get(cid).cloned().map_or_else(
                || test_response(StatusCode::NOT_FOUND, Body::empty()),
                |bytes| test_response(StatusCode::OK, bytes),
            )
        }

        async fn head_get(State(state): State<RecoveryHttpState>) -> Response {
            let state = state.lock().await;
            let Some(bytes) = &state.head else {
                return test_response(StatusCode::NOT_FOUND, Body::empty());
            };
            let mut response = test_response(StatusCode::OK, bytes.clone());
            response.headers_mut().insert(
                header::ETAG,
                HeaderValue::from_str(&format!("\"{}\"", state.head_generation))
                    .expect("canonical recovery ETag"),
            );
            response
        }

        async fn head_put(
            State(state): State<RecoveryHttpState>,
            headers: HeaderMap,
            body: Bytes,
        ) -> Response {
            let mut state = state.lock().await;
            if !state
                .required_cids
                .iter()
                .all(|cid| state.objects.contains_key(cid))
            {
                state.early_head_put = true;
                return test_response(StatusCode::INTERNAL_SERVER_ERROR, Body::empty());
            }
            if state.head.is_some()
                || headers.get(header::IF_NONE_MATCH) != Some(&HeaderValue::from_static("*"))
            {
                return test_response(StatusCode::PRECONDITION_FAILED, Body::empty());
            }
            state.head = Some(body.to_vec());
            state.head_generation = state.head_generation.saturating_add(1);
            test_response(StatusCode::NO_CONTENT, Body::empty())
        }

        let http_state = RecoveryHttpState::default();
        let router = Router::new()
            .route("/api/v0/add", post(add))
            .route("/api/v0/pin/add", post(pin_add))
            .route("/api/v0/pin/ls", post(pin_ls))
            .route("/api/v0/cat", post(cat))
            .route("/head", get(head_get).put(head_put))
            .with_state(Arc::clone(&http_state));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind recovery publication fixture");
        let address = listener.local_addr().expect("recovery fixture address");
        let http_task = tokio::spawn(async move {
            let _ = axum::serve(listener, router.into_make_service()).await;
        });

        let work = secure_temp_dir();
        let source_dir = work.path().join("source");
        let state_dir = work.path().join("state");
        let mut source = signed_source(2, 0x74, current_unix_timestamp_seconds().saturating_sub(5));
        materialize_source_snapshot(&source_dir, &mut source);
        let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        seed_producer_checkpoint(&checkpoint_provider, &source_dir, &source);
        let intent = intent_from_source(&source);
        let required_cids = intent
            .blocks
            .iter()
            .filter_map(|block| block.ipfs_cid.clone())
            .chain(intent.head_ipfs_cid.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(required_cids.len(), source.blocks.len() + 1);
        save_publish_intent(
            &test_checkpoint_store(Arc::clone(&checkpoint_provider)),
            None,
            &intent,
        )
        .expect("seal a crash-resumed intent with every CID already filled");
        http_state.lock().await.required_cids = required_cids.clone();

        let base_url = format!("http://{address}");
        let signed_head_url = format!("{base_url}/head");
        let view = real_kubo_service_view(
            &source,
            &source_dir,
            &state_dir,
            &base_url,
            &signed_head_url,
        );
        let mut service = Service::from_view(
            view.clone(),
            test_runtime_providers(&view, Arc::clone(&checkpoint_provider)),
        )
        .await
        .expect("construct crash-recovery service");

        http_state.lock().await.reject_add = true;
        service
            .reconcile_once()
            .await
            .expect_err("failed object repair must withdraw mirror readiness");
        {
            let api = service.api.0.read().await;
            assert!(!api.ready);
            assert_eq!(
                api.metrics.mirror_drift, 1,
                "a failed reconciliation must latch observable mirror drift"
            );
        }
        http_state.lock().await.reject_add = false;
        service
            .reconcile_once()
            .await
            .expect("repair every prefilled object before the public-head CAS");
        {
            let api = service.api.0.read().await;
            assert!(api.ready);
            assert_eq!(
                api.metrics.mirror_drift, 0,
                "only a checkpoint-coherent successful reconciliation clears mirror drift"
            );
        }

        let state = http_state.lock().await;
        assert!(
            !state.early_head_put,
            "public head crossed CAS before repair"
        );
        assert_eq!(state.add_count as usize, required_cids.len());
        assert!(
            required_cids
                .iter()
                .all(|cid| state.objects.contains_key(cid))
        );
        assert_eq!(state.head.as_deref(), Some(source.head_bytes.as_slice()));
        drop(state);
        assert!(service.checkpoint.is_some());
        assert!(service.intent.is_none());
        http_task.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires SORAFS_RUN_KUBO_INTEGRATION=1 and a local Kubo binary"]
    async fn real_kubo_publication_signed_head_restart_and_tamper_lane() {
        let kubo = KuboHarness::start().await;
        let endpoint = kubo.endpoint();
        assert_kubo_has_no_swarm_peers(&endpoint).await;
        let (head_endpoint, head_state, head_task) =
            spawn_signed_head(SignedHeadInner::default()).await;
        let signed_head_url = head_endpoint.url.to_string();

        let mut over_chunk_conformance = None;
        for (label, size) in [
            ("below-chunk", IPFS_UNIXFS_CHUNK_BYTES - 1),
            ("at-chunk", IPFS_UNIXFS_CHUNK_BYTES),
            ("over-chunk", IPFS_UNIXFS_CHUNK_BYTES + 1),
            ("max-object", GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1),
        ] {
            let payload = ipip_499_chacha20_bytes(label.as_bytes(), size);
            let expected_cid = canonical_ipfs_file_cid(&payload)
                .expect("Kubo conformance object fits the fixed UnixFS profile");
            let cid = ipfs_add_verified(
                &endpoint,
                &format!("fixed-unixfs-{label}.to"),
                &payload,
                payload.len() as u64,
                64 * 1024,
            )
            .await
            .unwrap_or_else(|err| panic!("real Kubo rejected {label} conformance vector: {err}"));
            assert_eq!(
                cid, expected_cid,
                "local UnixFS derivation diverged from Kubo for {label}"
            );
            if label == "over-chunk" {
                over_chunk_conformance = Some((payload, cid));
            }
        }
        let (direct_payload, direct_cid) =
            over_chunk_conformance.expect("the over-chunk conformance case ran");
        assert!(is_canonical_cid_v1(&direct_cid));
        assert_eq!(
            ipfs_cat(
                &endpoint,
                &direct_cid,
                direct_payload.len() as u64,
                64 * 1024
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
                64 * 1024,
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
        let view = real_kubo_service_view(
            &source,
            &source_dir,
            &state_dir,
            &kubo.api_url,
            &signed_head_url,
        );

        let mut service = Service::from_view(
            view.clone(),
            test_runtime_providers(&view, checkpoint_store.clone()),
        )
        .await
        .expect("initialize G-DAG service against real Kubo");
        service
            .reconcile_once()
            .await
            .expect("publish verified source through real Kubo and signed-head CAS");
        let checkpoint = service
            .checkpoint
            .clone()
            .expect("first reconciliation persists checkpoint");
        assert_eq!(checkpoint.block_count, source.blocks.len() as u64);
        assert_eq!(checkpoint.mirror_blocks.len(), source.blocks.len());
        assert!(state_dir.join(MIRROR_INDEX_STORE_NAME).is_dir());
        assert!(
            !state_dir.join("mirror-index.json").exists(),
            "the first-release service must not dual-write the retired mirror file"
        );
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
        let public = service
            .fetch_public_head()
            .await
            .expect("fetch published signed HTTP head");
        assert!(matches!(
            &public,
            PublicHead::Present { bytes, token }
                if bytes == &source.head_bytes && strong_http_entity_tag(
                    &HeaderValue::from_str(token).expect("signed-head ETag remains a header value")
                ).as_deref() == Some(token.as_str())
        ));

        let (mirror_snapshot, mirror_payload) =
            load_mirror_index_store(&service.config, &service.mirror_store)
                .expect("load published mirror payload");
        assert_eq!(mirror_payload.checkpoint_generation, checkpoint.generation);
        compare_and_swap_mirror_index_store(
            &service.config,
            &service.mirror_store,
            &mirror_snapshot,
            &MirrorIndexStorePayloadV1::empty(),
        )
        .expect("represent a hard-cut deployment without a local mirror payload");
        service
            .reconcile_once()
            .await
            .expect("steady-state reconciliation rebuilds an empty mirror store");
        let (_, recovered_payload) =
            load_mirror_index_store(&service.config, &service.mirror_store)
                .expect("load recovered mirror payload");
        assert_eq!(
            recovered_payload.checkpoint_generation,
            checkpoint.generation
        );
        assert_eq!(recovered_payload.mirror_blake3, checkpoint.mirror_blake3);

        kubo_unpin(&service.ipfs, &checkpoint.head_ipfs_cid).await;
        service
            .reconcile_once()
            .await
            .expect("steady state deterministically repairs a missing real Kubo head pin");
        ipfs_verify_pin(&service.ipfs, &checkpoint.head_ipfs_cid, 1024 * 1024)
            .await
            .expect("steady-state repair restores the recursive head pin");

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
        let mut restarted = Service::from_view(
            view.clone(),
            test_runtime_providers(&view, checkpoint_store),
        )
        .await
        .expect("restart G-DAG service from durable state");
        restarted
            .reconcile_once()
            .await
            .expect("restart verifies checkpoint, signed head, pins, and readback");
        assert_eq!(
            restarted
                .checkpoint
                .as_ref()
                .expect("restart loaded checkpoint")
                .generation,
            checkpoint.generation
        );
        assert!(restarted.api.0.read().await.ready);

        let attacker_bytes = b"concurrent-authorized-but-unexpected-signed-head";
        {
            let mut state = head_state.0.lock().await;
            state.bytes = Some(attacker_bytes.to_vec());
            state.etag = "\"attacker\"".to_owned();
        }
        let moved = restarted
            .reconcile_once()
            .await
            .expect_err("checkpoint reconciliation must reject unexpected signed-head movement");
        assert!(matches!(moved, GovernanceDagServiceError::Conflict(_)));

        {
            let mut state = head_state.0.lock().await;
            state.bytes = Some(source.head_bytes.clone());
            state.etag = "\"restored\"".to_owned();
        }
        restarted
            .reconcile_once()
            .await
            .expect("restored signed head returns service to steady state");

        eprintln!(
            "real Kubo G-DAG lane passed: direct_cid={direct_cid} head_cid={}",
            checkpoint.head_ipfs_cid
        );
        drop(restarted);
        head_task.abort();
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
