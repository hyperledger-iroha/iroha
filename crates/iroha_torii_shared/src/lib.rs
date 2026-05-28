//! Constant values used in Torii that might be re-used by client libraries as well.
use iroha_data_model::{
    account::{AccountAlias, AccountId, OpaqueAccountId},
    nexus::UniversalAccountId,
    transaction::error::TransactionRejectionReason,
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Shared data-availability helpers (sampling, assignment).
pub mod da;
/// Shared QR Code encoder used by Torii and CLI offline flows.
pub mod qr;

/// First-release Torii API version advertised by default (`major.minor`).
pub const API_VERSION_DEFAULT: &str = "1.0";
/// Supported Torii API versions in ascending order (`major.minor`).
pub const API_VERSION_SUPPORTED: &[&str] = &[API_VERSION_DEFAULT];
/// Minimum Torii API version required for proof/staking/fee endpoints.
pub const API_MIN_PROOF_VERSION: &str = API_VERSION_DEFAULT;
/// Optional unix timestamp when the oldest supported Torii API version sunsets.
pub const API_VERSION_SUNSET_UNIX: Option<u64> = None;

/// Header carrying the requested Torii API version (semantic `major.minor`).
pub const HEADER_API_VERSION: &str = "x-iroha-api-version";

pub mod uri {
    //! URI that Torii uses to route incoming requests.

    /// Query URI is used to handle incoming Query requests.
    pub const QUERY: &str = "/v1/query";
    /// Transaction URI is used to handle incoming signed transaction requests.
    pub const TRANSACTION: &str = "/v1/pipeline/transactions";
    /// Transaction entrypoint URI is used to handle sealed and non-external submissions.
    pub const TRANSACTION_ENTRYPOINT: &str = "/v1/pipeline/transaction-entrypoints";
    /// Batched transaction URI is used to handle multiple signed transaction submissions.
    pub const TRANSACTIONS_BATCH: &str = "/v1/pipeline/transactions/batch";
    /// Health URI is used to handle incoming Healthcheck requests.
    pub const HEALTH: &str = "/health";
    /// URI used to fetch a window of block headers (newest first, optional `from`/`limit`).
    pub const LEDGER_HEADERS: &str = "/v1/ledger/headers";
    /// URI used to fetch the execution state root for a block height.
    pub const LEDGER_STATE_ROOT: &str = "/v1/ledger/state/{height}";
    /// URI used to fetch the execution state proof/QC for a block height.
    pub const LEDGER_STATE_PROOF: &str = "/v1/ledger/state-proof/{height}";
    /// URI used to fetch Merkle proofs for a transaction entrypoint within a block.
    pub const LEDGER_BLOCK_PROOF: &str = "/v1/ledger/block/{height}/proof/{entry_hash}";
    /// URI used to list validator-set snapshots (newest first).
    pub const SUMERAGI_VALIDATOR_SETS: &str = "/v1/sumeragi/validator-sets";
    /// URI used to fetch a validator-set snapshot by block height.
    pub const SUMERAGI_VALIDATOR_SET_BY_HEIGHT: &str = "/v1/sumeragi/validator-sets/{height}";
    /// Peers URI is used to find all peers in the network
    pub const PEERS: &str = "/v1/peers";
    /// The web socket uri used to subscribe to block and transactions statuses.
    pub const SUBSCRIPTION: &str = "/v1/events/ws";
    /// URI for inspecting proof retention state and pruning candidates.
    pub const PROOF_RETENTION_STATUS: &str = "/v1/proofs/retention";
    /// URI used to fetch FASTPQ proof sidecars for a committed block height.
    pub const PIPELINE_FASTPQ_PROOFS: &str = "/v1/pipeline/recovery/{height}/fastpq-proofs";
    /// URI used to list historical trigger completion records from committed blocks.
    pub const TRIGGER_COMPLETIONS: &str = "/v1/triggers/completed";
    /// The web socket uri used to subscribe to blocks stream.
    pub const BLOCKS_STREAM: &str = "/v1/blocks/stream";
    /// Debug endpoint exposing cached AXT proof state per dataspace.
    pub const AXT_PROOF_CACHE_STATUS: &str = "/v1/debug/axt/cache";
    /// The URI for local config changing inspecting
    pub const CONFIGURATION: &str = "/v1/configuration";
    /// URI for applying Nexus lane lifecycle plans (add/retire lanes at runtime).
    pub const NEXUS_LANE_LIFECYCLE: &str = "/v1/nexus/lifecycle";
    /// URI to report status for administration
    pub const STATUS: &str = "/status";
    ///  Metrics URI is used to export metrics according to [Prometheus
    ///  Guidance](https://prometheus.io/docs/instrumenting/writing_exporters/).
    pub const METRICS: &str = "/metrics";
    /// URI for retrieving the schema with which Iroha was built.
    pub const SCHEMA: &str = "/v1/schema";
    /// URI for getting the API version currently used
    pub const API_VERSION: &str = "/v1/api/version";
    /// URI for listing supported Torii API versions and the default.
    pub const API_VERSIONS: &str = "/v1/api/versions";
    /// URI for getting cpu profile
    pub const PROFILE: &str = "/debug/pprof/profile";
    /// Base path for governance API endpoints
    pub const GOV_BASE: &str = "/v1/gov";
    /// Base path for Ministry API endpoints.
    pub const MINISTRY_BASE: &str = "/v1/ministry";
    /// Ministry: build a draft agenda proposal transaction for local signing.
    pub const MINISTRY_AGENDA_PROPOSAL_DRAFT: &str = "/v1/ministry/agenda/proposals/draft";
    /// Ministry: fetch a submitted agenda proposal by proposal id.
    pub const MINISTRY_AGENDA_PROPOSAL_GET: &str = "/v1/ministry/agenda/proposals/{proposal_id}";
    /// Governance: create a proposal to deploy IVM bytecode (.to)
    pub const GOV_PROPOSE_DEPLOY: &str = "/v1/gov/proposals/deploy-contract";
    /// Governance: submit a ZK ballot (default mode)
    pub const GOV_BALLOT_ZK: &str = "/v1/gov/ballots/zk";
    /// Governance: submit a non-ZK quadratic ballot (optional mode)
    pub const GOV_BALLOT_PLAIN: &str = "/v1/gov/ballots/plain";
    /// Governance: draft an equal signed Parliament stage ballot
    pub const GOV_PARLIAMENT_BALLOT: &str = "/v1/gov/parliament/ballots";
    /// Governance: finalize a referendum (compute tally and emit Approved/Rejected)
    pub const GOV_FINALIZE: &str = "/v1/gov/finalize";
    /// Governance: enact an approved referendum (build `EnactReferendum` instruction)
    pub const GOV_ENACT: &str = "/v1/gov/enact";
    /// Governance: query the current sortition council
    pub const GOV_COUNCIL_CURRENT: &str = "/v1/gov/council/current";
    /// Governance: query exact citizenship registry count
    pub const GOV_CITIZENS_COUNT: &str = "/v1/gov/citizens";
    /// Governance: query citizenship status for an account
    pub const GOV_CITIZEN_STATUS: &str = "/v1/gov/citizens/{account_id}";
    /// Governance: persist a VRF-derived council for an epoch (app API)
    pub const GOV_COUNCIL_PERSIST: &str = "/v1/gov/council/persist";
    /// Governance: replace a council member using the next alternate
    pub const GOV_COUNCIL_REPLACE: &str = "/v1/gov/council/replace";
    /// Governance: audit info for council derivation (seed/epoch)
    pub const GOV_COUNCIL_AUDIT: &str = "/v1/gov/council/audit";
    /// Governance: get a proposal by id (hex)
    pub const GOV_PROPOSAL_GET: &str = "/v1/gov/proposals/{id}";
    /// Governance: get token locks for a referendum id
    pub const GOV_LOCKS_GET: &str = "/v1/gov/locks/{rid}";
    /// Governance: get a referendum by id
    pub const GOV_REFERENDUM_GET: &str = "/v1/gov/referenda/{id}";
    /// Governance: get a current tally snapshot by referendum id
    pub const GOV_TALLY_GET: &str = "/v1/gov/tally/{id}";
    /// Governance: convenience endpoint to apply protected namespaces parameter
    pub const GOV_PROTECTED_SET: &str = "/v1/gov/protected-namespaces";
    /// Governance: read the active binding for a canonical contract address
    pub const GOV_CONTRACT_GET: &str = "/v1/gov/contracts/{contract_address}";
    /// Node: capabilities advert (runtime ABI version, etc.)
    pub const NODE_CAPABILITIES: &str = "/v1/node/capabilities";
    /// Node: latest persisted query projection checkpoint descriptor
    pub const NODE_QUERY_PROJECTION_CHECKPOINT: &str = "/v1/node/query/projection/checkpoint";
    /// Node: validate uploaded shard refs and preview a rebuilt projection checkpoint
    pub const NODE_QUERY_PROJECTION_CHECKPOINT_PLAN: &str =
        "/v1/node/query/projection/checkpoint/plan";
    /// Node: rebuild uploaded shard refs and persist the resulting projection checkpoint
    pub const NODE_QUERY_PROJECTION_CHECKPOINT_PUBLISH: &str =
        "/v1/node/query/projection/checkpoint/publish";
    /// Node: enumerate the canonical live query projection shard catalog for one resource family
    pub const NODE_QUERY_PROJECTION_SHARD_CATALOG: &str =
        "/v1/node/query/projection/catalog/{resource}";
    /// Node: export one canonical query projection shard archive
    pub const NODE_QUERY_PROJECTION_SHARD_EXPORT: &str =
        "/v1/node/query/projection/shards/{resource}/{partition_id}";
    /// Runtime: get the active ABI version
    pub const RUNTIME_ABI_ACTIVE: &str = "/v1/runtime/abi/active";
    /// Runtime: get canonical ABI hash for the node's active policy
    pub const RUNTIME_ABI_HASH: &str = "/v1/runtime/abi/hash";
    /// Runtime: list proposed/activated runtime upgrades
    pub const RUNTIME_UPGRADES_LIST: &str = "/v1/runtime/upgrades";
    /// Runtime: expose runtime metrics (JSON summary)
    pub const RUNTIME_METRICS: &str = "/v1/runtime/metrics";
    /// Runtime: propose a runtime upgrade (manifest body)
    pub const RUNTIME_UPGRADES_PROPOSE: &str = "/v1/runtime/upgrades/propose";
    /// Runtime: activate a runtime upgrade by id (hex)
    pub const RUNTIME_UPGRADES_ACTIVATE: &str = "/v1/runtime/upgrades/activate/{id}";
    /// Runtime: cancel a runtime upgrade by id (hex)
    pub const RUNTIME_UPGRADES_CANCEL: &str = "/v1/runtime/upgrades/cancel/{id}";
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

/// AXT rejection metadata returned with validation failures when available.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, Default,
)]
pub struct AxtErrorDetails {
    /// Stable AXT rejection code.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Human-readable AXT rejection label.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// AXT policy snapshot version used to reject the request.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub snapshot_version: Option<u64>,
    /// Dataspace id involved in the rejection.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub dataspace: Option<u64>,
    /// Lane id involved in the rejection.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub lane: Option<u32>,
    /// Minimum handle era a client should use for retry.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub next_min_handle_era: Option<u64>,
    /// Minimum sub-nonce a client should use for retry.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub next_min_sub_nonce: Option<u64>,
}

/// Structured metadata carried by [`ErrorEnvelope`].
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, Default,
)]
pub struct ErrorDetails {
    /// Public surface layer that produced the error (for example `cli`, `torii`, or `mcp`).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// ISO-20022-style or Torii-local rejection code when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub reject_code: Option<String>,
    /// Queue pressure snapshot at transaction rejection time.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub queue: Option<QueueErrorSnapshot>,
    /// Suggested retry delay in seconds for transient errors.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
    /// Endpoint associated with throttling or version failures.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Field associated with validation or decode failures.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Expected field value, status, profile, or discriminant.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// Actual field value, status, profile, or discriminant.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
    /// Network profile involved in the error.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// I105 chain discriminant involved in the error.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub chain_discriminant: Option<u16>,
    /// Signed transaction hash involved in finality/status failures.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    /// Last observed transaction status when a finality wait failed.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_status: Option<String>,
    /// Actionable debugging hint for callers.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// AXT rejection metadata when validation failed under AXT policy.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub axt: Option<AxtErrorDetails>,
}

impl ErrorDetails {
    /// Return whether this details payload carries any structured fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layer.is_none()
            && self.reject_code.is_none()
            && self.queue.is_none()
            && self.retry_after_seconds.is_none()
            && self.endpoint.is_none()
            && self.field.is_none()
            && self.expected.is_none()
            && self.actual.is_none()
            && self.profile.is_none()
            && self.chain_discriminant.is_none()
            && self.tx_hash.is_none()
            && self.last_status.is_none()
            && self.hint.is_none()
            && self.axt.is_none()
    }
}

/// Stable public network profile metadata used by clients and Torii helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkProfile {
    /// Profile name accepted by public tooling.
    pub name: &'static str,
    /// I105 chain discriminant assigned to this public network.
    pub chain_discriminant: u16,
}

/// Taira public testnet profile name.
pub const NETWORK_PROFILE_TAIRA: &str = "taira";
/// Minamoto public network profile name.
pub const NETWORK_PROFILE_MINAMOTO: &str = "minamoto";
/// Taira public testnet I105 chain discriminant.
pub const TAIRA_CHAIN_DISCRIMINANT: u16 = 369;
/// Minamoto public network I105 chain discriminant.
pub const MINAMOTO_CHAIN_DISCRIMINANT: u16 = 753;

/// Public network profiles accepted by developer-facing tools.
pub const NETWORK_PROFILES: &[NetworkProfile] = &[
    NetworkProfile {
        name: NETWORK_PROFILE_TAIRA,
        chain_discriminant: TAIRA_CHAIN_DISCRIMINANT,
    },
    NetworkProfile {
        name: NETWORK_PROFILE_MINAMOTO,
        chain_discriminant: MINAMOTO_CHAIN_DISCRIMINANT,
    },
];

/// Resolve a public network profile by name.
#[must_use]
pub fn network_profile(name: &str) -> Option<&'static NetworkProfile> {
    NETWORK_PROFILES
        .iter()
        .find(|profile| profile.name.eq_ignore_ascii_case(name.trim()))
}

/// Resolve a public network profile by I105 chain discriminant.
#[must_use]
pub fn network_profile_for_discriminant(discriminant: u16) -> Option<&'static NetworkProfile> {
    NETWORK_PROFILES
        .iter()
        .find(|profile| profile.chain_discriminant == discriminant)
}

/// Return a comma-separated list of supported public network profile names.
#[must_use]
pub fn network_profile_names() -> String {
    NETWORK_PROFILES
        .iter()
        .map(|profile| profile.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Canonical Torii error envelope returned for HTTP API failures.
#[derive(JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone)]
pub struct ErrorEnvelope {
    /// Stable error code string.
    pub code: String,
    /// Human-readable error detail.
    pub message: String,
    /// Optional machine-readable context for this error.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub details: Option<ErrorDetails>,
}

impl ErrorEnvelope {
    /// Construct a new error envelope.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// Attach structured details to an error envelope.
    #[must_use]
    pub fn with_details(mut self, details: ErrorDetails) -> Self {
        self.details = Some(details);
        self
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

/// Supported Torii API versions and defaults exposed over `/v1/api/versions`.
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

/// Typed status payload returned by `/v1/pipeline/transactions/status`.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
pub struct PipelineTransactionStatusResponse {
    /// Canonical signed transaction hash (hex, lowercase).
    pub hash: String,
    /// Current pipeline status details.
    pub status: PipelineTransactionStatus,
    /// Human-readable one-line summary for CLI and operator diagnostics.
    #[norito(default)]
    pub summary: String,
    /// Structured diagnostics decoded from rejection and execution failures.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<PipelineDiagnostic>,
    /// Trigger completions associated with this status response when Torii can hydrate them.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub trigger_completions: Vec<TriggerCompletionSummary>,
    /// Read scope applied by Torii (`local`, `auto`, `global`).
    pub scope: String,
    /// Source used to resolve the status (`cache`, `queue`, `state`).
    pub resolved_from: String,
}

/// Structured pipeline diagnostic extracted from a rejection reason or execution failure.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
pub struct PipelineDiagnostic {
    /// Stable broad category such as `validation`, `ivm_execution`, or `trigger_execution`.
    pub category: String,
    /// Stable machine-readable rejection code.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Decoded rejection reason text without transport encoding.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub decoded_reason: Option<String>,
    /// Contract address or alias when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
    /// Entrypoint name when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    /// Trigger identifier when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub trigger_id: Option<String>,
    /// Trigger step index when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub step_index: Option<u32>,
    /// VM program counter when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub vm_pc: Option<u64>,
    /// VM function name when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// Source location or source fragment when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// VM opcode when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub opcode: Option<String>,
    /// Host syscall when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub syscall: Option<String>,
    /// Lossless raw rejection reason string.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub raw_reason: Option<String>,
}

/// Compact trigger completion included in status and smoke evidence.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
pub struct TriggerCompletionSummary {
    /// Trigger identifier.
    pub trigger_id: String,
    /// Entrypoint hash for this trigger execution.
    pub trigger_execution_hash: String,
    /// Step index within the entrypoint execution.
    pub step_index: u32,
    /// `Success` or `Failure`.
    pub outcome: String,
    /// Failure message when present.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Historical trigger completion record returned by `/v1/triggers/completed`.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
pub struct TriggerCompletionRecord {
    /// Block height containing this trigger completion.
    pub block_height: u64,
    /// Entrypoint/result index in the block when it can be resolved.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub entrypoint_index: Option<u64>,
    /// Compact completion payload.
    pub completion: TriggerCompletionSummary,
    /// Evidence source: `block_result` for persisted completion events or `reconstructed_result`
    /// for legacy blocks reconstructed from transaction results.
    pub source: String,
}

/// Historical trigger completion query response.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
pub struct TriggerCompletionListResponse {
    /// Latest committed block height observed by the serving node.
    pub latest_height: u64,
    /// First block height scanned.
    pub from_height: u64,
    /// Last block height scanned.
    pub to_height: u64,
    /// Number of block heights inspected.
    pub scanned_blocks: u64,
    /// Maximum number of records requested.
    pub limit: u64,
    /// Matching completion records, newest block first.
    pub completions: Vec<TriggerCompletionRecord>,
}

/// Status details embedded in [`PipelineTransactionStatusResponse`].
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
pub struct PipelineTransactionStatus {
    /// Stable pipeline status kind (`Queued`, `Approved`, `Committed`, `Applied`, `Rejected`, `Expired`).
    pub kind: String,
    /// Block height reported for the status when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u64>,
    /// Structured rejection reason for rejected transactions.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<TransactionRejectionReason>,
}

impl PipelineTransactionStatusResponse {
    /// Construct a status response and populate summary/diagnostics from the status payload.
    #[must_use]
    pub fn new(
        hash: String,
        status: PipelineTransactionStatus,
        scope: String,
        resolved_from: String,
    ) -> Self {
        let diagnostics = status
            .rejection_reason
            .as_ref()
            .map(diagnostics_from_rejection_reason)
            .unwrap_or_default();
        let summary = pipeline_status_summary(&status, diagnostics.first());
        Self {
            hash,
            status,
            summary,
            diagnostics,
            trigger_completions: Vec::new(),
            scope,
            resolved_from,
        }
    }
}

fn pipeline_status_summary(
    status: &PipelineTransactionStatus,
    first_diagnostic: Option<&PipelineDiagnostic>,
) -> String {
    if let Some(diagnostic) = first_diagnostic {
        return format!("{}: {}", status.kind, diagnostic.message);
    }
    status.kind.clone()
}

fn diagnostics_from_rejection_reason(
    reason: &TransactionRejectionReason,
) -> Vec<PipelineDiagnostic> {
    let (category, code, message): (&str, &str, String) = match reason {
        TransactionRejectionReason::AccountDoesNotExist(err) => (
            "account",
            "account_does_not_exist",
            format!("account does not exist: {err}"),
        ),
        TransactionRejectionReason::LimitCheck(err) => {
            ("limit_check", "limit_check", err.to_string())
        }
        TransactionRejectionReason::Validation(err) => {
            ("validation", "validation", err.to_string())
        }
        TransactionRejectionReason::InstructionExecution(err) => (
            "instruction_execution",
            "instruction_execution",
            err.to_string(),
        ),
        TransactionRejectionReason::IvmExecution(err) => {
            ("ivm_execution", "ivm_execution", err.to_string())
        }
        TransactionRejectionReason::TriggerExecution(err) => {
            ("trigger_execution", "trigger_execution", err.to_string())
        }
    };

    let mut diagnostic = PipelineDiagnostic {
        category: category.to_owned(),
        code: Some(code.to_owned()),
        message: message.clone(),
        decoded_reason: Some(message.clone()),
        contract: extract_labeled_value(&message, &["contract=", "contract: "]),
        entrypoint: extract_labeled_value(&message, &["entrypoint=", "entrypoint: "]),
        trigger_id: extract_labeled_value(&message, &["trigger=", "trigger_id="]),
        step_index: extract_labeled_value(&message, &["step=", "step_index="])
            .and_then(|value| value.parse::<u32>().ok()),
        vm_pc: extract_labeled_value(&message, &["pc=", "vm_pc="])
            .and_then(|value| value.parse::<u64>().ok()),
        function: extract_labeled_value(&message, &["fn=", "function="]),
        source: extract_labeled_value(&message, &["src=", "source="]),
        opcode: extract_labeled_value(&message, &["opcode="]),
        syscall: extract_labeled_value(&message, &["syscall="]),
        raw_reason: Some(reason.to_string()),
    };
    if diagnostic.message.is_empty() {
        diagnostic.message = diagnostic
            .raw_reason
            .clone()
            .unwrap_or_else(|| "transaction rejected".to_owned());
        diagnostic.decoded_reason = Some(diagnostic.message.clone());
    }
    vec![diagnostic]
}

fn extract_labeled_value(message: &str, labels: &[&str]) -> Option<String> {
    for label in labels {
        let Some(offset) = message.find(label) else {
            continue;
        };
        let start = offset + label.len();
        let rest = &message[start..];
        let token = rest
            .split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';')
            .next()
            .unwrap_or_default()
            .trim_matches(['"', '\'', '`', '[', ']']);
        if !token.is_empty() {
            return Some(token.to_owned());
        }
    }
    None
}

/// Canonical account-read payload returned by `GET /v1/accounts/{account_id}`.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
pub struct AccountReadResponse {
    /// Canonical account identifier (domainless I105 literal).
    pub account_id: AccountId,
    /// Stable account label when assigned.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub label: Option<AccountAlias>,
    /// Universal account identifier bound to this account when registered in Nexus.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub uaid: Option<UniversalAccountId>,
    /// Opaque identifiers mapped to the account UAID.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub opaque_ids: Vec<OpaqueAccountId>,
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        ValidationFail,
        account::{AccountAlias, AccountAliasDomain, AccountId},
        name::Name,
        nexus::DataSpaceId,
        transaction::error::TransactionRejectionReason,
    };

    use super::{
        AccountReadResponse, ErrorDetails, ErrorEnvelope, MINAMOTO_CHAIN_DISCRIMINANT,
        NETWORK_PROFILE_MINAMOTO, NETWORK_PROFILE_TAIRA, PipelineTransactionStatus,
        PipelineTransactionStatusResponse, QueueErrorSnapshot, TAIRA_CHAIN_DISCRIMINANT,
        network_profile, network_profile_for_discriminant,
    };

    #[test]
    fn error_envelope_new_sets_fields() {
        let envelope = ErrorEnvelope::new("test_code", "test message");
        assert_eq!(envelope.code(), "test_code");
        assert_eq!(envelope.message(), "test message");
    }

    #[test]
    fn error_envelope_roundtrip_preserves_queue_details() {
        let envelope = ErrorEnvelope::new("queue_full", "transaction queue is at capacity")
            .with_details(ErrorDetails {
                reject_code: Some("TX_QUEUE_FULL".to_owned()),
                queue: Some(QueueErrorSnapshot {
                    state: "saturated".to_owned(),
                    queued: 24,
                    capacity: 24,
                    saturated: true,
                }),
                retry_after_seconds: Some(1),
                ..Default::default()
            });
        let bytes = norito::to_bytes(&envelope).expect("encode error envelope");
        let decoded: ErrorEnvelope =
            norito::decode_from_bytes(&bytes).expect("decode error envelope");
        assert_eq!(decoded.code, "queue_full");
        assert_eq!(decoded.message, "transaction queue is at capacity");
        let details = decoded.details.expect("error details");
        assert_eq!(details.reject_code.as_deref(), Some("TX_QUEUE_FULL"));
        assert_eq!(details.retry_after_seconds, Some(1));
        let queue = details.queue.expect("queue details");
        assert_eq!(queue.state, "saturated");
        assert_eq!(queue.queued, 24);
        assert_eq!(queue.capacity, 24);
        assert!(queue.saturated);
    }

    #[test]
    fn error_envelope_roundtrip_preserves_public_debug_details() {
        let envelope = ErrorEnvelope::new("transaction_finality_failed", "finality failed")
            .with_details(ErrorDetails {
                layer: Some("mcp".to_owned()),
                endpoint: Some("/v1/pipeline/transactions/status".to_owned()),
                field: Some("status.kind".to_owned()),
                expected: Some("Applied".to_owned()),
                actual: Some("Rejected".to_owned()),
                profile: Some(NETWORK_PROFILE_TAIRA.to_owned()),
                chain_discriminant: Some(TAIRA_CHAIN_DISCRIMINANT),
                tx_hash: Some("ab".repeat(32)),
                last_status: Some("Rejected".to_owned()),
                hint: Some("inspect transaction rejection reason".to_owned()),
                ..Default::default()
            });
        let bytes = norito::to_bytes(&envelope).expect("encode error envelope");
        let decoded: ErrorEnvelope =
            norito::decode_from_bytes(&bytes).expect("decode error envelope");
        let details = decoded.details.expect("error details");
        assert_eq!(details.layer.as_deref(), Some("mcp"));
        assert_eq!(
            details.endpoint.as_deref(),
            Some("/v1/pipeline/transactions/status")
        );
        assert_eq!(details.expected.as_deref(), Some("Applied"));
        assert_eq!(details.actual.as_deref(), Some("Rejected"));
        assert_eq!(details.profile.as_deref(), Some(NETWORK_PROFILE_TAIRA));
        assert_eq!(details.chain_discriminant, Some(TAIRA_CHAIN_DISCRIMINANT));
        assert_eq!(details.last_status.as_deref(), Some("Rejected"));
        assert_eq!(
            details.hint.as_deref(),
            Some("inspect transaction rejection reason")
        );
    }

    #[test]
    fn error_details_is_empty_tracks_optional_fields() {
        let mut details = ErrorDetails::default();
        assert!(details.is_empty());
        details.queue = Some(QueueErrorSnapshot {
            state: "saturated".to_owned(),
            queued: 24,
            capacity: 24,
            saturated: true,
        });
        assert!(!details.is_empty());
        details = ErrorDetails::default();
        details.last_status = Some("Expired".to_owned());
        assert!(!details.is_empty());
    }

    #[test]
    fn network_profile_registry_resolves_public_discriminants() {
        assert_eq!(
            network_profile(NETWORK_PROFILE_TAIRA).map(|profile| profile.chain_discriminant),
            Some(TAIRA_CHAIN_DISCRIMINANT)
        );
        assert_eq!(
            network_profile(NETWORK_PROFILE_MINAMOTO).map(|profile| profile.chain_discriminant),
            Some(MINAMOTO_CHAIN_DISCRIMINANT)
        );
        assert_eq!(
            network_profile_for_discriminant(TAIRA_CHAIN_DISCRIMINANT).map(|profile| profile.name),
            Some(NETWORK_PROFILE_TAIRA)
        );
    }

    #[test]
    fn pipeline_transaction_status_roundtrip_preserves_typed_rejection_reason() {
        let payload = PipelineTransactionStatusResponse::new(
            "ab".repeat(32),
            PipelineTransactionStatus {
                kind: "Rejected".to_owned(),
                block_height: Some(42),
                rejection_reason: Some(TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted("denied".to_owned()),
                )),
            },
            "auto".to_owned(),
            "state".to_owned(),
        );

        let encoded = norito::to_bytes(&payload).expect("encode status payload");
        let decoded: PipelineTransactionStatusResponse =
            norito::decode_from_bytes(&encoded).expect("decode status payload");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn pipeline_transaction_status_populates_diagnostics_from_rejection_reason() {
        let payload = PipelineTransactionStatusResponse::new(
            "cd".repeat(32),
            PipelineTransactionStatus {
                kind: "Rejected".to_owned(),
                block_height: Some(77),
                rejection_reason: Some(TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted("missing permission".to_owned()),
                )),
            },
            "global".to_owned(),
            "state".to_owned(),
        );

        assert!(payload.summary.contains("Rejected"));
        assert_eq!(payload.diagnostics.len(), 1);
        assert_eq!(payload.diagnostics[0].category, "validation");
        assert_eq!(payload.diagnostics[0].code.as_deref(), Some("validation"));
        assert!(
            payload.diagnostics[0]
                .message
                .contains("missing permission")
        );
        assert!(
            payload.diagnostics[0]
                .decoded_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("missing permission"))
        );
        assert!(payload.diagnostics[0].raw_reason.is_some());
    }

    #[test]
    fn account_read_response_roundtrip_preserves_subject_metadata() {
        let key_pair = iroha_crypto::KeyPair::random();
        let response = AccountReadResponse {
            account_id: AccountId::new(key_pair.public_key().clone()),
            label: Some(AccountAlias::new(
                "alice".parse::<Name>().expect("valid label"),
                Some(AccountAliasDomain::new(
                    "wonderland".parse::<Name>().expect("valid alias domain"),
                )),
                DataSpaceId::UNIVERSAL,
            )),
            uaid: None,
            opaque_ids: Vec::new(),
        };

        let encoded = norito::to_bytes(&response).expect("encode account response");
        let decoded: AccountReadResponse =
            norito::decode_from_bytes(&encoded).expect("decode account response");
        assert_eq!(decoded, response);
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
