//! Torii client utilities used by MOCHI.
//!
//! The client focuses on generating canonical endpoints and providing async
//! helpers for common HTTP and WebSocket interactions. UI layers can build on
//! top by wiring retries, auth, and payload codecs.
use crate::compose::{InstructionPermission, SigningAuthority};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
};
use futures::{SinkExt, future::join_all};
use iroha_crypto::{HashOf, KeyPair};
use iroha_data_model::{
    Identifiable,
    block::{
        SignedBlock,
        consensus::SumeragiDiagnosticsStatus,
        consensus_v2::SumeragiV2Status,
        stream::{BlockMessage, BlockSubscriptionRequest},
    },
    events::{
        EventBox, EventFilterBox,
        data::{DataEvent, DataEventFilter, prelude::*, sorafs},
        execute_trigger::ExecuteTriggerEventFilter,
        pipeline::{
            BlockEventFilter, MergeLedgerEventFilter, PipelineEventBox, TransactionEventFilter,
            WitnessEventFilter,
        },
        stream::{EventMessage, EventSubscriptionRequest},
        time::{ExecutionTime, TimeEventFilter},
        trigger_completed::TriggerCompletedEventFilter,
    },
    isi::{SetKeyValue, SetParameter},
    nexus::{LaneLifecycleParameterV1, LaneLifecyclePlan, LaneLifecycleStatusV1},
    parameter::Parameter,
    prelude::{AccountId, NetworkId},
    query::{QueryOutput, QueryRequest, SignedQuery},
    transaction::{SignedTransaction, TransactionBuilder, TransactionEntrypoint},
};
use iroha_primitives::json::Json;
use iroha_telemetry::metrics::Status as TelemetryStatus;
pub use iroha_telemetry::metrics::{GovernanceStatus, Uptime};
use iroha_torii_shared::{
    NORITO_V1_WEBSOCKET_SUBPROTOCOL, route_catalog as torii_routes, uri as torii_uri,
};
use iroha_version::codec::EncodeVersioned;
use norito::json;
use rand::{TryRngCore as _, rngs::OsRng};
use reqwest::{
    Client, Response, StatusCode,
    header::{HeaderMap, HeaderName, HeaderValue, SEC_WEBSOCKET_PROTOCOL},
};
use std::{
    convert::TryFrom,
    future::Future,
    io::Cursor,
    num::{NonZeroU32, NonZeroU64},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    net::TcpStream,
    runtime::Handle,
    sync::{
        Mutex,
        broadcast::{self, error::RecvError},
        watch,
    },
    task::JoinHandle,
    time::{MissedTickBehavior, sleep},
};
use tokio_stream::StreamExt;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Error as WebSocketError, Message, client::IntoClientRequest},
};
use url::Url;
mod operator_auth;
pub use operator_auth::OperatorSigningContext;
use operator_auth::build_operator_get_request;
include!("torii/sumeragi_response_bounds.rs");
/// Convenience result alias for Torii client operations.
pub type ToriiResult<T> = std::result::Result<T, ToriiError>;
/// Errors emitted by the Torii client.
#[derive(thiserror::Error, Debug)]
pub enum ToriiError {
    /// Base URL could not be parsed.
    #[error("invalid Torii base URL: {0}")]
    InvalidBaseUrl(url::ParseError),
    /// Endpoint URL composition failed.
    #[error("invalid Torii endpoint URL: {0}")]
    InvalidEndpoint(url::ParseError),
    /// A scheme other than HTTP(S) was supplied.
    #[error("unsupported Torii URL scheme `{scheme}`")]
    UnsupportedScheme { scheme: String },
    /// HTTP-level failure when talking to Torii.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// A response exceeded its route-specific retained-body budget or could
    /// not reserve memory within that budget.
    #[error("Torii {context} response exceeded its {maximum}-byte resource budget")]
    ResponseResourceLimit {
        /// Stable route family used for diagnostics without reflecting a URL.
        context: &'static str,
        /// Maximum complete response body admitted for the route family.
        maximum: usize,
    },
    /// Torii answered with a non-success status code.
    #[error("unexpected Torii status code {status}")]
    UnexpectedStatus {
        /// HTTP status code returned by Torii.
        status: StatusCode,
        /// Optional Torii reject code header value.
        reject_code: Option<String>,
        /// Optional error message decoded from the response body.
        message: Option<String>,
    },
    /// Torii throttled the request and may have supplied a retry delay.
    #[error("Torii request was rate limited")]
    RateLimited {
        /// Server-provided `Retry-After` delay, when it was a valid delta in seconds.
        retry_after: Option<Duration>,
    },
    /// Builder received an invalid header value.
    #[error("invalid HTTP header `{name}`: {source}")]
    InvalidHeader {
        /// Name of the header that failed to parse.
        name: String,
        /// Concrete header parse error.
        #[source]
        source: reqwest::header::InvalidHeaderValue,
    },
    /// WebSocket negotiation failed.
    #[error("websocket error: {0}")]
    WebSocket(#[from] WebSocketError),
    /// Constructed WebSocket request was invalid.
    #[error("invalid websocket request: {0}")]
    InvalidWebSocketRequest(String),
    /// Norito decoding failed.
    #[error("norito decode error: {0}")]
    Decode(String),
    /// Signed-query context could not be constructed safely.
    #[error("signed query context error: {0}")]
    SignedQueryContext(String),
    /// Timed out while waiting for Torii to produce a response.
    #[error("timeout while waiting for Torii: {context}")]
    Timeout { context: String },
    /// A smoke transaction was rejected (or expired) before commitment.
    #[error("smoke transaction {hash} rejected: {reason}")]
    SmokeRejected { hash: String, reason: String },
    /// Torii could not prove whether the exact smoke transaction was admitted.
    #[error("smoke transaction admission outcome remains unknown for {hash}")]
    SmokeAdmissionOutcomeUnknown { hash: String },
}
/// High-level classification for [`ToriiError`] variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToriiErrorKind {
    /// Invalid or unparsable base URL was supplied.
    InvalidBaseUrl,
    /// Derived endpoint URL was invalid.
    InvalidEndpoint,
    /// Unsupported URL scheme was supplied.
    UnsupportedScheme,
    /// Invalid HTTP header configuration.
    InvalidHeader,
    /// Transport-level HTTP failure before receiving a response body.
    HttpTransport,
    /// Torii response exceeded a route-specific memory budget.
    ResponseResourceLimit,
    /// Torii responded with an unexpected status code.
    UnexpectedStatus,
    /// WebSocket negotiation or framing failed.
    WebSocket,
    /// Constructed WebSocket request was invalid.
    InvalidWebSocketRequest,
    /// Norito payload decoding failed.
    Decode,
    /// Signed-query network, time, nonce, or signing context was unavailable.
    SignedQueryContext,
    /// Operation exceeded the configured timeout.
    Timeout,
    /// Smoke transaction was rejected or expired.
    SmokeRejected,
    /// Smoke transaction admission remained ambiguous after exact-hash reconciliation.
    SmokeAdmissionOutcomeUnknown,
}
/// Summary of a [`ToriiError`] capturing its user-facing message and kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToriiErrorInfo {
    /// Classified error kind.
    pub kind: ToriiErrorKind,
    /// Human-readable message suitable for UI surfaces.
    pub message: String,
    /// Optional detail string providing additional context (e.g., status code).
    pub detail: Option<String>,
    /// Optional Torii reject code attached to the response.
    pub reject_code: Option<String>,
}
impl ToriiErrorInfo {
    /// Construct a summary with no additional detail.
    #[must_use]
    pub fn new(kind: ToriiErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: None,
            reject_code: None,
        }
    }
    /// Construct a summary with an accompanying detail string.
    #[must_use]
    pub fn with_detail(
        kind: ToriiErrorKind,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: Some(detail.into()),
            reject_code: None,
        }
    }
}
impl ToriiError {
    /// Produce a classified summary of the error for display or logging purposes.
    #[must_use]
    pub fn summarize(&self) -> ToriiErrorInfo {
        match self {
            Self::InvalidBaseUrl(err) => ToriiErrorInfo::with_detail(
                ToriiErrorKind::InvalidBaseUrl,
                "Invalid Torii base URL",
                err.to_string(),
            ),
            Self::InvalidEndpoint(err) => ToriiErrorInfo::with_detail(
                ToriiErrorKind::InvalidEndpoint,
                "Invalid Torii endpoint URL",
                err.to_string(),
            ),
            Self::UnsupportedScheme { scheme } => ToriiErrorInfo::with_detail(
                ToriiErrorKind::UnsupportedScheme,
                "Unsupported Torii URL scheme",
                scheme.clone(),
            ),
            Self::Http(err) => ToriiErrorInfo::with_detail(
                ToriiErrorKind::HttpTransport,
                "HTTP transport error while contacting Torii",
                err.to_string(),
            ),
            Self::ResponseResourceLimit { context, maximum } => ToriiErrorInfo::with_detail(
                ToriiErrorKind::ResponseResourceLimit,
                "Torii response exceeded its memory budget",
                format!("{context} response limit: {maximum} bytes"),
            ),
            Self::UnexpectedStatus {
                status,
                reject_code,
                message,
            } => {
                let mut detail = status.to_string();
                if let Some(message) = message
                    && !message.is_empty()
                {
                    detail = format!("{detail} - {message}");
                }
                let mut info = ToriiErrorInfo::with_detail(
                    ToriiErrorKind::UnexpectedStatus,
                    format!("Unexpected Torii status code {status}"),
                    detail,
                );
                info.reject_code = reject_code.clone();
                info
            }
            Self::RateLimited { retry_after } => {
                let detail = retry_after.map_or_else(
                    || "HTTP 429 without a valid Retry-After hint".to_owned(),
                    |delay| format!("HTTP 429; retry after {delay:?}"),
                );
                ToriiErrorInfo::with_detail(
                    ToriiErrorKind::UnexpectedStatus,
                    "Torii request was rate limited",
                    detail,
                )
            }
            Self::InvalidHeader { name, source } => ToriiErrorInfo::with_detail(
                ToriiErrorKind::InvalidHeader,
                format!("Invalid HTTP header `{name}`"),
                source.to_string(),
            ),
            Self::WebSocket(err) => ToriiErrorInfo::with_detail(
                ToriiErrorKind::WebSocket,
                "WebSocket error while streaming from Torii",
                err.to_string(),
            ),
            Self::InvalidWebSocketRequest(message) => ToriiErrorInfo::with_detail(
                ToriiErrorKind::InvalidWebSocketRequest,
                "Invalid WebSocket request",
                message.clone(),
            ),
            Self::Decode(err) => ToriiErrorInfo::with_detail(
                ToriiErrorKind::Decode,
                "Failed to decode Norito payload from Torii",
                err.clone(),
            ),
            Self::SignedQueryContext(err) => ToriiErrorInfo::with_detail(
                ToriiErrorKind::SignedQueryContext,
                "Failed to construct a replay-safe signed query",
                err.clone(),
            ),
            Self::Timeout { context } => ToriiErrorInfo::with_detail(
                ToriiErrorKind::Timeout,
                "Timed out while waiting for Torii",
                context.clone(),
            ),
            Self::SmokeRejected { hash, reason } => ToriiErrorInfo::with_detail(
                ToriiErrorKind::SmokeRejected,
                format!("Smoke transaction {hash} was rejected"),
                reason.clone(),
            ),
            Self::SmokeAdmissionOutcomeUnknown { hash } => ToriiErrorInfo::with_detail(
                ToriiErrorKind::SmokeAdmissionOutcomeUnknown,
                format!("Smoke transaction admission outcome remains unknown for {hash}"),
                "Reconcile or resubmit only the byte-identical signed transaction".to_owned(),
            ),
        }
    }
    fn is_queue_plan_journal_outcome_unknown(&self) -> bool {
        matches!(
            self,
            Self::UnexpectedStatus {
                reject_code: Some(code),
                ..
            } if code == QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN_REJECT_CODE
        )
    }
    fn confirms_existing_submission(&self) -> bool {
        matches!(
            self,
            Self::UnexpectedStatus {
                reject_code: Some(code),
                ..
            } if matches!(
                code.as_str(),
                "PRTRY:ALREADY_ENQUEUED" | "PRTRY:ALREADY_COMMITTED"
            )
        )
    }
    /// Return the server-provided retry delay for a throttled request.
    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after } => *retry_after,
            _ => None,
        }
    }
}
#[derive(Debug, Clone, norito::NoritoDeserialize, norito::NoritoSerialize)]
struct ToriiErrorEnvelope {
    code: String,
    message: String,
}
impl ToriiErrorEnvelope {
    fn summary(&self) -> String {
        if self.code.is_empty() {
            self.message.clone()
        } else {
            format!("{}: {}", self.code, self.message)
        }
    }
}
include!("torii/response_error_headers.rs");
fn response_status_error(response: &reqwest::Response) -> ToriiError {
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        ToriiError::RateLimited {
            retry_after: retry_after_from_headers(response.headers()),
        }
    } else {
        ToriiError::UnexpectedStatus {
            status: response.status(),
            reject_code: reject_code_from_headers(response.headers()),
            message: None,
        }
    }
}
fn websocket_connect_error(error: WebSocketError) -> ToriiError {
    match error {
        WebSocketError::Http(response) if response.status() == StatusCode::TOO_MANY_REQUESTS => {
            ToriiError::RateLimited {
                retry_after: retry_after_from_headers(response.headers()),
            }
        }
        other => ToriiError::WebSocket(other),
    }
}
fn error_message_from_body(body: &[u8]) -> Option<String> {
    if let Ok(envelope) = decode_norito_with_alignment::<ToriiErrorEnvelope>(body) {
        return Some(envelope.summary());
    }
    if let Ok(value) = norito::json::from_slice::<json::Value>(body)
        && let Some(message) = value
            .get("message")
            .or_else(|| value.get("error"))
            .and_then(json::Value::as_str)
    {
        let code = value.get("code").and_then(json::Value::as_str);
        return Some(match code {
            Some(code) if !code.is_empty() => format!("{code}: {message}"),
            _ => message.to_owned(),
        });
    }
    let text = String::from_utf8_lossy(body).trim().to_owned();
    if text.is_empty() { None } else { Some(text) }
}
fn decode_bounded_json_response(bytes: &[u8], context: &'static str) -> ToriiResult<json::Value> {
    const MAX_VALUES: usize = 262_144;
    const MAX_STRING_BYTES: usize = 1024 * 1024;
    const MAX_CONTAINER_ENTRIES: usize = 65_536;
    const MAX_DEPTH: usize = 64;
    let limits = norito::json::JsonPreflightLimits::new(
        MAX_JSON_RESPONSE_BYTES,
        MAX_VALUES,
        MAX_JSON_RESPONSE_BYTES,
        MAX_STRING_BYTES,
        MAX_JSON_RESPONSE_BYTES,
        MAX_CONTAINER_ENTRIES,
        MAX_VALUES,
        MAX_VALUES,
        MAX_VALUES,
        MAX_DEPTH,
    );
    norito::json::preflight_slice(bytes, limits).map_err(|_| {
        ToriiError::Decode(format!(
            "{context} response failed bounded JSON syntax/resource preflight"
        ))
    })?;
    json::from_slice(bytes)
        .map_err(|_| ToriiError::Decode(format!("{context} response failed JSON decoding")))
}
async fn read_bounded_json_response(
    response: Response,
    context: &'static str,
) -> ToriiResult<json::Value> {
    let bytes = read_bounded_response(response, MAX_JSON_RESPONSE_BYTES, context).await?;
    decode_bounded_json_response(&bytes, context)
}
fn compose_base_urls(base_url: &str) -> ToriiResult<(Url, Url)> {
    let http_base = Url::parse(base_url).map_err(ToriiError::InvalidBaseUrl)?;
    let scheme = http_base.scheme().to_owned();
    let ws_scheme = match scheme.as_str() {
        "http" => "ws",
        "https" => "wss",
        other => {
            return Err(ToriiError::UnsupportedScheme {
                scheme: other.to_owned(),
            });
        }
    };
    let mut ws_base = http_base.clone();
    ws_base
        .set_scheme(ws_scheme)
        .map_err(|_| ToriiError::UnsupportedScheme {
            scheme: scheme.clone(),
        })?;
    Ok((http_base, ws_base))
}
/// Options for waiting until a peer responds to `/status`.
#[derive(Debug, Clone, Copy)]
pub struct ReadinessOptions {
    /// Maximum duration to wait before giving up.
    pub timeout: Duration,
    /// Delay between successive probes while waiting for readiness.
    pub poll_interval: Duration,
}
/// A managed Torii peer that failed to report a committed genesis block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedPeerGenesisFailure {
    /// Stable peer alias from the managed topology.
    pub alias: String,
    /// Torii base URL that was probed.
    pub base_url: String,
    /// Classified failure returned by the peer's readiness probe.
    pub error: ToriiErrorInfo,
}
/// Failure of the all-managed-peer genesis readiness gate.
#[derive(Debug, thiserror::Error)]
pub enum ManagedPeerGenesisReadinessError {
    /// No managed peers were supplied to the gate.
    #[error("cannot wait for committed genesis because no managed Torii peers were supplied")]
    NoManagedPeers,
    /// One or more managed peers failed to report committed genesis before the deadline.
    #[error("not every managed Torii peer reported committed genesis: {diagnostics}")]
    PeerFailures {
        /// Actionable alias, endpoint, and error details for every failed peer.
        diagnostics: String,
        /// Structured failures for callers that render their own diagnostics.
        failures: Vec<ManagedPeerGenesisFailure>,
    },
}
impl ManagedPeerGenesisReadinessError {
    /// Structured per-peer failures, or an empty slice when no peers were supplied.
    #[must_use]
    pub fn failures(&self) -> &[ManagedPeerGenesisFailure] {
        match self {
            Self::NoManagedPeers => &[],
            Self::PeerFailures { failures, .. } => failures,
        }
    }
}
/// Wait concurrently until every managed Torii peer reports at least one committed block.
///
/// Each peer receives the same bounded readiness deadline. Running the probes concurrently keeps
/// the topology-wide gate bounded by that deadline rather than multiplying it by the peer count.
/// No transaction should be submitted to a managed multi-peer network before this gate succeeds.
pub async fn wait_for_all_managed_peers_genesis(
    peers: Vec<(String, ToriiClient)>,
    options: ReadinessOptions,
) -> Result<Vec<(String, ToriiStatusSnapshot)>, ManagedPeerGenesisReadinessError> {
    if peers.is_empty() {
        return Err(ManagedPeerGenesisReadinessError::NoManagedPeers);
    }
    let probes = peers.into_iter().map(|(alias, client)| async move {
        let base_url = client.base_url().to_owned();
        let result = client.wait_for_genesis_commit(options).await;
        (alias, base_url, result)
    });
    let mut committed = Vec::new();
    let mut failures = Vec::new();
    for (alias, base_url, result) in join_all(probes).await {
        match result {
            Ok(snapshot) => committed.push((alias, snapshot)),
            Err(error) => failures.push(ManagedPeerGenesisFailure {
                alias,
                base_url,
                error: error.summarize(),
            }),
        }
    }
    if failures.is_empty() {
        return Ok(committed);
    }
    let diagnostics = failures
        .iter()
        .map(|failure| {
            let detail = failure
                .error
                .detail
                .as_deref()
                .map_or_else(String::new, |detail| format!(" ({detail})"));
            format!(
                "{} at {}: {}{}",
                failure.alias, failure.base_url, failure.error.message, detail
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(ManagedPeerGenesisReadinessError::PeerFailures {
        diagnostics,
        failures,
    })
}
impl ReadinessOptions {
    /// Create a readiness configuration with the supplied timeout and a default poll interval.
    #[must_use]
    pub const fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            poll_interval: Duration::from_millis(250),
        }
    }
    /// Override the poll interval used when waiting for readiness.
    #[must_use]
    pub const fn with_poll_interval(mut self, poll_interval: Duration) -> Self {
        self.poll_interval = poll_interval;
        self
    }
}
/// Options for waiting until a submitted transaction is observed in the committed block stream.
#[derive(Debug, Clone, Copy)]
pub struct SmokeCommitOptions {
    /// Maximum duration to wait for the smoke transaction to commit.
    pub timeout: Duration,
}
impl SmokeCommitOptions {
    /// Create options with the provided timeout.
    #[must_use]
    pub const fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}
impl Default for SmokeCommitOptions {
    fn default() -> Self {
        Self::new(Duration::from_secs(15))
    }
}
/// Successful observation of a committed smoke transaction.
#[derive(Debug, Clone)]
pub struct SmokeCommitSnapshot {
    /// Hash of the submitted transaction.
    pub tx_hash: HashOf<SignedTransaction>,
    /// Height of the block that carried the transaction.
    pub block_height: u64,
    /// Elapsed time between submission and commitment observation.
    pub elapsed: Duration,
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum SmokeTransactionStatus {
    Queued,
    Committed(u64),
    Rejected(String),
    Expired,
}
/// Options governing a full readiness smoke probe (status poll + commit check).
#[derive(Debug, Clone)]
pub struct ReadinessSmokePlan {
    /// `/status` probe options used before submitting the smoke transaction.
    pub status_options: ReadinessOptions,
    /// Commit wait options for each attempt.
    pub commit_options: SmokeCommitOptions,
    /// Backoff applied between attempts.
    pub backoff: Duration,
    /// Transactions to try in order (one per attempt).
    pub transactions: Vec<SignedTransaction>,
    /// Recipe used to renew Mochi-generated transactions after genesis readiness.
    ///
    /// Callers that provide pre-signed transactions through [`Self::new`] retain
    /// exact-envelope semantics and are never re-signed.
    factory: Option<ReadinessSmokeFactory>,
}
#[derive(Debug, Clone)]
struct ReadinessSmokeFactory {
    network_id: NetworkId,
    signer: SigningAuthority,
    attempts: usize,
    nonce_offset: usize,
}
impl ReadinessSmokeFactory {
    fn build_transactions(
        &self,
        creation_time: Duration,
        ttl: Duration,
    ) -> Result<Vec<SignedTransaction>, ReadinessSmokeBuildError> {
        (0..self.attempts)
            .map(|attempt| {
                build_readiness_smoke_transaction_at(
                    self.network_id,
                    &self.signer,
                    attempt + self.nonce_offset,
                    creation_time,
                    ttl,
                )
            })
            .collect()
    }
}
impl ReadinessSmokePlan {
    /// Construct a plan using the provided transactions and default timeouts/backoff.
    #[must_use]
    pub fn new(transactions: Vec<SignedTransaction>) -> Self {
        Self {
            status_options: ReadinessOptions::default(),
            commit_options: SmokeCommitOptions::default(),
            backoff: Duration::from_millis(400),
            transactions,
            factory: None,
        }
    }
    /// Build an exact-network plan that updates metadata on the signing account.
    ///
    /// Each attempt carries a unique nonce so retries do not collide.
    pub fn for_signer_with_attempts(
        network_id: NetworkId,
        signer: &SigningAuthority,
        attempts: usize,
    ) -> Result<Self, ReadinessSmokeBuildError> {
        Self::for_signer_with_attempts_and_offset(network_id, signer, attempts, 0)
    }
    /// Build an exact-network plan with unique nonces derived from the provided offset.
    pub fn for_signer_with_attempts_and_offset(
        network_id: NetworkId,
        signer: &SigningAuthority,
        attempts: usize,
        nonce_offset: usize,
    ) -> Result<Self, ReadinessSmokeBuildError> {
        let attempts = attempts.max(1);
        let factory = ReadinessSmokeFactory {
            network_id,
            signer: signer.clone(),
            attempts,
            nonce_offset,
        };
        let transactions = factory.build_transactions(unix_time_now(), SMOKE_TTL)?;
        Ok(Self {
            factory: Some(factory),
            ..Self::new(transactions)
        })
    }
    /// Build a single-attempt exact-network plan using the provided signer.
    pub fn for_signer(
        network_id: NetworkId,
        signer: &SigningAuthority,
    ) -> Result<Self, ReadinessSmokeBuildError> {
        Self::for_signer_with_attempts(network_id, signer, 1)
    }
    /// Iterator over the hashes of the configured smoke transactions.
    pub fn tx_hashes(&self) -> impl Iterator<Item = HashOf<SignedTransaction>> + '_ {
        self.transactions.iter().map(SignedTransaction::hash)
    }
    fn renew_generated_transactions_if_needed(
        &mut self,
        now: Duration,
    ) -> Result<(), ReadinessSmokeBuildError> {
        let Some(factory) = &self.factory else {
            return Ok(());
        };
        let required_lifetime = self.required_submission_lifetime();
        let required_ttl = required_lifetime.max(SMOKE_TTL);
        let renew_before = now.saturating_add(required_lifetime);
        let remains_fresh = self.transactions.iter().all(|transaction| {
            transaction
                .time_to_live()
                .and_then(|ttl| transaction.creation_time().checked_add(ttl))
                .is_some_and(|expires_at| expires_at >= renew_before)
        });
        if remains_fresh {
            return Ok(());
        }
        self.transactions = factory.build_transactions(now, required_ttl)?;
        Ok(())
    }
    fn required_submission_lifetime(&self) -> Duration {
        let attempts = u32::try_from(self.transactions.len().max(1)).unwrap_or(u32::MAX);
        let mut lifetime = self.commit_options.timeout.saturating_mul(attempts);
        let mut backoff = self.backoff.max(Duration::from_millis(50)).min(MAX_BACKOFF);
        for _ in 1..attempts {
            lifetime = lifetime.saturating_add(backoff);
            backoff = backoff.saturating_mul(2).min(MAX_BACKOFF);
        }
        lifetime.saturating_add(SMOKE_SUBMISSION_MARGIN)
    }
}
/// Errors that can occur while constructing a readiness smoke plan.
#[derive(Debug, thiserror::Error)]
pub enum ReadinessSmokeBuildError {
    /// Failed to construct the smoke domain identifier.
    #[error("invalid readiness smoke domain `{0}`")]
    InvalidDomain(String),
    /// Failed to sign a smoke transaction.
    #[error("failed to sign readiness smoke transaction: {reason}")]
    Signing {
        /// Human readable failure reason.
        reason: String,
    },
}
/// Result of a readiness smoke probe.
#[derive(Debug, Clone)]
pub struct ReadinessSmokeOutcome {
    /// Zero-based elapsed time for the full probe (including retries and status polling).
    pub total_elapsed: Duration,
    /// Attempt number (1-indexed) that yielded a commit.
    pub attempt: usize,
    /// Snapshot of the commit location for the smoke transaction.
    pub commit: SmokeCommitSnapshot,
    /// Optional status snapshot captured after the commit to surface queue depth.
    pub status: Option<ToriiStatusSnapshot>,
}
/// Summary of a local Torii MCP probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalMcpProbeResult {
    /// MCP protocol version advertised by `initialize`.
    pub protocol_version: String,
    /// Optional server toolset version hash returned by `tools/list`.
    pub toolset_version: Option<String>,
    /// Number of visible tools in the local MCP catalog.
    pub tool_count: usize,
    /// Visible tool names returned by `tools/list`.
    pub tool_names: Vec<String>,
}
impl LocalMcpProbeResult {
    fn from_documents(
        capabilities: &json::Value,
        initialize: &json::Value,
        tools_list: &json::Value,
    ) -> ToriiResult<Self> {
        if !capabilities.is_object() {
            return Err(decode_error(
                "mcp capabilities",
                "GET /v1/mcp must return a JSON object",
            ));
        }
        let init_result = initialize
            .as_object()
            .and_then(|doc| doc.get("result"))
            .and_then(json::Value::as_object)
            .ok_or_else(|| decode_error("mcp initialize", "missing result object"))?;
        let protocol_version =
            parse_required_string(init_result, &["protocolVersion"], "mcp initialize result")?;
        let tools_result = tools_list
            .as_object()
            .and_then(|doc| doc.get("result"))
            .and_then(json::Value::as_object)
            .ok_or_else(|| decode_error("mcp tools/list", "missing result object"))?;
        let tools = tools_result
            .get("tools")
            .and_then(json::Value::as_array)
            .ok_or_else(|| decode_error("mcp tools/list result", "missing tools array"))?;
        let mut tool_names = Vec::with_capacity(tools.len());
        for (index, tool) in tools.iter().enumerate() {
            let tool_obj = tool.as_object().ok_or_else(|| {
                decode_error(
                    "mcp tools/list result.tools",
                    format!("tool {index} must be an object"),
                )
            })?;
            tool_names.push(parse_required_string(
                tool_obj,
                &["name"],
                "mcp tools/list result.tools[].name",
            )?);
        }
        if !tool_names.iter().any(|name| name.starts_with("iroha.")) {
            return Err(decode_error(
                "mcp tools/list result",
                "expected curated iroha.* tools to be exposed",
            ));
        }
        if tool_names.iter().any(|name| name.starts_with("torii.")) {
            return Err(decode_error(
                "mcp tools/list result",
                "raw torii.* tools must stay hidden for local Mochi sandboxes",
            ));
        }
        Ok(Self {
            protocol_version,
            toolset_version: tools_result
                .get("toolsetVersion")
                .and_then(json::Value::as_str)
                .map(str::to_owned),
            tool_count: tool_names.len(),
            tool_names,
        })
    }
}
const SMOKE_TTL: Duration = Duration::from_secs(30);
const SMOKE_SUBMISSION_MARGIN: Duration = Duration::from_secs(5);
const SMOKE_EXACT_RESUBMIT_DELAY: Duration = Duration::from_millis(250);
const SMOKE_EXACT_RESUBMIT_INTERVAL: Duration = Duration::from_secs(1);
const QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN_REJECT_CODE: &str =
    "PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN";
fn unix_time_now() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
}
fn smoke_transaction_result_in_block(
    block: &SignedBlock,
    tx_hash: &HashOf<SignedTransaction>,
) -> Option<ToriiResult<u64>> {
    if !block.has_results() {
        return None;
    }
    block
        .entrypoint_results()
        .find_map(|(_, entrypoint, result)| {
            let is_match = match &entrypoint {
                TransactionEntrypoint::External(transaction) => transaction.hash() == *tx_hash,
                TransactionEntrypoint::SealedReveal(reveal) => {
                    reveal.signed_transaction().hash() == *tx_hash
                }
                TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => {
                    false
                }
            };
            is_match.then(|| match result.as_ref() {
                Ok(_) => Ok(block.header().height().get()),
                Err(reason) => Err(ToriiError::SmokeRejected {
                    hash: tx_hash.to_string(),
                    reason: format!("{reason:?}"),
                }),
            })
        })
}
#[derive(Debug, Default)]
struct ReadinessSmokeAttemptCursor {
    next_index: usize,
    pinned_index: Option<usize>,
}
impl ReadinessSmokeAttemptCursor {
    fn current_index(&self) -> usize {
        self.pinned_index.unwrap_or(self.next_index)
    }
    fn record_failure(&mut self, index: usize, error: &ToriiError) {
        if self.pinned_index.is_some() {
            return;
        }
        if matches!(error, ToriiError::SmokeAdmissionOutcomeUnknown { .. }) {
            self.pinned_index = Some(index);
        } else {
            self.next_index = index.saturating_add(1);
        }
    }
    fn is_pinned(&self) -> bool {
        self.pinned_index.is_some()
    }
}
fn build_lane_lifecycle_transaction(
    network_id: NetworkId,
    signer: &SigningAuthority,
    status: &LaneLifecycleStatusV1,
    plan: LaneLifecyclePlan,
) -> ToriiResult<SignedTransaction> {
    if !signer.allows_permission(InstructionPermission::SetParameters) {
        return Err(ToriiError::Decode(format!(
            "signer `{}` is not configured with CanSetParameters",
            signer.label()
        )));
    }
    if !status.nexus_enabled {
        return Err(ToriiError::Decode(
            "Nexus lane lifecycle is disabled on the serving node".to_owned(),
        ));
    }
    let catalog = status
        .validate()
        .map_err(|err| ToriiError::Decode(format!("invalid lane lifecycle status: {err}")))?;
    let custom = LaneLifecycleParameterV1::new(&catalog, &status.incarnations, plan)
        .map_err(|err| ToriiError::Decode(format!("invalid lane incarnation binding: {err}")))?
        .into_custom_parameter();
    let mut builder = TransactionBuilder::new(
        network_id,
        signer.account_id().clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([SetParameter::new(Parameter::Custom(custom))]);
    builder.set_ttl(SMOKE_TTL);
    builder
        .try_sign(signer.key_pair().private_key())
        .map_err(|err| {
            ToriiError::Decode(format!("failed to sign lane lifecycle transaction: {err}"))
        })
}
fn build_readiness_smoke_transaction_at(
    network_id: NetworkId,
    signer: &SigningAuthority,
    attempt: usize,
    creation_time: Duration,
    ttl: Duration,
) -> Result<SignedTransaction, ReadinessSmokeBuildError> {
    let now_ms = creation_time.as_millis();
    let key = "mochi_smoke"
        .parse()
        .expect("readiness smoke metadata key is valid");
    let value = Json::new(format!("{now_ms}:{attempt}"));
    let quantity = u32::try_from(attempt + 1).unwrap_or(u32::MAX);
    let authority = signer.account_id().clone();
    let mut builder = TransactionBuilder::new(
        network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([SetKeyValue::account(authority, key, value)]);
    if let Some(nonce) = NonZeroU32::new(quantity) {
        builder.set_nonce(nonce);
    }
    builder.set_creation_time(creation_time);
    builder.set_ttl(ttl);
    builder
        .try_sign(signer.key_pair().private_key())
        .map_err(|err| ReadinessSmokeBuildError::Signing {
            reason: err.to_string(),
        })
}
impl Default for ReadinessOptions {
    fn default() -> Self {
        Self::new(Duration::from_secs(10))
    }
}
/// Builder for [`ToriiClient`] that allows configuring headers and timeouts.
#[derive(Clone, Debug)]
pub struct ToriiClientBuilder {
    http_base: Url,
    ws_base: Url,
    network_id: Option<NetworkId>,
    operator_signing_context: Option<OperatorSigningContext>,
    default_headers: HeaderMap,
    timeout: Option<Duration>,
}
impl ToriiClientBuilder {
    /// Create a builder targeting the provided Torii base URL.
    pub fn new(base_url: impl AsRef<str>) -> ToriiResult<Self> {
        let (http_base, ws_base) = compose_base_urls(base_url.as_ref())?;
        Ok(Self {
            http_base,
            ws_base,
            network_id: None,
            operator_signing_context: None,
            default_headers: HeaderMap::new(),
            timeout: None,
        })
    }
    /// Attach the `x-api-token` header to every request.
    pub fn with_api_token(mut self, token: impl AsRef<str>) -> ToriiResult<Self> {
        let value =
            HeaderValue::from_str(token.as_ref()).map_err(|source| ToriiError::InvalidHeader {
                name: "x-api-token".into(),
                source,
            })?;
        self.default_headers
            .insert(HeaderName::from_static("x-api-token"), value);
        Ok(self)
    }
    /// Apply a custom header to every HTTP/WebSocket request.
    pub fn with_header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.default_headers.insert(name, value);
        self
    }
    /// Attach HTTP basic authentication credentials to every request.
    pub fn with_basic_auth(
        mut self,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> ToriiResult<Self> {
        let encoded =
            BASE64_STANDARD.encode(format!("{}:{}", username.as_ref(), password.as_ref()));
        let value = HeaderValue::from_str(&format!("Basic {encoded}")).map_err(|source| {
            ToriiError::InvalidHeader {
                name: "authorization".into(),
                source,
            }
        })?;
        self.default_headers
            .insert(HeaderName::from_static("authorization"), value);
        Ok(self)
    }
    /// Set the HTTP client timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    /// Bind all signed queries produced by this client to one exact genesis lineage.
    pub fn with_network_id(mut self, network_id: NetworkId) -> Self {
        self.network_id = Some(network_id);
        self
    }
    /// Install immutable operator signing material bound to one exact network.
    #[must_use]
    pub fn with_operator_signing_context(mut self, context: OperatorSigningContext) -> Self {
        self.operator_signing_context = Some(context);
        self
    }
    /// Consume the builder and construct a [`ToriiClient`].
    pub fn build(self) -> ToriiResult<ToriiClient> {
        let network_id = match (self.network_id, self.operator_signing_context.as_ref()) {
            (Some(configured), Some(context)) if configured != context.network_id() => {
                return Err(ToriiError::SignedQueryContext(format!(
                    "operator signing context network id `{}` does not match client network id `{configured}`",
                    context.network_id()
                )));
            }
            (Some(configured), _) => Some(configured),
            (None, Some(context)) => Some(context.network_id()),
            (None, None) => None,
        };
        // Signed query bodies are one-shot. A redirect could replay the same
        // nonce after the original endpoint already admitted the request.
        let mut client_builder = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never());
        if let Some(timeout) = self.timeout {
            client_builder = client_builder.timeout(timeout);
        }
        if !self.default_headers.is_empty() {
            client_builder = client_builder.default_headers(self.default_headers.clone());
        }
        let http = client_builder.build()?;
        Ok(ToriiClient {
            http_base: self.http_base,
            ws_base: self.ws_base,
            network_id,
            operator_signing_context: self.operator_signing_context,
            http,
            status_state: Arc::new(Mutex::new(StatusState::default())),
            default_headers: self.default_headers,
        })
    }
}
/// WebSocket stream type alias used by Torii.
pub type ToriiWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
/// Simplified representation of frames received from a Torii WebSocket.
#[derive(Debug, Clone)]
pub enum WsFrame {
    /// Binary payload (typically Norito-framed data).
    Binary(Vec<u8>),
    /// UTF-8 payload.
    Text(String),
    /// The remote closed the stream.
    Closed,
    /// The subscription reported an error.
    Error(String),
}
/// Metrics derived from consecutive Torii status samples.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusMetrics {
    /// Latest commit latency in milliseconds.
    pub commit_latency_ms: u64,
    /// Current transaction queue depth.
    pub queue_size: u64,
    /// Change in queue depth compared to the previous sample.
    pub queue_delta: i64,
    /// Number of blocks committed since the previous sample.
    pub block_delta: u64,
    /// Number of non-empty blocks committed since the previous sample.
    pub blocks_non_empty_delta: u64,
    /// DA reschedules observed since the previous sample.
    pub da_reschedule_delta: u64,
    /// Approved transaction increase since the previous sample.
    pub tx_approved_delta: u64,
    /// Rejected transaction increase since the previous sample.
    pub tx_rejected_delta: u64,
    /// View-change count increase since the previous sample.
    pub view_change_delta: u32,
    /// Milliseconds elapsed between this snapshot and the previous sample.
    pub sample_interval_ms: u64,
}
impl StatusMetrics {
    /// Compute derived metrics using the previous and current telemetry snapshots.
    #[must_use]
    pub fn from_samples(previous: Option<&TelemetryStatus>, current: &TelemetryStatus) -> Self {
        let queue_delta = previous
            .map(|prev| current.queue_size as i64 - prev.queue_size as i64)
            .unwrap_or_default();
        let (block_delta, blocks_non_empty_delta) = previous
            .map(|prev| {
                (
                    current.blocks.saturating_sub(prev.blocks),
                    current
                        .blocks_non_empty
                        .saturating_sub(prev.blocks_non_empty),
                )
            })
            .unwrap_or((0, 0));
        let (tx_approved_delta, tx_rejected_delta, view_change_delta) = previous
            .map(|prev| {
                (
                    current.txs_approved.saturating_sub(prev.txs_approved),
                    current.txs_rejected.saturating_sub(prev.txs_rejected),
                    current.view_changes.saturating_sub(prev.view_changes),
                )
            })
            .unwrap_or((0, 0, 0));
        let da_reschedule_delta = previous
            .map(|prev| {
                current
                    .da_reschedule_total
                    .saturating_sub(prev.da_reschedule_total)
            })
            .unwrap_or(0);
        Self {
            commit_latency_ms: current.commit_time_ms,
            queue_size: current.queue_size,
            queue_delta,
            block_delta,
            blocks_non_empty_delta,
            da_reschedule_delta,
            tx_approved_delta,
            tx_rejected_delta,
            view_change_delta,
            sample_interval_ms: 0,
        }
    }
    /// Whether any notable activity occurred between the last two samples.
    #[must_use]
    pub fn has_activity(&self) -> bool {
        self.tx_approved_delta > 0
            || self.tx_rejected_delta > 0
            || self.queue_delta != 0
            || self.da_reschedule_delta > 0
            || self.block_delta > 0
            || self.blocks_non_empty_delta > 0
            || self.view_change_delta > 0
    }
}
/// Telemetry snapshot enriched with derived metrics.
#[derive(Debug, Clone)]
pub struct ToriiStatusSnapshot {
    /// Instant when the sample was captured.
    pub timestamp: Instant,
    /// Raw telemetry payload returned by Torii.
    pub status: TelemetryStatus,
    /// Derived metrics computed from the last two samples.
    pub metrics: StatusMetrics,
}
impl ToriiStatusSnapshot {
    fn new(timestamp: Instant, status: TelemetryStatus, metrics: StatusMetrics) -> Self {
        Self {
            timestamp,
            status,
            metrics,
        }
    }
}
#[derive(Debug, Default)]
struct StatusState {
    previous: Option<StatusSample>,
}
impl StatusState {
    fn record(&mut self, timestamp: Instant, status: &TelemetryStatus) -> StatusMetrics {
        let mut metrics = StatusMetrics::from_samples(
            self.previous.as_ref().map(|sample| &sample.status),
            status,
        );
        metrics.sample_interval_ms = self
            .previous
            .as_ref()
            .and_then(|sample| timestamp.checked_duration_since(sample.timestamp))
            .map(duration_to_millis)
            .unwrap_or(0);
        self.previous = Some(StatusSample {
            timestamp,
            status: status.clone(),
        });
        metrics
    }
}
#[derive(Debug, Clone)]
struct StatusSample {
    timestamp: Instant,
    status: TelemetryStatus,
}
fn duration_to_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}
/// Selected gauges sampled from the `/metrics` Prometheus endpoint.
#[derive(Debug, Clone)]
pub struct ToriiMetricsSnapshot {
    /// Instant when the metrics payload was fetched.
    pub timestamp: Instant,
    /// Size of the transaction queue reported by telemetry.
    pub queue_size: Option<f64>,
    /// Number of view changes recorded in consensus.
    pub view_changes: Option<f64>,
    /// Transactions observed in the consensus queue.
    pub sumeragi_tx_queue_depth: Option<f64>,
    /// Configured consensus queue capacity.
    pub sumeragi_tx_queue_capacity: Option<f64>,
    /// Saturation flag emitted by consensus (0 = healthy, 1 = saturated).
    pub sumeragi_tx_queue_saturated: Option<f64>,
    /// Number of entries retained in the tiered-state hot tier.
    pub state_tiered_hot_entries: Option<f64>,
    /// Number of entries spilled to the tiered-state cold tier.
    pub state_tiered_cold_entries: Option<f64>,
    /// Bytes written to the tiered-state cold tier in the latest snapshot.
    pub state_tiered_cold_bytes: Option<f64>,
    /// Milliseconds elapsed since genesis according to telemetry.
    pub uptime_since_genesis_ms: Option<f64>,
}
/// Pagination metadata returned by Explorer APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExplorerPaginationMeta {
    /// Current page number (1-indexed).
    pub page: u64,
    /// Items per page configured by the backend.
    pub per_page: u64,
    /// Total number of available pages.
    pub total_pages: u64,
    /// Total number of items available on the backend.
    pub total_items: u64,
}
impl ExplorerPaginationMeta {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer blocks pagination", "must be a JSON object"))?;
        Ok(Self {
            page: parse_u64_field(
                record,
                &["page"],
                1,
                false,
                "explorer blocks pagination.page",
            )?,
            per_page: parse_u64_field(
                record,
                &["per_page", "perPage"],
                1,
                false,
                "explorer blocks pagination.per_page",
            )?,
            total_pages: parse_u64_field(
                record,
                &["total_pages", "totalPages"],
                0,
                true,
                "explorer blocks pagination.total_pages",
            )?,
            total_items: parse_u64_field(
                record,
                &["total_items", "totalItems"],
                0,
                true,
                "explorer blocks pagination.total_items",
            )?,
        })
    }
}
const EXPLORER_CURSOR_MAX_LENGTH: usize = 1_424;
const EXPLORER_CURSOR_MAX_LIMIT: u32 = 100;
fn require_exact_explorer_fields(
    record: &json::Map,
    expected: &[&str],
    context: &str,
) -> ToriiResult<()> {
    if record.len() != expected.len() || expected.iter().any(|field| !record.contains_key(*field)) {
        return Err(decode_error(
            context,
            format!("must contain exactly these fields: {}", expected.join(", ")),
        ));
    }
    Ok(())
}
fn validate_explorer_items_len(
    items_len: usize,
    pagination: &ExplorerCursorMeta,
    context: &str,
) -> ToriiResult<()> {
    if items_len > pagination.limit as usize {
        return Err(decode_error(
            context,
            format!(
                "must contain at most {} entries, matching pagination.limit",
                pagination.limit
            ),
        ));
    }
    Ok(())
}
/// Seek-pagination metadata returned by Explorer world-collection APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerCursorMeta {
    /// Maximum number of entries requested for this page.
    pub limit: u32,
    /// Opaque cursor for the next page, when another page exists.
    pub next_cursor: Option<String>,
    /// Whether another page exists for the same collection and filters.
    pub has_more: bool,
}
impl ExplorerCursorMeta {
    fn from_json(value: &json::Value, context: &str) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error(context, "must be a JSON object"))?;
        require_exact_explorer_fields(record, &["limit", "next_cursor", "has_more"], context)?;
        let limit_value = record
            .get("limit")
            .ok_or_else(|| decode_error(&format!("{context}.limit"), "missing field"))?;
        let limit = parse_u64_value(limit_value, false, &format!("{context}.limit"))?;
        let limit = u32::try_from(limit)
            .ok()
            .filter(|limit| *limit <= EXPLORER_CURSOR_MAX_LIMIT)
            .ok_or_else(|| {
                decode_error(
                    &format!("{context}.limit"),
                    format!("must be between 1 and {EXPLORER_CURSOR_MAX_LIMIT}"),
                )
            })?;
        let next_cursor = match record.get("next_cursor") {
            Some(value) if value.is_null() => None,
            Some(value) => {
                let cursor = value.as_str().ok_or_else(|| {
                    decode_error(
                        &format!("{context}.next_cursor"),
                        "must be a string or null",
                    )
                })?;
                validate_explorer_cursor(cursor, &format!("{context}.next_cursor"))?;
                Some(cursor.to_owned())
            }
            None => {
                return Err(decode_error(
                    &format!("{context}.next_cursor"),
                    "missing field",
                ));
            }
        };
        let has_more = record
            .get("has_more")
            .and_then(json::Value::as_bool)
            .ok_or_else(|| decode_error(&format!("{context}.has_more"), "must be a boolean"))?;
        if has_more != next_cursor.is_some() {
            return Err(decode_error(
                context,
                "has_more must match next_cursor availability",
            ));
        }
        Ok(Self {
            limit,
            next_cursor,
            has_more,
        })
    }
}
fn validate_explorer_cursor<'a>(cursor: &'a str, context: &str) -> ToriiResult<&'a str> {
    let canonical = !cursor.is_empty()
        && cursor.len() <= EXPLORER_CURSOR_MAX_LENGTH
        && URL_SAFE_NO_PAD
            .decode(cursor)
            .map(|decoded| URL_SAFE_NO_PAD.encode(decoded) == cursor)
            .unwrap_or(false);
    if !canonical {
        return Err(decode_error(
            context,
            format!(
                "must be canonical base64url without padding and at most {EXPLORER_CURSOR_MAX_LENGTH} characters"
            ),
        ));
    }
    Ok(cursor)
}
fn append_explorer_cursor_params(
    params: &mut Vec<(&'static str, String)>,
    cursor: Option<String>,
    limit: Option<u32>,
    context: &str,
) -> ToriiResult<()> {
    if let Some(cursor) = cursor {
        validate_explorer_cursor(&cursor, &format!("{context}.cursor"))?;
        params.push(("cursor", cursor));
    }
    if let Some(limit) = limit {
        if !(1..=EXPLORER_CURSOR_MAX_LIMIT).contains(&limit) {
            return Err(decode_error(
                &format!("{context}.limit"),
                format!("must be between 1 and {EXPLORER_CURSOR_MAX_LIMIT}"),
            ));
        }
        params.push(("limit", limit.to_string()));
    }
    Ok(())
}
/// Explorer block summary returned by `/v1/blocks` endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerBlockRecord {
    /// Hex-encoded block hash.
    pub hash: String,
    /// Block height (`1`-indexed).
    pub height: u64,
    /// RFC 3339 timestamp recorded by Explorer.
    pub created_at: String,
    /// Optional previous block hash.
    pub prev_block_hash: Option<String>,
    /// Optional transactions hash recorded on the block.
    pub transactions_hash: Option<String>,
    /// Count of rejected transactions.
    pub transactions_rejected: u64,
    /// Count of transactions included in the block.
    pub transactions_total: u64,
}
impl ExplorerBlockRecord {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer block record", "must be a JSON object"))?;
        let hash = parse_hex_field(
            record,
            &["hash", "block_hash", "blockHash"],
            "explorer block record.hash",
        )?;
        let created_at = parse_required_string(
            record,
            &["created_at", "createdAt"],
            "explorer block record.created_at",
        )?;
        Ok(Self {
            hash,
            height: parse_u64_field(
                record,
                &["height"],
                1,
                false,
                "explorer block record.height",
            )?,
            created_at,
            prev_block_hash: parse_optional_hex_field(
                record,
                &["prev_block_hash", "prevBlockHash"],
                "explorer block record.prev_block_hash",
            )?,
            transactions_hash: parse_optional_hex_field(
                record,
                &["transactions_hash", "transactionsHash"],
                "explorer block record.transactions_hash",
            )?,
            transactions_rejected: parse_u64_field(
                record,
                &["transactions_rejected", "transactionsRejected"],
                0,
                true,
                "explorer block record.transactions_rejected",
            )?,
            transactions_total: parse_u64_field(
                record,
                &["transactions_total", "transactionsTotal"],
                0,
                true,
                "explorer block record.transactions_total",
            )?,
        })
    }
}
/// Explorer `/v1/blocks` response model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerBlocksPage {
    /// Pagination metadata.
    pub pagination: ExplorerPaginationMeta,
    /// Block entries included in this page.
    pub items: Vec<ExplorerBlockRecord>,
}
impl ExplorerBlocksPage {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer blocks response", "must be a JSON object"))?;
        let pagination = record
            .get("pagination")
            .ok_or_else(|| decode_error("explorer blocks response", "missing pagination field"))
            .and_then(ExplorerPaginationMeta::from_json)?;
        let items_value = record
            .get("items")
            .ok_or_else(|| decode_error("explorer blocks response", "missing items field"))?;
        let items_array = items_value.as_array().ok_or_else(|| {
            decode_error("explorer blocks response.items", "must be a JSON array")
        })?;
        let mut items = Vec::with_capacity(items_array.len());
        for (index, entry) in items_array.iter().enumerate() {
            let record = ExplorerBlockRecord::from_json(entry).map_err(|err| {
                decode_error(
                    "explorer blocks response.items",
                    format!("failed to decode entry {index}: {err}"),
                )
            })?;
            items.push(record);
        }
        Ok(Self { pagination, items })
    }
}
/// Query parameters accepted by `/v1/blocks`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExplorerBlocksQuery {
    /// Optional block height offset.
    pub offset_height: Option<u64>,
    /// Maximum number of items to return.
    pub limit: Option<u32>,
}
/// Explorer account entry returned by `/v1/explorer/accounts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerAccountRecord {
    /// Canonical I105 identifier.
    pub id: String,
    /// I105-encoded literal for the account.
    pub i105_address: String,
    /// Network prefix emitted by Torii.
    pub network_prefix: u16,
    /// Metadata payload attached to the account.
    pub metadata: json::Value,
    /// Number of domains owned by the account.
    pub owned_domains: u64,
    /// Number of assets owned by the account.
    pub owned_assets: u64,
    /// Number of NFTs owned by the account.
    pub owned_nfts: u64,
}
impl ExplorerAccountRecord {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer account record", "must be a JSON object"))?;
        let id = parse_required_string(record, &["id"], "explorer account record.id")?;
        let i105_address = parse_required_string(
            record,
            &["i105_address"],
            "explorer account record.i105_address",
        )?;
        let network_prefix = parse_u64_field(
            record,
            &["network_prefix"],
            0,
            true,
            "explorer account record.network_prefix",
        )?;
        let owned_domains = parse_u64_field(
            record,
            &["owned_domains"],
            0,
            true,
            "explorer account record.owned_domains",
        )?;
        let owned_assets = parse_u64_field(
            record,
            &["owned_assets"],
            0,
            true,
            "explorer account record.owned_assets",
        )?;
        let owned_nfts = parse_u64_field(
            record,
            &["owned_nfts"],
            0,
            true,
            "explorer account record.owned_nfts",
        )?;
        let metadata = record
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| json::Value::Object(json::Map::new()));
        let prefix = u16::try_from(network_prefix).map_err(|_| {
            decode_error(
                "explorer account record.network_prefix",
                "value must fit in a u16",
            )
        })?;
        Ok(Self {
            id,
            i105_address,
            network_prefix: prefix,
            metadata,
            owned_domains,
            owned_assets,
            owned_nfts,
        })
    }
}
/// Explorer `/v1/explorer/accounts` response model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerAccountsPage {
    /// Seek-pagination metadata returned by Explorer.
    pub pagination: ExplorerCursorMeta,
    /// Account entries in the requested page.
    pub items: Vec<ExplorerAccountRecord>,
}
impl ExplorerAccountsPage {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let doc = value
            .as_object()
            .ok_or_else(|| decode_error("explorer accounts page", "must be a JSON object"))?;
        require_exact_explorer_fields(doc, &["pagination", "items"], "explorer accounts page")?;
        let pagination = doc
            .get("pagination")
            .ok_or_else(|| decode_error("explorer accounts page", "missing pagination field"))
            .and_then(|value| {
                ExplorerCursorMeta::from_json(value, "explorer accounts page.pagination")
            })?;
        let items_value = doc
            .get("items")
            .ok_or_else(|| decode_error("explorer accounts page", "missing items field"))?;
        let items = items_value
            .as_array()
            .ok_or_else(|| decode_error("explorer accounts page.items", "must be a JSON array"))?;
        validate_explorer_items_len(items.len(), &pagination, "explorer accounts page.items")?;
        let mut parsed = Vec::with_capacity(items.len());
        for (index, entry) in items.iter().enumerate() {
            let record = ExplorerAccountRecord::from_json(entry).map_err(|err| {
                decode_error(
                    "explorer accounts page.items",
                    format!("failed to decode entry {index}: {err}"),
                )
            })?;
            parsed.push(record);
        }
        Ok(Self {
            pagination,
            items: parsed,
        })
    }
}
/// Parameters accepted by `/v1/explorer/accounts`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplorerAccountsQuery {
    /// Opaque cursor returned by the preceding page.
    pub cursor: Option<String>,
    /// Maximum number of entries to return (1 through 100).
    pub limit: Option<u32>,
    /// Optional domain filter (canonical identifier).
    pub domain: Option<String>,
    /// Optional asset definition filter (`definition#domain` literal).
    pub with_asset: Option<String>,
}
/// Explorer domain entry returned by `/v1/explorer/domains`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerDomainRecord {
    /// Canonical domain identifier.
    pub id: String,
    /// Optional logo URL (if provided).
    pub logo: Option<String>,
    /// Metadata payload attached to the domain.
    pub metadata: json::Value,
    /// Account that owns the domain.
    pub owned_by: String,
    /// Number of accounts registered under the domain.
    pub accounts: u64,
    /// Number of assets registered under the domain.
    pub assets: u64,
    /// Number of NFTs registered under the domain.
    pub nfts: u64,
}
impl ExplorerDomainRecord {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer domain record", "must be a JSON object"))?;
        let id = parse_required_string(record, &["id"], "explorer domain record.id")?;
        let owned_by =
            parse_required_string(record, &["owned_by"], "explorer domain record.owned_by")?;
        let logo = record
            .get("logo")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let metadata = record
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| json::Value::Object(json::Map::new()));
        let accounts = parse_u64_field(
            record,
            &["accounts"],
            0,
            true,
            "explorer domain record.accounts",
        )?;
        let assets = parse_u64_field(
            record,
            &["assets"],
            0,
            true,
            "explorer domain record.assets",
        )?;
        let nfts = parse_u64_field(record, &["nfts"], 0, true, "explorer domain record.nfts")?;
        Ok(Self {
            id,
            logo,
            metadata,
            owned_by,
            accounts,
            assets,
            nfts,
        })
    }
}
/// Explorer `/v1/explorer/domains` response model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerDomainsPage {
    /// Seek-pagination metadata returned by Torii.
    pub pagination: ExplorerCursorMeta,
    /// Domain entries contained in the page.
    pub items: Vec<ExplorerDomainRecord>,
}
impl ExplorerDomainsPage {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer domains response", "must be a JSON object"))?;
        require_exact_explorer_fields(
            record,
            &["pagination", "items"],
            "explorer domains response",
        )?;
        let pagination = record
            .get("pagination")
            .ok_or_else(|| decode_error("explorer domains response", "missing pagination field"))
            .and_then(|value| {
                ExplorerCursorMeta::from_json(value, "explorer domains response.pagination")
            })?;
        let items_value = record
            .get("items")
            .ok_or_else(|| decode_error("explorer domains response", "missing items field"))?;
        let items_array = items_value.as_array().ok_or_else(|| {
            decode_error("explorer domains response.items", "must be a JSON array")
        })?;
        validate_explorer_items_len(
            items_array.len(),
            &pagination,
            "explorer domains response.items",
        )?;
        let mut items = Vec::with_capacity(items_array.len());
        for (index, entry) in items_array.iter().enumerate() {
            let record = ExplorerDomainRecord::from_json(entry).map_err(|err| {
                decode_error(
                    "explorer domains response.items",
                    format!("failed to decode entry {index}: {err}"),
                )
            })?;
            items.push(record);
        }
        Ok(Self { pagination, items })
    }
}
/// Parameters accepted by `/v1/explorer/domains`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplorerDomainsQuery {
    /// Opaque cursor returned by the preceding page.
    pub cursor: Option<String>,
    /// Maximum number of entries to return (1 through 100).
    pub limit: Option<u32>,
    /// Optional filter restricting the owning account.
    pub owned_by: Option<String>,
}
/// Explorer asset definition entry returned by `/v1/explorer/asset-definitions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerAssetDefinitionRecord {
    /// Canonical Base58 asset definition identifier.
    pub id: String,
    /// Mintability flag serialized by Torii (`Infinitely`, `Once`, etc.).
    pub mintable: String,
    /// Optional logo URL (if provided).
    pub logo: Option<String>,
    /// Metadata payload attached to the definition.
    pub metadata: json::Value,
    /// Account that registered the definition.
    pub owned_by: String,
    /// Number of asset instances registered for the definition.
    pub assets: u64,
}
impl ExplorerAssetDefinitionRecord {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value.as_object().ok_or_else(|| {
            decode_error("explorer asset definition record", "must be a JSON object")
        })?;
        let id = parse_required_string(record, &["id"], "explorer asset definition record.id")?;
        let mintable = parse_required_string(
            record,
            &["mintable"],
            "explorer asset definition record.mintable",
        )?;
        let owned_by = parse_required_string(
            record,
            &["owned_by"],
            "explorer asset definition record.owned_by",
        )?;
        let logo = record
            .get("logo")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let metadata = record
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| json::Value::Object(json::Map::new()));
        let assets = parse_u64_field(
            record,
            &["assets"],
            0,
            true,
            "explorer asset definition record.assets",
        )?;
        Ok(Self {
            id,
            mintable,
            logo,
            metadata,
            owned_by,
            assets,
        })
    }
}
/// Explorer `/v1/explorer/asset-definitions` response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerAssetDefinitionsPage {
    /// Seek-pagination metadata returned by Torii.
    pub pagination: ExplorerCursorMeta,
    /// Asset definition entries contained in the page.
    pub items: Vec<ExplorerAssetDefinitionRecord>,
}
impl ExplorerAssetDefinitionsPage {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value.as_object().ok_or_else(|| {
            decode_error(
                "explorer asset definitions response",
                "must be a JSON object",
            )
        })?;
        require_exact_explorer_fields(
            record,
            &["pagination", "items"],
            "explorer asset definitions response",
        )?;
        let pagination = record
            .get("pagination")
            .ok_or_else(|| {
                decode_error(
                    "explorer asset definitions response",
                    "missing pagination field",
                )
            })
            .and_then(|value| {
                ExplorerCursorMeta::from_json(
                    value,
                    "explorer asset definitions response.pagination",
                )
            })?;
        let items_value = record.get("items").ok_or_else(|| {
            decode_error("explorer asset definitions response", "missing items field")
        })?;
        let items_array = items_value.as_array().ok_or_else(|| {
            decode_error(
                "explorer asset definitions response.items",
                "must be a JSON array",
            )
        })?;
        validate_explorer_items_len(
            items_array.len(),
            &pagination,
            "explorer asset definitions response.items",
        )?;
        let mut items = Vec::with_capacity(items_array.len());
        for (index, entry) in items_array.iter().enumerate() {
            let record = ExplorerAssetDefinitionRecord::from_json(entry).map_err(|err| {
                decode_error(
                    "explorer asset definitions response.items",
                    format!("failed to decode entry {index}: {err}"),
                )
            })?;
            items.push(record);
        }
        Ok(Self { pagination, items })
    }
}
/// Parameters accepted by `/v1/explorer/asset-definitions`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplorerAssetDefinitionsQuery {
    /// Opaque cursor returned by the preceding page.
    pub cursor: Option<String>,
    /// Maximum number of entries to return (1 through 100).
    pub limit: Option<u32>,
    /// Optional domain filter restricting results.
    pub domain: Option<String>,
    /// Optional owning account filter.
    pub owned_by: Option<String>,
}
/// Explorer asset entry returned by `/v1/explorer/assets`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerAssetRecord {
    /// Canonical asset identifier (`norito:<hex>`).
    pub id: String,
    /// Definition backing the asset.
    pub definition_id: String,
    /// Owning account identifier.
    pub account_id: String,
    /// Value rendered as a string (mirrors Explorer payload).
    pub value: String,
}
impl ExplorerAssetRecord {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer asset record", "must be a JSON object"))?;
        let id = parse_required_string(record, &["id"], "explorer asset record.id")?;
        let definition_id = parse_required_string(
            record,
            &["definition_id"],
            "explorer asset record.definition_id",
        )?;
        let account_id =
            parse_required_string(record, &["account_id"], "explorer asset record.account_id")?;
        let value = parse_required_string(record, &["value"], "explorer asset record.value")?;
        Ok(Self {
            id,
            definition_id,
            account_id,
            value,
        })
    }
}
/// Explorer `/v1/explorer/assets` response model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerAssetsPage {
    /// Seek-pagination metadata returned by Torii.
    pub pagination: ExplorerCursorMeta,
    /// Asset entries in the page.
    pub items: Vec<ExplorerAssetRecord>,
}
impl ExplorerAssetsPage {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer assets response", "must be a JSON object"))?;
        require_exact_explorer_fields(
            record,
            &["pagination", "items"],
            "explorer assets response",
        )?;
        let pagination = record
            .get("pagination")
            .ok_or_else(|| decode_error("explorer assets response", "missing pagination field"))
            .and_then(|value| {
                ExplorerCursorMeta::from_json(value, "explorer assets response.pagination")
            })?;
        let items_value = record
            .get("items")
            .ok_or_else(|| decode_error("explorer assets response", "missing items field"))?;
        let items_array = items_value.as_array().ok_or_else(|| {
            decode_error("explorer assets response.items", "must be a JSON array")
        })?;
        validate_explorer_items_len(
            items_array.len(),
            &pagination,
            "explorer assets response.items",
        )?;
        let mut items = Vec::with_capacity(items_array.len());
        for (index, entry) in items_array.iter().enumerate() {
            let record = ExplorerAssetRecord::from_json(entry).map_err(|err| {
                decode_error(
                    "explorer assets response.items",
                    format!("failed to decode entry {index}: {err}"),
                )
            })?;
            items.push(record);
        }
        Ok(Self { pagination, items })
    }
}
/// Parameters accepted by `/v1/explorer/assets`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplorerAssetsQuery {
    /// Opaque cursor returned by the preceding page.
    pub cursor: Option<String>,
    /// Maximum number of entries to return (1 through 100).
    pub limit: Option<u32>,
    /// Optional owning account filter.
    pub owned_by: Option<String>,
    /// Optional definition filter (`definition#domain` literal).
    pub definition: Option<String>,
}
/// Explorer NFT entry returned by `/v1/explorer/nfts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerNftRecord {
    /// Canonical NFT identifier.
    pub id: String,
    /// Account that currently owns the NFT.
    pub owned_by: String,
    /// Metadata payload describing the NFT.
    pub metadata: json::Value,
}
impl ExplorerNftRecord {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer NFT record", "must be a JSON object"))?;
        let id = parse_required_string(record, &["id"], "explorer NFT record.id")?;
        let owned_by =
            parse_required_string(record, &["owned_by"], "explorer NFT record.owned_by")?;
        let metadata = record
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| json::Value::Object(json::Map::new()));
        Ok(Self {
            id,
            owned_by,
            metadata,
        })
    }
}
/// Explorer `/v1/explorer/nfts` response model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerNftsPage {
    /// Seek-pagination metadata returned by Torii.
    pub pagination: ExplorerCursorMeta,
    /// NFT entries included in the page.
    pub items: Vec<ExplorerNftRecord>,
}
impl ExplorerNftsPage {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer nfts response", "must be a JSON object"))?;
        require_exact_explorer_fields(record, &["pagination", "items"], "explorer nfts response")?;
        let pagination = record
            .get("pagination")
            .ok_or_else(|| decode_error("explorer nfts response", "missing pagination field"))
            .and_then(|value| {
                ExplorerCursorMeta::from_json(value, "explorer nfts response.pagination")
            })?;
        let items_value = record
            .get("items")
            .ok_or_else(|| decode_error("explorer nfts response", "missing items field"))?;
        let items_array = items_value
            .as_array()
            .ok_or_else(|| decode_error("explorer nfts response.items", "must be a JSON array"))?;
        validate_explorer_items_len(
            items_array.len(),
            &pagination,
            "explorer nfts response.items",
        )?;
        let mut items = Vec::with_capacity(items_array.len());
        for (index, entry) in items_array.iter().enumerate() {
            let record = ExplorerNftRecord::from_json(entry).map_err(|err| {
                decode_error(
                    "explorer nfts response.items",
                    format!("failed to decode entry {index}: {err}"),
                )
            })?;
            items.push(record);
        }
        Ok(Self { pagination, items })
    }
}
/// Parameters accepted by `/v1/explorer/nfts`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplorerNftsQuery {
    /// Opaque cursor returned by the preceding page.
    pub cursor: Option<String>,
    /// Maximum number of entries to return (1 through 100).
    pub limit: Option<u32>,
    /// Optional owning account filter.
    pub owned_by: Option<String>,
    /// Optional domain filter restricting NFT IDs.
    pub domain: Option<String>,
}
/// Parent-lot quantity returned with an Explorer RWA record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerRwaParentRecord {
    /// Canonical parent RWA identifier.
    pub rwa: String,
    /// Quantity inherited from the parent lot.
    pub quantity: String,
}
impl ExplorerRwaParentRecord {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer RWA parent", "must be a JSON object"))?;
        Ok(Self {
            rwa: parse_required_string(record, &["rwa"], "explorer RWA parent.rwa")?,
            quantity: parse_required_string(record, &["quantity"], "explorer RWA parent.quantity")?,
        })
    }
}
/// Explorer RWA entry returned by `/v1/explorer/rwas`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerRwaRecord {
    /// Canonical RWA identifier.
    pub id: String,
    /// Account that currently owns the RWA.
    pub owned_by: String,
    /// Total lot quantity.
    pub quantity: String,
    /// Quantity currently held from transfer.
    pub held_quantity: String,
    /// Primary external reference for the RWA.
    pub primary_reference: String,
    /// Optional lifecycle status.
    pub status: Option<String>,
    /// Whether transfers are frozen.
    pub is_frozen: bool,
    /// Metadata attached to the RWA.
    pub metadata: json::Value,
    /// Parent-lot relationships.
    pub parents: Vec<ExplorerRwaParentRecord>,
}
impl ExplorerRwaRecord {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer RWA record", "must be a JSON object"))?;
        let status = match record.get("status") {
            None | Some(json::Value::Null) => None,
            Some(value) => {
                let status = value.as_str().ok_or_else(|| {
                    decode_error("explorer RWA record.status", "must be a string or null")
                })?;
                if status.is_empty() {
                    return Err(decode_error(
                        "explorer RWA record.status",
                        "must not be empty",
                    ));
                }
                Some(status.to_owned())
            }
        };
        let is_frozen = record
            .get("is_frozen")
            .and_then(json::Value::as_bool)
            .ok_or_else(|| decode_error("explorer RWA record.is_frozen", "must be a boolean"))?;
        let parents = match record.get("parents") {
            None => Vec::new(),
            Some(value) => value
                .as_array()
                .ok_or_else(|| decode_error("explorer RWA record.parents", "must be a JSON array"))?
                .iter()
                .enumerate()
                .map(|(index, parent)| {
                    ExplorerRwaParentRecord::from_json(parent).map_err(|error| {
                        decode_error(
                            "explorer RWA record.parents",
                            format!("failed to decode entry {index}: {error}"),
                        )
                    })
                })
                .collect::<ToriiResult<Vec<_>>>()?,
        };
        Ok(Self {
            id: parse_required_string(record, &["id"], "explorer RWA record.id")?,
            owned_by: parse_required_string(record, &["owned_by"], "explorer RWA record.owned_by")?,
            quantity: parse_required_string(record, &["quantity"], "explorer RWA record.quantity")?,
            held_quantity: parse_required_string(
                record,
                &["held_quantity"],
                "explorer RWA record.held_quantity",
            )?,
            primary_reference: parse_required_string(
                record,
                &["primary_reference"],
                "explorer RWA record.primary_reference",
            )?,
            status,
            is_frozen,
            metadata: record
                .get("metadata")
                .cloned()
                .unwrap_or_else(|| json::Value::Object(json::Map::new())),
            parents,
        })
    }
}
/// Explorer `/v1/explorer/rwas` response model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerRwasPage {
    /// Seek-pagination metadata returned by Torii.
    pub pagination: ExplorerCursorMeta,
    /// RWA entries included in the page.
    pub items: Vec<ExplorerRwaRecord>,
}
impl ExplorerRwasPage {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer rwas response", "must be a JSON object"))?;
        require_exact_explorer_fields(record, &["pagination", "items"], "explorer rwas response")?;
        let pagination = record
            .get("pagination")
            .ok_or_else(|| decode_error("explorer rwas response", "missing pagination field"))
            .and_then(|value| {
                ExplorerCursorMeta::from_json(value, "explorer rwas response.pagination")
            })?;
        let items_array = record
            .get("items")
            .and_then(json::Value::as_array)
            .ok_or_else(|| decode_error("explorer rwas response.items", "must be a JSON array"))?;
        validate_explorer_items_len(
            items_array.len(),
            &pagination,
            "explorer rwas response.items",
        )?;
        let items = items_array
            .iter()
            .enumerate()
            .map(|(index, value)| {
                ExplorerRwaRecord::from_json(value).map_err(|error| {
                    decode_error(
                        "explorer rwas response.items",
                        format!("failed to decode entry {index}: {error}"),
                    )
                })
            })
            .collect::<ToriiResult<Vec<_>>>()?;
        Ok(Self { pagination, items })
    }
}
/// Parameters accepted by `/v1/explorer/rwas`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplorerRwasQuery {
    /// Opaque cursor returned by the preceding page.
    pub cursor: Option<String>,
    /// Maximum number of entries to return (1 through 100).
    pub limit: Option<u32>,
    /// Optional owning account filter.
    pub owned_by: Option<String>,
    /// Optional domain filter restricting RWA IDs.
    pub domain: Option<String>,
}
/// Trigger definition returned by Torii trigger endpoints.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerRecord {
    /// Unique trigger identifier.
    pub id: String,
    /// Raw trigger action payload.
    pub action: json::Value,
    /// Optional metadata attached to the trigger.
    pub metadata: json::Value,
    /// Raw JSON payload returned by Torii.
    pub raw: json::Value,
}
impl TriggerRecord {
    fn from_json(value: &json::Value, context: &str) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error(context, "must be a JSON object"))?;
        let id = parse_required_string(record, &["id"], &format!("{context}.id"))?;
        let action_value = record
            .get("action")
            .ok_or_else(|| decode_error(context, "missing action field"))?;
        if !action_value.is_object() {
            return Err(decode_error(
                &format!("{context}.action"),
                "must be a JSON object",
            ));
        }
        let metadata = match record.get("metadata").cloned() {
            None => json::Value::Object(json::Map::new()),
            Some(json::Value::Null) => json::Value::Object(json::Map::new()),
            Some(json::Value::Object(map)) => json::Value::Object(map),
            Some(_) => {
                return Err(decode_error(
                    &format!("{context}.metadata"),
                    "must be a JSON object",
                ));
            }
        };
        Ok(Self {
            id,
            action: action_value.clone(),
            metadata,
            raw: value.clone(),
        })
    }
}
/// Paginated trigger listing returned from `/v1/triggers`.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerListPage {
    /// Trigger entries contained in this page.
    pub items: Vec<TriggerRecord>,
    /// Total number of triggers reported by the endpoint.
    pub total: u64,
}
impl TriggerListPage {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("trigger list response", "must be a JSON object"))?;
        let empty_items = json::Value::Array(Vec::new());
        let items_value = record.get("items").unwrap_or(&empty_items);
        let items_array = items_value
            .as_array()
            .ok_or_else(|| decode_error("trigger list response.items", "must be a JSON array"))?;
        let mut items = Vec::with_capacity(items_array.len());
        for (index, entry) in items_array.iter().enumerate() {
            let record =
                TriggerRecord::from_json(entry, &format!("trigger list response.items[{index}]"))?;
            items.push(record);
        }
        let total = match record.get("total") {
            Some(value) => parse_u64_value(value, true, "trigger list response.total")?,
            None => items_array.len() as u64,
        };
        Ok(Self { items, total })
    }
}
/// Query parameters accepted by `/v1/triggers`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TriggerListQuery {
    /// Optional namespace filter.
    pub namespace: Option<String>,
    /// Optional authority filter.
    pub authority: Option<String>,
    /// Maximum number of triggers to return.
    pub limit: Option<u32>,
    /// Offset applied to the listing.
    pub offset: Option<u32>,
}
fn decode_error(context: &str, message: impl Into<String>) -> ToriiError {
    ToriiError::Decode(format!("{context}: {}", message.into()))
}
fn parse_required_string(record: &json::Map, keys: &[&str], context: &str) -> ToriiResult<String> {
    let value = pick_value(record, keys)
        .and_then(json::Value::as_str)
        .ok_or_else(|| decode_error(context, "expected non-empty string"))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(decode_error(context, "value cannot be empty"));
    }
    Ok(trimmed.to_owned())
}
fn parse_hex_field(record: &json::Map, keys: &[&str], context: &str) -> ToriiResult<String> {
    let value = parse_required_string(record, keys, context)?;
    if !is_hex(&value) {
        return Err(decode_error(context, "value must be a hex string"));
    }
    Ok(value)
}
fn parse_optional_hex_field(
    record: &json::Map,
    keys: &[&str],
    context: &str,
) -> ToriiResult<Option<String>> {
    let Some(value) = pick_value(record, keys) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let string_value = value
        .as_str()
        .ok_or_else(|| decode_error(context, "value must be a string"))?
        .trim();
    if string_value.is_empty() {
        return Ok(None);
    }
    if !is_hex(string_value) {
        return Err(decode_error(context, "value must be a hex string"));
    }
    Ok(Some(string_value.to_owned()))
}
fn parse_u64_field(
    record: &json::Map,
    keys: &[&str],
    default: u64,
    allow_zero: bool,
    context: &str,
) -> ToriiResult<u64> {
    match pick_value(record, keys) {
        Some(value) => parse_u64_value(value, allow_zero, context),
        None => Ok(default),
    }
}
fn parse_u64_value(value: &json::Value, allow_zero: bool, context: &str) -> ToriiResult<u64> {
    let parsed = match value {
        json::Value::Number(number) => number.as_u64(),
        json::Value::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
    .ok_or_else(|| decode_error(context, "value must be an unsigned integer"))?;
    if parsed == 0 && !allow_zero {
        return Err(decode_error(context, "value must be greater than zero"));
    }
    Ok(parsed)
}
fn parse_optional_u64_field(
    record: &json::Map,
    keys: &[&str],
    context: &str,
) -> ToriiResult<Option<u64>> {
    pick_value(record, keys)
        .map(|value| parse_u64_value(value, true, context))
        .transpose()
}
fn parse_pipeline_smoke_status(value: &json::Value) -> ToriiResult<Option<SmokeTransactionStatus>> {
    let record = value
        .as_object()
        .ok_or_else(|| decode_error("pipeline transaction status", "must be a JSON object"))?;
    let status = record
        .get("status")
        .and_then(json::Value::as_object)
        .ok_or_else(|| {
            decode_error(
                "pipeline transaction status.status",
                "must be a JSON object",
            )
        })?;
    let kind = parse_required_string(status, &["kind"], "pipeline transaction status.kind")?;
    let height = parse_optional_u64_field(
        status,
        &["block_height", "blockHeight"],
        "pipeline transaction status.block_height",
    )?;
    match kind.as_str() {
        "Committed" | "Applied" => Ok(Some(SmokeTransactionStatus::Committed(
            height.unwrap_or_default(),
        ))),
        "Approved" => Ok(height.map(SmokeTransactionStatus::Committed)),
        "Rejected" => Ok(Some(SmokeTransactionStatus::Rejected(
            smoke_rejection_reason(status),
        ))),
        "Expired" => Ok(Some(SmokeTransactionStatus::Expired)),
        "Queued" => Ok(Some(SmokeTransactionStatus::Queued)),
        _ => Ok(None),
    }
}
fn parse_explorer_smoke_status(value: &json::Value) -> ToriiResult<Option<SmokeTransactionStatus>> {
    let record = value
        .as_object()
        .ok_or_else(|| decode_error("explorer transaction record", "must be a JSON object"))?;
    let status = parse_required_string(record, &["status"], "explorer transaction record.status")?;
    match status.as_str() {
        "Committed" | "Applied" | "Approved" => {
            let height = parse_optional_u64_field(
                record,
                &["block", "block_height", "blockHeight"],
                "explorer transaction record.block",
            )?
            .unwrap_or_default();
            Ok(Some(SmokeTransactionStatus::Committed(height)))
        }
        "Rejected" => Ok(Some(SmokeTransactionStatus::Rejected(
            smoke_rejection_reason(record),
        ))),
        "Expired" => Ok(Some(SmokeTransactionStatus::Expired)),
        "Queued" | "Pending" => Ok(Some(SmokeTransactionStatus::Queued)),
        _ => Ok(None),
    }
}
fn smoke_rejection_reason(record: &json::Map) -> String {
    pick_value(record, &["rejection_reason", "rejectionReason", "reason"])
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| json::to_string(value).unwrap_or_else(|_| format!("{value:?}")))
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "rejected".to_owned())
}
fn pick_value<'a>(record: &'a json::Map, keys: &[&str]) -> Option<&'a json::Value> {
    keys.iter().find_map(|key| record.get(*key))
}
fn is_hex(value: &str) -> bool {
    !value.is_empty()
        && value.as_bytes().iter().all(|byte| byte.is_ascii_hexdigit())
        && value.len().is_multiple_of(2)
}
impl ToriiMetricsSnapshot {
    /// Parse a Prometheus plaintext payload into a structured snapshot.
    #[must_use]
    pub fn from_prometheus(now: Instant, body: &str) -> Self {
        let mut snapshot = Self {
            timestamp: now,
            queue_size: None,
            view_changes: None,
            sumeragi_tx_queue_depth: None,
            sumeragi_tx_queue_capacity: None,
            sumeragi_tx_queue_saturated: None,
            state_tiered_hot_entries: None,
            state_tiered_cold_entries: None,
            state_tiered_cold_bytes: None,
            uptime_since_genesis_ms: None,
        };
        for line in body.lines() {
            if let Some((name, value)) = parse_scalar_metric(line) {
                match name {
                    "queue_size" => snapshot.queue_size = Some(value),
                    "view_changes" => snapshot.view_changes = Some(value),
                    "sumeragi_tx_queue_depth" => snapshot.sumeragi_tx_queue_depth = Some(value),
                    "sumeragi_tx_queue_capacity" => {
                        snapshot.sumeragi_tx_queue_capacity = Some(value)
                    }
                    "sumeragi_tx_queue_saturated" => {
                        snapshot.sumeragi_tx_queue_saturated = Some(value)
                    }
                    "state_tiered_hot_entries" => snapshot.state_tiered_hot_entries = Some(value),
                    "state_tiered_cold_entries" => snapshot.state_tiered_cold_entries = Some(value),
                    "state_tiered_cold_bytes" => snapshot.state_tiered_cold_bytes = Some(value),
                    "uptime_since_genesis_ms" => snapshot.uptime_since_genesis_ms = Some(value),
                    _ => {}
                }
            }
        }
        snapshot
    }
    /// Ratio (0–1) representing how full the consensus queue is.
    ///
    /// Returns `None` when either the depth or capacity gauges were missing or the backend reported
    /// a zero/negative capacity (should never happen on healthy Torii deployments).
    #[must_use]
    pub fn queue_utilization(&self) -> Option<f64> {
        let depth = self.sumeragi_tx_queue_depth?;
        let capacity = self.sumeragi_tx_queue_capacity?;
        if capacity <= 0.0 {
            return None;
        }
        Some((depth / capacity).clamp(0.0, 1.0))
    }
    /// Boolean saturation flag derived from the exporter gauge.
    ///
    /// Returns `None` when the exporter did not emit the flag or reported an unexpected value
    /// (non-zero/non-one) so UI surfaces can retain a tri-state indicator.
    #[must_use]
    pub fn queue_saturation_flag(&self) -> Option<bool> {
        let value = self.sumeragi_tx_queue_saturated?;
        if value <= 0.0 {
            Some(false)
        } else if value >= 1.0 {
            Some(true)
        } else {
            None
        }
    }
    /// Percentage of entries that spilled into the cold tier (0–1).
    ///
    /// Returns `None` when the exporter lacks hot/cold counters or when the
    /// tiers report no entries (preventing division by zero).
    #[must_use]
    pub fn cold_entry_ratio(&self) -> Option<f64> {
        let hot = self.state_tiered_hot_entries?;
        let cold = self.state_tiered_cold_entries?;
        let total = hot + cold;
        if total <= 0.0 {
            return None;
        }
        Some((cold / total).clamp(0.0, 1.0))
    }
}
fn parse_scalar_metric(line: &str) -> Option<(&str, f64)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.contains('{') {
        return None;
    }
    let mut parts = trimmed.split_whitespace();
    let name = parts.next()?;
    let value = parts.next()?;
    let parsed = value.parse::<f64>().ok()?;
    Some((name, parsed))
}
/// Shared state published by [`ToriiStatusMonitor`].
#[derive(Debug, Clone, Default)]
pub struct StatusMonitorState {
    /// Most recent telemetry snapshot fetched from Torii.
    pub last_snapshot: Option<ToriiStatusSnapshot>,
    /// Timestamp of the latest successful poll.
    pub last_success_at: Option<Instant>,
    /// Latest classified error returned by the poller (if any).
    pub last_error: Option<ToriiErrorInfo>,
    /// Number of consecutive failures observed since the last successful poll.
    pub consecutive_failures: u32,
}
impl StatusMonitorState {
    /// Whether the monitor produced at least one snapshot.
    #[must_use]
    pub fn has_snapshot(&self) -> bool {
        self.last_snapshot.is_some()
    }
    /// Compute how stale the last successful poll is relative to the current instant.
    #[must_use]
    pub fn last_success_age(&self) -> Option<Duration> {
        self.last_success_at.map(|instant| instant.elapsed())
    }
}
/// Shared state published by [`ToriiMetricsMonitor`].
#[derive(Debug, Clone, Default)]
pub struct MetricsMonitorState {
    /// Most recent metrics snapshot fetched from Torii.
    pub last_snapshot: Option<ToriiMetricsSnapshot>,
    /// Timestamp of the latest successful poll.
    pub last_success_at: Option<Instant>,
    /// Latest classified error returned by the poller (if any).
    pub last_error: Option<ToriiErrorInfo>,
    /// Number of consecutive failures observed since the last successful poll.
    pub consecutive_failures: u32,
}
impl MetricsMonitorState {
    /// Whether the monitor produced at least one snapshot.
    #[must_use]
    pub fn has_snapshot(&self) -> bool {
        self.last_snapshot.is_some()
    }
    /// Compute how stale the last successful poll is relative to the current instant.
    #[must_use]
    pub fn last_success_age(&self) -> Option<Duration> {
        self.last_success_at.map(|instant| instant.elapsed())
    }
}
/// Background task that polls Torii status on an interval and publishes snapshots via a watch channel.
///
/// This fulfils the roadmap requirement for the MOCHI supervisor to stream `/status`
/// data continuously so UI panels can surface queue depth, DA reschedules, and
/// related telemetry without wiring bespoke timers in the front end.
#[derive(Debug)]
pub struct ToriiStatusMonitor {
    receiver: watch::Receiver<StatusMonitorState>,
    handle: JoinHandle<()>,
}
impl ToriiStatusMonitor {
    /// Spawn a monitor that polls the supplied fetcher at the configured interval.
    ///
    /// The fetcher closure is primarily exposed for tests; production callers should
    /// prefer [`ToriiClient::spawn_status_monitor`].
    pub fn spawn<F, Fut>(interval: Duration, fetcher: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToriiResult<ToriiStatusSnapshot>> + Send + 'static,
    {
        let period = if interval.is_zero() {
            Duration::from_millis(500)
        } else {
            interval
        };
        let (sender, receiver) = watch::channel(StatusMonitorState::default());
        let fetcher = Arc::new(fetcher);
        let handle = tokio::spawn({
            let fetcher = Arc::clone(&fetcher);
            async move {
                let mut ticker = tokio::time::interval(period);
                ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
                let mut state = StatusMonitorState::default();
                loop {
                    ticker.tick().await;
                    match fetcher().await {
                        Ok(snapshot) => {
                            let timestamp = snapshot.timestamp;
                            state.last_snapshot = Some(snapshot);
                            state.last_success_at = Some(timestamp);
                            state.last_error = None;
                            state.consecutive_failures = 0;
                        }
                        Err(err) => {
                            state.last_error = Some(err.summarize());
                            state.consecutive_failures =
                                state.consecutive_failures.saturating_add(1);
                        }
                    }
                    let _ = sender.send(state.clone());
                }
            }
        });
        Self { receiver, handle }
    }
    /// Subscribe to status monitor updates.
    pub fn subscribe(&self) -> watch::Receiver<StatusMonitorState> {
        self.receiver.clone()
    }
    /// Retrieve the latest published state without waiting for an update.
    #[must_use]
    pub fn latest(&self) -> StatusMonitorState {
        self.receiver.borrow().clone()
    }
    /// Stop the background polling task.
    pub fn stop(&self) {
        if !self.handle.is_finished() {
            self.handle.abort();
        }
    }
}
impl Drop for ToriiStatusMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}
/// Background task that polls Prometheus metrics on an interval and publishes structured snapshots.
///
/// This extends the real-time visibility roadmap goal by wiring `/metrics` polling
/// into the core client so dashboards do not need bespoke timers.
#[derive(Debug)]
pub struct ToriiMetricsMonitor {
    receiver: watch::Receiver<MetricsMonitorState>,
    handle: JoinHandle<()>,
}
impl ToriiMetricsMonitor {
    /// Spawn a monitor that polls the supplied fetcher at the configured interval.
    ///
    /// The fetcher closure is primarily exposed for tests; production callers should
    /// prefer [`ToriiClient::spawn_metrics_monitor`].
    pub fn spawn<F, Fut>(interval: Duration, fetcher: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToriiResult<ToriiMetricsSnapshot>> + Send + 'static,
    {
        let period = if interval.is_zero() {
            Duration::from_millis(500)
        } else {
            interval
        };
        let (sender, receiver) = watch::channel(MetricsMonitorState::default());
        let fetcher = Arc::new(fetcher);
        let handle = tokio::spawn({
            let fetcher = Arc::clone(&fetcher);
            async move {
                let mut ticker = tokio::time::interval(period);
                ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
                let mut state = MetricsMonitorState::default();
                loop {
                    ticker.tick().await;
                    match fetcher().await {
                        Ok(snapshot) => {
                            state.last_snapshot = Some(snapshot.clone());
                            state.last_success_at = Some(snapshot.timestamp);
                            state.last_error = None;
                            state.consecutive_failures = 0;
                        }
                        Err(err) => {
                            state.last_error = Some(err.summarize());
                            state.consecutive_failures =
                                state.consecutive_failures.saturating_add(1);
                        }
                    }
                    let _ = sender.send(state.clone());
                }
            }
        });
        Self { receiver, handle }
    }
    /// Subscribe to metrics monitor updates.
    pub fn subscribe(&self) -> watch::Receiver<MetricsMonitorState> {
        self.receiver.clone()
    }
    /// Retrieve the latest published state without waiting for an update.
    #[must_use]
    pub fn latest(&self) -> MetricsMonitorState {
        self.receiver.borrow().clone()
    }
    /// Stop the background polling task.
    pub fn stop(&self) {
        if !self.handle.is_finished() {
            self.handle.abort();
        }
    }
}
impl Drop for ToriiMetricsMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}
fn lag_to_usize(skipped: u64) -> usize {
    usize::try_from(skipped).unwrap_or(usize::MAX)
}
/// Decode a Norito payload, retrying with an aligned copy if the caller hands us
/// misaligned bytes (a common artefact of mock HTTP servers and FFI bindings).
///
/// Every attempt uses limits derived from the complete frame, so an untrusted header or nested
/// length cannot allocate outside the encoded payload's conservative first-release envelope.
///
/// The helper is exported so downstream crates (or language bindings) can share
/// the same alignment guard instead of cloning the `unsafe` retry logic.
pub fn decode_norito_with_alignment<T>(bytes: &[u8]) -> Result<T, ToriiError>
where
    T: for<'de> norito::core::NoritoDeserialize<'de>,
{
    const MAX_PAD: usize = 64;
    let attempt = |slice: &[u8]| {
        catch_unwind(AssertUnwindSafe(|| {
            norito::decode_from_reader_with_limits(
                Cursor::new(slice),
                norito::canonical_decode_limits(slice.len()),
            )
        }))
    };
    match attempt(bytes) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => Err(ToriiError::Decode(err.to_string())),
        Err(_) => {
            for pad in 1..=MAX_PAD {
                let capacity = bytes.len().checked_add(pad).ok_or_else(|| {
                    ToriiError::Decode("Norito alignment length overflowed".into())
                })?;
                let mut buffer = Vec::new();
                buffer.try_reserve_exact(capacity).map_err(|_| {
                    ToriiError::Decode("Norito alignment retry allocation failed".into())
                })?;
                buffer.resize(pad, 0);
                buffer.extend_from_slice(bytes);
                match attempt(&buffer[pad..]) {
                    Ok(Ok(value)) => return Ok(value),
                    Ok(Err(err)) => return Err(ToriiError::Decode(err.to_string())),
                    Err(_) => continue,
                }
            }
            Err(ToriiError::Decode(
                "Norito decode panicked on payload".into(),
            ))
        }
    }
}
/// Minimal Torii client supporting REST calls and WebSocket subscriptions.
#[derive(Clone, Debug)]
pub struct ToriiClient {
    http_base: Url,
    ws_base: Url,
    network_id: Option<NetworkId>,
    operator_signing_context: Option<OperatorSigningContext>,
    http: Client,
    status_state: Arc<Mutex<StatusState>>,
    default_headers: HeaderMap,
}
#[cfg(test)]
pub(crate) fn test_network_id() -> NetworkId {
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
        .parse()
        .expect("test network id")
}
fn canonical_event_filters() -> Vec<EventFilterBox> {
    vec![
        EventFilterBox::Pipeline(TransactionEventFilter::default().into()),
        EventFilterBox::Pipeline(BlockEventFilter::default().into()),
        EventFilterBox::Pipeline(MergeLedgerEventFilter::default().into()),
        EventFilterBox::Pipeline(WitnessEventFilter::default().into()),
        EventFilterBox::Data(DataEventFilter::Any),
        EventFilterBox::Time(TimeEventFilter::new(ExecutionTime::PreCommit)),
        EventFilterBox::ExecuteTrigger(ExecuteTriggerEventFilter::new()),
        EventFilterBox::TriggerCompleted(TriggerCompletedEventFilter::new()),
    ]
}
impl ToriiClient {
    /// Construct a client pointing at the supplied Torii HTTP base URL.
    pub fn new(base_url: impl AsRef<str>) -> ToriiResult<Self> {
        Self::builder(base_url)?.build()
    }
    /// Construct a client whose signed-query context is bound to one exact genesis lineage.
    pub fn new_for_network(base_url: impl AsRef<str>, network_id: NetworkId) -> ToriiResult<Self> {
        Self::builder(base_url)?.with_network_id(network_id).build()
    }
    /// Start constructing a [`ToriiClient`] with custom options.
    pub fn builder(base_url: impl AsRef<str>) -> ToriiResult<ToriiClientBuilder> {
        ToriiClientBuilder::new(base_url)
    }
    /// HTTP base URL used for REST calls (e.g., `http://127.0.0.1:8080`).
    pub fn base_url(&self) -> &str {
        self.http_base.as_str()
    }
    /// Return the immutable genesis lineage configured for signed queries.
    pub fn network_id(&self) -> Option<NetworkId> {
        self.network_id
    }
    fn require_network_id(&self) -> ToriiResult<NetworkId> {
        self.network_id.ok_or_else(|| {
            ToriiError::SignedQueryContext(
                "client has no exact genesis network_id configured for signed requests".to_owned(),
            )
        })
    }
    /// Build and sign a fresh one-shot query request for this client's network.
    pub fn sign_query(
        &self,
        request: QueryRequest,
        authority: AccountId,
        key_pair: &KeyPair,
    ) -> ToriiResult<SignedQuery> {
        const QUERY_TTL_MS: u64 = 100_000;
        let network_id = self.require_network_id()?;
        let creation_time_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| ToriiError::SignedQueryContext(error.to_string()))?
            .as_millis()
            .try_into()
            .map_err(|_| {
                ToriiError::SignedQueryContext("Unix timestamp does not fit u64".to_owned())
            })?;
        let mut nonce = [0_u8; 32];
        for _ in 0..16 {
            OsRng.try_fill_bytes(&mut nonce).map_err(|error| {
                ToriiError::SignedQueryContext(format!("OS nonce generation failed: {error}"))
            })?;
            if nonce != [0_u8; 32] {
                return request
                    .with_authority(
                        network_id,
                        authority,
                        creation_time_ms,
                        NonZeroU64::new(QUERY_TTL_MS).expect("signed-query TTL is nonzero"),
                        nonce,
                    )
                    .try_sign(key_pair)
                    .map_err(|error| ToriiError::SignedQueryContext(error.to_string()));
            }
        }
        Err(ToriiError::SignedQueryContext(
            "OS RNG repeatedly returned an all-zero query nonce".to_owned(),
        ))
    }
    /// URL of the canonical `/v1/pipeline/transactions` endpoint.
    pub fn transaction_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint(torii_uri::TRANSACTION)
    }
    /// URL of the canonical `/v1/query` endpoint.
    pub fn query_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint(torii_uri::QUERY)
    }
    /// URL of the canonical `/v1/blocks/stream` WebSocket endpoint.
    pub fn block_stream_endpoint(&self) -> ToriiResult<Url> {
        self.ws_endpoint(torii_uri::BLOCKS_STREAM)
    }
    /// URL of the canonical `/v1/events/ws` WebSocket endpoint.
    pub fn events_stream_endpoint(&self) -> ToriiResult<Url> {
        self.ws_endpoint(torii_uri::SUBSCRIPTION)
    }
    /// URL of the `/status` endpoint.
    pub fn status_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("status")
    }
    /// URL of the `/v1/sumeragi/status` endpoint.
    pub fn sumeragi_status_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/sumeragi/status")
    }
    /// URL of the `/v1/sumeragi/diagnostics` endpoint.
    pub fn sumeragi_diagnostics_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/sumeragi/diagnostics")
    }
    /// URL of the `/metrics` endpoint.
    pub fn metrics_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("metrics")
    }
    /// URL of the native `/v1/mcp` endpoint.
    pub fn mcp_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/mcp")
    }
    /// URL of the `/v1/blocks` Explorer endpoint.
    pub fn blocks_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/blocks")
    }
    /// URL of the `/v1/blocks/{height}` Explorer endpoint.
    pub fn block_by_height_endpoint(&self, height: u64) -> ToriiResult<Url> {
        self.http_endpoint(&format!("v1/blocks/{height}"))
    }
    /// URL of the `/v1/explorer/accounts` endpoint.
    pub fn explorer_accounts_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/explorer/accounts")
    }
    /// URL of the `/v1/explorer/domains` endpoint.
    pub fn explorer_domains_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/explorer/domains")
    }
    /// URL of the `/v1/explorer/asset-definitions` endpoint.
    pub fn explorer_asset_definitions_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/explorer/asset-definitions")
    }
    /// URL of the `/v1/explorer/assets` endpoint.
    pub fn explorer_assets_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/explorer/assets")
    }
    /// URL of the `/v1/explorer/nfts` endpoint.
    pub fn explorer_nfts_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/explorer/nfts")
    }
    /// URL of the `/v1/explorer/rwas` endpoint.
    pub fn explorer_rwas_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/explorer/rwas")
    }
    /// URL of the `/v1/explorer/transactions/{hash}` endpoint.
    pub fn explorer_transaction_endpoint(&self, hash: &str) -> ToriiResult<Url> {
        self.http_endpoint(&format!("v1/explorer/transactions/{hash}"))
    }
    /// URL of the `/v1/pipeline/transactions/status` endpoint.
    pub fn pipeline_transaction_status_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint(torii_routes::pipeline::TRANSACTION_STATUS.path())
    }
    /// URL of the `/v1/triggers` endpoint.
    pub fn triggers_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/triggers")
    }
    /// URL of the `/v1/triggers/{id}` endpoint.
    pub fn trigger_record_endpoint(&self, trigger_id: &str) -> ToriiResult<Url> {
        self.http_endpoint(&format!("v1/triggers/{trigger_id}"))
    }
    /// Spawn a background task that polls `/status` on the supplied interval and publishes snapshots.
    pub fn spawn_status_monitor(&self, interval: Duration) -> ToriiStatusMonitor {
        let client = self.clone();
        ToriiStatusMonitor::spawn(interval, move || {
            let client = client.clone();
            async move { client.fetch_status_snapshot().await }
        })
    }
    /// Spawn a background task that polls `/metrics` on the supplied interval and publishes snapshots.
    pub fn spawn_metrics_monitor(&self, interval: Duration) -> ToriiMetricsMonitor {
        let client = self.clone();
        ToriiMetricsMonitor::spawn(interval, move || {
            let client = client.clone();
            async move { client.fetch_metrics_snapshot().await }
        })
    }
    /// Probe `/status` until the peer responds or the timeout elapses.
    pub async fn wait_for_ready(
        &self,
        options: ReadinessOptions,
    ) -> ToriiResult<ToriiStatusSnapshot> {
        let mut backoff = options
            .poll_interval
            .max(Duration::from_millis(10))
            .min(MAX_BACKOFF);
        let start = Instant::now();
        let deadline = start
            .checked_add(options.timeout)
            .unwrap_or_else(|| start + options.timeout);
        loop {
            match self.fetch_status_snapshot().await {
                Ok(snapshot) => return Ok(snapshot),
                Err(err) => {
                    let now = Instant::now();
                    if now >= deadline {
                        return Err(err);
                    }
                    let remaining = deadline.saturating_duration_since(now);
                    sleep(backoff.min(remaining)).await;
                    backoff = (backoff.saturating_mul(2)).min(MAX_BACKOFF);
                }
            }
        }
    }
    /// Probe `/status` until the chain has committed its genesis block.
    ///
    /// A responsive height-zero peer is still bootstrapping and cannot yet admit transactions
    /// against committed validator authority. Transient status failures and height-zero responses
    /// share one bounded [`ReadinessOptions`] deadline and exponential backoff.
    pub async fn wait_for_genesis_commit(
        &self,
        options: ReadinessOptions,
    ) -> ToriiResult<ToriiStatusSnapshot> {
        let mut backoff = options
            .poll_interval
            .max(Duration::from_millis(10))
            .min(MAX_BACKOFF);
        let start = Instant::now();
        let deadline = start
            .checked_add(options.timeout)
            .unwrap_or_else(|| start + options.timeout);
        loop {
            let poll_error = match self.fetch_status_snapshot().await {
                Ok(snapshot) if snapshot.status.blocks > 0 => return Ok(snapshot),
                Ok(_) => None,
                Err(err) => Some(err),
            };
            let now = Instant::now();
            if now >= deadline {
                return match poll_error {
                    Some(err) => Err(err),
                    None => Err(ToriiError::Timeout {
                        context: "genesis commitment (status remained at zero committed blocks)"
                            .to_owned(),
                    }),
                };
            }
            let remaining = deadline.saturating_duration_since(now);
            sleep(backoff.min(remaining)).await;
            backoff = (backoff.saturating_mul(2)).min(MAX_BACKOFF);
        }
    }
    /// Run a readiness smoke probe that waits for `/status`, submits a smoke transaction,
    /// and observes its commitment with retries/backoff.
    pub async fn wait_for_readiness_smoke(
        &self,
        mut plan: ReadinessSmokePlan,
    ) -> ToriiResult<ReadinessSmokeOutcome> {
        if plan.transactions.is_empty() {
            return Err(ToriiError::Decode(
                "readiness smoke plan must include at least one transaction".to_owned(),
            ));
        }
        self.wait_for_genesis_commit(plan.status_options).await?;
        plan.renew_generated_transactions_if_needed(unix_time_now())
            .map_err(|err| {
                ToriiError::Decode(format!(
                    "failed to renew readiness smoke transactions after genesis commitment: {err}"
                ))
            })?;
        let attempts = plan.transactions.len();
        let started = Instant::now();
        let mut backoff = plan.backoff.max(Duration::from_millis(50)).min(MAX_BACKOFF);
        let mut cursor = ReadinessSmokeAttemptCursor::default();
        for attempt in 1..=attempts {
            let transaction_index = cursor.current_index();
            let transaction = &plan.transactions[transaction_index];
            match self
                .submit_and_wait_for_commit(transaction, plan.commit_options)
                .await
            {
                Ok(commit) => {
                    let status = self.fetch_status_snapshot().await.ok();
                    return Ok(ReadinessSmokeOutcome {
                        total_elapsed: started.elapsed(),
                        attempt,
                        commit,
                        status,
                    });
                }
                Err(err @ ToriiError::SmokeRejected { .. }) if cursor.is_pinned() => {
                    return Err(err);
                }
                Err(err) if attempt < attempts => {
                    cursor.record_failure(transaction_index, &err);
                    sleep(backoff).await;
                    backoff = (backoff.saturating_mul(2)).min(MAX_BACKOFF);
                }
                Err(err) => return Err(err),
            }
        }
        Err(ToriiError::Timeout {
            context: format!("smoke readiness attempts exhausted ({attempts})"),
        })
    }
    /// URL of the canonical `/v1/configuration` endpoint.
    pub fn configuration_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint(torii_uri::CONFIGURATION)
    }
    /// URL of the read-only `/v1/nexus/lifecycle` status endpoint.
    pub fn nexus_lifecycle_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/nexus/lifecycle")
    }
    /// Submit a Norito-encoded transaction to Torii.
    pub async fn submit_transaction(&self, payload: &[u8]) -> ToriiResult<()> {
        let url = self.transaction_endpoint()?;
        let response = self
            .http
            .post(url)
            .header("Content-Type", NORITO_MIME_TYPE)
            .body(payload.to_vec())
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let reject_code = reject_code_from_headers(response.headers());
            let body =
                read_bounded_response(response, MAX_ERROR_RESPONSE_BYTES, "transaction error")
                    .await?;
            let message = error_message_from_body(body.as_ref());
            return Err(ToriiError::UnexpectedStatus {
                status,
                reject_code,
                message,
            });
        }
        Ok(())
    }
    /// Submit a Norito-encoded query to Torii and return the raw response body.
    pub async fn submit_query(&self, payload: &[u8]) -> ToriiResult<Vec<u8>> {
        let url = self.query_endpoint()?;
        let response = self
            .http
            .post(url)
            .header("Content-Type", NORITO_MIME_TYPE)
            .body(payload.to_vec())
            .send()
            .await?;
        if response.status().is_success() {
            return read_bounded_response(response, MAX_QUERY_RESPONSE_BYTES, "query").await;
        }
        let status = response.status();
        let reject_code = reject_code_from_headers(response.headers());
        let body = read_bounded_response(response, MAX_ERROR_RESPONSE_BYTES, "query error").await?;
        let message = error_message_from_body(body.as_ref());
        Err(ToriiError::UnexpectedStatus {
            status,
            reject_code,
            message,
        })
    }
    /// Submit a signed transaction using its canonical versioned Norito encoding.
    pub async fn submit_signed_transaction(
        &self,
        transaction: &SignedTransaction,
    ) -> ToriiResult<()> {
        let bytes = transaction.encode_versioned();
        self.submit_transaction(&bytes).await
    }
    /// Submit a signed transaction and wait until local Torii reports it as committed.
    ///
    /// This helper is primarily intended for readiness smoke checks in local tooling.
    pub async fn submit_and_wait_for_commit(
        &self,
        transaction: &SignedTransaction,
        options: SmokeCommitOptions,
    ) -> ToriiResult<SmokeCommitSnapshot> {
        let tx_hash = transaction.hash();
        let tx_hash_str = tx_hash.to_string();
        let started = Instant::now();
        // Stream notifications are latency optimizations for this exact-hash
        // readiness check. Torii may temporarily throttle WebSocket handshakes
        // while all peers start; keep the canonical HTTP status reconciliation
        // authoritative instead of failing an otherwise healthy localnet.
        let block_stream = match self.block_stream().await {
            Ok(stream) => Some(stream),
            Err(ToriiError::RateLimited { .. }) => None,
            Err(error) => return Err(error),
        };
        let events_stream = match self.events_stream().await {
            Ok(stream) => Some(stream),
            Err(ToriiError::RateLimited { .. }) => None,
            Err(error) => return Err(error),
        };
        let mut block_rx = block_stream.as_ref().map(BlockStream::subscribe);
        let mut event_rx = events_stream.as_ref().map(EventStream::subscribe);
        let signed_bytes = transaction.encode_versioned();
        let mut admission_outcome_unknown = match self.submit_transaction(&signed_bytes).await {
            Ok(()) => false,
            Err(err) if err.confirms_existing_submission() => false,
            Err(err) if err.is_queue_plan_journal_outcome_unknown() => true,
            Err(err) => return Err(err),
        };
        let wait = async {
            let mut status_poll = tokio::time::interval(Duration::from_millis(250));
            let retry_start = tokio::time::Instant::now() + SMOKE_EXACT_RESUBMIT_DELAY;
            let mut exact_resubmit =
                tokio::time::interval_at(retry_start, SMOKE_EXACT_RESUBMIT_INTERVAL);
            exact_resubmit.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = status_poll.tick() => {
                        let status = match self
                            .fetch_smoke_transaction_status(tx_hash_str.as_str())
                            .await
                        {
                            Ok(status) => status,
                            Err(_err) if admission_outcome_unknown => None,
                            Err(err) => return Err(err),
                        };
                        if let Some(status) = status {
                            match status {
                                SmokeTransactionStatus::Queued => {
                                    admission_outcome_unknown = false;
                                }
                                SmokeTransactionStatus::Committed(height) => return Ok(height),
                                SmokeTransactionStatus::Rejected(reason) => {
                                    return Err(ToriiError::SmokeRejected {
                                        hash: tx_hash_str.clone(),
                                        reason,
                                    });
                                }
                                SmokeTransactionStatus::Expired => {
                                    return Err(ToriiError::SmokeRejected {
                                        hash: tx_hash_str.clone(),
                                        reason: "expired".to_owned(),
                                    });
                                }
                            }
                        }
                    }
                    _ = exact_resubmit.tick(), if admission_outcome_unknown => {
                        match self.submit_transaction(&signed_bytes).await {
                            Ok(()) => admission_outcome_unknown = false,
                            Err(err) if err.confirms_existing_submission() => {
                                admission_outcome_unknown = false;
                            }
                            Err(_err) => {}
                        }
                    }
                    message = async {
                        match &mut block_rx {
                            Some(receiver) => receiver.recv().await,
                            None => std::future::pending().await,
                        }
                    } => {
                        match message {
                            Ok(BlockStreamEvent::Block { block, .. }) => {
                                if let Some(result) =
                                    smoke_transaction_result_in_block(block.as_ref(), &tx_hash)
                                {
                                    return result;
                                }
                            }
                            Ok(BlockStreamEvent::DecodeError { error }) => {
                                if !admission_outcome_unknown {
                                    return Err(ToriiError::Decode(error.message));
                                }
                            }
                            Ok(BlockStreamEvent::Closed) => {}
                            Ok(BlockStreamEvent::Lagged { .. } | BlockStreamEvent::Text { .. }) => {}
                            Err(RecvError::Lagged(_)) | Err(RecvError::Closed) => {}
                        }
                    }
                    message = async {
                        match &mut event_rx {
                            Some(receiver) => receiver.recv().await,
                            None => std::future::pending().await,
                        }
                    } => {
                        match message {
                            Ok(EventStreamEvent::Event { event, .. }) => {
                                if let EventBox::Pipeline(PipelineEventBox::Transaction(tx_event)) = event.as_ref()
                                    && tx_event.hash() == &tx_hash
                                {
                                    match tx_event.status() {
                                        iroha_data_model::events::pipeline::TransactionStatus::Rejected(reason) => {
                                            return Err(ToriiError::SmokeRejected {
                                                hash: tx_hash_str.clone(),
                                                reason: format!("{reason:?}"),
                                            });
                                        }
                                        iroha_data_model::events::pipeline::TransactionStatus::Expired => {
                                            return Err(ToriiError::SmokeRejected {
                                                hash: tx_hash_str.clone(),
                                                reason: "expired".to_owned(),
                                            });
                                        }
                                        iroha_data_model::events::pipeline::TransactionStatus::Approved => {
                                            if let Some(height) =
                                                tx_event.block_height().map(std::num::NonZeroU64::get)
                                            {
                                                return Ok(height);
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            Ok(EventStreamEvent::DecodeError { error }) => {
                                if !admission_outcome_unknown {
                                    return Err(ToriiError::Decode(error.message));
                                }
                            }
                            Ok(EventStreamEvent::Closed) => {}
                            Ok(EventStreamEvent::Lagged { .. } | EventStreamEvent::Text { .. }) => {}
                            Err(RecvError::Lagged(_)) | Err(RecvError::Closed) => {}
                        }
                    }
                }
            }
        };
        let result = match tokio::time::timeout(options.timeout, wait).await {
            Ok(result) => result,
            Err(_) if admission_outcome_unknown => Err(ToriiError::SmokeAdmissionOutcomeUnknown {
                hash: tx_hash_str.clone(),
            }),
            Err(_) => Err(ToriiError::Timeout {
                context: format!("smoke commit {tx_hash_str}"),
            }),
        };
        drop(block_stream);
        drop(events_stream);
        match result {
            Ok(height) => Ok(SmokeCommitSnapshot {
                tx_hash,
                block_height: height,
                elapsed: started.elapsed(),
            }),
            Err(err) => Err(match err {
                ToriiError::Timeout { .. } => ToriiError::Timeout {
                    context: format!("smoke commit {tx_hash_str}"),
                },
                other => other,
            }),
        }
    }
    async fn fetch_smoke_transaction_status(
        &self,
        tx_hash: &str,
    ) -> ToriiResult<Option<SmokeTransactionStatus>> {
        if let Some(status) = self.fetch_pipeline_transaction_status(tx_hash).await? {
            return Ok(Some(status));
        }
        self.fetch_explorer_transaction_status(tx_hash).await
    }
    async fn fetch_pipeline_transaction_status(
        &self,
        tx_hash: &str,
    ) -> ToriiResult<Option<SmokeTransactionStatus>> {
        let url = self.pipeline_transaction_status_endpoint()?;
        let response = self
            .http
            .get(url)
            .query(&[("hash", tx_hash)])
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let bytes =
            read_bounded_response(response, MAX_JSON_RESPONSE_BYTES, "pipeline status").await?;
        let value = decode_bounded_json_response(&bytes, "pipeline status")?;
        parse_pipeline_smoke_status(&value)
    }
    async fn fetch_explorer_transaction_status(
        &self,
        tx_hash: &str,
    ) -> ToriiResult<Option<SmokeTransactionStatus>> {
        let url = self.explorer_transaction_endpoint(tx_hash)?;
        let response = self
            .http
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let bytes =
            read_bounded_response(response, MAX_JSON_RESPONSE_BYTES, "Explorer status").await?;
        let value = decode_bounded_json_response(&bytes, "Explorer status")?;
        parse_explorer_smoke_status(&value)
    }
    /// Submit a signed query and decode the response into a typed [`QueryOutput`].
    pub async fn execute_query(&self, query: &SignedQuery) -> ToriiResult<QueryOutput> {
        let response = self.submit_query(&query.encode_versioned()).await?;
        decode_norito_with_alignment(&response)
    }
    /// Fetch the Torii status snapshot.
    pub async fn fetch_status(&self) -> ToriiResult<TelemetryStatus> {
        let url = self.status_endpoint()?;
        let response = self
            .http
            .get(url)
            .header(reqwest::header::ACCEPT, NORITO_MIME_TYPE)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let body = read_bounded_response(response, MAX_STATUS_RESPONSE_BYTES, "status").await?;
        decode_norito_with_alignment(body.as_ref()).or_else(|_| {
            norito::with_decode_limits(norito::canonical_decode_limits(body.len()), || {
                norito::codec::decode_adaptive(body.as_ref())
            })
            .map_err(|err| ToriiError::Decode(err.to_string()))
        })
    }
    /// Fetch a telemetry snapshot together with derived metrics.
    pub async fn fetch_status_snapshot(&self) -> ToriiResult<ToriiStatusSnapshot> {
        let status = self.fetch_status().await?;
        let timestamp = Instant::now();
        let metrics = {
            let mut guard = self.status_state.lock().await;
            guard.record(timestamp, &status)
        };
        Ok(ToriiStatusSnapshot::new(timestamp, status, metrics))
    }
    /// Fetch the exact reducer-owned Sumeragi v2 status snapshot.
    pub async fn fetch_sumeragi_status(&self) -> ToriiResult<SumeragiV2Status> {
        let url = self.sumeragi_status_endpoint()?;
        let request = build_operator_get_request(
            &self.http,
            &self.default_headers,
            self.network_id,
            self.operator_signing_context.as_ref(),
            url,
        )?;
        let response = self.http.execute(request).await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let body = read_bounded_sumeragi_response(response).await?;
        let status: SumeragiV2Status = decode_norito_with_alignment(&body)?;
        status
            .validate()
            .map_err(|error| ToriiError::Decode(error.to_string()))?;
        Ok(status)
    }
    /// Fetch non-authoritative Sumeragi pipeline, queue, election, and lane diagnostics.
    pub async fn fetch_sumeragi_diagnostics(&self) -> ToriiResult<SumeragiDiagnosticsStatus> {
        let url = self.sumeragi_diagnostics_endpoint()?;
        let request = build_operator_get_request(
            &self.http,
            &self.default_headers,
            self.network_id,
            self.operator_signing_context.as_ref(),
            url,
        )?;
        let response = self.http.execute(request).await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let body = read_bounded_sumeragi_response(response).await?;
        let diagnostics: SumeragiDiagnosticsStatus = decode_norito_with_alignment(&body)?;
        if let Some(npos) = diagnostics.npos {
            npos.validate()
                .map_err(|reason| ToriiError::Decode(reason.to_owned()))?;
        }
        for envelope in &diagnostics.lane_relay_envelopes {
            envelope
                .verify()
                .map_err(|error| ToriiError::Decode(error.to_string()))?;
        }
        Ok(diagnostics)
    }
    /// Fetch the Torii node configuration as a Norito JSON value.
    pub async fn fetch_configuration(&self) -> ToriiResult<json::Value> {
        let url = self.configuration_endpoint()?;
        self.fetch_json(url).await
    }
    /// Fetch the native MCP capabilities payload.
    pub async fn fetch_mcp_capabilities(&self) -> ToriiResult<json::Value> {
        let url = self.mcp_endpoint()?;
        self.fetch_json(url).await
    }
    /// Run the local Mochi MCP smoke sequence against `/v1/mcp`.
    pub async fn validate_local_mcp(&self) -> ToriiResult<LocalMcpProbeResult> {
        let capabilities = self.fetch_mcp_capabilities().await?;
        let initialize = self.mcp_initialize().await?;
        self.mcp_initialized().await?;
        let tools = self.mcp_tools_list().await?;
        LocalMcpProbeResult::from_documents(&capabilities, &initialize, &tools)
    }
    /// Fetch and validate the exact current Nexus lane catalog commitment.
    pub async fn fetch_lane_lifecycle_status(&self) -> ToriiResult<LaneLifecycleStatusV1> {
        let url = self.nexus_lifecycle_endpoint()?;
        let response = self
            .http
            .get(url)
            .header(reqwest::header::ACCEPT, NORITO_MIME_TYPE)
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let reject_code = reject_code_from_headers(response.headers());
            let body =
                read_bounded_response(response, MAX_ERROR_RESPONSE_BYTES, "lane lifecycle error")
                    .await?;
            let message = error_message_from_body(body.as_ref());
            return Err(ToriiError::UnexpectedStatus {
                status,
                reject_code,
                message,
            });
        }
        let body =
            read_bounded_response(response, MAX_STATUS_RESPONSE_BYTES, "lane lifecycle").await?;
        let status: LaneLifecycleStatusV1 = decode_norito_with_alignment(body.as_ref())?;
        status
            .validate()
            .map_err(|err| ToriiError::Decode(format!("invalid lane lifecycle status: {err}")))?;
        Ok(status)
    }
    /// Submit and wait for a consensus-replayed Nexus lane lifecycle transaction.
    ///
    /// The transaction is bound to the exact network configured on this client.
    ///
    /// The status commitment is fetched once. A stale catalog or missing
    /// `CanSetParameters` permission is surfaced as a transaction rejection and
    /// is never silently retried against a different topology.
    pub async fn apply_lane_lifecycle(
        &self,
        network_id: NetworkId,
        signer: &SigningAuthority,
        plan: LaneLifecyclePlan,
    ) -> ToriiResult<SmokeCommitSnapshot> {
        let configured_network_id = self.require_network_id()?;
        if network_id != configured_network_id {
            return Err(ToriiError::SignedQueryContext(format!(
                "lane lifecycle network id `{network_id}` does not match the configured client network id `{configured_network_id}`"
            )));
        }
        let status = self.fetch_lane_lifecycle_status().await?;
        if !status.nexus_enabled {
            return Err(ToriiError::Decode(
                "Nexus lane lifecycle is disabled on the serving node".to_owned(),
            ));
        }
        let current_catalog = status
            .validate()
            .map_err(|err| ToriiError::Decode(format!("invalid lane lifecycle status: {err}")))?;
        let expected_catalog = current_catalog
            .apply_lifecycle(&plan)
            .map_err(|err| ToriiError::Decode(format!("invalid lane lifecycle plan: {err}")))?;
        let previous_incarnation_root = status.incarnation_root;
        let transaction = build_lane_lifecycle_transaction(network_id, signer, &status, plan)?;
        let options = SmokeCommitOptions::default();
        let committed = self
            .submit_and_wait_for_commit(&transaction, options)
            .await?;
        // Block persistence precedes WSV publication. Do not report success to
        // storage-reset callers until the committed catalog and its fresh
        // incarnation root are visible through the state-generation snapshot.
        let deadline = tokio::time::Instant::now() + options.timeout;
        loop {
            let observed = self.fetch_lane_lifecycle_status().await?;
            let observed_catalog = observed.validate().map_err(|err| {
                ToriiError::Decode(format!("invalid post-commit lifecycle status: {err}"))
            })?;
            if observed_catalog == expected_catalog
                && observed.incarnation_root != previous_incarnation_root
            {
                return Ok(committed);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(ToriiError::Timeout {
                    context: format!(
                        "lane lifecycle apply {} (committed at block {})",
                        committed.tx_hash, committed.block_height
                    ),
                });
            }
            sleep(Duration::from_millis(100)).await;
        }
    }
    /// Fetch the exposed metrics payload as plain text (Prometheus format).
    pub async fn fetch_metrics(&self) -> ToriiResult<String> {
        let url = self.metrics_endpoint()?;
        let response = self.http.get(url).send().await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let body = read_bounded_response(response, MAX_METRICS_RESPONSE_BYTES, "metrics").await?;
        String::from_utf8(body)
            .map_err(|_| ToriiError::Decode("metrics response is not UTF-8".to_owned()))
    }
    /// Fetch and parse the Prometheus metrics payload into a structured snapshot.
    pub async fn fetch_metrics_snapshot(&self) -> ToriiResult<ToriiMetricsSnapshot> {
        let body = self.fetch_metrics().await?;
        Ok(ToriiMetricsSnapshot::from_prometheus(Instant::now(), &body))
    }
    /// Fetch a single block from the Explorer API.
    pub async fn fetch_block(&self, height: u64) -> ToriiResult<Option<ExplorerBlockRecord>> {
        let url = self.block_by_height_endpoint(height)?;
        let response = self.http.get(url).send().await?;
        match response.status() {
            StatusCode::OK => {
                let value = read_bounded_json_response(response, "Explorer block").await?;
                ExplorerBlockRecord::from_json(&value).map(Some)
            }
            StatusCode::NOT_FOUND => Ok(None),
            status => Err(ToriiError::UnexpectedStatus {
                status,
                reject_code: None,
                message: None,
            }),
        }
    }
    /// List blocks from the Explorer API using optional pagination parameters.
    pub async fn fetch_blocks_page(
        &self,
        query: ExplorerBlocksQuery,
    ) -> ToriiResult<ExplorerBlocksPage> {
        let url = self.blocks_endpoint()?;
        let mut request = self.http.get(url);
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(offset) = query.offset_height {
            params.push(("offset_height", offset.to_string()));
        }
        if let Some(limit) = query.limit {
            params.push(("limit", limit.to_string()));
        }
        if !params.is_empty() {
            request = request.query(&params);
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let value = read_bounded_json_response(response, "Explorer blocks").await?;
        ExplorerBlocksPage::from_json(&value)
    }
    /// Fetch Explorer account summaries from `/v1/explorer/accounts`.
    pub async fn fetch_explorer_accounts_page(
        &self,
        query: ExplorerAccountsQuery,
    ) -> ToriiResult<ExplorerAccountsPage> {
        let url = self.explorer_accounts_endpoint()?;
        let mut request = self.http.get(url);
        let mut params: Vec<(&str, String)> = Vec::new();
        append_explorer_cursor_params(
            &mut params,
            query.cursor,
            query.limit,
            "explorer accounts query",
        )?;
        if let Some(domain) = query.domain {
            let trimmed = domain.trim();
            if !trimmed.is_empty() {
                params.push(("domain", trimmed.to_owned()));
            }
        }
        if let Some(asset) = query.with_asset {
            let trimmed = asset.trim();
            if !trimmed.is_empty() {
                params.push(("with_asset", trimmed.to_owned()));
            }
        }
        if !params.is_empty() {
            request = request.query(&params);
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let value = read_bounded_json_response(response, "Explorer accounts").await?;
        ExplorerAccountsPage::from_json(&value)
    }
    /// Fetch Explorer domain summaries from `/v1/explorer/domains`.
    pub async fn fetch_explorer_domains_page(
        &self,
        query: ExplorerDomainsQuery,
    ) -> ToriiResult<ExplorerDomainsPage> {
        let url = self.explorer_domains_endpoint()?;
        let mut request = self.http.get(url);
        let mut params: Vec<(&str, String)> = Vec::new();
        append_explorer_cursor_params(
            &mut params,
            query.cursor,
            query.limit,
            "explorer domains query",
        )?;
        if let Some(owned_by) = query
            .owned_by
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            params.push(("owned_by", owned_by.to_owned()));
        }
        if !params.is_empty() {
            request = request.query(&params);
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let value = read_bounded_json_response(response, "Explorer domains").await?;
        ExplorerDomainsPage::from_json(&value)
    }
    /// Fetch Explorer asset definitions from `/v1/explorer/asset-definitions`.
    pub async fn fetch_explorer_asset_definitions_page(
        &self,
        query: ExplorerAssetDefinitionsQuery,
    ) -> ToriiResult<ExplorerAssetDefinitionsPage> {
        let url = self.explorer_asset_definitions_endpoint()?;
        let mut request = self.http.get(url);
        let mut params: Vec<(&str, String)> = Vec::new();
        append_explorer_cursor_params(
            &mut params,
            query.cursor,
            query.limit,
            "explorer asset definitions query",
        )?;
        if let Some(domain) = query
            .domain
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            params.push(("domain", domain.to_owned()));
        }
        if let Some(owned_by) = query
            .owned_by
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            params.push(("owned_by", owned_by.to_owned()));
        }
        if !params.is_empty() {
            request = request.query(&params);
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let value = read_bounded_json_response(response, "Explorer asset definitions").await?;
        ExplorerAssetDefinitionsPage::from_json(&value)
    }
    /// Fetch Explorer asset summaries from `/v1/explorer/assets`.
    pub async fn fetch_explorer_assets_page(
        &self,
        query: ExplorerAssetsQuery,
    ) -> ToriiResult<ExplorerAssetsPage> {
        let url = self.explorer_assets_endpoint()?;
        let mut request = self.http.get(url);
        let mut params: Vec<(&str, String)> = Vec::new();
        append_explorer_cursor_params(
            &mut params,
            query.cursor,
            query.limit,
            "explorer assets query",
        )?;
        if let Some(owned_by) = query
            .owned_by
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            params.push(("owned_by", owned_by.to_owned()));
        }
        if let Some(definition) = query
            .definition
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            params.push(("definition", definition.to_owned()));
        }
        if !params.is_empty() {
            request = request.query(&params);
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let value = read_bounded_json_response(response, "Explorer assets").await?;
        ExplorerAssetsPage::from_json(&value)
    }
    /// Fetch Explorer NFT summaries from `/v1/explorer/nfts`.
    pub async fn fetch_explorer_nfts_page(
        &self,
        query: ExplorerNftsQuery,
    ) -> ToriiResult<ExplorerNftsPage> {
        let url = self.explorer_nfts_endpoint()?;
        let mut request = self.http.get(url);
        let mut params: Vec<(&str, String)> = Vec::new();
        append_explorer_cursor_params(
            &mut params,
            query.cursor,
            query.limit,
            "explorer nfts query",
        )?;
        if let Some(owned_by) = query
            .owned_by
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            params.push(("owned_by", owned_by.to_owned()));
        }
        if let Some(domain) = query
            .domain
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            params.push(("domain", domain.to_owned()));
        }
        if !params.is_empty() {
            request = request.query(&params);
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let value = read_bounded_json_response(response, "Explorer NFTs").await?;
        ExplorerNftsPage::from_json(&value)
    }
    /// Fetch Explorer RWA summaries from `/v1/explorer/rwas`.
    pub async fn fetch_explorer_rwas_page(
        &self,
        query: ExplorerRwasQuery,
    ) -> ToriiResult<ExplorerRwasPage> {
        let url = self.explorer_rwas_endpoint()?;
        let mut request = self.http.get(url);
        let mut params: Vec<(&str, String)> = Vec::new();
        append_explorer_cursor_params(
            &mut params,
            query.cursor,
            query.limit,
            "explorer rwas query",
        )?;
        if let Some(owned_by) = query
            .owned_by
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            params.push(("owned_by", owned_by.to_owned()));
        }
        if let Some(domain) = query
            .domain
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            params.push(("domain", domain.to_owned()));
        }
        if !params.is_empty() {
            request = request.query(&params);
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let value = read_bounded_json_response(response, "Explorer RWAs").await?;
        ExplorerRwasPage::from_json(&value)
    }
    /// List triggers exposed by `/v1/triggers`.
    pub async fn list_triggers(&self, query: TriggerListQuery) -> ToriiResult<TriggerListPage> {
        let url = self.triggers_endpoint()?;
        let mut request = self.http.get(url);
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(namespace) = query
            .namespace
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            params.push(("namespace", namespace.to_owned()));
        }
        if let Some(authority) = query
            .authority
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            params.push(("authority", authority.to_owned()));
        }
        if let Some(limit) = query.limit {
            params.push(("limit", limit.to_string()));
        }
        if let Some(offset) = query.offset {
            params.push(("offset", offset.to_string()));
        }
        if !params.is_empty() {
            request = request.query(&params);
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let value = read_bounded_json_response(response, "trigger list").await?;
        TriggerListPage::from_json(&value)
    }
    /// Fetch a single trigger definition.
    pub async fn get_trigger(&self, trigger_id: &str) -> ToriiResult<Option<TriggerRecord>> {
        let url = self.trigger_record_endpoint(trigger_id)?;
        let response = self.http.get(url).send().await?;
        match response.status() {
            StatusCode::OK => {
                let value = read_bounded_json_response(response, "trigger record").await?;
                TriggerRecord::from_json(&value, &format!("trigger response `{trigger_id}`"))
                    .map(Some)
            }
            StatusCode::NOT_FOUND => Ok(None),
            status => Err(ToriiError::UnexpectedStatus {
                status,
                reject_code: None,
                message: None,
            }),
        }
    }
    /// Register or update a trigger definition.
    pub async fn register_trigger(&self, trigger: &json::Value) -> ToriiResult<TriggerRecord> {
        let url = self.triggers_endpoint()?;
        let payload = json::to_vec(trigger)
            .map_err(|err| decode_error("trigger registration", err.to_string()))?;
        let response = self
            .http
            .post(url)
            .header("content-type", "application/json")
            .body(payload)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let value = read_bounded_json_response(response, "trigger registration").await?;
        TriggerRecord::from_json(&value, "trigger registration response")
    }
    /// Delete a trigger definition by id.
    pub async fn delete_trigger(&self, trigger_id: &str) -> ToriiResult<bool> {
        let url = self.trigger_record_endpoint(trigger_id)?;
        let response = self.http.delete(url).send().await?;
        match response.status() {
            StatusCode::OK | StatusCode::ACCEPTED | StatusCode::NO_CONTENT => Ok(true),
            StatusCode::NOT_FOUND => Ok(false),
            status => Err(ToriiError::UnexpectedStatus {
                status,
                reject_code: None,
                message: None,
            }),
        }
    }
    /// Establish a canonical WebSocket connection to `/v1/blocks/stream`.
    pub async fn connect_block_stream(&self) -> ToriiResult<ToriiWebSocket> {
        self.connect_ws(self.block_stream_endpoint()?).await
    }
    /// Establish a canonical WebSocket connection to `/v1/events/ws`.
    pub async fn connect_events_stream(&self) -> ToriiResult<ToriiWebSocket> {
        self.connect_ws(self.events_stream_endpoint()?).await
    }
    /// Subscribe to blocks from height one on `/v1/blocks/stream`.
    pub async fn subscribe_block_stream(&self) -> ToriiResult<WsSubscription> {
        self.subscribe_block_stream_from(NonZeroU64::MIN).await
    }
    /// Subscribe to blocks from the requested one-indexed height.
    pub async fn subscribe_block_stream_from(
        &self,
        height: NonZeroU64,
    ) -> ToriiResult<WsSubscription> {
        let request = BlockSubscriptionRequest::new(height);
        let first_message =
            norito::to_bytes(&request).expect("canonical block subscription request must encode");
        self.subscribe_ws(self.block_stream_endpoint()?, first_message)
            .await
    }
    /// Subscribe to all Explorer-facing event categories on `/v1/events/ws`.
    pub async fn subscribe_events_stream(&self) -> ToriiResult<WsSubscription> {
        let request = EventSubscriptionRequest::new(canonical_event_filters());
        let first_message =
            norito::to_bytes(&request).expect("canonical event subscription request must encode");
        self.subscribe_ws(self.events_stream_endpoint()?, first_message)
            .await
    }
    /// Subscribe to `/v1/blocks/stream` and publish decoded [`SignedBlock`] events.
    pub async fn block_stream(&self) -> ToriiResult<BlockStream> {
        let subscription = self.subscribe_block_stream().await?;
        Ok(BlockStream::new(subscription))
    }
    /// Subscribe to `/v1/events/ws` and publish decoded [`EventBox`] events.
    pub async fn events_stream(&self) -> ToriiResult<EventStream> {
        let subscription = self.subscribe_events_stream().await?;
        Ok(EventStream::new(subscription))
    }
    fn http_endpoint(&self, path: &str) -> ToriiResult<Url> {
        self.http_base
            .join(path.trim_start_matches('/'))
            .map_err(ToriiError::InvalidEndpoint)
    }
    fn ws_endpoint(&self, path: &str) -> ToriiResult<Url> {
        self.ws_base
            .join(path.trim_start_matches('/'))
            .map_err(ToriiError::InvalidEndpoint)
    }
    async fn fetch_json(&self, url: Url) -> ToriiResult<json::Value> {
        let response = self.http.get(url).send().await?;
        if !response.status().is_success() {
            return Err(response_status_error(&response));
        }
        read_bounded_json_response(response, "JSON API").await
    }
    async fn post_json(&self, url: Url, payload: &json::Value) -> ToriiResult<json::Value> {
        let body = json::to_vec(payload).map_err(|err| ToriiError::Decode(err.to_string()))?;
        let response = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(response_status_error(&response));
        }
        read_bounded_json_response(response, "JSON API").await
    }
    async fn post_notification(&self, url: Url, payload: &json::Value) -> ToriiResult<()> {
        let body = json::to_vec(payload).map_err(|err| ToriiError::Decode(err.to_string()))?;
        let response = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;
        if response.status() != StatusCode::ACCEPTED {
            return Err(response_status_error(&response));
        }
        let bytes = read_bounded_response(response, 1, "MCP notification").await?;
        if !bytes.is_empty() {
            return Err(decode_error(
                "mcp notifications/initialized",
                "expected an empty response body",
            ));
        }
        Ok(())
    }
    async fn mcp_initialize(&self) -> ToriiResult<json::Value> {
        let url = self.mcp_endpoint()?;
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {
                    "name": "mochi-local-sandbox",
                    "version": "1"
                }
            }
        });
        self.post_json(url, &payload).await
    }
    async fn mcp_initialized(&self) -> ToriiResult<()> {
        let url = self.mcp_endpoint()?;
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        self.post_notification(url, &payload).await
    }
    async fn mcp_tools_list(&self) -> ToriiResult<json::Value> {
        let url = self.mcp_endpoint()?;
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        });
        self.post_json(url, &payload).await
    }
    async fn connect_ws(&self, url: Url) -> ToriiResult<ToriiWebSocket> {
        let mut request = url
            .to_string()
            .into_client_request()
            .map_err(|err| ToriiError::InvalidWebSocketRequest(err.to_string()))?;
        {
            let headers = request.headers_mut();
            for (name, value) in self.default_headers.iter() {
                headers.insert(name.clone(), value.clone());
            }
            headers.insert(
                SEC_WEBSOCKET_PROTOCOL,
                HeaderValue::from_static(NORITO_V1_WEBSOCKET_SUBPROTOCOL),
            );
        }
        let (stream, response) = connect_async(request)
            .await
            .map_err(websocket_connect_error)?;
        let selected_protocol = response
            .headers()
            .get(SEC_WEBSOCKET_PROTOCOL)
            .and_then(|value| value.to_str().ok());
        if selected_protocol != Some(NORITO_V1_WEBSOCKET_SUBPROTOCOL) {
            return Err(ToriiError::InvalidWebSocketRequest(format!(
                "Torii WebSocket did not select required subprotocol `{NORITO_V1_WEBSOCKET_SUBPROTOCOL}`"
            )));
        }
        Ok(stream)
    }
    async fn subscribe_ws(
        &self,
        endpoint: Url,
        first_message: Vec<u8>,
    ) -> ToriiResult<WsSubscription> {
        let mut stream = self.connect_ws(endpoint).await?;
        stream.send(Message::Binary(first_message.into())).await?;
        let (sender, _receiver) = broadcast::channel(128);
        let forwarder = sender.clone();
        let handle: JoinHandle<()> = tokio::spawn(async move {
            let mut closed_emitted = false;
            while let Some(message) = stream.next().await {
                match message {
                    Ok(Message::Binary(data)) => {
                        let _ = forwarder.send(WsFrame::Binary(data.to_vec()));
                    }
                    Ok(Message::Text(text)) => {
                        let _ = forwarder.send(WsFrame::Error(format!(
                            "Torii canonical Norito WebSocket sent an unexpected text frame: {text}"
                        )));
                        break;
                    }
                    Ok(Message::Close(_)) => {
                        let _ = forwarder.send(WsFrame::Closed);
                        closed_emitted = true;
                        break;
                    }
                    Ok(Message::Frame(_)) | Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                    Err(err) => {
                        let _ = forwarder.send(WsFrame::Error(err.to_string()));
                        break;
                    }
                }
            }
            if !closed_emitted {
                let _ = forwarder.send(WsFrame::Closed);
            }
        });
        Ok(WsSubscription { sender, handle })
    }
}
#[cfg(test)]
include!("torii/commit_wait_test_support.rs");
/// Broadcast-backed WebSocket subscription.
#[derive(Debug)]
pub struct WsSubscription {
    /// Channel distributing frames to subscribers.
    sender: broadcast::Sender<WsFrame>,
    /// Join handle for the forwarding task.
    handle: JoinHandle<()>,
}
impl WsSubscription {
    /// Acquire a receiver that yields binary frames pushed by the subscription.
    pub fn subscribe(&self) -> broadcast::Receiver<WsFrame> {
        self.sender.subscribe()
    }
    /// Abort the underlying forwarding task.
    pub fn abort(&self) {
        if !self.handle.is_finished() {
            self.handle.abort();
        }
    }
    /// Check if the forwarding task has completed.
    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }
}
impl Drop for WsSubscription {
    fn drop(&mut self) {
        self.abort();
    }
}
/// Stage of decoding when a failure occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockDecodeStage {
    /// Failed to parse the Norito frame.
    Frame,
    /// Failed to decode the `SignedBlock` payload.
    Block,
    /// Underlying WebSocket stream aborted.
    Stream,
}
/// Details about a block stream decoding failure.
#[derive(Debug, Clone)]
pub struct BlockStreamDecodeError {
    /// Stage where decoding failed.
    pub stage: BlockDecodeStage,
    /// Length of the raw frame that triggered the error, if known.
    pub raw_len: usize,
    /// Human-readable error description.
    pub message: String,
}
impl BlockStreamDecodeError {
    fn new(stage: BlockDecodeStage, raw_len: usize, message: impl Into<String>) -> Self {
        Self {
            stage,
            raw_len,
            message: message.into(),
        }
    }
}
/// Lightweight view over fields commonly displayed for blocks in the UI.
#[derive(Debug, Clone)]
pub struct BlockSummary {
    /// Block height (`1`-indexed).
    pub height: u64,
    /// Hex-encoded block hash.
    pub hash_hex: String,
    /// Number of external transactions in the block.
    pub transaction_count: usize,
    /// Number of rejected transactions recorded in the block results.
    pub rejected_transaction_count: usize,
    /// Number of time-triggered entrypoints executed in the block.
    pub time_trigger_count: usize,
    /// Number of validator signatures attached to the block.
    pub signature_count: usize,
    /// View change index recorded by Sumeragi.
    pub view_change_index: u64,
    /// Unix timestamp of block creation in milliseconds.
    pub creation_time_ms: u64,
    /// Whether the block is the genesis block.
    pub is_genesis: bool,
}
impl BlockSummary {
    fn from_block(block: &SignedBlock) -> Self {
        let header = block.header();
        let transaction_count = block.external_entrypoint_count();
        let (time_trigger_count, rejected_transaction_count) = if block.has_results() {
            let time_triggers = block.time_triggers();
            let rejected = (0..transaction_count)
                .filter(|idx| block.error(*idx).is_some())
                .count();
            (time_triggers.len(), rejected)
        } else {
            (0, 0)
        };
        let signature_count = block.signatures().len();
        Self {
            height: header.height().get(),
            hash_hex: block.hash().to_string(),
            transaction_count,
            rejected_transaction_count,
            time_trigger_count,
            signature_count,
            view_change_index: header.view_change_index(),
            creation_time_ms: header
                .creation_time()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            is_genesis: header.is_genesis(),
        }
    }
}
/// Events emitted by the decoded block stream helper.
#[derive(Debug, Clone)]
pub enum BlockStreamEvent {
    /// Successfully decoded block payload.
    Block {
        /// Summary used by UI presenters.
        summary: BlockSummary,
        /// Shared block instance for richer viewers.
        block: Arc<SignedBlock>,
        /// Length of the raw frame before decoding.
        raw_len: usize,
    },
    /// Received UTF-8 payload on the block stream (includes reconnection notices with peer aliases).
    Text { text: String },
    /// Decoding or transport error.
    DecodeError { error: BlockStreamDecodeError },
    /// Broadcast receiver lagged behind the producer.
    Lagged { skipped: usize },
    /// Stream closed cleanly.
    Closed,
}
/// High-level helper that consumes WebSocket frames and publishes decoded blocks.
pub struct BlockStream {
    subscription: WsSubscription,
    sender: broadcast::Sender<BlockStreamEvent>,
    initial_receiver: std::sync::Mutex<Option<broadcast::Receiver<BlockStreamEvent>>>,
    decode_handle: JoinHandle<()>,
}
impl BlockStream {
    fn new(subscription: WsSubscription) -> Self {
        let mut receiver = subscription.subscribe();
        let (sender, _) = broadcast::channel(128);
        let initial_receiver = sender.subscribe();
        let forwarder = sender.clone();
        let decode_handle = tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(WsFrame::Binary(frame)) => {
                        let raw_len = frame.len();
                        match norito::decode_from_bytes::<BlockMessage>(&frame) {
                            Ok(message) => {
                                let block: SignedBlock = message.into();
                                let block = Arc::<SignedBlock>::new(block);
                                let summary = BlockSummary::from_block(block.as_ref());
                                let event = BlockStreamEvent::Block {
                                    summary,
                                    block,
                                    raw_len,
                                };
                                let _ = forwarder.send(event);
                            }
                            Err(err) => {
                                let _ = forwarder.send(BlockStreamEvent::DecodeError {
                                    error: BlockStreamDecodeError::new(
                                        BlockDecodeStage::Frame,
                                        raw_len,
                                        err.to_string(),
                                    ),
                                });
                            }
                        }
                    }
                    Ok(WsFrame::Text(text)) => {
                        let truncated = if text.len() > 256 {
                            format!("{}…", &text[..255])
                        } else {
                            text
                        };
                        let _ = forwarder.send(BlockStreamEvent::Text { text: truncated });
                    }
                    Ok(WsFrame::Error(message)) => {
                        let _ = forwarder.send(BlockStreamEvent::DecodeError {
                            error: BlockStreamDecodeError::new(
                                BlockDecodeStage::Stream,
                                0,
                                message,
                            ),
                        });
                        break;
                    }
                    Ok(WsFrame::Closed) => {
                        let _ = forwarder.send(BlockStreamEvent::Closed);
                        break;
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        let _ = forwarder.send(BlockStreamEvent::Lagged {
                            skipped: lag_to_usize(skipped),
                        });
                    }
                    Err(RecvError::Closed) => {
                        let _ = forwarder.send(BlockStreamEvent::Closed);
                        break;
                    }
                }
            }
        });
        Self {
            subscription,
            sender,
            initial_receiver: std::sync::Mutex::new(Some(initial_receiver)),
            decode_handle,
        }
    }
    /// Acquire a receiver for decoded block events.
    pub fn subscribe(&self) -> broadcast::Receiver<BlockStreamEvent> {
        self.initial_receiver
            .lock()
            .expect("block stream receiver lock poisoned")
            .take()
            .unwrap_or_else(|| self.sender.subscribe())
    }
    /// Abort both the raw WebSocket subscription and decoder task.
    pub fn abort(&self) {
        self.subscription.abort();
        if !self.decode_handle.is_finished() {
            self.decode_handle.abort();
        }
    }
    /// Check whether the underlying tasks finished.
    pub fn is_finished(&self) -> bool {
        self.subscription.is_finished() && self.decode_handle.is_finished()
    }
}
impl Drop for BlockStream {
    fn drop(&mut self) {
        self.abort();
    }
}
/// Categories of events emitted by Torii.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventCategory {
    /// Pipeline event (blocks, transactions, warnings).
    Pipeline,
    /// Data event reflecting state changes.
    Data,
    /// Time trigger event.
    Time,
    /// Trigger execution request event.
    ExecuteTrigger,
    /// Trigger completion event.
    TriggerCompleted,
}
impl EventCategory {
    pub fn label(self) -> &'static str {
        match self {
            EventCategory::Pipeline => "Pipeline",
            EventCategory::Data => "Data",
            EventCategory::Time => "Time",
            EventCategory::ExecuteTrigger => "Execute Trigger",
            EventCategory::TriggerCompleted => "Trigger Completed",
        }
    }
}
/// Lightweight summary of a decoded Torii event.
#[derive(Debug, Clone)]
pub struct EventSummary {
    /// Event category used for grouping in the UI.
    pub category: EventCategory,
    /// Short label describing the specific variant.
    pub label: String,
    /// Optional human-readable detail string.
    pub detail: Option<String>,
}
impl EventSummary {
    fn from_event(event: &EventBox) -> Self {
        match event {
            EventBox::Pipeline(pipeline) => {
                let (label, detail) = pipeline_summary(pipeline);
                Self {
                    category: EventCategory::Pipeline,
                    label,
                    detail,
                }
            }
            EventBox::PipelineBatch(events) => Self {
                category: EventCategory::Pipeline,
                label: "Pipeline Batch".to_owned(),
                detail: Some(format!("count={}", events.len())),
            },
            EventBox::Data(data) => {
                let (label, detail) = data_summary(data.as_ref());
                Self {
                    category: EventCategory::Data,
                    label,
                    detail: Some(detail),
                }
            }
            EventBox::Time(time_event) => {
                let interval = time_event.interval();
                let since_ms = interval.since().as_millis();
                let length_ms = interval.length().as_millis();
                Self {
                    category: EventCategory::Time,
                    label: "Interval".to_owned(),
                    detail: Some(format!("since={since_ms}ms length={length_ms}ms")),
                }
            }
            EventBox::ExecuteTrigger(exec) => {
                let trigger = exec.trigger_id();
                let authority = exec.authority();
                Self {
                    category: EventCategory::ExecuteTrigger,
                    label: format!("Trigger {trigger:?}"),
                    detail: Some(format!("authority={authority:?}")),
                }
            }
            EventBox::TriggerCompleted(completed) => Self {
                category: EventCategory::TriggerCompleted,
                label: "Outcome".to_owned(),
                detail: Some(format!("{completed:?}")),
            },
        }
    }
}
fn pipeline_summary(event: &PipelineEventBox) -> (String, Option<String>) {
    match event {
        PipelineEventBox::Transaction(transaction) => {
            let status = transaction.status();
            let height = transaction
                .block_height()
                .map(|h| h.get().to_string())
                .unwrap_or_else(|| "—".to_owned());
            let detail = format!("hash={:?} height={height}", transaction.hash());
            (format!("Transaction {status:?}"), Some(detail))
        }
        PipelineEventBox::Block(block) => {
            let header = block.header();
            let detail = format!(
                "height={} view={}",
                header.height().get(),
                header.view_change_index(),
            );
            (
                format!("Block {status:?}", status = block.status()),
                Some(detail),
            )
        }
        PipelineEventBox::Warning(warning) => ("Warning".to_owned(), Some(format!("{warning:?}"))),
        PipelineEventBox::Merge(merge) => ("Merge".to_owned(), Some(format!("{merge:?}"))),
        PipelineEventBox::Witness(witness) => ("Witness".to_owned(), Some(format!("{witness:?}"))),
    }
}
#[allow(unreachable_patterns)]
fn data_summary(event: &DataEvent) -> (String, String) {
    match event {
        DataEvent::Peer(peer) => peer_event_summary(peer),
        DataEvent::Domain(domain) => domain_event_summary(domain),
        DataEvent::Account(account) => account_event_summary(account),
        DataEvent::Asset(asset) => asset_event_summary(asset),
        DataEvent::AssetDefinition(definition) => asset_definition_event_summary(definition),
        DataEvent::Trigger(trigger) => ("Trigger".to_owned(), format!("{trigger:?}")),
        DataEvent::Role(role) => ("Role".to_owned(), format!("{role:?}")),
        DataEvent::Configuration(config) => ("Configuration".to_owned(), format!("{config:?}")),
        DataEvent::Executor(executor) => ("Executor".to_owned(), format!("{executor:?}")),
        DataEvent::Proof(proof) => ("Proof".to_owned(), format!("{proof:?}")),
        DataEvent::VerifyingKey(key) => ("VerifyingKey".to_owned(), format!("{key:?}")),
        DataEvent::RuntimeUpgrade(upgrade) => ("RuntimeUpgrade".to_owned(), format!("{upgrade:?}")),
        DataEvent::SmartContract(contract) => ("SmartContract".to_owned(), format!("{contract:?}")),
        DataEvent::Soradns(event) => ("Soradns".to_owned(), format!("{event:?}")),
        DataEvent::Sorafs(event) => sorafs_event_summary(event),
        DataEvent::SpaceDirectory(directory) => {
            ("SpaceDirectory".to_owned(), format!("{directory:?}"))
        }
        _ => ("Data".to_owned(), format!("{event:?}")),
    }
}
fn peer_event_summary(event: &PeerEvent) -> (String, String) {
    match event {
        PeerEvent::Added(peer) => ("Peer added".to_owned(), format!("{peer}")),
        PeerEvent::Removed(peer) => ("Peer removed".to_owned(), format!("{peer}")),
    }
}
fn domain_event_summary(event: &DomainEvent) -> (String, String) {
    match event {
        DomainEvent::Created(domain) => ("Domain created".to_owned(), domain.id().to_string()),
        DomainEvent::Deleted(id) => ("Domain deleted".to_owned(), id.to_string()),
        DomainEvent::Account(account) => account_event_summary(&account.event),
        DomainEvent::Asset(asset) => asset_event_summary(&asset.event),
        DomainEvent::AssetDefinition(definition) => {
            asset_definition_event_summary(&definition.event)
        }
        DomainEvent::Nft(nft) => nft_event_summary(nft),
        DomainEvent::MetadataInserted(change) => (
            "Domain metadata inserted".to_owned(),
            format!("domain={} key={}", change.target(), change.key()),
        ),
        DomainEvent::MetadataRemoved(change) => (
            "Domain metadata removed".to_owned(),
            format!("domain={} key={}", change.target(), change.key()),
        ),
        DomainEvent::OwnerChanged(change) => {
            ("Domain owner changed".to_owned(), format!("{change:?}"))
        }
        other => ("Domain event".to_owned(), format!("{other:?}")),
    }
}
fn account_event_summary(event: &AccountEvent) -> (String, String) {
    match event {
        AccountEvent::Created(account) => (
            "Account created".to_owned(),
            account.account.id().to_string(),
        ),
        AccountEvent::Deleted(id) => ("Account deleted".to_owned(), id.to_string()),
        AccountEvent::ControllerReplaced(change) => (
            "Account controller replaced".to_owned(),
            format!(
                "account={} previous_account={} previous_controller={} new_controller={}",
                change.account,
                change.previous_account,
                change.previous_controller,
                change.new_controller
            ),
        ),
        AccountEvent::PermissionAdded(change) => (
            "Account permission granted".to_owned(),
            format!("{change:?}"),
        ),
        AccountEvent::PermissionRemoved(change) => (
            "Account permission revoked".to_owned(),
            format!("{change:?}"),
        ),
        AccountEvent::RoleGranted(change) => {
            ("Account role granted".to_owned(), format!("{change:?}"))
        }
        AccountEvent::RoleRevoked(change) => {
            ("Account role revoked".to_owned(), format!("{change:?}"))
        }
        AccountEvent::MetadataInserted(change) => (
            "Account metadata inserted".to_owned(),
            format!("account={} key={}", change.target(), change.key()),
        ),
        AccountEvent::MetadataRemoved(change) => (
            "Account metadata removed".to_owned(),
            format!("account={} key={}", change.target(), change.key()),
        ),
        AccountEvent::Recovery(recovery_event) => account_recovery_event_summary(recovery_event),
        AccountEvent::Repo(repo_event) => repo_account_event_summary(repo_event),
    }
}
fn account_recovery_event_summary(event: &AccountRecoveryEvent) -> (String, String) {
    match event {
        AccountRecoveryEvent::PolicySet(payload) => (
            "Account recovery policy set".to_owned(),
            format!(
                "account={} alias={} guardians={} quorum={} timelock_ms={}",
                payload.account,
                account_alias_detail(&payload.alias),
                payload.policy.guardians().len(),
                payload.policy.quorum(),
                payload.policy.timelock_ms().get()
            ),
        ),
        AccountRecoveryEvent::PolicyCleared(payload) => (
            "Account recovery policy cleared".to_owned(),
            format!(
                "account={} alias={}",
                payload.account,
                account_alias_detail(&payload.alias)
            ),
        ),
        AccountRecoveryEvent::Proposed(payload) => (
            "Account recovery proposed".to_owned(),
            format!(
                "account={} alias={} proposed_by={} proposed_controller={} execute_after_ms={} approvals={}",
                payload.account,
                account_alias_detail(&payload.alias),
                payload.request.proposed_by,
                payload.request.proposed_controller,
                payload.request.execute_after_ms,
                payload.request.approvals.len()
            ),
        ),
        AccountRecoveryEvent::Approved(payload) => (
            "Account recovery approved".to_owned(),
            format!(
                "account={} alias={} approver={} approvals={}",
                payload.account,
                account_alias_detail(&payload.alias),
                payload.approver,
                payload.request.approvals.len()
            ),
        ),
        AccountRecoveryEvent::Cancelled(payload) => (
            "Account recovery cancelled".to_owned(),
            format!(
                "account={} alias={} cancelled_by={} status={:?}",
                payload.account,
                account_alias_detail(&payload.alias),
                payload.cancelled_by,
                payload.request.status
            ),
        ),
        AccountRecoveryEvent::Finalized(payload) => (
            "Account recovery finalized".to_owned(),
            format!(
                "account={} previous_account={} alias={} status={:?}",
                payload.account,
                payload.previous_account,
                account_alias_detail(&payload.alias),
                payload.request.status
            ),
        ),
    }
}
fn account_alias_detail(alias: &iroha_data_model::account::AccountAlias) -> String {
    let domain = alias
        .domain
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_owned());
    format!(
        "label={} domain={} dataspace={}",
        alias.label, domain, alias.dataspace
    )
}
fn repo_account_event_summary(event: &RepoAccountEvent) -> (String, String) {
    match event {
        RepoAccountEvent::Initiated(payload) => (
            "Repo agreement initiated".to_owned(),
            format!(
                "account={} counterparty={} agreement={}",
                payload.account(),
                payload.counterparty(),
                payload.agreement().id()
            ),
        ),
        RepoAccountEvent::Settled(payload) => (
            "Repo agreement settled".to_owned(),
            format!(
                "account={} agreement={} cash_leg={:?}",
                payload.account(),
                payload.agreement_id(),
                payload.cash_leg()
            ),
        ),
        RepoAccountEvent::MarginCalled(payload) => (
            "Repo margin call".to_owned(),
            format!(
                "account={} agreement={} timestamp_ms={}",
                payload.account(),
                payload.agreement_id(),
                payload.margin_timestamp_ms()
            ),
        ),
    }
}
fn asset_event_summary(event: &AssetEvent) -> (String, String) {
    match event {
        AssetEvent::Created(asset) => ("Asset created".to_owned(), asset.id().to_string()),
        AssetEvent::Deleted(id) => ("Asset deleted".to_owned(), id.to_string()),
        AssetEvent::Added(change) => (
            "Asset balance increased".to_owned(),
            format!("asset={} amount={}", change.asset(), change.amount()),
        ),
        AssetEvent::Removed(change) => (
            "Asset balance decreased".to_owned(),
            format!("asset={} amount={}", change.asset(), change.amount()),
        ),
        AssetEvent::Transferred(transfer) => (
            "Asset transferred".to_owned(),
            format!(
                "source={} destination={} amount={}",
                transfer.source(),
                transfer.destination(),
                transfer.amount()
            ),
        ),
        AssetEvent::MetadataInserted(change) => (
            "Asset metadata inserted".to_owned(),
            format!("asset={} key={}", change.target(), change.key()),
        ),
        AssetEvent::MetadataRemoved(change) => (
            "Asset metadata removed".to_owned(),
            format!("asset={} key={}", change.target(), change.key()),
        ),
        AssetEvent::BatchTransferOutcome(outcome) => (
            "Asset batch transfer leg".to_owned(),
            format!(
                "leg_index={} leg_id={} asset={} destination={} amount={} status={:?}",
                outcome.leg_index,
                outcome.leg_id,
                outcome.asset,
                outcome.destination,
                outcome.amount,
                outcome.status
            ),
        ),
    }
}
fn asset_definition_event_summary(event: &AssetDefinitionEvent) -> (String, String) {
    match event {
        AssetDefinitionEvent::Created(definition) => (
            "Asset definition created".to_owned(),
            definition.id().to_string(),
        ),
        AssetDefinitionEvent::Deleted(id) => {
            ("Asset definition deleted".to_owned(), id.to_string())
        }
        AssetDefinitionEvent::MetadataInserted(change) => (
            "Asset definition metadata inserted".to_owned(),
            format!("definition={} key={}", change.target(), change.key()),
        ),
        AssetDefinitionEvent::MetadataRemoved(change) => (
            "Asset definition metadata removed".to_owned(),
            format!("definition={} key={}", change.target(), change.key()),
        ),
        AssetDefinitionEvent::MintabilityChanged(id) => (
            "Asset definition mintability changed".to_owned(),
            id.to_string(),
        ),
        AssetDefinitionEvent::MintabilityChangedDetailed(change) => (
            "Asset definition mintability exhausted".to_owned(),
            format!("{change:?}"),
        ),
        AssetDefinitionEvent::TotalQuantityChanged(change) => (
            "Asset definition supply updated".to_owned(),
            format!("{change:?}"),
        ),
        AssetDefinitionEvent::OwnerChanged(change) => (
            "Asset definition owner changed".to_owned(),
            format!("{change:?}"),
        ),
    }
}
fn nft_event_summary(event: &NftEvent) -> (String, String) {
    match event {
        NftEvent::Created(nft) => ("NFT created".to_owned(), nft.id().to_string()),
        NftEvent::Deleted(id) => ("NFT deleted".to_owned(), id.to_string()),
        NftEvent::MetadataInserted(change) => (
            "NFT metadata inserted".to_owned(),
            format!("nft={} key={}", change.target(), change.key()),
        ),
        NftEvent::MetadataRemoved(change) => (
            "NFT metadata removed".to_owned(),
            format!("nft={} key={}", change.target(), change.key()),
        ),
        NftEvent::OwnerChanged(change) => ("NFT owner changed".to_owned(), format!("{change:?}")),
    }
}
fn sorafs_event_summary(event: &sorafs::SorafsGatewayEvent) -> (String, String) {
    match event {
        sorafs::SorafsGatewayEvent::GarViolation(payload) => {
            ("SoraFS GAR violation".to_owned(), format!("{payload:?}"))
        }
        sorafs::SorafsGatewayEvent::ProofHealth(alert) => {
            ("SoraFS proof health alert".to_owned(), format!("{alert:?}"))
        }
        sorafs::SorafsGatewayEvent::RepairLedger(payload) => (
            "SoraFS repair ledger event".to_owned(),
            format!("{payload:?}"),
        ),
        sorafs::SorafsGatewayEvent::ModerationLedger(payload) => (
            "SoraFS moderation ledger event".to_owned(),
            format!("{payload:?}"),
        ),
        sorafs::SorafsGatewayEvent::OrderbookLedger(payload) => (
            "SoraFS orderbook ledger event".to_owned(),
            format!("{payload:?}"),
        ),
        sorafs::SorafsGatewayEvent::ReserveLedger(payload) => (
            "SoraFS reserve ledger event".to_owned(),
            format!("{payload:?}"),
        ),
        sorafs::SorafsGatewayEvent::ReputationJournal(payload) => (
            "SoraFS reputation journal event".to_owned(),
            format!("{payload:?}"),
        ),
    }
}
/// Stage of decoding when a Torii event failure occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDecodeStage {
    /// Failed to parse the Norito frame.
    Frame,
    /// Failed to decode the `EventBox` payload.
    Event,
    /// Underlying WebSocket stream aborted.
    Stream,
}
/// Details about a Torii event stream decoding failure.
#[derive(Debug, Clone)]
pub struct EventStreamDecodeError {
    /// Stage where decoding failed.
    pub stage: EventDecodeStage,
    /// Length of the raw frame that triggered the error, if known.
    pub raw_len: usize,
    /// Human-readable error description.
    pub message: String,
}
impl EventStreamDecodeError {
    fn new(stage: EventDecodeStage, raw_len: usize, message: impl Into<String>) -> Self {
        Self {
            stage,
            raw_len,
            message: message.into(),
        }
    }
}
/// Events emitted by the decoded Torii event stream helper.
#[derive(Debug, Clone)]
pub enum EventStreamEvent {
    /// Successfully decoded event payload.
    Event {
        /// Summary used by UI presenters.
        summary: EventSummary,
        /// Shared event instance for richer viewers.
        event: Arc<EventBox>,
        /// Length of the raw frame before decoding.
        raw_len: usize,
    },
    /// Received UTF-8 payload on the event stream.
    Text { text: String },
    /// Decoding or transport error.
    DecodeError { error: EventStreamDecodeError },
    /// Broadcast receiver lagged behind the producer.
    Lagged { skipped: usize },
    /// Stream closed cleanly.
    Closed,
}
/// High-level helper that consumes WebSocket frames and publishes decoded events.
pub struct EventStream {
    subscription: WsSubscription,
    sender: broadcast::Sender<EventStreamEvent>,
    initial_receiver: std::sync::Mutex<Option<broadcast::Receiver<EventStreamEvent>>>,
    decode_handle: JoinHandle<()>,
}
include!("torii/event_stream_runtime.rs");
include!("torii/managed_streams.rs");
#[cfg(test)]
mod tests {
    include!("torii/tests_part1.rs");
    include!("torii/tests_part2.rs");
}
