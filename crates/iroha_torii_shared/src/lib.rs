//! Constant values used in Torii that might be re-used by client libraries as well.
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Shared data-availability helpers (sampling, assignment).
pub mod da;

/// Latest Torii API version advertised by default (`major.minor`).
pub const API_VERSION_DEFAULT: &str = "1.1";
/// Supported Torii API versions in ascending order (`major.minor`).
pub const API_VERSION_SUPPORTED: &[&str] = &["1.0", API_VERSION_DEFAULT];
/// Minimum Torii API version required for proof/staking/fee endpoints.
pub const API_MIN_PROOF_VERSION: &str = API_VERSION_DEFAULT;
/// Optional unix timestamp when the oldest supported Torii API version sunsets.
pub const API_VERSION_SUNSET_UNIX: Option<u64> = Some(1_893_456_000);

/// Header carrying the requested Torii API version (semantic `major.minor`).
pub const HEADER_API_VERSION: &str = "x-iroha-api-version";

pub mod uri {
    //! URI that Torii uses to route incoming requests.

    /// Query URI is used to handle incoming Query requests.
    pub const QUERY: &str = "/query";
    /// Transaction URI is used to handle incoming ISI requests.
    pub const TRANSACTION: &str = "/transaction";
    /// Health URI is used to handle incoming Healthcheck requests.
    pub const HEALTH: &str = "/health";
    /// URI used to fetch a window of block headers (newest first, optional `from`/`limit`).
    pub const LEDGER_HEADERS: &str = "/v2/ledger/headers";
    /// URI used to fetch the execution state root for a block height.
    pub const LEDGER_STATE_ROOT: &str = "/v2/ledger/state/{height}";
    /// URI used to fetch the execution state proof/QC for a block height.
    pub const LEDGER_STATE_PROOF: &str = "/v2/ledger/state-proof/{height}";
    /// URI used to fetch Merkle proofs for a transaction entrypoint within a block.
    pub const LEDGER_BLOCK_PROOF: &str = "/v2/ledger/block/{height}/proof/{entry_hash}";
    /// URI used to list validator-set snapshots (newest first).
    pub const SUMERAGI_VALIDATOR_SETS: &str = "/v2/sumeragi/validator-sets";
    /// URI used to fetch a validator-set snapshot by block height.
    pub const SUMERAGI_VALIDATOR_SET_BY_HEIGHT: &str = "/v2/sumeragi/validator-sets/{height}";
    /// Peers URI is used to find all peers in the network
    pub const PEERS: &str = "/peers";
    /// The web socket uri used to subscribe to block and transactions statuses.
    pub const SUBSCRIPTION: &str = "/events";
    /// URI for inspecting proof retention state and pruning candidates.
    pub const PROOF_RETENTION_STATUS: &str = "/v2/proofs/retention";
    /// The web socket uri used to subscribe to blocks stream.
    pub const BLOCKS_STREAM: &str = "/block/stream";
    /// Debug endpoint exposing cached AXT proof state per dataspace.
    pub const AXT_PROOF_CACHE_STATUS: &str = "/v2/debug/axt/cache";
    /// The URI for local config changing inspecting
    pub const CONFIGURATION: &str = "/configuration";
    /// URI for applying Nexus lane lifecycle plans (add/retire lanes at runtime).
    pub const NEXUS_LANE_LIFECYCLE: &str = "/v2/nexus/lifecycle";
    /// URI to report status for administration
    pub const STATUS: &str = "/status";
    ///  Metrics URI is used to export metrics according to [Prometheus
    ///  Guidance](https://prometheus.io/docs/instrumenting/writing_exporters/).
    pub const METRICS: &str = "/metrics";
    /// URI for retrieving the schema with which Iroha was built.
    pub const SCHEMA: &str = "/schema";
    /// URI for getting the API version currently used
    pub const API_VERSION: &str = "/api_version";
    /// URI for listing supported Torii API versions and the default.
    pub const API_VERSIONS: &str = "/v2/api/versions";
    /// URI for getting cpu profile
    pub const PROFILE: &str = "/debug/pprof/profile";
    /// Base path for governance API endpoints
    pub const GOV_BASE: &str = "/v2/gov";
    /// Governance: create a proposal to deploy IVM bytecode (.to)
    pub const GOV_PROPOSE_DEPLOY: &str = "/v2/gov/proposals/deploy-contract";
    /// Governance: submit a ZK ballot (default mode)
    pub const GOV_BALLOT_ZK: &str = "/v2/gov/ballots/zk";
    /// Governance: submit a non-ZK quadratic ballot (optional mode)
    pub const GOV_BALLOT_PLAIN: &str = "/v2/gov/ballots/plain";
    /// Governance: finalize a referendum (compute tally and emit Approved/Rejected)
    pub const GOV_FINALIZE: &str = "/v2/gov/finalize";
    /// Governance: enact an approved referendum (build `EnactReferendum` instruction)
    pub const GOV_ENACT: &str = "/v2/gov/enact";
    /// Governance: query the current sortition council
    pub const GOV_COUNCIL_CURRENT: &str = "/v2/gov/council/current";
    /// Governance: persist a VRF-derived council for an epoch (app API)
    pub const GOV_COUNCIL_PERSIST: &str = "/v2/gov/council/persist";
    /// Governance: replace a council member using the next alternate
    pub const GOV_COUNCIL_REPLACE: &str = "/v2/gov/council/replace";
    /// Governance: audit info for council derivation (seed/epoch)
    pub const GOV_COUNCIL_AUDIT: &str = "/v2/gov/council/audit";
    /// Governance: get a proposal by id (hex)
    pub const GOV_PROPOSAL_GET: &str = "/v2/gov/proposals/{id}";
    /// Governance: get token locks for a referendum id
    pub const GOV_LOCKS_GET: &str = "/v2/gov/locks/{rid}";
    /// Governance: get a referendum by id
    pub const GOV_REFERENDUM_GET: &str = "/v2/gov/referenda/{id}";
    /// Governance: get a current tally snapshot by referendum id
    pub const GOV_TALLY_GET: &str = "/v2/gov/tally/{id}";
    /// Governance: convenience endpoint to apply protected namespaces parameter
    pub const GOV_PROTECTED_SET: &str = "/v2/gov/protected-namespaces";
    /// Governance: list active contract instances for a namespace
    pub const GOV_INSTANCES_BY_NS: &str = "/v2/gov/instances/{ns}";
    /// Node: capabilities advert (supported ABI versions, etc.)
    pub const NODE_CAPABILITIES: &str = "/v2/node/capabilities";
    /// Runtime: get active ABI versions
    pub const RUNTIME_ABI_ACTIVE: &str = "/v2/runtime/abi/active";
    /// Runtime: get canonical ABI hash for the node's active policy
    pub const RUNTIME_ABI_HASH: &str = "/v2/runtime/abi/hash";
    /// Runtime: list proposed/activated runtime upgrades
    pub const RUNTIME_UPGRADES_LIST: &str = "/v2/runtime/upgrades";
    /// Runtime: expose runtime metrics (JSON summary)
    pub const RUNTIME_METRICS: &str = "/v2/runtime/metrics";
    /// Runtime: propose a runtime upgrade (manifest body)
    pub const RUNTIME_UPGRADES_PROPOSE: &str = "/v2/runtime/upgrades/propose";
    /// Runtime: activate a runtime upgrade by id (hex)
    pub const RUNTIME_UPGRADES_ACTIVATE: &str = "/v2/runtime/upgrades/activate/{id}";
    /// Runtime: cancel a runtime upgrade by id (hex)
    pub const RUNTIME_UPGRADES_CANCEL: &str = "/v2/runtime/upgrades/cancel/{id}";
}

/// Canonical Torii error envelope returned for HTTP API failures.
#[derive(JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone)]
pub struct ErrorEnvelope {
    /// Stable error code string.
    pub code: String,
    /// Human-readable error detail.
    pub message: String,
}

impl ErrorEnvelope {
    /// Construct a new error envelope.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Stable error code string.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Human-readable error detail.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Queue pressure snapshot returned with transaction queue rejections.
#[derive(JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone)]
pub struct QueueErrorSnapshot {
    /// Queue state label (`healthy` or `saturated`).
    pub state: String,
    /// Current queued transaction count.
    pub queued: u64,
    /// Configured queue capacity.
    pub capacity: u64,
    /// Whether the queue is currently saturated.
    pub saturated: bool,
}

/// Structured queue rejection payload returned by Torii.
#[derive(JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone)]
pub struct QueueErrorEnvelope {
    /// Stable queue rejection code (`queue_full`, `per_user_queue_limit`, ...).
    pub code: String,
    /// Human-readable queue rejection detail.
    pub message: String,
    /// Queue pressure snapshot at rejection time.
    pub queue: QueueErrorSnapshot,
    /// Suggested retry delay in seconds for transient queue errors.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

/// Supported Torii API versions and defaults exposed over `/v2/api/versions`.
#[derive(JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone)]
pub struct ApiVersionInfo {
    /// Default API version the node will assume when no header is present.
    pub default: String,
    /// All supported API versions (sorted ascending).
    pub supported: Vec<String>,
    /// Optional unix timestamp when the lowest supported version sunsets.
    pub sunset_unix: Option<u64>,
    /// Minimum API version required for proof/staking/fee endpoints.
    pub min_proof_version: String,
}

/// Per-backend proof retention snapshot.
#[derive(JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone)]
pub struct ProofRetentionBackendStatus {
    /// Backend identifier (e.g., `halo2/ipa`).
    pub backend: String,
    /// Total proof records currently tracked for this backend.
    pub records: u64,
    /// Proof records that would be pruned if retention runs now.
    pub prunable: u64,
    /// Oldest verification height (if recorded).
    pub oldest_height: Option<u64>,
    /// Newest verification height (if recorded).
    pub newest_height: Option<u64>,
}

/// Proof retention configuration and live counters.
#[derive(JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone)]
pub struct ProofRetentionStatus {
    /// Configured per-backend cap (0 = unlimited).
    pub cap_per_backend: usize,
    /// Grace window (blocks) retained before pruning by age.
    pub grace_blocks: u64,
    /// Maximum removals per enforcement pass (0 = unlimited).
    pub prune_batch: usize,
    /// Aggregate proof count across all backends.
    pub total_records: u64,
    /// Aggregate proof count that would be pruned if enforcement runs now.
    pub total_prunable: u64,
    /// Per-backend retention snapshots.
    pub backends: Vec<ProofRetentionBackendStatus>,
}

#[cfg(test)]
mod tests {
    use super::{ErrorEnvelope, QueueErrorEnvelope, QueueErrorSnapshot};

    #[test]
    fn error_envelope_new_sets_fields() {
        let envelope = ErrorEnvelope::new("test_code", "test message");
        assert_eq!(envelope.code(), "test_code");
        assert_eq!(envelope.message(), "test message");
    }

    #[test]
    fn queue_error_envelope_roundtrip_preserves_fields() {
        let envelope = QueueErrorEnvelope {
            code: "queue_full".to_owned(),
            message: "transaction queue is at capacity".to_owned(),
            queue: QueueErrorSnapshot {
                state: "saturated".to_owned(),
                queued: 24,
                capacity: 24,
                saturated: true,
            },
            retry_after_seconds: Some(1),
        };
        let bytes = norito::to_bytes(&envelope).expect("encode queue envelope");
        let decoded: QueueErrorEnvelope =
            norito::decode_from_bytes(&bytes).expect("decode queue envelope");
        assert_eq!(decoded.code, "queue_full");
        assert_eq!(decoded.message, "transaction queue is at capacity");
        assert_eq!(decoded.queue.state, "saturated");
        assert_eq!(decoded.queue.queued, 24);
        assert_eq!(decoded.queue.capacity, 24);
        assert!(decoded.queue.saturated);
        assert_eq!(decoded.retry_after_seconds, Some(1));
    }
}

/// Iroha Connect protocol types (WalletConnect‑style overlay).
///
/// These are Norito‑encoded wire types used over Torii WebSockets and the
/// Iroha P2P relay for pairing dApps and wallets. This module contains only
/// data structures and no transport/server logic.
pub mod connect;
/// Shared retry utilities for Connect clients (reconnect policy, jitter tables).
pub mod connect_retry;
/// Helper SDK for sealing/opening Connect frames and key derivation.
pub mod connect_sdk;
