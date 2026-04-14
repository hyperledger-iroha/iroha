//! Torii client utilities used by MOCHI.
//!
//! The client focuses on generating canonical endpoints and providing async
//! helpers for common HTTP and WebSocket interactions. UI layers can build on
//! top by wiring retries, auth, and payload codecs.

use std::{
    convert::TryFrom,
    future::Future,
    io::Cursor,
    num::NonZeroU32,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_crypto::HashOf;
use iroha_data_model::{
    Identifiable,
    asset::AssetId,
    block::{self, SignedBlock, consensus::SumeragiStatusWire},
    events::{
        EventBox,
        data::{DataEvent, offline, prelude::*, sorafs},
        pipeline::PipelineEventBox,
        stream::EventMessage,
    },
    isi::Mint,
    nexus::LaneLifecyclePlan,
    prelude::ChainId,
    query::{QueryOutput, SignedQuery},
    transaction::{SignedTransaction, TransactionBuilder},
};
use iroha_telemetry::metrics::Status as TelemetryStatus;
pub use iroha_telemetry::metrics::{GovernanceStatus, Uptime};
use iroha_version::{codec::EncodeVersioned, error::Error as VersionError};
use norito::json;
use reqwest::{
    Client, StatusCode,
    header::{HeaderMap, HeaderName, HeaderValue},
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

use crate::compose::SigningAuthority;

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
    /// Timed out while waiting for Torii to produce a response.
    #[error("timeout while waiting for Torii: {context}")]
    Timeout { context: String },
    /// A smoke transaction was rejected (or expired) before commitment.
    #[error("smoke transaction {hash} rejected: {reason}")]
    SmokeRejected { hash: String, reason: String },
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
    /// Torii responded with an unexpected status code.
    UnexpectedStatus,
    /// WebSocket negotiation or framing failed.
    WebSocket,
    /// Constructed WebSocket request was invalid.
    InvalidWebSocketRequest,
    /// Norito payload decoding failed.
    Decode,
    /// Operation exceeded the configured timeout.
    Timeout,
    /// Smoke transaction was rejected or expired.
    SmokeRejected,
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

fn reject_code_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-iroha-reject-code")
        .or_else(|| headers.get("x-iroha-axt-code"))
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn error_message_from_body(body: &[u8]) -> Option<String> {
    decode_norito_with_alignment::<ToriiErrorEnvelope>(body)
        .ok()
        .map(|envelope| envelope.summary())
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
        }
    }

    /// Build a plan that mints a small amount of `62Fk4FPcMuLvW5QjDGNF2a4jAmjM` with the supplied signer.
    ///
    /// Each attempt carries a unique nonce so retries do not collide.
    pub fn for_signer_with_attempts(
        chain_id: &str,
        signer: &SigningAuthority,
        attempts: usize,
    ) -> Result<Self, ReadinessSmokeBuildError> {
        Self::for_signer_with_attempts_and_offset(chain_id, signer, attempts, 0)
    }

    /// Build a plan with unique nonces derived from the provided offset.
    pub fn for_signer_with_attempts_and_offset(
        chain_id: &str,
        signer: &SigningAuthority,
        attempts: usize,
        nonce_offset: usize,
    ) -> Result<Self, ReadinessSmokeBuildError> {
        let attempts = attempts.max(1);
        let mut transactions = Vec::with_capacity(attempts);
        for attempt in 0..attempts {
            let tx = build_readiness_smoke_transaction(chain_id, signer, attempt + nonce_offset)?;
            transactions.push(tx);
        }
        Ok(Self::new(transactions))
    }

    /// Build a single-attempt plan using the bundled development signer.
    pub fn for_signer(
        chain_id: &str,
        signer: &SigningAuthority,
    ) -> Result<Self, ReadinessSmokeBuildError> {
        Self::for_signer_with_attempts(chain_id, signer, 1)
    }

    /// Iterator over the hashes of the configured smoke transactions.
    pub fn tx_hashes(&self) -> impl Iterator<Item = HashOf<SignedTransaction>> + '_ {
        self.transactions.iter().map(SignedTransaction::hash)
    }
}

/// Errors that can occur while constructing a readiness smoke plan.
#[derive(Debug, thiserror::Error)]
pub enum ReadinessSmokeBuildError {
    /// Failed to parse the default smoke asset identifier.
    #[error("invalid readiness smoke asset `{0}`")]
    InvalidAsset(String),
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

const SMOKE_ASSET_ID: &str = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const SMOKE_TTL: Duration = Duration::from_secs(30);

fn build_readiness_smoke_transaction(
    chain_id: &str,
    signer: &SigningAuthority,
    attempt: usize,
) -> Result<SignedTransaction, ReadinessSmokeBuildError> {
    let chain_id = ChainId::from(chain_id.to_owned());
    let definition = SMOKE_ASSET_ID
        .parse()
        .map_err(|_| ReadinessSmokeBuildError::InvalidAsset(SMOKE_ASSET_ID.to_owned()))?;
    let asset_id = AssetId::new(definition, signer.account_id().clone());
    let quantity = u32::try_from(attempt + 1).unwrap_or(u32::MAX);
    let mut builder = TransactionBuilder::new(chain_id, signer.account_id().clone())
        .with_instructions([Mint::asset_numeric(quantity, asset_id)]);

    if let Some(nonce) = NonZeroU32::new(quantity) {
        builder.set_nonce(nonce);
    }
    builder.set_ttl(SMOKE_TTL);

    Ok(builder.sign(signer.key_pair().private_key()))
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

    /// Consume the builder and construct a [`ToriiClient`].
    pub fn build(self) -> ToriiResult<ToriiClient> {
        let mut client_builder = Client::builder();
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
    /// Pagination metadata returned by Explorer.
    pub pagination: ExplorerPaginationMeta,
    /// Account entries in the requested page.
    pub items: Vec<ExplorerAccountRecord>,
}

impl ExplorerAccountsPage {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let doc = value
            .as_object()
            .ok_or_else(|| decode_error("explorer accounts page", "must be a JSON object"))?;
        let pagination = doc
            .get("pagination")
            .ok_or_else(|| decode_error("explorer accounts page", "missing pagination field"))
            .and_then(ExplorerPaginationMeta::from_json)?;
        let items_value = doc
            .get("items")
            .ok_or_else(|| decode_error("explorer accounts page", "missing items field"))?;
        let items = items_value
            .as_array()
            .ok_or_else(|| decode_error("explorer accounts page.items", "must be a JSON array"))?;
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
    /// Page number (1-indexed). Mirrors Torii defaults when omitted.
    pub page: Option<u64>,
    /// Maximum number of entries per page.
    pub per_page: Option<u64>,
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
    /// Pagination metadata returned by Torii.
    pub pagination: ExplorerPaginationMeta,
    /// Domain entries contained in the page.
    pub items: Vec<ExplorerDomainRecord>,
}

impl ExplorerDomainsPage {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer domains response", "must be a JSON object"))?;
        let pagination = record
            .get("pagination")
            .ok_or_else(|| decode_error("explorer domains response", "missing pagination field"))
            .and_then(ExplorerPaginationMeta::from_json)?;
        let items_value = record
            .get("items")
            .ok_or_else(|| decode_error("explorer domains response", "missing items field"))?;
        let items_array = items_value.as_array().ok_or_else(|| {
            decode_error("explorer domains response.items", "must be a JSON array")
        })?;
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
    /// Page number (1-indexed). Mirrors Torii defaults when omitted.
    pub page: Option<u64>,
    /// Maximum number of entries per page.
    pub per_page: Option<u64>,
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
    /// Pagination metadata returned by Torii.
    pub pagination: ExplorerPaginationMeta,
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
        let pagination = record
            .get("pagination")
            .ok_or_else(|| {
                decode_error(
                    "explorer asset definitions response",
                    "missing pagination field",
                )
            })
            .and_then(ExplorerPaginationMeta::from_json)?;
        let items_value = record.get("items").ok_or_else(|| {
            decode_error("explorer asset definitions response", "missing items field")
        })?;
        let items_array = items_value.as_array().ok_or_else(|| {
            decode_error(
                "explorer asset definitions response.items",
                "must be a JSON array",
            )
        })?;
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
    /// Page number (1-indexed). Mirrors Torii defaults when omitted.
    pub page: Option<u64>,
    /// Maximum number of entries per page.
    pub per_page: Option<u64>,
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
    /// Pagination metadata returned by Torii.
    pub pagination: ExplorerPaginationMeta,
    /// Asset entries in the page.
    pub items: Vec<ExplorerAssetRecord>,
}

impl ExplorerAssetsPage {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer assets response", "must be a JSON object"))?;
        let pagination = record
            .get("pagination")
            .ok_or_else(|| decode_error("explorer assets response", "missing pagination field"))
            .and_then(ExplorerPaginationMeta::from_json)?;
        let items_value = record
            .get("items")
            .ok_or_else(|| decode_error("explorer assets response", "missing items field"))?;
        let items_array = items_value.as_array().ok_or_else(|| {
            decode_error("explorer assets response.items", "must be a JSON array")
        })?;
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
    /// Page number (1-indexed). Mirrors Torii defaults when omitted.
    pub page: Option<u64>,
    /// Maximum number of entries per page.
    pub per_page: Option<u64>,
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
    /// Pagination metadata returned by Torii.
    pub pagination: ExplorerPaginationMeta,
    /// NFT entries included in the page.
    pub items: Vec<ExplorerNftRecord>,
}

impl ExplorerNftsPage {
    fn from_json(value: &json::Value) -> ToriiResult<Self> {
        let record = value
            .as_object()
            .ok_or_else(|| decode_error("explorer nfts response", "must be a JSON object"))?;
        let pagination = record
            .get("pagination")
            .ok_or_else(|| decode_error("explorer nfts response", "missing pagination field"))
            .and_then(ExplorerPaginationMeta::from_json)?;
        let items_value = record
            .get("items")
            .ok_or_else(|| decode_error("explorer nfts response", "missing items field"))?;
        let items_array = items_value
            .as_array()
            .ok_or_else(|| decode_error("explorer nfts response.items", "must be a JSON array"))?;
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
    /// Page number (1-indexed). Mirrors Torii defaults when omitted.
    pub page: Option<u64>,
    /// Maximum number of entries per page.
    pub per_page: Option<u64>,
    /// Optional owning account filter.
    pub owned_by: Option<String>,
    /// Optional domain filter restricting NFT IDs.
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
    /// Returns `None` when either the depth or capacity gauges were missing or
    /// the backend reported a zero/negative capacity (should never happen on
    /// healthy Torii deployments).
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
    /// Returns `None` when the exporter did not emit the flag or reported an
    /// unexpected value (non-zero/non-one) so UI surfaces can retain a
    /// tri-state indicator.
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
/// The helper is exported so downstream crates (or language bindings) can share
/// the same alignment guard instead of cloning the `unsafe` retry logic.
pub fn decode_norito_with_alignment<T>(bytes: &[u8]) -> Result<T, ToriiError>
where
    T: for<'de> norito::core::NoritoDeserialize<'de>,
{
    const MAX_PAD: usize = 64;

    let attempt = |slice: &[u8]| {
        catch_unwind(AssertUnwindSafe(|| {
            norito::decode_from_reader(Cursor::new(slice))
        }))
    };

    match attempt(bytes) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => Err(ToriiError::Decode(err.to_string())),
        Err(_) => {
            for pad in 1..=MAX_PAD {
                let mut buffer = Vec::with_capacity(bytes.len() + pad);
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
    http: Client,
    status_state: Arc<Mutex<StatusState>>,
    default_headers: HeaderMap,
}

impl ToriiClient {
    /// Construct a client pointing at the supplied Torii HTTP base URL.
    pub fn new(base_url: impl AsRef<str>) -> ToriiResult<Self> {
        Self::builder(base_url)?.build()
    }

    /// Start constructing a [`ToriiClient`] with custom options.
    pub fn builder(base_url: impl AsRef<str>) -> ToriiResult<ToriiClientBuilder> {
        ToriiClientBuilder::new(base_url)
    }

    /// HTTP base URL used for REST calls (e.g., `http://127.0.0.1:8080`).
    pub fn base_url(&self) -> &str {
        self.http_base.as_str()
    }

    /// URL of the `/transaction` endpoint.
    pub fn transaction_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("transaction")
    }

    /// URL of the `/query` endpoint.
    pub fn query_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("query")
    }

    /// URL of the `/block/stream` WebSocket endpoint.
    pub fn block_stream_endpoint(&self) -> ToriiResult<Url> {
        self.ws_endpoint("block/stream")
    }

    /// URL of the `/events` WebSocket endpoint.
    pub fn events_stream_endpoint(&self) -> ToriiResult<Url> {
        self.ws_endpoint("events")
    }

    /// URL of the `/status` endpoint.
    pub fn status_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("status")
    }

    /// URL of the `/v1/sumeragi/status` endpoint.
    pub fn sumeragi_status_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/sumeragi/status")
    }

    /// URL of the `/metrics` endpoint.
    pub fn metrics_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("metrics")
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

    /// Run a readiness smoke probe that waits for `/status`, submits a smoke transaction,
    /// and observes its commitment with retries/backoff.
    pub async fn wait_for_readiness_smoke(
        &self,
        plan: ReadinessSmokePlan,
    ) -> ToriiResult<ReadinessSmokeOutcome> {
        if plan.transactions.is_empty() {
            return Err(ToriiError::Decode(
                "readiness smoke plan must include at least one transaction".to_owned(),
            ));
        }

        self.wait_for_ready(plan.status_options).await?;

        let attempts = plan.transactions.len();
        let started = Instant::now();
        let mut backoff = plan.backoff.max(Duration::from_millis(50)).min(MAX_BACKOFF);

        for (index, transaction) in plan.transactions.iter().enumerate() {
            let attempt = index + 1;
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
                Err(_err) if attempt < attempts => {
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

    /// URL of the `/configuration` endpoint.
    pub fn configuration_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("configuration")
    }

    /// URL of the `/v1/nexus/lifecycle` endpoint.
    pub fn nexus_lifecycle_endpoint(&self) -> ToriiResult<Url> {
        self.http_endpoint("v1/nexus/lifecycle")
    }

    /// Submit a Norito-encoded transaction to Torii.
    pub async fn submit_transaction(&self, payload: &[u8]) -> ToriiResult<()> {
        let url = self.transaction_endpoint()?;
        let response = self
            .http
            .post(url)
            .header("Content-Type", "application/octet-stream")
            .body(payload.to_vec())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let reject_code = reject_code_from_headers(response.headers());
            let body = response.bytes().await?;
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
            .header("Content-Type", "application/octet-stream")
            .body(payload.to_vec())
            .send()
            .await?;

        if response.status().is_success() {
            return Ok(response.bytes().await?.to_vec());
        }
        let status = response.status();
        let reject_code = reject_code_from_headers(response.headers());
        let body = response.bytes().await?;
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

    /// Submit a signed transaction and wait until it appears in the committed block stream.
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

        let block_stream = self.block_stream().await?;
        let events_stream = self.events_stream().await?;
        let block_rx = block_stream.subscribe();
        let event_rx = events_stream.subscribe();

        let result = submit_and_wait_for_commit_with_receivers(
            tx_hash,
            options,
            self.submit_signed_transaction(transaction),
            block_rx,
            event_rx,
        )
        .await;

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
            .header(reqwest::header::ACCEPT, "application/norito")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }

        let body = response.bytes().await?;
        decode_norito_with_alignment(body.as_ref())
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

    /// Fetch the full Sumeragi status snapshot.
    pub async fn fetch_sumeragi_status(&self) -> ToriiResult<SumeragiStatusWire> {
        let url = self.sumeragi_status_endpoint()?;
        let response = self
            .http
            .get(url)
            .header(reqwest::header::ACCEPT, "application/norito")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }

        let body = response.bytes().await?;
        decode_norito_with_alignment(body.as_ref())
    }

    /// Fetch the Torii node configuration as a Norito JSON value.
    pub async fn fetch_configuration(&self) -> ToriiResult<json::Value> {
        let url = self.configuration_endpoint()?;
        self.fetch_json(url).await
    }

    /// Apply a Nexus lane lifecycle plan via Torii.
    pub async fn apply_lane_lifecycle(&self, plan: &LaneLifecyclePlan) -> ToriiResult<json::Value> {
        let url = self.nexus_lifecycle_endpoint()?;
        let payload = json::to_vec(plan).map_err(|err| ToriiError::Decode(err.to_string()))?;
        let response = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await?;

        if response.status().is_success() {
            let bytes = response.bytes().await?;
            if bytes.is_empty() {
                return Ok(json::Value::Object(json::Map::new()));
            }
            return json::from_slice_value(bytes.as_ref())
                .map_err(|err| ToriiError::Decode(err.to_string()));
        }
        let status = response.status();
        let reject_code = reject_code_from_headers(response.headers());
        let body = response.bytes().await?;
        let message = error_message_from_body(body.as_ref());
        Err(ToriiError::UnexpectedStatus {
            status,
            reject_code,
            message,
        })
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
        Ok(response.text().await?)
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
                let bytes = response.bytes().await?;
                let value =
                    json::from_slice(&bytes).map_err(|err| ToriiError::Decode(err.to_string()))?;
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
        let bytes = response.bytes().await?;
        let value = json::from_slice(&bytes).map_err(|err| ToriiError::Decode(err.to_string()))?;
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
        if let Some(page) = query.page {
            params.push(("page", page.to_string()));
        }
        if let Some(per_page) = query.per_page {
            params.push(("per_page", per_page.to_string()));
        }
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
        let bytes = response.bytes().await?;
        let value = json::from_slice(&bytes).map_err(|err| ToriiError::Decode(err.to_string()))?;
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
        if let Some(page) = query.page {
            params.push(("page", page.to_string()));
        }
        if let Some(per_page) = query.per_page {
            params.push(("per_page", per_page.to_string()));
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
        let bytes = response.bytes().await?;
        let value = json::from_slice(&bytes).map_err(|err| ToriiError::Decode(err.to_string()))?;
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
        if let Some(page) = query.page {
            params.push(("page", page.to_string()));
        }
        if let Some(per_page) = query.per_page {
            params.push(("per_page", per_page.to_string()));
        }
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
        let bytes = response.bytes().await?;
        let value = json::from_slice(&bytes).map_err(|err| ToriiError::Decode(err.to_string()))?;
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
        if let Some(page) = query.page {
            params.push(("page", page.to_string()));
        }
        if let Some(per_page) = query.per_page {
            params.push(("per_page", per_page.to_string()));
        }
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
        let bytes = response.bytes().await?;
        let value = json::from_slice(&bytes).map_err(|err| ToriiError::Decode(err.to_string()))?;
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
        if let Some(page) = query.page {
            params.push(("page", page.to_string()));
        }
        if let Some(per_page) = query.per_page {
            params.push(("per_page", per_page.to_string()));
        }
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
        let bytes = response.bytes().await?;
        let value = json::from_slice(&bytes).map_err(|err| ToriiError::Decode(err.to_string()))?;
        ExplorerNftsPage::from_json(&value)
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
        let bytes = response.bytes().await?;
        let value = json::from_slice(&bytes).map_err(|err| ToriiError::Decode(err.to_string()))?;
        TriggerListPage::from_json(&value)
    }

    /// Fetch a single trigger definition.
    pub async fn get_trigger(&self, trigger_id: &str) -> ToriiResult<Option<TriggerRecord>> {
        let url = self.trigger_record_endpoint(trigger_id)?;
        let response = self.http.get(url).send().await?;
        match response.status() {
            StatusCode::OK => {
                let bytes = response.bytes().await?;
                let value =
                    json::from_slice(&bytes).map_err(|err| ToriiError::Decode(err.to_string()))?;
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
        let bytes = response.bytes().await?;
        let value = json::from_slice(&bytes).map_err(|err| ToriiError::Decode(err.to_string()))?;
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

    /// Establish a WebSocket connection to `/block/stream`.
    pub async fn connect_block_stream(&self) -> ToriiResult<ToriiWebSocket> {
        self.connect_ws(self.block_stream_endpoint()?).await
    }

    /// Establish a WebSocket connection to `/events`.
    pub async fn connect_events_stream(&self) -> ToriiResult<ToriiWebSocket> {
        self.connect_ws(self.events_stream_endpoint()?).await
    }

    /// Subscribe to the `/block/stream` endpoint and forward frames to a broadcast channel.
    pub async fn subscribe_block_stream(&self) -> ToriiResult<WsSubscription> {
        self.subscribe_ws(self.block_stream_endpoint()?).await
    }

    /// Subscribe to the `/events` endpoint and forward frames to a broadcast channel.
    pub async fn subscribe_events_stream(&self) -> ToriiResult<WsSubscription> {
        self.subscribe_ws(self.events_stream_endpoint()?).await
    }

    /// Subscribe to `/block/stream` and publish decoded [`SignedBlock`] events.
    pub async fn block_stream(&self) -> ToriiResult<BlockStream> {
        let subscription = self.subscribe_block_stream().await?;
        Ok(BlockStream::new(subscription))
    }

    /// Subscribe to `/events` and publish decoded [`EventBox`] events.
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
            return Err(ToriiError::UnexpectedStatus {
                status: response.status(),
                reject_code: None,
                message: None,
            });
        }
        let bytes = response.bytes().await?;
        json::from_slice(&bytes).map_err(|err| ToriiError::Decode(err.to_string()))
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
        }
        let (stream, _response) = connect_async(request).await?;
        Ok(stream)
    }

    async fn subscribe_ws(&self, endpoint: Url) -> ToriiResult<WsSubscription> {
        let mut stream = self.connect_ws(endpoint).await?;
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
                        let _ = forwarder.send(WsFrame::Text(text.to_string()));
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

async fn submit_and_wait_for_commit_with_receivers<Fut>(
    tx_hash: HashOf<SignedTransaction>,
    options: SmokeCommitOptions,
    submit: Fut,
    mut block_rx: broadcast::Receiver<BlockStreamEvent>,
    mut event_rx: broadcast::Receiver<EventStreamEvent>,
) -> ToriiResult<u64>
where
    Fut: Future<Output = ToriiResult<()>>,
{
    submit.await?;
    let tx_hash_str = tx_hash.to_string();

    let wait = async {
        loop {
            tokio::select! {
                message = block_rx.recv() => {
                    match message {
                        Ok(BlockStreamEvent::Block { block, .. }) => {
                            if block.transactions_vec().iter().any(|tx| tx.hash() == tx_hash) {
                                return Ok(block.header().height().get());
                            }
                        }
                        Ok(BlockStreamEvent::DecodeError { error }) => {
                            return Err(ToriiError::Decode(error.message));
                        }
                        Ok(BlockStreamEvent::Closed) => {
                            return Err(ToriiError::Timeout { context: "block stream closed".to_owned() });
                        }
                        Ok(BlockStreamEvent::Lagged { .. } | BlockStreamEvent::Text { .. }) => {}
                        Err(RecvError::Lagged(_)) => {}
                        Err(RecvError::Closed) => {
                            return Err(ToriiError::Timeout { context: "block stream closed".to_owned() });
                        }
                    }
                }
                message = event_rx.recv() => {
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
                            return Err(ToriiError::Decode(error.message));
                        }
                        Ok(EventStreamEvent::Closed) => {}
                        Ok(EventStreamEvent::Lagged { .. } | EventStreamEvent::Text { .. }) => {}
                        Err(RecvError::Lagged(_)) => {}
                        Err(RecvError::Closed) => {}
                    }
                }
            }
        }
    };

    tokio::time::timeout(options.timeout, wait)
        .await
        .map_err(|_| ToriiError::Timeout {
            context: format!("smoke commit {tx_hash_str}"),
        })?
}

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
        let transaction_count = block.transactions_vec().len();
        let (time_trigger_count, rejected_transaction_count) = if block.has_results() {
            let time_triggers = block.time_triggers();
            let rejected = block
                .transactions_vec()
                .iter()
                .enumerate()
                .filter(|(idx, _)| block.error(*idx).is_some())
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
#[derive(Debug)]
pub struct BlockStream {
    subscription: WsSubscription,
    sender: broadcast::Sender<BlockStreamEvent>,
    decode_handle: JoinHandle<()>,
}

impl BlockStream {
    fn new(subscription: WsSubscription) -> Self {
        let mut receiver = subscription.subscribe();
        let (sender, _receiver) = broadcast::channel(128);
        let forwarder = sender.clone();

        let decode_handle = tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(WsFrame::Binary(frame)) => {
                        let raw_len = frame.len();
                        match block::decode_versioned_signed_block(&frame) {
                            Ok(block) => {
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
                                let stage = match &err {
                                    VersionError::NoritoCodec(_) | VersionError::NotVersioned => {
                                        BlockDecodeStage::Frame
                                    }
                                    _ => BlockDecodeStage::Block,
                                };
                                let _ = forwarder.send(BlockStreamEvent::DecodeError {
                                    error: BlockStreamDecodeError::new(
                                        stage,
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
            decode_handle,
        }
    }

    /// Acquire a receiver for decoded block events.
    pub fn subscribe(&self) -> broadcast::Receiver<BlockStreamEvent> {
        self.sender.subscribe()
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
        DataEvent::Trigger(trigger) => ("Trigger".to_owned(), format!("{trigger:?}")),
        DataEvent::Role(role) => ("Role".to_owned(), format!("{role:?}")),
        DataEvent::Configuration(config) => ("Configuration".to_owned(), format!("{config:?}")),
        DataEvent::Executor(executor) => ("Executor".to_owned(), format!("{executor:?}")),
        DataEvent::Proof(proof) => ("Proof".to_owned(), format!("{proof:?}")),
        DataEvent::Confidential(confidential) => {
            ("Confidential".to_owned(), format!("{confidential:?}"))
        }
        DataEvent::VerifyingKey(key) => ("VerifyingKey".to_owned(), format!("{key:?}")),
        DataEvent::RuntimeUpgrade(upgrade) => ("RuntimeUpgrade".to_owned(), format!("{upgrade:?}")),
        DataEvent::SmartContract(contract) => ("SmartContract".to_owned(), format!("{contract:?}")),
        DataEvent::Soradns(event) => ("Soradns".to_owned(), format!("{event:?}")),
        DataEvent::Sorafs(event) => sorafs_event_summary(event),
        DataEvent::Offline(event) => offline_event_summary(event),
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
        DomainEvent::Account(account) => account_event_summary(account),
        DomainEvent::AssetDefinition(definition) => asset_definition_event_summary(definition),
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
        AccountEvent::Asset(asset_event) => asset_event_summary(asset_event),
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
        AssetEvent::MetadataInserted(change) => (
            "Asset metadata inserted".to_owned(),
            format!("asset={} key={}", change.target(), change.key()),
        ),
        AssetEvent::MetadataRemoved(change) => (
            "Asset metadata removed".to_owned(),
            format!("asset={} key={}", change.target(), change.key()),
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

fn offline_event_summary(event: &offline::OfflineTransferEvent) -> (String, String) {
    match event {
        offline::OfflineTransferEvent::RevocationImported(payload) => (
            "Offline revocation imported".to_owned(),
            format!("{payload:?}"),
        ),
        offline::OfflineTransferEvent::Settled(payload) => {
            ("Offline bundle settled".to_owned(), format!("{payload:?}"))
        }
        offline::OfflineTransferEvent::AllowanceReclaimed(payload) => (
            "Offline allowance reclaimed".to_owned(),
            format!("{payload:?}"),
        ),
        offline::OfflineTransferEvent::Archived(payload) => {
            ("Offline bundle archived".to_owned(), format!("{payload:?}"))
        }
        offline::OfflineTransferEvent::Pruned(payload) => {
            ("Offline bundle pruned".to_owned(), format!("{payload:?}"))
        }
    }
}

fn sorafs_event_summary(event: &sorafs::SorafsGatewayEvent) -> (String, String) {
    match event {
        sorafs::SorafsGatewayEvent::GarViolation(payload) => {
            ("SoraFS GAR violation".to_owned(), format!("{payload:?}"))
        }
        sorafs::SorafsGatewayEvent::DealUsage(payload) => {
            ("SoraFS deal usage".to_owned(), format!("{payload:?}"))
        }
        sorafs::SorafsGatewayEvent::DealSettlement(payload) => {
            ("SoraFS deal settlement".to_owned(), format!("{payload:?}"))
        }
        sorafs::SorafsGatewayEvent::ProofHealth(alert) => {
            ("SoraFS proof health alert".to_owned(), format!("{alert:?}"))
        }
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
#[derive(Debug)]
pub struct EventStream {
    subscription: WsSubscription,
    sender: broadcast::Sender<EventStreamEvent>,
    decode_handle: JoinHandle<()>,
}

impl EventStream {
    fn new(subscription: WsSubscription) -> Self {
        let mut receiver = subscription.subscribe();
        let (sender, _receiver) = broadcast::channel(128);
        let forwarder = sender.clone();

        let decode_handle = tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(WsFrame::Binary(frame)) => {
                        let raw_len = frame.len();
                        match decode_norito_with_alignment::<EventMessage>(&frame) {
                            Ok(message) => {
                                let event_box: EventBox = message.into();
                                let summary = EventSummary::from_event(&event_box);
                                let event = Arc::new(event_box);
                                let _ = forwarder.send(EventStreamEvent::Event {
                                    summary,
                                    event,
                                    raw_len,
                                });
                            }
                            Err(err) => {
                                let _ = forwarder.send(EventStreamEvent::DecodeError {
                                    error: EventStreamDecodeError::new(
                                        EventDecodeStage::Frame,
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
                        let _ = forwarder.send(EventStreamEvent::Text { text: truncated });
                    }
                    Ok(WsFrame::Error(message)) => {
                        let _ = forwarder.send(EventStreamEvent::DecodeError {
                            error: EventStreamDecodeError::new(
                                EventDecodeStage::Stream,
                                0,
                                message,
                            ),
                        });
                        break;
                    }
                    Ok(WsFrame::Closed) => {
                        let _ = forwarder.send(EventStreamEvent::Closed);
                        break;
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        let _ = forwarder.send(EventStreamEvent::Lagged {
                            skipped: lag_to_usize(skipped),
                        });
                    }
                    Err(RecvError::Closed) => {
                        let _ = forwarder.send(EventStreamEvent::Closed);
                        break;
                    }
                }
            }
        });

        Self {
            subscription,
            sender,
            decode_handle,
        }
    }

    /// Acquire a receiver for decoded events.
    pub fn subscribe(&self) -> broadcast::Receiver<EventStreamEvent> {
        self.sender.subscribe()
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

impl Drop for EventStream {
    fn drop(&mut self) {
        self.abort();
    }
}

const INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const MAX_BACKOFF: Duration = Duration::from_secs(8);

/// Reconnecting wrapper around [`BlockStream`] that automatically retries with backoff.
#[derive(Debug)]
pub struct ManagedBlockStream {
    sender: broadcast::Sender<BlockStreamEvent>,
    shutdown: watch::Sender<bool>,
    worker: JoinHandle<()>,
    alias: Arc<str>,
}

impl ManagedBlockStream {
    /// Spawn a reconnection loop for the `/block/stream` endpoint using the provided runtime handle.
    ///
    /// The `alias` is used for diagnostics so UI layers can attribute log messages
    /// and reconnection notices to the originating peer.
    pub fn spawn(handle: &Handle, alias: String, client: ToriiClient) -> Self {
        Self::spawn_with_factory(handle, alias, move || {
            let client = client.clone();
            async move { client.subscribe_block_stream().await }
        })
    }

    fn spawn_with_factory<F, Fut>(handle: &Handle, alias: impl Into<String>, factory: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToriiResult<WsSubscription>> + Send + 'static,
    {
        let (shutdown, mut shutdown_rx) = watch::channel(false);
        let (sender, _) = broadcast::channel(128);
        let alias: Arc<str> = Arc::from(alias.into().into_boxed_str());
        let factory = Arc::new(factory);
        let run_factory = factory.clone();
        let run_sender = sender.clone();
        let run_alias = alias.clone();
        let worker = handle.spawn(async move {
            run_managed_block_stream(run_alias, run_factory, run_sender, &mut shutdown_rx).await;
        });

        Self {
            sender,
            shutdown,
            worker,
            alias,
        }
    }

    /// Acquire a receiver that yields decoded block events with reconnection semantics.
    pub fn subscribe(&self) -> broadcast::Receiver<BlockStreamEvent> {
        self.sender.subscribe()
    }

    /// Abort the reconnection loop and underlying subscription, if running.
    pub fn abort(&self) {
        let _ = self.shutdown.send(true);
        if !self.worker.is_finished() {
            self.worker.abort();
        }
    }

    /// Returns `true` when the reconnection loop has finished executing.
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }

    /// Returns the alias associated with this managed stream.
    pub fn alias(&self) -> &str {
        self.alias.as_ref()
    }
}

impl Drop for ManagedBlockStream {
    fn drop(&mut self) {
        self.abort();
    }
}

async fn run_managed_block_stream<F, Fut>(
    alias: Arc<str>,
    factory: Arc<F>,
    sender: broadcast::Sender<BlockStreamEvent>,
    shutdown: &mut watch::Receiver<bool>,
) where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ToriiResult<WsSubscription>> + Send + 'static,
{
    let mut backoff = INITIAL_BACKOFF;
    let mut has_connected = false;
    loop {
        if shutdown_requested(shutdown) {
            break;
        }

        let subscription = match (factory.as_ref())().await {
            Ok(subscription) => subscription,
            Err(err) => {
                let _ = sender.send(BlockStreamEvent::DecodeError {
                    error: BlockStreamDecodeError::new(
                        BlockDecodeStage::Stream,
                        0,
                        err.to_string(),
                    ),
                });
                let _ = sender.send(BlockStreamEvent::Text {
                    text: format!(
                        "Block stream `{}` reconnecting after error: {err}",
                        alias.as_ref()
                    ),
                });

                if wait_for_shutdown_or_delay(shutdown, backoff).await {
                    break;
                }
                backoff = (backoff.saturating_mul(2)).min(MAX_BACKOFF);
                continue;
            }
        };

        if has_connected {
            let _ = sender.send(BlockStreamEvent::Text {
                text: format!("Block stream `{}` reconnected.", alias.as_ref()),
            });
        } else {
            has_connected = true;
        }
        backoff = INITIAL_BACKOFF;
        let block_stream = BlockStream::new(subscription);
        let mut receiver = block_stream.subscribe();
        let mut should_stop = false;

        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(_) => {
                            if shutdown_requested(shutdown) {
                                should_stop = true;
                                break;
                            }
                        }
                        Err(_) => {
                            should_stop = true;
                            break;
                        }
                    }
                }
                item = receiver.recv() => {
                    match item {
                        Ok(event) => {
                            let _ = sender.send(event.clone());
                            if matches!(event, BlockStreamEvent::Closed) {
                                break;
                            }
                        }
                        Err(RecvError::Lagged(skipped)) => {
                            let _ = sender.send(BlockStreamEvent::Lagged {
                                skipped: lag_to_usize(skipped),
                            });
                        }
                        Err(RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }

        block_stream.abort();
        if should_stop || shutdown_requested(shutdown) {
            break;
        }

        if wait_for_shutdown_or_delay(shutdown, backoff).await {
            break;
        }
    }
}

/// Reconnecting wrapper around [`EventStream`] with exponential backoff.
#[derive(Debug)]
pub struct ManagedEventStream {
    sender: broadcast::Sender<EventStreamEvent>,
    shutdown: watch::Sender<bool>,
    worker: JoinHandle<()>,
    alias: Arc<str>,
}

impl ManagedEventStream {
    /// Spawn a reconnection loop for the `/events` endpoint using the provided runtime handle.
    pub fn spawn(handle: &Handle, alias: String, client: ToriiClient) -> Self {
        Self::spawn_with_factory(handle, alias, move || {
            let client = client.clone();
            async move { client.subscribe_events_stream().await }
        })
    }

    fn spawn_with_factory<F, Fut>(handle: &Handle, alias: impl Into<String>, factory: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToriiResult<WsSubscription>> + Send + 'static,
    {
        let (shutdown, mut shutdown_rx) = watch::channel(false);
        let (sender, _) = broadcast::channel(128);
        let alias: Arc<str> = Arc::from(alias.into().into_boxed_str());
        let factory = Arc::new(factory);
        let run_factory = factory.clone();
        let run_sender = sender.clone();
        let run_alias = alias.clone();
        let worker = handle.spawn(async move {
            run_managed_event_stream(run_alias, run_factory, run_sender, &mut shutdown_rx).await;
        });

        Self {
            sender,
            shutdown,
            worker,
            alias,
        }
    }

    /// Acquire a receiver that yields decoded events with reconnection semantics.
    pub fn subscribe(&self) -> broadcast::Receiver<EventStreamEvent> {
        self.sender.subscribe()
    }

    /// Abort the reconnection loop and underlying subscription, if running.
    pub fn abort(&self) {
        let _ = self.shutdown.send(true);
        if !self.worker.is_finished() {
            self.worker.abort();
        }
    }

    /// Returns `true` when the reconnection loop has finished executing.
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }

    /// Returns the alias associated with this managed stream.
    pub fn alias(&self) -> &str {
        self.alias.as_ref()
    }
}

impl Drop for ManagedEventStream {
    fn drop(&mut self) {
        self.abort();
    }
}

async fn run_managed_event_stream<F, Fut>(
    alias: Arc<str>,
    factory: Arc<F>,
    sender: broadcast::Sender<EventStreamEvent>,
    shutdown: &mut watch::Receiver<bool>,
) where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ToriiResult<WsSubscription>> + Send + 'static,
{
    let mut backoff = INITIAL_BACKOFF;
    let mut has_connected = false;
    loop {
        if shutdown_requested(shutdown) {
            break;
        }

        let subscription = match (factory.as_ref())().await {
            Ok(subscription) => subscription,
            Err(err) => {
                let _ = sender.send(EventStreamEvent::DecodeError {
                    error: EventStreamDecodeError::new(
                        EventDecodeStage::Stream,
                        0,
                        err.to_string(),
                    ),
                });
                let _ = sender.send(EventStreamEvent::Text {
                    text: format!(
                        "Event stream `{}` reconnecting after error: {err}",
                        alias.as_ref()
                    ),
                });

                if wait_for_shutdown_or_delay(shutdown, backoff).await {
                    break;
                }
                backoff = (backoff.saturating_mul(2)).min(MAX_BACKOFF);
                continue;
            }
        };

        if has_connected {
            let _ = sender.send(EventStreamEvent::Text {
                text: format!("Event stream `{}` reconnected.", alias.as_ref()),
            });
        } else {
            has_connected = true;
        }
        backoff = INITIAL_BACKOFF;
        let event_stream = EventStream::new(subscription);
        let mut receiver = event_stream.subscribe();
        let mut should_stop = false;

        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(_) => {
                            if shutdown_requested(shutdown) {
                                should_stop = true;
                                break;
                            }
                        }
                        Err(_) => {
                            should_stop = true;
                            break;
                        }
                    }
                }
                item = receiver.recv() => {
                    match item {
                        Ok(event) => {
                            let _ = sender.send(event.clone());
                            if matches!(event, EventStreamEvent::Closed) {
                                break;
                            }
                        }
                        Err(RecvError::Lagged(skipped)) => {
                            let _ = sender.send(EventStreamEvent::Lagged {
                                skipped: lag_to_usize(skipped),
                            });
                        }
                        Err(RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }

        event_stream.abort();
        if should_stop || shutdown_requested(shutdown) {
            break;
        }

        if wait_for_shutdown_or_delay(shutdown, backoff).await {
            break;
        }
    }
}

/// Events emitted by the status polling helper.
#[derive(Debug, Clone)]
pub enum StatusStreamEvent {
    /// Fresh status snapshot returned by Torii.
    Snapshot {
        /// Shared telemetry snapshot.
        snapshot: Arc<ToriiStatusSnapshot>,
        /// Optional Sumeragi status payload.
        sumeragi: Option<Arc<SumeragiStatusWire>>,
        /// Optional metrics payload parsed from `/metrics`. When metrics polling is throttled,
        /// this value reuses the last successfully fetched snapshot until the refresh interval
        /// elapses.
        metrics: Option<Arc<ToriiMetricsSnapshot>>,
        /// Error information describing why metrics could not be fetched.
        metrics_error: Option<ToriiErrorInfo>,
    },
    /// Failed to fetch a snapshot; includes summary and failure count.
    Error {
        /// Classified error information suitable for UI display.
        error: ToriiErrorInfo,
        /// Number of consecutive failures observed so far.
        consecutive_failures: u32,
    },
    /// Status polling loop exited.
    Closed,
}

/// Configuration values used when spawning [`ManagedStatusStream`].
#[derive(Debug, Clone)]
pub struct StatusStreamOptions {
    /// Delay between successive `/status` polls.
    pub poll_interval: Duration,
    /// Optional refresh cadence for `/metrics`. When unset, metrics are fetched on every poll.
    /// Supply `Some(Duration::ZERO)` to disable metrics entirely, or a positive duration to
    /// throttle sampling to at most once per interval.
    pub metrics_poll_interval: Option<Duration>,
}

impl StatusStreamOptions {
    /// Create options with the supplied poll interval and default metrics behaviour.
    #[must_use]
    pub const fn new(poll_interval: Duration) -> Self {
        Self {
            poll_interval,
            metrics_poll_interval: None,
        }
    }

    /// Override the metrics refresh cadence.
    #[must_use]
    pub const fn with_metrics_poll_interval(mut self, interval: Option<Duration>) -> Self {
        self.metrics_poll_interval = interval;
        self
    }
}

/// Periodic status poller with exponential backoff on failures.
#[derive(Debug)]
pub struct ManagedStatusStream {
    sender: broadcast::Sender<StatusStreamEvent>,
    shutdown: watch::Sender<bool>,
    worker: JoinHandle<()>,
    alias: Arc<str>,
}

impl ManagedStatusStream {
    /// Spawn a polling loop that fetches `/status` on the requested interval.
    ///
    /// `poll_interval` controls the delay between successful samples. Failures
    /// automatically retry using the standard exponential backoff window shared
    /// with the streamed endpoints.
    pub fn spawn(
        handle: &Handle,
        alias: impl Into<String>,
        client: ToriiClient,
        poll_interval: Duration,
    ) -> Self {
        Self::spawn_with_options(
            handle,
            alias,
            client,
            StatusStreamOptions::new(poll_interval),
        )
    }

    /// Spawn a polling loop using the supplied options.
    pub fn spawn_with_options(
        handle: &Handle,
        alias: impl Into<String>,
        client: ToriiClient,
        options: StatusStreamOptions,
    ) -> Self {
        let (shutdown, mut shutdown_rx) = watch::channel(false);
        let (sender, _) = broadcast::channel(128);
        let alias: Arc<str> = Arc::from(alias.into().into_boxed_str());
        let worker_alias = alias.clone();
        let worker_sender = sender.clone();
        let worker_client = client.clone();
        let worker_options = options.clone();
        let worker = handle.spawn(async move {
            run_managed_status_stream(
                worker_alias,
                worker_client,
                worker_options,
                worker_sender,
                &mut shutdown_rx,
            )
            .await;
        });

        Self {
            sender,
            shutdown,
            worker,
            alias,
        }
    }

    /// Acquire a receiver yielding poll results and failure notices.
    pub fn subscribe(&self) -> broadcast::Receiver<StatusStreamEvent> {
        self.sender.subscribe()
    }

    /// Abort the polling loop immediately.
    pub fn abort(&self) {
        let _ = self.shutdown.send(true);
        if !self.worker.is_finished() {
            self.worker.abort();
        }
    }

    /// Returns `true` once the polling loop has terminated.
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }

    /// Returns the alias associated with this status stream.
    pub fn alias(&self) -> &str {
        self.alias.as_ref()
    }
}

impl Drop for ManagedStatusStream {
    fn drop(&mut self) {
        self.abort();
    }
}

#[derive(Default)]
struct MetricsCache {
    last_snapshot: Option<Arc<ToriiMetricsSnapshot>>,
    last_error: Option<ToriiErrorInfo>,
    last_poll: Option<Instant>,
}

async fn run_managed_status_stream(
    _alias: Arc<str>,
    client: ToriiClient,
    options: StatusStreamOptions,
    sender: broadcast::Sender<StatusStreamEvent>,
    shutdown: &mut watch::Receiver<bool>,
) {
    let mut backoff = INITIAL_BACKOFF;
    let mut consecutive_failures = 0u32;
    let mut metrics_cache = MetricsCache::default();
    let poll_interval = options.poll_interval;
    let metrics_interval = options.metrics_poll_interval;

    loop {
        if shutdown_requested(shutdown) {
            break;
        }

        match client.fetch_status_snapshot().await {
            Ok(snapshot) => {
                let snapshot_arc = Arc::new(snapshot);
                let sumeragi = match client.fetch_sumeragi_status().await {
                    Ok(status) => Some(Arc::new(status)),
                    Err(err) => {
                        let _ = sender.send(StatusStreamEvent::Error {
                            error: err.summarize(),
                            consecutive_failures,
                        });
                        None
                    }
                };
                let (metrics, metrics_error) =
                    fetch_metrics_snapshot_if_needed(&client, metrics_interval, &mut metrics_cache)
                        .await;
                consecutive_failures = 0;
                backoff = INITIAL_BACKOFF;
                let _ = sender.send(StatusStreamEvent::Snapshot {
                    snapshot: snapshot_arc,
                    sumeragi,
                    metrics,
                    metrics_error,
                });
            }
            Err(err) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                let _ = sender.send(StatusStreamEvent::Error {
                    error: err.summarize(),
                    consecutive_failures,
                });

                if wait_for_shutdown_or_delay(shutdown, backoff).await {
                    let _ = sender.send(StatusStreamEvent::Closed);
                    return;
                }
                backoff = (backoff.saturating_mul(2)).min(MAX_BACKOFF);
                continue;
            }
        }

        if wait_for_shutdown_or_delay(shutdown, poll_interval).await {
            break;
        }
    }

    let _ = sender.send(StatusStreamEvent::Closed);
}

async fn fetch_metrics_snapshot_if_needed(
    client: &ToriiClient,
    interval: Option<Duration>,
    cache: &mut MetricsCache,
) -> (Option<Arc<ToriiMetricsSnapshot>>, Option<ToriiErrorInfo>) {
    match interval {
        Some(delay) if delay.is_zero() => (None, None),
        None => fetch_metrics_snapshot_now(client, cache).await,
        Some(delay) => {
            let now = Instant::now();
            let should_fetch = cache
                .last_poll
                .map(|last| now.saturating_duration_since(last) >= delay)
                .unwrap_or(true);
            if should_fetch {
                cache.last_poll = Some(now);
                match client.fetch_metrics_snapshot().await {
                    Ok(snapshot) => {
                        let arc = Arc::new(snapshot);
                        cache.last_snapshot = Some(arc.clone());
                        cache.last_error = None;
                        (Some(arc), None)
                    }
                    Err(err) => {
                        let summary = err.summarize();
                        cache.last_error = Some(summary.clone());
                        (cache.last_snapshot.clone(), Some(summary))
                    }
                }
            } else {
                (cache.last_snapshot.clone(), cache.last_error.clone())
            }
        }
    }
}

async fn fetch_metrics_snapshot_now(
    client: &ToriiClient,
    cache: &mut MetricsCache,
) -> (Option<Arc<ToriiMetricsSnapshot>>, Option<ToriiErrorInfo>) {
    cache.last_poll = Some(Instant::now());
    match client.fetch_metrics_snapshot().await {
        Ok(snapshot) => {
            let arc = Arc::new(snapshot);
            cache.last_snapshot = Some(arc.clone());
            cache.last_error = None;
            (Some(arc), None)
        }
        Err(err) => {
            let summary = err.summarize();
            cache.last_error = Some(summary.clone());
            (None, Some(summary))
        }
    }
}

fn shutdown_requested(shutdown: &watch::Receiver<bool>) -> bool {
    *shutdown.borrow()
}

async fn wait_for_shutdown_or_delay(shutdown: &mut watch::Receiver<bool>, delay: Duration) -> bool {
    if shutdown_requested(shutdown) {
        return true;
    }

    tokio::select! {
        changed = shutdown.changed() => {
            match changed {
                Ok(_) => shutdown_requested(shutdown),
                Err(_) => true,
            }
        }
        _ = sleep(delay) => false,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        fs,
        io::{ErrorKind, Read, Write},
        net::{SocketAddr, TcpListener as StdTcpListener},
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
        thread,
        time::{Duration, Instant},
    };

    use futures::SinkExt;
    use httpmock::{
        Method::{DELETE, GET, POST},
        MockServer,
    };
    use iroha_crypto::{Hash, KeyPair};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        block::consensus::{ExecWitness, ExecWitnessMsg},
        events::{
            EventBox, SharedDataEvent,
            data::{
                DataEvent, offline,
                prelude::{
                    AccountControllerReplaced, AccountEvent, AccountRecoveryEvent,
                    AccountRecoveryPolicySet, PeerEvent,
                },
            },
            pipeline::PipelineEventBox,
            stream::EventMessage,
            time::{TimeEvent, TimeInterval},
        },
        isi::InstructionBox,
        nexus::LaneLifecyclePlan,
        peer::PeerId,
        query::{
            QueryOutput, QueryOutputBatchBoxTuple, QueryRequest, executor::FindExecutorDataModel,
            prelude::SingularQueryBox,
        },
        transaction::signed::TransactionBuilder,
    };
    use iroha_telemetry::metrics::{
        GovernanceStatus, Status as TelemetryStatus, TxGossipSnapshot, Uptime,
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR, PEER_KEYPAIR};
    use reqwest::{
        StatusCode,
        header::{HeaderMap, HeaderValue},
    };

    use super::*;

    fn handle_bind_result<T>(result: std::io::Result<T>, context: &str) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!("skipping {context}: {err}");
                None
            }
            Err(err) => panic!("{context}: {err}"),
        }
    }
    use std::iter;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::{Mutex as AsyncMutex, Notify, broadcast, oneshot},
        time::{sleep, timeout},
    };
    use tokio_tungstenite::tungstenite::{
        handshake::server::{Request as WsHandshakeRequest, Response as WsHandshakeResponse},
        protocol::Message as WsMessage,
    };

    fn try_start_mock_server() -> Option<MockServer> {
        std::panic::catch_unwind(MockServer::start)
            .ok()
            .or_else(|| {
                eprintln!(
                    "Skipping test: unable to bind mock HTTP server in sandboxed environment."
                );
                None
            })
    }

    fn spawn_status_stub(
        responses: Vec<(u16, Vec<u8>)>,
    ) -> Option<(SocketAddr, mpsc::Sender<()>, thread::JoinHandle<()>)> {
        let listener = match StdTcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(err)
                if matches!(
                    err.kind(),
                    ErrorKind::PermissionDenied | ErrorKind::AddrNotAvailable
                ) =>
            {
                eprintln!("skipping spawn_status_stub: {err}");
                return None;
            }
            Err(err) => panic!("bind status stub: {err}"),
        };
        listener
            .set_nonblocking(true)
            .expect("configure nonblocking");
        let addr = listener.local_addr().expect("local addr");
        let (shutdown_tx, shutdown_rx) = mpsc::channel();

        let shared = Arc::new(Mutex::new(VecDeque::from(responses)));
        let fallback = shared
            .lock()
            .expect("responses mutex")
            .back()
            .cloned()
            .unwrap_or((503, Vec::new()));

        let handle = thread::spawn(move || {
            loop {
                if shutdown_rx.try_recv().is_ok() {
                    break;
                }

                match listener.accept() {
                    Ok((mut stream, _peer)) => {
                        let mut buffer = [0u8; 1024];
                        let _ = stream.read(&mut buffer);
                        let (status, body) = {
                            let mut guard = shared.lock().expect("responses mutex");
                            guard.pop_front().unwrap_or_else(|| fallback.clone())
                        };
                        let reason = if status == 200 {
                            "OK"
                        } else {
                            "Service Unavailable"
                        };
                        let header = format!(
                            "HTTP/1.1 {status} {reason}\r\ncontent-length: {}\r\ncontent-type: application/norito\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(&body);
                    }
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Some((addr, shutdown_tx, handle))
    }

    #[test]
    fn normalises_base_url_without_trailing_slash() {
        let client = ToriiClient::new("http://localhost:8080/").expect("valid url");
        assert_eq!(client.base_url(), "http://localhost:8080/");
    }

    #[test]
    fn summarizes_errors_with_kind_and_detail() {
        let summary = ToriiError::UnsupportedScheme {
            scheme: "ftp".to_string(),
        }
        .summarize();
        assert_eq!(summary.kind, ToriiErrorKind::UnsupportedScheme);
        assert_eq!(summary.message, "Unsupported Torii URL scheme");
        assert_eq!(summary.detail.as_deref(), Some("ftp"));

        let summary = ToriiError::UnexpectedStatus {
            status: StatusCode::NOT_FOUND,
            reject_code: None,
            message: None,
        }
        .summarize();
        assert_eq!(summary.kind, ToriiErrorKind::UnexpectedStatus);
        assert_eq!(summary.detail.as_deref(), Some("404 Not Found"));
        assert!(
            summary
                .message
                .starts_with("Unexpected Torii status code 404")
        );

        let summary = ToriiError::Decode("bad payload".into()).summarize();
        assert_eq!(summary.kind, ToriiErrorKind::Decode);
        assert_eq!(summary.detail.as_deref(), Some("bad payload"));
        assert_eq!(
            summary.message,
            "Failed to decode Norito payload from Torii"
        );
    }

    #[test]
    fn reject_code_and_message_helpers_extract_values() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-iroha-reject-code",
            HeaderValue::from_static("PRTRY:TX_SIGNATURE_INVALID"),
        );
        assert_eq!(
            reject_code_from_headers(&headers).as_deref(),
            Some("PRTRY:TX_SIGNATURE_INVALID")
        );
        let mut axt_headers = HeaderMap::new();
        axt_headers.insert(
            "x-iroha-axt-code",
            HeaderValue::from_static("AXT_HANDLE_ERA"),
        );
        assert_eq!(
            reject_code_from_headers(&axt_headers).as_deref(),
            Some("AXT_HANDLE_ERA")
        );

        let envelope = ToriiErrorEnvelope {
            code: "PRTRY:AXT_HANDLE_ERA".to_owned(),
            message: "handle era too low".to_owned(),
        };
        let body = norito::to_bytes(&envelope).expect("encode envelope");
        assert_eq!(
            error_message_from_body(&body).as_deref(),
            Some("PRTRY:AXT_HANDLE_ERA: handle era too low")
        );
    }

    #[test]
    fn builder_applies_basic_auth_header() {
        let client = ToriiClient::builder("http://localhost:8080")
            .expect("builder")
            .with_basic_auth("demo", "secret")
            .expect("basic auth header")
            .build()
            .expect("client");
        let header = client
            .default_headers
            .get("authorization")
            .expect("authorization header");
        assert_eq!(header.to_str().unwrap(), "Basic ZGVtbzpzZWNyZXQ=");
    }

    #[test]
    fn rejects_unsupported_base_scheme() {
        let err =
            ToriiClient::new("ftp://localhost:8080").expect_err("unsupported scheme should error");
        match err {
            ToriiError::UnsupportedScheme { scheme } => assert_eq!(scheme, "ftp"),
            other => panic!("expected UnsupportedScheme, got {other:?}"),
        }
    }

    #[test]
    fn builds_ws_base_from_https_scheme() {
        let builder = ToriiClientBuilder::new("https://example.com/api/").expect("valid base url");
        assert_eq!(builder.http_base.scheme(), "https");
        assert_eq!(builder.ws_base.scheme(), "wss");
        assert_eq!(builder.ws_base.host_str(), Some("example.com"));
        assert_eq!(builder.ws_base.path(), "/api/");
    }

    #[test]
    fn composes_transaction_endpoint() {
        let client = ToriiClient::new("http://127.0.0.1:8080").expect("valid url");
        assert_eq!(
            client.transaction_endpoint().unwrap().as_str(),
            "http://127.0.0.1:8080/transaction"
        );
    }

    #[test]
    fn composes_nexus_lifecycle_endpoint() {
        let client = ToriiClient::new("http://127.0.0.1:8080").expect("valid url");
        assert_eq!(
            client.nexus_lifecycle_endpoint().unwrap().as_str(),
            "http://127.0.0.1:8080/v1/nexus/lifecycle"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_transaction_reports_http_error() {
        let client = ToriiClient::new("http://127.0.0.1:65535").expect("valid url");
        let err = client
            .submit_transaction(&[])
            .await
            .expect_err("connection should fail");
        matches!(err, ToriiError::Http(_));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_transaction_returns_unexpected_status_on_non_success() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(POST).path("/transaction");
            then.status(503);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .submit_transaction(&[1, 2, 3])
            .await
            .expect_err("non-success status should error");

        mock.assert();
        match err {
            ToriiError::UnexpectedStatus { status, .. } => {
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            }
            other => panic!("expected UnexpectedStatus, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_status_returns_unexpected_status_with_accept_header() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET).path("/status");
            then.status(503);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .fetch_status()
            .await
            .expect_err("non-success status should error");

        mock.assert();
        match err {
            ToriiError::UnexpectedStatus { status, .. } => {
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            }
            other => panic!("expected UnexpectedStatus, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_status_returns_decode_error_on_invalid_payload() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET).path("/status");
            then.status(200).body(vec![0xAA, 0xBB]);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .fetch_status()
            .await
            .expect_err("invalid payload should fail");

        mock.assert();
        match err {
            ToriiError::Decode(message) => {
                assert!(
                    !message.is_empty(),
                    "decode error should propagate message: {message:?}"
                );
            }
            other => panic!("expected Decode error, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_sumeragi_status_returns_unexpected_status_on_non_success() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET).path("/v1/sumeragi/status");
            then.status(500);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .fetch_sumeragi_status()
            .await
            .expect_err("non-success status should error");

        mock.assert();
        match err {
            ToriiError::UnexpectedStatus { status, .. } => {
                assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            }
            other => panic!("expected UnexpectedStatus, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_sumeragi_status_returns_decode_error_on_invalid_payload() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET).path("/v1/sumeragi/status");
            then.status(200).body(vec![0x10, 0x42]);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .fetch_sumeragi_status()
            .await
            .expect_err("invalid payload should fail");

        mock.assert();
        match err {
            ToriiError::Decode(message) => {
                assert!(
                    !message.is_empty(),
                    "decode error should propagate message: {message:?}"
                );
            }
            other => panic!("expected Decode error, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_configuration_returns_unexpected_status_on_non_success() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET).path("/configuration");
            then.status(404);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .fetch_configuration()
            .await
            .expect_err("non-success status should error");

        mock.assert();
        match err {
            ToriiError::UnexpectedStatus { status, .. } => {
                assert_eq!(status, StatusCode::NOT_FOUND);
            }
            other => panic!("expected UnexpectedStatus, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_configuration_returns_decode_error_on_invalid_json() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET).path("/configuration");
            then.status(200).body("not-json");
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .fetch_configuration()
            .await
            .expect_err("invalid json should fail");

        mock.assert();
        match err {
            ToriiError::Decode(message) => {
                assert!(
                    !message.is_empty(),
                    "decode error should propagate message: {message:?}"
                );
            }
            other => panic!("expected Decode error, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_metrics_returns_unexpected_status_on_non_success() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET).path("/metrics");
            then.status(503);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .fetch_metrics()
            .await
            .expect_err("non-success status should error");

        mock.assert();
        match err {
            ToriiError::UnexpectedStatus { status, .. } => {
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            }
            other => panic!("expected UnexpectedStatus, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_query_returns_bytes_on_success() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let response_body = vec![0xAA, 0xBB, 0xCC];
        let mock = server.mock(|when, then| {
            when.method(POST).path("/query");
            then.status(200).body(response_body.clone());
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let bytes = client
            .submit_query(&[0x10, 0x11, 0x12])
            .await
            .expect("query bytes");

        mock.assert();
        assert_eq!(bytes, response_body);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn builder_applies_api_token_to_http_requests() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let status = TelemetryStatus::default();
        let body = norito::to_bytes(&status).expect("encode status");
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/status")
                .header("x-api-token", "secret-token");
            then.status(200).body(body.clone());
        });

        let client = ToriiClient::builder(server.url("/"))
            .expect("builder")
            .with_api_token("secret-token")
            .expect("token builder")
            .build()
            .expect("client");

        let fetched = client.fetch_status().await.expect("status");

        mock.assert();
        assert_eq!(fetched.queue_size, status.queue_size);
        assert_eq!(fetched.txs_approved, status.txs_approved);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_query_returns_unexpected_status_on_non_success() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(POST).path("/query");
            then.status(500);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .submit_query(&[])
            .await
            .expect_err("non-success status should error");

        mock.assert();
        match err {
            ToriiError::UnexpectedStatus { status, .. } => {
                assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            }
            other => panic!("expected UnexpectedStatus, got {other:?}"),
        }
    }

    #[test]
    fn builder_rejects_invalid_api_token_value() {
        let err = ToriiClient::builder("http://127.0.0.1:8080")
            .and_then(|builder| builder.with_api_token("invalid\r\ntoken"))
            .expect_err("invalid header should error");
        matches!(err, ToriiError::InvalidHeader { .. });
    }

    #[tokio::test(flavor = "current_thread")]
    async fn builder_applies_api_token_to_websocket_requests() {
        let listener =
            match handle_bind_result(TcpListener::bind("127.0.0.1:0").await, "bind ws listener") {
                Some(listener) => listener,
                None => return,
            };
        let addr = listener.local_addr().expect("listener addr");
        let (header_tx, header_rx) = oneshot::channel();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept ws stream");
            let mut tx = Some(header_tx);
            let callback = move |req: &WsHandshakeRequest, response: WsHandshakeResponse| {
                if let Some(sender) = tx.take() {
                    let value = req
                        .headers()
                        .get("x-api-token")
                        .and_then(|header| header.to_str().ok())
                        .map(str::to_owned);
                    let _ = sender.send(value);
                }
                Ok(response)
            };
            let mut ws = tokio_tungstenite::accept_hdr_async(stream, callback)
                .await
                .expect("handshake");
            ws.close(None).await.expect("server close");
        });

        let mut ws = ToriiClient::builder(format!("http://{addr}"))
            .expect("builder")
            .with_api_token("secret-token")
            .expect("token builder")
            .build()
            .expect("client")
            .connect_block_stream()
            .await
            .expect("connect block stream with header");
        ws.close(None).await.expect("client close");

        let header = header_rx.await.expect("header observed");
        assert_eq!(header.as_deref(), Some("secret-token"));

        server.await.expect("server join");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn block_stream_reports_ws_error() {
        let client = ToriiClient::new("http://127.0.0.1:65535").expect("valid url");
        let err = client
            .connect_block_stream()
            .await
            .expect_err("connection should fail");
        matches!(err, ToriiError::WebSocket(_));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn subscribe_block_stream_reports_ws_error() {
        let client = ToriiClient::new("http://127.0.0.1:65535").expect("valid url");
        let err = client
            .subscribe_block_stream()
            .await
            .expect_err("connection should fail");
        matches!(err, ToriiError::WebSocket(_));
    }

    const BLOCK_WIRE_FIXTURE: &[u8] = include_bytes!("../tests/fixtures/canonical_block_wire.bin");
    const EVENT_MESSAGE_FIXTURE: &[u8] =
        include_bytes!("../tests/fixtures/canonical_event_message.bin");
    const PIPELINE_EVENT_MESSAGE_FIXTURE: &[u8] =
        include_bytes!("../tests/fixtures/canonical_pipeline_event_message.bin");
    const DATA_EVENT_MESSAGE_FIXTURE: &[u8] =
        include_bytes!("../tests/fixtures/canonical_data_event_message.bin");

    fn sample_block() -> SignedBlock {
        let chain: ChainId = "mochi-block-stream".parse().expect("chain id");
        let mut builder = TransactionBuilder::new(chain.clone(), ALICE_ID.clone());
        builder.set_creation_time(Duration::from_secs(42));
        let builder = builder.with_instructions(core::iter::empty::<InstructionBox>());
        let tx = builder.sign(ALICE_KEYPAIR.private_key());
        SignedBlock::genesis(vec![tx], PEER_KEYPAIR.private_key(), None, None)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_and_wait_for_commit_reports_block_height() {
        let block = sample_block();
        let tx_hash = block
            .transactions_vec()
            .first()
            .expect("sample block tx")
            .hash();
        let expected_height = block.header().height().get();

        let (block_tx, block_rx) = broadcast::channel(8);
        let (_event_tx, event_rx) = broadcast::channel(8);

        let summary = BlockSummary::from_block(&block);
        let event = BlockStreamEvent::Block {
            summary,
            block: Arc::new(block),
            raw_len: 0,
        };

        let handle = tokio::spawn(async move {
            submit_and_wait_for_commit_with_receivers(
                tx_hash,
                SmokeCommitOptions::new(Duration::from_secs(1)),
                async { Ok(()) },
                block_rx,
                event_rx,
            )
            .await
        });

        let _ = block_tx.send(event);

        let observed_height = handle
            .await
            .expect("join smoke wait task")
            .expect("smoke commit should succeed");
        assert_eq!(observed_height, expected_height);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_and_wait_for_commit_times_out_without_events() {
        let block = sample_block();
        let tx_hash = block
            .transactions_vec()
            .first()
            .expect("sample block tx")
            .hash();

        let (_block_tx, block_rx) = broadcast::channel(8);
        let (_event_tx, event_rx) = broadcast::channel(8);

        let err = submit_and_wait_for_commit_with_receivers(
            tx_hash,
            SmokeCommitOptions::new(Duration::from_millis(25)),
            async { Ok(()) },
            block_rx,
            event_rx,
        )
        .await
        .expect_err("should time out");
        assert!(matches!(err, ToriiError::Timeout { .. }));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_and_wait_for_commit_reports_rejected_when_expired_event_arrives() {
        use iroha_data_model::{
            events::pipeline::{TransactionEvent, TransactionStatus},
            nexus::{DataSpaceId, LaneId},
        };

        let block = sample_block();
        let tx_hash = block
            .transactions_vec()
            .first()
            .expect("sample block tx")
            .hash();
        let task_tx_hash = tx_hash;

        let (_block_tx, block_rx) = broadcast::channel(8);
        let (event_tx, event_rx) = broadcast::channel(8);

        let handle = tokio::spawn(async move {
            submit_and_wait_for_commit_with_receivers(
                task_tx_hash,
                SmokeCommitOptions::new(Duration::from_secs(1)),
                async { Ok(()) },
                block_rx,
                event_rx,
            )
            .await
        });

        let event_box = EventBox::Pipeline(PipelineEventBox::Transaction(TransactionEvent {
            hash: tx_hash,
            block_height: None,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            status: TransactionStatus::Expired,
        }));
        let summary = EventSummary::from_event(&event_box);
        let _ = event_tx.send(EventStreamEvent::Event {
            summary,
            event: Arc::new(event_box),
            raw_len: 0,
        });

        let err = handle
            .await
            .expect("join smoke wait task")
            .expect_err("expired event should reject the smoke transaction");
        match err {
            ToriiError::SmokeRejected { hash, reason } => {
                assert!(hash.contains(&tx_hash.to_string()));
                assert_eq!(reason, "expired");
            }
            other => panic!("expected SmokeRejected error, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_and_wait_for_commit_reports_rejected_when_pipeline_event_rejects() {
        use iroha_data_model::{
            events::pipeline::{TransactionEvent, TransactionStatus},
            nexus::{DataSpaceId, LaneId},
            transaction::error::{TransactionLimitError, TransactionRejectionReason},
        };

        let block = sample_block();
        let tx_hash = block
            .transactions_vec()
            .first()
            .expect("sample block tx")
            .hash();
        let task_tx_hash = tx_hash;

        let (_block_tx, block_rx) = broadcast::channel(8);
        let (event_tx, event_rx) = broadcast::channel(8);

        let handle = tokio::spawn(async move {
            submit_and_wait_for_commit_with_receivers(
                task_tx_hash,
                SmokeCommitOptions::new(Duration::from_secs(1)),
                async { Ok(()) },
                block_rx,
                event_rx,
            )
            .await
        });

        let rejection = Box::new(TransactionRejectionReason::LimitCheck(
            TransactionLimitError {
                reason: "limit".to_owned(),
            },
        ));
        let expected_reason = format!("{rejection:?}");
        let event_box = EventBox::Pipeline(PipelineEventBox::Transaction(TransactionEvent {
            hash: tx_hash,
            block_height: None,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            status: TransactionStatus::Rejected(rejection),
        }));
        let summary = EventSummary::from_event(&event_box);
        let _ = event_tx.send(EventStreamEvent::Event {
            summary,
            event: Arc::new(event_box),
            raw_len: 0,
        });

        let err = handle
            .await
            .expect("join smoke wait task")
            .expect_err("rejected event should reject the smoke transaction");
        match err {
            ToriiError::SmokeRejected { hash, reason } => {
                assert_eq!(hash, tx_hash.to_string());
                assert_eq!(reason, expected_reason);
            }
            other => panic!("expected SmokeRejected error, got {other:?}"),
        }
    }

    fn sample_time_event_box() -> EventBox {
        let interval = TimeInterval::new(Duration::from_secs(42), Duration::from_secs(3));
        EventBox::Time(TimeEvent::new(interval))
    }

    fn sample_pipeline_event_box() -> EventBox {
        let block = sample_block();
        let header = block.header();
        let witness = ExecWitnessMsg {
            block_hash: block.hash(),
            height: header.height().get(),
            view: header.view_change_index(),
            epoch: 0,
            witness: ExecWitness {
                reads: Vec::new(),
                writes: Vec::new(),
                fastpq_transcripts: Vec::new(),
                fastpq_batches: Vec::new(),
            },
        };
        EventBox::Pipeline(PipelineEventBox::Witness(witness))
    }

    fn sample_data_event_box() -> EventBox {
        let peer_id = PeerId::from(PEER_KEYPAIR.public_key().clone());
        let event = DataEvent::Peer(PeerEvent::Added(peer_id));
        EventBox::Data(SharedDataEvent::from(event))
    }

    fn sample_time_event_message() -> EventMessage {
        event_message_from_box(sample_time_event_box())
    }

    fn sample_pipeline_event_message() -> EventMessage {
        event_message_from_box(sample_pipeline_event_box())
    }

    fn sample_data_event_message() -> EventMessage {
        event_message_from_box(sample_data_event_box())
    }

    fn event_message_from_box(event: EventBox) -> EventMessage {
        EventMessage::new(event)
    }

    fn time_event_fixture_message() -> EventMessage {
        decode_norito_with_alignment(EVENT_MESSAGE_FIXTURE).expect("decode event fixture")
    }

    fn pipeline_event_fixture_message() -> EventMessage {
        decode_norito_with_alignment(PIPELINE_EVENT_MESSAGE_FIXTURE)
            .expect("decode pipeline event fixture")
    }

    fn data_event_fixture_message() -> EventMessage {
        decode_norito_with_alignment(DATA_EVENT_MESSAGE_FIXTURE).expect("decode data event fixture")
    }

    fn time_event_fixture_event() -> EventBox {
        time_event_fixture_message().into()
    }

    fn pipeline_event_fixture_event() -> EventBox {
        pipeline_event_fixture_message().into()
    }

    fn data_event_fixture_event() -> EventBox {
        data_event_fixture_message().into()
    }

    fn sample_sumeragi_status_wire() -> SumeragiStatusWire {
        use iroha_crypto::{Hash, HashOf};
        use iroha_data_model::{
            block::{
                BlockHeader,
                consensus::{
                    SumeragiBlockSyncRosterStatus, SumeragiCommitQuorumStatus,
                    SumeragiDaGateReason, SumeragiDaGateSatisfaction, SumeragiDaGateStatus,
                    SumeragiDataspaceCommitment, SumeragiKuraStoreStatus, SumeragiLaneCommitment,
                    SumeragiLaneGovernance, SumeragiMembershipMismatchStatus,
                    SumeragiMembershipStatus, SumeragiMissingBlockFetchStatus,
                    SumeragiPendingRbcStatus, SumeragiRbcStoreStatus, SumeragiRuntimeUpgradeHook,
                    SumeragiStatusWire, SumeragiValidationRejectStatus,
                    SumeragiViewChangeCauseStatus,
                },
            },
            nexus::{DataSpaceId, LaneId},
        };

        SumeragiStatusWire {
            mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_string(),
            staged_mode_tag: None,
            staged_mode_activation_height: None,
            mode_activation_lag_blocks: None,
            mode_flip_kill_switch: true,
            mode_flip_blocked: false,
            mode_flip_success_total: 0,
            mode_flip_fail_total: 0,
            mode_flip_blocked_total: 0,
            last_mode_flip_timestamp_ms: None,
            last_mode_flip_error: None,
            consensus_caps: None,
            leader_index: 3,
            highest_qc_height: 15,
            highest_qc_view: 6,
            highest_qc_subject: None,
            locked_qc_height: 14,
            locked_qc_view: 5,
            locked_qc_subject: None,
            commit_quorum: SumeragiCommitQuorumStatus::default(),
            view_change_proof_accepted_total: 9,
            view_change_proof_stale_total: 10,
            view_change_proof_rejected_total: 11,
            view_change_suggest_total: 12,
            view_change_install_total: 13,
            view_change_causes: SumeragiViewChangeCauseStatus::default(),
            gossip_fallback_total: 7,
            block_created_dropped_by_lock_total: 2,
            block_created_hint_mismatch_total: 1,
            block_created_proposal_mismatch_total: 4,
            validation_reject_total: 0,
            validation_reject_reason: None,
            validation_rejects: SumeragiValidationRejectStatus::default(),
            peer_key_policy: Default::default(),
            block_sync_roster: SumeragiBlockSyncRosterStatus::default(),
            pacemaker_backpressure_deferrals_total: 8,
            commit_pipeline_tick_total: 0,
            da_reschedule_total: 0,
            missing_block_fetch: SumeragiMissingBlockFetchStatus {
                total: 0,
                last_targets: 0,
                last_dwell_ms: 0,
            },
            committed_edge_conflict_obsolete_total: 0,
            da_gate: SumeragiDaGateStatus {
                reason: SumeragiDaGateReason::None,
                last_satisfied: SumeragiDaGateSatisfaction::None,
                missing_local_data_total: 0,
                manifest_guard_total: 0,
            },
            kura_store: SumeragiKuraStoreStatus {
                failures_total: 0,
                abort_total: 0,
                last_retry_attempt: 0,
                last_retry_backoff_ms: 0,
                last_height: 0,
                last_view: 0,
                last_hash: None,
                ..Default::default()
            },
            rbc_store: SumeragiRbcStoreStatus {
                sessions: 0,
                bytes: 0,
                pressure_level: 0,
                backpressure_deferrals_total: 0,
                persist_drops_total: 0,
                evictions_total: 0,
                recent_evictions: Vec::new(),
            },
            pending_rbc: SumeragiPendingRbcStatus::default(),
            tx_queue_depth: 11,
            tx_queue_capacity: 64,
            tx_queue_saturated: true,
            epoch_length_blocks: 3600,
            epoch_commit_deadline_offset: 120,
            epoch_reveal_deadline_offset: 160,
            prf_epoch_seed: Some([0x11; 32]),
            prf_height: 15,
            prf_view: 6,
            vrf_penalty_epoch: 4,
            vrf_committed_no_reveal_total: 2,
            vrf_no_participation_total: 1,
            vrf_late_reveals_total: 3,
            consensus_penalties_applied_total: 0,
            consensus_penalties_pending: 0,
            vrf_penalties_applied_total: 0,
            vrf_penalties_pending: 0,
            membership: SumeragiMembershipStatus {
                height: 15,
                view: 6,
                epoch: 4,
                view_hash: Some([0xAB; 32]),
            },
            membership_mismatch: SumeragiMembershipMismatchStatus::default(),
            lane_commitments: vec![SumeragiLaneCommitment {
                block_height: 15,
                lane_id: LaneId::new(1),
                tx_count: 4,
                total_chunks: 2,
                rbc_bytes_total: 128,
                teu_total: 64,
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0x91; Hash::LENGTH],
                )),
            }],
            dataspace_commitments: vec![SumeragiDataspaceCommitment {
                block_height: 15,
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(7),
                tx_count: 2,
                total_chunks: 1,
                rbc_bytes_total: 96,
                teu_total: 32,
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0x92; Hash::LENGTH],
                )),
            }],
            lane_settlement_commitments: Vec::new(),
            lane_relay_envelopes: Vec::new(),
            lane_governance_sealed_total: 0,
            lane_governance_sealed_aliases: Vec::new(),
            lane_governance: vec![SumeragiLaneGovernance {
                lane_id: LaneId::new(1),
                alias: "alpha".to_owned(),
                governance: Some("parliament".to_owned()),
                manifest_required: true,
                manifest_ready: true,
                manifest_path: Some("/etc/iroha/lanes/alpha.json".to_owned()),
                validator_ids: vec![
                    "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
                        .to_owned(),
                ],
                quorum: Some(2),
                protected_namespaces: vec!["finance".to_owned()],
                runtime_upgrade: Some(SumeragiRuntimeUpgradeHook {
                    allow: true,
                    require_metadata: true,
                    metadata_key: Some("upgrade_id".to_owned()),
                    allowed_ids: vec!["alpha-upgrade".to_owned()],
                }),
            }],
            worker_loop: Default::default(),
            commit_inflight: Default::default(),
            ..Default::default()
        }
    }

    #[test]
    #[ignore]
    fn regenerate_block_wire_fixture() {
        let wire = sample_block()
            .canonical_wire()
            .expect("canonical wire")
            .into_vec();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let fixture_path = root.join("tests/fixtures/canonical_block_wire.bin");
        if let Some(parent) = fixture_path.parent() {
            fs::create_dir_all(parent).expect("create fixture dir");
        }
        fs::write(&fixture_path, &wire).expect("write fixture");
    }

    #[test]
    #[ignore]
    fn regenerate_time_event_message_fixture() {
        let message = sample_time_event_message();
        let (payload, flags) = norito::codec::encode_with_header_flags(&message);
        let bytes = norito::core::frame_bare_with_header_flags::<EventMessage>(&payload, flags)
            .expect("frame event message");
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let fixture_path = root.join("tests/fixtures/canonical_event_message.bin");
        if let Some(parent) = fixture_path.parent() {
            fs::create_dir_all(parent).expect("create fixture dir");
        }
        fs::write(&fixture_path, &bytes).expect("write event fixture");
    }

    #[test]
    #[ignore]
    fn regenerate_pipeline_event_message_fixture() {
        let message = sample_pipeline_event_message();
        let (payload, flags) = norito::codec::encode_with_header_flags(&message);
        let bytes = norito::core::frame_bare_with_header_flags::<EventMessage>(&payload, flags)
            .expect("frame pipeline event message");
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let fixture_path = root.join("tests/fixtures/canonical_pipeline_event_message.bin");
        if let Some(parent) = fixture_path.parent() {
            fs::create_dir_all(parent).expect("create fixture dir");
        }
        fs::write(&fixture_path, &bytes).expect("write pipeline event fixture");
    }

    #[test]
    #[ignore]
    fn regenerate_data_event_message_fixture() {
        let message = sample_data_event_message();
        let (payload, flags) = norito::codec::encode_with_header_flags(&message);
        let bytes = norito::core::frame_bare_with_header_flags::<EventMessage>(&payload, flags)
            .expect("frame data event message");
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let fixture_path = root.join("tests/fixtures/canonical_data_event_message.bin");
        if let Some(parent) = fixture_path.parent() {
            fs::create_dir_all(parent).expect("create fixture dir");
        }
        fs::write(&fixture_path, &bytes).expect("write data event fixture");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn block_stream_decodes_block_events() {
        let expected_block = sample_block();
        let expected_summary = BlockSummary::from_block(&expected_block);
        let (sender, _) = broadcast::channel(8);
        let handle = tokio::spawn(async {});
        let subscription = WsSubscription {
            sender: sender.clone(),
            handle,
        };
        let stream = BlockStream::new(subscription);
        let mut receiver = stream.subscribe();

        sender
            .send(WsFrame::Binary(BLOCK_WIRE_FIXTURE.to_vec()))
            .expect("send frame");
        sender.send(WsFrame::Closed).expect("send close");

        let event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("timely block event")
            .expect("block event value");
        match event {
            BlockStreamEvent::Block {
                summary,
                block,
                raw_len,
            } => {
                assert_eq!(raw_len, BLOCK_WIRE_FIXTURE.len());
                assert_eq!(summary.height, expected_summary.height);
                assert_eq!(
                    summary.transaction_count,
                    expected_summary.transaction_count
                );
                assert_eq!(
                    summary.rejected_transaction_count,
                    expected_summary.rejected_transaction_count
                );
                assert_eq!(summary.signature_count, expected_summary.signature_count);
                assert_eq!(summary.hash_hex, expected_summary.hash_hex);
                assert_eq!(
                    summary.view_change_index,
                    expected_summary.view_change_index
                );
                assert_eq!(summary.creation_time_ms, expected_summary.creation_time_ms);
                assert_eq!(summary.is_genesis, expected_summary.is_genesis);
                assert_eq!(block.as_ref(), &expected_block);
            }
            other => panic!("expected block event, got {other:?}"),
        }

        stream.abort();
    }

    #[test]
    fn block_canonical_wire_matches_fixture() {
        let wire = sample_block()
            .canonical_wire()
            .expect("canonical wire")
            .into_vec();
        assert_eq!(wire.as_slice(), BLOCK_WIRE_FIXTURE);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn block_stream_end_to_end_decodes_canonical_block() {
        let listener = match handle_bind_result(
            TcpListener::bind("127.0.0.1:0").await,
            "bind block stream listener",
        ) {
            Some(listener) => listener,
            None => return,
        };
        let addr = listener.local_addr().expect("listener addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept block stream");
            let mut ws = tokio_tungstenite::accept_async(stream)
                .await
                .expect("handshake for block stream");
            ws.send(WsMessage::Binary(BLOCK_WIRE_FIXTURE.to_vec().into()))
                .await
                .expect("send block fixture");
            ws.send(WsMessage::Close(None))
                .await
                .expect("send block close");
        });

        let client = ToriiClient::new(format!("http://{addr}")).expect("block client");
        let stream = client
            .block_stream()
            .await
            .expect("connect block stream end-to-end");
        let mut receiver = stream.subscribe();

        let event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("timely block event")
            .expect("block event value");

        let expected_block = sample_block();
        let expected_summary = BlockSummary::from_block(&expected_block);
        match event {
            BlockStreamEvent::Block {
                summary,
                block,
                raw_len,
            } => {
                assert_eq!(raw_len, BLOCK_WIRE_FIXTURE.len());
                assert_eq!(summary.hash_hex, expected_summary.hash_hex);
                assert_eq!(summary.height, expected_summary.height);
                assert_eq!(summary.signature_count, expected_summary.signature_count);
                assert_eq!(
                    summary.transaction_count,
                    expected_summary.transaction_count
                );
                assert_eq!(
                    summary.rejected_transaction_count,
                    expected_summary.rejected_transaction_count
                );
                assert_eq!(
                    summary.view_change_index,
                    expected_summary.view_change_index
                );
                assert_eq!(summary.creation_time_ms, expected_summary.creation_time_ms);
                assert_eq!(summary.is_genesis, expected_summary.is_genesis);
                assert_eq!(block.as_ref(), &expected_block);
            }
            other => panic!("expected block event, got {other:?}"),
        }

        stream.abort();
        server.await.expect("server task finished");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn block_stream_reports_decode_errors() {
        let (sender, _) = broadcast::channel(8);
        let handle = tokio::spawn(async {});
        let subscription = WsSubscription {
            sender: sender.clone(),
            handle,
        };
        let stream = BlockStream::new(subscription);
        let mut receiver = stream.subscribe();

        sender
            .send(WsFrame::Binary(vec![0, 1, 2]))
            .expect("send invalid frame");
        sender.send(WsFrame::Closed).expect("send close");

        let event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("timely error event")
            .expect("error event value");
        match event {
            BlockStreamEvent::DecodeError { error } => {
                assert!(matches!(
                    error.stage,
                    BlockDecodeStage::Frame | BlockDecodeStage::Block
                ));
                assert_eq!(error.raw_len, 3);
            }
            other => panic!("expected decode error event, got {other:?}"),
        }

        stream.abort();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_stream_reports_decode_errors() {
        let (sender, _) = broadcast::channel(8);
        let handle = tokio::spawn(async {});
        let subscription = WsSubscription {
            sender: sender.clone(),
            handle,
        };
        let stream = EventStream::new(subscription);
        let mut receiver = stream.subscribe();

        sender
            .send(WsFrame::Binary(vec![1, 2, 3]))
            .expect("send invalid frame");
        sender.send(WsFrame::Closed).expect("send close");

        let event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("error available")
            .expect("error value");

        match event {
            EventStreamEvent::DecodeError { error } => {
                assert_eq!(error.stage, EventDecodeStage::Frame);
            }
            other => panic!("expected decode error event, got {other:?}"),
        }

        stream.abort();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_stream_decodes_time_events() {
        let expected_event = sample_time_event_box();
        assert_eq!(time_event_fixture_event(), expected_event);
        let expected_summary = EventSummary::from_event(&expected_event);

        let (sender, _) = broadcast::channel(8);
        let handle = tokio::spawn(async {});
        let subscription = WsSubscription {
            sender: sender.clone(),
            handle,
        };
        let stream = EventStream::new(subscription);
        let mut receiver = stream.subscribe();

        sender
            .send(WsFrame::Binary(EVENT_MESSAGE_FIXTURE.to_vec()))
            .expect("send event frame");
        sender.send(WsFrame::Closed).expect("send close");

        let event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("timely event")
            .expect("event value");
        match event {
            EventStreamEvent::Event {
                summary: emitted_summary,
                event,
                raw_len,
            } => {
                assert_eq!(raw_len, EVENT_MESSAGE_FIXTURE.len());
                assert_eq!(emitted_summary.category, EventCategory::Time);
                assert_eq!(emitted_summary.label, expected_summary.label);
                assert_eq!(emitted_summary.detail, expected_summary.detail);
                match (event.as_ref(), &expected_event) {
                    (EventBox::Time(actual), EventBox::Time(expected)) => {
                        assert_eq!(actual.interval().since(), expected.interval().since());
                        assert_eq!(actual.interval().length(), expected.interval().length());
                    }
                    (other, _) => panic!("expected time event, got {other:?}"),
                }

                assert_eq!(event.as_ref(), &expected_event);
            }
            other => panic!("expected decoded event, got {other:?}"),
        }

        stream.abort();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_stream_decodes_pipeline_events() {
        let expected_event_box = sample_pipeline_event_box();
        assert_eq!(pipeline_event_fixture_event(), expected_event_box);
        let expected_summary = EventSummary::from_event(&expected_event_box);
        let expected_event = Arc::new(expected_event_box);

        let (sender, _) = broadcast::channel(8);
        let handle = tokio::spawn(async {});
        let subscription = WsSubscription {
            sender: sender.clone(),
            handle,
        };
        let stream = EventStream::new(subscription);
        let mut receiver = stream.subscribe();

        sender
            .send(WsFrame::Binary(PIPELINE_EVENT_MESSAGE_FIXTURE.to_vec()))
            .expect("send pipeline event frame");
        sender.send(WsFrame::Closed).expect("send close");

        let event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("timely pipeline event")
            .expect("pipeline event value");
        match event {
            EventStreamEvent::Event {
                summary: emitted_summary,
                event,
                raw_len,
            } => {
                assert_eq!(raw_len, PIPELINE_EVENT_MESSAGE_FIXTURE.len());
                assert_eq!(emitted_summary.category, EventCategory::Pipeline);
                assert_eq!(emitted_summary.label, expected_summary.label);
                assert_eq!(emitted_summary.detail, expected_summary.detail);
                match (event.as_ref(), expected_event.as_ref()) {
                    (EventBox::Pipeline(actual), EventBox::Pipeline(expected)) => {
                        assert_eq!(actual, expected);
                    }
                    (other, _) => panic!("expected pipeline event, got {other:?}"),
                }
            }
            other => panic!("expected decoded pipeline event, got {other:?}"),
        }

        stream.abort();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn events_stream_end_to_end_decodes_pipeline_event() {
        let listener = match handle_bind_result(
            TcpListener::bind("127.0.0.1:0").await,
            "bind events listener",
        ) {
            Some(listener) => listener,
            None => return,
        };
        let addr = listener.local_addr().expect("events listener addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept events stream");
            let mut ws = tokio_tungstenite::accept_async(stream)
                .await
                .expect("handshake for events stream");
            ws.send(WsMessage::Binary(
                PIPELINE_EVENT_MESSAGE_FIXTURE.to_vec().into(),
            ))
            .await
            .expect("send pipeline event fixture");
            ws.send(WsMessage::Close(None))
                .await
                .expect("send events close");
        });

        let client = ToriiClient::new(format!("http://{addr}")).expect("events client");
        let stream = client
            .events_stream()
            .await
            .expect("connect events stream end-to-end");
        let mut receiver = stream.subscribe();

        let event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("timely pipeline event")
            .expect("pipeline event value");

        let expected_event_box = pipeline_event_fixture_event();
        let expected_summary = EventSummary::from_event(&expected_event_box);
        match event {
            EventStreamEvent::Event {
                summary: emitted_summary,
                event,
                raw_len,
            } => {
                assert_eq!(raw_len, PIPELINE_EVENT_MESSAGE_FIXTURE.len());
                assert_eq!(emitted_summary.category, EventCategory::Pipeline);
                assert_eq!(emitted_summary.label, expected_summary.label);
                assert_eq!(emitted_summary.detail, expected_summary.detail);
                assert_eq!(event.as_ref(), &expected_event_box);
            }
            other => panic!("expected decoded pipeline event, got {other:?}"),
        }

        stream.abort();
        server.await.expect("events server task finished");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_stream_decodes_data_events() {
        let expected_event_box = sample_data_event_box();
        assert_eq!(data_event_fixture_event(), expected_event_box);
        let expected_summary = EventSummary::from_event(&expected_event_box);
        let expected_event = Arc::new(expected_event_box);

        let (sender, _) = broadcast::channel(8);
        let handle = tokio::spawn(async {});
        let subscription = WsSubscription {
            sender: sender.clone(),
            handle,
        };
        let stream = EventStream::new(subscription);
        let mut receiver = stream.subscribe();

        sender
            .send(WsFrame::Binary(DATA_EVENT_MESSAGE_FIXTURE.to_vec()))
            .expect("send data event frame");
        sender.send(WsFrame::Closed).expect("send close");

        let event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("timely data event")
            .expect("data event value");
        match event {
            EventStreamEvent::Event {
                summary: emitted_summary,
                event,
                raw_len,
            } => {
                assert_eq!(raw_len, DATA_EVENT_MESSAGE_FIXTURE.len());
                assert_eq!(emitted_summary.category, EventCategory::Data);
                assert_eq!(emitted_summary.label, expected_summary.label);
                assert_eq!(emitted_summary.detail, expected_summary.detail);
                match (event.as_ref(), expected_event.as_ref()) {
                    (EventBox::Data(actual), EventBox::Data(expected)) => {
                        assert_eq!(actual.as_ref(), expected.as_ref());
                    }
                    (other, _) => panic!("expected data event, got {other:?}"),
                }
            }
            other => panic!("expected decoded data event, got {other:?}"),
        }

        stream.abort();
    }

    #[test]
    fn time_event_message_matches_fixture() {
        let message = time_event_fixture_message();
        let decoded_again: EventBox =
            decode_norito_with_alignment::<EventMessage>(EVENT_MESSAGE_FIXTURE)
                .expect("decode fixture message")
                .into();
        assert_eq!(EventBox::from(message), decoded_again);
    }

    #[test]
    fn pipeline_event_message_matches_fixture() {
        let message = pipeline_event_fixture_message();
        let decoded_again: EventBox =
            decode_norito_with_alignment::<EventMessage>(PIPELINE_EVENT_MESSAGE_FIXTURE)
                .expect("decode pipeline fixture")
                .into();
        assert_eq!(EventBox::from(message), decoded_again);
    }

    #[test]
    fn data_event_message_matches_fixture() {
        let message = data_event_fixture_message();
        let decoded_again: EventBox =
            decode_norito_with_alignment::<EventMessage>(DATA_EVENT_MESSAGE_FIXTURE)
                .expect("decode data fixture")
                .into();
        assert_eq!(EventBox::from(message), decoded_again);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_block_stream_reconnects_and_forwards_events() {
        let handle = tokio::runtime::Handle::current();
        let senders = Arc::new(Mutex::new(Vec::<broadcast::Sender<WsFrame>>::new()));
        let notify = Arc::new(Notify::new());

        let factory = {
            let senders = senders.clone();
            let notify = notify.clone();
            move || {
                let senders = senders.clone();
                let notify = notify.clone();
                async move {
                    let (sender, _) = broadcast::channel(16);
                    let task = tokio::spawn(async {});
                    let subscription = WsSubscription {
                        sender: sender.clone(),
                        handle: task,
                    };
                    senders.lock().expect("factory mutex poisoned").push(sender);
                    notify.notify_waiters();
                    Ok(subscription)
                }
            }
        };

        let stream = ManagedBlockStream::spawn_with_factory(&handle, "reconnect-peer", factory);
        let mut receiver = stream.subscribe();

        notify.notified().await;
        assert_eq!(stream.alias(), "reconnect-peer");
        tokio::task::yield_now().await;
        let first_sender = {
            let guard = senders.lock().expect("sender mutex poisoned");
            guard.last().expect("first sender present").clone()
        };

        first_sender
            .send(WsFrame::Text("hello".to_owned()))
            .expect("send hello frame");

        match timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("receive first frame")
        {
            Ok(BlockStreamEvent::Text { text }) => assert_eq!(text, "hello"),
            other => panic!("unexpected event: {other:?}"),
        }

        first_sender
            .send(WsFrame::Closed)
            .expect("send closed frame");

        match timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("receive closed event")
        {
            Ok(BlockStreamEvent::Closed) => {}
            other => panic!("expected closed event, got {other:?}"),
        }

        sleep(INITIAL_BACKOFF).await;
        notify.notified().await;
        tokio::task::yield_now().await;

        let second_sender = {
            let guard = senders.lock().expect("sender mutex poisoned");
            guard.last().expect("second sender present").clone()
        };
        second_sender
            .send(WsFrame::Text("reconnected".to_owned()))
            .expect("send reconnection frame");

        let mut saw_notice = false;
        let mut saw_custom = false;
        for _ in 0..3 {
            match timeout(Duration::from_secs(1), receiver.recv())
                .await
                .expect("receive reconnection events")
            {
                Ok(BlockStreamEvent::Text { text })
                    if text == "Block stream `reconnect-peer` reconnected." =>
                {
                    saw_notice = true;
                }
                Ok(BlockStreamEvent::Text { text }) if text == "reconnected" => {
                    saw_custom = true;
                }
                Ok(other) => panic!("unexpected event after reconnect: {other:?}"),
                Err(err) => panic!("receiver closed unexpectedly: {err:?}"),
            }
            if saw_notice && saw_custom {
                break;
            }
        }
        assert!(saw_notice, "reconnection notice event expected");
        assert!(saw_custom, "custom reconnection frame expected");

        stream.abort();
        sleep(Duration::from_millis(20)).await;
        assert!(stream.is_finished());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_event_stream_reconnects_and_forwards_events() {
        let handle = tokio::runtime::Handle::current();
        let senders = Arc::new(Mutex::new(Vec::<broadcast::Sender<WsFrame>>::new()));
        let notify = Arc::new(Notify::new());

        let factory = {
            let senders = senders.clone();
            let notify = notify.clone();
            move || {
                let senders = senders.clone();
                let notify = notify.clone();
                async move {
                    let (sender, _) = broadcast::channel(16);
                    let task = tokio::spawn(async {});
                    let subscription = WsSubscription {
                        sender: sender.clone(),
                        handle: task,
                    };
                    senders.lock().expect("factory mutex poisoned").push(sender);
                    notify.notify_waiters();
                    Ok(subscription)
                }
            }
        };

        let stream = ManagedEventStream::spawn_with_factory(&handle, "events-peer", factory);
        let mut receiver = stream.subscribe();

        notify.notified().await;
        assert_eq!(stream.alias(), "events-peer");
        tokio::task::yield_now().await;
        let first_sender = {
            let guard = senders.lock().expect("sender mutex poisoned");
            guard.last().expect("first sender present").clone()
        };

        first_sender
            .send(WsFrame::Text("hello-events".to_owned()))
            .expect("send text frame");

        match timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("receive first frame")
        {
            Ok(EventStreamEvent::Text { text }) => assert_eq!(text, "hello-events"),
            other => panic!("unexpected event: {other:?}"),
        }

        first_sender
            .send(WsFrame::Closed)
            .expect("send closed frame");

        match timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("receive closed event")
        {
            Ok(EventStreamEvent::Closed) => {}
            other => panic!("expected closed event, got {other:?}"),
        }

        sleep(INITIAL_BACKOFF).await;
        notify.notified().await;
        tokio::task::yield_now().await;

        let second_sender = {
            let guard = senders.lock().expect("sender mutex poisoned");
            guard.last().expect("second sender present").clone()
        };
        second_sender
            .send(WsFrame::Text("reconnected-events".to_owned()))
            .expect("send reconnection frame");

        let mut saw_notice = false;
        let mut saw_custom = false;
        for _ in 0..3 {
            match timeout(Duration::from_secs(1), receiver.recv())
                .await
                .expect("receive reconnection events")
            {
                Ok(EventStreamEvent::Text { text })
                    if text == "Event stream `events-peer` reconnected." =>
                {
                    saw_notice = true;
                }
                Ok(EventStreamEvent::Text { text }) if text == "reconnected-events" => {
                    saw_custom = true;
                }
                Ok(other) => panic!("unexpected event after reconnect: {other:?}"),
                Err(err) => panic!("receiver closed unexpectedly: {err:?}"),
            }
            if saw_notice && saw_custom {
                break;
            }
        }
        assert!(saw_notice, "reconnection notice event expected");
        assert!(saw_custom, "custom reconnection frame expected");
        stream.abort();
        sleep(Duration::from_millis(20)).await;
        assert!(stream.is_finished());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_status_stream_emits_snapshots() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let status = TelemetryStatus {
            queue_size: 7,
            ..TelemetryStatus::default()
        };
        let body = norito::to_bytes(&status).expect("encode status");
        let sumeragi = sample_sumeragi_status_wire();
        let sumeragi_body = norito::to_bytes(&sumeragi).expect("encode sumeragi status");

        server.mock(|when, then| {
            when.method(GET).path("/status");
            then.status(200)
                .header("content-type", "application/norito")
                .body(body.clone());
        });
        server.mock(|when, then| {
            when.method(GET).path("/v1/sumeragi/status");
            then.status(200)
                .header("content-type", "application/norito")
                .body(sumeragi_body.clone());
        });
        server.mock(|when, then| {
            when.method(GET).path("/metrics");
            then.status(200)
                .body("queue_size 7\nsumeragi_tx_queue_depth 4");
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let handle = tokio::runtime::Handle::current();
        let stream =
            ManagedStatusStream::spawn(&handle, "status-peer", client, Duration::from_millis(10));
        let mut receiver = stream.subscribe();

        match timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("receive snapshot")
        {
            Ok(StatusStreamEvent::Snapshot {
                snapshot,
                sumeragi,
                metrics,
                metrics_error,
            }) => {
                assert_eq!(snapshot.status.queue_size, 7);
                assert!(sumeragi.is_some());
                assert!(metrics.is_some());
                assert!(metrics_error.is_none());
            }
            other => panic!("expected status snapshot event, got {other:?}"),
        }

        stream.abort();
        sleep(Duration::from_millis(10)).await;
        assert!(stream.is_finished());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_status_stream_reports_errors() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        server.mock(|when, then| {
            when.method(GET).path("/status");
            then.status(503);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let handle = tokio::runtime::Handle::current();
        let stream =
            ManagedStatusStream::spawn(&handle, "status-peer", client, Duration::from_millis(10));
        let mut receiver = stream.subscribe();

        match timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("receive error event")
        {
            Ok(StatusStreamEvent::Error {
                error,
                consecutive_failures,
            }) => {
                assert_eq!(error.kind, ToriiErrorKind::UnexpectedStatus);
                assert_eq!(consecutive_failures, 1);
            }
            other => panic!("expected status error event, got {other:?}"),
        }

        stream.abort();
        sleep(Duration::from_millis(10)).await;
        assert!(stream.is_finished());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_status_stream_reports_sumeragi_errors() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let status = TelemetryStatus {
            queue_size: 3,
            ..TelemetryStatus::default()
        };
        let body = norito::to_bytes(&status).expect("encode status");

        server.mock(|when, then| {
            when.method(GET).path("/status");
            then.status(200)
                .header("content-type", "application/norito")
                .body(body.clone());
        });
        server.mock(|when, then| {
            when.method(GET).path("/v1/sumeragi/status");
            then.status(503);
        });
        server.mock(|when, then| {
            when.method(GET).path("/metrics");
            then.status(200).body("queue_size 3");
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let handle = tokio::runtime::Handle::current();
        let stream =
            ManagedStatusStream::spawn(&handle, "status-peer", client, Duration::from_millis(10));
        let mut receiver = stream.subscribe();

        let error_event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("receive sumeragi error")
            .expect("error event");
        match error_event {
            StatusStreamEvent::Error {
                error,
                consecutive_failures,
            } => {
                assert_eq!(error.kind, ToriiErrorKind::UnexpectedStatus);
                assert_eq!(consecutive_failures, 0);
            }
            other => panic!("expected sumeragi error event, got {other:?}"),
        }

        let snapshot_event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("receive snapshot after sumeragi error")
            .expect("snapshot event");
        match snapshot_event {
            StatusStreamEvent::Snapshot {
                snapshot,
                sumeragi,
                metrics,
                metrics_error,
            } => {
                assert_eq!(snapshot.status.queue_size, 3);
                assert!(sumeragi.is_none());
                assert!(metrics.is_some());
                assert!(metrics_error.is_none());
            }
            other => panic!("expected snapshot event, got {other:?}"),
        }

        stream.abort();
        sleep(Duration::from_millis(10)).await;
        assert!(stream.is_finished());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_status_stream_throttles_metrics_requests() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let status = TelemetryStatus {
            queue_size: 2,
            ..TelemetryStatus::default()
        };
        let body = norito::to_bytes(&status).expect("encode status");
        let sumeragi = sample_sumeragi_status_wire();
        let sumeragi_body = norito::to_bytes(&sumeragi).expect("encode sumeragi status");

        server.mock(|when, then| {
            when.method(GET).path("/status");
            then.status(200)
                .header("content-type", "application/norito")
                .body(body.clone());
        });
        server.mock(|when, then| {
            when.method(GET).path("/v1/sumeragi/status");
            then.status(200)
                .header("content-type", "application/norito")
                .body(sumeragi_body.clone());
        });
        let metrics_mock = server.mock(|when, then| {
            when.method(GET).path("/metrics");
            then.status(200)
                .body("queue_size 2\nsumeragi_tx_queue_depth 1");
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let handle = tokio::runtime::Handle::current();
        let options = StatusStreamOptions::new(Duration::from_millis(10))
            .with_metrics_poll_interval(Some(Duration::from_secs(60)));
        let stream =
            ManagedStatusStream::spawn_with_options(&handle, "status-peer", client, options);
        let mut receiver = stream.subscribe();

        let first_event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("first snapshot");
        let first_metrics_present = matches!(
            first_event,
            Ok(StatusStreamEvent::Snapshot {
                metrics: Some(_),
                metrics_error: None,
                ..
            })
        );
        assert!(first_metrics_present, "first poll must fetch metrics");

        let second_event = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("second snapshot");
        let second_metrics_present = matches!(
            second_event,
            Ok(StatusStreamEvent::Snapshot {
                metrics: Some(_),
                ..
            })
        );
        assert!(
            second_metrics_present,
            "cached metrics should be propagated between polls"
        );
        assert_eq!(metrics_mock.calls(), 1);

        stream.abort();
        sleep(Duration::from_millis(10)).await;
        assert!(stream.is_finished());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_status_stream_can_disable_metrics_polling() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let status = TelemetryStatus {
            queue_size: 4,
            ..TelemetryStatus::default()
        };
        let body = norito::to_bytes(&status).expect("encode status");
        let sumeragi = sample_sumeragi_status_wire();
        let sumeragi_body = norito::to_bytes(&sumeragi).expect("encode sumeragi status");

        server.mock(|when, then| {
            when.method(GET).path("/status");
            then.status(200)
                .header("content-type", "application/norito")
                .body(body.clone());
        });
        server.mock(|when, then| {
            when.method(GET).path("/v1/sumeragi/status");
            then.status(200)
                .header("content-type", "application/norito")
                .body(sumeragi_body.clone());
        });
        let metrics_mock = server.mock(|when, then| {
            when.method(GET).path("/metrics");
            then.status(200).body("queue_size 4");
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let handle = tokio::runtime::Handle::current();
        let options = StatusStreamOptions::new(Duration::from_millis(10))
            .with_metrics_poll_interval(Some(Duration::ZERO));
        let stream =
            ManagedStatusStream::spawn_with_options(&handle, "status-peer", client, options);
        let mut receiver = stream.subscribe();

        match timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("snapshot without metrics")
        {
            Ok(StatusStreamEvent::Snapshot {
                metrics,
                metrics_error,
                ..
            }) => {
                assert!(metrics.is_none());
                assert!(metrics_error.is_none());
            }
            other => panic!("expected snapshot without metrics, got {other:?}"),
        }
        assert_eq!(metrics_mock.calls(), 0);

        stream.abort();
        sleep(Duration::from_millis(10)).await;
        assert!(stream.is_finished());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_status_stream_reports_metrics_failures() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let status = TelemetryStatus {
            queue_size: 5,
            ..TelemetryStatus::default()
        };
        let body = norito::to_bytes(&status).expect("encode status");
        let sumeragi = sample_sumeragi_status_wire();
        let sumeragi_body = norito::to_bytes(&sumeragi).expect("encode sumeragi status");

        server.mock(|when, then| {
            when.method(GET).path("/status");
            then.status(200)
                .header("content-type", "application/norito")
                .body(body.clone());
        });
        server.mock(|when, then| {
            when.method(GET).path("/v1/sumeragi/status");
            then.status(200)
                .header("content-type", "application/norito")
                .body(sumeragi_body.clone());
        });
        server.mock(|when, then| {
            when.method(GET).path("/metrics");
            then.status(503);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let handle = tokio::runtime::Handle::current();
        let stream =
            ManagedStatusStream::spawn(&handle, "status-peer", client, Duration::from_millis(10));
        let mut receiver = stream.subscribe();

        match timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("receive metrics snapshot")
        {
            Ok(StatusStreamEvent::Snapshot {
                snapshot,
                sumeragi,
                metrics,
                metrics_error,
            }) => {
                assert_eq!(snapshot.status.queue_size, 5);
                assert!(sumeragi.is_some());
                assert!(metrics.is_none(), "metrics snapshot should be absent");
                let error = metrics_error.expect("metrics error should be reported");
                assert_eq!(error.kind, ToriiErrorKind::UnexpectedStatus);
            }
            other => panic!("expected snapshot with metrics error, got {other:?}"),
        }

        stream.abort();
        sleep(Duration::from_millis(10)).await;
        assert!(stream.is_finished());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wait_for_ready_retries_until_status_returns_ok() {
        let status = TelemetryStatus {
            queue_size: 9,
            ..TelemetryStatus::default()
        };
        let ok_body = norito::to_bytes(&status).expect("encode status");
        let Some((addr, shutdown, handle)) =
            spawn_status_stub(vec![(503, Vec::new()), (200, ok_body.clone())])
        else {
            return;
        };

        let client = ToriiClient::new(format!("http://{addr}")).expect("client");
        let options = ReadinessOptions::new(Duration::from_millis(400))
            .with_poll_interval(Duration::from_millis(20));
        let snapshot = client
            .wait_for_ready(options)
            .await
            .expect("readiness snapshot");

        assert_eq!(snapshot.status.queue_size, 9);
        let _ = shutdown.send(());
        let _ = handle.join();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wait_for_ready_times_out_when_status_never_recovers() {
        let Some((addr, shutdown, handle)) = spawn_status_stub(vec![(503, Vec::new())]) else {
            return;
        };
        let client = ToriiClient::new(format!("http://{addr}")).expect("client");
        let options = ReadinessOptions::new(Duration::from_millis(120))
            .with_poll_interval(Duration::from_millis(15));

        match client.wait_for_ready(options).await {
            Ok(_) => panic!("expected readiness error"),
            Err(ToriiError::UnexpectedStatus { status, .. }) => {
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            }
            Err(other) => panic!("unexpected readiness error: {other:?}"),
        }

        let _ = shutdown.send(());
        let _ = handle.join();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_block_stream_emits_alias_on_error() {
        let handle = tokio::runtime::Handle::current();
        let attempts = Arc::new(AtomicUsize::new(0));
        let factory = {
            let attempts = attempts.clone();
            move || {
                let attempts = attempts.clone();
                async move {
                    if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                        Err(ToriiError::UnexpectedStatus {
                            status: StatusCode::SERVICE_UNAVAILABLE,
                            reject_code: None,
                            message: None,
                        })
                    } else {
                        let (sender, _) = broadcast::channel(4);
                        let task = tokio::spawn(async {});
                        Ok(WsSubscription {
                            sender,
                            handle: task,
                        })
                    }
                }
            }
        };

        let stream = ManagedBlockStream::spawn_with_factory(&handle, "alias-error", factory);
        assert_eq!(stream.alias(), "alias-error");
        let mut receiver = stream.subscribe();

        tokio::task::yield_now().await;

        match timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("decode error event produced")
            .expect("decode error value")
        {
            BlockStreamEvent::DecodeError { .. } => {}
            other => panic!("expected decode error, got {other:?}"),
        }

        match timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("alias notice event produced")
            .expect("alias notice value")
        {
            BlockStreamEvent::Text { text } => {
                assert!(text.contains("alias-error"), "alias missing in {text}");
            }
            other => panic!("expected alias text notice, got {other:?}"),
        }

        sleep(INITIAL_BACKOFF).await;
        tokio::task::yield_now().await;

        stream.abort();
        sleep(Duration::from_millis(20)).await;
        assert!(stream.is_finished());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_block_stream_abort_stops_worker() {
        let handle = tokio::runtime::Handle::current();
        let stream = ManagedBlockStream::spawn_with_factory(&handle, "abort-peer", || async {
            let (sender, _) = broadcast::channel(1);
            let task = tokio::spawn(async {});
            Ok(WsSubscription {
                sender,
                handle: task,
            })
        });
        assert_eq!(stream.alias(), "abort-peer");

        stream.abort();
        tokio::task::yield_now().await;
        assert!(stream.is_finished());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_signed_transaction_posts_versioned_bytes() {
        let listener = match handle_bind_result(
            TcpListener::bind("127.0.0.1:0").await,
            "bind transaction listener",
        ) {
            Some(listener) => listener,
            None => return,
        };
        let addr = listener.local_addr().expect("listener address");
        let recorded = Arc::new(AsyncMutex::new(None::<(String, Vec<u8>)>));
        let server_task = {
            let recorded = Arc::clone(&recorded);
            tokio::spawn(async move {
                if let Ok((mut socket, _)) = listener.accept().await {
                    let mut header_bytes = Vec::new();
                    loop {
                        match socket.read_u8().await {
                            Ok(byte) => {
                                header_bytes.push(byte);
                                if header_bytes.ends_with(b"\r\n\r\n") {
                                    break;
                                }
                            }
                            Err(_) => return,
                        }
                    }
                    let header_str = String::from_utf8_lossy(&header_bytes);
                    let request_line = header_str.lines().next().unwrap_or_default().to_string();
                    let content_length = header_str
                        .lines()
                        .find_map(|line| {
                            let mut parts = line.splitn(2, ':');
                            let name = parts.next()?.trim().to_ascii_lowercase();
                            if name == "content-length" {
                                parts.next()?.trim().parse::<usize>().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    let mut body = vec![0u8; content_length];
                    if socket.read_exact(&mut body).await.is_err() {
                        return;
                    }
                    {
                        let mut guard = recorded.lock().await;
                        *guard = Some((request_line, body));
                    }
                    let _ = socket
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                        .await;
                }
            })
        };

        let keypair = KeyPair::random();
        let chain: ChainId = "mochi-test".parse().expect("chain id");
        let tx = TransactionBuilder::new(chain, ALICE_ID.clone())
            .with_instructions(iter::empty::<InstructionBox>())
            .sign(keypair.private_key());
        let versioned = tx.encode_versioned();

        let client = ToriiClient::new(format!("http://{addr}")).expect("client");
        client
            .submit_signed_transaction(&tx)
            .await
            .expect("submit transaction");

        server_task.await.expect("server task finished");
        let guard = recorded.lock().await;
        let (request_line, body) = guard.clone().expect("captured request");
        assert!(
            request_line.starts_with("POST /transaction"),
            "unexpected request line: {request_line}"
        );
        assert_eq!(body, versioned);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_query_decodes_response() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let query_output = QueryOutput::new(QueryOutputBatchBoxTuple::new(vec![]), 0, None);
        let encoded = norito::to_bytes(&query_output).expect("encode query output");

        let mock = server.mock(|when, then| {
            when.method(POST).path("/query");
            then.status(200).body(encoded.clone());
        });

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let signed_query = QueryRequest::Singular(SingularQueryBox::FindExecutorDataModel(
            FindExecutorDataModel,
        ))
        .with_authority(account_id)
        .sign(&keypair);

        let client = ToriiClient::new(server.url("/")).expect("client");
        let response = client
            .execute_query(&signed_query)
            .await
            .expect("decode query output");

        mock.assert();
        let (_, remaining_items, continue_cursor) = response.into_parts();
        assert_eq!(remaining_items, 0);
        assert!(continue_cursor.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_query_returns_unexpected_status() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(POST).path("/query");
            then.status(500);
        });

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let signed_query = QueryRequest::Singular(SingularQueryBox::FindExecutorDataModel(
            FindExecutorDataModel,
        ))
        .with_authority(account_id)
        .sign(&keypair);

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .execute_query(&signed_query)
            .await
            .expect_err("non-success status should error");

        mock.assert();
        match err {
            ToriiError::UnexpectedStatus { status, .. } => {
                assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            }
            other => panic!("expected UnexpectedStatus, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_query_reports_decode_error() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(POST).path("/query");
            then.status(200).body(vec![0, 1, 2, 3]);
        });

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let signed_query = QueryRequest::Singular(SingularQueryBox::FindExecutorDataModel(
            FindExecutorDataModel,
        ))
        .with_authority(account_id)
        .sign(&keypair);

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .execute_query(&signed_query)
            .await
            .expect_err("malformed payload should error");

        mock.assert();
        matches!(err, ToriiError::Decode(_));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_status_decodes_norito_payload() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let status = TelemetryStatus {
            peers: 2,
            blocks: 5,
            blocks_non_empty: 3,
            commit_time_ms: 42,
            da_reschedule_total: 0,
            txs_approved: 7,
            txs_rejected: 1,
            last_rejection_at_ms: None,
            txs_rejected_recent_5m: 0,
            uptime: Uptime(Duration::from_secs(123)),
            view_changes: 0,
            queue_size: 4,
            crypto: iroha_telemetry::metrics::CryptoStatus::default(),
            stack: iroha_telemetry::metrics::StackStatus::default(),
            sumeragi: None,
            governance: GovernanceStatus::default(),
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            tx_gossip: TxGossipSnapshot::default(),
            sorafs_micropayments: Vec::new(),
            taikai_alias_rotations: Vec::new(),
            taikai_ingest: Vec::new(),
            da_receipt_cursors: Vec::new(),
        };
        let encoded = norito::to_bytes(&status).expect("encode status");

        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/status")
                .header("accept", "application/norito");
            then.status(200)
                .header("content-type", "application/norito")
                .body(encoded.clone());
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let decoded = client.fetch_status().await.expect("status");

        mock.assert();
        assert_eq!(decoded.blocks, status.blocks);
        assert_eq!(decoded.queue_size, status.queue_size);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_status_snapshot_tracks_metrics_across_calls() {
        let listener = match handle_bind_result(
            TcpListener::bind("127.0.0.1:0").await,
            "bind status listener",
        ) {
            Some(listener) => listener,
            None => return,
        };
        let addr = listener.local_addr().expect("listener address");

        let initial = TelemetryStatus {
            peers: 2,
            blocks: 10,
            blocks_non_empty: 8,
            commit_time_ms: 45,
            da_reschedule_total: 2,
            txs_approved: 5,
            txs_rejected: 1,
            last_rejection_at_ms: None,
            txs_rejected_recent_5m: 0,
            uptime: Uptime(Duration::from_secs(5)),
            view_changes: 0,
            queue_size: 4,
            crypto: iroha_telemetry::metrics::CryptoStatus::default(),
            stack: iroha_telemetry::metrics::StackStatus::default(),
            sumeragi: None,
            governance: GovernanceStatus::default(),
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            tx_gossip: TxGossipSnapshot::default(),
            sorafs_micropayments: Vec::new(),
            taikai_alias_rotations: Vec::new(),
            taikai_ingest: Vec::new(),
            da_receipt_cursors: Vec::new(),
        };
        let updated = TelemetryStatus {
            peers: 3,
            blocks: 11,
            blocks_non_empty: 9,
            commit_time_ms: 120,
            da_reschedule_total: 5,
            txs_approved: 9,
            txs_rejected: 3,
            last_rejection_at_ms: Some(7_000),
            txs_rejected_recent_5m: 3,
            uptime: Uptime(Duration::from_secs(7)),
            view_changes: 2,
            queue_size: 9,
            crypto: iroha_telemetry::metrics::CryptoStatus::default(),
            stack: iroha_telemetry::metrics::StackStatus::default(),
            sumeragi: None,
            governance: GovernanceStatus::default(),
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            tx_gossip: TxGossipSnapshot::default(),
            sorafs_micropayments: Vec::new(),
            taikai_alias_rotations: Vec::new(),
            taikai_ingest: Vec::new(),
            da_receipt_cursors: Vec::new(),
        };

        let responses = vec![
            norito::to_bytes(&initial).expect("encode initial status"),
            norito::to_bytes(&updated).expect("encode updated status"),
        ];

        let server_task = tokio::spawn(async move {
            for payload in responses {
                if let Ok((mut socket, _)) = listener.accept().await {
                    let mut header_bytes = Vec::new();
                    loop {
                        match socket.read_u8().await {
                            Ok(byte) => {
                                header_bytes.push(byte);
                                if header_bytes.ends_with(b"\r\n\r\n") {
                                    break;
                                }
                            }
                            Err(_) => return,
                        }
                    }
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/norito\r\nConnection: close\r\n\r\n",
                        payload.len()
                    )
                    .into_bytes();
                    if socket.write_all(&response).await.is_err() {
                        return;
                    }
                    if socket.write_all(&payload).await.is_err() {
                        return;
                    }
                }
            }
        });

        let client = ToriiClient::new(format!("http://{addr}")).expect("client");
        let first = client
            .fetch_status_snapshot()
            .await
            .expect("first snapshot");
        assert_eq!(first.status.queue_size, initial.queue_size);
        assert_eq!(first.metrics.queue_delta, 0);
        assert_eq!(first.metrics.da_reschedule_delta, 0);
        assert_eq!(first.metrics.tx_approved_delta, 0);
        assert_eq!(first.metrics.tx_rejected_delta, 0);
        assert_eq!(first.metrics.view_change_delta, 0);

        let second = client
            .fetch_status_snapshot()
            .await
            .expect("second snapshot");
        assert_eq!(second.status.queue_size, updated.queue_size);
        assert_eq!(
            second.metrics.queue_delta,
            updated.queue_size as i64 - initial.queue_size as i64
        );
        assert_eq!(
            second.metrics.da_reschedule_delta,
            updated.da_reschedule_total - initial.da_reschedule_total
        );
        assert_eq!(
            second.metrics.tx_approved_delta,
            updated.txs_approved - initial.txs_approved
        );
        assert_eq!(
            second.metrics.tx_rejected_delta,
            updated.txs_rejected - initial.txs_rejected
        );
        assert_eq!(
            second.metrics.view_change_delta,
            updated.view_changes - initial.view_changes
        );

        server_task.await.expect("server task finished");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_status_reports_decode_error_for_invalid_payload() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/status")
                .header("accept", "application/norito");
            then.status(200).body(vec![0, 1, 2, 3, 4]);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .fetch_status()
            .await
            .expect_err("invalid payload should fail");

        mock.assert();
        matches!(err, ToriiError::Decode(_));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_sumeragi_status_decodes_payload() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let status = sample_sumeragi_status_wire();
        let encoded = norito::to_bytes(&status).expect("encode status");

        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/sumeragi/status")
                .header("accept", "application/norito");
            then.status(200)
                .header("content-type", "application/norito")
                .body(encoded.clone());
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let decoded = client
            .fetch_sumeragi_status()
            .await
            .expect("status payload");

        mock.assert();
        assert_eq!(decoded.leader_index, status.leader_index);
        assert_eq!(decoded.membership.height, status.membership.height);
        assert_eq!(decoded.membership.view_hash, status.membership.view_hash);
        assert_eq!(decoded.lane_governance, status.lane_governance);
        assert_eq!(decoded.lane_commitments, status.lane_commitments);
        assert_eq!(decoded.dataspace_commitments, status.dataspace_commitments);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_sumeragi_status_reports_unexpected_status() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET).path("/v1/sumeragi/status");
            then.status(503);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .fetch_sumeragi_status()
            .await
            .expect_err("non-success status should error");

        mock.assert();
        match err {
            ToriiError::UnexpectedStatus { status, .. } => {
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            }
            other => panic!("expected UnexpectedStatus, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_status_returns_unexpected_status_on_non_success() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/status")
                .header("accept", "application/norito");
            then.status(502);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .fetch_status()
            .await
            .expect_err("non-success response should error");

        mock.assert();
        match err {
            ToriiError::UnexpectedStatus { status, .. } => {
                assert_eq!(status, StatusCode::BAD_GATEWAY);
            }
            other => panic!("expected UnexpectedStatus, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_configuration_reports_decode_error() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET).path("/configuration");
            then.status(200)
                .header("content-type", "application/json")
                .body(&b"{not-json}"[..]);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .fetch_configuration()
            .await
            .expect_err("malformed json should error");

        mock.assert();
        matches!(err, ToriiError::Decode(_));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_metrics_returns_unexpected_status_on_error() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET).path("/metrics");
            then.status(503);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let err = client
            .fetch_metrics()
            .await
            .expect_err("non-success response should error");

        mock.assert();
        match err {
            ToriiError::UnexpectedStatus { status, .. } => {
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            }
            other => panic!("expected UnexpectedStatus, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_configuration_returns_json() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET).path("/configuration");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"chain_id":"mochi"}"#);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let value = client.fetch_configuration().await.expect("config");

        mock.assert();
        assert_eq!(value["chain_id"].as_str(), Some("mochi"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn apply_lane_lifecycle_returns_json() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/nexus/lifecycle")
                .header("content-type", "application/json");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"ok":true}"#);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let plan = LaneLifecyclePlan::default();
        let value = client
            .apply_lane_lifecycle(&plan)
            .await
            .expect("lifecycle response");

        mock.assert();
        assert_eq!(value["ok"].as_bool(), Some(true));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn apply_lane_lifecycle_reports_unexpected_status() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/nexus/lifecycle");
            then.status(503);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let plan = LaneLifecyclePlan::default();
        let err = client
            .apply_lane_lifecycle(&plan)
            .await
            .expect_err("non-success response should error");

        mock.assert();
        match err {
            ToriiError::UnexpectedStatus { status, .. } => {
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            }
            other => panic!("expected UnexpectedStatus, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_metrics_returns_text() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let metrics_body = "queue_size 3";
        let mock = server.mock(|when, then| {
            when.method(GET).path("/metrics");
            then.status(200).body(metrics_body);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let body = client.fetch_metrics().await.expect("metrics");

        mock.assert();
        assert_eq!(body, metrics_body);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_metrics_snapshot_parses_values() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let metrics_body = "\
queue_size 4
view_changes 2
sumeragi_tx_queue_depth 5
state_tiered_hot_entries 10
";
        let mock = server.mock(|when, then| {
            when.method(GET).path("/metrics");
            then.status(200).body(metrics_body);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let snapshot = client
            .fetch_metrics_snapshot()
            .await
            .expect("metrics snapshot");

        mock.assert();
        assert_eq!(snapshot.queue_size, Some(4.0));
        assert_eq!(snapshot.view_changes, Some(2.0));
        assert_eq!(snapshot.sumeragi_tx_queue_depth, Some(5.0));
        assert_eq!(snapshot.state_tiered_hot_entries, Some(10.0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_block_parses_payload() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let body = r#"{
  "hash":"aa00bb11",
  "height":7,
  "created_at":"2026-01-01T00:00:00Z",
  "prev_block_hash":"cc22dd33",
  "transactions_hash":"ee44ff55",
  "transactions_rejected":1,
  "transactions_total":2
}"#;
        let mock = server.mock(|when, then| {
            when.method(GET).path("/v1/blocks/7");
            then.status(200)
                .header("content-type", "application/json")
                .body(body);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let record = client
            .fetch_block(7)
            .await
            .expect("request")
            .expect("record");

        mock.assert();
        assert_eq!(record.height, 7);
        assert_eq!(record.prev_block_hash.as_deref(), Some("cc22dd33"));
        assert_eq!(record.transactions_total, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_block_returns_none_on_404() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let mock = server.mock(|when, then| {
            when.method(GET).path("/v1/blocks/99");
            then.status(404);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let record = client.fetch_block(99).await.expect("request");

        mock.assert();
        assert!(record.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_blocks_page_supports_query_params() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let body = r#"{
  "pagination":{"page":1,"per_page":2,"total_pages":2,"total_items":4},
  "items":[
    {
      "hash":"aa00bb11",
      "height":5,
      "created_at":"2026-01-01T00:00:00Z",
      "transactions_rejected":0,
      "transactions_total":1
    },
    {
      "block_hash":"cc22dd33",
      "height":"6",
      "createdAt":"2026-01-01T01:00:00Z",
      "transactionsRejected":"1",
      "transactionsTotal":"3"
    }
  ]
}"#;
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/blocks")
                .query_param("offset_height", "5")
                .query_param("limit", "2");
            then.status(200)
                .header("content-type", "application/json")
                .body(body);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let page = client
            .fetch_blocks_page(ExplorerBlocksQuery {
                offset_height: Some(5),
                limit: Some(2),
            })
            .await
            .expect("page");

        mock.assert();
        assert_eq!(page.pagination.per_page, 2);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].hash, "aa00bb11");
        assert_eq!(page.items[1].hash, "cc22dd33");
        assert_eq!(page.items[1].transactions_rejected, 1);
    }

    #[test]
    fn metrics_parser_ignores_comments_and_labels() {
        let body = r#"
# HELP queue_size number of txs
queue_size 3
view_changes 1
accounts{domain="wonderland"} 10
sumeragi_tx_queue_capacity 64
state_tiered_cold_entries 2
"#;
        let now = Instant::now();
        let snapshot = ToriiMetricsSnapshot::from_prometheus(now, body);
        assert_eq!(snapshot.queue_size, Some(3.0));
        assert_eq!(snapshot.view_changes, Some(1.0));
        assert_eq!(snapshot.sumeragi_tx_queue_capacity, Some(64.0));
        assert_eq!(snapshot.state_tiered_cold_entries, Some(2.0));
        assert!(snapshot.state_tiered_hot_entries.is_none());
    }

    #[test]
    fn status_metrics_report_activity_deltas() {
        let previous = TelemetryStatus {
            queue_size: 4,
            txs_approved: 1,
            txs_rejected: 0,
            da_reschedule_total: 2,
            view_changes: 3,
            blocks: 10,
            blocks_non_empty: 9,
            ..TelemetryStatus::default()
        };
        let current = TelemetryStatus {
            commit_time_ms: 25,
            queue_size: 7,
            txs_approved: 4,
            txs_rejected: 2,
            da_reschedule_total: 5,
            view_changes: 4,
            blocks: 12,
            blocks_non_empty: 11,
            ..TelemetryStatus::default()
        };

        let metrics = StatusMetrics::from_samples(Some(&previous), &current);
        assert_eq!(metrics.commit_latency_ms, 25);
        assert_eq!(metrics.queue_delta, 3);
        assert_eq!(metrics.tx_approved_delta, 3);
        assert_eq!(metrics.tx_rejected_delta, 2);
        assert_eq!(metrics.da_reschedule_delta, 3);
        assert_eq!(metrics.view_change_delta, 1);
        assert_eq!(metrics.block_delta, 2);
        assert_eq!(metrics.blocks_non_empty_delta, 2);
        assert_eq!(metrics.sample_interval_ms, 0);
        assert!(metrics.has_activity());
    }

    #[test]
    fn status_metrics_report_idle_when_snapshots_match() {
        let snapshot = TelemetryStatus {
            queue_size: 2,
            txs_approved: 5,
            txs_rejected: 1,
            da_reschedule_total: 1,
            view_changes: 0,
            ..TelemetryStatus::default()
        };

        let metrics = StatusMetrics::from_samples(Some(&snapshot), &snapshot);
        assert_eq!(metrics.commit_latency_ms, snapshot.commit_time_ms);
        assert_eq!(metrics.queue_delta, 0);
        assert_eq!(metrics.tx_approved_delta, 0);
        assert_eq!(metrics.tx_rejected_delta, 0);
        assert_eq!(metrics.da_reschedule_delta, 0);
        assert_eq!(metrics.view_change_delta, 0);
        assert_eq!(metrics.block_delta, 0);
        assert_eq!(metrics.blocks_non_empty_delta, 0);
        assert_eq!(metrics.sample_interval_ms, 0);
        assert!(!metrics.has_activity());
    }

    #[test]
    fn status_state_records_sample_interval_and_block_delta() {
        let mut state = StatusState::default();
        let now = Instant::now();
        let first = TelemetryStatus {
            blocks: 1,
            ..TelemetryStatus::default()
        };
        let first_metrics = state.record(now, &first);
        assert_eq!(first_metrics.block_delta, 0);
        assert_eq!(first_metrics.sample_interval_ms, 0);

        let later = now + Duration::from_millis(150);
        let second = TelemetryStatus {
            blocks: 3,
            blocks_non_empty: 2,
            ..TelemetryStatus::default()
        };
        let second_metrics = state.record(later, &second);
        assert_eq!(second_metrics.block_delta, 2);
        assert_eq!(second_metrics.blocks_non_empty_delta, 2);
        assert_eq!(second_metrics.sample_interval_ms, 150);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn status_monitor_streams_snapshots() {
        let counter = Arc::new(AtomicUsize::new(0));
        let monitor = ToriiStatusMonitor::spawn(Duration::from_millis(5), {
            let counter = Arc::clone(&counter);
            move || {
                let counter = Arc::clone(&counter);
                async move {
                    let value = counter.fetch_add(1, Ordering::SeqCst) as u64 + 1;
                    let status = TelemetryStatus {
                        queue_size: value,
                        ..TelemetryStatus::default()
                    };
                    let metrics = StatusMetrics::from_samples(None, &status);
                    Ok(ToriiStatusSnapshot::new(Instant::now(), status, metrics))
                }
            }
        });

        let mut receiver = monitor.subscribe();
        timeout(Duration::from_secs(1), receiver.changed())
            .await
            .expect("monitor emitted value")
            .expect("channel open");
        let state = receiver.borrow().clone();
        assert!(state.has_snapshot(), "monitor should publish snapshots");
        assert!(
            state.last_success_at.is_some(),
            "monitor should record last success timestamp"
        );
        assert_eq!(
            state
                .last_snapshot
                .as_ref()
                .expect("snapshot available")
                .status
                .queue_size,
            1
        );
        assert!(state.last_error.is_none());

        monitor.stop();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn status_monitor_records_errors_and_clears_on_success() {
        let step = Arc::new(AtomicUsize::new(0));
        let monitor = ToriiStatusMonitor::spawn(Duration::from_millis(5), {
            let step = Arc::clone(&step);
            move || {
                let step = Arc::clone(&step);
                async move {
                    let attempt = step.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        Err(ToriiError::UnexpectedStatus {
                            status: StatusCode::SERVICE_UNAVAILABLE,
                            reject_code: None,
                            message: None,
                        })
                    } else {
                        let status = TelemetryStatus {
                            queue_size: attempt as u64,
                            ..TelemetryStatus::default()
                        };
                        let metrics = StatusMetrics::from_samples(None, &status);
                        Ok(ToriiStatusSnapshot::new(Instant::now(), status, metrics))
                    }
                }
            }
        });

        let mut receiver = monitor.subscribe();
        timeout(Duration::from_secs(1), receiver.changed())
            .await
            .expect("first update")
            .expect("channel open");
        let first = receiver.borrow().clone();
        assert!(first.last_error.is_some());
        assert!(!first.has_snapshot());

        timeout(Duration::from_secs(1), receiver.changed())
            .await
            .expect("second update")
            .expect("channel open");
        let second = receiver.borrow().clone();
        assert!(second.last_error.is_none());
        assert_eq!(
            second
                .last_snapshot
                .as_ref()
                .expect("snapshot available")
                .status
                .queue_size,
            1
        );

        monitor.stop();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn status_monitor_tracks_last_success_and_exposes_age() {
        let step = Arc::new(AtomicUsize::new(0));
        let monitor = ToriiStatusMonitor::spawn(Duration::from_millis(5), {
            let step = Arc::clone(&step);
            move || {
                let step = Arc::clone(&step);
                async move {
                    let attempt = step.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        let status = TelemetryStatus {
                            queue_size: 7,
                            ..TelemetryStatus::default()
                        };
                        let metrics = StatusMetrics::from_samples(None, &status);
                        Ok(ToriiStatusSnapshot::new(Instant::now(), status, metrics))
                    } else {
                        Err(ToriiError::UnexpectedStatus {
                            status: StatusCode::BAD_GATEWAY,
                            reject_code: None,
                            message: None,
                        })
                    }
                }
            }
        });

        let mut receiver = monitor.subscribe();
        timeout(Duration::from_secs(1), receiver.changed())
            .await
            .expect("first update")
            .expect("channel open");
        let first = receiver.borrow().clone();
        let first_timestamp = first
            .last_success_at
            .expect("first poll should record success timestamp");
        let first_age = first
            .last_success_age()
            .expect("first poll should expose success age");
        assert!(
            first_age >= Duration::ZERO,
            "age should be present for successful poll"
        );
        assert!(first.last_error.is_none(), "first poll should succeed");

        timeout(Duration::from_secs(1), receiver.changed())
            .await
            .expect("second update")
            .expect("channel open");
        let second = receiver.borrow().clone();
        assert_eq!(
            second.last_success_at,
            Some(first_timestamp),
            "error should not clear last success timestamp"
        );
        assert_eq!(
            second.consecutive_failures, 1,
            "first error should increment failure counter"
        );
        assert!(
            second.last_success_age().is_some(),
            "age should remain available after errors"
        );
        assert!(second.last_error.is_some(), "second poll should fail");

        monitor.stop();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn metrics_monitor_streams_snapshots() {
        let counter = Arc::new(AtomicUsize::new(0));
        let monitor = ToriiMetricsMonitor::spawn(Duration::from_millis(5), {
            let counter = Arc::clone(&counter);
            move || {
                let counter = Arc::clone(&counter);
                async move {
                    let value = counter.fetch_add(1, Ordering::SeqCst) as f64 + 1.0;
                    let snapshot = ToriiMetricsSnapshot::from_prometheus(
                        Instant::now(),
                        &format!("queue_size {value}\n"),
                    );
                    Ok(snapshot)
                }
            }
        });

        let mut receiver = monitor.subscribe();
        timeout(Duration::from_secs(1), receiver.changed())
            .await
            .expect("monitor emitted value")
            .expect("channel open");
        let state = receiver.borrow().clone();
        assert!(state.has_snapshot(), "monitor should publish snapshots");
        assert!(
            state.last_success_at.is_some(),
            "monitor should record last success timestamp"
        );
        assert!(
            matches!(
                state
                    .last_snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.queue_size),
                Some(value) if (value - 1.0).abs() < f64::EPSILON
            ),
            "monitor should retain queue gauge"
        );
        assert!(state.last_error.is_none());

        monitor.stop();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn metrics_monitor_records_errors_and_clears_on_success() {
        let step = Arc::new(AtomicUsize::new(0));
        let monitor = ToriiMetricsMonitor::spawn(Duration::from_millis(5), {
            let step = Arc::clone(&step);
            move || {
                let step = Arc::clone(&step);
                async move {
                    let attempt = step.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        Err(ToriiError::UnexpectedStatus {
                            status: StatusCode::GATEWAY_TIMEOUT,
                            reject_code: None,
                            message: None,
                        })
                    } else {
                        Ok(ToriiMetricsSnapshot::from_prometheus(
                            Instant::now(),
                            "queue_size 4\n",
                        ))
                    }
                }
            }
        });

        let mut receiver = monitor.subscribe();
        timeout(Duration::from_secs(1), receiver.changed())
            .await
            .expect("first update")
            .expect("channel open");
        let first = receiver.borrow().clone();
        assert!(first.last_error.is_some());
        assert!(!first.has_snapshot());

        timeout(Duration::from_secs(1), receiver.changed())
            .await
            .expect("second update")
            .expect("channel open");
        let second = receiver.borrow().clone();
        assert!(second.last_error.is_none());
        assert!(
            matches!(
                second
                    .last_snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.queue_size),
                Some(value) if (value - 4.0).abs() < f64::EPSILON
            ),
            "successful poll should publish snapshot"
        );

        monitor.stop();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn metrics_monitor_tracks_last_success_and_exposes_age() {
        let step = Arc::new(AtomicUsize::new(0));
        let monitor = ToriiMetricsMonitor::spawn(Duration::from_millis(5), {
            let step = Arc::clone(&step);
            move || {
                let step = Arc::clone(&step);
                async move {
                    let attempt = step.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        Ok(ToriiMetricsSnapshot::from_prometheus(
                            Instant::now(),
                            "queue_size 2\n",
                        ))
                    } else {
                        Err(ToriiError::UnexpectedStatus {
                            status: StatusCode::BAD_GATEWAY,
                            reject_code: None,
                            message: None,
                        })
                    }
                }
            }
        });

        let mut receiver = monitor.subscribe();
        timeout(Duration::from_secs(1), receiver.changed())
            .await
            .expect("first update")
            .expect("channel open");
        let first = receiver.borrow().clone();
        let first_timestamp = first
            .last_success_at
            .expect("poll should record success timestamp");
        assert!(
            first.last_success_age().is_some(),
            "successful poll should expose age helper"
        );
        assert!(first.last_error.is_none());

        timeout(Duration::from_secs(1), receiver.changed())
            .await
            .expect("second update")
            .expect("channel open");
        let second = receiver.borrow().clone();
        assert!(
            second.last_success_at == Some(first_timestamp),
            "failed poll should retain last_success_at"
        );
        assert!(
            second.last_error.is_some(),
            "failed poll should surface the error"
        );

        monitor.stop();
    }

    #[test]
    fn metrics_snapshot_queue_utilization_reports_ratio() {
        let mut snapshot = empty_metrics_snapshot();
        snapshot.sumeragi_tx_queue_depth = Some(5.0);
        snapshot.sumeragi_tx_queue_capacity = Some(10.0);
        assert_eq!(
            snapshot.queue_utilization(),
            Some(0.5),
            "expected half-full queue to report 0.5 utilisation"
        );
        snapshot.sumeragi_tx_queue_depth = Some(12.0);
        assert_eq!(
            snapshot.queue_utilization(),
            Some(1.0),
            "ratio should clamp when depth exceeds capacity"
        );
        snapshot.sumeragi_tx_queue_capacity = Some(0.0);
        assert!(
            snapshot.queue_utilization().is_none(),
            "zero capacity should skip utilisation computation"
        );
    }

    #[test]
    fn metrics_snapshot_queue_saturation_interprets_flags() {
        let mut snapshot = empty_metrics_snapshot();
        snapshot.sumeragi_tx_queue_saturated = Some(0.0);
        assert_eq!(snapshot.queue_saturation_flag(), Some(false));
        snapshot.sumeragi_tx_queue_saturated = Some(1.0);
        assert_eq!(snapshot.queue_saturation_flag(), Some(true));
        snapshot.sumeragi_tx_queue_saturated = Some(0.5);
        assert!(
            snapshot.queue_saturation_flag().is_none(),
            "non-binary values should bubble up as indeterminate"
        );
    }

    #[test]
    fn metrics_snapshot_cold_entry_ratio_handles_missing_totals() {
        let mut snapshot = empty_metrics_snapshot();
        snapshot.state_tiered_hot_entries = Some(75.0);
        snapshot.state_tiered_cold_entries = Some(25.0);
        assert_eq!(
            snapshot.cold_entry_ratio(),
            Some(0.25),
            "cold tier share should reflect proportional occupancy"
        );
        snapshot.state_tiered_hot_entries = Some(0.0);
        snapshot.state_tiered_cold_entries = Some(0.0);
        assert!(
            snapshot.cold_entry_ratio().is_none(),
            "zero totals should skip ratio computation"
        );
    }

    #[test]
    fn explorer_block_record_parses_camel_case_fields() {
        let value = norito::json!({
            "blockHash":"1122aabb",
            "height":"9",
            "createdAt":"2026-02-10T00:00:00Z",
            "transactionsRejected":"0",
            "transactionsTotal":"3"
        });
        let record = ExplorerBlockRecord::from_json(&value).expect("record");
        assert_eq!(record.hash, "1122aabb");
        assert_eq!(record.height, 9);
        assert!(record.prev_block_hash.is_none());
        assert_eq!(record.transactions_total, 3);
    }

    #[test]
    fn explorer_blocks_page_errors_when_items_invalid() {
        let value = norito::json!({
            "pagination":{"page":1,"per_page":1,"total_pages":0,"total_items":0},
            "items":{"block_hash":"aa"}
        });
        let err = ExplorerBlocksPage::from_json(&value).expect_err("expected failure");
        assert!(matches!(err, ToriiError::Decode(_)));
    }

    #[test]
    fn explorer_account_record_decodes_payload() {
        let value = norito::json!({
            "id": "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE",
            "i105_address": "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE",
            "network_prefix": 42,
            "metadata": { "role": "admin" },
            "owned_domains": 2,
            "owned_assets": 5,
            "owned_nfts": 1
        });
        let record = ExplorerAccountRecord::from_json(&value).expect("record");
        assert_eq!(
            record.id,
            "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE"
        );
        assert_eq!(
            record.i105_address,
            "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE"
        );
        assert_eq!(record.network_prefix, 42);
        assert_eq!(record.metadata, norito::json!({ "role": "admin" }));
        assert_eq!(record.owned_domains, 2);
        assert_eq!(record.owned_assets, 5);
        assert_eq!(record.owned_nfts, 1);
    }

    #[test]
    fn explorer_accounts_page_decodes_entries() {
        let value = norito::json!({
            "pagination": {
                "page": 1,
                "per_page": 10,
                "total_pages": 1,
                "total_items": 1
            },
            "items": [
                {
                    "id": "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE",
                    "i105_address": "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE",
                    "network_prefix": 1,
                    "metadata": {},
                    "owned_domains": 0,
                    "owned_assets": 0,
                    "owned_nfts": 0
                }
            ]
        });
        let page = ExplorerAccountsPage::from_json(&value).expect("page");
        assert_eq!(page.pagination.page, 1);
        assert_eq!(page.pagination.per_page, 10);
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.items[0].id,
            "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE"
        );
    }

    #[test]
    fn explorer_domain_record_decodes_payload() {
        let value = norito::json!({
            "id": "sora",
            "logo": "https://example/logo.svg",
            "metadata": { "tier": "p0" },
            "owned_by": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
            "accounts": 5,
            "assets": 3,
            "nfts": 1
        });
        let record = ExplorerDomainRecord::from_json(&value).expect("record");
        assert_eq!(record.id, "sora");
        assert_eq!(record.logo.as_deref(), Some("https://example/logo.svg"));
        assert_eq!(
            record.owned_by,
            "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
        );
        assert_eq!(record.accounts, 5);
        assert_eq!(record.assets, 3);
        assert_eq!(record.nfts, 1);
    }

    #[test]
    fn explorer_domains_page_validates_entries() {
        let value = norito::json!({
            "pagination":{"page":1,"per_page":10,"total_pages":1,"total_items":1},
            "items":[{ "id":"sora","owned_by":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D","accounts":1,"assets":0,"nfts":0 }]
        });
        let page = ExplorerDomainsPage::from_json(&value).expect("page");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "sora");
    }

    #[test]
    fn explorer_asset_definition_record_decodes_payload() {
        let value = norito::json!({
            "id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
            "mintable": "Infinitely",
            "logo": null,
            "metadata": { "decimals": 2 },
            "owned_by": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
            "assets": 10
        });
        let record = ExplorerAssetDefinitionRecord::from_json(&value).expect("record");
        assert_eq!(record.id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
        assert_eq!(record.mintable, "Infinitely");
        assert_eq!(record.assets, 10);
        assert_eq!(
            record.owned_by,
            "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
        );
    }

    #[test]
    fn explorer_asset_definition_page_validates_entries() {
        let value = norito::json!({
            "pagination":{"page":1,"per_page":5,"total_pages":1,"total_items":1},
            "items":[
                {"id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","mintable":"Infinitely","owned_by":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D","assets":2}
            ]
        });
        let page = ExplorerAssetDefinitionsPage::from_json(&value).expect("page");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    }

    #[test]
    fn explorer_asset_record_decodes_payload() {
        let value = norito::json!({
            "id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
            "definition_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
            "account_id": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
            "value": "10.0"
        });
        let record = ExplorerAssetRecord::from_json(&value).expect("record");
        assert_eq!(record.id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
        assert_eq!(
            record.account_id,
            "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
        );
        assert_eq!(record.value, "10.0");
    }

    #[test]
    fn explorer_assets_page_validates_entries() {
        let value = norito::json!({
            "pagination":{"page":1,"per_page":10,"total_pages":1,"total_items":1},
            "items":[
                {"id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","definition_id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","account_id":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D","value":"10"}
            ]
        });
        let page = ExplorerAssetsPage::from_json(&value).expect("page");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].definition_id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    }

    #[test]
    fn explorer_nft_record_decodes_payload() {
        let value = norito::json!({
            "id": "art#gallery",
            "owned_by": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
            "metadata": { "uri": "ipfs://cid" }
        });
        let record = ExplorerNftRecord::from_json(&value).expect("record");
        assert_eq!(record.id, "art#gallery");
        assert_eq!(
            record.owned_by,
            "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
        );
        assert_eq!(record.metadata, norito::json!({ "uri": "ipfs://cid" }));
    }

    #[test]
    fn explorer_nfts_page_validates_entries() {
        let value = norito::json!({
            "pagination":{"page":1,"per_page":1,"total_pages":1,"total_items":1},
            "items":[{"id":"art#gallery","owned_by":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D","metadata":{}}]
        });
        let page = ExplorerNftsPage::from_json(&value).expect("page");
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.items[0].owned_by,
            "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_explorer_accounts_page_applies_filters() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let body = norito::json!({
            "pagination": {
                "page": 2,
                "per_page": 25,
                "total_pages": 3,
                "total_items": 60
            },
            "items": [
                {
                    "id": "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE",
                    "i105_address": "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE",
                    "network_prefix": 1,
                    "metadata": { "owned_assets": 4 },
                    "owned_domains": 0,
                    "owned_assets": 4,
                    "owned_nfts": 0
                }
            ]
        });
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/explorer/accounts")
                .query_param("page", "2")
                .query_param("per_page", "25")
                .query_param("domain", "sora")
                .query_param("with_asset", "usd#sora");
            then.status(200)
                .body(norito::json::to_string(&body).expect("serialize mock body"));
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let page = client
            .fetch_explorer_accounts_page(ExplorerAccountsQuery {
                page: Some(2),
                per_page: Some(25),
                domain: Some("sora".into()),
                with_asset: Some("usd#sora".into()),
            })
            .await
            .expect("page");

        mock.assert();
        assert_eq!(page.pagination.page, 2);
        assert_eq!(page.pagination.per_page, 25);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].owned_assets, 4);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_explorer_domains_page_applies_filters() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let body = norito::json!({
            "pagination":{"page":1,"per_page":10,"total_pages":1,"total_items":1},
            "items":[{"id":"sora","owned_by":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D","accounts":1,"assets":0,"nfts":0}]
        });
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/explorer/domains")
                .query_param("page", "1")
                .query_param("per_page", "10")
                .query_param(
                    "owned_by",
                    "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                );
            then.status(200)
                .body(norito::json::to_string(&body).expect("serialize"));
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let page = client
            .fetch_explorer_domains_page(ExplorerDomainsQuery {
                page: Some(1),
                per_page: Some(10),
                owned_by: Some(
                    "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D".into(),
                ),
            })
            .await
            .expect("page");

        mock.assert();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "sora");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_explorer_asset_definitions_page_applies_filters() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let body = norito::json!({
            "pagination":{"page":2,"per_page":5,"total_pages":3,"total_items":12},
            "items":[{"id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","mintable":"Infinitely","owned_by":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D","assets":7}]
        });
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/explorer/asset-definitions")
                .query_param("page", "2")
                .query_param("per_page", "5")
                .query_param("domain", "sora")
                .query_param(
                    "owned_by",
                    "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                );
            then.status(200)
                .body(norito::json::to_string(&body).expect("serialize"));
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let page = client
            .fetch_explorer_asset_definitions_page(ExplorerAssetDefinitionsQuery {
                page: Some(2),
                per_page: Some(5),
                domain: Some("sora".into()),
                owned_by: Some(
                    "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D".into(),
                ),
            })
            .await
            .expect("page");
        mock.assert();
        assert_eq!(page.pagination.page, 2);
        assert_eq!(page.items[0].id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_explorer_assets_page_applies_filters() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let body = norito::json!({
            "pagination":{"page":1,"per_page":50,"total_pages":1,"total_items":1},
            "items":[{"id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","definition_id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","account_id":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D","value":"10"}]
        });
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/explorer/assets")
                .query_param("page", "1")
                .query_param("per_page", "50")
                .query_param(
                    "owned_by",
                    "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                )
                .query_param("definition", "usd#sora");
            then.status(200)
                .body(norito::json::to_string(&body).expect("serialize"));
        });
        let client = ToriiClient::new(server.url("/")).expect("client");
        let page = client
            .fetch_explorer_assets_page(ExplorerAssetsQuery {
                page: Some(1),
                per_page: Some(50),
                owned_by: Some(
                    "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D".into(),
                ),
                definition: Some("usd#sora".into()),
            })
            .await
            .expect("page");
        mock.assert();
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.items[0].account_id,
            "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fetch_explorer_nfts_page_applies_filters() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let body = norito::json!({
            "pagination":{"page":3,"per_page":5,"total_pages":6,"total_items":25},
            "items":[{"id":"art#gallery","owned_by":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D","metadata":{}}]
        });
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/explorer/nfts")
                .query_param("page", "3")
                .query_param("per_page", "5")
                .query_param(
                    "owned_by",
                    "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                )
                .query_param("domain", "gallery");
            then.status(200)
                .body(norito::json::to_string(&body).expect("serialize"));
        });
        let client = ToriiClient::new(server.url("/")).expect("client");
        let page = client
            .fetch_explorer_nfts_page(ExplorerNftsQuery {
                page: Some(3),
                per_page: Some(5),
                owned_by: Some(
                    "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D".into(),
                ),
                domain: Some("gallery".into()),
            })
            .await
            .expect("page");
        mock.assert();
        assert_eq!(page.pagination.page, 3);
        assert_eq!(page.items[0].id, "art#gallery");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_triggers_parses_results() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let payload = norito::json!({
            "items": [
                {
                    "id": "daily-airdrop",
                    "action": { "Mint": { "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "account_id": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", "value": "5" } },
                    "metadata": { "cron": "0 0 * * *" }
                }
            ],
            "total": 7
        });
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/triggers")
                .query_param("namespace", "core")
                .query_param(
                    "authority",
                    "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                )
                .query_param("limit", "5")
                .query_param("offset", "10");
            then.status(200)
                .body(norito::json::to_string(&payload).expect("serialize payload"));
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let page = client
            .list_triggers(TriggerListQuery {
                namespace: Some(" core ".into()),
                authority: Some(
                    "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D".into(),
                ),
                limit: Some(5),
                offset: Some(10),
            })
            .await
            .expect("page");

        mock.assert();
        assert_eq!(page.total, 7);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "daily-airdrop");
        assert_eq!(
            page.items[0].metadata,
            norito::json!({ "cron": "0 0 * * *" })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn get_trigger_supports_missing() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let not_found = server.mock(|when, then| {
            when.method(GET).path("/v1/triggers/missing");
            then.status(404);
        });
        let body = norito::json!({
            "id": "mint-hook",
            "action": { "Register": { "Account": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D" } },
            "metadata": {}
        });
        let found = server.mock(|when, then| {
            when.method(GET).path("/v1/triggers/mint-hook");
            then.status(200)
                .body(norito::json::to_string(&body).expect("serialize body"));
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        assert!(
            client
                .get_trigger("missing")
                .await
                .expect("request")
                .is_none()
        );
        not_found.assert();

        let record = client
            .get_trigger("mint-hook")
            .await
            .expect("request")
            .expect("record");
        found.assert();
        assert_eq!(record.id, "mint-hook");
        assert_eq!(record.action, body.get("action").unwrap().clone());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn register_trigger_posts_json() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let request = norito::json!({
            "id": "hook",
            "action": { "Mint": { "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "account_id": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", "value": "42" } },
            "metadata": { "note": "demo" }
        });
        let response = request.clone();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/triggers")
                .header("content-type", "application/json")
                .body(norito::json::to_string(&request).expect("serialize request"));
            then.status(200)
                .body(norito::json::to_string(&response).expect("serialize payload"));
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        let record = client.register_trigger(&request).await.expect("request");

        mock.assert();
        assert_eq!(record.id, "hook");
        assert_eq!(record.action, response.get("action").unwrap().clone());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_trigger_reports_outcome() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let deleted = server.mock(|when, then| {
            when.method(DELETE).path("/v1/triggers/hook");
            then.status(204);
        });
        let missing = server.mock(|when, then| {
            when.method(DELETE).path("/v1/triggers/missing");
            then.status(404);
        });

        let client = ToriiClient::new(server.url("/")).expect("client");
        assert!(client.delete_trigger("hook").await.expect("delete"));
        deleted.assert();
        assert!(!client.delete_trigger("missing").await.expect("delete"));
        missing.assert();
    }

    #[test]
    fn account_deleted_summary_mentions_account_id() {
        let event_box =
            EventBox::Data(DataEvent::from(AccountEvent::Deleted(ALICE_ID.clone())).into());
        let summary = EventSummary::from_event(&event_box);
        assert_eq!(summary.label, "Account deleted");
        let detail = summary.detail.expect("detail");
        let alice_literal = ALICE_ID.to_string();
        assert!(
            detail.contains(&alice_literal),
            "detail `{detail}` should mention {alice_literal}"
        );
    }

    #[test]
    fn account_controller_replaced_summary_mentions_old_and_new_controllers() {
        let event_box = EventBox::Data(
            DataEvent::from(AccountEvent::ControllerReplaced(
                AccountControllerReplaced {
                    account: ALICE_ID.clone(),
                    previous_account: BOB_ID.clone(),
                    previous_controller: iroha_data_model::account::AccountController::single(
                        BOB_KEYPAIR.public_key().clone(),
                    ),
                    new_controller: iroha_data_model::account::AccountController::single(
                        ALICE_KEYPAIR.public_key().clone(),
                    ),
                },
            ))
            .into(),
        );

        let summary = EventSummary::from_event(&event_box);
        assert_eq!(summary.label, "Account controller replaced");
        let detail = summary.detail.expect("detail");
        assert!(
            detail.contains(&ALICE_ID.to_string()) && detail.contains(&BOB_ID.to_string()),
            "detail `{detail}` should mention both account ids"
        );
        assert!(
            detail.contains("previous_controller=single")
                && detail.contains("new_controller=single"),
            "detail `{detail}` should mention both controller summaries"
        );
    }

    #[test]
    fn account_recovery_policy_summary_mentions_alias_and_quorum() {
        let alias = iroha_data_model::account::AccountAlias::domainless(
            "primary".parse().expect("valid alias label"),
            iroha_data_model::nexus::DataSpaceId::new(7),
        );
        let policy = iroha_data_model::account::AccountRecoveryPolicy::new(
            vec![iroha_data_model::account::RecoveryGuardian::new(
                BOB_ID.clone(),
                1,
            )],
            1,
            std::num::NonZeroU64::new(60_000).expect("non-zero timelock"),
        )
        .expect("valid recovery policy");
        let event_box = EventBox::Data(
            DataEvent::from(AccountEvent::Recovery(AccountRecoveryEvent::PolicySet(
                AccountRecoveryPolicySet {
                    account: ALICE_ID.clone(),
                    alias,
                    policy,
                },
            )))
            .into(),
        );

        let summary = EventSummary::from_event(&event_box);
        assert_eq!(summary.label, "Account recovery policy set");
        let detail = summary.detail.expect("detail");
        assert!(
            detail.contains(&ALICE_ID.to_string())
                && detail.contains("label=primary")
                && detail.contains("quorum=1")
                && detail.contains("timelock_ms=60000"),
            "detail `{detail}` should summarize the recovery policy"
        );
    }

    #[test]
    fn offline_settled_summary_includes_bundle_and_amount() {
        let payload = iroha_data_model::offline::OfflineVerdictRevocation {
            verdict_id: Hash::prehashed([1_u8; Hash::LENGTH]),
            issuer: ALICE_ID.clone(),
            revoked_at_ms: 1_234,
            reason: iroha_data_model::offline::OfflineVerdictRevocationReason::IssuerRequest,
            note: Some("bundle_id=demo amount=42".to_owned()),
            metadata: iroha_data_model::metadata::Metadata::default(),
        };
        let offline_event = offline::OfflineTransferEvent::RevocationImported(payload);
        let event_box = EventBox::Data(DataEvent::Offline(offline_event).into());
        let summary = EventSummary::from_event(&event_box);
        assert_eq!(summary.label, "Offline revocation imported");
        let detail = summary.detail.expect("detail");
        assert!(
            detail.contains("bundle_id") && detail.contains("amount"),
            "detail `{detail}` should include bundle id and amount markers"
        );
    }

    fn empty_metrics_snapshot() -> ToriiMetricsSnapshot {
        ToriiMetricsSnapshot {
            timestamp: Instant::now(),
            queue_size: None,
            view_changes: None,
            sumeragi_tx_queue_depth: None,
            sumeragi_tx_queue_capacity: None,
            sumeragi_tx_queue_saturated: None,
            state_tiered_hot_entries: None,
            state_tiered_cold_entries: None,
            state_tiered_cold_bytes: None,
            uptime_since_genesis_ms: None,
        }
    }
}
