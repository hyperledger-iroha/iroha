//! Individually indexed gateway rows and bounded atomic transition deltas.
//!
//! Admission, acknowledgement, context, token identity, lease grant and terminal rows are permanent history.
//! Only live quota rows and their expiry indexes are retired. A finite cache must never replace
//! the permanent context index: forgetting it would permit quota and callback replay.
//! Decoded rows and heads are claims until a source-bound World reader authenticates them;
//! these storage types do not prove signed execution or native finality.

use iroha_data_model::sorafs::{
    capacity::ProviderId,
    stream_token_gateway::{
        StreamTokenGatewayAdmissionAckV1, StreamTokenGatewayAdmissionRecordV1,
        StreamTokenGatewayAdmissionRequestV1, StreamTokenGatewayAdmissionResultV1,
        native::StreamTokenGatewayExecutionV1,
    },
};
use norito::codec::{Decode, Encode};

pub(crate) use iroha_data_model::sorafs::stream_token_gateway::native::STREAM_TOKEN_GATEWAY_MAX_EXPIRY_ITEMS_V1 as MAX_EXPIRY_ITEMS;
/// Maximum individual row changes returned by a transition.
pub(crate) const MAX_TRANSITION_WRITES: usize = 4 * MAX_EXPIRY_ITEMS as usize;

/// Small per-gateway mutable head; historical rows are independently indexed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::query::stream_token_gateway::GatewayHeadV1")]
pub(crate) struct GatewayHeadV1 {
    /// Strictly increasing revision for transitions that actually change state.
    pub revision: u64,
    /// Last allocated callback sequence, never reset by policy rotation or expiry.
    pub high_water_sequence: u64,
    /// Exact contiguous prefix durably handed to the reputation owner.
    pub acknowledged_through_sequence: u64,
    /// Number of live quota rows, independent of permanent token identities.
    pub live_tokens: u32,
    /// Latest deterministic execution time of a state-changing transition.
    pub last_execution_unix_ms: u64,
}

/// Exact original admission and its execution-derived provenance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::query::stream_token_gateway::AdmissionRowV1")]
pub(crate) struct AdmissionRowV1 {
    /// Original complete validation attestation, including its original timestamp.
    pub request: StreamTokenGatewayAdmissionRequestV1,
    /// Byte-identical callback result and original policy snapshot.
    pub record: StreamTokenGatewayAdmissionRecordV1,
    /// Actual direct signed execution, supplied by the native executor.
    pub execution: StreamTokenGatewayExecutionV1,
}

/// Permanent first ordered callback acknowledgement, never replaced by later retries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::query::stream_token_gateway::AcknowledgementRowV1")]
pub(crate) struct AcknowledgementRowV1 {
    /// Exact retained admission and sequence handed to the reputation owner.
    pub record: StreamTokenGatewayAdmissionRecordV1,
    /// Original governed policy revision authorizing this acknowledgement.
    pub policy_revision: u64,
    /// Actual first successful native acknowledgement execution.
    pub execution: StreamTokenGatewayExecutionV1,
    /// Exact source-bound native delivery disposition; an operator assertion alone cannot acknowledge.
    pub reputation_delivery:
        crate::smartcontracts::isi::sorafs_reputation::stream_token_delivery::DeliveryState,
}

/// Permanent exact-context replay index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::query::stream_token_gateway::ContextRowV1")]
pub(crate) struct ContextRowV1 {
    /// Original gateway sequence.
    pub sequence: u64,
    /// Domain-separated digest of the complete canonical original request.
    pub request_digest: [u8; 32],
}

/// Permanent signed token identity, first pinned only by an Accepted Torii attestation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::query::stream_token_gateway::TokenIdentityV1")]
pub(crate) struct TokenIdentityV1 {
    /// Authoritative serving provider.
    pub provider_id: ProviderId,
    /// Canonical token identifier within this provider and gateway.
    pub token_id: String,
    /// Exact canonical signed token body digest.
    pub body_digest: [u8; 32],
    /// Exact signing-key version.
    pub key_version: u32,
    /// Signed concurrent-stream bound.
    pub max_streams: u16,
    /// Signed request-window allowance.
    pub requests_per_minute: u32,
    /// Signed byte-window allowance.
    pub rate_limit_bytes: u64,
    /// Signed token expiry in Unix milliseconds.
    pub expires_at_unix_ms: u64,
}

/// Bounded live counters; their clocks originate in native execution, never the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::query::stream_token_gateway::QuotaRowV1")]
pub(crate) struct QuotaRowV1 {
    /// Exact permanent lifecycle generation of this active quota row.
    pub generation: u64,
    /// First accepted execution in the current sixty-second window.
    pub request_window_start_ms: u64,
    /// Accepted requests in that window.
    pub requests_used: u32,
    /// First accepted execution in the current one-second byte window.
    pub byte_window_start_ms: u64,
    /// Accepted bytes in that window.
    pub bytes_used: u64,
    /// Unreleased and unexpired leases according to the committed expiry index.
    pub active_leases: u16,
    /// Conservative time after which both windows and every lease are irrelevant.
    pub retire_at_unix_ms: u64,
}

/// Permanent token lifecycle distinguishes legitimate retirement from missing live counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::query::stream_token_gateway::QuotaLifecycleV1")]
pub(crate) struct QuotaLifecycleV1 {
    /// Never-reused quota incarnation, zero before first successful quota admission.
    pub generation: u64,
    /// Exactly one live quota row exists if and only if this is true.
    pub active: bool,
}

/// Immutable accepted concurrency grant, retained after release or expiry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::query::stream_token_gateway::LeaseRowV1")]
pub(crate) struct LeaseRowV1 {
    /// Original admission sequence.
    pub sequence: u64,
    /// Domain-separated provider/token identity key.
    pub token_scope: [u8; 32],
    /// Exact quota incarnation whose live concurrency counter contains this grant.
    pub quota_generation: u64,
    /// Original immutable exclusive deadline.
    pub expires_at_unix_ms: u64,
}

/// Permanent terminal fact for a lease; either release or expiry prevents reacquisition.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::query::stream_token_gateway::LeaseTerminalV1")]
pub(crate) struct LeaseTerminalV1 {
    /// Original governed policy revision authorizing release or maintenance.
    pub policy_revision: u64,
    /// Exact original immutable grant, repeated to bind terminal history to its row.
    pub grant: LeaseRowV1,
    /// True for deterministic expiry, false for explicit early release.
    pub expired: bool,
    /// Actual native execution that terminalized the grant.
    pub execution: StreamTokenGatewayExecutionV1,
}

/// Ordered expiry target; leases sort before quota retirement at equal deadlines.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::query::stream_token_gateway::GatewayExpiryTargetV1")]
pub(crate) enum GatewayExpiryTargetV1 {
    /// Exact immutable lease identifier.
    Lease([u8; 32]),
    /// Exact live provider/token quota scope.
    Quota([u8; 32]),
}

/// Authenticated ordered expiry index key.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::query::stream_token_gateway::GatewayExpiryKeyV1")]
pub(crate) struct GatewayExpiryKeyV1 {
    /// Exclusive original expiry or conservative quota retirement time.
    pub at_unix_ms: u64,
    /// Exact indexed row identity.
    pub target: GatewayExpiryTargetV1,
}

/// Gateway-local storage key. The World adapter prepends the independently bound gateway.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum GatewayRowKey {
    /// Permanent original admission.
    Admission(u64),
    /// Permanent original callback acknowledgement indexed by admission sequence.
    Acknowledgement(u64),
    /// Permanent exact request-context index.
    Context([u8; 32]),
    /// Permanent signed token identity.
    TokenIdentity([u8; 32]),
    /// Permanent active/retired quota lifecycle; never removed during live-row retirement.
    QuotaLifecycle([u8; 32]),
    /// Live bounded token counters.
    Quota([u8; 32]),
    /// Permanent original lease grant.
    Lease([u8; 32]),
    /// Permanent lease terminal fact.
    LeaseTerminal([u8; 32]),
    /// Ordered live expiry marker.
    Expiry(GatewayExpiryKeyV1),
}

/// Typed individually encoded row. A mismatched key/variant is corrupt history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GatewayRow {
    /// Permanent original admission.
    Admission(AdmissionRowV1),
    /// Permanent exact first acknowledgement.
    Acknowledgement(AcknowledgementRowV1),
    /// Permanent exact-context index.
    Context(ContextRowV1),
    /// Permanent signed token identity.
    TokenIdentity(TokenIdentityV1),
    /// Permanent active/retired quota lifecycle.
    QuotaLifecycle(QuotaLifecycleV1),
    /// Live bounded token counters.
    Quota(QuotaRowV1),
    /// Permanent original lease grant.
    Lease(LeaseRowV1),
    /// Permanent lease terminal fact.
    LeaseTerminal(LeaseTerminalV1),
    /// Marker repeats its exact key to detect substitution.
    Expiry(GatewayExpiryKeyV1),
}

/// Payload-free deterministic transition error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransitionError {
    /// Malformed request or arithmetic bound.
    Invalid,
    /// Policy or request does not bind this gateway.
    BindingMismatch,
    /// Stored rows or indexes disagree.
    CorruptHistory,
    /// A permanent replay identity conflicts.
    Conflict,
    /// Live capacity is exhausted; no row is fabricated.
    Capacity,
    /// Due expiry work must be committed before new quota admission.
    MaintenanceRequired,
    /// Original observation, token or lease is no longer usable.
    Unavailable,
}

/// Source-bound, lazy reads from one authoritative State transaction view.
///
/// `expiry_prefix` MUST return the exact sorted due prefix, up to the requested limit, using
/// an authenticated ordered World range. An empty or short result claims complete absence of
/// any further due key; an external caller, finite cache or filtered iterator cannot supply it.
/// The adapter must bound each canonical row decode and reject other gateway namespaces.
pub(crate) trait GatewayRows {
    /// Read exactly one gateway-local typed row.
    fn read(&self, key: &GatewayRowKey) -> Result<Option<GatewayRow>, TransitionError>;
    /// Read the earliest due keys, never more than `max_items` (at most 257).
    fn expiry_prefix(
        &self,
        now_unix_ms: u64,
        max_items: u32,
    ) -> Result<Vec<GatewayExpiryKeyV1>, TransitionError>;
}

/// One atomic compare-and-swap row change, fully prepared before World publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GatewayRowWrite {
    /// Exact gateway-local row key.
    pub key: GatewayRowKey,
    /// Exact source-view value expected before publication.
    pub before: Option<GatewayRow>,
    /// Replacement, or removal of a live row/index only.
    pub after: Option<GatewayRow>,
}

/// Result of a successful deterministic action, not proof of finalized execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TransitionResult {
    /// Original immutable admission result.
    Admission(StreamTokenGatewayAdmissionResultV1),
    /// Exact ordered callback acknowledgement.
    Acknowledged(StreamTokenGatewayAdmissionAckV1),
    /// Exact lease release or already terminal grant.
    Released(StreamTokenGatewayAdmissionAckV1),
    /// Number of expiry markers consumed.
    Expired(u32),
}

/// Complete bounded delta. Integration must encode and validate every CAS before mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TransitionDelta {
    /// Source-view head expected by the native executor.
    pub before: GatewayHeadV1,
    /// New head, unchanged for exact replay and empty maintenance.
    pub after: GatewayHeadV1,
    /// Deterministically key-ordered independent row changes.
    pub writes: Vec<GatewayRowWrite>,
    /// Typed action result authenticated only by the subsequent native proof reader.
    pub result: TransitionResult,
}
