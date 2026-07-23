//! Constant values used in Torii that might be re-used by client libraries as well.
use iroha_data_model::{
    account::{AccountAlias, AccountId, OpaqueAccountId},
    asset::AssetDefinitionId,
    nexus::{DataSpaceId, FeeDebitSource, FeeSponsorProgramId, UniversalAccountId},
    prelude::Quantity,
    transaction::{
        FeeChargeKind, FeePaymentIntent, TransactionPayload, error::TransactionRejectionReason,
    },
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Shared data-availability helpers (sampling, assignment).
pub mod da;
/// Public Torii DTOs for the offline cash lifecycle.
pub mod offline_api;
/// Shared QR Code encoder used by Torii and CLI offline flows.
pub mod qr;
/// Canonical Torii route metadata and projection helpers.
pub mod route_catalog;
/// Public Torii DTOs for Parliament-governed validation-fee policy state.
pub mod validation_fee_api;

/// Required WebSocket subprotocol for canonical Norito event and block streams.
pub const NORITO_V1_WEBSOCKET_SUBPROTOCOL: &str = "iroha-norito-v1";

/// Canonical request body for account-signed `POST /v1/fees/quote`.
///
/// The draft fixes every non-fee transaction field and selects the payer,
/// sponsor-program revision, and gas bound. Before signing, replace only its
/// `fee_payment` field with the response's fixed-point [`FeePaymentIntent`].
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct FeeQuoteRequest {
    /// Exact canonical unsigned transaction payload to evaluate.
    pub payload: TransactionPayload,
}

/// Ledger observation used for a deterministic fee quote.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
pub struct FeeQuoteObservation {
    /// Creation time of the latest committed block, in Unix milliseconds.
    pub ledger_time_ms: u64,
    /// Height at which this payload would next be admitted.
    pub next_block_height: u64,
    /// Dataspace selected by canonical transaction routing.
    pub route_dataspace_id: DataSpaceId,
}

/// One canonically ordered maximum fee component.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
pub struct FeeQuoteComponent {
    /// Fee component represented by this bound.
    pub kind: FeeChargeKind,
    /// Exact asset definition in which this component is charged.
    pub asset_definition_id: AssetDefinitionId,
    /// Maximum deterministic charge at the observed state.
    pub max_amount: Quantity,
}

/// Remaining sponsor-program capacity for one fee asset.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
pub struct FeeQuoteCapacity {
    /// Asset definition governed by this capacity snapshot.
    pub asset_definition_id: AssetDefinitionId,
    /// Current isolated program-vault balance.
    pub vault_balance: Quantity,
    /// Balance that must remain after settlement.
    pub reserve_floor: Quantity,
    /// Remaining program capacity in the observed block window.
    pub block_remaining: Quantity,
    /// Remaining program capacity in the observed program epoch.
    pub program_epoch_remaining: Quantity,
    /// Remaining beneficiary capacity in the observed beneficiary epoch.
    pub beneficiary_epoch_remaining: Quantity,
}

/// Successful deterministic fee-admission decision.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(tag = "status", content = "value", rename_all = "snake_case")]
pub enum FeeQuoteDecision {
    /// The payload is admissible at the observed state.
    #[norito(rename = "accepted")]
    Accepted {
        /// Exact account or isolated program vault that will be debited.
        debit_source: FeeDebitSource,
        /// Exact active immutable sponsor revision, when sponsored.
        #[norito(default)]
        #[norito(skip_serializing_if = "Option::is_none")]
        program_revision: Option<u64>,
    },
}

/// Successful response returned by account-signed `POST /v1/fees/quote`.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
pub struct FeeQuoteResponse {
    /// Exact signature-bound intent evaluated by Core.
    pub intent: FeePaymentIntent,
    /// Ledger observation that fixes the state-dependent result.
    pub observation: FeeQuoteObservation,
    /// Canonically ordered maximum charge components.
    pub components: Vec<FeeQuoteComponent>,
    /// Canonically ordered sponsor capacities; empty for authority payment.
    pub capacities: Vec<FeeQuoteCapacity>,
    /// Successful admission decision and selected debit source.
    pub decision: FeeQuoteDecision,
}

/// Canonical request body for exact sponsor-program lookup.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct FeeSponsorProgramByIdRequest {
    /// Canonical `sponsor/program` literal.
    pub program_id: String,
}

impl FeeSponsorProgramByIdRequest {
    /// Construct a lookup request from a typed program identifier.
    #[must_use]
    pub fn new(program_id: &FeeSponsorProgramId) -> Self {
        Self {
            program_id: program_id.to_string(),
        }
    }
}

pub mod uri {
    //! URI that Torii uses to route incoming requests.

    /// Query URI is used to handle incoming Query requests.
    pub const QUERY: &str = "/v1/query";
    /// URI used to evaluate offline-payment readiness.
    pub const OFFLINE_READINESS: &str = crate::route_catalog::offline::READINESS_PATH;
    /// URI used to resolve proof-bearing active receiver registration lineage.
    pub const OFFLINE_RECIPIENT_LINEAGE: &str =
        crate::route_catalog::offline::RECIPIENT_LINEAGE_PATH;
    /// URI used to submit an online-to-offline top-up operation.
    pub const OFFLINE_TOP_UP: &str = crate::route_catalog::offline::TOP_UP_PATH;
    /// URI used to submit an offline redemption operation.
    pub const OFFLINE_REDEEM: &str = crate::route_catalog::offline::REDEEM_PATH;
    /// URI used to fetch a finality-bound current validation-fee registry.
    pub const VALIDATION_FEE_CURRENT_POLICY_PROOF: &str =
        crate::route_catalog::runtime_governance::VALIDATION_FEE_CURRENT_POLICY_PROOF_PATH;
    /// URI used to list typed validation-fee Parliament proposals.
    pub const VALIDATION_FEE_PROPOSALS: &str =
        crate::route_catalog::runtime_governance::VALIDATION_FEE_PROPOSALS_PATH;
    /// URI template used to fetch one typed validation-fee Parliament proposal.
    pub const VALIDATION_FEE_PROPOSAL_DETAIL: &str =
        crate::route_catalog::runtime_governance::VALIDATION_FEE_PROPOSAL_DETAIL_PATH;
    /// URI used to draft one strict native validation-fee Parliament proposal.
    pub const VALIDATION_FEE_PROPOSAL_DRAFT: &str =
        crate::route_catalog::runtime_governance::VALIDATION_FEE_PROPOSAL_DRAFT_PATH;
    /// URI used to fetch an offline operation by ID.
    pub const OFFLINE_OPERATION: &str = crate::route_catalog::offline::OPERATION_PATH;
    /// Transaction URI is used to handle incoming signed transaction requests.
    pub const TRANSACTION: &str = "/v1/pipeline/transactions";
    /// Transaction entrypoint URI is used to handle sealed and non-external submissions.
    pub const TRANSACTION_ENTRYPOINT: &str = "/v1/pipeline/transaction-entrypoints";
    /// Batched transaction URI is used to handle multiple signed transaction submissions.
    pub const TRANSACTIONS_BATCH: &str = "/v1/pipeline/transactions/batch";
    /// URI used to quote the exact fee intent of an unsigned transaction payload.
    pub const FEES_QUOTE: &str = crate::route_catalog::fees::QUOTE_PATH;
    /// URI used to read one exact on-chain sponsor program.
    pub const FEE_SPONSOR_PROGRAM_BY_ID: &str =
        crate::route_catalog::fees::SPONSOR_PROGRAM_BY_ID_PATH;
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
    /// URI for reading the Nexus lifecycle catalog commitment.
    ///
    /// Lifecycle changes are signed `SetParameter` transactions; no HTTP
    /// mutation or compatibility route is mounted at this path.
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
    /// Draft one closed SCCP route-governance proposal.
    pub const GOV_PROPOSE_SCCP_ROUTE_GOVERNANCE: &str = "/v1/gov/proposals/sccp-route-governance";
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

/// Structured fee-payment rejection metadata returned to authorized callers.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, Default,
)]
pub struct FeeErrorDetails {
    /// Stable snake-case [`iroha_data_model::nexus::FeeRejectionCode`] label.
    pub code: String,
    /// Whether retrying after a state or capacity change can succeed unchanged.
    pub retryable: bool,
    /// Exact sponsor program selected by the signed transaction, when applicable.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub program_id: Option<String>,
    /// Exact sponsor-program revision selected by the signed transaction.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub program_revision: Option<u64>,
    /// Canonical fee asset involved in the rejection.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub asset_definition_id: Option<String>,
    /// Deterministic amount required at the observation state.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub required: Option<String>,
    /// Deterministic amount available at the observation state.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub available: Option<String>,
    /// Stable matching rule identifier, when disclosure is authorized.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    /// Consensus height at which this decision was evaluated.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub observation_height: Option<u64>,
    /// Concrete repair or retry guidance.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
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
    /// Fee-payment rejection metadata when admission or settlement failed.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub fee: Option<FeeErrorDetails>,
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
            && self.fee.is_none()
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
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ValidationFail,
        account::{AccountAlias, AccountAliasDomain, AccountId},
        name::Name,
        nexus::{DataSpaceId, FeeDebitSource, FeeSponsorProgramId},
        transaction::{FeePaymentIntent, error::TransactionRejectionReason},
    };

    use super::{
        AccountReadResponse, ErrorDetails, ErrorEnvelope, FeeErrorDetails, FeeQuoteDecision,
        FeeQuoteObservation, FeeQuoteResponse, FeeSponsorProgramByIdRequest,
        MINAMOTO_CHAIN_DISCRIMINANT, NETWORK_PROFILE_MINAMOTO, NETWORK_PROFILE_TAIRA,
        PipelineTransactionStatus, PipelineTransactionStatusResponse, QueueErrorSnapshot,
        TAIRA_CHAIN_DISCRIMINANT, network_profile, network_profile_for_discriminant,
    };

    fn checked_test_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("Torii shared test fixture key derivation should succeed")
    }

    #[test]
    fn error_envelope_new_sets_fields() {
        let envelope = ErrorEnvelope::new("test_code", "test message");
        assert_eq!(envelope.code(), "test_code");
        assert_eq!(envelope.message(), "test message");
    }

    #[test]
    fn fee_program_selector_and_quote_response_roundtrip_canonically() {
        let account = AccountId::new(checked_test_keypair(0x24).public_key().clone());
        let program_id = FeeSponsorProgramId::new(
            account.clone(),
            "retail".parse().expect("canonical sponsor-program name"),
        );
        let selector = FeeSponsorProgramByIdRequest::new(&program_id);
        let selector_json = norito::json::to_vec(&selector).expect("encode program selector");
        let decoded_selector: FeeSponsorProgramByIdRequest =
            norito::json::from_slice(&selector_json).expect("decode program selector");
        assert_eq!(decoded_selector, selector);
        assert_eq!(decoded_selector.program_id, program_id.to_string());

        let quote = FeeQuoteResponse {
            intent: FeePaymentIntent::authority(Vec::new(), None),
            observation: FeeQuoteObservation {
                ledger_time_ms: 42,
                next_block_height: 7,
                route_dataspace_id: DataSpaceId::UNIVERSAL,
            },
            components: Vec::new(),
            capacities: Vec::new(),
            decision: FeeQuoteDecision::Accepted {
                debit_source: FeeDebitSource::Account(account),
                program_revision: None,
            },
        };
        let quote_json = norito::json::to_vec(&quote).expect("encode typed fee quote");
        let decoded_quote: FeeQuoteResponse =
            norito::json::from_slice(&quote_json).expect("decode typed fee quote");
        assert_eq!(decoded_quote, quote);
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
    fn error_envelope_roundtrip_preserves_fee_details() {
        let envelope = ErrorEnvelope::new("fee_payment_rejected", "fee payment rejected")
            .with_details(ErrorDetails {
                fee: Some(FeeErrorDetails {
                    code: "vault_insufficient".to_owned(),
                    retryable: true,
                    program_id: Some("sponsor/default".to_owned()),
                    program_revision: Some(3),
                    asset_definition_id: Some("xor#fees".to_owned()),
                    required: Some("10".to_owned()),
                    available: Some("4".to_owned()),
                    observation_height: Some(42),
                    remediation: Some("fund the program vault".to_owned()),
                    ..Default::default()
                }),
                ..Default::default()
            });

        let bytes = norito::to_bytes(&envelope).expect("encode fee error envelope");
        let decoded: ErrorEnvelope =
            norito::decode_from_bytes(&bytes).expect("decode fee error envelope");
        let fee = decoded
            .details
            .and_then(|details| details.fee)
            .expect("fee error details");
        assert_eq!(fee.code, "vault_insufficient");
        assert!(fee.retryable);
        assert_eq!(fee.program_revision, Some(3));
        assert_eq!(fee.required.as_deref(), Some("10"));
        assert_eq!(fee.available.as_deref(), Some("4"));
        assert_eq!(fee.observation_height, Some(42));
    }

    #[test]
    fn error_envelope_json_discards_unknown_members_and_rejects_duplicates() {
        let decoded: ErrorEnvelope = norito::json::from_str(
            r#"{"code":"bad_request","message":"invalid","unknown":"discarded","details":{"field":"amount","unknown_nested":{"secret":true}}}"#,
        )
        .expect("decode envelope with independently additive members");
        assert_eq!(decoded.code(), "bad_request");
        assert_eq!(
            decoded
                .details
                .as_ref()
                .and_then(|details| details.field.as_deref()),
            Some("amount")
        );
        let canonical = norito::json::to_string(&decoded).expect("re-encode closed envelope");
        assert!(!canonical.contains("unknown"));
        assert!(!canonical.contains("secret"));

        for json in [
            r#"{"code":"bad_request","code":"conflict","message":"invalid"}"#,
            r#"{"code":"bad_request","message":"invalid","details":{"field":"amount","field":"asset"}}"#,
        ] {
            let error = norito::json::from_str::<ErrorEnvelope>(json)
                .expect_err("duplicate declared error member must fail");
            assert!(
                error.to_string().contains("duplicate field"),
                "unexpected duplicate-member error: {error}"
            );
        }
    }

    #[test]
    fn error_envelope_json_rejects_untyped_details_values() {
        for details in ["[]", "\"dynamic\"", "1", "true"] {
            let json =
                format!(r#"{{"code":"bad_request","message":"invalid","details":{details}}}"#);
            norito::json::from_str::<ErrorEnvelope>(&json)
                .expect_err("details must be the declared typed record or null");
        }
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
        details = ErrorDetails::default();
        details.fee = Some(FeeErrorDetails {
            code: "invalid_fee_intent".to_owned(),
            ..Default::default()
        });
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
    fn account_read_response_fixture_uses_checked_key_derivation() {
        let key_pair = checked_test_keypair(0x23);
        let expected = KeyPair::try_from_seed(vec![0x23; 32], Algorithm::Ed25519)
            .expect("direct checked Torii shared fixture key derivation");

        assert_eq!(key_pair.public_key(), expected.public_key());
        assert!(KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err());
    }

    #[test]
    fn account_read_response_roundtrip_preserves_subject_metadata() {
        let key_pair = checked_test_keypair(0x23);
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
