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
    GovernanceDagRequestAuthenticationEnvelopeV1, GovernanceDagRequestAuthenticationReplayStoreV1,
    GovernanceDagRequestAuthenticator, GovernanceDagRequestIngressBindingV1,
    GovernanceDagRequestIngressQualificationV1, GovernanceDagRuntimeProviderQualificationV1,
    GovernanceDagSealedCheckpointStore, GovernanceDagSealedStateRecord,
    GovernanceDagSealedStateSlot,
    governance::{
        GOVERNANCE_DAG_LOGICAL_ROOT as RUNTIME_INDEX_LOGICAL_ROOT,
        GOVERNANCE_DAG_REQUEST_AUTH_MAX_ENVELOPE_LIFETIME_SECS_V1,
        GOVERNANCE_DAG_REQUEST_AUTH_MAX_FUTURE_SKEW_SECS_V1,
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
    governance_dag_sealed_state_payload_max_bytes_v1,
    governance_rooted_fs::{
        FileBinding, FileSnapshot, RetainedFile, TwoSlotSnapshotV1, TwoSlotStoreConfigV1,
        TwoSlotStoreV1,
    },
};
use axum::{
    Router,
    body::Body,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode, Version, header},
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
const REQUEST_AUTH_REPLAY_STATE_VERSION_V1: u8 = 1;
const BLOCK_PREFIX_ARCHIVE_VERSION_V1: u8 = 1;
const BLOCK_PREFIX_ARCHIVE_MAX_ENTRIES_V1: usize = 1024;
const BLOCK_PREFIX_ARCHIVE_CANONICAL_OVERHEAD_BYTES_V1: usize = 1024 * 1024;
const BLOCK_PREFIX_ARCHIVE_MULTIPART_OVERHEAD_BYTES_V1: usize = 64 * 1024;
const BLOCK_PREFIX_ARCHIVE_MAX_CANONICAL_BYTES_V1: usize =
    match GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1
        .checked_add(BLOCK_PREFIX_ARCHIVE_CANONICAL_OVERHEAD_BYTES_V1)
    {
        Some(limit) => limit,
        None => panic!("Governance DAG block-prefix archive byte ceiling overflow"),
    };
const BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1: usize =
    match BLOCK_PREFIX_ARCHIVE_MAX_CANONICAL_BYTES_V1
        .checked_add(BLOCK_PREFIX_ARCHIVE_MULTIPART_OVERHEAD_BYTES_V1)
    {
        Some(limit) => limit,
        None => panic!("Governance DAG block-prefix archive request ceiling overflow"),
    };
const BLOCK_PREFIX_ARCHIVE_CANONICAL_URL_MAX_BYTES_V1: usize = 4096;
const BLOCK_PREFIX_ARCHIVE_PAYLOAD_KIND_MAX_BYTES_V1: usize = 128;
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
    head_authenticator_handle: Option<String>,
    head_authenticator_qualification: Option<GovernanceDagRuntimeProviderQualificationV1>,
    head_request_ingress_binding: Option<GovernanceDagRequestIngressBindingV1>,
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

    /// Stable handle for signed-head compare-and-swap authentication, when active.
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

    /// Exact configured signed-head endpoint, key, request-size, and timing policy, when active.
    #[must_use]
    pub const fn head_request_ingress_binding(
        &self,
    ) -> Option<GovernanceDagRequestIngressBindingV1> {
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
    authentication_scope: GovernanceDagAuthenticationScope,
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
            .field("authentication_scope", &self.authentication_scope)
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
        authentication_scope: GovernanceDagAuthenticationScope,
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
            authentication_scope,
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
            self.authentication_scope,
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

#[derive(Debug)]
struct SealedRequestAuthReplayStore<'a> {
    store: &'a OpaqueCheckpointStore,
    scope: GovernanceDagAuthenticationScope,
}

impl GovernanceDagRequestAuthenticationReplayStoreV1 for SealedRequestAuthReplayStore<'_> {
    fn consume_nonce(
        &mut self,
        nonce: [u8; 32],
        expires_at_unix_secs: u64,
        now_unix_secs: u64,
    ) -> Result<(), GovernanceDagRequestAuthenticationErrorV1> {
        match consume_sealed_request_auth_nonce(
            self.store,
            request_auth_replay_slot(self.scope),
            nonce,
            expires_at_unix_secs,
            now_unix_secs,
        ) {
            Ok(()) => Ok(()),
            Err(GovernanceDagServiceError::Network(message))
                if message == GovernanceDagRequestAuthenticationErrorV1::Replay.to_string() =>
            {
                Err(GovernanceDagRequestAuthenticationErrorV1::Replay)
            }
            Err(GovernanceDagServiceError::Network(message))
                if message
                    == GovernanceDagRequestAuthenticationErrorV1::ReplayCacheFull.to_string() =>
            {
                Err(GovernanceDagRequestAuthenticationErrorV1::ReplayCacheFull)
            }
            Err(_) => Err(GovernanceDagRequestAuthenticationErrorV1::ReplayStoreUnavailable),
        }
    }
}

/// Qualified receiver boundary for authenticated Governance DAG HTTP ingress.
///
/// The receiver retains only public endpoint policy and one opaque, qualified
/// sealed-store adapter. It canonicalizes and verifies a complete request with
/// the shared V1 HTTP receiver, then uses the scope-specific sealed CAS slot as
/// the sole replay authority. A fresh process-local cache is created for each
/// call only because the shared signature verifier requires one; it is never
/// consulted across calls and cannot authorize backend dispatch.
///
/// Independently administered Kubo/IPFS/IPNS and signed-head frontends can
/// reuse this type at their last pre-dispatch boundary. Constructing it does
/// not install, package, or supervise such a frontend.
#[derive(Clone)]
pub struct GovernanceDagSealedHttpRequestReceiverV1 {
    scope: GovernanceDagAuthenticationScope,
    max_body_bytes: u64,
    verification_policy: GovernanceDagRequestAuthenticationPolicyV1,
    replay_store: OpaqueCheckpointStore,
}

impl fmt::Debug for GovernanceDagSealedHttpRequestReceiverV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernanceDagSealedHttpRequestReceiverV1")
            .field("scope", &self.scope)
            .field("max_body_bytes", &self.max_body_bytes)
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
            .field("sealed_store_handle", &self.replay_store.handle)
            .finish_non_exhaustive()
    }
}

impl GovernanceDagSealedHttpRequestReceiverV1 {
    /// Bind public request policy to one exact production sealed-store adapter.
    ///
    /// Constructor inputs are limited to an endpoint scope, public request
    /// bounds and key policy, a stable credential-free store handle, its public
    /// qualification, and the opaque store adapter. Credentials and private
    /// keys do not cross this boundary.
    ///
    /// # Errors
    ///
    /// Rejects a zero body bound or a missing, substituted, stale,
    /// test-marked, unavailable, or ambiguously qualified sealed-store adapter.
    pub fn try_new(
        scope: GovernanceDagAuthenticationScope,
        max_body_bytes: u64,
        verification_policy: GovernanceDagRequestAuthenticationPolicyV1,
        checkpoint_store_handle: &str,
        checkpoint_store_qualification: GovernanceDagRuntimeProviderQualificationV1,
        checkpoint_store: Option<Arc<dyn GovernanceDagSealedCheckpointStore>>,
    ) -> Result<Self, GovernanceDagServiceError> {
        if max_body_bytes == 0 {
            return Err(GovernanceDagServiceError::Config(
                "Governance DAG ingress request body bound must be non-zero".to_owned(),
            ));
        }
        let replay_store = OpaqueCheckpointStore::try_new(
            checkpoint_store_handle,
            checkpoint_store_qualification,
            checkpoint_store.ok_or_else(|| {
                GovernanceDagServiceError::Config(
                    "Governance DAG ingress sealed replay store was not injected".to_owned(),
                )
            })?,
        )?;
        Ok(Self {
            scope,
            max_body_bytes,
            verification_policy,
            replay_store,
        })
    }

    /// Authenticate and durably consume one complete inbound HTTP request.
    ///
    /// The exact method, canonical URL, selected public headers, framing, and
    /// byte body are verified before any sealed state is touched. A descriptor
    /// is returned only after the nonce has committed through strict monotonic
    /// compare-and-swap and passed qualified post-CAS readback.
    ///
    /// # Errors
    ///
    /// Returns a payload-free request or sealed-state rejection. Every request
    /// failure preceding replay consumption leaves sealed state unchanged;
    /// store conflict, drift, corruption, rollback, or readback ambiguity fails
    /// closed without authorizing backend dispatch.
    pub fn verify_http_request<'h>(
        &self,
        method: &str,
        canonical_url: &str,
        headers: impl IntoIterator<Item = (&'h str, &'h [u8])>,
        body: &[u8],
        now_unix_secs: u64,
    ) -> Result<GovernanceDagCanonicalRequestV1, GovernanceDagServiceError> {
        let request_url = Url::parse(canonical_url).map_err(|_| {
            GovernanceDagServiceError::Network(
                GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest.to_string(),
            )
        })?;
        let mut endpoint = request_url.clone();
        endpoint.set_query(None);
        if self.scope == GovernanceDagAuthenticationScope::Ipfs {
            endpoint.set_path("/");
        }
        let endpoint_binding =
            governance_dag_request_ingress_endpoint_binding_v1(self.scope, endpoint.as_str())
                .map_err(|_| {
                    GovernanceDagServiceError::Network(
                        GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest.to_string(),
                    )
                })?;
        let binding = GovernanceDagRequestIngressBindingV1::try_new(
            self.scope,
            endpoint_binding,
            self.verification_policy.public_key(),
            self.max_body_bytes,
            self.verification_policy.max_envelope_lifetime_secs(),
            self.verification_policy.max_future_skew_secs(),
        )
        .map_err(|error| GovernanceDagServiceError::Network(error.to_string()))?;
        let origin = request_url.origin().ascii_serialization();
        let authority = origin
            .split_once("://")
            .map(|(_, value)| value)
            .ok_or_else(|| {
                GovernanceDagServiceError::Network(
                    GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest.to_string(),
                )
            })?;
        let target = match request_url.query() {
            Some(query) => format!("{}?{query}", request_url.path()),
            None => request_url.path().to_owned(),
        };
        let mut request = Request::builder()
            .method(method)
            .uri(target)
            .version(Version::HTTP_11)
            .header(header::HOST, authority)
            .body(body.to_vec())
            .map_err(|_| {
                GovernanceDagServiceError::Network(
                    GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest.to_string(),
                )
            })?;
        for (name, value) in headers {
            let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
                GovernanceDagServiceError::Network(
                    GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader.to_string(),
                )
            })?;
            let value = HeaderValue::from_bytes(value).map_err(|_| {
                GovernanceDagServiceError::Network(
                    GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader.to_string(),
                )
            })?;
            request.headers_mut().append(name, value);
        }
        let mut replay_store = SealedRequestAuthReplayStore {
            store: &self.replay_store,
            scope: self.scope,
        };
        let mut verifier = GovernanceDagHttpRequestReceiverV1::try_new(
            endpoint.as_str(),
            binding,
            &mut replay_store,
        )
        .map_err(|error| GovernanceDagServiceError::Network(error.to_string()))?;
        let verified = verifier
            .verify_http_request(request, now_unix_secs)
            .map_err(|error| GovernanceDagServiceError::Network(error.to_string()))?;
        Ok(verified.descriptor().clone())
    }

    /// Endpoint scope bound to this receiver.
    #[must_use]
    pub const fn scope(&self) -> GovernanceDagAuthenticationScope {
        self.scope
    }

    /// Maximum complete body size accepted by this receiver.
    #[must_use]
    pub const fn max_body_bytes(&self) -> u64 {
        self.max_body_bytes
    }

    /// Stable credential-free handle of the authoritative sealed replay store.
    #[must_use]
    pub fn checkpoint_store_handle(&self) -> &str {
        &self.replay_store.handle
    }

    /// Exact public qualification pinned for the sealed replay store.
    #[must_use]
    pub const fn checkpoint_store_qualification(
        &self,
    ) -> GovernanceDagRuntimeProviderQualificationV1 {
        self.replay_store.qualification
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
struct BlockPrefixArchivePublicationV1 {
    canonical_url: String,
    issued_at_unix_secs: u64,
    expires_at_unix_secs: u64,
    nonce: [u8; 32],
    request_digest: [u8; 32],
    public_key: [u8; 32],
    signature: [u8; 64],
}

impl BlockPrefixArchivePublicationV1 {
    fn from_envelope(
        envelope: &GovernanceDagRequestAuthenticationEnvelopeV1,
        descriptor: &GovernanceDagCanonicalRequestV1,
    ) -> Result<Self, GovernanceDagServiceError> {
        if envelope.scope() != GovernanceDagAuthenticationScope::Ipfs {
            return Err(GovernanceDagServiceError::State(
                "block-prefix archive publication used the wrong authentication scope".to_owned(),
            ));
        }
        Ok(Self {
            canonical_url: descriptor.canonical_url().to_owned(),
            issued_at_unix_secs: envelope.issued_at_unix_secs(),
            expires_at_unix_secs: envelope.expires_at_unix_secs(),
            nonce: envelope.nonce(),
            request_digest: envelope.request_digest(),
            public_key: envelope.public_key(),
            signature: envelope.signature(),
        })
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct BlockPrefixArchiveHeadV1 {
    generation: u64,
    digest: [u8; 32],
    ipfs_cid: String,
    archived_block_count: u64,
    last_block_cid: Vec<u8>,
    last_node_cid: Vec<u8>,
    predecessor_checkpoint_revision: [u8; 32],
    predecessor_checkpoint_digest: [u8; 32],
    predecessor_block_count: u64,
    predecessor_head_block_cid: Vec<u8>,
    publication: Option<BlockPrefixArchivePublicationV1>,
}

impl BlockPrefixArchiveHeadV1 {
    fn empty() -> Self {
        Self {
            generation: 0,
            digest: [0; 32],
            ipfs_cid: String::new(),
            archived_block_count: 0,
            last_block_cid: Vec::new(),
            last_node_cid: Vec::new(),
            predecessor_checkpoint_revision: [0; 32],
            predecessor_checkpoint_digest: [0; 32],
            predecessor_block_count: 0,
            predecessor_head_block_cid: Vec::new(),
            publication: None,
        }
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct SignedBlockPrefixArchiveEntryV1 {
    published: PublishedBlockV1,
    signed_block_bytes: Vec<u8>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct SignedBlockPrefixArchiveV1 {
    version: u8,
    archive_generation: u64,
    predecessor: BlockPrefixArchiveHeadV1,
    predecessor_checkpoint_revision: [u8; 32],
    predecessor_checkpoint_digest: [u8; 32],
    predecessor_block_count: u64,
    predecessor_head_block_cid: Vec<u8>,
    target_checkpoint_generation: u64,
    target_head_block_cid: Vec<u8>,
    target_block_count: u64,
    target_source_chain_blake3: [u8; 32],
    ipfs_authenticator_handle: String,
    ipfs_authenticator_revision: u64,
    ipfs_authenticator_policy_digest: [u8; 32],
    ipfs_authenticator_public_key: [u8; 32],
    checkpoint_store_handle: String,
    checkpoint_store_revision: u64,
    checkpoint_store_policy_digest: [u8; 32],
    archived_block_count: u64,
    blocks: Vec<SignedBlockPrefixArchiveEntryV1>,
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
    archive_head: BlockPrefixArchiveHeadV1,
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
    archive_head: BlockPrefixArchiveHeadV1,
    blocks: Vec<IntentBlockV1>,
    head_ipfs_cid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckpointCommitmentV1 {
    revision: [u8; 32],
    digest: [u8; 32],
    block_count: u64,
    head_block_cid: Vec<u8>,
}

impl CheckpointCommitmentV1 {
    fn empty() -> Self {
        Self {
            revision: [0; 32],
            digest: [0; 32],
            block_count: 0,
            head_block_cid: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct RequestAuthReplayEntryV1 {
    nonce: [u8; 32],
    expires_at_unix_secs: u64,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct RequestAuthReplayStateV1 {
    version: u8,
    entries: Vec<RequestAuthReplayEntryV1>,
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
    fn chain_blake3(&self) -> [u8; 32];
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

    fn chain_blake3(&self) -> [u8; 32] {
        self.chain_blake3
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

    fn chain_blake3(&self) -> [u8; 32] {
        self.chain_blake3
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

#[derive(Debug)]
enum HeadMode {
    SignedHttp(Box<PinnedEndpoint>),
    Ipns { name: String, key_name: String },
}

struct AuthenticatedResponse {
    response: reqwest::Response,
    authenticator: OpaqueAuthenticator,
    envelope: GovernanceDagRequestAuthenticationEnvelopeV1,
    descriptor: GovernanceDagCanonicalRequestV1,
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
    head_mode: HeadMode,
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
/// is unavailable or stale, or an unexpected signed-head provider is supplied
/// in IPNS mode.
pub fn validate_governance_dag_service_runtime_providers(
    view: &SorafsGovernanceDagServiceView,
    providers: &GovernanceDagServiceRuntimeProviders,
) -> Result<(), GovernanceDagServiceError> {
    let bindings = runtime_provider_bindings(view)?;
    let _checkpoint_store = OpaqueCheckpointStore::try_new(
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
        bindings.ipfs_authenticator_handle(),
        bindings.ipfs_authenticator_qualification(),
        bindings.ipfs_request_ingress_binding(),
        providers.ipfs_authenticator.clone().ok_or_else(|| {
            GovernanceDagServiceError::Config(
                "IPFS authentication is enabled but no runtime provider was injected".to_owned(),
            )
        })?,
        GovernanceDagAuthenticationScope::Ipfs,
        "IPFS authenticator",
    )?;
    match (
        bindings.head_authenticator_handle(),
        bindings.head_authenticator_qualification(),
        bindings.head_request_ingress_binding(),
        providers.head_authenticator.clone(),
    ) {
        (Some(handle), Some(qualification), Some(ingress_binding), Some(provider)) => {
            OpaqueAuthenticator::try_new(
                handle,
                qualification,
                ingress_binding,
                provider,
                GovernanceDagAuthenticationScope::SignedHead,
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
    let (head_authenticator_handle, head_authenticator_qualification, head_request_ingress_binding) =
        match service.head_mode.as_str() {
            "signed_http" => {
                if service.ipns_name.is_some() || service.ipns_key_name.is_some() {
                    return Err(GovernanceDagServiceError::Config(
                        "IPNS selectors must be absent in signed_http mode".to_owned(),
                    ));
                }
                let signed_head_url = service.signed_head_url.as_deref().ok_or_else(|| {
                    GovernanceDagServiceError::Config("signed head URL is missing".to_owned())
                })?;
                let public_key = service.head_request_auth_public_key.ok_or_else(|| {
                    GovernanceDagServiceError::Config(
                        "signed-head request-auth public key is missing".to_owned(),
                    )
                })?;
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
                    Some(configured_request_ingress_binding(
                        GovernanceDagAuthenticationScope::SignedHead,
                        signed_head_url,
                        public_key,
                        service.max_request_bytes.0,
                        service.request_auth_max_envelope_lifetime_secs,
                        service.request_auth_max_future_skew_secs,
                        "signed-head authenticator",
                    )?),
                )
            }
            "ipns" => {
                if service.signed_head_url.is_some()
                    || service.head_authenticator_handle.is_some()
                    || service.head_authenticator_revision.is_some()
                    || service.head_authenticator_policy_digest.is_some()
                    || service.head_request_auth_public_key.is_some()
                {
                    return Err(GovernanceDagServiceError::Config(
                        "signed-head URL and authenticator binding must be absent in IPNS mode"
                            .to_owned(),
                    ));
                }
                if service.ipns_name.is_none() || service.ipns_key_name.is_none() {
                    return Err(GovernanceDagServiceError::Config(
                        "IPNS name and key name are required in IPNS mode".to_owned(),
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
        let archive_request_max = u64::try_from(BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1)
            .map_err(|_| {
                GovernanceDagServiceError::Config(
                    "canonical Governance DAG block-prefix archive request ceiling exceeds host limits"
                        .to_owned(),
                )
            })?;
        if service.max_request_bytes.0 < archive_request_max {
            return Err(GovernanceDagServiceError::Config(format!(
                "max_request_bytes must be at least the canonical single-entry Governance DAG block-prefix archive request ceiling of {archive_request_max} bytes"
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
            GovernanceDagAuthenticationScope::Ipfs,
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
                if service.ipns_name.is_some() || service.ipns_key_name.is_some() {
                    return Err(GovernanceDagServiceError::Config(
                        "IPNS selectors must be absent in signed_http mode".to_owned(),
                    ));
                }
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
                let url = service.signed_head_url.clone().ok_or_else(|| {
                    GovernanceDagServiceError::Config("signed head URL is missing".to_owned())
                })?;
                let ingress_binding = configured_request_ingress_binding(
                    GovernanceDagAuthenticationScope::SignedHead,
                    &url,
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
                let authenticator = OpaqueAuthenticator::try_new(
                    handle,
                    qualification,
                    ingress_binding,
                    head_authenticator.ok_or_else(|| {
                        GovernanceDagServiceError::Config(
                            "signed-head authentication is enabled but no runtime provider was injected"
                                .to_owned(),
                        )
                    })?,
                    GovernanceDagAuthenticationScope::SignedHead,
                    "signed-head authenticator",
                )?;
                QualifiedHeadMode::SignedHttp { url, authenticator }
            }
            "ipns" => {
                if service.signed_head_url.is_some()
                    || service.head_authenticator_handle.is_some()
                    || service.head_authenticator_revision.is_some()
                    || service.head_authenticator_policy_digest.is_some()
                    || service.head_request_auth_public_key.is_some()
                    || head_authenticator.is_some()
                {
                    return Err(GovernanceDagServiceError::Config(
                        "signed-head URL, authenticator binding, and provider must be absent in IPNS mode"
                            .to_owned(),
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
        let head_mode = match qualified_head_mode {
            QualifiedHeadMode::SignedHttp { url, authenticator } => HeadMode::SignedHttp(Box::new(
                build_pinned_endpoint(
                    &url,
                    authenticator,
                    GovernanceDagAuthenticationScope::SignedHead,
                    &service,
                    false,
                )
                .await?,
            )),
            QualifiedHeadMode::Ipns { name, key_name } => HeadMode::Ipns { name, key_name },
        };
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
            head_mode,
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

const fn request_auth_replay_slot(
    scope: GovernanceDagAuthenticationScope,
) -> GovernanceDagSealedStateSlot {
    match scope {
        GovernanceDagAuthenticationScope::Ipfs => GovernanceDagSealedStateSlot::IpfsRequestReplay,
        GovernanceDagAuthenticationScope::SignedHead => {
            GovernanceDagSealedStateSlot::SignedHeadRequestReplay
        }
    }
}

fn decode_request_auth_replay_state(
    record: &GovernanceDagSealedStateRecord,
    slot: GovernanceDagSealedStateSlot,
    now_unix_secs: u64,
) -> Result<RequestAuthReplayStateV1, GovernanceDagServiceError> {
    let max_bytes = governance_dag_sealed_state_payload_max_bytes_v1(slot);
    if record.payload.len() > max_bytes {
        return Err(GovernanceDagServiceError::State(
            "sealed request-auth replay state exceeds its byte bound".to_owned(),
        ));
    }
    let state: RequestAuthReplayStateV1 = norito::decode_from_bytes_with_limits(
        &record.payload,
        request_auth_replay_decode_limits(max_bytes),
    )
    .map_err(|_| {
        GovernanceDagServiceError::State(
            "sealed request-auth replay state is not valid canonical Norito".to_owned(),
        )
    })?;
    let canonical = norito::to_bytes(&state).map_err(|_| {
        GovernanceDagServiceError::State(
            "sealed request-auth replay state could not be canonically encoded".to_owned(),
        )
    })?;
    if canonical != record.payload
        || state.version != REQUEST_AUTH_REPLAY_STATE_VERSION_V1
        || state.entries.len() > GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1
    {
        return Err(GovernanceDagServiceError::State(
            "sealed request-auth replay state is noncanonical or out of bounds".to_owned(),
        ));
    }
    let maximum_live_expiry = now_unix_secs
        .saturating_add(GOVERNANCE_DAG_REQUEST_AUTH_MAX_FUTURE_SKEW_SECS_V1)
        .saturating_add(GOVERNANCE_DAG_REQUEST_AUTH_MAX_ENVELOPE_LIFETIME_SECS_V1);
    let mut previous_nonce = None;
    for entry in &state.entries {
        if entry.nonce.iter().all(|byte| *byte == 0)
            || entry.expires_at_unix_secs == 0
            || previous_nonce.is_some_and(|previous| previous >= entry.nonce)
            || (entry.expires_at_unix_secs > now_unix_secs
                && entry.expires_at_unix_secs > maximum_live_expiry)
        {
            return Err(GovernanceDagServiceError::State(
                "sealed request-auth replay state contains invalid entries".to_owned(),
            ));
        }
        previous_nonce = Some(entry.nonce);
    }
    Ok(state)
}

fn consume_sealed_request_auth_nonce(
    store: &OpaqueCheckpointStore,
    slot: GovernanceDagSealedStateSlot,
    nonce: [u8; 32],
    expires_at_unix_secs: u64,
    now_unix_secs: u64,
) -> Result<(), GovernanceDagServiceError> {
    if !matches!(
        slot,
        GovernanceDagSealedStateSlot::IpfsRequestReplay
            | GovernanceDagSealedStateSlot::SignedHeadRequestReplay
    ) {
        return Err(GovernanceDagServiceError::State(
            "request-auth replay state selected a non-replay sealed slot".to_owned(),
        ));
    }
    if nonce.iter().all(|byte| *byte == 0) {
        return Err(GovernanceDagServiceError::Network(
            GovernanceDagRequestAuthenticationErrorV1::MalformedEnvelope.to_string(),
        ));
    }
    let maximum_expiry = now_unix_secs
        .saturating_add(GOVERNANCE_DAG_REQUEST_AUTH_MAX_FUTURE_SKEW_SECS_V1)
        .saturating_add(GOVERNANCE_DAG_REQUEST_AUTH_MAX_ENVELOPE_LIFETIME_SECS_V1);
    if expires_at_unix_secs <= now_unix_secs || expires_at_unix_secs > maximum_expiry {
        return Err(GovernanceDagServiceError::Network(
            GovernanceDagRequestAuthenticationErrorV1::InvalidTiming.to_string(),
        ));
    }
    let loaded = load_sealed_record(store, slot)?;
    let (mut state, expected_revision, next_generation) = match loaded.as_ref() {
        Some(record) => (
            decode_request_auth_replay_state(record, slot, now_unix_secs)?,
            Some(record.revision),
            record.generation.checked_add(1).ok_or_else(|| {
                GovernanceDagServiceError::State(
                    "sealed request-auth replay generation is exhausted".to_owned(),
                )
            })?,
        ),
        None => (
            RequestAuthReplayStateV1 {
                version: REQUEST_AUTH_REPLAY_STATE_VERSION_V1,
                entries: Vec::new(),
            },
            None,
            1,
        ),
    };
    state
        .entries
        .retain(|entry| entry.expires_at_unix_secs > now_unix_secs);
    let insertion_index = match state
        .entries
        .binary_search_by_key(&nonce, |entry| entry.nonce)
    {
        Ok(_) => {
            return Err(GovernanceDagServiceError::Network(
                GovernanceDagRequestAuthenticationErrorV1::Replay.to_string(),
            ));
        }
        Err(index) => index,
    };
    if state.entries.len() >= GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1 {
        return Err(GovernanceDagServiceError::Network(
            GovernanceDagRequestAuthenticationErrorV1::ReplayCacheFull.to_string(),
        ));
    }
    state.entries.insert(
        insertion_index,
        RequestAuthReplayEntryV1 {
            nonce,
            expires_at_unix_secs,
        },
    );
    let payload = norito::to_bytes(&state).map_err(|_| {
        GovernanceDagServiceError::State(
            "sealed request-auth replay state could not be canonically encoded".to_owned(),
        )
    })?;
    if payload.is_empty() || payload.len() > governance_dag_sealed_state_payload_max_bytes_v1(slot)
    {
        return Err(GovernanceDagServiceError::State(
            "sealed request-auth replay state exceeds its byte bound".to_owned(),
        ));
    }
    let next = GovernanceDagSealedStateRecord::new(slot, next_generation, payload);
    store.assert_identity()?;
    let result = store
        .provider
        .compare_and_swap(slot, expected_revision, next.clone());
    store.assert_identity()?;
    result.map_err(|_| {
        GovernanceDagServiceError::State(
            "sealed request-auth replay compare-and-swap failed".to_owned(),
        )
    })?;

    let observed = load_sealed_record(store, slot)?.ok_or_else(|| {
        GovernanceDagServiceError::State(
            "sealed request-auth replay state disappeared after compare-and-swap".to_owned(),
        )
    })?;
    let observed_state = decode_request_auth_replay_state(&observed, slot, now_unix_secs)?;
    let contains_exact_nonce = observed_state
        .entries
        .binary_search_by_key(&nonce, |entry| entry.nonce)
        .ok()
        .and_then(|index| observed_state.entries.get(index))
        .is_some_and(|entry| entry.expires_at_unix_secs == expires_at_unix_secs);
    if observed.generation < next.generation || !contains_exact_nonce {
        return Err(GovernanceDagServiceError::State(
            "sealed request-auth replay readback diverged".to_owned(),
        ));
    }
    Ok(())
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
    let value = mirror_json_value(payload)?;
    if payload.checkpoint_generation != checkpoint.generation
        || payload.mirror_blake3 != checkpoint.mirror_blake3
    {
        return Err(GovernanceDagServiceError::State(
            "mirror two-slot metadata does not match the authenticated checkpoint".to_owned(),
        ));
    }
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

fn request_auth_replay_decode_limits(max_bytes: usize) -> DecodeLimits {
    DecodeLimits::new(
        GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1,
        max_bytes,
        max_bytes.saturating_mul(8),
        max_bytes.saturating_mul(CANONICAL_DECODE_ALLOCATION_MULTIPLIER),
        128,
    )
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

fn checkpoint_commitment(
    checkpoint: Option<&CheckpointBodyV1>,
    revision: Option<[u8; 32]>,
) -> Result<CheckpointCommitmentV1, GovernanceDagServiceError> {
    match (checkpoint, revision) {
        (None, None) => Ok(CheckpointCommitmentV1::empty()),
        (Some(checkpoint), Some(revision)) => {
            validate_checkpoint_body(checkpoint)?;
            if revision == [0; 32] {
                return Err(GovernanceDagServiceError::State(
                    "checkpoint commitment revision is zero".to_owned(),
                ));
            }
            let bytes = norito::to_bytes(checkpoint).map_err(|err| {
                GovernanceDagServiceError::State(format!(
                    "checkpoint commitment encode failed: {err}"
                ))
            })?;
            let digest = blake3_array(&bytes);
            let sealed = GovernanceDagSealedStateRecord::new(
                GovernanceDagSealedStateSlot::Checkpoint,
                checkpoint.generation,
                bytes,
            );
            if sealed.revision != revision {
                return Err(GovernanceDagServiceError::State(
                    "checkpoint commitment revision does not bind the exact canonical body"
                        .to_owned(),
                ));
            }
            Ok(CheckpointCommitmentV1 {
                revision,
                digest,
                block_count: checkpoint.block_count,
                head_block_cid: checkpoint.head_block_cid.clone(),
            })
        }
        _ => Err(GovernanceDagServiceError::State(
            "checkpoint body and sealed revision presence disagree".to_owned(),
        )),
    }
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
    validate_block_prefix_archive_head(&body.archive_head)?;
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
        .first()
        .is_none_or(|block| block.sequence != body.archive_head.archived_block_count)
        || body
            .mirror_blocks
            .last()
            .and_then(|block| block.sequence.checked_add(1))
            != Some(body.block_count)
    {
        return Err(GovernanceDagServiceError::State(
            "checkpoint mirror and archive do not cover one exact block prefix".to_owned(),
        ));
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
        || block.payload_kind.len() > BLOCK_PREFIX_ARCHIVE_PAYLOAD_KIND_MAX_BYTES_V1
        || block.encoded_len == 0
        || !is_canonical_cid_v1(&block.ipfs_cid)
    {
        return Err(GovernanceDagServiceError::State(
            "published block fields violate first-release bounds".to_owned(),
        ));
    }
    Ok(())
}

fn validate_block_prefix_archive_publication_fields(
    publication: &BlockPrefixArchivePublicationV1,
) -> Result<(), GovernanceDagServiceError> {
    let lifetime = publication
        .expires_at_unix_secs
        .checked_sub(publication.issued_at_unix_secs)
        .ok_or_else(|| {
            GovernanceDagServiceError::State(
                "block-prefix archive publication timing is inverted".to_owned(),
            )
        })?;
    if publication.issued_at_unix_secs == 0
        || publication.canonical_url.is_empty()
        || publication.canonical_url.len() > BLOCK_PREFIX_ARCHIVE_CANONICAL_URL_MAX_BYTES_V1
        || Url::parse(&publication.canonical_url).is_err()
        || lifetime == 0
        || lifetime > GOVERNANCE_DAG_REQUEST_AUTH_MAX_ENVELOPE_LIFETIME_SECS_V1
        || publication.nonce.iter().all(|byte| *byte == 0)
        || publication.request_digest.iter().all(|byte| *byte == 0)
        || publication.public_key.iter().all(|byte| *byte == 0)
        || publication.signature.iter().all(|byte| *byte == 0)
        || PublicKey::from_bytes(Algorithm::Ed25519, &publication.public_key).is_err()
    {
        return Err(GovernanceDagServiceError::State(
            "block-prefix archive publication attestation is malformed".to_owned(),
        ));
    }
    Ok(())
}

fn validate_block_prefix_archive_head(
    head: &BlockPrefixArchiveHeadV1,
) -> Result<(), GovernanceDagServiceError> {
    if head.generation == 0 {
        if head.digest != [0; 32]
            || !head.ipfs_cid.is_empty()
            || head.archived_block_count != 0
            || !head.last_block_cid.is_empty()
            || !head.last_node_cid.is_empty()
            || head.predecessor_checkpoint_revision != [0; 32]
            || head.predecessor_checkpoint_digest != [0; 32]
            || head.predecessor_block_count != 0
            || !head.predecessor_head_block_cid.is_empty()
            || head.publication.is_some()
        {
            return Err(GovernanceDagServiceError::State(
                "empty block-prefix archive head is noncanonical".to_owned(),
            ));
        }
        return Ok(());
    }
    let predecessor_commitment_invalid = if head.predecessor_head_block_cid.is_empty() {
        head.predecessor_checkpoint_revision != [0; 32]
            || head.predecessor_checkpoint_digest != [0; 32]
            || head.predecessor_block_count != 0
    } else {
        head.predecessor_head_block_cid.len() != 32
            || head.predecessor_checkpoint_revision == [0; 32]
            || head.predecessor_checkpoint_digest == [0; 32]
            || head.predecessor_block_count == 0
    };
    if head.digest == [0; 32]
        || !is_canonical_cid_v1(&head.ipfs_cid)
        || head.archived_block_count == 0
        || head.generation > head.archived_block_count
        || head.last_block_cid.len() != 32
        || head.last_node_cid.len() != 32
        || predecessor_commitment_invalid
    {
        return Err(GovernanceDagServiceError::State(
            "block-prefix archive head violates first-release bounds".to_owned(),
        ));
    }
    validate_block_prefix_archive_publication_fields(head.publication.as_ref().ok_or_else(
        || {
            GovernanceDagServiceError::State(
                "block-prefix archive head has no authenticated publication".to_owned(),
            )
        },
    )?)
}

fn block_prefix_archive_decode_limits(max_bytes: usize) -> DecodeLimits {
    DecodeLimits::new(
        SOURCE_ENTRY_HARD_CAP,
        max_bytes,
        CANONICAL_DECODE_MAX_TOTAL_ELEMENTS,
        max_bytes.saturating_mul(CANONICAL_DECODE_ALLOCATION_MULTIPLIER),
        128,
    )
}

fn decode_signed_block_prefix_archive(
    bytes: &[u8],
    max_bytes: u64,
) -> Result<SignedBlockPrefixArchiveV1, GovernanceDagServiceError> {
    let max_bytes = usize::try_from(
        max_bytes
            .min(u64::try_from(BLOCK_PREFIX_ARCHIVE_MAX_CANONICAL_BYTES_V1).unwrap_or(u64::MAX)),
    )
    .unwrap_or(usize::MAX);
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err(GovernanceDagServiceError::State(
            "signed block-prefix archive exceeds its canonical byte bound".to_owned(),
        ));
    }
    let archive: SignedBlockPrefixArchiveV1 =
        norito::decode_from_bytes_with_limits(bytes, block_prefix_archive_decode_limits(max_bytes))
            .map_err(|err| {
                GovernanceDagServiceError::State(format!(
                    "signed block-prefix archive decode failed: {err}"
                ))
            })?;
    if norito::to_bytes(&archive)
        .map_err(|err| GovernanceDagServiceError::State(err.to_string()))?
        != bytes
    {
        return Err(GovernanceDagServiceError::State(
            "signed block-prefix archive encoding is not canonical".to_owned(),
        ));
    }
    validate_signed_block_prefix_archive(&archive)?;
    Ok(archive)
}

fn validate_signed_block_prefix_archive(
    archive: &SignedBlockPrefixArchiveV1,
) -> Result<(), GovernanceDagServiceError> {
    validate_block_prefix_archive_head(&archive.predecessor)?;
    let expected_generation = archive
        .predecessor
        .generation
        .checked_add(1)
        .ok_or_else(|| {
            GovernanceDagServiceError::State(
                "block-prefix archive generation is exhausted".to_owned(),
            )
        })?;
    let initial_checkpoint = archive.predecessor_head_block_cid.is_empty();
    let ipfs_qualification = GovernanceDagRuntimeProviderQualificationV1::new(
        archive.ipfs_authenticator_revision,
        archive.ipfs_authenticator_policy_digest,
    );
    let store_qualification = GovernanceDagRuntimeProviderQualificationV1::new(
        archive.checkpoint_store_revision,
        archive.checkpoint_store_policy_digest,
    );
    let predecessor_commitment_invalid = if initial_checkpoint {
        archive.predecessor_checkpoint_revision != [0; 32]
            || archive.predecessor_checkpoint_digest != [0; 32]
            || archive.predecessor_block_count != 0
    } else {
        archive.predecessor_head_block_cid.len() != 32
            || archive.predecessor_checkpoint_revision == [0; 32]
            || archive.predecessor_checkpoint_digest == [0; 32]
            || archive.predecessor_block_count == 0
    };
    if archive.version != BLOCK_PREFIX_ARCHIVE_VERSION_V1
        || archive.archive_generation != expected_generation
        || archive.target_checkpoint_generation == 0
        || archive.target_head_block_cid.len() != 32
        || archive.target_block_count == 0
        || archive.target_source_chain_blake3 == [0; 32]
        || archive.blocks.is_empty()
        || archive.blocks.len() > BLOCK_PREFIX_ARCHIVE_MAX_ENTRIES_V1
        || archive.archived_block_count == 0
        || predecessor_commitment_invalid
        || validate_production_runtime_handle(&archive.ipfs_authenticator_handle).is_err()
        || !ipfs_qualification.is_valid()
        || PublicKey::from_bytes(Algorithm::Ed25519, &archive.ipfs_authenticator_public_key)
            .is_err()
        || validate_production_runtime_handle(&archive.checkpoint_store_handle).is_err()
        || !store_qualification.is_valid()
    {
        return Err(GovernanceDagServiceError::State(
            "signed block-prefix archive fields violate first-release bounds".to_owned(),
        ));
    }
    let block_count = u64::try_from(archive.blocks.len()).map_err(|_| {
        GovernanceDagServiceError::State(
            "signed block-prefix archive entry count exceeds u64".to_owned(),
        )
    })?;
    let expected_first = archive
        .archived_block_count
        .checked_sub(block_count)
        .ok_or_else(|| {
            GovernanceDagServiceError::State(
                "signed block-prefix archive count underflows its entries".to_owned(),
            )
        })?;
    if expected_first != archive.predecessor.archived_block_count
        || archive.target_block_count < archive.archived_block_count
        || archive.target_block_count < archive.predecessor_block_count
    {
        return Err(GovernanceDagServiceError::State(
            "signed block-prefix archive is not the exact predecessor successor".to_owned(),
        ));
    }
    let mut previous_block: Option<GovernanceDagBlockV1> = None;
    for (position, entry) in archive.blocks.iter().enumerate() {
        validate_published_block(&entry.published)?;
        let expected_sequence = expected_first
            .checked_add(u64::try_from(position).map_err(|_| {
                GovernanceDagServiceError::State(
                    "signed block-prefix archive position exceeds u64".to_owned(),
                )
            })?)
            .ok_or_else(|| {
                GovernanceDagServiceError::State(
                    "signed block-prefix archive sequence is exhausted".to_owned(),
                )
            })?;
        if entry.published.sequence != expected_sequence
            || entry.signed_block_bytes.is_empty()
            || entry.signed_block_bytes.len() > GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1
            || entry.published.encoded_len
                != u64::try_from(entry.signed_block_bytes.len()).unwrap_or(u64::MAX)
            || entry.published.encoded_blake3 != blake3_array(&entry.signed_block_bytes)
            || entry.published.ipfs_cid != canonical_raw_sha256_cid(&entry.signed_block_bytes)
        {
            return Err(GovernanceDagServiceError::State(
                "signed block-prefix archive entry metadata is inconsistent".to_owned(),
            ));
        }
        let block: GovernanceDagBlockV1 = norito::decode_from_bytes_with_limits(
            &entry.signed_block_bytes,
            block_prefix_archive_decode_limits(GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1),
        )
        .map_err(|err| {
            GovernanceDagServiceError::State(format!(
                "signed block-prefix archive block decode failed: {err}"
            ))
        })?;
        if block
            .canonical_bytes()
            .map_err(|err| GovernanceDagServiceError::State(err.to_string()))?
            != entry.signed_block_bytes
            || block.validate().is_err()
            || block.sequence != entry.published.sequence
            || block.block_cid != entry.published.governance_block_cid
            || block.node.node_cid != entry.published.governance_node_cid
            || crate::governance::runtime_dag_payload_kind(&block.node.payload)
                != entry.published.payload_kind
            || block.timestamp != entry.published.timestamp
        {
            return Err(GovernanceDagServiceError::State(
                "signed block-prefix archive contains a substituted block".to_owned(),
            ));
        }
        let predecessor_parent_invalid = if archive.predecessor.archived_block_count == 0 {
            block.prev_block_cid.is_some() || block.node.prev_cid.is_some()
        } else {
            block.prev_block_cid.as_deref() != Some(archive.predecessor.last_block_cid.as_slice())
                || block.node.prev_cid.as_deref()
                    != Some(archive.predecessor.last_node_cid.as_slice())
        };
        if position == 0 && predecessor_parent_invalid {
            return Err(GovernanceDagServiceError::State(
                "signed block-prefix archive does not extend its exact predecessor block"
                    .to_owned(),
            ));
        }
        if let Some(previous) = &previous_block
            && (block.prev_block_cid.as_deref() != Some(previous.block_cid.as_slice())
                || block.node.prev_cid.as_deref() != Some(previous.node.node_cid.as_slice()))
        {
            return Err(GovernanceDagServiceError::State(
                "signed block-prefix archive contains a fork or gap".to_owned(),
            ));
        }
        previous_block = Some(block);
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
    validate_block_prefix_archive_head(&body.archive_head)?;
    if body.archive_head.archived_block_count > body.target_block_count {
        return Err(GovernanceDagServiceError::State(
            "publish intent archive is ahead of its target chain".to_owned(),
        ));
    }
    let mut previous: Option<u64> = None;
    let mut seen = BTreeSet::new();
    for block in &body.blocks {
        if block.governance_block_cid.len() != 32
            || block.governance_node_cid.len() != 32
            || block.payload_kind.is_empty()
            || block.payload_kind.len() > BLOCK_PREFIX_ARCHIVE_PAYLOAD_KIND_MAX_BYTES_V1
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
            envelope,
            descriptor,
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
        ..
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
    ipfs_add_verified_with_publication(endpoint, name, bytes, max_request_bytes, max_response_bytes)
        .await
        .map(|(cid, _publication)| cid)
}

async fn ipfs_add_verified_with_publication(
    endpoint: &PinnedEndpoint,
    name: &str,
    bytes: &[u8],
    max_request_bytes: u64,
    max_response_bytes: u64,
) -> Result<(String, BlockPrefixArchivePublicationV1), GovernanceDagServiceError> {
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
    let publication =
        BlockPrefixArchivePublicationV1::from_envelope(&response.envelope, &response.descriptor)?;
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
    Ok((cid, publication))
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

fn block_prefix_archive_lengths_fit(
    canonical_bytes: usize,
    multipart_bytes: usize,
    configured_request_bytes: u64,
) -> bool {
    canonical_bytes != 0
        && canonical_bytes <= BLOCK_PREFIX_ARCHIVE_MAX_CANONICAL_BYTES_V1
        && multipart_bytes <= BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1
        && u64::try_from(multipart_bytes)
            .is_ok_and(|multipart| multipart <= configured_request_bytes)
}

fn block_prefix_archive_filename(
    archive: &SignedBlockPrefixArchiveV1,
) -> Result<String, GovernanceDagServiceError> {
    let first = archive
        .blocks
        .first()
        .ok_or_else(|| {
            GovernanceDagServiceError::State(
                "signed block-prefix archive has no first entry".to_owned(),
            )
        })?
        .published
        .sequence;
    let last = archive
        .blocks
        .last()
        .ok_or_else(|| {
            GovernanceDagServiceError::State(
                "signed block-prefix archive has no last entry".to_owned(),
            )
        })?
        .published
        .sequence;
    Ok(format!(
        "governance-dag-prefix-{:020}-{first:020}-{last:020}.to",
        archive.archive_generation
    ))
}

#[cfg(test)]
fn block_prefix_archive_add_descriptor(
    endpoint: &PinnedEndpoint,
    archive: &SignedBlockPrefixArchiveV1,
    archive_bytes: &[u8],
) -> Result<GovernanceDagCanonicalRequestV1, GovernanceDagServiceError> {
    let url = endpoint.ipfs_url("api/v0/add", IPFS_UNIXFS_V1_ADD_QUERY)?;
    block_prefix_archive_add_descriptor_for_url(
        url.as_str(),
        archive,
        archive_bytes,
        endpoint.authenticated_wire_body_max_bytes,
    )
}

fn block_prefix_archive_add_descriptor_for_url(
    canonical_url: &str,
    archive: &SignedBlockPrefixArchiveV1,
    archive_bytes: &[u8],
    max_body_bytes: u64,
) -> Result<GovernanceDagCanonicalRequestV1, GovernanceDagServiceError> {
    let filename = block_prefix_archive_filename(archive)?;
    let (boundary, body) = canonical_ipfs_multipart_body(&filename, archive_bytes)?;
    if !block_prefix_archive_lengths_fit(archive_bytes.len(), body.len(), max_body_bytes) {
        return Err(GovernanceDagServiceError::State(
            "signed block-prefix archive exceeds its canonical or request byte ceiling".to_owned(),
        ));
    }
    let content_type = format!("multipart/form-data; boundary={boundary}");
    GovernanceDagCanonicalRequestV1::try_from_http_parts(
        GovernanceDagAuthenticationScope::Ipfs,
        Method::POST.as_str(),
        canonical_url,
        [
            (header::ACCEPT_ENCODING.as_str(), b"identity".as_slice()),
            (header::CONTENT_TYPE.as_str(), content_type.as_bytes()),
        ],
        &body,
        max_body_bytes,
    )
    .map_err(|_| {
        GovernanceDagServiceError::State(
            "signed block-prefix archive add descriptor is noncanonical".to_owned(),
        )
    })
}

fn verify_block_prefix_archive_publication(
    archive: &SignedBlockPrefixArchiveV1,
    archive_bytes: &[u8],
    head: &BlockPrefixArchiveHeadV1,
) -> Result<(), GovernanceDagServiceError> {
    validate_block_prefix_archive_head(head)?;
    let publication = head.publication.as_ref().ok_or_else(|| {
        GovernanceDagServiceError::State(
            "signed block-prefix archive publication is missing".to_owned(),
        )
    })?;
    let last = archive.blocks.last().ok_or_else(|| {
        GovernanceDagServiceError::State(
            "signed block-prefix archive publication has no last block".to_owned(),
        )
    })?;
    if head.generation != archive.archive_generation
        || head.digest != blake3_array(archive_bytes)
        || head.archived_block_count != archive.archived_block_count
        || head.last_block_cid != last.published.governance_block_cid
        || head.last_node_cid != last.published.governance_node_cid
        || head.predecessor_checkpoint_revision != archive.predecessor_checkpoint_revision
        || head.predecessor_checkpoint_digest != archive.predecessor_checkpoint_digest
        || head.predecessor_block_count != archive.predecessor_block_count
        || head.predecessor_head_block_cid != archive.predecessor_head_block_cid
        || publication.public_key != archive.ipfs_authenticator_public_key
    {
        return Err(GovernanceDagServiceError::State(
            "signed block-prefix archive provider or head binding is substituted".to_owned(),
        ));
    }
    let descriptor = block_prefix_archive_add_descriptor_for_url(
        &publication.canonical_url,
        archive,
        archive_bytes,
        u64::try_from(BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1).unwrap_or(u64::MAX),
    )?;
    if publication.request_digest != descriptor.request_digest()
        || publication
            .expires_at_unix_secs
            .checked_sub(publication.issued_at_unix_secs)
            .is_none_or(|lifetime| {
                lifetime == 0
                    || lifetime > GOVERNANCE_DAG_REQUEST_AUTH_MAX_ENVELOPE_LIFETIME_SECS_V1
            })
    {
        return Err(GovernanceDagServiceError::State(
            "signed block-prefix archive publication descriptor diverged".to_owned(),
        ));
    }
    let public_key =
        PublicKey::from_bytes(Algorithm::Ed25519, &publication.public_key).map_err(|_| {
            GovernanceDagServiceError::State(
                "signed block-prefix archive publication key is malformed".to_owned(),
            )
        })?;
    let signature =
        iroha_crypto::ed25519_parse_signature(&publication.signature).map_err(|_| {
            GovernanceDagServiceError::State(
                "signed block-prefix archive publication signature is malformed".to_owned(),
            )
        })?;
    let signing_payload = GovernanceDagRequestAuthenticationEnvelopeV1::signing_payload(
        &descriptor,
        publication.issued_at_unix_secs,
        publication.expires_at_unix_secs,
        publication.nonce,
        publication.public_key,
    );
    signature
        .verify(&public_key, &signing_payload)
        .map_err(|_| {
            GovernanceDagServiceError::State(
                "signed block-prefix archive publication signature is invalid".to_owned(),
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
        if let Some(checkpoint) = &self.checkpoint {
            self.verify_block_prefix_archive_head(
                &checkpoint.archive_head,
                checkpoint.generation,
                None,
                &source,
            )
            .await?;
        }
        if let Some(intent) = &self.intent {
            let _ = validate_intent_against_source(
                intent,
                self.checkpoint.as_ref(),
                self.checkpoint_revision,
                &source,
            )?;
            if self
                .checkpoint
                .as_ref()
                .is_none_or(|checkpoint| checkpoint.archive_head != intent.archive_head)
            {
                self.verify_block_prefix_archive_head(
                    &intent.archive_head,
                    intent.generation,
                    Some(intent.generation),
                    &source,
                )
                .await?;
            }
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

    async fn verify_block_prefix_archive_head(
        &self,
        expected: &BlockPrefixArchiveHeadV1,
        maximum_target_generation: u64,
        required_target_generation: Option<u64>,
        source: &SourceSnapshot,
    ) -> Result<(), GovernanceDagServiceError> {
        validate_block_prefix_archive_head(expected)?;
        if expected.generation == 0 {
            return Ok(());
        }
        let archive_max_bytes =
            u64::try_from(BLOCK_PREFIX_ARCHIVE_MAX_CANONICAL_BYTES_V1).unwrap_or(u64::MAX);
        self.checkpoint_store.assert_identity()?;
        self.ipfs.authenticator.assert_identity()?;

        // Fetch by the sealed content address before checking local pin state.
        // A newly promoted Kubo replica can therefore recover the authenticated
        // tail instead of failing solely because its local pinset is cold.
        let bytes = ipfs_cat(
            &self.ipfs,
            &expected.ipfs_cid,
            archive_max_bytes,
            archive_max_bytes,
        )
        .await?;
        if blake3_array(&bytes) != expected.digest
            || validate_ipfs_cid_for_bytes(&expected.ipfs_cid, &bytes).is_err()
        {
            return Err(GovernanceDagServiceError::State(
                "block-prefix archive readback digest or content address diverged".to_owned(),
            ));
        }
        let archive = decode_signed_block_prefix_archive(&bytes, archive_max_bytes)?;
        verify_block_prefix_archive_publication(&archive, &bytes, expected)?;
        validate_block_prefix_archive_against_source(&archive, source)?;
        if archive.target_checkpoint_generation > maximum_target_generation
            || required_target_generation
                .is_some_and(|required| archive.target_checkpoint_generation != required)
        {
            return Err(GovernanceDagServiceError::State(
                "block-prefix archive target generation is ahead of durable state".to_owned(),
            ));
        }

        ipfs_pin(
            &self.ipfs,
            &expected.ipfs_cid,
            self.config.max_response_bytes,
        )
        .await?;
        ipfs_verify_pin(
            &self.ipfs,
            &expected.ipfs_cid,
            self.config.max_response_bytes,
        )
        .await?;
        let pinned = ipfs_cat(
            &self.ipfs,
            &expected.ipfs_cid,
            u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            archive_max_bytes,
        )
        .await?;
        if pinned != bytes {
            return Err(GovernanceDagServiceError::State(
                "block-prefix archive changed after recovery pinning".to_owned(),
            ));
        }
        self.ipfs.authenticator.assert_identity()?;
        self.checkpoint_store.assert_identity()?;
        Ok(())
    }

    async fn archive_would_be_pruned_prefix<S: SourceChainView + ?Sized>(
        &mut self,
        intent: &mut PublishIntentBodyV1,
        source: &S,
        retained_blocks: &[PublishedBlockV1],
    ) -> Result<BlockPrefixArchiveHeadV1, GovernanceDagServiceError> {
        let retained_start = retained_blocks
            .first()
            .ok_or_else(|| {
                GovernanceDagServiceError::State(
                    "mirror retention produced an empty suffix".to_owned(),
                )
            })?
            .sequence;
        let mut predecessor = intent.archive_head.clone();
        validate_block_prefix_archive_head(&predecessor)?;
        if retained_start < predecessor.archived_block_count {
            return Err(GovernanceDagServiceError::State(
                "mirror retention would replay an archived block prefix".to_owned(),
            ));
        }
        if retained_start == predecessor.archived_block_count {
            return Ok(predecessor);
        }
        let by_sequence = published_blocks_by_sequence(self.checkpoint.as_ref(), intent)?;
        let predecessor_checkpoint =
            checkpoint_commitment(self.checkpoint.as_ref(), self.checkpoint_revision)?;
        let archive_max_bytes =
            u64::try_from(BLOCK_PREFIX_ARCHIVE_MAX_CANONICAL_BYTES_V1).unwrap_or(u64::MAX);
        let archive_max_bytes_usize = usize::try_from(archive_max_bytes).unwrap_or(usize::MAX);
        let archive_max_entries =
            u64::try_from(BLOCK_PREFIX_ARCHIVE_MAX_ENTRIES_V1).unwrap_or(u64::MAX);
        let mut start = predecessor.archived_block_count;
        while start < retained_start {
            let mut end = start;
            let mut estimated_bytes = 16 * 1024_usize;
            while end < retained_start && end.saturating_sub(start) < archive_max_entries {
                let position = usize::try_from(end).map_err(|_| {
                    GovernanceDagServiceError::State(
                        "block-prefix archive sequence exceeds host limits".to_owned(),
                    )
                })?;
                let source_block = source.blocks().get(position).ok_or_else(|| {
                    GovernanceDagServiceError::State(
                        "block-prefix archive source range is incomplete".to_owned(),
                    )
                })?;
                let published = by_sequence.get(&end).ok_or_else(|| {
                    GovernanceDagServiceError::State(
                        "would-be-pruned block has no authenticated IPFS mapping".to_owned(),
                    )
                })?;
                let next = estimated_bytes
                    .checked_add(estimated_block_prefix_archive_entry_bytes(
                        source_block,
                        published,
                    )?)
                    .ok_or_else(|| {
                        GovernanceDagServiceError::State(
                            "block-prefix archive size estimate overflowed".to_owned(),
                        )
                    })?;
                if next > archive_max_bytes_usize && end != start {
                    break;
                }
                estimated_bytes = next;
                end = end.checked_add(1).ok_or_else(|| {
                    GovernanceDagServiceError::State(
                        "block-prefix archive sequence is exhausted".to_owned(),
                    )
                })?;
            }
            if end == start {
                end = start.checked_add(1).ok_or_else(|| {
                    GovernanceDagServiceError::State(
                        "block-prefix archive sequence is exhausted".to_owned(),
                    )
                })?;
            }

            let (archive, archive_bytes, filename) = loop {
                let blocks = block_prefix_archive_entries(source, &by_sequence, start, end)?;
                let archive = SignedBlockPrefixArchiveV1 {
                    version: BLOCK_PREFIX_ARCHIVE_VERSION_V1,
                    archive_generation: predecessor.generation.checked_add(1).ok_or_else(|| {
                        GovernanceDagServiceError::State(
                            "block-prefix archive generation is exhausted".to_owned(),
                        )
                    })?,
                    predecessor: predecessor.clone(),
                    predecessor_checkpoint_revision: predecessor_checkpoint.revision,
                    predecessor_checkpoint_digest: predecessor_checkpoint.digest,
                    predecessor_block_count: predecessor_checkpoint.block_count,
                    predecessor_head_block_cid: predecessor_checkpoint.head_block_cid.clone(),
                    target_checkpoint_generation: intent.generation,
                    target_head_block_cid: intent.target_head_block_cid.clone(),
                    target_block_count: intent.target_block_count,
                    target_source_chain_blake3: intent.target_source_chain_blake3,
                    ipfs_authenticator_handle: self.ipfs.authenticator.handle.clone(),
                    ipfs_authenticator_revision: self
                        .ipfs
                        .authenticator
                        .ingress_qualification
                        .provider()
                        .revision,
                    ipfs_authenticator_policy_digest: self
                        .ipfs
                        .authenticator
                        .ingress_qualification
                        .provider()
                        .policy_digest,
                    ipfs_authenticator_public_key: self
                        .ipfs
                        .authenticator
                        .verification_policy
                        .public_key(),
                    checkpoint_store_handle: self.checkpoint_store.handle.clone(),
                    checkpoint_store_revision: self.checkpoint_store.qualification.revision,
                    checkpoint_store_policy_digest: self
                        .checkpoint_store
                        .qualification
                        .policy_digest,
                    archived_block_count: end,
                    blocks,
                };
                validate_signed_block_prefix_archive(&archive)?;
                validate_block_prefix_archive_against_source(&archive, source)?;
                let archive_bytes = norito::to_bytes(&archive).map_err(|err| {
                    GovernanceDagServiceError::State(format!(
                        "signed block-prefix archive encode failed: {err}"
                    ))
                })?;
                let filename = block_prefix_archive_filename(&archive)?;
                let (_, multipart) = canonical_ipfs_multipart_body(&filename, &archive_bytes)?;
                if block_prefix_archive_lengths_fit(
                    archive_bytes.len(),
                    multipart.len(),
                    self.config.max_request_bytes,
                ) {
                    break (archive, archive_bytes, filename);
                }
                if end == start.saturating_add(1) {
                    return Err(GovernanceDagServiceError::State(
                        "one signed block-prefix archive entry exceeds the configured request bound"
                            .to_owned(),
                    ));
                }
                end = end.saturating_sub(1);
            };

            self.checkpoint_store.assert_identity()?;
            self.ipfs.authenticator.assert_identity()?;
            let (ipfs_cid, publication) = ipfs_add_verified_with_publication(
                &self.ipfs,
                &filename,
                &archive_bytes,
                self.config.max_request_bytes,
                self.config.max_response_bytes,
            )
            .await?;
            let readback = ipfs_cat(
                &self.ipfs,
                &ipfs_cid,
                u64::try_from(archive_bytes.len()).unwrap_or(u64::MAX),
                archive_max_bytes,
            )
            .await?;
            if readback != archive_bytes
                || decode_signed_block_prefix_archive(&readback, archive_max_bytes)? != archive
            {
                return Err(GovernanceDagServiceError::State(
                    "signed block-prefix archive exact readback diverged".to_owned(),
                ));
            }
            let last = archive.blocks.last().ok_or_else(|| {
                GovernanceDagServiceError::State(
                    "validated signed block-prefix archive lost its last block".to_owned(),
                )
            })?;
            let next = BlockPrefixArchiveHeadV1 {
                generation: archive.archive_generation,
                digest: blake3_array(&archive_bytes),
                ipfs_cid,
                archived_block_count: archive.archived_block_count,
                last_block_cid: last.published.governance_block_cid.clone(),
                last_node_cid: last.published.governance_node_cid.clone(),
                predecessor_checkpoint_revision: archive.predecessor_checkpoint_revision,
                predecessor_checkpoint_digest: archive.predecessor_checkpoint_digest,
                predecessor_block_count: archive.predecessor_block_count,
                predecessor_head_block_cid: archive.predecessor_head_block_cid.clone(),
                publication: Some(publication),
            };
            verify_block_prefix_archive_publication(&archive, &archive_bytes, &next)?;
            self.ipfs.authenticator.assert_identity()?;
            self.checkpoint_store.assert_identity()?;
            intent.archive_head = next.clone();
            self.intent_revision = Some(save_publish_intent(
                &self.checkpoint_store,
                self.intent_revision,
                intent,
            )?);
            self.intent_generation_floor = intent.generation;
            predecessor = next;
            start = end;
        }
        if predecessor.archived_block_count != retained_start {
            return Err(GovernanceDagServiceError::State(
                "signed block-prefix archives did not cover the exact pruned prefix".to_owned(),
            ));
        }
        Ok(predecessor)
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
        if let Some(checkpoint) = &self.checkpoint {
            self.verify_block_prefix_archive_head(
                &checkpoint.archive_head,
                checkpoint.generation,
                None,
                &source,
            )
            .await?;
        }
        if let Some(intent) = &self.intent {
            let _ = validate_intent_against_source(
                intent,
                self.checkpoint.as_ref(),
                self.checkpoint_revision,
                &source,
            )?;
            if self
                .checkpoint
                .as_ref()
                .is_none_or(|checkpoint| checkpoint.archive_head != intent.archive_head)
            {
                self.verify_block_prefix_archive_head(
                    &intent.archive_head,
                    intent.generation,
                    Some(intent.generation),
                    &source,
                )
                .await?;
            }
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
                archive_head: self
                    .checkpoint
                    .as_ref()
                    .map_or_else(BlockPrefixArchiveHeadV1::empty, |checkpoint| {
                        checkpoint.archive_head.clone()
                    }),
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
        let target_source = validate_intent_against_source(
            &intent,
            self.checkpoint.as_ref(),
            self.checkpoint_revision,
            &source,
        )?;
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
        let archive_head = self
            .archive_would_be_pruned_prefix(&mut intent, &target_source, &published_blocks)
            .await?;
        // Seal the publication time in the intent so retries and competing
        // instances derive byte-identical mirror payloads.
        let published_at = intent.created_at_unix;
        let mirror = mirror_index_value(
            &target_source,
            &published_blocks,
            &archive_head,
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
            self.install_public_head(&intent.target_head_bytes, &head_ipfs_cid, &current)
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
            archive_head,
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
            if matches!(&self.head_mode, HeadMode::Ipns { .. }) {
                state.metrics.ipns_update_success_total =
                    state.metrics.ipns_update_success_total.saturating_add(1);
                state.metrics.last_ipns_update_timestamp_seconds = published_at;
            }
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
    checkpoint_revision: Option<[u8; 32]>,
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
        if checkpoint.is_none_or(|checkpoint| checkpoint.archive_head != intent.archive_head) {
            return Err(GovernanceDagServiceError::State(
                "completed checkpoint and retained publish intent disagree on archive progress"
                    .to_owned(),
            ));
        }
    } else {
        let base_archive = checkpoint.map_or_else(BlockPrefixArchiveHeadV1::empty, |checkpoint| {
            checkpoint.archive_head.clone()
        });
        if intent.archive_head.generation < base_archive.generation
            || intent.archive_head.archived_block_count < base_archive.archived_block_count
        {
            return Err(GovernanceDagServiceError::State(
                "publish intent rolled block-prefix archive progress back".to_owned(),
            ));
        }
        if intent.archive_head.generation == base_archive.generation {
            if intent.archive_head != base_archive {
                return Err(GovernanceDagServiceError::State(
                    "publish intent substituted its predecessor archive head".to_owned(),
                ));
            }
        } else {
            if intent.archive_head.archived_block_count <= base_archive.archived_block_count {
                return Err(GovernanceDagServiceError::State(
                    "publish intent advanced archive generation without advancing coverage"
                        .to_owned(),
                ));
            }
            let commitment = checkpoint_commitment(checkpoint, checkpoint_revision)?;
            if intent.archive_head.predecessor_checkpoint_revision != commitment.revision
                || intent.archive_head.predecessor_checkpoint_digest != commitment.digest
                || intent.archive_head.predecessor_block_count != commitment.block_count
                || intent.archive_head.predecessor_head_block_cid != commitment.head_block_cid
            {
                return Err(GovernanceDagServiceError::State(
                    "publish intent archive does not bind its exact predecessor checkpoint"
                        .to_owned(),
                ));
            }
        }
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
    let mut by_sequence = published_blocks_by_sequence(checkpoint, intent)?;
    for block in backfilled {
        insert_published_block(&mut by_sequence, block.clone())?;
    }
    select_mirror_suffix(
        &by_sequence,
        source,
        GOVERNANCE_DAG_MIRROR_MAX_ENTRIES_V1,
        GOVERNANCE_DAG_MIRROR_MAX_BYTES_V1,
    )
}

fn published_blocks_by_sequence(
    checkpoint: Option<&CheckpointBodyV1>,
    intent: &PublishIntentBodyV1,
) -> Result<BTreeMap<u64, PublishedBlockV1>, GovernanceDagServiceError> {
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
    Ok(by_sequence)
}

fn select_mirror_suffix<S: SourceChainView + ?Sized>(
    by_sequence: &BTreeMap<u64, PublishedBlockV1>,
    source: &S,
    max_entries: usize,
    max_bytes: u64,
) -> Result<Vec<PublishedBlockV1>, GovernanceDagServiceError> {
    if max_entries == 0 || max_bytes == 0 {
        return Err(GovernanceDagServiceError::State(
            "mirror retention bounds must be non-zero".to_owned(),
        ));
    }
    let mut retained_sequences = Vec::new();
    let mut retained_bytes = 0_u64;
    for source_block in source.blocks().iter().rev() {
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
                    "the newest block alone exceeds the version-1 mirror byte ceiling".to_owned(),
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

fn validate_block_prefix_archive_against_source<S: SourceChainView + ?Sized>(
    archive: &SignedBlockPrefixArchiveV1,
    source: &S,
) -> Result<(), GovernanceDagServiceError> {
    let target_count = usize::try_from(archive.target_block_count).map_err(|_| {
        GovernanceDagServiceError::State(
            "block-prefix archive target count exceeds host limits".to_owned(),
        )
    })?;
    let target = target_count
        .checked_sub(1)
        .and_then(|position| source.blocks().get(position))
        .ok_or_else(|| {
            GovernanceDagServiceError::Conflict(
                "block-prefix archive target is outside the verified source".to_owned(),
            )
        })?;
    let predecessor_matches = if archive.predecessor_block_count == 0 {
        archive.predecessor_head_block_cid.is_empty()
    } else {
        usize::try_from(archive.predecessor_block_count)
            .ok()
            .and_then(|count| count.checked_sub(1))
            .and_then(|position| source.blocks().get(position))
            .is_some_and(|block| {
                block.block.block_cid.as_slice() == archive.predecessor_head_block_cid.as_slice()
            })
    };
    if target.block.block_cid != archive.target_head_block_cid
        || (target_count == source.blocks().len()
            && archive.target_source_chain_blake3 != source.chain_blake3())
        || !predecessor_matches
    {
        return Err(GovernanceDagServiceError::Conflict(
            "block-prefix archive head or source binding diverged".to_owned(),
        ));
    }
    for entry in &archive.blocks {
        let position = usize::try_from(entry.published.sequence).map_err(|_| {
            GovernanceDagServiceError::State(
                "block-prefix archive sequence exceeds host limits".to_owned(),
            )
        })?;
        let source_block = source.blocks().get(position).ok_or_else(|| {
            GovernanceDagServiceError::Conflict(
                "block-prefix archive points outside the verified source".to_owned(),
            )
        })?;
        if source_block.bytes != entry.signed_block_bytes
            || source_block.block.block_cid != entry.published.governance_block_cid
            || source_block.block.node.node_cid != entry.published.governance_node_cid
            || source_block.payload_kind != entry.published.payload_kind
            || source_block.block.timestamp != entry.published.timestamp
            || source_block.encoded_blake3 != entry.published.encoded_blake3
            || u64::try_from(source_block.bytes.len()).unwrap_or(u64::MAX)
                != entry.published.encoded_len
        {
            return Err(GovernanceDagServiceError::Conflict(
                "block-prefix archive no longer matches the verified source".to_owned(),
            ));
        }
    }
    Ok(())
}

fn block_prefix_archive_entries<S: SourceChainView + ?Sized>(
    source: &S,
    by_sequence: &BTreeMap<u64, PublishedBlockV1>,
    start_sequence: u64,
    end_sequence: u64,
) -> Result<Vec<SignedBlockPrefixArchiveEntryV1>, GovernanceDagServiceError> {
    let count = end_sequence.checked_sub(start_sequence).ok_or_else(|| {
        GovernanceDagServiceError::State("block-prefix archive range is inverted".to_owned())
    })?;
    if count == 0 || count > u64::try_from(BLOCK_PREFIX_ARCHIVE_MAX_ENTRIES_V1).unwrap_or(u64::MAX)
    {
        return Err(GovernanceDagServiceError::State(
            "block-prefix archive range violates its entry bound".to_owned(),
        ));
    }
    (start_sequence..end_sequence)
        .map(|sequence| {
            let position = usize::try_from(sequence).map_err(|_| {
                GovernanceDagServiceError::State(
                    "block-prefix archive sequence exceeds host limits".to_owned(),
                )
            })?;
            let source_block = source.blocks().get(position).ok_or_else(|| {
                GovernanceDagServiceError::State(
                    "block-prefix archive source range is incomplete".to_owned(),
                )
            })?;
            let published = by_sequence.get(&sequence).cloned().ok_or_else(|| {
                GovernanceDagServiceError::State(
                    "would-be-pruned block has no authenticated IPFS mapping".to_owned(),
                )
            })?;
            Ok(SignedBlockPrefixArchiveEntryV1 {
                published,
                signed_block_bytes: source_block.bytes.clone(),
            })
        })
        .collect()
}

fn estimated_block_prefix_archive_entry_bytes(
    source_block: &SourceBlock,
    published: &PublishedBlockV1,
) -> Result<usize, GovernanceDagServiceError> {
    source_block
        .bytes
        .len()
        .checked_add(published.payload_kind.len())
        .and_then(|size| size.checked_add(published.ipfs_cid.len()))
        .and_then(|size| size.checked_add(1024))
        .ok_or_else(|| {
            GovernanceDagServiceError::State(
                "block-prefix archive size estimate overflowed".to_owned(),
            )
        })
}

fn mirror_index_value<S: SourceChainView + ?Sized>(
    source: &S,
    blocks: &[PublishedBlockV1],
    archive_head: &BlockPrefixArchiveHeadV1,
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
    validate_block_prefix_archive_head(archive_head)?;
    let mut archive = JsonMap::new();
    archive.insert(
        "generation".into(),
        JsonValue::from(archive_head.generation),
    );
    archive.insert(
        "archived_block_count".into(),
        JsonValue::from(archive_head.archived_block_count),
    );
    archive.insert(
        "blake3".into(),
        if archive_head.generation != 0 {
            JsonValue::from(hex::encode(archive_head.digest))
        } else {
            JsonValue::Null
        },
    );
    archive.insert(
        "ipfs_cid".into(),
        if archive_head.generation != 0 {
            JsonValue::from(archive_head.ipfs_cid.clone())
        } else {
            JsonValue::Null
        },
    );
    let mut root = JsonMap::new();
    root.insert("schema".into(), JsonValue::from(MIRROR_INDEX_SCHEMA));
    root.insert("generation".into(), JsonValue::from(generation));
    root.insert("generated_at".into(), JsonValue::from(published_at));
    root.insert("head".into(), JsonValue::Object(head));
    root.insert("archive".into(), JsonValue::Object(archive));
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
        &checkpoint.archive_head,
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
        "archive_generation".into(),
        JsonValue::from(checkpoint.archive_head.generation),
    );
    value.insert(
        "archived_block_count".into(),
        JsonValue::from(checkpoint.archive_head.archived_block_count),
    );
    value.insert(
        "archive_blake3_hex".into(),
        if checkpoint.archive_head.generation != 0 {
            JsonValue::from(hex::encode(checkpoint.archive_head.digest))
        } else {
            JsonValue::Null
        },
    );
    value.insert(
        "archive_ipfs_cid".into(),
        if checkpoint.archive_head.generation != 0 {
            JsonValue::from(checkpoint.archive_head.ipfs_cid.clone())
        } else {
            JsonValue::Null
        },
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
    include!("governance_service/tests/support.rs");
    include!("governance_service/tests/archive_and_replay.rs");
    include!("governance_service/tests/authenticated_receivers_and_startup.rs");
    include!("governance_service/tests/publication_and_mirror.rs");
    include!("governance_service/tests/restart_and_live_kubo.rs");
}
